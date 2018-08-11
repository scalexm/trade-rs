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
use num_rational::Ratio;
use std::convert::TryInto;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An object carrying the number of ticks per unit of something
/// and representative of its tick size.
/// 
/// Example: BTC is quoted on exchanges up to a precision of 1e-8, i.e.
/// the tick size is 1e-8, so the number of ticks per unit would be 1e8.
/// 
/// Used for both prices and sizes.
pub struct Tick(u64);

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}^-1)", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum ValueType {
    Ticked(u64),
    Unticked(String),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
#[fail(display = "Failed to convert {:?} with tick {}", value, tick)]
/// An error which indicates that the conversion between an unticked value and a
/// value in ticks has failed.
pub struct ConversionError {
    tick: Tick,
    value: ValueType,
}

impl ConversionError {
    fn ticked(value: u64, tick: Tick) -> Self {
        ConversionError {
            tick,
            value: ValueType::Ticked(value),
        }
    }

    fn unticked(value: &str, tick: Tick) -> Self {
        ConversionError {
            tick,
            value: ValueType::Unticked(value.to_owned()),
        }
    }
}

impl Tick {
    /// Return a new `Tick` with given `ticks_per_unit`.
    pub fn new(ticks_per_unit: u64) -> Self {
        if ticks_per_unit == 0 {
            panic!("`ticks_per_unit` cannot be 0");
        }

        Tick(ticks_per_unit)
    }

    /// Return the number of ticks per unit carried by `self`.
    pub fn ticks_per_unit(&self) -> u64 {
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
    pub fn convert_unticked(self, unticked: &str) -> Result<u64, ConversionError> {
        let mut parts = unticked.split('.').take(2);
        let (int, fract) = match (parts.next(), parts.next()) {
            (Some(int), Some(fract)) => (int, fract),
            (Some(int), None) => (int, ""),
            (None, _) => return Err(ConversionError::unticked(unticked, self)),
        };

        let denom = 10_u128.pow(fract.len() as u32);

        let int = if int.is_empty() {
            0
        } else {
            match int.parse::<u128>() {
                Ok(int) => int,
                Err(..) => return Err(ConversionError::unticked(unticked, self)),
            }
        };

        let fract = if fract.is_empty() {
            0
        } else {
            match fract.parse::<u128>() {
                Ok(fract) => fract,
                Err(..) => return Err(ConversionError::unticked(unticked, self)),
            }
        };

        // denom is non null so `Ratio::new` cannot fail
        let ratio = Ratio::new((int * denom + fract) * u128::from(self.0), denom);

        if !ratio.is_integer() {
            return Err(ConversionError::unticked(unticked, self));
        }

        Ok(ratio.to_integer().try_into().unwrap())
    }

    /// Convert a value expressed in ticks back to an unticked value.
    ///
    /// # Errors
    /// Return `Err` if the number of ticks per unit does not divide some power of 10.
    /// 
    /// # Panics
    /// Panic in case of overflow.
    pub fn convert_ticked(self, ticked: u64) -> Result<String, ConversionError> {
        let mut pow = 1;
        let mut pad = 0;
        while self.0 > pow {
            pad += 1;
            pow *= 10;
        }

        if pow % self.0 != 0 {
            return Err(ConversionError::ticked(ticked, self));
        }

        let int = ticked / self.0;
        let prevent_overflow = u128::from(pow) * u128::from(ticked)
            / u128::from(self.0);
        let prevent_overflow: u64 = prevent_overflow.try_into().unwrap();
        
        Ok(format!(
            "{0}.{1:02$}",
            int,
            prevent_overflow % pow,
            pad
        ))
    }
}
