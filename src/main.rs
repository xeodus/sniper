use crate::{
    backtesting::BackTesting,
    config::Config,
    data::{Candles, OrderReq, Signal, TradingBot},
    db::Database,
    notification::NotificationService,
    rest_client::BinanceClient,
    websocket::WebSocketClient,
};
use anyhow::Result;
use dotenv::dotenv;
use futures_util::{pin_mut, StreamExt};
use rust_decimal::{prelude::FromPrimitive, Decimal};
use std::env;
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{interval, sleep, Duration},
};
use tracing::{error, info, warn};

mod backtesting;
mod config;
mod data;
mod db;
mod engine;
mod notification;
mod position_manager;
mod rest_client;
mod sign;
mod signal;
mod websocket;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt().init();

    info!("Starting Sniper Bot...");

    // Load configuration
    let config = Config::load().unwrap_or_else(|e| {
        warn!("Failed to load config, using defaults: {}", e);
        Config::default()
    });

    info!(
        "Configuration loaded: symbol={}, timeframe={}, testnet={}",
        config.symbol, config.timeframe, config.testnet
    );

    // Load credentials from environment
    let api_key = env::var("API_KEY").expect("API_KEY not found in environment");
    let secret_key = env::var("SECRET_KEY").expect("SECRET_KEY not found in environment");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set in environment");

    // Initialize services
    let db = Arc::new(Database::new(&database_url).await?);
    let notification = Arc::new(NotificationService::from_env());
    let binance_client = Arc::new(BinanceClient::new(api_key, secret_key, config.testnet));

    // Run backtest on historical data if available
    let historical_data: Vec<Candles> = db.load_from_db().await.unwrap_or_default();
    if !historical_data.is_empty() {
        info!(
            "Running backtest on {} historical candles...",
            historical_data.len()
        );
        let initial_capital = Decimal::from_i64(10_000).unwrap();
        let mut backtester = BackTesting::new(initial_capital);
        let result = backtester.run(historical_data, config.normalized_symbol());
        result.print_summary();
    } else {
        info!("No historical data found, skipping backtest");
    }

    // Create communication channels
    let (signal_tx, mut signal_rx) = mpsc::channel::<Signal>(100);
    let (order_tx, mut order_rx) = mpsc::channel::<OrderReq>(100);

    // Get initial balance
    let initial_balance = match binance_client.account_balance().await {
        Ok(balance) => {
            info!("Initial account balance: {} USDT", balance);
            balance
        }
        Err(e) => {
            warn!("Failed to get account balance, using default: {}", e);
            Decimal::new(1000, 0)
        }
    };

    // Initialize the trading bot
    let bot = Arc::new(TradingBot::new(
        signal_tx.clone(),
        order_tx,
        initial_balance,
        binance_client.clone(),
        db.clone(),
        notification.clone(),
    )?);

    bot.initializer().await?;

    info!("Trading bot initialized successfully!");

    // Send startup notification
    if let Err(e) = notification
        .notify_startup(&config.symbol, &config.timeframe)
        .await
    {
        warn!("Failed to send startup notification: {}", e);
    }

    // Signal monitoring task
    let signal_monitor = tokio::spawn(async move {
        while let Some(signal) = signal_rx.recv().await {
            info!(
                "📊 Signal: {:?} {} @ ${} | Confidence: {:.1}% | Trend: {:?}",
                signal.action,
                signal.symbol,
                signal.price,
                signal.confidence * Decimal::new(100, 0),
                signal.trend
            );
        }
    });

    // Order monitoring task
    let order_monitor = tokio::spawn(async move {
        while let Some(order) = order_rx.recv().await {
            info!(
                "📝 Order: {:?} {} | Size: {} @ ${}",
                order.side, order.symbol, order.size, order.price
            );
        }
    });

    // WebSocket handler
    let ws_symbol = config.ws_symbol();
    let timeframe = config.timeframe.clone();
    let symbol_display = config.symbol.clone();
    let bot_clone = bot.clone();
    let binance_client_clone = binance_client.clone();
    let notification_clone = notification.clone();

    let ws_handler = tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(30);
        let ws = WebSocketClient::new(&ws_symbol, &timeframe);
        let mut balance_check_interval = interval(Duration::from_secs(60));

        loop {
            // Connect to WebSocket
            let stream = match ws.connect().await {
                Ok(s) => {
                    info!("✅ WebSocket connected to {}", symbol_display);
                    backoff = Duration::from_secs(1);
                    s
                }
                Err(e) => {
                    error!("❌ WebSocket connection failed: {}", e);
                    let _ = notification_clone
                        .notify_error(&format!("WebSocket connection failed: {}", e))
                        .await;
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                    continue;
                }
            };

            pin_mut!(stream);

            loop {
                tokio::select! {
                    // Handle incoming candle data
                    candle_opt = stream.next() => {
                        match candle_opt {
                            Some(Ok(candle)) => {
                                // Log candle data
                                info!(
                                    "🕯️ {} | O: {} H: {} L: {} C: {} V: {}",
                                    symbol_display,
                                    candle.open,
                                    candle.high,
                                    candle.low,
                                    candle.close,
                                    candle.volume
                                );

                                // Process the candle
                                if let Err(e) = bot_clone.process_candle(candle, &symbol_display).await {
                                    error!("Failed to process candle: {}", e);
                                }
                            }
                            Some(Err(e)) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                warn!("WebSocket stream ended");
                                break;
                            }
                        }
                    }

                    // Periodic balance check
                    _ = balance_check_interval.tick() => {
                        match binance_client_clone.account_balance().await {
                            Ok(balance) => {
                                info!("💰 Account balance: {} USDT", balance);
                                bot_clone.update_balance(balance).await;
                            }
                            Err(e) => {
                                warn!("Failed to get account balance: {}", e);
                            }
                        }
                    }
                }
            }

            // Reconnect with exponential backoff
            warn!(
                "🔄 WebSocket disconnected, reconnecting in {:?}...",
                backoff
            );
            sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, max_backoff);
        }
    });

    info!("🚀 Bot is running! Press Ctrl+C to exit.");

    // Wait for shutdown signal
    tokio::select! {
        result = signal_monitor => {
            error!("Signal monitor stopped unexpectedly: {:?}", result);
        }
        result = order_monitor => {
            error!("Order monitor stopped unexpectedly: {:?}", result);
        }
        result = ws_handler => {
            error!("WebSocket handler stopped unexpectedly: {:?}", result);
        }
        _ = tokio::signal::ctrl_c() => {
            info!("⏹️ Shutdown signal received");
        }
    }

    // Send shutdown notification
    if let Err(e) = notification.notify_shutdown().await {
        warn!("Failed to send shutdown notification: {}", e);
    }

    info!("👋 Shutting down gracefully...");

    Ok(())
}
