use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Order {
    pub id: String,
    pub user_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub amount: f64,
    pub filled_amount: f64,
    pub remaining_amount: f64,
    pub status: OrderStatus,
    pub created_at: i64,
}

impl Order {
    pub fn new_limit(
        user_id: impl Into<String>,
        symbol: impl Into<String>,
        side: OrderSide,
        price: f64,
        amount: f64,
    ) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.into(),
            symbol: symbol.into(),
            side,
            order_type: OrderType::Limit,
            price: Some(price),
            amount,
            filled_amount: 0.0,
            remaining_amount: amount,
            status: OrderStatus::New,
            created_at: now,
        }
    }

    pub fn new_market(
        user_id: impl Into<String>,
        symbol: impl Into<String>,
        side: OrderSide,
        amount: f64,
    ) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.into(),
            symbol: symbol.into(),
            side,
            order_type: OrderType::Market,
            price: None,
            amount,
            filled_amount: 0.0,
            remaining_amount: amount,
            status: OrderStatus::New,
            created_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trade {
    pub id: String,
    pub symbol: String,
    pub maker_order_id: String,
    pub taker_order_id: String,
    pub price: f64,
    pub amount: f64,
    pub taker_side: OrderSide,
    pub timestamp: i64,
}

impl Trade {
    pub fn new(
        symbol: impl Into<String>,
        maker_order_id: impl Into<String>,
        taker_order_id: impl Into<String>,
        price: f64,
        amount: f64,
        taker_side: OrderSide,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            symbol: symbol.into(),
            maker_order_id: maker_order_id.into(),
            taker_order_id: taker_order_id.into(),
            price,
            amount,
            taker_side,
            timestamp: Utc::now().timestamp_millis(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderBookLevel {
    pub price: f64,
    pub amount: f64,
    pub orders_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderBookSnapshot {
    pub symbol: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candle {
    pub symbol: String,
    pub interval: String,
    pub open_time: i64,
    pub close_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Market {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub current_price: f64,
    pub change_24h: f64,
    pub volume_24h: f64,
    pub high_24h: f64,
    pub low_24h: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_creation() {
        let order = Order::new_limit("user1", "BTC-USD", OrderSide::Buy, 50000.0, 1.5);
        assert_eq!(order.side, OrderSide::Buy);
        assert_eq!(order.price, Some(50000.0));
        assert_eq!(order.amount, 1.5);
        assert_eq!(order.remaining_amount, 1.5);
        assert_eq!(order.status, OrderStatus::New);
    }

    #[test]
    fn test_trade_creation() {
        let trade = Trade::new("BTC-USD", "maker-1", "taker-1", 50000.0, 0.5, OrderSide::Buy);
        assert_eq!(trade.price, 50000.0);
        assert_eq!(trade.amount, 0.5);
        assert_eq!(trade.symbol, "BTC-USD");
    }
}
