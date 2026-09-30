use dashmap::DashMap;
use shared::{Order, OrderBookSnapshot, Trade};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::broadcast;

use crate::order_book::OrderBook;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Order not found: {0}")]
    OrderNotFound(String),
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("Internal lock error")]
    LockError,
}

#[derive(Debug, Clone)]
pub struct EngineEvent {
    pub symbol: String,
    pub order: Order,
    pub trades: Vec<Trade>,
    pub snapshot: OrderBookSnapshot,
}

pub struct MatchingEngine {
    // Symbol -> Mutex<OrderBook>
    books: DashMap<String, Arc<Mutex<OrderBook>>>,
    event_sender: broadcast::Sender<EngineEvent>,
}

impl MatchingEngine {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(10000);
        let engine = Self {
            books: DashMap::new(),
            event_sender: tx,
        };

        // Initialize default markets
        engine.init_market("BTC-USD");
        engine.init_market("ETH-USD");
        engine.init_market("BTC-DEC-PRED");

        engine
    }

    pub fn init_market(&self, symbol: &str) {
        if !self.books.contains_key(symbol) {
            self.books.insert(
                symbol.to_string(),
                Arc::new(Mutex::new(OrderBook::new(symbol))),
            );
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<EngineEvent> {
        self.event_sender.subscribe()
    }

    pub fn place_order(&self, order: Order) -> Result<(Order, Vec<Trade>), EngineError> {
        let symbol = order.symbol.clone();
        self.init_market(&symbol);

        let book_ref = self.books.get(&symbol).ok_or_else(|| EngineError::SymbolNotFound(symbol.clone()))?;
        let mut book = book_ref.lock().map_err(|_| EngineError::LockError)?;

        let (processed_order, trades) = book.match_order(order);
        let snapshot = book.get_snapshot(20);

        // Broadcast event
        let _ = self.event_sender.send(EngineEvent {
            symbol,
            order: processed_order.clone(),
            trades: trades.clone(),
            snapshot,
        });

        Ok((processed_order, trades))
    }

    pub fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<Order, EngineError> {
        let book_ref = self.books.get(symbol).ok_or_else(|| EngineError::SymbolNotFound(symbol.to_string()))?;
        let mut book = book_ref.lock().map_err(|_| EngineError::LockError)?;

        let cancelled = book
            .cancel_order(order_id)
            .ok_or_else(|| EngineError::OrderNotFound(order_id.to_string()))?;

        let snapshot = book.get_snapshot(20);
        let _ = self.event_sender.send(EngineEvent {
            symbol: symbol.to_string(),
            order: cancelled.clone(),
            trades: vec![],
            snapshot,
        });

        Ok(cancelled)
    }

    pub fn get_snapshot(&self, symbol: &str, depth: usize) -> Result<OrderBookSnapshot, EngineError> {
        let book_ref = self.books.get(symbol).ok_or_else(|| EngineError::SymbolNotFound(symbol.to_string()))?;
        let book = book_ref.lock().map_err(|_| EngineError::LockError)?;
        Ok(book.get_snapshot(depth))
    }
}
