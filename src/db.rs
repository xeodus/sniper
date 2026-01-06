use crate::data::{Candles, Position, PositionSide, Signal};
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

pub struct Database {
    pub pool: PgPool,
}

#[allow(dead_code)]
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
        let opened_at = Utc
            .timestamp_opt(opened, 0)
            .single()
            .context("Invalid timestamp")?;

        sqlx::query!(
            r#"
            INSERT INTO trades (trade_id, symbol, side, entry_price, quantity,
            stop_loss, take_profit, opened_at, status, manual)
            VAlUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)               
            "#,
            position.id,
            position.symbol,
            format!("{:?}", position.position_side),
            position.entry_price,
            position.size,
            position.stop_loss,
            position.take_profit,
            opened_at,
            "open",
            manual
        )
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
        sqlx::query!(
            r#"
            UPDATE trades
            SET closed_at = $1, exit_price = $2, pnl = $3, status = 'closed'
            WHERE trade_id = $4
            "#,
            now,
            exit_price,
            pnl,
            trade_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn save_signal(&self, signal: &Signal) -> Result<()> {
        let ts = signal.timestamp;
        let timestamp: DateTime<Utc> = Utc
            .timestamp_opt(ts, 0)
            .single()
            .context("Invalid timestamp")?;

        sqlx::query!(
            r#"
            INSERT INTO signals (id, timestamp, symbol, action, price, confidence, trend)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            signal.id,
            timestamp,
            signal.symbol,
            format!("{:?}", signal.action),
            signal.price,
            signal.confidence,
            format!("{:?}", signal.trend)
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn save_candle(&self, candle: &Candles, symbol: &str) -> Result<()> {
        // Use runtime query to avoid need for sqlx prepare
        sqlx::query(
            r#"
            INSERT INTO candles (symbol, timestamp, open, high, low, close, volume)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (symbol, timestamp) DO UPDATE SET
                open = EXCLUDED.open,
                high = EXCLUDED.high,
                low = EXCLUDED.low,
                close = EXCLUDED.close,
                volume = EXCLUDED.volume
            "#,
        )
        .bind(symbol)
        .bind(candle.timestamp)
        .bind(candle.open)
        .bind(candle.high)
        .bind(candle.low)
        .bind(candle.close)
        .bind(candle.volume)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_open_orders(&self) -> Result<Vec<Position>> {
        let rows = sqlx::query_as::<
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
        .await?;

        let positions = rows
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

        Ok(positions)
    }

    pub async fn load_candles(&self, symbol: &str, limit: i64) -> Result<Vec<Candles>> {
        let rows = sqlx::query_as::<_, (i64, Decimal, Decimal, Decimal, Decimal, Decimal)>(
            r#"
            SELECT timestamp, open, high, low, close, volume
            FROM candles
            WHERE symbol = $1
            ORDER BY timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(symbol)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let candles = rows
            .into_iter()
            .rev() // Reverse to get oldest first
            .map(|row| Candles {
                timestamp: row.0,
                open: row.1,
                high: row.2,
                low: row.3,
                close: row.4,
                volume: row.5,
            })
            .collect();

        Ok(candles)
    }

    /// Load all candles from the database (for backtesting)
    pub async fn load_from_db(&self) -> Result<Vec<Candles>> {
        let rows = sqlx::query_as::<_, (i64, Decimal, Decimal, Decimal, Decimal, Decimal)>(
            r#"
            SELECT timestamp, open, high, low, close, volume
            FROM candles
            ORDER BY timestamp ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let candles = rows
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

        Ok(candles)
    }

    pub async fn get_trade_stats(&self, symbol: &str) -> Result<(i64, i64, Decimal)> {
        let result = sqlx::query_as::<_, (i64, i64, Option<Decimal>)>(
            r#"
            SELECT 
                COUNT(*) FILTER (WHERE pnl > 0) as wins,
                COUNT(*) FILTER (WHERE pnl <= 0) as losses,
                SUM(pnl) as total_pnl
            FROM trades
            WHERE symbol = $1 AND status = 'closed'
            "#,
        )
        .bind(symbol)
        .fetch_one(&self.pool)
        .await?;

        Ok((result.0, result.1, result.2.unwrap_or(Decimal::ZERO)))
    }
}
