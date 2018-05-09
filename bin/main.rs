extern crate trade_rs;
use trade_rs::matching_engine::*;

fn main() {
    let mut engine = MatchingEngine::new(100);
    engine.limit(Order {
        price: 100,
        size: 10,
        side: Side::Buy,
        trader: 0,
    });
    engine.limit(Order {
        price: 200,
        size: 7,
        side: Side::Sell,
        trader: 1,
    });
    
    println!("{}", engine);

    engine.limit(Order {
        price: 201,
        size: 3,
        side: Side::Buy,
        trader: 2,
    });

    engine.limit(Order {
        price: 99,
        size: 2,
        side: Side::Sell,
        trader: 2,
    });

    println!("{}", engine);

    engine.limit(Order {
        price: 198,
        size: 1,
        side: Side::Sell,
        trader: 2,
    });

    println!("{}", engine);

    engine.limit(Order {
        price: 199,
        size: 3,
        side: Side::Buy,
        trader: 2,
    });

    println!("{}", engine);
}
