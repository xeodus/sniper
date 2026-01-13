use crate::{
    config::TradingConfig,
    data::{OrderReq, Signal, TradingBot},
    db::Database,
    exchange::{ExchangeFactory, ExchangeType},
    grid::GridStrategy,
    volatility::VolatilityStrategy,
    websocket::WebSocketClient,
};
use anyhow::{anyhow, Result};
use dotenv::dotenv;
use futures_util::{pin_mut, FutureExt, StreamExt};
use rust_decimal::Decimal;
use std::env;
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{interval, sleep, Duration},
};
use tracing::{error, info, warn};

mod config;
mod data;
mod db;
mod engine;
mod exchange;
mod grid;
mod notification;
mod position_manager;
mod rest_client;
mod sign;
mod signal;
mod volatility;
mod websocket;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt().init();

    info!("Starting latency-critical trading engine...");

    // Load environment variables
    let api_key = env::var("API_KEY")
        .map_err(|_| anyhow!("API_KEY not found in environment"))?;
    let secret_key = env::var("SECRET_KEY")
        .map_err(|_| anyhow!("SECRET_KEY not found in environment"))?;
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| anyhow!("DATABASE_URL not set in environment"))?;

    // Load configuration
    let trading_config = TradingConfig::from_file("config.json")
        .unwrap_or_else(|_| {
            warn!("Failed to load config.json, using defaults");
            TradingConfig::default()
        });

    // Initialize database
    let db = Arc::new(Database::new(&database_url).await?);
    
    // Try to load historical data (optional, don't fail if empty)
    if let Err(e) = db.load_from_db().await {
        warn!("No historical data available: {}", e);
    }

    // Create exchange instance
    let exchange: Arc<dyn crate::exchange::Exchange> = 
        ExchangeFactory::create_cex(ExchangeType::Binance, api_key, secret_key, true)?;

    // Create channels for signals and orders
    let (signal_tx, mut signal_rx) = mpsc::channel::<Signal>(1000);
    let (order_tx, mut order_rx) = mpsc::channel::<OrderReq>(1000);

    // Get initial balance
    let initial_balance = exchange.account_balance().await.unwrap_or(Decimal::new(10000, 0));
    info!("Initial account balance: {}", initial_balance);

    // Create trading bot
    let bot = Arc::new(TradingBot::new(
        signal_tx.clone(),
        order_tx.clone(),
        initial_balance,
        exchange.clone(),
        db.clone(),
    )?);

    // Initialize bot
    bot.initializer().await?;
    info!("Trading bot initialized!");

    // Initialize grid and volatility strategies if configured
    let mut grid_strategy: Option<GridStrategy> = None;
    let mut volatility_strategy: Option<VolatilityStrategy> = None;

    if let Some(grid_cfg) = &trading_config.grid {
        if grid_cfg.enabled {
            // Get current price to initialize grid
            let current_price = exchange
                .get_price(&trading_config.symbol)
                .await
                .unwrap_or(Decimal::new(2000, 0)); // Fallback price
            
            if let Some(grid_config) = trading_config.to_grid_config(current_price) {
                grid_strategy = Some(GridStrategy::new(grid_config, current_price));
                info!("Grid trading strategy initialized");
            }
        }
    }

    if let Some(vol_cfg) = &trading_config.volatility {
        volatility_strategy = Some(VolatilityStrategy::new(
            vol_cfg.window_size,
            vol_cfg.high_vol_threshold,
            vol_cfg.low_vol_threshold,
            vol_cfg.percentile_threshold,
        ));
        info!("Volatility strategy initialized");
    }

    // Spawn signal monitoring task
    let signal_monitor = tokio::spawn(async move {
        while let Some(signal) = signal_rx.recv().await {
            info!(
                "Signal: {:?} {} @ {} (confidence: {:.2}%)",
                signal.action,
                signal.symbol,
                signal.price,
                signal.confidence * Decimal::new(100, 0)
            );
        }
    });

    // Spawn order monitoring task
    let order_monitor = tokio::spawn(async move {
        while let Some(order) = order_rx.recv().await {
            info!(
                "Order: {:?} {:?} {} @ {}",
                order.side, order.order_type, order.symbol, order.price
            );
        }
    });

    // WebSocket connection - clone strings before moving into async block
    let symbol = trading_config.symbol.clone();
    let symbol_lower = symbol.to_lowercase().replace("/", "");
    let timeframe = trading_config.timeframe.clone();

    info!("Connecting to market data for: {} ({})", symbol, timeframe);

    let bot_clone = bot.clone();
    let grid_strategy_arc = Arc::new(tokio::sync::Mutex::new(grid_strategy));
    let volatility_strategy_arc = Arc::new(tokio::sync::Mutex::new(volatility_strategy));

    let ws_handler = tokio::spawn(async move {
        let mut backoff = Duration::from_millis(100);
        let max_backoff = Duration::from_secs(10);
        let ws = WebSocketClient::new(&symbol_lower, &timeframe);
        let mut balance_interval = interval(Duration::from_secs(60)); // Check balance every minute

        loop {
            let stream = match ws.connect().await {
                Ok(s) => {
                    info!("WebSocket connected!");
                    backoff = Duration::from_millis(100);
                    s
                }
                Err(e) => {
                    error!("WebSocket connection failed: {}", e);
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                    continue;
                }
            };

            pin_mut!(stream);

            while let Some(candle_result) = stream.next().await {
                // Check balance periodically (non-blocking)
                if balance_interval.tick().now_or_never().is_some() {
                    let exchange_clone = exchange.clone();
                    tokio::spawn(async move {
                        match exchange_clone.account_balance().await {
                            Ok(balance) => {
                                info!("Account balance: {}", balance);
                            }
                            Err(e) => {
                                warn!("Failed to get account balance: {}", e);
                            }
                        }
                    });
                }

                match candle_result {
                    Ok(candle) => {
                        // Process candle with grid/volatility if available
                        let mut grid_guard = grid_strategy_arc.lock().await;
                        let mut vol_guard = volatility_strategy_arc.lock().await;

                        if let (Some(ref mut grid), Some(ref mut vol)) = (grid_guard.as_mut(), vol_guard.as_mut()) {
                            if let Err(e) = bot_clone
                                .process_candle_with_grid(candle.clone(), &symbol, grid, vol)
                                .await
                            {
                                error!("Error processing candle with grid: {}", e);
                            }
                        } else {
                            // Fallback to standard processing
                            drop(grid_guard);
                            drop(vol_guard);
                            if let Err(e) = bot_clone.process_candle(candle, &symbol).await {
                                error!("Error processing candle: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("WebSocket stream error: {}", e);
                        break;
                    }
                }
            }

            warn!("WebSocket stream ended, reconnecting in {:?}...", backoff);
            sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, max_backoff);
        }
    });

    info!("Trading engine running. Press Ctrl+C to exit.");

    tokio::select! {
        result = signal_monitor => {
            error!("Signal monitoring stopped: {:?}", result);
        }
        result = order_monitor => {
            error!("Order monitoring stopped: {:?}", result);
        }
        result = ws_handler => {
            error!("WebSocket handler stopped: {:?}", result);
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Shutdown signal received");
        }
    }

    info!("Shutting down trading engine...");
    Ok(())
}
