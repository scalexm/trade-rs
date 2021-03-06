#![cfg(test)]

use crate::Side;
use crate::tick::TickUnit;
use crate::order_book::{OrderBook, LimitUpdate};

fn lu(price: TickUnit, size: TickUnit, side: Side) -> LimitUpdate {
    LimitUpdate::new(price, size, side)
}

#[test]
fn test_diff() {
    let mut odb1 = OrderBook::new();
    odb1.update(lu(100, 10, Side::Ask));
    odb1.update(lu(90, 6, Side::Ask));
    odb1.update(lu(80, 8, Side::Bid));
    odb1.update(lu(77, 9, Side::Bid));

    let mut odb2 = OrderBook::new();
    odb2.update(lu(100, 10, Side::Ask));
    odb2.update(lu(91, 6, Side::Ask));
    odb2.update(lu(90, 3, Side::Ask));
    odb2.update(lu(78, 5, Side::Bid));
    odb2.update(lu(77, 4, Side::Bid));

    let mut diff: Vec<_> = odb1.diff(&odb2).collect();
    diff.sort_by(|x, y| x.price.cmp(&y.price));

    assert_eq!(
        diff,
        vec![
            lu(77, 4, Side::Bid),
            lu(78, 5, Side::Bid),
            lu(80, 0, Side::Bid),
            lu(90, 3, Side::Ask),
            lu(91, 6, Side::Ask),
        ]
    );

    for u in diff {
        odb1.update(u);
    }

    assert_eq!(odb1, odb2);

    let mut odb1 = OrderBook::new();
    for u in odb1.diff(&odb2) {
        odb1.update(u);
    }
    assert_eq!(odb1, odb2);
}
