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

const DECIMALS: usize = 4;
const SCALE: u64 = 10_000;

impl Amount {
    #[cfg(test)]
    pub const ZERO: Amount = Amount(0);

    #[cfg(test)]
    pub const fn from_scaled(units: i64) -> Amount {
        Amount(units)
    }

    // Only checked arithmetic is exposed, so an unchecked operation on money
    // cannot be written by accident.
    pub fn checked_add(self, rhs: Amount) -> Option<Amount> {
        self.0.checked_add(rhs.0).map(Amount)
    }

    pub fn checked_sub(self, rhs: Amount) -> Option<Amount> {
        self.0.checked_sub(rhs.0).map(Amount)
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }
}

impl FromStr for Amount {
    type Err = AmountError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(AmountError::Empty);
        }
        let (negative, unsigned) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        let is_digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
        if (whole.is_empty() && fraction.is_empty()) || !is_digits(whole) || !is_digits(fraction) {
            return Err(AmountError::InvalidFormat);
        }
        if fraction.len() > DECIMALS {
            return Err(AmountError::TooManyDecimals);
        }

        // Negative values accumulate downwards so that i64::MIN, whose
        // magnitude has no positive i64 counterpart, still parses.
        let padding = std::iter::repeat_n(b'0', DECIMALS - fraction.len());
        let mut digits = whole.bytes().chain(fraction.bytes()).chain(padding);
        let units = digits.try_fold(0i64, |acc, digit| {
            let digit = i64::from(digit - b'0');
            let shifted = acc.checked_mul(10)?;
            if negative {
                shifted.checked_sub(digit)
            } else {
                shifted.checked_add(digit)
            }
        });
        units.map(Amount).ok_or(AmountError::Overflow)
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let magnitude = self.0.unsigned_abs();
        write!(f, "{sign}{}.{:04}", magnitude / SCALE, magnitude % SCALE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn parse(s: &str) -> Result<Amount, AmountError> {
        s.parse()
    }

    fn scaled(units: i64) -> Amount {
        Amount::from_scaled(units)
    }

    #[test]
    fn parses_whole_and_fractional_values() {
        assert_eq!(parse("1"), Ok(scaled(10_000)));
        assert_eq!(parse("0"), Ok(Amount::ZERO));
        assert_eq!(parse("1.5"), Ok(scaled(15_000)));
        assert_eq!(parse("1.2345"), Ok(scaled(12_345)));
        assert_eq!(parse("0.0001"), Ok(scaled(1)));
        assert_eq!(parse("007.10"), Ok(scaled(71_000)));
    }

    #[test]
    fn parses_bare_trailing_or_leading_point() {
        assert_eq!(parse("1."), Ok(scaled(10_000)));
        assert_eq!(parse(".5"), Ok(scaled(5_000)));
    }

    #[test]
    fn parses_negative_values() {
        assert_eq!(parse("-1.5"), Ok(scaled(-15_000)));
        assert_eq!(parse("-.5"), Ok(scaled(-5_000)));
        assert_eq!(parse("-0"), Ok(Amount::ZERO));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(parse("  2.5 "), Ok(scaled(25_000)));
        assert_eq!(parse("\t3\t"), Ok(scaled(30_000)));
    }

    #[test]
    fn parses_range_bounds() {
        assert_eq!(parse("922337203685477.5807"), Ok(scaled(i64::MAX)));
        assert_eq!(parse("-922337203685477.5808"), Ok(scaled(i64::MIN)));
    }

    #[test]
    fn rejects_values_beyond_range() {
        assert_eq!(parse("922337203685477.5808"), Err(AmountError::Overflow));
        assert_eq!(parse("-922337203685477.5809"), Err(AmountError::Overflow));
        assert_eq!(parse("99999999999999999999"), Err(AmountError::Overflow));
    }

    #[test]
    fn rejects_more_than_four_decimals() {
        assert_eq!(parse("1.23456"), Err(AmountError::TooManyDecimals));
        assert_eq!(parse("0.00001"), Err(AmountError::TooManyDecimals));
        assert_eq!(parse("1.00000"), Err(AmountError::TooManyDecimals));
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse(""), Err(AmountError::Empty));
        assert_eq!(parse("   "), Err(AmountError::Empty));
    }

    #[test]
    fn rejects_malformed_input() {
        for input in [
            ".", "-", "-.", "+1", "1e3", "1 .5", "1. 5", "1.2.3", "abc", "--1", "1,5", "0x10",
            "1-", "١", "- 1",
        ] {
            assert_eq!(parse(input), Err(AmountError::InvalidFormat), "{input:?}");
        }
    }

    #[test]
    fn formats_with_exactly_four_decimals() {
        assert_eq!(Amount::ZERO.to_string(), "0.0000");
        assert_eq!(scaled(1).to_string(), "0.0001");
        assert_eq!(scaled(15_000).to_string(), "1.5000");
        assert_eq!(scaled(12_345_678).to_string(), "1234.5678");
    }

    #[test]
    fn formats_negative_values() {
        assert_eq!(scaled(-2_500).to_string(), "-0.2500");
        assert_eq!(scaled(-15_000).to_string(), "-1.5000");
    }

    #[test]
    fn formats_range_bounds() {
        assert_eq!(scaled(i64::MAX).to_string(), "922337203685477.5807");
        assert_eq!(scaled(i64::MIN).to_string(), "-922337203685477.5808");
    }

    #[test]
    fn checked_add_detects_overflow() {
        assert_eq!(scaled(1).checked_add(scaled(2)), Some(scaled(3)));
        assert_eq!(scaled(-5).checked_add(scaled(2)), Some(scaled(-3)));
        assert_eq!(scaled(i64::MAX).checked_add(scaled(1)), None);
        assert_eq!(scaled(i64::MIN).checked_add(scaled(-1)), None);
    }

    #[test]
    fn checked_sub_detects_overflow_and_allows_negative_results() {
        assert_eq!(scaled(3).checked_sub(scaled(2)), Some(scaled(1)));
        assert_eq!(scaled(2).checked_sub(scaled(3)), Some(scaled(-1)));
        assert_eq!(scaled(i64::MIN).checked_sub(scaled(1)), None);
        assert_eq!(scaled(i64::MAX).checked_sub(scaled(-1)), None);
    }

    #[test]
    fn is_positive_excludes_zero_and_negatives() {
        assert!(scaled(1).is_positive());
        assert!(!Amount::ZERO.is_positive());
        assert!(!scaled(-1).is_positive());
    }

    proptest! {
        #[test]
        fn format_then_parse_round_trips(units in any::<i64>()) {
            let amount = scaled(units);
            prop_assert_eq!(parse(&amount.to_string()), Ok(amount));
        }

        #[test]
        fn parse_never_panics(input in any::<String>()) {
            let _ = parse(&input);
        }

        #[test]
        fn parse_accepts_any_four_decimal_number(whole in 0u64..1_000_000_000, frac in 0u32..10_000) {
            let expected = scaled(whole as i64 * 10_000 + i64::from(frac));
            prop_assert_eq!(parse(&format!("{whole}.{frac:04}")), Ok(expected));
        }
    }
}
