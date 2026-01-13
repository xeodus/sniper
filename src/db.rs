use crate::data::{Candles, Position, PositionSide, Signal};
use anyhow::Context;
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

pub struct Database {
    pub pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("Failed to connect to database!")?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    pub async fn save_order(&self, position: &Position, manual: bool) -> Result<()> {
        let opened = position.opened_at;
        let opened_at = Utc.timestamp_opt(opened, 0).single().unwrap();

        sqlx::query(
            r#"
            INSERT INTO trades (trade_id, symbol, side, entry_price, quantity,
            stop_loss, take_profit, opened_at, status, manual)
            VAlUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)               
            "# 
        )
        .bind(&position.id)
        .bind(&position.symbol)
        .bind(format!("{:?}", position.position_side))
        .bind(position.entry_price)
        .bind(position.size)
        .bind(position.stop_loss)
        .bind(position.take_profit)
        .bind(opened_at)
        .bind("open")
        .bind(manual)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn close_order(
        &self,
        trade_id: &str,
        exit_price: Decimal,
        pnl: Decimal,
    ) -> Result<()> {
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE trades
            SET closed_at = $1, exit_price = $2, pnl = $3, status = 'closed'
            WHERE trade_id = $4
            "# 
        )
        .bind(now)
        .bind(exit_price)
        .bind(pnl)
        .bind(trade_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn save_signal(&self, signal: Signal) -> Result<()> {
        let ts = signal.timestamp;
        let timestamp: DateTime<Utc> = Utc.timestamp_opt(ts, 0).single().unwrap();

        sqlx::query(
            r#"
            INSERT INTO signals (id, timestamp, symbol, action, price, confidence, trend)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "# 
        )
        .bind(&signal.id)
        .bind(timestamp)
        .bind(&signal.symbol)
        .bind(format!("{:?}", signal.action))
        .bind(signal.price)
        .bind(signal.confidence)
        .bind(format!("{:?}", signal.trend))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_open_orders(&self) -> Result<Vec<Position>> {
        let query = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Decimal,
                Decimal,
                Decimal,
                Decimal,
                DateTime<Utc>,
            ),
        >(
            r#"
            SELECT trade_id, symbol, side, entry_price, quantity, 
            stop_loss, take_profit, opened_at
            FROM trades 
            WHERE status = 'open'
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .unwrap();

        let position = query
            .into_iter()
            .map(|row| Position {
                id: row.0,
                symbol: row.1,
                position_side: if row.2 == "Long" {
                    PositionSide::Long
                } else {
                    PositionSide::Short
                },
                entry_price: row.3,
                size: row.4,
                stop_loss: row.5,
                take_profit: row.6,
                opened_at: row.7.timestamp(),
            })
            .collect();

        Ok(position)
    }

    pub async fn load_from_db(&self) -> Result<Vec<Candles>> {
        let query = sqlx::query_as::<_, (i64, Decimal, Decimal, Decimal, Decimal, Decimal)>(
            r#"
            SELECT timestamp, open, high, low, close, volume
            FROM candles
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let candle = query
            .into_iter()
            .map(|row| Candles {
                timestamp: row.0,
                open: row.1,
                high: row.2,
                low: row.3,
                close: row.4,
                volume: row.5,
            })
            .collect();

        Ok(candle)
    }
}
