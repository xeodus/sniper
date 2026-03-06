use crate::data::{OrderRequest, Side};
use crate::sign::signature;
use anyhow::{anyhow, Result};
use chrono::Utc;
use reqwest::Client;
use rust_decimal::Decimal;
use tracing::info;

pub struct BinanceClient {
    pub client: Client,
    pub base_url: String,
    pub api_key: String,
    pub api_secret: String,
}

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
            client: Client::new(),
            base_url,
            api_key,
            api_secret
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

    pub async fn place_market_order(&self, req: &OrderRequest) -> Result<String> { 
        let req_ = match req {
            OrderRequest::Hyperliquid(hl_req) => hl_req,
            _  => {
                return Err(anyhow!("Hyperliquid exchange received non-hyperliquid order request"));
            } 
        };
        //let symbol = req_.base.symbol.replace("-", "").to_uppercase();
        let timestamp = Utc::now().timestamp_millis();

        /*let side = match req_.base.side {
            Side::Buy => "BUY".to_string(),
            Side::Sell => "SELL".to_string(),
            Side::Hold => "HOLD".to_string()
        };*/

        if req_.base.size.is_zero() {
            return Err(anyhow!("Refusing to place market order for size zero: {}", req_.base.symbol));
        }

        let url = format!("{}/exchange",self.base_url);
        let order = serde_json::json!({
            "a": req_.asset_index,
            "b": req_.is_buy,
            "p": req_.base.price,
            "s": req_.base.size,
            "r": req_.reduce_only,
            "t": { "market": {"tif": "Ioc"} },
            "c": req_.cloid
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
            return Err(anyhow!("Invalid response: {:?}", response.text().await?));
        }

        info!("Placing market order {:?} for {} of size {} @ {}", req_.base.side, req_.base.symbol, req_.base.size, req_.base.price);

        Ok(response.text().await?)
    }

    pub async fn place_limit_order(&self, req: &OrderRequest) -> Result<String> { 
        let req_ = match req {
            OrderRequest::Hyperliquid(hl_req) => hl_req,
            _ => {
                return Err(anyhow!("Hyperliquid exchange received non-hyperliquid order requests"));
            }
        };
        //let symbol = req_.base.symbol.replace("-", "").to_uppercase();

        /*let side = match req_.base.side {
            Side::Buy => "BUY".to_string(),
            Side::Sell => "SELL".to_string(),
            Side::Hold => "HOLD".to_string()
        };*/

        let timestamp = Utc::now().timestamp_millis();
        let url = format!("{}/exchange", self.base_url); 

        let order = serde_json::json!({
            "a": req_.asset_index,
            "b": req_.is_buy,
            "p": req_.base.price,
            "s": req_.base.size,
            "r": req_.reduce_only,
            "t": { "limit": {"tif": "Gtc"}},
            "c": req_.cloid
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
            return Err(anyhow!("Invalid response received: {:?}", response.text().await?));
        }

        info!("Placing limit order {:?} for {} of size {} @ {}", req_.base.side, req_.base.symbol, req_.base.size, req_.base.price);

        Ok(response.text().await?)
    }

    pub async fn cancel_orders(&self, req: &OrderRequest) -> Result<String> {
        let req_ = match req {
            OrderRequest::Hyperliquid(hl_req) => hl_req,
            _ => {
                return Err(anyhow!("Hyperliquid exchange received non-hyperliquid order requests"));
            }
        };

        //let symbol = req_.base.symbol.replace("-", "").to_uppercase();
        let url = format!("{}/exchange", self.base_url);
        let timestamp = Utc::now().timestamp_millis();

        let action = serde_json::json!({
            "type": "cancel",
            "cancels": [{
                "a": req_.asset_index,
                "o": req_.base.id
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
            return Err(anyhow!("Invalid response received: {}", response.text().await?));
        }

        info!("Cancelling the order for ID {} and symbol {}", req_.base.id, req_.base.symbol);

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

    pub async fn place_market_order(&self, req: &OrderRequest) -> Result<String> {
        let req_ = match req {
            OrderRequest::Binance(bn_req) => bn_req,
            _ => return Err(anyhow!("Binance exchange received a non-Binance order request")),
        };

        if req_.base.size.is_zero() {
            return Err(anyhow!("Refusing to place order of size zero for: {}", req_.base.symbol));
        }

        let symbol = req_.base.symbol.replace("/", "").to_uppercase();
        let side = match req_.base.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
            Side::Hold => "HOLD",
        };

        let body = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&newClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, side, req_.base.size, req_.base.id, Utc::now().timestamp_millis()
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

        info!("Placing market order {:?} for {} of size {} @ {}", req_.base.side, req_.base.symbol, req_.base.size, req_.base.price);

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }

    pub async fn place_limit_order(&self, req: &OrderRequest) -> Result<String> {
        let req_ = match req {
            OrderRequest::Binance(bn_req) => bn_req,
            _ => return Err(anyhow!("Binance exchange received a non-Binance order request")),
        };

        if req_.base.size.is_zero() {
            return Err(anyhow!("Refusing to place order of size zero for: {}", req_.base.symbol));
        }

        let symbol = req_.base.symbol.replace("/", "").to_uppercase();
        let side = match req_.base.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
            Side::Hold => "HOLD",
        };

        let body = format!(
            "symbol={}&side={}&type=LIMIT&quantity={}&price={}&timeInForce=GTC&newClientOrderId={}&recvWindow=5000&timestamp={}",
            symbol, side, req_.base.size, req_.base.price, req_.base.id, Utc::now().timestamp_millis()
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

        info!("Placing limit order {:?} for {} of size {} @ {}", req_.base.side, req_.base.symbol, req_.base.size, req_.base.price);

        let res = response.json::<serde_json::Value>().await?;
        Ok(res.to_string())
    }

    pub async fn cancel_orders(&self, req: &OrderRequest) -> Result<String> {
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
