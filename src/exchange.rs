use crate::data::OrderReq;
use crate::rest_client::{BinanceClient, HyperliquidClient};
use anyhow::{anyhow, Result};
use reqwest::Response;
use rust_decimal::Decimal;
use tracing::warn;
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
    async fn place_market_order(&self, req: &OrderReq) -> Result<String>;

    /// Place a limit order
    async fn place_limit_order(&self, req: &OrderReq) -> Result<String>;

    /// Cancel an order
    async fn cancel_order(&self, req: &OrderReq) -> Result<()>;

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
    pub client: Arc<BinanceClient>
}

impl BinanceExchange {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Result<Self> {
        /*let base_url = if testnet {
            "https://testnet.binance.vision".to_string()
        } else {
            "https://api.binance.com".to_string()
        };*/

        Ok(Self {
            client: Arc::new(BinanceClient::new(api_key, api_secret, testnet)) 
        })
    }
}

// TODO: Implement KuCoin exchange

/// Hyperliquid exchange implementation
#[derive(Debug)]
pub struct HyperliquidExchange {
    pub client: Arc<HyperliquidClient>
}

impl HyperliquidExchange {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Result<Self> {
        /*let base_url = if testnet {
            "".to_string()
        }
        else {
            "https://api.hyperliquid.xyz/exchange".to_string()
        };*/

        Ok(Self{
            client: Arc::new(HyperliquidClient::new(api_key, api_secret, testnet))
        })
    }

    fn asset_index(symbol: &str) -> u32 {
        match symbol.to_uppercase().trim_end_matches("/USDT").trim_end_matches("USDT") {
            "BTC" => 0,
            "ETH" => 1,
            "SOL" => 2,
            "ARB" => 3,
            "MATIC" => 4,
            other => {
                warn!("Unknown HL asset {}, defaulting to index 2: SOL", other);
                2
            }
        }
    }
}


#[async_trait::async_trait]
impl Exchange for HyperliquidExchange {
    fn exchange_type(&self) -> ExchangeType {
        ExchangeType::Hyperliquid
    }

    async fn place_market_order(&self, req: &OrderReq) -> Result<String> {
        let hyperliquid_client = HyperliquidClient::new(
            self.client.api_key.clone(), 
            self.client.api_secret.clone(), 
            self.client.base_url.contains("testnet")
        );

        hyperliquid_client.place_market_order(req).await
    }

    async fn place_limit_order(&self, req: &OrderReq) -> Result<String> {
        let hyperliquid_client = HyperliquidClient::new(
            self.client.api_key.clone(),
            self.client.api_secret.clone(),
            self.client.base_url.contains("testnet")
        );

        hyperliquid_client.place_limit_order(req).await
    }
    
    async fn cancel_order(&self, req: &OrderReq) -> Result<()> {
        let hyperliquid_client = HyperliquidClient::new(
            self.client.api_key.clone(),
            self.client.api_secret.clone(),
            self.client.base_url.contains("testnet")
        );

        hyperliquid_client.cancel_orders(&req).await?;
        Ok(())
    }

    async fn account_balance(&self) -> Result<Decimal> {
        let hyperliquid_client = HyperliquidClient::new(
            self.client.api_key.clone(),
            self.client.api_secret.clone(),
            self.client.base_url.contains("testnet")
        );

        hyperliquid_client.account_balance().await
    }

    async fn get_price(&self, symbol: &str) -> Result<Decimal> {
        let symbol = symbol.replace("-", "").to_uppercase();
        let url = format!("{}/info", self.client.base_url);
        let body = serde_json::json!( {"type": "allMids"} );
        let response: Response = self.client.client.post(url).json(&body).send().await?;

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

    async fn place_market_order(&self, req: &OrderReq) -> Result<String> {
        let binance_client = BinanceClient::new(
            self.client.api_key.clone(),
            self.client.api_secret.clone(),
            self.client.base_url.contains("testnet")
        );

        binance_client.place_market_order(req).await
    }

    async fn place_limit_order(&self, req: &OrderReq) -> Result<String> {
        let binance_client = BinanceClient::new(
            self.client.api_key.clone(),
            self.client.api_secret.clone(),
            self.client.base_url.contains("testnet")
        );

        binance_client.place_limit_order(req).await
    }

    async fn cancel_order(&self, req: &OrderReq) -> Result<()> {
        let binance_client = BinanceClient::new(
            self.client.api_key.clone(),
            self.client.api_secret.clone(),
            self.client.base_url.contains("testnet")
        );

        binance_client.cancel_orders(&req).await?;
        Ok(())
    }

    async fn account_balance(&self) -> Result<Decimal> {
        let binance_client = BinanceClient::new(
            self.client.api_key.clone(),
            self.client.api_secret.clone(),
            self.client.base_url.contains("testnet")
        );

        binance_client.account_balance().await
    }

    async fn get_price(&self, symbol: &str) -> Result<Decimal> {
        let symbol = symbol.replace("/", "").to_uppercase();
        let url = format!(
            "{}/api/v3/ticker/price?symbol={}",
            self.client.base_url, symbol
        );

        let response = self.client.client
            .get(&url)
            .header("X-MBX-APIKEY", self.client.api_key.clone())   
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
