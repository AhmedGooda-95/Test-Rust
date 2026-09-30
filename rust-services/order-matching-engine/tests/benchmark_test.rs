use order_matching_engine::OrderBook;
use shared::{Order, OrderSide};
use std::time::Instant;

#[test]
fn test_order_matching_engine_throughput_100k() {
    let mut book = OrderBook::new("BTC-USD");

    // Pre-populate some liquidity with 10,000 limit orders
    for i in 0..10_000 {
        let price = 50000.0 + (i as f64 * 0.1);
        let order = Order::new_limit(
            format!("maker_{}", i),
            "BTC-USD",
            OrderSide::Sell,
            price,
            1.0,
        );
        book.match_order(order);
    }

    // Benchmark 100,000 orders
    let total_orders = 100_000;
    println!("Benchmarking matching of {} orders...", total_orders);
    let start = Instant::now();

    for i in 0..total_orders {
        // Alternating buy and sell orders
        let side = if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell };
        let price = 50000.0 + ((i % 100) as f64 * 0.1);
        let order = Order::new_limit(
            format!("user_{}", i),
            "BTC-USD",
            side,
            price,
            0.5,
        );
        book.match_order(order);
    }

    let elapsed = start.elapsed();
    let seconds = elapsed.as_secs_f64();
    let orders_per_sec = total_orders as f64 / seconds;

    println!("=====================================================");
    println!("PROCESSED {} ORDERS IN {:.4} SECONDS", total_orders, seconds);
    println!("THROUGHPUT: {:.2} ORDERS / SECOND", orders_per_sec);
    println!("=====================================================");

    assert!(
        orders_per_sec >= 100_000.0,
        "Expected at least 100,000 orders/sec, but achieved {:.2}",
        orders_per_sec
    );
}
