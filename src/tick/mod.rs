mod test;

use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Tick {
    ticks_per_unit: usize,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Fail)]
pub struct ConversionError(String, Tick);

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to convert unticked value {} with tick {:?}", self.0, self.1)
    }
}

impl Tick {
    pub fn new(ticks_per_unit: usize) -> Self {
        if ticks_per_unit == 0 {
            panic!("`ticks_per_unit` cannot be 0");
        }

        Tick {
            ticks_per_unit,
        }
    }

    pub fn convert_unticked(&self, unticked: &str) -> Result<usize, ConversionError> {
        let mut parts = unticked.split('.').take(2);
        let (int, fract) = match (parts.next(), parts.next()) {
            (Some(int), Some(fract)) => (int, fract),
            (Some(int), None) => (int, ""),
            (None, _) => return Err(ConversionError(unticked.to_owned(), *self)),
        };

        if parts.next().is_some() {
            return Err(ConversionError(unticked.to_owned(), *self));
        }

        let int = if int.is_empty() {
            0
        } else {
            match int.parse() {
                Ok(int) => int,
                Err(..) => return Err(ConversionError(unticked.to_owned(), *self)),
            }
        };

        let (fract, divisor) = if fract.is_empty() {
            (0, 1)
        } else {
            let trailing_zeros = fract.chars().rev().take_while(|&c| c == '0').count();
            if trailing_zeros == fract.len() {
                (0, 1)
            } else {
                let preceding_zeros = fract.chars().take_while(|&c| c == '0').count();
                let part_without_zeros = &fract[preceding_zeros .. (fract.len() - trailing_zeros)];

                let fract = match part_without_zeros.parse() {
                    Ok(fract) => fract,
                    Err(..) => return Err(ConversionError(unticked.to_owned(), *self)),
                };
                (fract, 10_usize.pow((part_without_zeros.len() + preceding_zeros) as _))
            }
        };

        let mut value = self.ticks_per_unit * fract;
        if value % divisor != 0 {
            return Err(ConversionError(unticked.to_owned(), *self));
        }
        value /= divisor;

        Ok(int * self.ticks_per_unit + value)
    }
}
