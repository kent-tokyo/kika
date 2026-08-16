//! Exact floating-point arithmetic core: error-free transformations and
//! nonoverlapping expansions, per Dekker (1971) and Shewchuk (1997). See
//! `docs/adr/ADR-001-numeric-robustness-strategy.md` for why this uses
//! only `+`, `-`, `*` (never `f64::mul_add`), and
//! `docs/numerical-model.md` for the exactness properties relied on here.

use super::sign::Sign;

/// Splitter for [`split`]: `2^27 + 1`, chosen so a 53-bit `f64` mantissa
/// splits into two ~26-bit halves whose pairwise products can be summed
/// without rounding.
const SPLITTER: f64 = 134_217_729.0; // 2^27 + 1

/// Exact sum: `hi = fl(a + b)`, and `hi + lo == a + b` exactly (as real
/// numbers), for any finite `a`, `b`. No magnitude ordering required.
/// (Knuth / Møller.)
#[inline]
pub(crate) fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let hi = a + b;
    let b_virtual = hi - a;
    let a_virtual = hi - b_virtual;
    let b_round = b - b_virtual;
    let a_round = a - a_virtual;
    let lo = a_round + b_round;
    (hi, lo)
}

/// Exact sum with the same postcondition as [`two_sum`], but requires
/// `|a| >= |b|`. Cheaper (3 flops vs. 6).
#[inline]
pub(crate) fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    debug_assert!(a.abs() >= b.abs() || a == 0.0 || b == 0.0);
    let hi = a + b;
    let b_virtual = hi - a;
    let lo = b - b_virtual;
    (hi, lo)
}

/// Splits `a` into two `f64` halves `(hi, lo)` with `hi + lo == a` exactly.
#[inline]
fn split(a: f64) -> (f64, f64) {
    let c = SPLITTER * a;
    let a_big = c - a;
    let hi = c - a_big;
    let lo = a - hi;
    (hi, lo)
}

/// Exact product: `hi = fl(a * b)`, and `hi + lo == a * b` exactly, for any
/// finite `a`, `b`. (Dekker 1971, via two [`split`] calls — deliberately
/// not FMA-based, see module docs.)
#[inline]
pub(crate) fn two_product(a: f64, b: f64) -> (f64, f64) {
    let hi = a * b;
    let (a_hi, a_lo) = split(a);
    let (b_hi, b_lo) = split(b);
    let err1 = hi - (a_hi * b_hi);
    let err2 = err1 - (a_lo * b_hi);
    let err3 = err2 - (a_hi * b_lo);
    let lo = (a_lo * b_lo) - err3;
    (hi, lo)
}

/// Adds a single `f64` `b` to a nonoverlapping expansion `e` (components
/// sorted by increasing magnitude), returning a new nonoverlapping
/// expansion of length `e.len() + 1` with the same exact sum.
///
/// Zero components are kept rather than elided, so the output length is
/// always exactly `e.len() + 1`; [`expansion_sign`] already skips zeros
/// when reading the result.
pub(crate) fn grow_expansion(e: &[f64], b: f64) -> Vec<f64> {
    let mut result = Vec::with_capacity(e.len() + 1);
    let mut q = b;
    for &e_i in e {
        let (sum, err) = two_sum(q, e_i);
        result.push(err);
        q = sum;
    }
    result.push(q);
    result
}

/// The two-component nonoverlapping expansion for the exact value `a * b`.
pub(crate) fn product_expansion(a: f64, b: f64) -> [f64; 2] {
    let (hi, lo) = two_product(a, b);
    [lo, hi]
}

/// Merges `addend`'s components into `base` (both nonoverlapping
/// expansions), returning a nonoverlapping expansion of length
/// `base.len() + addend.len()` with the sum of both.
///
/// This is the straightforward O(n*m) repeated-[`grow_expansion`] merge,
/// not Shewchuk's linear-time `fast-expansion-sum`.
///
/// ponytail: O(n*m) merge; the small, fixed-size expansions Phase 1's
/// determinants produce (at most a handful of components) make this
/// irrelevant in practice. Upgrade to a linear-time merge if profiling
/// ever shows expansion growth dominates (§13).
pub(crate) fn expansion_sum(base: &[f64], addend: &[f64]) -> Vec<f64> {
    let mut result = base.to_vec();
    for &b in addend {
        result = grow_expansion(&result, b);
    }
    result
}

/// The exact sign of a nonoverlapping expansion: the sign of its most
/// significant (last) nonzero component. See `docs/numerical-model.md`.
pub(crate) fn expansion_sign(e: &[f64]) -> Sign {
    for &value in e.iter().rev() {
        if value != 0.0 {
            return Sign::of_exact(value);
        }
    }
    Sign::Zero
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::{Signed, Zero};

    /// Converts a finite `f64` to an exact `BigRational`, independent of
    /// this module's own arithmetic — used only to check the primitives
    /// against ground truth, never in production code (ADR-005).
    fn exact(x: f64) -> BigRational {
        assert!(x.is_finite());
        if x == 0.0 {
            return BigRational::zero();
        }
        let bits = x.to_bits();
        let sign = if (bits >> 63) & 1 == 1 { -1 } else { 1 };
        let exponent_bits = ((bits >> 52) & 0x7ff) as i64;
        let mantissa_bits = bits & 0xf_ffff_ffff_ffff;
        let (mantissa, exponent) = if exponent_bits == 0 {
            (mantissa_bits, -1074i64)
        } else {
            (mantissa_bits | (1 << 52), exponent_bits - 1075)
        };
        let mantissa = BigInt::from(mantissa) * BigInt::from(sign);
        let mant_rat = BigRational::from_integer(mantissa);
        if exponent >= 0 {
            mant_rat * BigRational::from_integer(BigInt::from(2).pow(exponent as u32))
        } else {
            mant_rat / BigRational::from_integer(BigInt::from(2).pow((-exponent) as u32))
        }
    }

    fn expansion_exact_sum(e: &[f64]) -> BigRational {
        e.iter().fold(BigRational::zero(), |acc, &v| acc + exact(v))
    }

    /// Broad-range values for [`two_sum`]/[`fast_two_sum`], which are
    /// proven exact for *any* finite operands (no exponent-range
    /// restriction — unlike split-based [`two_product`], see below).
    fn sample_values() -> Vec<f64> {
        vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.1,
            1e-300,
            1e300,
            1e-16,
            core::f64::consts::PI,
            123456789.123456,
            f64::MIN_POSITIVE,
            f64::MIN_POSITIVE * 2.0,
            2.0_f64.powi(-1070), // subnormal
            -2.0_f64.powi(-1070),
        ]
    }

    /// Values safe for an all-pairs [`two_product`] exactness check.
    ///
    /// The exact-product-as-(hi,lo) representation has a hard floor: it
    /// needs the true rounding error of `hi = fl(a*b)` to itself be
    /// representable as a single `f64`, which (worked out and verified
    /// empirically, see module docs "Known limitation") requires
    /// `|a*b| >= 2^-968 ~= 1.7e-292`. Every pairwise product of these
    /// values stays far above that floor (worst case ~1e-200) and far
    /// below overflow.
    fn two_product_safe_values() -> Vec<f64> {
        vec![
            0.0,
            -0.0,
            1.0,
            -1.0,
            0.1,
            -0.1,
            1e-100,
            1e100,
            -1e-100,
            1e-16,
            core::f64::consts::PI,
            123456789.123456,
        ]
    }

    /// `(tiny-but-safe, ordinary-scale)` pairs, well inside the floor
    /// derived above. This is the realistic robustness scenario the
    /// project spec calls "subnormal付近" (near subnormal): a tiny
    /// coordinate *difference* multiplied by an ordinary-scale coordinate.
    /// Genuinely subnormal operands (magnitude below ~2.2e-308) are a
    /// separate, documented known limitation — see
    /// `two_product_true_subnormal_operand_does_not_panic` below.
    fn two_product_near_subnormal_pairs() -> Vec<(f64, f64)> {
        let mut pairs = vec![];
        for &s in &[1e-140_f64, -1e-140, 1e-120, -1e-120] {
            for &n in &[1.0, -1.0, 0.1, -0.1, core::f64::consts::PI, 2.0, 1e-10] {
                pairs.push((s, n));
            }
        }
        pairs
    }

    #[test]
    fn two_sum_is_exact() {
        let values = sample_values();
        for &a in &values {
            for &b in &values {
                let (hi, lo) = two_sum(a, b);
                let got = exact(hi) + exact(lo);
                let want = exact(a) + exact(b);
                assert_eq!(got, want, "two_sum({a}, {b}) exactness");
            }
        }
    }

    #[test]
    fn fast_two_sum_is_exact_when_ordered() {
        let values = sample_values();
        for &a in &values {
            for &b in &values {
                if a == 0.0 || b == 0.0 || a.abs() >= b.abs() {
                    let (hi, lo) = fast_two_sum(a, b);
                    let got = exact(hi) + exact(lo);
                    let want = exact(a) + exact(b);
                    assert_eq!(got, want, "fast_two_sum({a}, {b}) exactness");
                }
            }
        }
    }

    #[test]
    fn two_product_is_exact() {
        let values = two_product_safe_values();
        for &a in &values {
            for &b in &values {
                let (hi, lo) = two_product(a, b);
                let got = exact(hi) + exact(lo);
                let want = exact(a) * exact(b);
                assert_eq!(got, want, "two_product({a}, {b}) exactness");
            }
        }
    }

    #[test]
    fn two_product_near_subnormal_operands_stay_exact() {
        for (a, b) in two_product_near_subnormal_pairs() {
            let (hi, lo) = two_product(a, b);
            let got = exact(hi) + exact(lo);
            let want = exact(a) * exact(b);
            assert_eq!(got, want, "two_product({a}, {b}) exactness");
        }
    }

    /// Genuinely subnormal operands (magnitude below `f64::MIN_POSITIVE`,
    /// the smallest *normal* float) fall below the representability floor
    /// derived above. This is a fundamental limit of representing an
    /// exact product as two `f64`s, verified to affect a correctly-rounded
    /// FMA-based `two_product` identically (see `docs/numerical-model.md`)
    /// — not specific to this split-based implementation. So this test
    /// does not assert exactness for these operands, only the universal
    /// API contract: no panic and no NaN/Infinity from finite inputs.
    #[test]
    fn two_product_true_subnormal_operand_does_not_panic() {
        for &a in &[
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            2.0_f64.powi(-1030),
            -2.0_f64.powi(-1030),
        ] {
            for &b in &[1.0_f64, -1.0, 0.1, core::f64::consts::PI] {
                let (hi, lo) = two_product(a, b);
                assert!(
                    hi.is_finite() && lo.is_finite(),
                    "two_product({a}, {b}) produced non-finite output"
                );
            }
        }
    }

    #[test]
    fn grow_expansion_preserves_sum() {
        let values = sample_values();
        let mut e: Vec<f64> = vec![];
        for (i, &b) in values.iter().enumerate() {
            e = grow_expansion(&e, b);
            assert_eq!(e.len(), i + 1);
        }
        let expected: BigRational = values
            .iter()
            .fold(BigRational::zero(), |acc, &v| acc + exact(v));
        assert_eq!(expansion_exact_sum(&e), expected);
    }

    #[test]
    fn expansion_sum_merges_two_expansions() {
        let a = product_expansion(123.456, 789.012);
        let b = product_expansion(-0.001, 999999.999);
        let merged = expansion_sum(&a, &b);
        let expected = exact(123.456) * exact(789.012) + exact(-0.001) * exact(999999.999);
        assert_eq!(expansion_exact_sum(&merged), expected);
    }

    #[test]
    fn sign_matches_leading_term() {
        // Construct expansions with cancellation and verify expansion_sign
        // agrees with the exact sign of the true sum, for many random-ish
        // products built from the exact primitives (never independently
        // summed in f64).
        let pairs = [
            (1.0, 1.0),
            (-1.0, 1.0),
            (1e16, 1.0),
            (1e-16, 1.0),
            (123456789.123456, 0.1),
            (-123456789.123456, 0.1),
        ];
        let mut e: Vec<f64> = vec![];
        for &(a, b) in &pairs {
            e = expansion_sum(&e, &product_expansion(a, b));
            let exact_sum = expansion_exact_sum(&e);
            let want = if exact_sum.is_positive() {
                Sign::Positive
            } else if exact_sum.is_negative() {
                Sign::Negative
            } else {
                Sign::Zero
            };
            assert_eq!(
                expansion_sign(&e),
                want,
                "expansion {e:?} exact_sum {exact_sum}"
            );
        }
    }
}
