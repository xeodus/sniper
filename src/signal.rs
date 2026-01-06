use crate::data::{Candles, Side, Signal, Trend};
use rust_decimal::prelude::*;
use uuid::Uuid;

/// Market signal analyzer using technical indicators
pub struct MarketSignal {
    pub candles: Vec<Candles>,
    pub rsi_period: usize,
    pub ema_slow: usize,
    pub ema_fast: usize,
    pub max_candles: usize,
}

impl Default for MarketSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl MarketSignal {
    pub fn new() -> Self {
        Self {
            candles: Vec::new(),
            rsi_period: 14,
            ema_slow: 26,
            ema_fast: 12,
            max_candles: 200,
        }
    }

    /// Add a new candle to the analyzer
    pub fn add_candles(&mut self, candle: Candles) {
        self.candles.push(candle);

        // Keep only the last N candles
        if self.candles.len() > self.max_candles {
            self.candles.remove(0);
        }
    }

    /// Calculate Relative Strength Index (RSI)
    pub fn calculate_rsi(&self) -> f64 {
        if self.candles.len() < self.rsi_period + 1 {
            return 50.0; // Neutral when not enough data
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        let start = self.candles.len() - self.rsi_period;
        for i in start..self.candles.len() {
            let change = (self.candles[i].close - self.candles[i - 1].close)
                .to_f64()
                .unwrap_or(0.0);

            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / self.rsi_period as f64;
        let avg_loss = losses / self.rsi_period as f64;

        if avg_loss == 0.0 {
            return 100.0;
        }

        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    /// Calculate Exponential Moving Average (EMA)
    pub fn calculate_ema(&self, period: usize) -> Decimal {
        if self.candles.is_empty() {
            return Decimal::ZERO;
        }

        if self.candles.len() < period {
            // Not enough data, return simple average
            let sum: Decimal = self.candles.iter().map(|c| c.close).sum();
            return sum / Decimal::from(self.candles.len());
        }

        let multiplier = Decimal::new(2, 0) / Decimal::new((period + 1) as i64, 0);

        // Start with SMA for first N periods
        let sma: Decimal = self.candles[..period]
            .iter()
            .map(|c| c.close)
            .sum::<Decimal>()
            / Decimal::from(period);

        let mut ema = sma;

        // Calculate EMA for remaining periods
        for candle in self.candles.iter().skip(period) {
            ema = (candle.close - ema) * multiplier + ema;
        }

        ema
    }

    /// Calculate MACD (Moving Average Convergence Divergence)
    pub fn calculate_macd(&self) -> (f64, f64) {
        let ema_fast = self.calculate_ema(self.ema_fast).to_f64().unwrap_or(0.0);
        let ema_slow = self.calculate_ema(self.ema_slow).to_f64().unwrap_or(0.0);
        let macd = ema_fast - ema_slow;

        // Signal line is typically 9-period EMA of MACD
        // Simplified: using 80% of MACD as approximation
        let signal = macd * 0.8;
        (macd, signal)
    }

    /// Calculate trading confidence based on indicators
    pub fn calculate_confidence(&self, rsi: f64, macd: f64, trend: &Trend) -> f64 {
        let mut confidence: f64 = 0.5;

        // RSI contribution (0.0 - 0.25)
        if !(30.0..=70.0).contains(&rsi) {
            confidence += 0.2;
        } else if !(40.0..=60.0).contains(&rsi) {
            confidence += 0.1;
        }

        // MACD contribution (0.0 - 0.15)
        if macd.abs() > 0.01 {
            confidence += 0.15;
        } else if macd.abs() > 0.005 {
            confidence += 0.08;
        }

        // Trend contribution (0.0 - 0.15)
        if *trend != Trend::Sideways {
            confidence += 0.15;
        }

        confidence.min(1.0)
    }

    /// Detect the current market trend
    pub fn detect_trend(&self) -> Trend {
        if self.candles.len() < 50 {
            return Trend::Sideways;
        }

        let ema_20 = self.calculate_ema(20);
        let ema_50 = self.calculate_ema(50);
        let recent_close = self.candles.last().unwrap().close;

        if recent_close > ema_20 && ema_20 > ema_50 {
            Trend::Up
        } else if recent_close < ema_20 && ema_20 < ema_50 {
            Trend::Down
        } else {
            Trend::Sideways
        }
    }

    /// Determine trading action based on indicators
    pub fn determine_action(&self, rsi: f64, macd: f64, signal_line: f64) -> Side {
        let trend = self.detect_trend();

        match trend {
            Trend::Up => {
                if rsi < 30.0 && macd > signal_line {
                    Side::Buy // Oversold in uptrend
                } else if rsi > 70.0 {
                    Side::Sell // Overbought
                } else if rsi < 45.0 && macd > signal_line {
                    Side::Buy // Mild buy signal
                } else {
                    Side::Hold
                }
            }
            Trend::Down => {
                if rsi > 70.0 && macd < signal_line {
                    Side::Sell // Overbought in downtrend
                } else if rsi < 30.0 {
                    Side::Buy // Oversold bounce potential
                } else {
                    Side::Hold
                }
            }
            Trend::Sideways => {
                if rsi < 30.0 {
                    Side::Buy // Oversold
                } else if rsi > 70.0 {
                    Side::Sell // Overbought
                } else {
                    Side::Hold
                }
            }
        }
    }

    /// Analyze market and generate a trading signal
    pub fn analyze(&self, symbol: String) -> Option<Signal> {
        // Need at least 50 candles for reliable analysis
        if self.candles.len() < 50 {
            return None;
        }

        let trend = self.detect_trend();
        let rsi = self.calculate_rsi();
        let (macd, signal_line) = self.calculate_macd();
        let action = self.determine_action(rsi, macd, signal_line);
        let latest_candle = self.candles.last()?;
        let confidence = Decimal::from_f64(self.calculate_confidence(rsi, macd, &trend))?;

        Some(Signal {
            id: Uuid::new_v4().to_string(),
            timestamp: latest_candle.timestamp,
            symbol,
            action,
            trend,
            price: latest_candle.close,
            confidence,
        })
    }

    /// Get the number of candles currently stored
    pub fn candle_count(&self) -> usize {
        self.candles.len()
    }

    /// Clear all candles
    pub fn clear(&mut self) {
        self.candles.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candle(close: f64, timestamp: i64) -> Candles {
        let price = Decimal::from_f64(close).unwrap();
        Candles {
            open: price,
            high: price * Decimal::new(101, 2),
            low: price * Decimal::new(99, 2),
            close: price,
            volume: Decimal::new(1000, 0),
            timestamp,
        }
    }

    #[test]
    fn test_new_market_signal() {
        let signal = MarketSignal::new();
        assert_eq!(signal.rsi_period, 14);
        assert_eq!(signal.ema_fast, 12);
        assert_eq!(signal.ema_slow, 26);
        assert!(signal.candles.is_empty());
    }

    #[test]
    fn test_add_candles() {
        let mut signal = MarketSignal::new();
        signal.max_candles = 5;

        for i in 0..7 {
            signal.add_candles(create_test_candle(100.0 + i as f64, i));
        }

        assert_eq!(signal.candles.len(), 5);
    }

    #[test]
    fn test_rsi_neutral_without_data() {
        let signal = MarketSignal::new();
        assert_eq!(signal.calculate_rsi(), 50.0);
    }

    #[test]
    fn test_ema_empty() {
        let signal = MarketSignal::new();
        assert_eq!(signal.calculate_ema(14), Decimal::ZERO);
    }

    #[test]
    fn test_trend_sideways_without_data() {
        let signal = MarketSignal::new();
        assert_eq!(signal.detect_trend(), Trend::Sideways);
    }

    #[test]
    fn test_analyze_returns_none_without_data() {
        let signal = MarketSignal::new();
        assert!(signal.analyze("ETHUSDT".to_string()).is_none());
    }

    #[test]
    fn test_confidence_calculation() {
        let signal = MarketSignal::new();

        // High confidence scenario
        let conf = signal.calculate_confidence(25.0, 0.02, &Trend::Down);
        assert!(conf > 0.7);

        // Low confidence scenario
        let conf = signal.calculate_confidence(50.0, 0.001, &Trend::Sideways);
        assert!(conf <= 0.6);
    }
}
