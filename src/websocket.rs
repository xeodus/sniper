use crate::data::{BinanceKlineEvent, Candles};
use anyhow::{Context, Result};
use futures_util::StreamExt;
use rust_decimal::Decimal;
use std::str::FromStr;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

pub struct WebSocketClient {
    pub url: String,
}

impl WebSocketClient {
    pub fn new(symbol: &str, interval: &str) -> Self {
        let symbol_lower = symbol.to_lowercase().replace("/", "");
        let url = format!(
            "wss://stream.binance.com:9443/ws/{}@kline_{}",
            symbol_lower,
            interval.to_lowercase()
        );

        info!("ws url: {}", url);

        Self { url }
    }

    pub async fn connect(&self) -> Result<impl StreamExt<Item = Result<Candles, anyhow::Error>>> {
        let (ws_srteam, response) = connect_async(&self.url)
            .await
            .context("WebSocket connection failed")?;

        info!(
            "Connected to Binance WebSocket. HTTP status: {}",
            response.status()
        );

        let (_, read) = ws_srteam.split();
        let stream = read.filter_map(|msg| async move {
            match msg {
                Ok(Message::Text(text)) => {
                    let evt: BinanceKlineEvent = match serde_json::from_str(&text) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("Failed to parse raw json from WebSocket stream: {}", e);
                            return None;
                        }
                    };

                    let k = evt.kline;

                    if let (Ok(open), Ok(high), Ok(low), Ok(close), Ok(volume)) = (
                        Decimal::from_str(&k.open),
                        Decimal::from_str(&k.high),
                        Decimal::from_str(&k.low),
                        Decimal::from_str(&k.close),
                        Decimal::from_str(&k.volume),
                    ) {
                        Some(Ok(Candles {
                            timestamp: k.open_time / 1000,
                            open,
                            high,
                            low,
                            close,
                            volume,
                        }))
                    } else {
                        warn!("Failed to parse OHLCV decimals from kline: {:?}", k);
                        None
                    }
                }
                Ok(Message::Ping(_) | Message::Pong(_)) => None,
                Ok(Message::Close(frame)) => {
                    info!("WebSocket closed by peer: {:?}", frame);
                    None
                }
                Err(e) => Some(Err(anyhow::anyhow!("Failed to connect WebSocket: {}", e))),
                _ => None,
            }
        });

        Ok(stream)
    }
}
