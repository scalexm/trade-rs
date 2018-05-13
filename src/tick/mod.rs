mod test;

use std::fmt;
use num::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
/// An object carrying the number of ticks per unit of something
/// and representative of its tick size.
/// 
/// Example: BTC is quoted on exchanges up to a precision of 1e-8, i.e.
/// the tick size is 1e-8, so the number of ticks per unit would be 1e8.
/// 
/// Used for both prices and sizes.
pub struct Tick {
    ticks_per_unit: usize,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
/// An error which indicated that the conversion of an unticked value to a
/// value in ticks has failed.
pub struct ConversionError(Tick);

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to convert unticked value with tick {:?}", self.0)
    }
}

impl Tick {
    /// Return a new `Tick` with given `ticks_per_unit`.
    pub fn new(ticks_per_unit: usize) -> Self {
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
    pub fn convert_unticked(&self, unticked: &str) -> Result<usize, ConversionError> {
        let mut parts = unticked.split('.').take(2);
        let (int, fract) = match (parts.next(), parts.next()) {
            (Some(int), Some(fract)) => (int, fract),
            (Some(int), None) => (int, ""),
            (None, _) => return Err(ConversionError(*self)),
        };

        let ratio: rational::Ratio<usize> = match Num::from_str_radix(
            &format!("{}{}/{}", int, fract, 10_usize.pow(fract.len() as u32)),
            10
        )
        {
            Ok(result) => result,
            Err(..) => return Err(ConversionError(*self)),
        };

        let ratio = rational::Ratio::from_integer(self.ticks_per_unit) * ratio;

        if !ratio.is_integer() {
            return Err(ConversionError(*self));
        }

        Ok(ratio.to_integer())
    }

    pub fn convert_ticked(&self, ticked: usize) -> Result<String, ConversionError> {
        let mut pow = 1;
        let mut pad = 0;
        while self.ticks_per_unit > pow {
            pad += 1;
            pow *= 10;
        }

        if pow % self.ticks_per_unit != 0 {
            return Err(ConversionError(*self));
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
