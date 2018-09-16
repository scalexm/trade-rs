#[macro_use] extern crate criterion;
extern crate trade;

use criterion::Criterion;
use trade::{TickUnit, Side, OrderBook};
use trade::order_book::LimitUpdate;

fn lu(price: TickUnit, size: TickUnit, side: Side) -> LimitUpdate {
    LimitUpdate::new(price, size, side)
}

fn criterion_benchmark(c: &mut Criterion) {
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

    c.bench_function(
        "diff",
        move |b| b.iter(|| odb1.diff(&odb2))
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
