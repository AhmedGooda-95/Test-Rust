use market_data_service::{MarketBroadcaster, MarketDataAggregator, MarketUpdate};
use shared::{OrderSide, Trade};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_market_data_concurrent_subscribers() {
    let aggregator = Arc::new(MarketDataAggregator::new());
    let broadcaster = Arc::new(MarketBroadcaster::new(5000));

    let client_count = 100;
    let mut handles = Vec::new();

    // Spawn 100 concurrent subscribers
    for i in 0..client_count {
        let mut rx = broadcaster.subscribe();
        let handle = tokio::spawn(async move {
            let mut received = 0;
            while let Ok(update) = rx.recv().await {
                if let MarketUpdate::Trade { trade } = update {
                    if trade.symbol == "BTC-USD" {
                        received += 1;
                        if received == 10 {
                            break;
                        }
                    }
                }
            }
            received
        });
        handles.push(handle);
    }

    // Give subscribers time to attach
    sleep(Duration::from_millis(50)).await;

    // Broadcast 10 trades
    for i in 0..10 {
        let trade = Trade::new("BTC-USD", format!("maker_{}", i), format!("taker_{}", i), 50000.0 + i as f64, 1.0, OrderSide::Buy);
        aggregator.process_trade(&trade);
        broadcaster.broadcast_trade(trade);
    }

    for handle in handles {
        let count = handle.await.unwrap();
        assert_eq!(count, 10);
    }

    // Verify candle was created
    let candles = aggregator.get_candles("BTC-USD", "1m", 10);
    assert_eq!(candles.len(), 1);
    assert_eq!(candles[0].volume, 10.0);
}
