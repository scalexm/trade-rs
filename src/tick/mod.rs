//! On electronic exchanges, prices and sizes do not take continuous real values,
//! but rather take their values on a discrete grid whose step is known as a *tick*.
//! In other words, the price tick is the smallest possible change of the price of
//! an asset, and the size tick is the smallest possible change of the size of an
//! order.
//! 
//! It is important to represent prices and sizes in ticks and not with fractional
//! values like `100.27`.
//! 
//! Indeed, these fractional values could maybe be represented with a floating point
//! numeric type, but then some prices would not be represented exactly and rounded
//! to the nearest representable value, which is problematic because for some assets,
//! even e.g. a 1 cent difference is a lot. They could also be represented with
//! an arbitrary precision numeric type, but this would incur a lot of overhead.
//! 
//! Another problem is that many trading algorithms which involve making numerical
//! computations output floating values, and those values must be rounded to the
//! nearest tick in order to have a valid price / size, so generally the tick size
//! must be carried along anyway.
//! 
//! This module defines utilities for converting between fractional values represented
//! as strings (for exact precision) and values expressed in tick units.

mod test;

use std::fmt;
use std::convert::TryInto;
use std::borrow::Cow;
use failure_derive::Fail;
use serde_derive::{Serialize, Deserialize};

/// Base type for tick units;
pub type TickUnit = u64;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An object carrying the number of ticks per unit of something
/// and representative of its tick size.
/// 
/// Example: BTC is quoted on exchanges up to a precision of 1e-8, i.e.
/// the tick size is 1e-8, so the number of ticks per unit would be 1e8.
/// 
/// Used for both prices and sizes.
pub struct Tick(TickUnit);

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}^-1)", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Tickable {
    Ticked(TickUnit),
    Unticked(String),
}

impl Tickable {
    pub fn ticked(&self, tick: Tick) -> TickUnit {
        match self {
            Tickable::Ticked(value) => *value,
            Tickable::Unticked(value) => tick.ticked(&value).unwrap()
        }
    }

    pub fn unticked(&'_ self, tick: Tick) -> Cow<'_, str> {
        match self {
            Tickable::Ticked(value) => Cow::Owned(tick.unticked(*value).unwrap()),
            Tickable::Unticked(value) => Cow::Borrowed(&value)
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
#[fail(display = "Failed to convert {:?} with tick {}", value, tick)]
/// An error which indicates that the conversion between an unticked value and a
/// value in ticks has failed.
pub struct ConversionError {
    tick: Tick,
    value: Tickable,
}

impl ConversionError {
    fn ticked(value: TickUnit, tick: Tick) -> Self {
        ConversionError {
            tick,
            value: Tickable::Ticked(value),
        }
    }

    fn unticked(value: String, tick: Tick) -> Self {
        ConversionError {
            tick,
            value: Tickable::Unticked(value),
        }
    }
}

impl Tick {
    /// Return a new `Tick` with given `ticks_per_unit`.
    /// 
    /// # Panics
    /// Panic if `ticks_per_unit` is `0`.
    pub fn new(ticks_per_unit: TickUnit) -> Self {
        if ticks_per_unit == 0 {
            panic!("`ticks_per_unit` cannot be 0");
        }

        Tick(ticks_per_unit)
    }

    /// Return the number of ticks per unit carried by `self`.
    pub fn ticks_per_unit(self) -> TickUnit {
        self.0
    }

    /// Convert an unticked value, e.g. "0.001" into a value expressed in ticks,
    /// e.g. if `self.ticks_per_unit == 1000" then this would return `Ok(1)`.
    /// 
    /// # Errors
    /// Return `Err` if the value is in an incorrect format or if the number of ticks per
    /// unit is badly chosen.
    /// 
    /// # Panics
    /// Panic in case of overflow.
    /// Should correctly handle numbers up to `100,000,000,000.00000001` when using a 10^-8
    /// precision, which seems ok. 
    pub fn ticked(self, unticked: &str) -> Result<TickUnit, ConversionError> {
        let mut denom: u128 = 0;

        let mut int: u64 = 0;
        let mut fract: u64 = 0;
        let mut base: u64 = 1;
        let mut left = false;
        for c in unticked.chars().rev() {
            let digit = match c {
                '0' ... '9' => (c as u64) - ('0' as u64),
                '.' => {
                    left = true;
                    denom = u128::from(base);
                    base = 1;
                    continue;
                }
                _ => return Err(ConversionError::unticked(unticked.to_owned(), self)),
            };

            if left {
                int = int.checked_add(digit.checked_mul(base).unwrap()).unwrap();
            } else {
                fract = fract.checked_add(digit.checked_mul(base).unwrap()).unwrap();
            }
            base = base.checked_mul(10).unwrap();
        }

        if !left {
            int = fract;
            fract = 0;
            denom = 1;
        }

        let num = u128::from(int)
            .checked_mul(denom).unwrap()
            .checked_add(u128::from(fract)).unwrap()
            .checked_mul(u128::from(self.0)).unwrap();

        if num % denom != 0 {
            return Err(ConversionError::unticked(unticked.to_owned(), self));
        }

        Ok((num / denom).try_into().unwrap())
    }

    /// Convert a value expressed in ticks back to an unticked value.
    ///
    /// # Errors
    /// Return `Err` if the number of ticks per unit does not divide some power of 10.
    /// 
    /// # Panics
    /// Panic in case of overflow.
    pub fn unticked(self, ticked: TickUnit) -> Result<String, ConversionError> {
        let mut pad: usize = 0;
        let mut pow: u64 = 1;
        while self.0 > pow {
            pad += 1;
            pow = pow.checked_mul(10).unwrap();
        }

        if pow % self.0 != 0 {
            return Err(ConversionError::ticked(ticked.to_owned(), self));
        }

        let int = ticked / self.0;

        let pow = u128::from(pow);
        let fract = (pow * u128::from(ticked) / u128::from(self.0)) % pow; // cannot overflow
        let fract: u64 = fract.try_into().unwrap();

        fn write(mut num: u64, out: &mut [u8], mut used: usize) -> usize {
            loop {
                let digit = (num % 10) as u8;
                out[used] = b'0' + digit;

                num /= 10;
                used += 1;

                if num == 0 {
                    break;
                }
            }
            used
        };
        
        let mut out = [b'0'; 21];
        let _ = write(fract, &mut out[..], 0);
        out[pad] = b'.';
        let used = write(int, &mut out[..], pad + 1);
        
        let mut s = Vec::with_capacity(used);
        for c in out[..used].iter().rev() {
            s.push(*c as u8);
        }
        Ok(unsafe { String::from_utf8_unchecked(s) })
    }
}
