#[macro_use] extern crate criterion;
extern crate trade;

use criterion::Criterion;
use trade::Tick;

fn criterion_benchmark(c: &mut Criterion) {
    let tick = Tick::new(1000);

    c.bench_function(
        "unticked",
        move |b| b.iter(|| tick.convert_unticked("1278.853").unwrap())
    );

    c.bench_function(
        "tick",
        move |b| b.iter(|| tick.convert_ticked(1278853).unwrap())
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
