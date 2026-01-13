use crate::{
    data::{Candles, OrderReq, OrderType, Position, PositionSide, Side, Signal, TradingBot},
    db::Database,
    exchange::Exchange,
    grid::GridStrategy,
    position_manager::PositionManager,
    signal::MarketSignal,
    volatility::VolatilityStrategy,
};
use anyhow::Result;
use chrono::Utc;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

impl TradingBot {
    pub fn new(
        signal_tx: mpsc::Sender<Signal>,
        order_tx: mpsc::Sender<OrderReq>,
        initial_balance: Decimal,
        exchange: Arc<dyn Exchange>,
        db: Arc<Database>,
    ) -> Result<Self> {
        let position_manager = Arc::new(PositionManager::new(Decimal::new(2, 2), db.clone()));
        Ok(Self {
            analyzer: Arc::new(RwLock::new(MarketSignal::new())),
            position_manager,
            signal_tx,
            order_tx,
            exchange,
            account_balance: Arc::new(RwLock::new(initial_balance)),
            db,
        })
    }

    pub async fn initializer(&self) -> Result<()> {
        self.position_manager.load_open_orders().await?;
        Ok(())
    }

    /// Main processing function - processes each candle with proper separation of concerns
    pub async fn process_candle(&self, candle: Candles, symbol: &str) -> Result<()> {
        // 1. Update market analyzer with new candle
        {
            let mut analyzer = self.analyzer.write().await;
            analyzer.add_candles(candle.clone());
        }

        // 2. Check for positions that need to be closed (stop loss / take profit)
        let positions_to_close = self
            .position_manager
            .check_positions(candle.close, symbol)
            .await;

        // 3. Close positions that hit stop loss or take profit
        for (position_id, exit_price, _position_side) in positions_to_close {
            if let Some(position) = self
                .position_manager
                .get_position_by_id(&position_id)
                .await
            {
                let exit_side = match position.position_side {
                    PositionSide::Long => Side::Sell,
                    PositionSide::Short => Side::Buy,
                };

                let req = OrderReq {
                    id: position_id.clone(),
                    symbol: symbol.to_string(),
                    side: exit_side,
                    price: exit_price,
                    size: position.size,
                    order_type: OrderType::Market, // Use market for immediate execution
                    sl: None,
                    tp: None,
                    manual: false,
                };

                match self.execute_order(req).await {
                    Ok(_) => {
                        info!("Order succeeded, closing position...");
                        if let Err(e) = self
                            .position_manager
                            .close_position(&position_id, exit_price)
                            .await
                        {
                            error!("Failed to close position in database: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to place exit order: {}", e);
                    }
                }
            }
        }

        // 4. Generate trading signals (separated from position management)
        let signal_opt = {
            let analyzer = self.analyzer.read().await;
            analyzer.analyze(symbol.to_string())
        };

        // 5. Process signal if generated
        if let Some(signal) = signal_opt {
            // Save signal to database
            if let Err(e) = self.db.save_signal(signal.clone()).await {
                warn!("Failed to save signal to database: {}", e);
            }

            // Send signal to monitoring channel
            if let Err(e) = self.signal_tx.send(signal.clone()).await {
                warn!("Failed to send signal: {}", e);
            }

            // Execute trades based on signal (if confidence is high enough)
            let confidence_threshold = Decimal::new(70, 2);
            if signal.confidence >= confidence_threshold {
                let position_side = match signal.action {
                    Side::Buy => PositionSide::Long,
                    Side::Sell => PositionSide::Short,
                    Side::Hold => return Ok(()), // Don't trade on hold signals
                };

                if let Err(e) = self
                    .execute_entry_order(signal, position_side, OrderType::Market)
                    .await
                {
                    error!("Failed to execute entry order: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Process candle with grid trading strategy
    pub async fn process_candle_with_grid(
        &self,
        candle: Candles,
        symbol: &str,
        grid_strategy: &mut GridStrategy,
        volatility_strategy: &mut VolatilityStrategy,
    ) -> Result<()> {
        // Update volatility calculator
        volatility_strategy.update(candle.clone());

        // Update grid with current price and volatility
        let volatility = volatility_strategy.get_volatility();
        grid_strategy.update(candle.close, volatility);

        // Check if we should use grid trading (low volatility) or momentum (high volatility)
        if volatility_strategy.should_use_grid() {
            // Grid trading mode - place orders at grid levels
            let unfilled_buys = grid_strategy.get_unfilled_buys();
            let unfilled_sells = grid_strategy.get_unfilled_sells();

            // Place buy orders at grid levels below current price
            for level in unfilled_buys.iter().take(3) {
                // Limit to 3 orders to avoid over-trading
                let req = OrderReq {
                    id: level.id.clone(),
                    symbol: symbol.to_string(),
                    side: Side::Buy,
                    price: level.price,
                    size: level.quantity,
                    order_type: OrderType::Limit,
                    sl: None,
                    tp: None,
                    manual: false,
                };

                if let Err(e) = self.execute_order(req).await {
                    warn!("Failed to place grid buy order: {}", e);
                }
            }

            // Place sell orders at grid levels above current price
            for level in unfilled_sells.iter().take(3) {
                let req = OrderReq {
                    id: level.id.clone(),
                    symbol: symbol.to_string(),
                    side: Side::Sell,
                    price: level.price,
                    size: level.quantity,
                    order_type: OrderType::Limit,
                    sl: None,
                    tp: None,
                    manual: false,
                };

                if let Err(e) = self.execute_order(req).await {
                    warn!("Failed to place grid sell order: {}", e);
                }
            }
        } else {
            // High volatility mode - use momentum/trend following
            self.process_candle(candle, symbol).await?;
        }

        Ok(())
    }

    pub async fn execute_entry_order(
        &self,
        signal: Signal,
        position_side: PositionSide,
        order_type: OrderType,
    ) -> Result<()> {
        let account_balance = *self.account_balance.read().await;

        // Calculate stop loss and take profit based on position side
        let (take_profit, stop_loss) = match position_side {
            PositionSide::Long => (
                signal.price * Decimal::new(104, 2), // 4% profit target
                signal.price * Decimal::new(98, 2),  // 2% stop loss
            ),
            PositionSide::Short => (
                signal.price * Decimal::new(96, 2),  // 4% profit target
                signal.price * Decimal::new(102, 2), // 2% stop loss
            ),
        };

        let position_size = self
            .position_manager
            .calculate_position_size(account_balance, signal.price, stop_loss)
            .await;

        if position_size <= Decimal::ZERO {
            return Err(anyhow::anyhow!("Invalid position size calculated"));
        }

        let order = OrderReq {
            id: signal.id.clone(),
            symbol: signal.symbol.clone(),
            side: signal.action.clone(),
            price: signal.price,
            size: position_size,
            order_type,
            tp: Some(take_profit),
            sl: Some(stop_loss),
            manual: false,
        };

        let position = Position {
            id: signal.id.clone(),
            symbol: signal.symbol.clone(),
            entry_price: signal.price,
            size: position_size,
            position_side,
            opened_at: Utc::now().timestamp(),
            take_profit,
            stop_loss,
        };

        match self.execute_order(order).await {
            Ok(_) => {
                self.position_manager.open_position(position, false).await?;
                info!("Position opened successfully!");
            }
            Err(e) => {
                warn!("Failed to execute order: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    pub async fn execute_order(&self, order: OrderReq) -> Result<()> {
        match order.order_type {
            OrderType::Limit => {
                self.exchange.place_limit_order(&order).await?;
                info!("Placed limit order: {}", order.id);
            }
            OrderType::Market => {
                self.exchange.place_market_order(&order).await?;
                info!("Placed market order: {}", order.id);
            }
        }

        // Send order to monitoring channel
        if let Err(e) = self.order_tx.send(order).await {
            warn!("Failed to send order to monitoring channel: {}", e);
        }

        Ok(())
    }
}
