use crate::{
    data::{Position, PositionSide},
    db::Database,
};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub struct PositionManager {
    pub positions: Arc<RwLock<Vec<Position>>>,
    pub risk_per_trade: Decimal,
    pub db: Arc<Database>,
}

#[allow(dead_code)]
impl PositionManager {
    pub fn new(risk_per_trade: Decimal, db: Arc<Database>) -> Self {
        Self {
            positions: Arc::new(RwLock::new(Vec::new())),
            risk_per_trade,
            db,
        }
    }

    /// Load open orders from database into memory
    pub async fn load_open_orders(&self) -> Result<()> {
        let db_positions = self.db.get_open_orders().await?;
        let count = db_positions.len();
        let mut positions = self.positions.write().await;
        *positions = db_positions;

        info!("Loaded {} open positions from database", count);
        Ok(())
    }

    /// Get a position by its ID
    pub async fn get_positions_by_id(&self, position_id: &str) -> Option<Position> {
        let positions = self.positions.read().await;
        positions.iter().find(|p| p.id == position_id).cloned()
    }

    /// Check if there are any open positions
    pub async fn has_positions(&self) -> bool {
        let positions = self.positions.read().await;
        !positions.is_empty()
    }

    /// Check if there's an open position for a specific symbol
    pub async fn has_position_for_symbol(&self, symbol: &str) -> bool {
        let positions = self.positions.read().await;
        // Normalize symbol comparison (remove slashes, compare uppercase)
        let normalized = symbol.replace("/", "").to_uppercase();
        positions
            .iter()
            .any(|p| p.symbol.replace("/", "").to_uppercase() == normalized)
    }

    /// Get all positions for a symbol
    pub async fn get_positions_for_symbol(&self, symbol: &str) -> Vec<Position> {
        let positions = self.positions.read().await;
        let normalized = symbol.replace("/", "").to_uppercase();
        positions
            .iter()
            .filter(|p| p.symbol.replace("/", "").to_uppercase() == normalized)
            .cloned()
            .collect()
    }

    /// Open a new position
    pub async fn open_position(&self, position: Position, manual: bool) -> Result<()> {
        if position.entry_price == Decimal::ZERO || position.size == Decimal::ZERO {
            info!("Attempt to open position with zero price or size, rejected");
            return Ok(());
        }

        // Check if we already have a position for this symbol
        if self.has_position_for_symbol(&position.symbol).await {
            info!(
                "Already have an open position for {}, skipping",
                position.symbol
            );
            return Ok(());
        }

        // Save to database first
        self.db.save_order(&position, manual).await?;

        // Add to in-memory list
        let mut positions = self.positions.write().await;
        positions.push(position.clone());

        info!(
            "New position opened: {} {:?} @ {} (Size: {})",
            position.symbol, position.position_side, position.entry_price, position.size
        );

        Ok(())
    }

    /// Close a position
    pub async fn close_positions(&self, position_id: &str, exit_price: Decimal) -> Result<()> {
        let mut positions = self.positions.write().await;

        let position = positions
            .iter()
            .find(|p| p.id == position_id)
            .ok_or_else(|| anyhow!("Position {} not found", position_id))?;

        // Calculate PnL
        let pnl = match position.position_side {
            PositionSide::Long => (exit_price - position.entry_price) * position.size,
            PositionSide::Short => (position.entry_price - exit_price) * position.size,
        };

        // Update database
        self.db.close_order(position_id, exit_price, pnl).await?;

        info!(
            "Closed position {}: exit price: {}, PnL: {}",
            position_id, exit_price, pnl
        );

        // Remove from in-memory list
        positions.retain(|p| p.id != position_id);

        Ok(())
    }

    /// Check positions for stop loss or take profit triggers
    pub async fn check_positions(
        &self,
        current_price: Decimal,
        symbol: &str,
    ) -> Vec<(String, Decimal, PositionSide)> {
        let positions = self.positions.read().await;
        let mut to_close = Vec::new();

        let normalized_symbol = symbol.replace("/", "").to_uppercase();

        for position in positions.iter() {
            let pos_symbol = position.symbol.replace("/", "").to_uppercase();
            if pos_symbol != normalized_symbol {
                continue;
            }

            match position.position_side {
                PositionSide::Long => {
                    if current_price <= position.stop_loss {
                        to_close.push((position.id.clone(), current_price, position.position_side));
                        info!(
                            "Stop loss triggered for Long position {}: current {} <= stop {}",
                            position.id, current_price, position.stop_loss
                        );
                    } else if current_price >= position.take_profit {
                        to_close.push((position.id.clone(), current_price, position.position_side));
                        info!(
                            "Take profit triggered for Long position {}: current {} >= tp {}",
                            position.id, current_price, position.take_profit
                        );
                    }
                }
                PositionSide::Short => {
                    if current_price >= position.stop_loss {
                        to_close.push((position.id.clone(), current_price, position.position_side));
                        info!(
                            "Stop loss triggered for Short position {}: current {} >= stop {}",
                            position.id, current_price, position.stop_loss
                        );
                    } else if current_price <= position.take_profit {
                        to_close.push((position.id.clone(), current_price, position.position_side));
                        info!(
                            "Take profit triggered for Short position {}: current {} <= tp {}",
                            position.id, current_price, position.take_profit
                        );
                    }
                }
            }
        }

        to_close
    }

    /// Calculate position size based on risk management
    pub async fn calculate_position_size(
        &self,
        account_balance: Decimal,
        entry_price: Decimal,
        stop_loss: Decimal,
    ) -> Decimal {
        let risk_amount = account_balance * self.risk_per_trade;
        let risk_per_unit = (entry_price - stop_loss).abs();

        if risk_per_unit == Decimal::ZERO {
            return Decimal::ZERO;
        }

        risk_amount / risk_per_unit
    }

    /// Get total unrealized PnL
    pub async fn get_unrealized_pnl(&self, current_prices: &[(String, Decimal)]) -> Decimal {
        let positions = self.positions.read().await;
        let mut total_pnl = Decimal::ZERO;

        for position in positions.iter() {
            let normalized_symbol = position.symbol.replace("/", "").to_uppercase();
            if let Some((_, price)) = current_prices
                .iter()
                .find(|(s, _)| s.replace("/", "").to_uppercase() == normalized_symbol)
            {
                let pnl = match position.position_side {
                    PositionSide::Long => (*price - position.entry_price) * position.size,
                    PositionSide::Short => (position.entry_price - *price) * position.size,
                };
                total_pnl += pnl;
            }
        }

        total_pnl
    }

    /// Get count of open positions
    pub async fn position_count(&self) -> usize {
        let positions = self.positions.read().await;
        positions.len()
    }
}
