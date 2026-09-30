use order_matching_engine::{server::EngineServer, MatchingEngine};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting Order Matching Engine...");
    let engine = Arc::new(MatchingEngine::new());
    let server = EngineServer::new(engine, "127.0.0.1:8081");

    server.run().await?;
    Ok(())
}
