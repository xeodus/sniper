use crate::data::{OrderReq, Side};
use crate::rest_client::BinanceClient;
use anyhow::Result;
use rust_decimal::Decimal;
use std::fmt::Debug;
use std::sync::Arc;

/// Exchange type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeType {
    Binance,
    KuCoin,
    Uniswap,  // DEX
    PancakeSwap, // DEX
}

/// Trait for exchange operations - supports both CEX and DEX
#[async_trait::async_trait]
pub trait Exchange: Send + Sync + Debug {
    /// Get exchange type
    fn exchange_type(&self) -> ExchangeType;
    
    /// Place a market order
    async fn place_market_order(&self, req: &OrderReq) -> Result<String>;
    
    /// Place a limit order
    async fn place_limit_order(&self, req: &OrderReq) -> Result<String>;
    
    /// Cancel an order
    async fn cancel_order(&self, order_id: &str, symbol: &str) -> Result<()>;
    
    /// Get account balance
    async fn account_balance(&self) -> Result<Decimal>;
    
    /// Get current price for a symbol
    async fn get_price(&self, symbol: &str) -> Result<Decimal>;
    
    /// Check if exchange is a DEX
    fn is_dex(&self) -> bool {
        matches!(
            self.exchange_type(),
            ExchangeType::Uniswap | ExchangeType::PancakeSwap
        )
    }
}

/// Exchange factory for creating exchange instances
pub struct ExchangeFactory;

impl ExchangeFactory {
    pub fn create_cex(
        exchange_type: ExchangeType,
        api_key: String,
        api_secret: String,
        testnet: bool,
    ) -> Result<Arc<dyn Exchange>> {
        match exchange_type {
            ExchangeType::Binance => {
                Ok(Arc::new(BinanceExchange::new(api_key, api_secret, testnet)?))
            }
            ExchangeType::KuCoin => {
                // TODO: Implement KuCoin exchange
                Err(anyhow::anyhow!("KuCoin exchange not yet implemented"))
            }
            _ => Err(anyhow::anyhow!("Invalid CEX type")),
        }
    }
    
    pub fn create_dex(exchange_type: ExchangeType) -> Result<Arc<dyn Exchange>> {
        match exchange_type {
            ExchangeType::Uniswap => {
                // TODO: Implement Uniswap DEX
                Err(anyhow::anyhow!("Uniswap DEX not yet implemented"))
            }
            ExchangeType::PancakeSwap => {
                // TODO: Implement PancakeSwap DEX
                Err(anyhow::anyhow!("PancakeSwap DEX not yet implemented"))
            }
            _ => Err(anyhow::anyhow!("Invalid DEX type")),
        }
    }
}

/// Binance exchange implementation
#[derive(Debug)]
pub struct BinanceExchange {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    api_secret: String,
}

impl BinanceExchange {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Result<Self> {
        let base_url = if testnet {
            "https://testnet.binance.vision".to_string()
        } else {
            "https://api.binance.com".to_string()
        };

        Ok(Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
            api_secret,
        })
    }
}

#[async_trait::async_trait]
impl Exchange for BinanceExchange {
    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Binance
    }

    async fn place_market_order(&self, req: &OrderReq) -> Result<String> {
        let binance_client = BinanceClient::new(
            self.api_key.clone(),
            self.api_secret.clone(),
            self.base_url.contains("testnet"),
        );
        binance_client.place_market_order(req).await
    }

    async fn place_limit_order(&self, req: &OrderReq) -> Result<String> {
        let binance_client = BinanceClient::new(
            self.api_key.clone(),
            self.api_secret.clone(),
            self.base_url.contains("testnet"),
        );
        binance_client.place_limit_order(req).await
    }

    async fn cancel_order(&self, order_id: &str, symbol: &str) -> Result<()> {
        use crate::data::OrderType;
        let binance_client = BinanceClient::new(
            self.api_key.clone(),
            self.api_secret.clone(),
            self.base_url.contains("testnet"),
        );
        let req = crate::data::OrderReq {
            id: order_id.to_string(),
            symbol: symbol.to_string(),
            side: Side::Buy, // Dummy value, not used in cancel
            order_type: OrderType::Limit,
            price: Decimal::ZERO,
            size: Decimal::ZERO,
            sl: None,
            tp: None,
            manual: false,
        };
        binance_client.cancel_orders(&req).await?;
        Ok(())
    }

    async fn account_balance(&self) -> Result<Decimal> {
        let binance_client = BinanceClient::new(
            self.api_key.clone(),
            self.api_secret.clone(),
            self.base_url.contains("testnet"),
        );
        binance_client.account_balance().await
    }

    async fn get_price(&self, symbol: &str) -> Result<Decimal> {
        let symbol_upper = symbol.replace("/", "").to_uppercase();
        let url = format!("{}/api/v3/ticker/price?symbol={}", self.base_url, symbol_upper);
        
        let response = self
            .client
            .get(&url)
            .send()
            .await?;
        
        let json: serde_json::Value = response.json().await?;
        let price_str = json["price"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Price not found in response"))?;
        
        Ok(rust_decimal::Decimal::from_str_exact(price_str)?)
    }
}

