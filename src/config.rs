use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    /// Trading pair symbol (e.g., "ETH/USDT")
    pub symbol: String,

    /// Candle timeframe (e.g., "1m", "5m", "1h")
    pub timeframe: String,

    /// Position size multiplier
    #[serde(default = "default_size")]
    pub size: u32,

    /// Percentage of account to risk per trade
    #[serde(default = "default_risk_per_trade")]
    pub risk_per_trade: f64,

    /// Maximum number of open positions
    #[serde(default = "default_max_positions")]
    pub max_positions: u32,

    /// Minimum confidence level to open a position (0.0 - 1.0)
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f64,

    /// Stop loss percentage
    #[serde(default = "default_stop_loss_percent")]
    pub stop_loss_percent: f64,

    /// Take profit percentage
    #[serde(default = "default_take_profit_percent")]
    pub take_profit_percent: f64,

    /// Use testnet instead of mainnet
    #[serde(default = "default_testnet")]
    pub testnet: bool,

    /// Enable Discord notifications
    #[serde(default)]
    pub notifications_enabled: bool,
}

fn default_size() -> u32 {
    1
}

fn default_risk_per_trade() -> f64 {
    2.0
}

fn default_max_positions() -> u32 {
    3
}

fn default_min_confidence() -> f64 {
    0.7
}

fn default_stop_loss_percent() -> f64 {
    2.0
}

fn default_take_profit_percent() -> f64 {
    4.0
}

fn default_testnet() -> bool {
    true
}

#[allow(dead_code)]
impl Config {
    /// Load configuration from a JSON file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        let config: Config =
            serde_json::from_str(&contents).context("Failed to parse config JSON")?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variables with fallback to file
    pub fn load() -> Result<Self> {
        // Try loading from file first
        let config_path =
            std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.json".to_string());

        if Path::new(&config_path).exists() {
            Self::from_file(&config_path)
        } else {
            // Use defaults if no config file exists
            Ok(Self::default())
        }
    }

    /// Validate configuration values
    fn validate(&self) -> Result<()> {
        if self.symbol.is_empty() {
            anyhow::bail!("Symbol cannot be empty");
        }

        if self.timeframe.is_empty() {
            anyhow::bail!("Timeframe cannot be empty");
        }

        if self.risk_per_trade <= 0.0 || self.risk_per_trade > 100.0 {
            anyhow::bail!("risk_per_trade must be between 0 and 100");
        }

        if self.min_confidence < 0.0 || self.min_confidence > 1.0 {
            anyhow::bail!("min_confidence must be between 0.0 and 1.0");
        }

        if self.stop_loss_percent <= 0.0 || self.stop_loss_percent > 100.0 {
            anyhow::bail!("stop_loss_percent must be between 0 and 100");
        }

        if self.take_profit_percent <= 0.0 || self.take_profit_percent > 100.0 {
            anyhow::bail!("take_profit_percent must be between 0 and 100");
        }

        Ok(())
    }

    /// Get risk per trade as Decimal
    pub fn risk_per_trade_decimal(&self) -> Decimal {
        Decimal::from_f64_retain(self.risk_per_trade / 100.0).unwrap_or(Decimal::new(2, 2))
    }

    /// Get minimum confidence as Decimal
    pub fn min_confidence_decimal(&self) -> Decimal {
        Decimal::from_f64_retain(self.min_confidence).unwrap_or(Decimal::new(70, 2))
    }

    /// Get stop loss multiplier (e.g., 2% = 0.98 for long, 1.02 for short)
    pub fn stop_loss_multiplier_long(&self) -> Decimal {
        Decimal::from_f64_retain(1.0 - self.stop_loss_percent / 100.0)
            .unwrap_or(Decimal::new(98, 2))
    }

    pub fn stop_loss_multiplier_short(&self) -> Decimal {
        Decimal::from_f64_retain(1.0 + self.stop_loss_percent / 100.0)
            .unwrap_or(Decimal::new(102, 2))
    }

    /// Get take profit multiplier
    pub fn take_profit_multiplier_long(&self) -> Decimal {
        Decimal::from_f64_retain(1.0 + self.take_profit_percent / 100.0)
            .unwrap_or(Decimal::new(104, 2))
    }

    pub fn take_profit_multiplier_short(&self) -> Decimal {
        Decimal::from_f64_retain(1.0 - self.take_profit_percent / 100.0)
            .unwrap_or(Decimal::new(96, 2))
    }

    /// Get normalized symbol (without slash, uppercase)
    pub fn normalized_symbol(&self) -> String {
        self.symbol.replace("/", "").to_uppercase()
    }

    /// Get lowercase symbol for WebSocket subscriptions
    pub fn ws_symbol(&self) -> String {
        self.symbol.replace("/", "").to_lowercase()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            symbol: "ETH/USDT".to_string(),
            timeframe: "1m".to_string(),
            size: default_size(),
            risk_per_trade: default_risk_per_trade(),
            max_positions: default_max_positions(),
            min_confidence: default_min_confidence(),
            stop_loss_percent: default_stop_loss_percent(),
            take_profit_percent: default_take_profit_percent(),
            testnet: default_testnet(),
            notifications_enabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.symbol, "ETH/USDT");
        assert_eq!(config.timeframe, "1m");
        assert_eq!(config.risk_per_trade, 2.0);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        config.symbol = "".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_normalized_symbol() {
        let config = Config {
            symbol: "ETH/USDT".to_string(),
            ..Default::default()
        };
        assert_eq!(config.normalized_symbol(), "ETHUSDT");
        assert_eq!(config.ws_symbol(), "ethusdt");
    }
}
