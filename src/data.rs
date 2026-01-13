use crate::{
    db::Database,
    exchange::Exchange,
    position_manager::PositionManager,
    signal::MarketSignal,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[derive(Debug, Clone, Copy)]
pub enum PositionSide {
    Long,
    Short
}

#[derive(Debug, Clone, PartialEq)]
pub enum Side {
    Buy,
    Sell,
    Hold
}

#[derive(Debug, Clone)]
pub enum OrderType {
    Market,
    Limit
}

#[derive(Debug, Clone, PartialEq)]
pub enum Trend {
    Up,
    Down,
    Sideways
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Position {
    pub id: String,
    pub symbol: String,
    pub position_side: PositionSide,
    pub entry_price: Decimal,
    pub size: Decimal,
    pub stop_loss: Decimal,
    pub take_profit: Decimal,
    pub opened_at: i64
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Candles {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub timestamp: i64
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OrderReq {
    pub id: String,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Decimal,
    pub size: Decimal,
    pub sl: Option<Decimal>,
    pub tp: Option<Decimal>,
    pub manual: bool
}

#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    pub id: String,
    pub timestamp: i64,
    pub symbol: String,
    pub action: Side,
    pub price: Decimal,
    pub trend: Trend,
    pub confidence: Decimal
}

#[allow(dead_code)]
pub struct TradingBot {
    pub analyzer: Arc<RwLock<MarketSignal>>,
    pub position_manager: Arc<PositionManager>,
    pub exchange: Arc<dyn Exchange>,
    pub signal_tx: mpsc::Sender<Signal>,
    pub order_tx: mpsc::Sender<OrderReq>,
    pub account_balance: Arc<RwLock<Decimal>>,
    pub db: Arc<Database>
}

#[derive(Debug, Clone, Deserialize)]
pub struct BinanceKline {
    #[serde(rename = "t")]
    pub open_time: i64,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "v")]
    pub volume: String
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceKlineEvent {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "k")]
    pub kline: BinanceKline
}
