use std::collections::{BTreeMap, HashMap, VecDeque};
use shared::{Order, OrderBookLevel, OrderBookSnapshot, OrderSide, OrderStatus, OrderType, Trade};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Price(pub f64);

impl Eq for Price {}

impl Ord for Price {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl std::hash::Hash for Price {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

pub struct OrderBook {
    pub symbol: String,
    // Bids sorted ascending, highest bid is at the end (bids.iter().rev())
    bids: BTreeMap<Price, VecDeque<Order>>,
    // Asks sorted ascending, lowest ask is at the beginning (asks.iter())
    asks: BTreeMap<Price, VecDeque<Order>>,
    // Quick lookup for cancellation: order_id -> (Price, OrderSide)
    order_locations: HashMap<String, (Price, OrderSide)>,
}

impl OrderBook {
    pub fn new(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_locations: HashMap::new(),
        }
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.bids.iter().next_back().map(|(price, _)| price.0)
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks.iter().next().map(|(price, _)| price.0)
    }

    pub fn cancel_order(&mut self, order_id: &str) -> Option<Order> {
        if let Some((price, side)) = self.order_locations.remove(order_id) {
            let levels = match side {
                OrderSide::Buy => &mut self.bids,
                OrderSide::Sell => &mut self.asks,
            };

            if let Some(queue) = levels.get_mut(&price) {
                if let Some(pos) = queue.iter().position(|o| o.id == order_id) {
                    let mut order = queue.remove(pos).unwrap();
                    order.status = OrderStatus::Cancelled;
                    if queue.is_empty() {
                        levels.remove(&price);
                    }
                    return Some(order);
                }
            }
        }
        None
    }

    pub fn match_order(&mut self, mut taker_order: Order) -> (Order, Vec<Trade>) {
        let mut trades = Vec::new();

        match taker_order.side {
            OrderSide::Buy => {
                while taker_order.remaining_amount > 1e-9 {
                    // Check best ask
                    let best_ask_price = match self.asks.keys().next() {
                        Some(&p) => p,
                        None => break,
                    };

                    // For limit order, check if best ask is within limit price
                    if taker_order.order_type == OrderType::Limit {
                        if let Some(limit_price) = taker_order.price {
                            if best_ask_price.0 > limit_price {
                                break;
                            }
                        }
                    }

                    let match_price = best_ask_price.0;
                    let queue = self.asks.get_mut(&best_ask_price).unwrap();
                    let mut maker_order = queue.pop_front().unwrap();

                    let trade_amount = taker_order.remaining_amount.min(maker_order.remaining_amount);

                    maker_order.filled_amount += trade_amount;
                    maker_order.remaining_amount -= trade_amount;
                    taker_order.filled_amount += trade_amount;
                    taker_order.remaining_amount -= trade_amount;

                    let trade = Trade::new(
                        &self.symbol,
                        &maker_order.id,
                        &taker_order.id,
                        match_price,
                        trade_amount,
                        OrderSide::Buy,
                    );
                    trades.push(trade);

                    if maker_order.remaining_amount > 1e-9 {
                        maker_order.status = OrderStatus::PartiallyFilled;
                        queue.push_front(maker_order);
                    } else {
                        maker_order.status = OrderStatus::Filled;
                        self.order_locations.remove(&maker_order.id);
                    }

                    if self.asks.get(&best_ask_price).map_or(false, |q| q.is_empty()) {
                        self.asks.remove(&best_ask_price);
                    }
                }

                // If limit order has remaining amount, insert into bids
                if taker_order.order_type == OrderType::Limit && taker_order.remaining_amount > 1e-9 {
                    if let Some(price) = taker_order.price {
                        taker_order.status = if taker_order.filled_amount > 1e-9 {
                            OrderStatus::PartiallyFilled
                        } else {
                            OrderStatus::New
                        };
                        let p = Price(price);
                        self.order_locations.insert(taker_order.id.clone(), (p, OrderSide::Buy));
                        self.bids.entry(p).or_default().push_back(taker_order.clone());
                    }
                } else if taker_order.remaining_amount <= 1e-9 {
                    taker_order.status = OrderStatus::Filled;
                } else {
                    // Market order with remaining amount and no liquidity
                    taker_order.status = if taker_order.filled_amount > 1e-9 {
                        OrderStatus::PartiallyFilled
                    } else {
                        OrderStatus::Rejected
                    };
                }
            }
            OrderSide::Sell => {
                while taker_order.remaining_amount > 1e-9 {
                    // Check best bid
                    let best_bid_price = match self.bids.keys().next_back() {
                        Some(&p) => p,
                        None => break,
                    };

                    // For limit order, check if best bid meets limit price
                    if taker_order.order_type == OrderType::Limit {
                        if let Some(limit_price) = taker_order.price {
                            if best_bid_price.0 < limit_price {
                                break;
                            }
                        }
                    }

                    let match_price = best_bid_price.0;
                    let queue = self.bids.get_mut(&best_bid_price).unwrap();
                    let mut maker_order = queue.pop_front().unwrap();

                    let trade_amount = taker_order.remaining_amount.min(maker_order.remaining_amount);

                    maker_order.filled_amount += trade_amount;
                    maker_order.remaining_amount -= trade_amount;
                    taker_order.filled_amount += trade_amount;
                    taker_order.remaining_amount -= trade_amount;

                    let trade = Trade::new(
                        &self.symbol,
                        &maker_order.id,
                        &taker_order.id,
                        match_price,
                        trade_amount,
                        OrderSide::Sell,
                    );
                    trades.push(trade);

                    if maker_order.remaining_amount > 1e-9 {
                        maker_order.status = OrderStatus::PartiallyFilled;
                        queue.push_front(maker_order);
                    } else {
                        maker_order.status = OrderStatus::Filled;
                        self.order_locations.remove(&maker_order.id);
                    }

                    if self.bids.get(&best_bid_price).map_or(false, |q| q.is_empty()) {
                        self.bids.remove(&best_bid_price);
                    }
                }

                // If limit order has remaining amount, insert into asks
                if taker_order.order_type == OrderType::Limit && taker_order.remaining_amount > 1e-9 {
                    if let Some(price) = taker_order.price {
                        taker_order.status = if taker_order.filled_amount > 1e-9 {
                            OrderStatus::PartiallyFilled
                        } else {
                            OrderStatus::New
                        };
                        let p = Price(price);
                        self.order_locations.insert(taker_order.id.clone(), (p, OrderSide::Sell));
                        self.asks.entry(p).or_default().push_back(taker_order.clone());
                    }
                } else if taker_order.remaining_amount <= 1e-9 {
                    taker_order.status = OrderStatus::Filled;
                } else {
                    taker_order.status = if taker_order.filled_amount > 1e-9 {
                        OrderStatus::PartiallyFilled
                    } else {
                        OrderStatus::Rejected
                    };
                }
            }
        }

        (taker_order, trades)
    }

    pub fn get_snapshot(&self, depth: usize) -> OrderBookSnapshot {
        let bids = self
            .bids
            .iter()
            .rev()
            .take(depth)
            .map(|(price, queue)| OrderBookLevel {
                price: price.0,
                amount: queue.iter().map(|o| o.remaining_amount).sum(),
                orders_count: queue.len(),
            })
            .collect();

        let asks = self
            .asks
            .iter()
            .take(depth)
            .map(|(price, queue)| OrderBookLevel {
                price: price.0,
                amount: queue.iter().map(|o| o.remaining_amount).sum(),
                orders_count: queue.len(),
            })
            .collect();

        OrderBookSnapshot {
            symbol: self.symbol.clone(),
            bids,
            asks,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limit_order_matching() {
        let mut book = OrderBook::new("BTC-USD");

        // Place sell order (maker): 1 BTC @ 50,000
        let sell_order = Order::new_limit("maker", "BTC-USD", OrderSide::Sell, 50000.0, 1.0);
        let (processed_sell, trades) = book.match_order(sell_order);
        assert_eq!(trades.len(), 0);
        assert_eq!(processed_sell.status, OrderStatus::New);
        assert_eq!(book.best_ask(), Some(50000.0));

        // Place buy order (taker): 0.4 BTC @ 50,000
        let buy_order = Order::new_limit("taker", "BTC-USD", OrderSide::Buy, 50000.0, 0.4);
        let (processed_buy, trades) = book.match_order(buy_order);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].amount, 0.4);
        assert_eq!(trades[0].price, 50000.0);
        assert_eq!(processed_buy.status, OrderStatus::Filled);
        assert_eq!(processed_buy.remaining_amount, 0.0);

        // Check maker order remaining in book (0.6 BTC)
        let snapshot = book.get_snapshot(5);
        assert_eq!(snapshot.asks.len(), 1);
        assert_eq!(snapshot.asks[0].amount, 0.6);
    }

    #[test]
    fn test_order_cancellation() {
        let mut book = OrderBook::new("BTC-USD");
        let order = Order::new_limit("user1", "BTC-USD", OrderSide::Buy, 49000.0, 2.0);
        let order_id = order.id.clone();
        book.match_order(order);

        assert_eq!(book.best_bid(), Some(49000.0));
        let cancelled = book.cancel_order(&order_id);
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().status, OrderStatus::Cancelled);
        assert_eq!(book.best_bid(), None);
    }
}
