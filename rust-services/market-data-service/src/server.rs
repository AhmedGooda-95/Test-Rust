use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use crate::aggregator::MarketDataAggregator;
use crate::broadcaster::{MarketBroadcaster, MarketUpdate};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClientAction {
    Subscribe { symbol: Option<String> },
    GetCandles { symbol: String, interval: String, limit: Option<usize> },
    GetIndicator { symbol: String, interval: String, indicator: String, period: Option<usize> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerResponse {
    Subscribed { symbol: Option<String> },
    Candles { symbol: String, interval: String, candles: Vec<shared::Candle> },
    Indicator { symbol: String, indicator: String, value: Option<f64> },
    Update { data: MarketUpdate },
    Error { message: String },
}

pub struct MarketDataServer {
    aggregator: Arc<MarketDataAggregator>,
    broadcaster: Arc<MarketBroadcaster>,
    bind_addr: String,
}

impl MarketDataServer {
    pub fn new(
        aggregator: Arc<MarketDataAggregator>,
        broadcaster: Arc<MarketBroadcaster>,
        bind_addr: impl Into<String>,
    ) -> Self {
        Self {
            aggregator,
            broadcaster,
            bind_addr: bind_addr.into(),
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!("Market Data WebSocket server listening on {}", self.bind_addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let aggregator = self.aggregator.clone();
            let broadcaster = self.broadcaster.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_client(stream, aggregator, broadcaster).await {
                    warn!("Client {} disconnected: {}", addr, e);
                }
            });
        }

        Ok(())
    }
}

async fn handle_client(
    stream: TcpStream,
    aggregator: Arc<MarketDataAggregator>,
    broadcaster: Arc<MarketBroadcaster>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut broadcast_rx = broadcaster.subscribe();

    let subscribed_symbol: Arc<tokio::sync::RwLock<Option<String>>> = Arc::new(tokio::sync::RwLock::new(None));
    let sub_sym_clone = subscribed_symbol.clone();
    let mut is_subscribed = false;

    // Outgoing channel for client
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(256);

    // Forward broadcast updates task
    tokio::spawn(async move {
        while let Ok(update) = broadcast_rx.recv().await {
            let filter = sub_sym_clone.read().await;
            let matches = match filter.as_ref() {
                None => true,
                Some(sub_sym) => match &update {
                    MarketUpdate::Trade { trade } => &trade.symbol == sub_sym,
                    MarketUpdate::Candle { candle } => &candle.symbol == sub_sym,
                    MarketUpdate::OrderBook { snapshot } => &snapshot.symbol == sub_sym,
                },
            };

            if matches {
                let resp = ServerResponse::Update { data: update };
                if let Ok(json) = serde_json::to_string(&resp) {
                    if out_tx.send(json).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientAction>(&text) {
                            Ok(ClientAction::Subscribe { symbol }) => {
                                is_subscribed = true;
                                *subscribed_symbol.write().await = symbol.clone();
                                let resp = ServerResponse::Subscribed { symbol };
                                ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                            }
                            Ok(ClientAction::GetCandles { symbol, interval, limit }) => {
                                let l = limit.unwrap_or(50);
                                let candles = aggregator.get_candles(&symbol, &interval, l);
                                let resp = ServerResponse::Candles { symbol, interval, candles };
                                ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                            }
                            Ok(ClientAction::GetIndicator { symbol, interval, indicator, period }) => {
                                let p = period.unwrap_or(14);
                                let value = match indicator.to_uppercase().as_str() {
                                    "SMA" => aggregator.calculate_sma(&symbol, &interval, p),
                                    "RSI" => aggregator.calculate_rsi(&symbol, &interval, p),
                                    _ => None,
                                };
                                let resp = ServerResponse::Indicator { symbol, indicator, value };
                                ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                            }
                            Err(e) => {
                                let resp = ServerResponse::Error { message: format!("Invalid message: {}", e) };
                                ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {},
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }
            Some(outgoing) = out_rx.recv(), if is_subscribed => {
                if ws_sender.send(Message::Text(outgoing)).await.is_err() {
                    break;
                }
            }
        }
    }

    Ok(())
}
