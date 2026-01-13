use rust_decimal::Decimal;
use std::collections::HashMap;

/// Grid trading strategy configuration
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// Upper bound of the grid
    pub upper_bound: Decimal,
    /// Lower bound of the grid
    pub lower_bound: Decimal,
    /// Number of grid levels
    pub grid_count: u32,
    /// Grid spacing (percentage or fixed)
    pub spacing_type: GridSpacing,
    /// Order size per grid level
    pub order_size: Decimal,
    /// Enable dynamic grid adjustment based on volatility
    pub dynamic_adjustment: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridSpacing {
    /// Fixed price spacing
    Fixed(Decimal),
    /// Percentage-based spacing
    Percentage(Decimal),
    /// Automatic spacing based on volatility
    VolatilityBased,
}

/// Grid level representing a buy/sell order
#[derive(Debug, Clone)]
pub struct GridLevel {
    pub id: String,
    pub price: Decimal,
    pub side: GridSide,
    pub order_id: Option<String>,
    pub filled: bool,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridSide {
    Buy,
    Sell,
}

/// Grid trading strategy implementation
pub struct GridStrategy {
    config: GridConfig,
    levels: HashMap<String, GridLevel>,
    current_price: Decimal,
    base_price: Decimal,
    volatility: Decimal,
}

impl GridStrategy {
    pub fn new(config: GridConfig, initial_price: Decimal) -> Self {
        let mut strategy = Self {
            config,
            levels: HashMap::new(),
            current_price: initial_price,
            base_price: initial_price,
            volatility: Decimal::ZERO,
        };
        
        strategy.initialize_grid();
        strategy
    }

    /// Initialize grid levels
    fn initialize_grid(&mut self) {
        self.levels.clear();
        
        let price_range = self.config.upper_bound - self.config.lower_bound;
        let spacing = match self.config.spacing_type {
            GridSpacing::Fixed(amount) => amount,
            GridSpacing::Percentage(pct) => {
                self.base_price * pct / Decimal::from(100)
            }
            GridSpacing::VolatilityBased => {
                // Use volatility if available, otherwise use percentage
                if self.volatility > Decimal::ZERO {
                    self.base_price * self.volatility
                } else {
                    price_range / Decimal::from(self.config.grid_count)
                }
            }
        };

        // Create buy levels (below current price)
        let mut buy_price = self.base_price;
        let mut buy_level = 0;
        while buy_price >= self.config.lower_bound && buy_level < self.config.grid_count / 2 {
            let level_id = format!("buy_{}", buy_level);
            self.levels.insert(
                level_id.clone(),
                GridLevel {
                    id: level_id,
                    price: buy_price,
                    side: GridSide::Buy,
                    order_id: None,
                    filled: false,
                    quantity: self.config.order_size,
                },
            );
            buy_price -= spacing;
            buy_level += 1;
        }

        // Create sell levels (above current price)
        let mut sell_price = self.base_price + spacing;
        let mut sell_level = 0;
        while sell_price <= self.config.upper_bound && sell_level < self.config.grid_count / 2 {
            let level_id = format!("sell_{}", sell_level);
            self.levels.insert(
                level_id.clone(),
                GridLevel {
                    id: level_id,
                    price: sell_price,
                    side: GridSide::Sell,
                    order_id: None,
                    filled: false,
                    quantity: self.config.order_size,
                },
            );
            sell_price += spacing;
            sell_level += 1;
        }
    }

    /// Update grid based on new price and volatility
    pub fn update(&mut self, price: Decimal, volatility: Decimal) {
        self.current_price = price;
        self.volatility = volatility;

        // Check for filled orders and create new levels
        self.check_filled_orders();

        // Dynamic grid adjustment if enabled
        if self.config.dynamic_adjustment {
            self.adjust_grid_dynamically();
        }
    }

    /// Check which grid levels have been filled
    fn check_filled_orders(&mut self) {
        for level in self.levels.values_mut() {
            if level.filled {
                continue;
            }

            // Check if price has crossed the grid level
            match level.side {
                GridSide::Buy => {
                    if self.current_price <= level.price {
                        level.filled = true;
                    }
                }
                GridSide::Sell => {
                    if self.current_price >= level.price {
                        level.filled = true;
                    }
                }
            }
        }
    }

    /// Dynamically adjust grid based on volatility
    fn adjust_grid_dynamically(&mut self) {
        // If volatility has changed significantly, reinitialize grid
        let volatility_change_threshold = Decimal::new(20, 2); // 20% change
        
        if self.volatility > Decimal::ZERO {
            let volatility_change = (self.volatility - self.volatility).abs() / self.volatility;
            
            if volatility_change > volatility_change_threshold {
                // Adjust bounds based on volatility
                let volatility_multiplier = Decimal::ONE + self.volatility;
                let range = self.config.upper_bound - self.config.lower_bound;
                let center = (self.config.upper_bound + self.config.lower_bound) / Decimal::from(2);
                
                self.config.upper_bound = center + (range * volatility_multiplier) / Decimal::from(2);
                self.config.lower_bound = center - (range * volatility_multiplier) / Decimal::from(2);
                
                self.initialize_grid();
            }
        }
    }

    /// Get all unfilled buy orders
    pub fn get_unfilled_buys(&self) -> Vec<&GridLevel> {
        self.levels
            .values()
            .filter(|level| !level.filled && level.side == GridSide::Buy)
            .collect()
    }

    /// Get all unfilled sell orders
    pub fn get_unfilled_sells(&self) -> Vec<&GridLevel> {
        self.levels
            .values()
            .filter(|level| !level.filled && level.side == GridSide::Sell)
            .collect()
    }

    /// Mark a grid level as filled
    pub fn mark_filled(&mut self, level_id: &str, order_id: String) {
        if let Some(level) = self.levels.get_mut(level_id) {
            level.filled = true;
            level.order_id = Some(order_id);
        }
    }

    /// Get grid level by ID
    pub fn get_level(&self, level_id: &str) -> Option<&GridLevel> {
        self.levels.get(level_id)
    }

    /// Get all grid levels
    pub fn get_all_levels(&self) -> Vec<&GridLevel> {
        self.levels.values().collect()
    }

    /// Calculate grid profit (sum of all filled sell orders - sum of all filled buy orders)
    pub fn calculate_profit(&self) -> Decimal {
        let mut profit = Decimal::ZERO;
        
        for level in self.levels.values() {
            if level.filled {
                match level.side {
                    GridSide::Buy => {
                        profit -= level.price * level.quantity;
                    }
                    GridSide::Sell => {
                        profit += level.price * level.quantity;
                    }
                }
            }
        }
        
        profit
    }
}

