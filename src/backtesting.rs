use crate::{
    data::{Candles, Position, PositionSide, Side},
    signal::MarketSignal,
};
use rust_decimal::{
    prelude::{FromPrimitive, ToPrimitive},
    Decimal,
};

pub struct BackTesting {
    pub analyzer: MarketSignal,
    pub init_amount: Decimal,
    pub positions: Vec<Position>,
}

pub struct BacktestResult {
    pub init_balance: Decimal,
    pub final_balance: Decimal,
    pub total_pnl: Decimal,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: f64,
    pub return_pct: f64,
}

impl BackTesting {
    pub fn new(init_amount: Decimal) -> Self {
        Self {
            analyzer: MarketSignal::new(),
            init_amount,
            positions: Vec::new(),
        }
    }

    pub fn run(&mut self, historical_data: Vec<Candles>, symbol: String) -> BacktestResult {
        let mut balance = self.init_amount;
        let mut total_pnl = Decimal::ZERO;
        let mut total_trades = 0;
        let mut winning_trades = 0;

        for candle in historical_data {
            self.analyzer.add_candles(candle.clone());

            let mut closed_positions = Vec::new();

            for (i, position) in self.positions.iter().enumerate() {
                if candle.low <= position.stop_loss {
                    let pnl = (position.stop_loss - position.entry_price) * position.size;
                    total_pnl += pnl;
                    balance += position.stop_loss * position.size;
                    total_trades += 1;

                    if pnl > Decimal::ZERO {
                        winning_trades += 1;
                    }

                    closed_positions.push(i);
                } else if candle.high >= position.take_profit {
                    let pnl = (position.take_profit - position.entry_price) * position.size;
                    total_pnl += pnl;
                    balance += position.take_profit * position.size;
                    total_trades += 1;

                    if pnl > Decimal::ZERO {
                        winning_trades += 1;
                    }

                    closed_positions.push(i);
                }
            }

            for i in closed_positions.iter().rev() {
                self.positions.remove(*i);
            }

            if let Some(signal) = self.analyzer.analyze(symbol.clone()) {
                let decimal = Decimal::from_f64(0.7).unwrap();

                if signal.confidence > decimal && signal.action == Side::Buy {
                    let stop_loss = signal.price * Decimal::new(98, 2);
                    let take_profit = signal.price * Decimal::new(104, 2);
                    let risk_amount = balance * Decimal::new(2, 2);
                    let risk_per_unit = signal.price - stop_loss;

                    if risk_per_unit > Decimal::ZERO {
                        let quantity = risk_amount / risk_per_unit;
                        let cost = signal.price * quantity;

                        if cost <= balance {
                            balance -= cost;
                            self.positions.push(Position {
                                id: signal.id,
                                symbol: symbol.clone(),
                                position_side: PositionSide::Long,
                                entry_price: signal.price,
                                size: quantity,
                                stop_loss,
                                take_profit,
                                opened_at: candle.timestamp,
                            });
                        }
                    }
                }
            }
        }

        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        let return_pct = ((balance - self.init_amount) / self.init_amount * Decimal::new(100, 0))
            .to_f64()
            .unwrap_or(0.0);

        BacktestResult {
            init_balance: self.init_amount,
            final_balance: balance,
            total_pnl,
            total_trades,
            winning_trades,
            losing_trades: total_trades - winning_trades,
            win_rate,
            return_pct,
        }
    }
}

impl BacktestResult {
    pub fn print_summary(&self) {
        println!("\n======== BACKTEST RESULTS ============");
        println!("Initial Balance:    ${}", self.init_balance);
        println!("Final Balance:      ${}", self.final_balance);
        println!("Total PnL:          ${}", self.total_pnl);
        println!("Total Trades:       {}", self.total_trades);
        println!("Winning Trades:     {}", self.winning_trades);
        println!("Losing Trades:      {}", self.losing_trades);
        println!("Win Rate:           {:.2}%", self.win_rate);
        println!("Return:             {:.2}%", self.return_pct);
        println!("======================================\n");
    }
}
