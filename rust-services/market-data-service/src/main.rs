use market_data_service::{MarketBroadcaster, MarketDataAggregator, MarketDataServer};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting Market Data Service...");
    let aggregator = Arc::new(MarketDataAggregator::new());
    let broadcaster = Arc::new(MarketBroadcaster::new(10000));

    let server = MarketDataServer::new(aggregator, broadcaster, "127.0.0.1:8082");
    server.run().await?;

    Ok(())
}
