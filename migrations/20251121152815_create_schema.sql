CREATE TABLE IF NOT EXISTS trades (
    trade_id TEXT NOT NULL PRIMARY KEY,
    symbol VARCHAR(50) NOT NULL,
    side TEXT NOT NULL, 
    entry_price DECIMAL(20, 8) NOT NULL,
    quantity DECIMAL(20, 8) NOT NULL,
    stop_loss DECIMAL(20, 8),
    take_profit DECIMAL(20, 8),
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ NOT NULL,
    exit_price DECIMAL(20, 8),
    pnl DECIMAL(20, 8),
    status VARCHAR(20) NOT NULL,
    manual BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS signals (
    id TEXT NOT NULL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(50) NOT NULL,
    action TEXT NOT NULL,
    price DECIMAL(20, 8) NOT NULL,
    confidence DECIMAL(5, 4) NOT NULL,
    trend TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS candles (
    timestamp TIMESTAMPTZ NOT NULL,
    open DECIMAL(20, 8),
    high DECIMAL(20, 8),
    low DECIMAL(20, 8),
    close DECIMAL(20, 8),
    volume DECIMAL(20, 8)
);

CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
CREATE INDEX IF NOT EXISTS idx_trades_status ON trades(status);
CREATE INDEX IF NOT EXISTS idx_signals_timestamp ON signals(timestamp);
CREATE INDEX IF NOT EXISTS idx_candles_timestamp ON signals(timestamp);
