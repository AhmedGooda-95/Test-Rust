use dashmap::DashMap;
use shared::{Candle, Trade};
use std::sync::RwLock;

pub struct MarketDataAggregator {
    // Key: (symbol, interval) -> List of finalized candles
    candles: DashMap<(String, String), RwLock<Vec<Candle>>>,
    // Key: (symbol, interval) -> Current active candle
    active_candles: DashMap<(String, String), RwLock<Option<Candle>>>,
}

impl MarketDataAggregator {
    pub fn new() -> Self {
        Self {
            candles: DashMap::new(),
            active_candles: DashMap::new(),
        }
    }

    fn interval_duration_ms(interval: &str) -> i64 {
        match interval {
            "1m" => 60 * 1000,
            "5m" => 5 * 60 * 1000,
            "15m" => 15 * 60 * 1000,
            "1h" => 60 * 60 * 1000,
            "1d" => 24 * 60 * 60 * 1000,
            _ => 60 * 1000,
        }
    }

    pub fn process_trade(&self, trade: &Trade) -> Vec<Candle> {
        let intervals = ["1m", "5m", "1h", "1d"];
        let mut completed_candles = Vec::new();

        for &interval in &intervals {
            let key = (trade.symbol.clone(), interval.to_string());
            let duration = Self::interval_duration_ms(interval);
            let candle_open_time = (trade.timestamp / duration) * duration;
            let candle_close_time = candle_open_time + duration;

            let active_lock = self
                .active_candles
                .entry(key.clone())
                .or_insert_with(|| RwLock::new(None));

            let mut active = active_lock.write().unwrap();

            if let Some(ref mut candle) = *active {
                if trade.timestamp < candle.close_time {
                    // Update current candle
                    candle.high = candle.high.max(trade.price);
                    candle.low = candle.low.min(trade.price);
                    candle.close = trade.price;
                    candle.volume += trade.amount;
                    continue;
                } else {
                    // Finalize current candle
                    let finished = candle.clone();
                    completed_candles.push(finished.clone());

                    let history_lock = self
                        .candles
                        .entry(key.clone())
                        .or_insert_with(|| RwLock::new(Vec::new()));
                    history_lock.write().unwrap().push(finished);
                }
            }

            // Start a new candle
            *active = Some(Candle {
                symbol: trade.symbol.clone(),
                interval: interval.to_string(),
                open_time: candle_open_time,
                close_time: candle_close_time,
                open: trade.price,
                high: trade.price,
                low: trade.price,
                close: trade.price,
                volume: trade.amount,
            });
        }

        completed_candles
    }

    pub fn get_candles(&self, symbol: &str, interval: &str, limit: usize) -> Vec<Candle> {
        let key = (symbol.to_string(), interval.to_string());
        let mut result = Vec::new();

        if let Some(history_lock) = self.candles.get(&key) {
            let history = history_lock.read().unwrap();
            let start = history.len().saturating_sub(limit);
            result.extend(history[start..].iter().cloned());
        }

        if let Some(active_lock) = self.active_candles.get(&key) {
            if let Some(ref current) = *active_lock.read().unwrap() {
                result.push(current.clone());
            }
        }

        result
    }

    /// Calculate Simple Moving Average (SMA)
    pub fn calculate_sma(&self, symbol: &str, interval: &str, period: usize) -> Option<f64> {
        let candles = self.get_candles(symbol, interval, period);
        if candles.len() < period || period == 0 {
            return None;
        }
        let sum: f64 = candles.iter().map(|c| c.close).sum();
        Some(sum / period as f64)
    }

    /// Calculate Relative Strength Index (RSI)
    pub fn calculate_rsi(&self, symbol: &str, interval: &str, period: usize) -> Option<f64> {
        let candles = self.get_candles(symbol, interval, period + 1);
        if candles.len() < period + 1 || period == 0 {
            return None;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for window in candles.windows(2) {
            let diff = window[1].close - window[0].close;
            if diff > 0.0 {
                gains += diff;
            } else {
                losses += diff.abs();
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::OrderSide;

    #[test]
    fn test_candle_aggregation() {
        let aggregator = MarketDataAggregator::new();
        let trade1 = Trade::new("BTC-USD", "m1", "t1", 50000.0, 1.0, OrderSide::Buy);
        let mut trade2 = Trade::new("BTC-USD", "m2", "t2", 51000.0, 0.5, OrderSide::Buy);
        trade2.timestamp = trade1.timestamp + 500; // Same minute

        aggregator.process_trade(&trade1);
        aggregator.process_trade(&trade2);

        let candles = aggregator.get_candles("BTC-USD", "1m", 10);
        assert_eq!(candles.len(), 1);
        let candle = &candles[0];
        assert_eq!(candle.open, 50000.0);
        assert_eq!(candle.high, 51000.0);
        assert_eq!(candle.low, 50000.0);
        assert_eq!(candle.close, 51000.0);
        assert_eq!(candle.volume, 1.5);
    }
}
