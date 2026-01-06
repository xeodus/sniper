# Sniper Bot

![Build Status](https://github.com/xeodus/sniper/workflows/CI/badge.svg)
![Rust](https://img.shields.io/badge/Rust-006845?style=flat&logo=rust&logoColor=white&labelColor=333333)
![License](https://img.shields.io/badge/License-MIT%20-white.svg)

A high-performance cryptocurrency trading bot built in Rust for fast-paced trading environments. This low-latency, cross-platform bot leverages robust market algorithms and statistical models for high-frequency trading on exchanges like Binance.

## Features

- **Advanced Market Analysis** - RSI, EMA, MACD indicators with confidence-based signal generation
- **Risk Management** - Position sizing, stop-loss, and take-profit automation
- **Binance Integration** - REST API and WebSocket support for real-time trading
- **Position Management** - Track and manage multiple positions with automatic exit triggers
- **Backtesting Engine** - Test strategies against historical data
- **Discord Notifications** - Real-time alerts for signals, positions, and errors
- **PostgreSQL Storage** - Persistent storage for trades, signals, and candle data
- **Configuration Driven** - JSON configuration for easy customization
- **Memory Safe & Concurrent** - Built with Rust's safety guarantees and async/await

## Architecture

```
src/
├── main.rs              # Application entry point and orchestration
├── config.rs            # Configuration loading and validation
├── data.rs              # Data structures (positions, orders, signals, candles)
├── engine.rs            # Trading bot logic and order execution
├── signal.rs            # Market analysis and signal generation
├── position_manager.rs  # Position tracking and risk management
├── rest_client.rs       # Binance REST API client
├── websocket.rs         # Binance WebSocket client
├── notification.rs      # Discord webhook notifications
├── db.rs                # PostgreSQL database operations
├── backtesting.rs       # Backtesting engine
└── sign.rs              # API signature generation
```

## Requirements

- Rust 1.70+
- PostgreSQL 16+
- Binance API key and secret

## Quick Start

### 1. Clone the Repository

```bash
git clone https://github.com/xeodus/Sniper.git
cd Sniper
```

### 2. Set Up PostgreSQL

```bash
# Using Docker
docker-compose up -d

# Or connect to existing PostgreSQL instance
```

### 3. Configure Environment

Create a `.env` file with your credentials:

```bash
# Database
DATABASE_URL=postgresql://ricky:password@localhost:5432/sniper

# Binance API (get from https://www.binance.com/en/my/settings/api-management)
API_KEY=your_binance_api_key
SECRET_KEY=your_binance_secret_key

# Discord Notifications (optional)
WEBHOOK_URL=https://discord.com/api/webhooks/your_webhook_url
```

### 4. Configure Trading Parameters

Edit `config.json`:

```json
{
  "symbol": "ETH/USDT",
  "timeframe": "1m",
  "size": 1,
  "risk_per_trade": 2.0,
  "max_positions": 3,
  "min_confidence": 0.7,
  "stop_loss_percent": 2.0,
  "take_profit_percent": 4.0,
  "testnet": true,
  "notifications_enabled": true
}
```

### 5. Build and Run

```bash
# Build
cargo build --release

# Run
cargo run --release
```

## Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `symbol` | string | "ETH/USDT" | Trading pair |
| `timeframe` | string | "1m" | Candle timeframe |
| `risk_per_trade` | float | 2.0 | Percentage of account to risk per trade |
| `max_positions` | int | 3 | Maximum concurrent positions |
| `min_confidence` | float | 0.7 | Minimum signal confidence (0.0-1.0) |
| `stop_loss_percent` | float | 2.0 | Stop loss percentage |
| `take_profit_percent` | float | 4.0 | Take profit percentage |
| `testnet` | bool | true | Use Binance testnet |
| `notifications_enabled` | bool | true | Enable Discord notifications |

## Development

### Running Tests

```bash
cargo test
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings
```

### Database Migrations

```bash
# Install sqlx CLI
cargo install sqlx-cli --no-default-features --features postgres,rustls

# Run migrations
cargo sqlx migrate run

# Prepare offline queries
cargo sqlx prepare
```

## Signal Generation

The bot uses a combination of technical indicators:

1. **RSI (Relative Strength Index)** - Identifies overbought/oversold conditions
2. **EMA (Exponential Moving Average)** - Trend detection using 12/26 period EMAs
3. **MACD (Moving Average Convergence Divergence)** - Momentum confirmation

Signals are generated with a confidence score (0-100%) based on:
- RSI extremes (<30 or >70)
- MACD crossover strength
- Trend direction alignment

## Risk Management

- **Position Sizing**: Risk-based position sizing using account balance and stop-loss distance
- **Stop Loss**: Automatic stop-loss orders at configurable percentage
- **Take Profit**: Automatic take-profit orders at configurable percentage
- **Single Position**: Only one position per symbol at a time

## API Endpoints Used

### REST API
- `GET /api/v3/account` - Account balance
- `POST /api/v3/order` - Place orders
- `DELETE /api/v3/order` - Cancel orders
- `GET /api/v3/ticker/price` - Current price
- `GET /api/v3/klines` - Historical candles

### WebSocket
- `wss://stream.binance.com:9443/ws/{symbol}@kline_{interval}` - Real-time klines

## Database Schema

```sql
-- Trades table
CREATE TABLE trades (
    trade_id TEXT PRIMARY KEY,
    symbol VARCHAR(50) NOT NULL,
    side TEXT NOT NULL,
    entry_price DECIMAL(20, 8) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    stop_loss DECIMAL(20, 8),
    take_profit DECIMAL(20, 8),
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ,
    exit_price DECIMAL(20, 8),
    pnl DECIMAL(20, 8),
    status VARCHAR(20) NOT NULL,
    manual BOOLEAN DEFAULT FALSE
);

-- Signals table
CREATE TABLE signals (
    id TEXT PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    action TEXT NOT NULL,
    price DECIMAL(20, 8) NOT NULL,
    confidence DECIMAL(5, 4) NOT NULL,
    trend TEXT NOT NULL
);

-- Candles table
CREATE TABLE candles (
    id SERIAL PRIMARY KEY,
    symbol VARCHAR(50) NOT NULL,
    timestamp BIGINT NOT NULL,
    open DECIMAL(20, 8) NOT NULL,
    high DECIMAL(20, 8) NOT NULL,
    low DECIMAL(20, 8) NOT NULL,
    close DECIMAL(20, 8) NOT NULL,
    volume DECIMAL(20, 8) NOT NULL,
    UNIQUE(symbol, timestamp)
);
```

## Safety Notice

⚠️ **Use at your own risk.** Trading cryptocurrencies involves substantial risk of loss. This bot is provided as-is without any guarantees. Always:

- Start with testnet mode (`testnet: true`)
- Use only funds you can afford to lose
- Monitor the bot's activity
- Understand the trading strategy before deploying with real funds

## License

MIT License - see [LICENSE](LICENSE) for details.
