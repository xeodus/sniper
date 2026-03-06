use crate::data::{Candles, Side, Signal, Trend};
use rust_decimal::prelude::*;
use uuid::Uuid;

pub struct MarketSignal {
    pub candles: Vec<Candles>,
    pub rsi: usize,
    pub ema_slow: usize,
    pub ema_fast: usize,
}

impl MarketSignal {
    pub fn new() -> Self {
        Self {
            candles: Vec::new(),
            rsi: 14,
            ema_slow: 26,
            ema_fast: 12,
        }
    }

    pub fn add_candles(&mut self, candle: Candles) {
        self.candles.push(candle);

        if self.candles.len() > 200 {
            self.candles.remove(0);
        }
    }

    pub fn calculate_rsi(&self) -> f64 {
        if self.candles.len() < self.rsi + 1 {
            return 50.0;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (self.candles.len() - self.rsi)..self.candles.len() {
            let change = (self.candles[i].close - self.candles[i - 1].close)
                .to_f64()
                .unwrap();

            if change > 0.0 {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let ave_gain = gains / self.rsi as f64;
        let ave_loss = losses / self.rsi as f64;

        if ave_loss == 0.0 {
            return 100.0;
        }

        let rs = ave_gain / ave_loss;
        100.0 - (100.0 / (1.0 + rs))
    }

    pub fn calculate_ema(&self, period: usize) -> Decimal {
        if self.candles.is_empty() {
            return Decimal::ZERO;
        }

        let multiplier = Decimal::new(2, 0) / Decimal::new((period + 1) as i64, 0);
        let mut ema = self.candles[0].close;

        for candle in self.candles.iter().skip(1) {
            ema = (candle.close - ema) * multiplier + ema;
        }

        ema
    }

    pub fn calculate_macd(&self) -> (f64, f64) {
        let ema_fast = self.calculate_ema(self.ema_fast).to_f64().unwrap();
        let ema_slow = self.calculate_ema(self.ema_slow).to_f64().unwrap();
        let macd = ema_fast - ema_slow;
        let signal = macd * 0.8;
        (macd, signal)
    }

    pub fn calculate_confidence(&self, rsi: f64, macd: f64, trend: &Trend) -> f64 {
        let mut confidence = 0.5;
        if !(30.0..70.0).contains(&rsi) {
            confidence += 0.2;
        }
        if macd.abs() > 0.01 {
            confidence += 0.15;
        }
        if *trend != Trend::Sideways {
            confidence += 0.15;
        }
        confidence
    }

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

    pub fn determine_action(&self, rsi: f64, macd: f64, signal_line: f64) -> Side {
        match self.detect_trend() {
            Trend::Up => {
                if rsi < 30.0 && macd > signal_line {
                    Side::Buy
                } else if rsi > 70.0 {
                    Side::Sell
                } else {
                    Side::Hold
                }
            }
            Trend::Down => {
                if rsi > 70.0 && macd < signal_line {
                    Side::Sell
                } else {
                    Side::Hold
                }
            }
            Trend::Sideways => {
                if rsi < 30.0 {
                    Side::Buy
                } else if rsi > 70.0 {
                    Side::Sell
                } else {
                    Side::Hold
                }
            }
        }
    }

    pub fn analyze(&self, symbol: String) -> Option<Signal> {
        if self.candles.len() < 50 {
            return None;
        }

        let trend = self.detect_trend();
        let rsi = self.calculate_rsi();
        let (macd, signal) = self.calculate_macd();
        let action = self.determine_action(rsi, macd, signal);
        let latest_candle = self.candles.last()?;
        let confidence = Decimal::from_f64(self.calculate_confidence(rsi, macd, &trend)).unwrap();

        Some(Signal {
            id: Uuid::new_v4().to_string(),
            timestamp: latest_candle.timestamp,
            symbol,
            action,
            trend: trend.clone(),
            price: latest_candle.close,
            confidence,
        })
    }
}
