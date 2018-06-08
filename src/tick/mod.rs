mod test;

use std::fmt;
use num::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
/// An object carrying the number of ticks per unit of something
/// and representative of its tick size.
/// 
/// Example: BTC is quoted on exchanges up to a precision of 1e-8, i.e.
/// the tick size is 1e-8, so the number of ticks per unit would be 1e8.
/// 
/// Used for both prices and sizes.
pub struct Tick {
    ticks_per_unit: u64,
}

impl fmt::Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}^-1)", self.ticks_per_unit)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum ValueType {
    Ticked(u64),
    Unticked(String),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
#[fail(display = "Failed to convert {:?} with tick {}", value, tick)]
/// An error which indicated that the conversion of an unticked value to a
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

        Tick {
            ticks_per_unit,
        }
    }

    /// Convert an unticked value, e.g. "0.001" into a value expressed in ticks,
    /// e.g. if `self.ticks_per_unit == 1000" then this would return `Ok(1)`.
    /// 
    /// Return an error if the value was in an incorrect format or if the number of ticks per
    /// unit was badly chosen.
    pub fn convert_unticked(&self, unticked: &str) -> Result<u64, ConversionError> {
        let mut parts = unticked.split('.').take(2);
        let (int, fract) = match (parts.next(), parts.next()) {
            (Some(int), Some(fract)) => (int, fract),
            (Some(int), None) => (int, ""),
            (None, _) => return Err(ConversionError::unticked(unticked, *self)),
        };

        let ratio: rational::Ratio<u64> = match Num::from_str_radix(
            &format!("{}{}/{}", int, fract, 10_u64.pow(fract.len() as u32)),
            10
        )
        {
            Ok(result) => result,
            Err(..) => return Err(ConversionError::unticked(unticked, *self)),
        };

        let ratio = rational::Ratio::from_integer(self.ticks_per_unit) * ratio;

        if !ratio.is_integer() {
            return Err(ConversionError::unticked(unticked, *self));
        }

        Ok(ratio.to_integer())
    }

    /// Convert a value expressed in ticks back to an unticked value.
    ///
    /// Return an error if the number of ticks per unit does not divide some power of 10.
    pub fn convert_ticked(&self, ticked: u64) -> Result<String, ConversionError> {
        let mut pow = 1;
        let mut pad = 0;
        while self.ticks_per_unit > pow {
            pad += 1;
            pow *= 10;
        }

        if pow % self.ticks_per_unit != 0 {
            return Err(ConversionError::ticked(ticked, *self));
        }

        let int = ticked / self.ticks_per_unit;
        Ok(format!(
                "{0}.{1:02$}",
            int,
            (pow * ticked / self.ticks_per_unit) % pow,
            pad
        ))
    }
}
