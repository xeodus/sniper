use crate::data::{HyperliquidRequest, OrderReq, Side};
use crate::sign::signature;
use anyhow::{anyhow, Result};
use chrono::Utc;
use reqwest::Client;
use rust_decimal::Decimal;
use tracing::info;

#[derive(Debug)]
pub struct BinanceClient {
    pub client: Client,
    pub base_url: String,
    pub api_key: String,
    pub api_secret: String,
}

#[derive(Debug)]
pub struct HyperliquidClient {
    pub client: Client,
    pub base_url: String,
    pub api_key: String,
    pub api_secret: String
}

impl HyperliquidClient {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Self {
        let base_url = if testnet {
            "https://api.hyperliquid-testnet.xyz".to_string()
        }
        else {
            "https://api.hyperliquid.xyz".to_string()
        };

        Self {
            client: Client::builder()
                .tcp_nodelay(true)
                .pool_idle_timeout(None)
                .build()
                .expect("Failed to build reqwest client"),
            api_key,
            api_secret,
            base_url
        }
    }

    pub async fn account_balance(&self) -> Result<Decimal> {
        let url = format!("{}/info", self.base_url);
        let body = serde_json::json!({
            "type": "clearinghouseState",
            "user": self.api_key
        });

        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response received: {:?}", response.text().await?));
        }

        let res = response.json::<serde_json::Value>().await?;
        let balance_str = res["marginSummary"]["accountValue"].as_str()
            .ok_or_else(|| anyhow!("accountValue not found in the response!"))?;
        let balance = Decimal::from_str_exact(balance_str)
            .map_err(|e| anyhow!("Failed to parse balance as decimal: {}", e))?;

        Ok(balance)
    }

    pub async fn place_market_order(&self, req: &OrderReq) -> Result<String> { 
        let timestamp = Utc::now().timestamp_millis();

        if req.size.is_zero() {
            return Err(anyhow!("Refusing HL market order for size zero: {}", req.symbol));
        }

        let is_buy = req.side == Side::Buy;

        let url = format!("{}/exchange",self.base_url);
        let order = serde_json::json!({
            "a": req.asset_index,
            "b": is_buy,
            "p": req.price,
            "s": req.size,
            "r": req.reduce_only,
            "t": { "market": {"tif": "Ioc"} },
            "c": req.cloid
        });

        let action = serde_json::json!({
            "type": "order",
            "orders": [order],
            "grouping": "na"
        });

        let signature = signature(self.api_secret.as_bytes(), &action.to_string()).await;

        let body = serde_json::json!({
            "action": action,
            "nonce": timestamp,
            "signature": signature
        });

        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response from HL market otder: {:?}", response.text().await?));
        }

        info!("Placing market order on HL {:?} for {} of size {} @ {}", req.side, req.symbol, req.size, req.price);

        Ok(response.text().await?)
    }

    pub async fn place_limit_order(&self, req: &OrderReq) -> Result<String> {
        let timestamp = Utc::now().timestamp_millis();

        if req.size.is_zero() {
            return Err(anyhow!("Refusing HL limit order for size zero: {}", req.symbol));
        }
        
        let is_buy = req.side == Side::Buy;
        let url = format!("{}/exchange", self.base_url); 

        let order = serde_json::json!({
            "a": req.asset_index,
            "b": is_buy,
            "p": req.price,
            "s": req.size,
            "r": req.reduce_only,
            "t": { "limit": {"tif": "Gtc"}},
            "c": req.cloid
        });

        let action = serde_json::json!({
            "type": "order",
            "orders": [order],
            "grouping": "na"
        });
        
        let signature = signature(self.api_secret.as_bytes(), &action.to_string()).await;

        let body = serde_json::json!({
            "action": action,
            "nonce": timestamp,
            "signature": signature
        });

        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response received HL limit order: {:?}", response.text().await?));
        }

        info!("Placing limit order {:?} for {} of size {} @ {}", req.side, req.symbol, req.size, req.price);

        Ok(response.text().await?)
    }

    pub async fn cancel_orders(&self, req: &OrderReq) -> Result<String> {
        let url = format!("{}/exchange", self.base_url);
        let timestamp = Utc::now().timestamp_millis();

        let action = serde_json::json!({
            "type": "cancel",
            "cancels": [{
                "a": req.asset_index,
                "o": req.id
            }]
        });

        let signature = signature(self.api_secret.as_bytes(), &action.to_string()).await;

        let body = serde_json::json!({
            "action": action,
            "nonce": timestamp,
            "signature": signature
        });

        let response = self.client.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response received while cancelling order for HL: {}", response.text().await?));
        }

        info!("Cancelling the order for ID {} and symbol {}", req.id, req.symbol);

        Ok(response.text().await?)
    }
}

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
        let sign = signature(self.api_secret.as_bytes(), &query_string).await;
        let url = format!("{}/api/v3/account?{}&signature={}", self.base_url, query_string, sign);
        let response = self.client.get(&url)
            .header("X-MBX-APIKEY", self.api_key.clone())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response received: {}", response.text().await?));
        }

        let res = response.json::<serde_json::Value>().await?;
        let balance_str = res["balances"]
            .as_array()
            .ok_or_else(|| anyhow!("No balances array in response"))?
            .iter()
            .find(|b| b["asset"].as_str() == Some("USDT"))
            .ok_or_else(|| anyhow!("USDT balances not found"))?["free"]
            .as_str()
            .ok_or_else(|| anyhow!("Failed to parse balance as Decimal"))?;

        let balance = Decimal::from_str_exact(balance_str)
            .map_err(|e| anyhow!("Failed to parse balance as decimal: {}", e))?;

        Ok(balance)
    }

    pub async fn place_market_order(&self, req: &OrderReq) -> Result<String> {
        if req.size.is_zero() {
            return Err(anyhow!("Refusing to place order of size zero for: {}", req.symbol));
        }

        let symbol = req.symbol.replace("/", "").to_uppercase();
        let side = match req.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
            Side::Hold => "HOLD",
        };

        let body = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&newClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, side, req.size, req.id, Utc::now().timestamp_millis()
        );

        let url = format!("{}/api/v3/order", self.base_url);
        let sign = signature(self.api_secret.as_bytes(), &body).await;
        let response = self.client
            .post(format!("{}?{}&signature={}", url, body, sign))
            .header("X-MBX-APIKEY", self.api_key.clone())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response while placing market order on Binance: {:?}", response.text().await));
        }

        info!("Placing market order {:?} for {} of size {} @ {}", req.side, req.symbol, req.size, req.price);

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }

    pub async fn place_limit_order(&self, req: &OrderReq) -> Result<String> {
        if req.size.is_zero() {
            return Err(anyhow!("Refusing to place order of size zero for: {}", req.symbol));
        }

        let symbol = req.symbol.replace("/", "").to_uppercase();
        let side = match req.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
            Side::Hold => "HOLD",
        };

        let body = format!(
            "symbol={}&side={}&type=LIMIT&quantity={}&price={}&timeInForce=GTC&newClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, side, req.size, req.price, req.id, Utc::now().timestamp_millis()
        );

        let url = format!("{}/api/v3/order", self.base_url);
        let sign = signature(self.api_secret.as_bytes(), &body).await;
        let response = self.client
            .post(format!("{}?{}&signature={}", url, body, sign))
            .header("X-MBX-APIKEY", self.api_key.clone())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response while placing limit order on Binance: {:?}", response.text().await));
        }

        info!("Placing limit order {:?} for {} of size {} @ {}", req.side, req.symbol, req.size, req.price);

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }

    pub async fn cancel_orders(&self, req: &OrderReq) -> Result<String> {
        let req_ = match req {
            OrderRequest::Binance(bn_req) => bn_req,
            _ => return Err(anyhow!("Binance exchange received a non-Binance order request")),
        };

        let symbol = req_.base.symbol.replace("/", "").to_uppercase();
        let query_string = format!(
            "symbol={}&originClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, req_.base.id, Utc::now().timestamp_millis()
        );

        let url = format!("{}/api/v3/order", self.base_url);
        let sign = signature(self.api_secret.as_bytes(), &query_string).await;
        let response = self.client
            .delete(format!("{}?{}&signature={}", url, query_string, sign))
            .header("X-MBX-APIKEY", self.api_key.clone())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid response while cancelling order on Binance: {:?}", response.text().await));
        }

        info!("Cancelling order for ID {} and symbol {}", req_.base.id, req_.base.symbol);

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }
}
