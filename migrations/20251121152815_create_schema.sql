-- Trades table for storing trade positions
CREATE TABLE IF NOT EXISTS trades (
    trade_id TEXT NOT NULL PRIMARY KEY,
    symbol VARCHAR(50) NOT NULL,
    side TEXT NOT NULL, 
    entry_price DECIMAL(20, 8) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    stop_loss DECIMAL(20, 8),
    take_profit DECIMAL(20, 8),
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ,  -- Nullable: open trades don't have a close time
    exit_price DECIMAL(20, 8),
    pnl DECIMAL(20, 8),
    status VARCHAR(20) NOT NULL DEFAULT 'open',
    manual BOOLEAN NOT NULL DEFAULT FALSE
);

-- Signals table for storing trading signals
CREATE TABLE IF NOT EXISTS signals (
    id TEXT NOT NULL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    action TEXT NOT NULL,
    price DECIMAL(20, 8) NOT NULL,
    confidence DECIMAL(5, 4) NOT NULL,
    trend TEXT NOT NULL
);

-- Candles table for storing historical price data
CREATE TABLE IF NOT EXISTS candles (
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

-- Indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);
CREATE INDEX IF NOT EXISTS idx_signals_timestamp ON signals(timestamp);
CREATE INDEX IF NOT EXISTS idx_signals_symbol ON signals(symbol);
CREATE INDEX IF NOT EXISTS idx_candles_timestamp ON candles(timestamp);
CREATE INDEX IF NOT EXISTS idx_candles_symbol ON candles(symbol);
