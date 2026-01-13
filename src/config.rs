use crate::grid::{GridConfig, GridSpacing};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fs;

/// Main trading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub symbol: String,
    pub timeframe: String,
    pub risk_per_trade: Decimal,
    pub max_positions: u32,
    pub min_confidence: Decimal,
    pub stop_loss_percent: Decimal,
    pub take_profit_percent: Decimal,
    pub grid: Option<GridTradingConfig>,
    pub volatility: Option<VolatilityConfig>,
}

/// Grid trading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridTradingConfig {
    pub enabled: bool,
    pub upper_bound_percent: Decimal, // Percentage above current price
    pub lower_bound_percent: Decimal,  // Percentage below current price
    pub grid_count: u32,
    pub spacing_type: String, // "fixed", "percentage", "volatility"
    pub spacing_value: Decimal,
    pub order_size: Decimal,
    pub dynamic_adjustment: bool,
}

/// Volatility-based trading configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityConfig {
    pub window_size: usize,
    pub high_vol_threshold: Decimal,
    pub low_vol_threshold: Decimal,
    pub percentile_threshold: Decimal,
    pub use_atr_for_stops: bool,
    pub atr_period: usize,
    pub atr_multiplier: Decimal,
}

impl TradingConfig {
    /// Load configuration from JSON file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: TradingConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Convert to GridConfig for grid strategy
    pub fn to_grid_config(&self, current_price: Decimal) -> Option<GridConfig> {
        self.grid.as_ref().map(|g| {
            let spacing = match g.spacing_type.as_str() {
                "fixed" => GridSpacing::Fixed(g.spacing_value),
                "percentage" => GridSpacing::Percentage(g.spacing_value),
                "volatility" => GridSpacing::VolatilityBased,
                _ => GridSpacing::Percentage(Decimal::new(1, 2)), // Default 1%
            };

            GridConfig {
                upper_bound: current_price * (Decimal::ONE + g.upper_bound_percent / Decimal::from(100)),
                lower_bound: current_price * (Decimal::ONE - g.lower_bound_percent / Decimal::from(100)),
                grid_count: g.grid_count,
                spacing_type: spacing,
                order_size: g.order_size,
                dynamic_adjustment: g.dynamic_adjustment,
            }
        })
    }
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            symbol: "ETH/USDT".to_string(),
            timeframe: "1s".to_string(),
            risk_per_trade: Decimal::new(2, 2), // 2%
            max_positions: 3,
            min_confidence: Decimal::new(70, 2), // 0.70
            stop_loss_percent: Decimal::new(2, 0), // 2%
            take_profit_percent: Decimal::new(4, 0), // 4%
            grid: Some(GridTradingConfig {
                enabled: true,
                upper_bound_percent: Decimal::new(5, 0), // 5%
                lower_bound_percent: Decimal::new(5, 0),  // 5%
                grid_count: 20,
                spacing_type: "percentage".to_string(),
                spacing_value: Decimal::new(1, 2), // 1%
                order_size: Decimal::new(100, 0),
                dynamic_adjustment: true,
            }),
            volatility: Some(VolatilityConfig {
                window_size: 100,
                high_vol_threshold: Decimal::new(3, 2), // 0.03 (3%)
                low_vol_threshold: Decimal::new(1, 2),   // 0.01 (1%)
                percentile_threshold: Decimal::new(50, 0), // 50th percentile
                use_atr_for_stops: true,
                atr_period: 14,
                atr_multiplier: Decimal::new(2, 0), // 2x ATR
            }),
        }
    }
}

