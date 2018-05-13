#![cfg(test)]

use tick::*;

#[test]
fn convert_unticked() {
    assert_eq!(
        Ok(1),
        Tick::new(2).convert_unticked("0.5")
    );

    assert_eq!(
        Ok(5),
        Tick::new(10).convert_unticked("0.5")
    );

    assert_eq!(
        Ok(4),
        Tick::new(2000).convert_unticked("0.002")
    );

    assert_eq!(
        Ok(35),
        Tick::new(10).convert_unticked("3.5")
    );

    assert_eq!(
        Ok(127),
        Tick::new(20).convert_unticked("6.35")
    );
}

#[test]
fn trailing_zeros() {
    assert_eq!(
        Ok(127),
        Tick::new(20).convert_unticked("6.3500000"),
    );

    assert_eq!(
        Ok(2),
        Tick::new(2).convert_unticked("1.0000"),
    );

    assert_eq!(
        Ok(4),
        Tick::new(2000).convert_unticked("0.0020")
    );
}

#[test]
fn empty_fract_part() {
    assert_eq!(
        Ok(2),
        Tick::new(2).convert_unticked("1")
    );

    assert_eq!(
        Ok(2),
        Tick::new(2).convert_unticked("1.")
    );

    assert_eq!(
        Ok(0),
        Tick::new(321).convert_unticked("0.")
    );
}

#[test]
fn empty_int_part() {
    assert_eq!(
        Ok(5),
        Tick::new(10).convert_unticked(".5")
    );
}

#[test]
fn bad_int_part() {
    assert!(
        Tick::new(10).convert_unticked("abc").is_err()
    );

    assert!(
        Tick::new(10).convert_unticked("abc.5").is_err()
    );
}

#[test]
fn bad_fract_part() {
    assert!(
        Tick::new(10).convert_unticked("5.abc").is_err()
    );
}

#[test]
fn multiple_commas() {
    assert!(
        Tick::new(10).convert_unticked("5.23.4").is_err()
    );
}

#[test]
fn bad_divisor() {
    assert!(
        Tick::new(10).convert_unticked("5.11").is_err()
    );
}

#[test]
fn convert_ticked() {
    assert_eq!(
        Ok("1.15".to_owned()),
        Tick::new(100).convert_ticked(Tick::new(100).convert_unticked("1.15").unwrap())
    );

    assert_eq!(
        Ok("1.01".to_owned()),
        Tick::new(100).convert_ticked(Tick::new(100).convert_unticked("1.01").unwrap())
    );

    assert_eq!(
        Ok("0.15".to_owned()),
        Tick::new(20).convert_ticked(Tick::new(20).convert_unticked("0.15").unwrap())
    );

    assert_eq!(
        Ok("75.5".to_owned()),
        Tick::new(2).convert_ticked(Tick::new(2).convert_unticked("75.5").unwrap())
    );

    assert_eq!(
        Ok("1.00".to_owned()),
        Tick::new(20).convert_ticked(Tick::new(20).convert_unticked("1").unwrap())
    );

    assert!(
        Tick::new(23).convert_ticked(Tick::new(10).convert_unticked("75.4").unwrap()).is_err()
    );
}
