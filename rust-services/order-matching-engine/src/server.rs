use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use shared::{Order, OrderBookSnapshot, Trade};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

use crate::engine::MatchingEngine;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RequestMessage {
    PlaceOrder { order: Order },
    CancelOrder { symbol: String, order_id: String },
    GetSnapshot { symbol: String, depth: Option<usize> },
    Subscribe { symbol: Option<String> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseMessage {
    OrderPlaced { order: Order, trades: Vec<Trade> },
    OrderCancelled { order: Order },
    Snapshot { snapshot: OrderBookSnapshot },
    Event { symbol: String, order: Order, trades: Vec<Trade>, snapshot: OrderBookSnapshot },
    Error { message: String },
}

pub struct EngineServer {
    engine: Arc<MatchingEngine>,
    bind_addr: String,
}

impl EngineServer {
    pub fn new(engine: Arc<MatchingEngine>, bind_addr: impl Into<String>) -> Self {
        Self {
            engine,
            bind_addr: bind_addr.into(),
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!("Order Matching Engine WebSocket server listening on {}", self.bind_addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let engine = self.engine.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, engine).await {
                    warn!("Connection error with {}: {}", addr, e);
                }
            });
        }

        Ok(())
    }
}

async fn handle_connection(
    stream: TcpStream,
    engine: Arc<MatchingEngine>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut rx = engine.subscribe_events();

    // Spawn a background task to forward engine events if subscribed
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<String>(100);
    let subscribed_symbol: Arc<tokio::sync::RwLock<Option<String>>> = Arc::new(tokio::sync::RwLock::new(None));
    let sub_sym_clone = subscribed_symbol.clone();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let filter = sub_sym_clone.read().await;
            let matches = match filter.as_ref() {
                Some(sym) => sym == &event.symbol,
                None => true,
            };
            if matches {
                let resp = ResponseMessage::Event {
                    symbol: event.symbol,
                    order: event.order,
                    trades: event.trades,
                    snapshot: event.snapshot,
                };
                if let Ok(json) = serde_json::to_string(&resp) {
                    if event_tx.send(json).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut is_subscribed = false;

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<RequestMessage>(&text) {
                            Ok(RequestMessage::PlaceOrder { order }) => {
                                match engine.place_order(order) {
                                    Ok((order, trades)) => {
                                        let resp = ResponseMessage::OrderPlaced { order, trades };
                                        let json = serde_json::to_string(&resp)?;
                                        ws_sender.send(Message::Text(json)).await?;
                                    }
                                    Err(e) => {
                                        let resp = ResponseMessage::Error { message: e.to_string() };
                                        ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                                    }
                                }
                            }
                            Ok(RequestMessage::CancelOrder { symbol, order_id }) => {
                                match engine.cancel_order(&symbol, &order_id) {
                                    Ok(order) => {
                                        let resp = ResponseMessage::OrderCancelled { order };
                                        ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                                    }
                                    Err(e) => {
                                        let resp = ResponseMessage::Error { message: e.to_string() };
                                        ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                                    }
                                }
                            }
                            Ok(RequestMessage::GetSnapshot { symbol, depth }) => {
                                let d = depth.unwrap_or(20);
                                match engine.get_snapshot(&symbol, d) {
                                    Ok(snapshot) => {
                                        let resp = ResponseMessage::Snapshot { snapshot };
                                        ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                                    }
                                    Err(e) => {
                                        let resp = ResponseMessage::Error { message: e.to_string() };
                                        ws_sender.send(Message::Text(serde_json::to_string(&resp)?)).await?;
                                    }
                                }
                            }
                            Ok(RequestMessage::Subscribe { symbol }) => {
                                is_subscribed = true;
                                *subscribed_symbol.write().await = symbol;
                            }
                            Err(e) => {
                                let resp = ResponseMessage::Error { message: format!("Invalid request payload: {}", e) };
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
            Some(event_json) = event_rx.recv(), if is_subscribed => {
                if ws_sender.send(Message::Text(event_json)).await.is_err() {
                    break;
                }
            }
        }
    }

    Ok(())
}
