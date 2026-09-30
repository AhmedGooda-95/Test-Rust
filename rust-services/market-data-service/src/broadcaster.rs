use serde::{Deserialize, Serialize};
use shared::{Candle, OrderBookSnapshot, Trade};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MarketUpdate {
    Trade { trade: Trade },
    Candle { candle: Candle },
    OrderBook { snapshot: OrderBookSnapshot },
}

pub struct MarketBroadcaster {
    sender: broadcast::Sender<MarketUpdate>,
}

impl MarketBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { sender: tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MarketUpdate> {
        self.sender.subscribe()
    }

    pub fn broadcast_trade(&self, trade: Trade) {
        let _ = self.sender.send(MarketUpdate::Trade { trade });
    }

    pub fn broadcast_candle(&self, candle: Candle) {
        let _ = self.sender.send(MarketUpdate::Candle { candle });
    }

    pub fn broadcast_orderbook(&self, snapshot: OrderBookSnapshot) {
        let _ = self.sender.send(MarketUpdate::OrderBook { snapshot });
    }
}
