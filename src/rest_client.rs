use crate::data::{OrderReq, Side};
use crate::sign::signature;
use anyhow::{anyhow, Result};
use chrono::Utc;
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
struct AccountInfo {
    balances: Vec<Balance>,
}

#[derive(Debug, Deserialize)]
struct Balance {
    asset: String,
    free: String,
}

pub struct BinanceClient {
    pub client: Client,
    pub base_url: String,
    pub api_key: String,
    pub api_secret: String,
}

#[allow(dead_code)]
impl BinanceClient {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Self {
        let base_url = if testnet {
            "https://testnet.binance.vision".to_string()
        } else {
            "https://api.binance.com".to_string()
        };

        Self {
            client: Client::new(),
            base_url,
            api_key,
            api_secret,
        }
    }

    pub async fn account_balance(&self) -> Result<Decimal> {
        let timestamp = Utc::now().timestamp_millis();
        let query_string = format!("recvWindow=5000&timestamp={}", timestamp);
        let sign = signature(self.api_secret.as_bytes(), &query_string);

        let url = format!(
            "{}/api/v3/account?{}&signature={}",
            self.base_url, query_string, sign
        );

        let response = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get account balance: {}", error_text));
        }

        let account: AccountInfo = response.json().await?;

        // Get USDT balance (or default to 0)
        let usdt_balance = account
            .balances
            .iter()
            .find(|b| b.asset == "USDT")
            .and_then(|b| b.free.parse::<Decimal>().ok())
            .unwrap_or(Decimal::ZERO);

        info!("Account USDT balance: {}", usdt_balance);
        Ok(usdt_balance)
    }

    pub async fn place_market_order(&self, req: &OrderReq) -> Result<String> {
        info!(
            "Placing market order {:?} for {} of size {} @ {}",
            req.side, req.symbol, req.size, req.price
        );

        let symbol = req.symbol.replace("/", "").to_uppercase();
        let side = side_to_string(&req.side);

        if req.size.is_zero() {
            return Err(anyhow!(
                "Refusing to place order of size zero for: {}",
                req.symbol
            ));
        }

        let timestamp = Utc::now().timestamp_millis();
        let body = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&newClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, side, req.size, req.id, timestamp
        );

        let sign = signature(self.api_secret.as_bytes(), &body);
        let url = format!("{}/api/v3/order", self.base_url);

        let response = self
            .client
            .post(format!("{}?{}&signature={}", url, body, sign))
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!(
                "Failed to place market order on Binance: {}",
                error_text
            ));
        }

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }

    pub async fn place_limit_order(&self, req: &OrderReq) -> Result<String> {
        info!(
            "Placing limit order {:?} for {} of size {} @ {}",
            req.side, req.symbol, req.size, req.price
        );

        let symbol = req.symbol.replace("/", "").to_uppercase();
        let side = side_to_string(&req.side);

        if req.size.is_zero() {
            return Err(anyhow!(
                "Refusing to place order of size zero for: {}",
                req.symbol
            ));
        }

        let timestamp = Utc::now().timestamp_millis();
        // Fixed: Using LIMIT order type with proper price and timeInForce
        let body = format!(
            "symbol={}&side={}&type=LIMIT&timeInForce=GTC&quantity={}&price={}&newClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, side, req.size, req.price, req.id, timestamp
        );

        let sign = signature(self.api_secret.as_bytes(), &body);
        let url = format!("{}/api/v3/order", self.base_url);

        let response = self
            .client
            .post(format!("{}?{}&signature={}", url, body, sign))
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!(
                "Failed to place limit order on Binance: {}",
                error_text
            ));
        }

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }

    pub async fn cancel_order(&self, req: &OrderReq) -> Result<String> {
        info!(
            "Cancelling order for ID {} and symbol {}",
            req.id, req.symbol
        );

        let timestamp = Utc::now().timestamp_millis();
        let symbol = req.symbol.replace("/", "").to_uppercase();
        let query_string = format!(
            "symbol={}&origClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, req.id, timestamp
        );

        let sign = signature(self.api_secret.as_bytes(), &query_string);
        let url = format!("{}/api/v3/order", self.base_url);

        let response = self
            .client
            .delete(format!("{}?{}&signature={}", url, query_string, sign))
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to cancel order on Binance: {}", error_text));
        }

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }

    pub async fn get_ticker_price(&self, symbol: &str) -> Result<Decimal> {
        let symbol = symbol.replace("/", "").to_uppercase();
        let url = format!("{}/api/v3/ticker/price?symbol={}", self.base_url, symbol);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get ticker price: {}", error_text));
        }

        #[derive(Deserialize)]
        struct TickerPrice {
            price: String,
        }

        let ticker: TickerPrice = response.json().await?;
        let price = ticker
            .price
            .parse::<Decimal>()
            .map_err(|e| anyhow!("Failed to parse price: {}", e))?;

        Ok(price)
    }

    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
    ) -> Result<Vec<crate::data::Candles>> {
        let symbol = symbol.replace("/", "").to_uppercase();
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&limit={}",
            self.base_url, symbol, interval, limit
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get klines: {}", error_text));
        }

        let data: Vec<Vec<serde_json::Value>> = response.json().await?;

        let candles: Vec<crate::data::Candles> = data
            .into_iter()
            .filter_map(|k| {
                Some(crate::data::Candles {
                    timestamp: k.first()?.as_i64()? / 1000,
                    open: k.get(1)?.as_str()?.parse().ok()?,
                    high: k.get(2)?.as_str()?.parse().ok()?,
                    low: k.get(3)?.as_str()?.parse().ok()?,
                    close: k.get(4)?.as_str()?.parse().ok()?,
                    volume: k.get(5)?.as_str()?.parse().ok()?,
                })
            })
            .collect();

        Ok(candles)
    }
}

fn side_to_string(side: &Side) -> &'static str {
    match side {
        Side::Buy => "BUY",
        Side::Sell => "SELL",
        Side::Hold => "HOLD", // This shouldn't be used for orders
    }
}
