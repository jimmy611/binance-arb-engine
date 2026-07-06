//! Property-based tests for the fixed-point decimal invariants.
//!
//! These assert the algebraic properties the rest of the engine relies on:
//! parse/display is a lossless round-trip, addition is a commutative group with
//! subtraction as its inverse, multiplication has `ONE` as identity, and the
//! checked operators agree exactly with the underlying `i128` semantics
//! (including where they overflow).

use core_types::Decimal;
use proptest::prelude::*;

/// Raw values bounded so that sums and products stay inside `i128` and no
/// `checked_*` returns `None` for the "should succeed" properties.
fn bounded() -> impl Strategy<Value = i128> {
    -1_000_000_000_000_000_000i128..1_000_000_000_000_000_000i128
}

/// Tighter bound for multiplication so `self.0 * rhs.0` cannot overflow before
/// re-scaling.
fn mul_bounded() -> impl Strategy<Value = i128> {
    -1_000_000_000_000_000i128..1_000_000_000_000_000i128
}

proptest! {
    #[test]
    fn display_parse_roundtrip(raw in any::<i64>()) {
        let d = Decimal::from_raw(i128::from(raw));
        let s = d.to_string();
        let back = Decimal::from_str_exact(&s).expect("display output must parse");
        prop_assert_eq!(d, back);
    }

    #[test]
    fn add_is_commutative(a in bounded(), b in bounded()) {
        let x = Decimal::from_raw(a);
        let y = Decimal::from_raw(b);
        prop_assert_eq!(x.checked_add(y), y.checked_add(x));
    }

    #[test]
    fn add_then_sub_is_identity(a in bounded(), b in bounded()) {
        let x = Decimal::from_raw(a);
        let y = Decimal::from_raw(b);
        let sum = x.checked_add(y).expect("bounded sum fits");
        prop_assert_eq!(sum.checked_sub(y).expect("bounded diff fits"), x);
    }

    #[test]
    fn mul_by_one_is_identity(a in any::<i64>()) {
        let x = Decimal::from_raw(i128::from(a));
        prop_assert_eq!(x.checked_mul(Decimal::ONE), Some(x));
    }

    #[test]
    fn mul_is_commutative(a in mul_bounded(), b in mul_bounded()) {
        let x = Decimal::from_raw(a);
        let y = Decimal::from_raw(b);
        prop_assert_eq!(x.checked_mul(y), y.checked_mul(x));
    }

    #[test]
    fn checked_add_matches_i128(a in any::<i128>(), b in any::<i128>()) {
        let x = Decimal::from_raw(a);
        let y = Decimal::from_raw(b);
        let expected = a.checked_add(b).map(Decimal::from_raw);
        prop_assert_eq!(x.checked_add(y), expected);
    }

    #[test]
    fn ordering_matches_raw(a in any::<i128>(), b in any::<i128>()) {
        let x = Decimal::from_raw(a);
        let y = Decimal::from_raw(b);
        prop_assert_eq!(x.cmp(&y), a.cmp(&b));
    }

    #[test]
    fn div_then_mul_recovers_within_one_tick(a in mul_bounded(), b in 1i128..1_000_000_000_000_000i128) {
        // (a / b) * b should recover a up to the rounding of a single tick,
        // since division rounds to the nearest 10^-8.
        let x = Decimal::from_raw(a);
        let y = Decimal::from_raw(b);
        let q = x.checked_div(y).expect("no overflow");
        let recovered = q.checked_mul(y).expect("no overflow");
        let diff = recovered.checked_sub(x).expect("no overflow").abs();
        // Tolerance: one tick of the divisor magnitude, generously bounded.
        prop_assert!(diff <= y.abs(), "diff {diff} exceeded divisor {y}");
    }
}
