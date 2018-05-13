#![cfg(test)]

use matching_engine::*;

fn new_order(price: Price, size: u64, side: Side) -> Order {
    Order {
        price,
        size,
        side,
        owner: 0,
    }
}

#[test]
fn matching_engine() {
    let mut m = MatchingEngine::new(100);

    m.limit(new_order(100, 10, Side::Buy));
    m.limit(new_order(200, 5, Side::Sell));

    assert_eq!(m.best_limits(), (100, 200));
    assert_eq!(m.size_at_price(100), 10);
    assert_eq!(m.size_at_price(200), 5);
}

#[test]
fn marketable_order() {
    let mut m = MatchingEngine::new(100);

    m.limit(new_order(100, 10, Side::Buy));
    m.limit(new_order(200, 5, Side::Sell));
    m.limit(new_order(200, 3, Side::Buy));

    m.limit(new_order(100, 2, Side::Sell));

    assert_eq!(m.best_limits(), (100, 200));
    assert_eq!(m.size_at_price(100), 8);
    assert_eq!(m.size_at_price(200), 2);
}

#[test]
fn marketable_order_cross_multiple_limits() {
    let mut m = MatchingEngine::new(100);

    m.limit(new_order(96, 4, Side::Buy));
    m.limit(new_order(99, 4, Side::Buy));
    m.limit(new_order(100, 10, Side::Buy));
    m.limit(new_order(200, 5, Side::Sell));
    m.limit(new_order(202, 5, Side::Sell));
    m.limit(new_order(203, 5, Side::Sell));

    m.limit(new_order(99, 3, Side::Sell));
    
    assert_eq!(m.best_limits(), (100, 200));
    assert_eq!(m.size_at_price(100), 7);

    m.limit(new_order(97, 9, Side::Sell));

    assert_eq!(m.best_limits(), (99, 200));
    assert_eq!(m.size_at_price(99), 2);

    m.limit(new_order(203, 12, Side::Buy));
    
    assert_eq!(m.best_limits(), (99, 203));
    assert_eq!(m.size_at_price(203), 3);
}

#[test]
fn insert_within_bid_ask_spread() {
    let mut m = MatchingEngine::new(100);

    m.limit(new_order(100, 10, Side::Buy));
    m.limit(new_order(200, 5, Side::Sell));

    m.limit(new_order(101, 3, Side::Buy));
    m.limit(new_order(200, 2, Side::Sell));

    assert_eq!(m.best_limits(), (101, 200));
    assert_eq!(m.size_at_price(100), 10);
    assert_eq!(m.size_at_price(101), 3);
    assert_eq!(m.size_at_price(200), 7);

    m.limit(new_order(156, 1, Side::Sell));
    assert_eq!(m.best_limits(), (101, 156));
}

#[test]
fn consume_and_insert() {
    let mut m = MatchingEngine::new(100);

    m.limit(new_order(100, 10, Side::Buy));
    m.limit(new_order(200, 5, Side::Sell));
    m.limit(new_order(201, 5, Side::Sell));
    m.limit(new_order(203, 5, Side::Sell));

    m.limit(new_order(202, 12, Side::Buy));

    assert_eq!(m.best_limits(), (202, 203));
    assert_eq!(m.size_at_price(202), 2);

    m.limit(new_order(150, 3, Side::Sell));

    assert_eq!(m.best_limits(), (100, 150));
    assert_eq!(m.size_at_price(150), 1);
}

#[test]
fn consume_all_liquidity() {
    let mut m = MatchingEngine::new(100);

    m.limit(new_order(100, 10, Side::Buy));
    m.limit(new_order(200, 5, Side::Sell));
    m.limit(new_order(201, 5, Side::Sell));
    m.limit(new_order(203, 5, Side::Sell));

    m.limit(new_order(203, 15, Side::Buy));

    assert_eq!(m.best_limits(), (100, Price::max_value()));
    assert_eq!(m.size_at_price(203), 0);
    assert_eq!(m.size_at_price(Price::max_value()), 0);

    m.limit(new_order(150, 10, Side::Sell));

    assert_eq!(m.best_limits(), (100, 150));
}
