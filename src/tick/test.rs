#![cfg(test)]

use crate::tick::Tick;

#[test]
fn convert_unticked() {
    assert_eq!(
        Ok(1),
        Tick::new(2).ticked("0.5")
    );

    assert_eq!(
        Ok(5),
        Tick::new(10).ticked("0.5")
    );

    assert_eq!(
        Ok(5),
        Tick::new(4).ticked("1.25")
    );

    assert_eq!(
        Ok(4),
        Tick::new(2000).ticked("0.002")
    );

    assert_eq!(
        Ok(35),
        Tick::new(10).ticked("3.5")
    );

    assert_eq!(
        Ok(127),
        Tick::new(20).ticked("6.35")
    );
}

#[test]
fn trailing_zeros() {
    assert_eq!(
        Ok(127),
        Tick::new(20).ticked("6.3500000"),
    );

    assert_eq!(
        Ok(2),
        Tick::new(2).ticked("1.0000"),
    );

    assert_eq!(
        Ok(4),
        Tick::new(2000).ticked("0.0020")
    );
}

#[test]
fn empty_fract_part() {
    assert_eq!(
        Ok(2),
        Tick::new(2).ticked("1")
    );

    assert_eq!(
        Ok(2),
        Tick::new(2).ticked("1.")
    );

    assert_eq!(
        Ok(0),
        Tick::new(321).ticked("0.")
    );
}

#[test]
fn empty_int_part() {
    assert_eq!(
        Ok(5),
        Tick::new(10).ticked(".5")
    );
}

#[test]
fn bad_int_part() {
    assert!(
        Tick::new(10).ticked("abc").is_err()
    );

    assert!(
        Tick::new(10).ticked("abc.5").is_err()
    );
}

#[test]
fn bad_fract_part() {
    assert!(
        Tick::new(10).ticked("5.abc").is_err()
    );
}

#[test]
fn do_not_overflow_ticked() {
    assert_eq!(
        Ok(10_000_000_000_000_000_001),
        Tick::new(100_000_000).ticked("100000000000.00000001")
    )
}

#[test]
#[should_panic]
fn overflow_ticked() {
    let _ = Tick::new(100_000_000).ticked("1000000000000.00000001");
}

#[test]
fn truncate() {
    assert_eq!(
        Ok(51),
        Tick::new(10).ticked("5.11")
    );

    assert_eq!(
        Ok(3),
        Tick::new(4).ticked("0.76")
    );
}

#[test]
fn convert_ticked() {
    assert_eq!(
        Ok("1.15".to_owned()),
        Tick::new(100).unticked(Tick::new(100).ticked("1.15").unwrap())
    );

    assert_eq!(
        Ok("1.01".to_owned()),
        Tick::new(100).unticked(Tick::new(100).ticked("1.01").unwrap())
    );

    assert_eq!(
        Ok("0.15".to_owned()),
        Tick::new(20).unticked(Tick::new(20).ticked("0.15").unwrap())
    );

    assert_eq!(
        Ok("75.5".to_owned()),
        Tick::new(2).unticked(Tick::new(2).ticked("75.5").unwrap())
    );

    assert_eq!(
        Ok("1.00".to_owned()),
        Tick::new(20).unticked(Tick::new(20).ticked("1").unwrap())
    );

    assert_eq!(
        Ok("0.0".to_owned()),
        Tick::new(10).unticked(Tick::new(10).ticked("0").unwrap()),
    );

    assert!(
        Tick::new(23).unticked(Tick::new(10).ticked("75.4").unwrap()).is_err()
    );
}

#[test]
fn do_not_overflow_unticked() {
    let value = Tick::new(100_000_000).ticked("100000000000.00000001").unwrap();
    assert_eq!(
        Ok("100000000000.00000001".to_owned()),
        Tick::new(100_000_000).unticked(value)
    );
}

#[test]
fn tick_size() {
    assert_eq!(
        Some(Tick::new(1)),
        Tick::tick_size("1")
    );

    assert_eq!(
        Some(Tick::new(1)),
        Tick::tick_size("1.")
    );

    assert_eq!(
        Some(Tick::new(1)),
        Tick::tick_size("1.00")
    );

    assert_eq!(
        Some(Tick::new(100)),
        Tick::tick_size("0.01000")
    );

    assert_eq!(
        Some(Tick::new(100_000_000)),
        Tick::tick_size(".00000001")
    );

    assert_eq!(
        Some(Tick::new(2)),
        Tick::tick_size(".5")
    );

    assert_eq!(
        Some(Tick::new(400)),
        Tick::tick_size("0.0025")
    );
}
