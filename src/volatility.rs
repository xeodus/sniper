use crate::data::Candles;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use std::collections::VecDeque;

/// Volatility calculation methods
pub struct VolatilityCalculator {
    /// Historical candles for calculation
    candles: VecDeque<Candles>,
    /// Window size for volatility calculation
    window_size: usize,
}

impl VolatilityCalculator {
    pub fn new(window_size: usize) -> Self {
        Self {
            candles: VecDeque::with_capacity(window_size * 2),
            window_size,
        }
    }

    /// Add a new candle and update volatility
    pub fn add_candle(&mut self, candle: Candles) {
        self.candles.push_back(candle);
        if self.candles.len() > self.window_size {
            self.candles.pop_front();
        }
    }

    /// Calculate standard deviation-based volatility (annualized)
    pub fn calculate_volatility(&self) -> Decimal {
        if self.candles.len() < 2 {
            return Decimal::ZERO;
        }

        let returns: Vec<Decimal> = self
            .candles
            .iter()
            .zip(self.candles.iter().skip(1))
            .map(|(prev, curr)| {
                if prev.close > Decimal::ZERO {
                    (curr.close - prev.close) / prev.close
                } else {
                    Decimal::ZERO
                }
            })
            .collect();

        if returns.is_empty() {
            return Decimal::ZERO;
        }

        // Calculate mean return
        let mean: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        
        // Calculate variance
        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from(returns.len());

        // Standard deviation (volatility) - convert to f64 for sqrt
        let variance_f64 = variance.to_f64().unwrap_or(0.0);
        let std_dev_f64 = variance_f64.sqrt();
        let std_dev = Decimal::from_f64(std_dev_f64).unwrap_or(Decimal::ZERO);
        
        // Annualize (assuming 1-second candles, 365*24*3600 seconds per year)
        // For HFT, we might want shorter timeframes
        let annualization_factor_f64 = (365.0_f64 * 24.0 * 3600.0).sqrt();
        std_dev * Decimal::from_f64(annualization_factor_f64).unwrap_or(Decimal::ONE)
    }

    /// Calculate ATR (Average True Range) - better for volatility in trading
    pub fn calculate_atr(&self, period: usize) -> Decimal {
        if self.candles.len() < period + 1 {
            return Decimal::ZERO;
        }

        let true_ranges: Vec<Decimal> = self
            .candles
            .iter()
            .rev()
            .take(period + 1)
            .collect::<Vec<_>>()
            .windows(2)
            .map(|window| {
                let curr = window[0];
                let prev = window[1];
                
                let tr1 = curr.high - curr.low;
                let tr2 = (curr.high - prev.close).abs();
                let tr3 = (curr.low - prev.close).abs();
                
                tr1.max(tr2).max(tr3)
            })
            .collect();

        if true_ranges.is_empty() {
            return Decimal::ZERO;
        }

        true_ranges.iter().sum::<Decimal>() / Decimal::from(true_ranges.len())
    }

    /// Calculate realized volatility (using high-low range)
    pub fn calculate_realized_volatility(&self) -> Decimal {
        if self.candles.len() < 2 {
            return Decimal::ZERO;
        }

        let ranges: Vec<Decimal> = self
            .candles
            .iter()
            .map(|c| {
                if c.close > Decimal::ZERO {
                    (c.high - c.low) / c.close
                } else {
                    Decimal::ZERO
                }
            })
            .collect();

        if ranges.is_empty() {
            return Decimal::ZERO;
        }

        ranges.iter().sum::<Decimal>() / Decimal::from(ranges.len())
    }

    /// Get volatility percentile (0-100) for current market conditions
    pub fn get_volatility_percentile(&self, lookback: usize) -> Decimal {
        if self.candles.len() < lookback {
            return Decimal::new(50, 0); // Default to median
        }

        let current_vol = self.calculate_realized_volatility();
        let historical_vols: Vec<Decimal> = self
            .candles
            .iter()
            .rev()
            .take(lookback)
            .collect::<Vec<_>>()
            .windows(2)
            .map(|window| {
                let curr = window[0];
                let prev = window[1];
                if prev.close > Decimal::ZERO {
                    ((curr.close - prev.close) / prev.close).abs()
                } else {
                    Decimal::ZERO
                }
            })
            .collect();

        if historical_vols.is_empty() {
            return Decimal::new(50, 0);
        }

        let mut sorted_vols = historical_vols.clone();
        sorted_vols.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        
        let below_count = sorted_vols.iter().filter(|&&v| v < current_vol).count();
        let percentile = (below_count as f64 / sorted_vols.len() as f64) * 100.0;
        
        Decimal::from_f64(percentile).unwrap_or(Decimal::new(50, 0))
    }

    /// Check if market is in high volatility regime
    pub fn is_high_volatility(&self, threshold: Decimal) -> bool {
        self.calculate_realized_volatility() > threshold
    }

    /// Check if market is in low volatility regime
    pub fn is_low_volatility(&self, threshold: Decimal) -> bool {
        self.calculate_realized_volatility() < threshold
    }
}

/// HFT-style volatility-based trading signals
pub struct VolatilityStrategy {
    calculator: VolatilityCalculator,
    /// High volatility threshold
    high_vol_threshold: Decimal,
    /// Low volatility threshold
    low_vol_threshold: Decimal,
    /// Volatility percentile threshold for trading
    percentile_threshold: Decimal,
}

impl VolatilityStrategy {
    pub fn new(
        window_size: usize,
        high_vol_threshold: Decimal,
        low_vol_threshold: Decimal,
        percentile_threshold: Decimal,
    ) -> Self {
        Self {
            calculator: VolatilityCalculator::new(window_size),
            high_vol_threshold,
            low_vol_threshold,
            percentile_threshold,
        }
    }

    /// Update with new candle
    pub fn update(&mut self, candle: Candles) {
        self.calculator.add_candle(candle);
    }

    /// Get current volatility
    pub fn get_volatility(&self) -> Decimal {
        self.calculator.calculate_realized_volatility()
    }

    /// Get ATR for position sizing
    pub fn get_atr(&self, period: usize) -> Decimal {
        self.calculator.calculate_atr(period)
    }

    /// Determine if we should trade based on volatility regime
    pub fn should_trade(&self) -> bool {
        let percentile = self.calculator.get_volatility_percentile(100);
        percentile >= self.percentile_threshold
    }

    /// Get volatility-based position size multiplier
    /// Higher volatility = smaller position size
    pub fn get_position_size_multiplier(&self) -> Decimal {
        let vol = self.get_volatility();
        
        if vol > self.high_vol_threshold {
            // Reduce position size in high volatility
            Decimal::new(50, 2) // 50% of normal size
        } else if vol < self.low_vol_threshold {
            // Increase position size in low volatility
            Decimal::new(150, 2) // 150% of normal size
        } else {
            Decimal::ONE // Normal size
        }
    }

    /// Get volatility-based stop loss distance
    /// Higher volatility = wider stop loss
    pub fn get_stop_loss_distance(&self, base_distance: Decimal) -> Decimal {
        let vol = self.get_volatility();
        let multiplier = if vol > self.high_vol_threshold {
            Decimal::new(200, 2) // 2x stop loss in high vol
        } else if vol < self.low_vol_threshold {
            Decimal::new(75, 2) // 0.75x stop loss in low vol
        } else {
            Decimal::ONE
        };
        
        base_distance * multiplier
    }

    /// Check if we should use grid trading (low volatility) or momentum (high volatility)
    pub fn should_use_grid(&self) -> bool {
        self.calculator.is_low_volatility(self.low_vol_threshold)
    }
}

