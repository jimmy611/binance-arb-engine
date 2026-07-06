//! Fixed-point decimal used for every price, quantity, and notional in the
//! engine.
//!
//! # Why fixed-point and not `f64`
//!
//! Prices and quantities must be exact. A float round-trip can turn a Binance
//! quantity of `0.10000000` into `0.099999999999999992`, and once that error
//! enters a chain of multiply/divide steps across three legs it becomes visible
//! money. We therefore store every monetary value as an `i128` scaled by
//! `10^8` (eight fractional digits, which is Binance spot's maximum quoting
//! precision). All arithmetic is integer arithmetic with explicit,
//! round-half-away-from-zero rounding at the single point where a product or
//! quotient must be re-scaled.
//!
//! # Why `i128` and eight digits
//!
//! `i128` at scale `10^8` represents magnitudes up to roughly `1.7e30`, which
//! dwarfs any realistic notional, while keeping every intermediate product of
//! two values (up to `1e17 * 1e17 = 1e34`) inside the type before re-scaling.
//! Eight fractional digits match Binance's `LOT_SIZE`/`PRICE_FILTER` precision
//! exactly, so no exchange-quoted value ever loses information on ingest.
//!
//! # Money versus rates
//!
//! This type is for *amounts*. Exchange *rates* (for example `1 / ask`) and the
//! log-space weights used by the negative-cycle detector are held as `f64`,
//! exactly as the specification permits, because a rate like `1/60000` needs
//! more than eight significant fractional digits and its relative error there is
//! bounded and controlled. Amounts stay exact; rates are allowed to be
//! floating point.
//!
//! # Hot-path discipline
//!
//! The `checked_*` methods are the API for the hot path: they return `None` on
//! overflow so the caller can treat it as a risk event rather than a crash. The
//! `+ - * /` operators are provided for tests and cold paths and will panic on
//! overflow (with a clear message) or division by zero. Do not use the
//! operators on the hot path.

use core::fmt;
use core::iter::Sum;
use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use core::str::FromStr;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::CoreError;

/// Number of fractional decimal digits stored.
pub const SCALE: u32 = 8;

/// `10^SCALE`. Every [`Decimal`] stores `value * SCALE_FACTOR` as its raw i128.
pub const SCALE_FACTOR: i128 = 100_000_000;

// Compile-time guarantee that SCALE and SCALE_FACTOR stay in sync.
const _: () = assert!(SCALE_FACTOR == 10_i128.pow(SCALE));

/// A signed fixed-point decimal with eight fractional digits, stored as an
/// `i128` equal to `value * 10^8`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Decimal(i128);

impl Decimal {
    /// The value `0`.
    pub const ZERO: Decimal = Decimal(0);
    /// The value `1`.
    pub const ONE: Decimal = Decimal(SCALE_FACTOR);
    /// The smallest representable positive value (`0.00000001`).
    pub const TICK: Decimal = Decimal(1);
    /// The largest representable value.
    pub const MAX: Decimal = Decimal(i128::MAX);
    /// The smallest (most negative) representable value.
    pub const MIN: Decimal = Decimal(i128::MIN);

    /// Constructs from a raw scaled integer (`value * 10^8`). Use when the
    /// source already speaks the internal representation, for example an
    /// exchange integer price at a known scale.
    #[inline]
    #[must_use]
    pub const fn from_raw(raw: i128) -> Decimal {
        Decimal(raw)
    }

    /// Returns the raw scaled integer (`value * 10^8`).
    #[inline]
    #[must_use]
    pub const fn raw(self) -> i128 {
        self.0
    }

    /// Constructs from a whole integer number of units.
    #[inline]
    #[must_use]
    pub const fn from_int(n: i64) -> Decimal {
        // i64::MAX * 10^8 is about 9.2e26, comfortably inside i128.
        Decimal((n as i128) * SCALE_FACTOR)
    }

    /// The number of fractional digits (always [`SCALE`]).
    #[inline]
    #[must_use]
    pub const fn scale() -> u32 {
        SCALE
    }

    /// Parses an exact decimal string such as `"0.00012300"` or `"-42"`.
    ///
    /// Accepts an optional leading sign, an integer part, and an optional
    /// fractional part. Inputs with more than [`SCALE`] fractional digits are
    /// rounded half away from zero (a safety net; Binance never quotes beyond
    /// eight digits). Returns [`CoreError::ParseDecimal`] on any malformed input
    /// or on magnitude overflow.
    pub fn from_str_exact(input: &str) -> Result<Decimal, CoreError> {
        let parse_err = |reason: &'static str| CoreError::ParseDecimal {
            input: input.to_owned(),
            reason,
        };

        let bytes = input.as_bytes();
        if bytes.is_empty() {
            return Err(parse_err("empty"));
        }

        let (neg, rest) = match bytes[0] {
            b'+' => (false, &input[1..]),
            b'-' => (true, &input[1..]),
            _ => (false, input),
        };
        if rest.is_empty() {
            return Err(parse_err("no digits"));
        }

        let mut parts = rest.splitn(2, '.');
        let int_str = parts.next().unwrap_or("");
        let frac_str = parts.next().unwrap_or("");
        if frac_str.contains('.') {
            return Err(parse_err("multiple decimal points"));
        }
        if int_str.is_empty() && frac_str.is_empty() {
            return Err(parse_err("no digits"));
        }

        // Integer part.
        let mut int_val: i128 = 0;
        for &c in int_str.as_bytes() {
            if !c.is_ascii_digit() {
                return Err(parse_err("non-digit in integer part"));
            }
            int_val = int_val
                .checked_mul(10)
                .and_then(|v| v.checked_add(i128::from(c - b'0')))
                .ok_or_else(|| parse_err("magnitude too large"))?;
        }
        let mut scaled = int_val
            .checked_mul(SCALE_FACTOR)
            .ok_or_else(|| parse_err("magnitude too large"))?;

        // Fractional part: take the first SCALE digits, round on the next one.
        let fb = frac_str.as_bytes();
        for &c in fb {
            if !c.is_ascii_digit() {
                return Err(parse_err("non-digit in fractional part"));
            }
        }
        let mut frac_scaled: i128 = 0;
        for i in 0..SCALE as usize {
            let d = if i < fb.len() {
                i128::from(fb[i] - b'0')
            } else {
                0
            };
            frac_scaled = frac_scaled * 10 + d;
        }
        if fb.len() > SCALE as usize && (fb[SCALE as usize] - b'0') >= 5 {
            // Round half away from zero. The carry (0.99999999 -> 1.0) is
            // absorbed correctly because frac_scaled may reach SCALE_FACTOR and
            // is simply added to the scaled integer part below.
            frac_scaled += 1;
        }

        scaled = scaled
            .checked_add(frac_scaled)
            .ok_or_else(|| parse_err("magnitude too large"))?;

        let raw = if neg {
            // scaled is non-negative, so negation cannot overflow.
            -scaled
        } else {
            scaled
        };
        Ok(Decimal(raw))
    }

    /// Lossy conversion to `f64`, used only in the log-space detector and for
    /// display/analytics. Never feed the result back into money arithmetic.
    #[inline]
    #[must_use]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / SCALE_FACTOR as f64
    }

    /// Constructs from an `f64`, rounding to the nearest tick. Intended for
    /// configuration and tests, not for exchange data (which arrives as exact
    /// strings). Returns `None` if the value is non-finite or out of range.
    #[must_use]
    pub fn from_f64(x: f64) -> Option<Decimal> {
        if !x.is_finite() {
            return None;
        }
        let scaled = (x * SCALE_FACTOR as f64).round();
        let lo = i128::MIN as f64;
        let hi = i128::MAX as f64;
        if !(lo..=hi).contains(&scaled) {
            return None;
        }
        Some(Decimal(scaled as i128))
    }

    /// Checked addition. Returns `None` on overflow.
    #[inline]
    #[must_use]
    pub const fn checked_add(self, rhs: Decimal) -> Option<Decimal> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Decimal(v)),
            None => None,
        }
    }

    /// Checked subtraction. Returns `None` on overflow.
    #[inline]
    #[must_use]
    pub const fn checked_sub(self, rhs: Decimal) -> Option<Decimal> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Decimal(v)),
            None => None,
        }
    }

    /// Checked multiplication with round-half-away-from-zero re-scaling. Returns
    /// `None` on overflow of the intermediate product.
    #[inline]
    #[must_use]
    pub const fn checked_mul(self, rhs: Decimal) -> Option<Decimal> {
        match self.0.checked_mul(rhs.0) {
            Some(prod) => match div_round(prod, SCALE_FACTOR) {
                Some(v) => Some(Decimal(v)),
                None => None,
            },
            None => None,
        }
    }

    /// Checked division with round-half-away-from-zero. Returns `None` on
    /// division by zero or on overflow of the intermediate numerator.
    #[inline]
    #[must_use]
    pub const fn checked_div(self, rhs: Decimal) -> Option<Decimal> {
        match self.0.checked_mul(SCALE_FACTOR) {
            Some(num) => match div_round(num, rhs.0) {
                Some(v) => Some(Decimal(v)),
                None => None,
            },
            None => None,
        }
    }

    /// Absolute value. Returns `None` only for [`Decimal::MIN`].
    #[inline]
    #[must_use]
    pub const fn checked_abs(self) -> Option<Decimal> {
        match self.0.checked_abs() {
            Some(v) => Some(Decimal(v)),
            None => None,
        }
    }

    /// Absolute value, saturating [`Decimal::MIN`] to [`Decimal::MAX`]. Real
    /// values are nowhere near the representable bound, so this is exact in
    /// practice.
    #[inline]
    #[must_use]
    pub const fn abs(self) -> Decimal {
        Decimal(self.0.saturating_abs())
    }

    /// Returns `true` if the value is exactly zero.
    #[inline]
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Returns `true` if the value is strictly greater than zero.
    #[inline]
    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Returns `true` if the value is strictly less than zero.
    #[inline]
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Returns `-1`, `0`, or `1` matching the sign of the value.
    #[inline]
    #[must_use]
    pub const fn signum(self) -> i8 {
        if self.0 < 0 {
            -1
        } else if self.0 > 0 {
            1
        } else {
            0
        }
    }

    /// The smaller of two values.
    #[inline]
    #[must_use]
    pub fn min(self, other: Decimal) -> Decimal {
        if self.0 <= other.0 {
            self
        } else {
            other
        }
    }

    /// The larger of two values.
    #[inline]
    #[must_use]
    pub fn max(self, other: Decimal) -> Decimal {
        if self.0 >= other.0 {
            self
        } else {
            other
        }
    }
}

/// Divides `num` by `den`, rounding half away from zero. `const` so the checked
/// arithmetic can be used in const contexts. Returns `None` only when `den` is
/// zero.
const fn div_round(num: i128, den: i128) -> Option<i128> {
    if den == 0 {
        return None;
    }
    let q = num / den;
    let r = num % den;
    if r == 0 {
        return Some(q);
    }
    // r_abs * 2 cannot overflow u128: |r| < |den| <= i128::MAX < 2^127.
    let round_away = r.unsigned_abs() * 2 >= den.unsigned_abs();
    if round_away {
        let negative = (num < 0) != (den < 0);
        if negative {
            q.checked_sub(1)
        } else {
            q.checked_add(1)
        }
    } else {
        Some(q)
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mag = self.0.unsigned_abs();
        let factor = SCALE_FACTOR as u128;
        let int = mag / factor;
        let frac = mag % factor;
        if self.0 < 0 && (int != 0 || frac != 0) {
            write!(f, "-")?;
        }
        if frac == 0 {
            write!(f, "{int}")
        } else {
            let padded = format!("{frac:08}");
            let trimmed = padded.trim_end_matches('0');
            write!(f, "{int}.{trimmed}")
        }
    }
}

impl fmt::Debug for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Decimal({self})")
    }
}

impl FromStr for Decimal {
    type Err = CoreError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Decimal::from_str_exact(s)
    }
}

impl TryFrom<&str> for Decimal {
    type Error = CoreError;
    #[inline]
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Decimal::from_str_exact(s)
    }
}

impl From<i64> for Decimal {
    #[inline]
    fn from(n: i64) -> Decimal {
        Decimal::from_int(n)
    }
}

impl Add for Decimal {
    type Output = Decimal;
    #[inline]
    fn add(self, rhs: Decimal) -> Decimal {
        self.checked_add(rhs)
            .expect("decimal add overflow; use checked_add on the hot path")
    }
}

impl Sub for Decimal {
    type Output = Decimal;
    #[inline]
    fn sub(self, rhs: Decimal) -> Decimal {
        self.checked_sub(rhs)
            .expect("decimal sub overflow; use checked_sub on the hot path")
    }
}

impl Mul for Decimal {
    type Output = Decimal;
    #[inline]
    fn mul(self, rhs: Decimal) -> Decimal {
        self.checked_mul(rhs)
            .expect("decimal mul overflow; use checked_mul on the hot path")
    }
}

impl Div for Decimal {
    type Output = Decimal;
    #[inline]
    fn div(self, rhs: Decimal) -> Decimal {
        assert!(!rhs.is_zero(), "decimal division by zero");
        self.checked_div(rhs)
            .expect("decimal div overflow; use checked_div on the hot path")
    }
}

impl Neg for Decimal {
    type Output = Decimal;
    #[inline]
    fn neg(self) -> Decimal {
        Decimal(
            self.0
                .checked_neg()
                .expect("decimal negation overflow (Decimal::MIN)"),
        )
    }
}

impl AddAssign for Decimal {
    #[inline]
    fn add_assign(&mut self, rhs: Decimal) {
        *self = *self + rhs;
    }
}

impl SubAssign for Decimal {
    #[inline]
    fn sub_assign(&mut self, rhs: Decimal) {
        *self = *self - rhs;
    }
}

impl Sum for Decimal {
    fn sum<I: Iterator<Item = Decimal>>(iter: I) -> Decimal {
        iter.fold(Decimal::ZERO, |acc, x| acc + x)
    }
}

impl<'a> Sum<&'a Decimal> for Decimal {
    fn sum<I: Iterator<Item = &'a Decimal>>(iter: I) -> Decimal {
        iter.fold(Decimal::ZERO, |acc, x| acc + *x)
    }
}

impl Serialize for Decimal {
    /// Serializes as the canonical decimal string, keeping values exact and
    /// human-readable in logs and JSON. Binary formats (WAL) encode the same
    /// string; it round-trips losslessly through [`Decimal::from_str_exact`].
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Decimal::from_str_exact(&s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display_roundtrip() {
        for s in [
            "0",
            "1",
            "-1",
            "42",
            "0.00000001",
            "0.1",
            "123.456",
            "-123.456",
            "1000000.00000001",
        ] {
            let d = Decimal::from_str_exact(s).unwrap();
            assert_eq!(d.to_string(), s, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn display_trims_trailing_zeros() {
        assert_eq!(
            Decimal::from_str_exact("1.50000000").unwrap().to_string(),
            "1.5"
        );
        assert_eq!(
            Decimal::from_str_exact("2.00000000").unwrap().to_string(),
            "2"
        );
    }

    #[test]
    fn negative_zero_displays_as_zero() {
        assert_eq!(Decimal::from_str_exact("-0").unwrap().to_string(), "0");
        assert_eq!(
            Decimal::from_str_exact("-0.00000000").unwrap().to_string(),
            "0"
        );
    }

    #[test]
    fn rounds_beyond_eight_digits() {
        // Ninth digit 5 rounds the eighth up.
        assert_eq!(
            Decimal::from_str_exact("0.000000005").unwrap(),
            Decimal::from_str_exact("0.00000001").unwrap()
        );
        // Ninth digit 4 truncates.
        assert_eq!(
            Decimal::from_str_exact("0.000000004").unwrap(),
            Decimal::ZERO
        );
        // Carry across the decimal point.
        assert_eq!(
            Decimal::from_str_exact("0.999999995").unwrap(),
            Decimal::ONE
        );
    }

    #[test]
    fn rejects_malformed() {
        for s in ["", "-", "+", "1.2.3", "abc", "1..2", "1a", "0x10", " 1"] {
            assert!(Decimal::from_str_exact(s).is_err(), "should reject {s:?}");
        }
    }

    #[test]
    fn arithmetic_basics() {
        let a = Decimal::from_str_exact("1.5").unwrap();
        let b = Decimal::from_str_exact("0.25").unwrap();
        assert_eq!((a + b).to_string(), "1.75");
        assert_eq!((a - b).to_string(), "1.25");
        assert_eq!((a * b).to_string(), "0.375");
        assert_eq!((a / b).to_string(), "6");
    }

    #[test]
    fn mul_rounds_half_away_from_zero() {
        // 0.00000001 * 0.5 = 0.000000005 -> rounds to 0.00000001.
        let tick = Decimal::TICK;
        let half = Decimal::from_str_exact("0.5").unwrap();
        assert_eq!(tick.checked_mul(half).unwrap(), Decimal::TICK);
        // And on the negative side, away from zero.
        assert_eq!((-tick).checked_mul(half).unwrap(), -Decimal::TICK);
    }

    #[test]
    fn identity_and_zero() {
        let x = Decimal::from_str_exact("123.45").unwrap();
        assert_eq!(x * Decimal::ONE, x);
        assert_eq!(x * Decimal::ZERO, Decimal::ZERO);
        assert_eq!(x + Decimal::ZERO, x);
    }

    #[test]
    fn checked_overflow_returns_none() {
        assert_eq!(Decimal::MAX.checked_add(Decimal::ONE), None);
        assert_eq!(Decimal::MAX.checked_mul(Decimal::from_int(2)), None);
    }

    #[test]
    fn checked_div_by_zero_is_none() {
        assert_eq!(Decimal::ONE.checked_div(Decimal::ZERO), None);
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn operator_div_by_zero_panics() {
        let _ = Decimal::ONE / Decimal::ZERO;
    }

    #[test]
    fn ordering_matches_value() {
        let a = Decimal::from_str_exact("-1").unwrap();
        let b = Decimal::from_str_exact("0").unwrap();
        let c = Decimal::from_str_exact("0.00000001").unwrap();
        assert!(a < b && b < c);
        assert_eq!(a.max(c), c);
        assert_eq!(a.min(c), a);
    }

    #[test]
    fn signum_and_abs() {
        assert_eq!(Decimal::from_int(-5).signum(), -1);
        assert_eq!(Decimal::ZERO.signum(), 0);
        assert_eq!(Decimal::from_int(5).signum(), 1);
        assert_eq!(Decimal::from_int(-5).abs(), Decimal::from_int(5));
    }

    #[test]
    fn serde_json_roundtrip() {
        let x = Decimal::from_str_exact("-123.45678901").unwrap();
        let json = serde_json::to_string(&x).unwrap();
        assert_eq!(json, "\"-123.45678901\"");
        let back: Decimal = serde_json::from_str(&json).unwrap();
        assert_eq!(back, x);
    }

    #[test]
    fn sum_iterator() {
        let xs = [
            Decimal::from_str_exact("0.1").unwrap(),
            Decimal::from_str_exact("0.2").unwrap(),
            Decimal::from_str_exact("0.3").unwrap(),
        ];
        let s: Decimal = xs.iter().copied().sum();
        assert_eq!(s.to_string(), "0.6");
    }
}
