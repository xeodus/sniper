use crate::{
    data::{Candles, OrderReq, OrderType, Position, PositionSide, Side, Signal, TradingBot},
    db::Database,
    notification::NotificationService,
    position_manager::PositionManager,
    rest_client::BinanceClient,
    signal::MarketSignal,
};
use anyhow::Result;
use chrono::Utc;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

#[allow(dead_code)]
impl TradingBot {
    pub fn new(
        signal_tx: mpsc::Sender<Signal>,
        order_tx: mpsc::Sender<OrderReq>,
        initial_balance: Decimal,
        binance_client: Arc<BinanceClient>,
        db: Arc<Database>,
        notification: Arc<NotificationService>,
    ) -> Result<Self> {
        let position_manager = Arc::new(PositionManager::new(Decimal::new(2, 2), db.clone()));
        Ok(Self {
            analyzer: Arc::new(RwLock::new(MarketSignal::new())),
            position_manager,
            signal_tx,
            order_tx,
            binance_client,
            account_balance: Arc::new(RwLock::new(initial_balance)),
            db,
            notification,
        })
    }

    pub async fn initializer(&self) -> Result<()> {
        self.position_manager.load_open_orders().await?;
        Ok(())
    }

    pub async fn process_candle(&self, candle: Candles, symbol: &str) -> Result<()> {
        // Update the analyzer with the new candle
        {
            let mut analyzer = self.analyzer.write().await;
            analyzer.add_candles(candle.clone());
        }

        // Check if any positions need to be closed (stop loss or take profit hit)
        let positions_to_close = self
            .position_manager
            .check_positions(candle.close, symbol)
            .await;

        // Close positions that hit their stop loss or take profit
        for (position_id, current_price, position_side) in positions_to_close {
            if let Some(position) = self
                .position_manager
                .get_positions_by_id(&position_id)
                .await
            {
                let exit_side = match position_side {
                    PositionSide::Long => Side::Sell,
                    PositionSide::Short => Side::Buy,
                };

                let req = OrderReq {
                    id: position_id.to_string(),
                    symbol: symbol.to_string(),
                    side: exit_side,
                    price: current_price,
                    size: position.size,
                    order_type: OrderType::Market,
                    sl: None,
                    tp: None,
                    manual: false,
                };

                match self.execute_order(&req).await {
                    Ok(_) => {
                        info!("Order succeeded, closing position...");
                        let pnl = match position.position_side {
                            PositionSide::Long => {
                                (current_price - position.entry_price) * position.size
                            }
                            PositionSide::Short => {
                                (position.entry_price - current_price) * position.size
                            }
                        };

                        if let Err(e) = self
                            .position_manager
                            .close_positions(&position_id, current_price)
                            .await
                        {
                            error!("Failed to close position in database: {}", e);
                        }

                        // Send notification
                        if let Err(e) = self
                            .notification
                            .notify_position_closed(&position, current_price, pnl)
                            .await
                        {
                            warn!("Failed to send position closed notification: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to place order: {}", e);
                        let _ = self
                            .notification
                            .notify_error(&format!("Failed to place close order: {}", e))
                            .await;
                    }
                }
            }
        }

        // Analyze market and generate signals (independent of position closing)
        let signal_opt = {
            let analyzer = self.analyzer.read().await;
            analyzer.analyze(symbol.to_string())
        };

        if let Some(signal) = signal_opt {
            // Save signal to database
            if let Err(e) = self.db.save_signal(&signal).await {
                warn!("Failed to save signal onto database: {}", e);
            }

            // Send signal through channel
            if let Err(e) = self.signal_tx.send(signal.clone()).await {
                warn!("Failed to send signal: {}", e);
            }

            // Execute trades based on signal confidence
            let confidence_threshold = Decimal::new(70, 2);

            if signal.confidence >= confidence_threshold {
                // Notify about high confidence signal
                if let Err(e) = self.notification.notify_signal(&signal).await {
                    warn!("Failed to send signal notification: {}", e);
                }

                match signal.action {
                    Side::Buy => {
                        // Only open new position if we don't already have one
                        if !self.position_manager.has_position_for_symbol(symbol).await {
                            if let Err(e) = self
                                .execute_entry_order(&signal, PositionSide::Long, OrderType::Market)
                                .await
                            {
                                error!("Failed to place buy order: {}", e);
                            }
                        }
                    }
                    Side::Sell => {
                        // Only open short position if we don't already have one
                        if !self.position_manager.has_position_for_symbol(symbol).await {
                            if let Err(e) = self
                                .execute_entry_order(
                                    &signal,
                                    PositionSide::Short,
                                    OrderType::Market,
                                )
                                .await
                            {
                                error!("Failed to place sell order: {}", e);
                            }
                        }
                    }
                    Side::Hold => {
                        info!("Unclear trend detected, holding positions...");
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn execute_entry_order(
        &self,
        signal: &Signal,
        position_side: PositionSide,
        order_type: OrderType,
    ) -> Result<()> {
        let account_balance = *self.account_balance.read().await;

        let (take_profit, stop_loss) = match position_side {
            PositionSide::Long => (
                signal.price * Decimal::new(104, 2), // 4% profit
                signal.price * Decimal::new(98, 2),  // 2% loss
            ),
            PositionSide::Short => (
                signal.price * Decimal::new(96, 2),  // 4% profit (price goes down)
                signal.price * Decimal::new(102, 2), // 2% loss (price goes up)
            ),
        };

        let position_size = self
            .position_manager
            .calculate_position_size(account_balance, signal.price, stop_loss)
            .await;

        if position_size <= Decimal::ZERO {
            warn!("Invalid position size calculated, skipping order");
            return Ok(());
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

        match self.execute_order(&order).await {
            Ok(_) => {
                self.position_manager
                    .open_position(position.clone(), false)
                    .await?;
                info!(
                    "Position opened: {} {} @ {} (SL: {}, TP: {})",
                    signal.symbol, signal.action, signal.price, stop_loss, take_profit
                );

                // Send notification
                if let Err(e) = self.notification.notify_position_opened(&position).await {
                    warn!("Failed to send position opened notification: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to execute order: {}", e);
                let _ = self
                    .notification
                    .notify_error(&format!("Failed to execute order: {}", e))
                    .await;
            }
        }

        Ok(())
    }

    pub async fn execute_order(&self, order: &OrderReq) -> Result<()> {
        match order.order_type {
            OrderType::Limit => {
                self.binance_client.place_limit_order(order).await?;
                info!("Placed limit order for: {}", order.id);
            }
            OrderType::Market => {
                self.binance_client.place_market_order(order).await?;
                info!("Placed market order for: {}", order.id);
            }
        }

        // Send order to monitoring channel
        if let Err(e) = self.order_tx.send(order.clone()).await {
            warn!("Failed to send order to monitor: {}", e);
        }

        Ok(())
    }

    pub async fn update_balance(&self, new_balance: Decimal) {
        let mut balance = self.account_balance.write().await;
        *balance = new_balance;
    }

    pub async fn get_balance(&self) -> Decimal {
        *self.account_balance.read().await
    }
}
