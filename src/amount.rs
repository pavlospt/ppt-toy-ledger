use std::fmt;
use std::str::FromStr;

use thiserror::Error;

// Money is a fixed-point integer in 1/10_000 units rather than a float or a
// general decimal: four places is the whole precision contract, so the type
// enforces it, and an 8-byte value keeps the per-deposit store small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Amount(i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AmountError {
    #[error("amount is empty")]
    Empty,
    #[error("amount is not a plain decimal number")]
    InvalidFormat,
    // Rejected rather than rounded: silently altering money is worse than
    // refusing a row.
    #[error("amount has more than 4 decimal places")]
    TooManyDecimals,
    #[error("amount is out of range")]
    Overflow,
}

impl Amount {
    pub const ZERO: Amount = Amount(0);

    #[cfg(test)]
    pub const fn from_scaled(units: i64) -> Amount {
        Amount(units)
    }

    // Only checked arithmetic is exposed, so an unchecked operation on money
    // cannot be written by accident.
    pub fn checked_add(self, rhs: Amount) -> Option<Amount> {
        todo!()
    }

    pub fn checked_sub(self, rhs: Amount) -> Option<Amount> {
        todo!()
    }

    pub fn is_positive(self) -> bool {
        todo!()
    }
}

impl FromStr for Amount {
    type Err = AmountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}
