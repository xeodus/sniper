use anyhow::{Context, Result};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::Serialize;
use tracing::{error, info};

use crate::data::{Position, PositionSide, Side, Signal};

/// Discord webhook notification service
pub struct NotificationService {
    client: Client,
    webhook_url: Option<String>,
    enabled: bool,
}

#[derive(Serialize)]
struct DiscordMessage {
    content: Option<String>,
    embeds: Option<Vec<DiscordEmbed>>,
}

#[derive(Serialize)]
struct DiscordEmbed {
    title: String,
    description: Option<String>,
    color: u32,
    fields: Vec<DiscordField>,
    timestamp: Option<String>,
}

#[derive(Serialize)]
struct DiscordField {
    name: String,
    value: String,
    inline: bool,
}

#[allow(dead_code)]
impl NotificationService {
    /// Create a new notification service
    pub fn new(webhook_url: Option<String>, enabled: bool) -> Self {
        Self {
            client: Client::new(),
            webhook_url,
            enabled,
        }
    }

    /// Create from environment variable
    pub fn from_env() -> Self {
        let webhook_url = std::env::var("WEBHOOK_URL").ok();
        let enabled = webhook_url.is_some();
        Self::new(webhook_url, enabled)
    }

    /// Check if notifications are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.webhook_url.is_some()
    }

    /// Send a raw message
    async fn send(&self, message: DiscordMessage) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let url = self
            .webhook_url
            .as_ref()
            .context("Webhook URL not configured")?;

        let response = self
            .client
            .post(url)
            .json(&message)
            .send()
            .await
            .context("Failed to send Discord notification")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Discord webhook error: {}", error_text);
        }

        Ok(())
    }

    /// Send a simple text notification
    pub async fn send_text(&self, text: &str) -> Result<()> {
        info!("Sending notification: {}", text);

        let message = DiscordMessage {
            content: Some(text.to_string()),
            embeds: None,
        };

        self.send(message).await
    }

    /// Notify about a new signal
    pub async fn notify_signal(&self, signal: &Signal) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let color = match signal.action {
            Side::Buy => 0x00FF00,  // Green
            Side::Sell => 0xFF0000, // Red
            Side::Hold => 0xFFFF00, // Yellow
        };

        let action_emoji = match signal.action {
            Side::Buy => "🟢",
            Side::Sell => "🔴",
            Side::Hold => "🟡",
        };

        let embed = DiscordEmbed {
            title: format!("{} Trading Signal: {}", action_emoji, signal.symbol),
            description: Some(format!(
                "New **{:?}** signal detected with **{:.1}%** confidence",
                signal.action,
                signal.confidence * Decimal::new(100, 0)
            )),
            color,
            fields: vec![
                DiscordField {
                    name: "Symbol".to_string(),
                    value: signal.symbol.clone(),
                    inline: true,
                },
                DiscordField {
                    name: "Action".to_string(),
                    value: format!("{:?}", signal.action),
                    inline: true,
                },
                DiscordField {
                    name: "Price".to_string(),
                    value: format!("${}", signal.price),
                    inline: true,
                },
                DiscordField {
                    name: "Trend".to_string(),
                    value: format!("{:?}", signal.trend),
                    inline: true,
                },
                DiscordField {
                    name: "Confidence".to_string(),
                    value: format!("{:.2}%", signal.confidence * Decimal::new(100, 0)),
                    inline: true,
                },
            ],
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        };

        let message = DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
        };

        self.send(message).await
    }

    /// Notify about a new position opened
    pub async fn notify_position_opened(&self, position: &Position) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let (color, emoji) = match position.position_side {
            PositionSide::Long => (0x00FF00, "📈"),
            PositionSide::Short => (0xFF0000, "📉"),
        };

        let embed = DiscordEmbed {
            title: format!("{} Position Opened: {}", emoji, position.symbol),
            description: Some(format!(
                "New **{:?}** position opened",
                position.position_side
            )),
            color,
            fields: vec![
                DiscordField {
                    name: "Entry Price".to_string(),
                    value: format!("${}", position.entry_price),
                    inline: true,
                },
                DiscordField {
                    name: "Size".to_string(),
                    value: format!("{}", position.size),
                    inline: true,
                },
                DiscordField {
                    name: "Stop Loss".to_string(),
                    value: format!("${}", position.stop_loss),
                    inline: true,
                },
                DiscordField {
                    name: "Take Profit".to_string(),
                    value: format!("${}", position.take_profit),
                    inline: true,
                },
            ],
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        };

        let message = DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
        };

        self.send(message).await
    }

    /// Notify about a position closed
    pub async fn notify_position_closed(
        &self,
        position: &Position,
        exit_price: Decimal,
        pnl: Decimal,
    ) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let (color, emoji) = if pnl > Decimal::ZERO {
            (0x00FF00, "✅")
        } else {
            (0xFF0000, "❌")
        };

        let pnl_percent =
            ((exit_price - position.entry_price) / position.entry_price) * Decimal::new(100, 0);

        let embed = DiscordEmbed {
            title: format!("{} Position Closed: {}", emoji, position.symbol),
            description: Some(format!(
                "**{:?}** position closed with **{}{}** PnL",
                position.position_side,
                if pnl > Decimal::ZERO { "+" } else { "" },
                pnl
            )),
            color,
            fields: vec![
                DiscordField {
                    name: "Entry Price".to_string(),
                    value: format!("${}", position.entry_price),
                    inline: true,
                },
                DiscordField {
                    name: "Exit Price".to_string(),
                    value: format!("${}", exit_price),
                    inline: true,
                },
                DiscordField {
                    name: "PnL".to_string(),
                    value: format!(
                        "${} ({}{:.2}%)",
                        pnl,
                        if pnl_percent > Decimal::ZERO { "+" } else { "" },
                        pnl_percent
                    ),
                    inline: true,
                },
                DiscordField {
                    name: "Size".to_string(),
                    value: format!("{}", position.size),
                    inline: true,
                },
            ],
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        };

        let message = DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
        };

        self.send(message).await
    }

    /// Notify about an error
    pub async fn notify_error(&self, error: &str) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let embed = DiscordEmbed {
            title: "⚠️ Error".to_string(),
            description: Some(error.to_string()),
            color: 0xFF0000,
            fields: vec![],
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        };

        let message = DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
        };

        self.send(message).await
    }

    /// Send bot startup notification
    pub async fn notify_startup(&self, symbol: &str, timeframe: &str) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let embed = DiscordEmbed {
            title: "🚀 Sniper Bot Started".to_string(),
            description: Some("Trading bot is now online and monitoring the market".to_string()),
            color: 0x00BFFF, // Blue
            fields: vec![
                DiscordField {
                    name: "Symbol".to_string(),
                    value: symbol.to_string(),
                    inline: true,
                },
                DiscordField {
                    name: "Timeframe".to_string(),
                    value: timeframe.to_string(),
                    inline: true,
                },
            ],
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        };

        let message = DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
        };

        self.send(message).await
    }

    /// Send bot shutdown notification
    pub async fn notify_shutdown(&self) -> Result<()> {
        if !self.is_enabled() {
            return Ok(());
        }

        let embed = DiscordEmbed {
            title: "🛑 Sniper Bot Stopped".to_string(),
            description: Some("Trading bot is shutting down".to_string()),
            color: 0x808080, // Gray
            fields: vec![],
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
        };

        let message = DiscordMessage {
            content: None,
            embeds: Some(vec![embed]),
        };

        self.send(message).await
    }
}
