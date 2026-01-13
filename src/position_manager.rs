use crate::{
    data::{Position, PositionSide},
    db::Database,
};
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub struct PositionManager {
    /// Use HashMap for O(1) position lookups by ID
    pub positions: Arc<RwLock<HashMap<String, Position>>>,
    /// Positions by symbol for efficient filtering
    pub positions_by_symbol: Arc<RwLock<HashMap<String, Vec<String>>>>,
    pub risk_per_trade: Decimal,
    pub db: Arc<Database>,
}

impl PositionManager {
    pub fn new(risk_per_trade: Decimal, db: Arc<Database>) -> Self {
        Self {
            positions: Arc::new(RwLock::new(HashMap::new())),
            positions_by_symbol: Arc::new(RwLock::new(HashMap::new())),
            risk_per_trade,
            db,
        }
    }

    pub async fn load_open_orders(&self) -> Result<()> {
        let positions_vec = self.db.get_open_orders().await?;
        let count = positions_vec.len();
        
        let mut positions = self.positions.write().await;
        let mut by_symbol = self.positions_by_symbol.write().await;
        
        positions.clear();
        by_symbol.clear();
        
        for position in positions_vec {
            let symbol = position.symbol.clone();
            let id = position.id.clone();
            
            positions.insert(id.clone(), position);
            by_symbol.entry(symbol).or_insert_with(Vec::new).push(id);
        }

        info!("Loaded {} open positions into memory", count);
        Ok(())
    }

    pub async fn get_position_by_id(&self, position_id: &str) -> Option<Position> {
        let positions = self.positions.read().await;
        positions.get(position_id).cloned()
    }

    pub async fn has_positions(&self) -> bool {
        let positions = self.positions.read().await;
        !positions.is_empty()
    }

    pub async fn get_positions_count(&self) -> usize {
        let positions = self.positions.read().await;
        positions.len()
    }

    pub async fn open_position(&self, position: Position, manual: bool) -> Result<()> {
        if position.entry_price == Decimal::ZERO || position.size == Decimal::ZERO {
            info!("Attempt to open position with size zero, rejected...");
            return Ok(());
        }

        // Check if position already exists
        {
            let positions = self.positions.read().await;
            if positions.contains_key(&position.id) {
                info!("Position {} already exists, skipping...", position.id);
                return Ok(());
            }
        }

        self.db.save_order(&position, manual).await?;
        
        let mut positions = self.positions.write().await;
        let mut by_symbol = self.positions_by_symbol.write().await;
        
        let id = position.id.clone();
        let symbol = position.symbol.clone();
        
        positions.insert(id.clone(), position);
        by_symbol.entry(symbol).or_insert_with(Vec::new).push(id.clone());

        info!("New position opened: {}", id);
        Ok(())
    }

    pub async fn close_position(&self, position_id: &str, exit_price: Decimal) -> Result<()> {
        let position = {
            let positions = self.positions.read().await;
            positions.get(position_id).cloned()
        };

        if let Some(pos) = position {
            let pnl = match pos.position_side {
                PositionSide::Long => (exit_price - pos.entry_price) * pos.size,
                PositionSide::Short => (pos.entry_price - exit_price) * pos.size,
            };
            
            self.db.close_order(position_id, exit_price, pnl).await?;
            
            let mut positions = self.positions.write().await;
            let mut by_symbol = self.positions_by_symbol.write().await;
            
            positions.remove(position_id);
            if let Some(symbol_positions) = by_symbol.get_mut(&pos.symbol) {
                symbol_positions.retain(|id| id != position_id);
                if symbol_positions.is_empty() {
                    by_symbol.remove(&pos.symbol);
                }
            }
            
            info!(
                "Closed position {} at price {} with PnL: {}",
                position_id, exit_price, pnl
            );
            Ok(())
        } else {
            Err(anyhow!("Position {} not found", position_id))
        }
    }

    /// Check positions for stop loss or take profit triggers
    /// Returns list of (position_id, exit_price, position_side) tuples
    pub async fn check_positions(
        &self,
        current_price: Decimal,
        symbol: &str,
    ) -> Vec<(String, Decimal, PositionSide)> {
        let positions = self.positions.read().await;
        let by_symbol = self.positions_by_symbol.read().await;
        
        let mut to_close = Vec::new();
        
        // Get position IDs for this symbol (O(1) lookup)
        if let Some(position_ids) = by_symbol.get(symbol) {
            for position_id in position_ids {
                if let Some(position) = positions.get(position_id) {
                    let should_close = match position.position_side {
                        PositionSide::Long => {
                            if current_price <= position.stop_loss {
                                info!(
                                    "Stop loss triggered for Long position {} at price: {}",
                                    position.id, current_price
                                );
                                true
                            } else if current_price >= position.take_profit {
                                info!(
                                    "Take profit triggered for Long position {} at price: {}",
                                    position.id, current_price
                                );
                                true
                            } else {
                                false
                            }
                        }
                        PositionSide::Short => {
                            if current_price >= position.stop_loss {
                                info!(
                                    "Stop loss triggered for Short position {} at price: {}",
                                    position.id, current_price
                                );
                                true
                            } else if current_price <= position.take_profit {
                                info!(
                                    "Take profit triggered for Short position {} at price: {}",
                                    position.id, current_price
                                );
                                true
                            } else {
                                false
                            }
                        }
                    };
                    
                    if should_close {
                        to_close.push((position.id.clone(), current_price, position.position_side));
                    }
                }
            }
        }
        
        to_close
    }
    
    /// Get all positions for a symbol
    pub async fn get_positions_by_symbol(&self, symbol: &str) -> Vec<Position> {
        let positions = self.positions.read().await;
        let by_symbol = self.positions_by_symbol.read().await;
        
        if let Some(position_ids) = by_symbol.get(symbol) {
            position_ids
                .iter()
                .filter_map(|id| positions.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

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
}
