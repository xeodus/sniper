use crate::data::OrderRequest;
use crate::rest_client::{BinanceClient, HyperliquidClient};
use anyhow::{anyhow, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use std::fmt::Debug;
use std::sync::Arc;

/// Exchange type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeType {
    Binance,
    KuCoin,
    Hyperliquid
}

/// Trait for exchange operations - supports both CEX and DEX
#[async_trait::async_trait]
pub trait Exchange: Send + Sync + Debug {
    /// Get exchange type
    fn exchange_type(&self) -> ExchangeType;

    /// Place a market order
    async fn place_market_order(&self, req: &OrderRequest) -> Result<String>;

    /// Place a limit order
    async fn place_limit_order(&self, req: &OrderRequest) -> Result<String>;

    /// Cancel an order
    async fn cancel_order(&self, req: &OrderRequest) -> Result<()>;

    /// Get account balance
    async fn account_balance(&self) -> Result<Decimal>;

    /// Get current price for a symbol
    async fn get_price(&self, symbol: &str) -> Result<Decimal>; 
}

/// Exchange factory for creating exchange instances
pub struct ExchangeFactory;

impl ExchangeFactory {
    pub fn create_exchange (
        exchange_type: ExchangeType,
        api_key: String,
        api_secret: String,
        testnet: bool,
    ) -> Result<Arc<dyn Exchange>> {
        match exchange_type {
            ExchangeType::Binance => Ok(Arc::new(BinanceExchange::new(api_key, api_secret, testnet)?)),
            ExchangeType::KuCoin => {
                // TODO: Implement KuCoin exchange
                Err(anyhow!("KuCoin exchange not yet implemented"))
            },
            ExchangeType::Hyperliquid => {
                Ok(Arc::new(HyperliquidExchange::new(api_key, api_secret, testnet)?))
            } 
        }
    } 
}

/// Binance exchange implementation
#[derive(Debug)]
pub struct BinanceExchange {
    client: Client,
    base_url: String,
    api_key: String,
    api_secret: String,
}

/// Hyperliquid exchange implementation
#[derive(Debug)]
struct HyperliquidExchange {
    client: Client,
    base_url: String,
    api_key: String,
    api_secret: String
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

impl HyperliquidExchange {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Result<Self> {
        let base_url = if testnet {
            "".to_string()
        }
        else {
            "https://api.hyperliquid.xyz/exchange".to_string()
        };

        Ok(Self{
            client: Client::new(),
            base_url,
            api_key,
            api_secret
        })
    }
}


#[async_trait::async_trait]
impl Exchange for HyperliquidExchange {
    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Hyperliquid
    }

    async fn place_market_order(&self, req: &OrderRequest) -> Result<String> {
        let hyperliquid_client = HyperliquidClient::new(
            self.api_key.clone(), 
            self.api_secret.clone(), 
            self.base_url.contains("testnet")
        );
        hyperliquid_client.place_market_order(req).await
    }

    async fn place_limit_order(&self, req: &OrderRequest) -> Result<String> {
        let hyperliquid_client = HyperliquidClient::new(
            self.api_key.clone(), 
            self.api_secret.clone(), 
            self.base_url.contains("testnet")
        );
        hyperliquid_client.place_limit_order(req).await
    }
    
    async fn cancel_order(&self, req: &OrderRequest) -> Result<()> {
        let hyperliquid_client = HyperliquidClient::new(self.api_key.clone(), self.api_secret.clone(), self.base_url.contains("testnet")); 
        hyperliquid_client.cancel_orders(&req).await?;
        Ok(())
    }

    async fn account_balance(&self) -> Result<Decimal> {
        let hyperliquid_client = HyperliquidClient::new(self.api_key.clone(), self.api_secret.clone(), self.base_url.contains("testnet"));
        hyperliquid_client.account_balance().await
    }

    async fn get_price(&self, symbol: &str) -> Result<Decimal> {
        let symbol = symbol.replace("-", "").to_uppercase();
        let url = format!("{}/info", self.base_url);
        let body = serde_json::json!( {"type": "allMids"} );
        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response received: {:?}", response.text().await?));
        }

        let res = response.json::<serde_json::Value>().await?;
        let price_str = res[&symbol].as_str()
            .ok_or_else(|| anyhow!("Symbol {} not found in allMids response", symbol))?;
        let price = Decimal::from_str_exact(price_str)
            .map_err(|e| anyhow!("Failed to parse price as Decimal: {}", e))?;

        Ok(price)
    }
}

#[async_trait::async_trait]
impl Exchange for BinanceExchange {
    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Binance
    }

    async fn place_market_order(&self, req: &OrderRequest) -> Result<String> {
        let binance_client = BinanceClient::new(
            self.api_key.clone(),
            self.api_secret.clone(),
            self.base_url.contains("testnet"),
        );
        binance_client.place_market_order(req).await
    }

    async fn place_limit_order(&self, req: &OrderRequest) -> Result<String> {
        let binance_client = BinanceClient::new(
            self.api_key.clone(),
            self.api_secret.clone(),
            self.base_url.contains("testnet"),
        );
        binance_client.place_limit_order(req).await
    }

    async fn cancel_order(&self, req: &OrderRequest) -> Result<()> {
        let binance_client = BinanceClient::new(
            self.api_key.clone(),
            self.api_secret.clone(),
            self.base_url.contains("testnet"),
        );

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
        let symbol = symbol.replace("/", "").to_uppercase();
        let url = format!(
            "{}/api/v3/ticker/price?symbol={}",
            self.base_url, symbol
        );

        let response = self.client
            .get(&url)
            .header("X-MBX-APIKEY", self.api_key.clone())   
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response received: {:?}", response.text().await?));
        }

        let res = response.json::<serde_json::Value>().await?;

        let price_str = res["price"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Price not found in response"))?;

        Ok(rust_decimal::Decimal::from_str_exact(price_str)?)
    }
}
