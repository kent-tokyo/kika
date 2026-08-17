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

/// The two-component nonoverlapping expansion for the exact value `a * b`.
pub(crate) fn product_expansion(a: f64, b: f64) -> [f64; 2] {
    let (hi, lo) = two_product(a, b);
    [lo, hi]
}

/// The two-component nonoverlapping expansion for the exact value `a - b`,
/// for any finite `a`, `b` — no exponent-range restriction, unlike
/// [`product_expansion`] (this is `two_sum(a, -b)`, and negation is
/// always exact).
pub(crate) fn diff_expansion(a: f64, b: f64) -> [f64; 2] {
    let (hi, lo) = two_sum(a, -b);
    [lo, hi]
}

/// Merges `base` and `addend` (both nonoverlapping expansions) into a
/// nonoverlapping expansion of length `base.len() + addend.len()` with
/// the sum of both, in O(base.len() + addend.len()) time: a standard
/// merge-by-magnitude of the two (already magnitude-sorted) inputs,
/// followed by a single cascading [`two_sum`] pass over the merged
/// sequence (Shewchuk's "linear-time expansion sum"). Two_sum's
/// correctness doesn't depend on processing order (§ `two_sum`'s own
/// doc), but the *nonoverlapping* postcondition — needed for
/// [`expansion_sign`]'s leading-term shortcut — does, which is why the
/// merge step (not just the cascade) matters.
///
/// An earlier version injected `addend` one component at a time (via
/// single-element cascading `two_sum`, each call linear in the growing
/// accumulator), which is effectively quadratic — and calling *this*
/// function repeatedly in a loop (as [`scale_expansion`]/
/// [`product_of_expansions`] must, to combine one small piece per input
/// component) compounds an O(n+m)-per-call cost into O(total²) overall
/// if each call's `base` is the ever-growing accumulator. That
/// combination pattern — not just this function's own complexity — is
/// what made `insphere`'s exact fallback take whole seconds per call
/// before the fix; see those two functions' docs for the other half of
/// the fix (balanced merging instead of a linear fold).
pub(crate) fn expansion_sum(base: &[f64], addend: &[f64]) -> Vec<f64> {
    if base.is_empty() {
        return addend.to_vec();
    }
    if addend.is_empty() {
        return base.to_vec();
    }

    let mut merged = Vec::with_capacity(base.len() + addend.len());
    let (mut i, mut j) = (0, 0);
    while i < base.len() && j < addend.len() {
        if base[i].abs() <= addend[j].abs() {
            merged.push(base[i]);
            i += 1;
        } else {
            merged.push(addend[j]);
            j += 1;
        }
    }
    merged.extend_from_slice(&base[i..]);
    merged.extend_from_slice(&addend[j..]);

    let mut result = Vec::with_capacity(merged.len());
    let mut q = merged[0];
    for &g in &merged[1..] {
        let (sum, err) = two_sum(q, g);
        result.push(err);
        q = sum;
    }
    result.push(q);
    result
}

/// Merges a list of nonoverlapping expansions (e.g. the per-component
/// pieces [`scale_expansion`]/[`product_of_expansions`] produce) into
/// one, combining pairwise in balanced binary-tree order rather than
/// left-to-right. Folding left-to-right into a single growing
/// accumulator costs O(count²) even with a linear-time [`expansion_sum`]
/// per step, because each step's cost is proportional to the
/// accumulator's *current* (ever-growing) size; halving the number of
/// pieces at each tree level instead gives O(total_size * log(count)).
pub(crate) fn merge_all(mut parts: Vec<Vec<f64>>) -> Vec<f64> {
    if parts.is_empty() {
        return vec![];
    }
    while parts.len() > 1 {
        let mut next = Vec::with_capacity(parts.len().div_ceil(2));
        let mut it = parts.into_iter();
        while let Some(a) = it.next() {
            match it.next() {
                Some(b) => next.push(expansion_sum(&a, &b)),
                None => next.push(a),
            }
        }
        parts = next;
    }
    parts.into_iter().next().unwrap_or_default()
}

/// The exact expansion for `e * s` (a nonoverlapping expansion times a
/// single `f64` scalar): distributes the multiplication over each
/// component of `e` via [`product_expansion`], then combines the results
/// with [`merge_all`] (balanced, not a linear fold — see its docs).
pub(crate) fn scale_expansion(e: &[f64], s: f64) -> Vec<f64> {
    merge_all(
        e.iter()
            .map(|&e_i| product_expansion(e_i, s).to_vec())
            .collect(),
    )
}

/// The exact expansion for `e * f` (two nonoverlapping expansions
/// multiplied together): distributes over each component of `f` via
/// [`scale_expansion`], then combines with [`merge_all`]. Needed wherever
/// a predicate's exact fallback must multiply two *derived* quantities
/// (e.g. two coordinate differences) rather than a raw input scalar —
/// see `docs/numerical-model.md` "Known limitation: exactness starts at
/// the original coordinates" for why this matters.
pub(crate) fn product_of_expansions(e: &[f64], f: &[f64]) -> Vec<f64> {
    merge_all(f.iter().map(|&f_i| scale_expansion(e, f_i)).collect())
}

/// The exact sign of a nonoverlapping expansion: the sign of its most
/// significant (last) nonzero component. See `docs/numerical-model.md`.
pub(crate) fn expansion_sign(e: &[f64]) -> Sign {
    for &value in e.iter().rev() {
        if value != 0.0 {
            return Sign::of(value);
        }
    }
    Sign::Zero
}

/// The expansion with every component's sign flipped — `-e`.
pub(crate) fn negate(e: &[f64]) -> Vec<f64> {
    e.iter().map(|v| -v).collect()
}

/// The 3x3 cofactor determinant of rows `p`, `q`, `r`
/// (`det[p; q; r]` expanded along row `p`), plus a **pre-cancellation**
/// magnitude bound suitable for an outer filter that multiplies this
/// result by another factor and sums several such terms — see
/// `orient3d`/`incircle`/`insphere`'s own `*_ERR_BOUND_FACTOR` doc
/// comments and `docs/numerical-model.md` "Known limitation (fixed):
/// filter bound must use pre-cancellation magnitudes" for why the
/// pre-cancellation (not post-cancellation) magnitude is the only sound
/// bound for a nested cofactor computation. Shared by all three
/// predicates: `orient3d`'s determinant (rows are raw coordinate diffs),
/// `incircle`'s (a 2D paraboloid lift — the third "row" is a squared-distance
/// term), and `insphere`'s four cofactors (a 3D paraboloid lift) are all
/// exactly this same 3x3 structure, just with different inputs for `p`,
/// `q`, `r`.
pub(crate) fn det3_with_precancel_bound(
    p: (f64, f64, f64),
    q: (f64, f64, f64),
    r: (f64, f64, f64),
) -> (f64, f64) {
    let qr_12 = q.1 * r.2;
    let qr_21 = q.2 * r.1;
    let qr_02 = q.0 * r.2;
    let qr_20 = q.2 * r.0;
    let qr_01 = q.0 * r.1;
    let qr_10 = q.1 * r.0;

    let value = p.0 * (qr_12 - qr_21) - p.1 * (qr_02 - qr_20) + p.2 * (qr_01 - qr_10);
    let precancel_bound = p.0.abs() * (qr_12.abs() + qr_21.abs())
        + p.1.abs() * (qr_02.abs() + qr_20.abs())
        + p.2.abs() * (qr_01.abs() + qr_10.abs());
    (value, precancel_bound)
}

/// The exact 3x3 cofactor determinant expansion of rows `p`, `q`, `r`
/// (each a triple of expansions), mirroring [`det3_with_precancel_bound`]
/// but built from [`product_of_expansions`]/[`expansion_sum`] throughout
/// — see that function's doc comment for which predicates share this.
pub(crate) fn det3_exact(
    p: (&[f64], &[f64], &[f64]),
    q: (&[f64], &[f64], &[f64]),
    r: (&[f64], &[f64], &[f64]),
) -> Vec<f64> {
    let qr_12 = product_of_expansions(q.1, r.2);
    let qr_21 = product_of_expansions(q.2, r.1);
    let qr_02 = product_of_expansions(q.0, r.2);
    let qr_20 = product_of_expansions(q.2, r.0);
    let qr_01 = product_of_expansions(q.0, r.1);
    let qr_10 = product_of_expansions(q.1, r.0);

    let term0 = product_of_expansions(p.0, &expansion_sum(&qr_12, &negate(&qr_21)));
    let term1 = product_of_expansions(p.1, &expansion_sum(&qr_02, &negate(&qr_20)));
    let term2 = product_of_expansions(p.2, &expansion_sum(&qr_01, &negate(&qr_10)));

    expansion_sum(&expansion_sum(&term0, &negate(&term1)), &term2)
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

    /// Broad-range values for [`two_sum`], proven exact for *any* finite
    /// operands (no exponent-range restriction — unlike split-based
    /// [`two_product`], see below).
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
    fn expansion_sum_single_element_injection_preserves_sum() {
        let values = sample_values();
        let mut e: Vec<f64> = vec![];
        for (i, &b) in values.iter().enumerate() {
            e = expansion_sum(&e, &[b]);
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
    fn scale_expansion_is_exact() {
        // A 2-term expansion (from product_expansion) times a scalar,
        // checked against the independent bigint oracle: this is the
        // building block orient3d/incircle/insphere use for triple/
        // quadruple products.
        let pair = product_expansion(123.456, -789.012);
        let pair_exact = exact(123.456) * exact(-789.012);
        for &s in &[2.0_f64, -3.5, 0.001, 1e10, 1e-10, core::f64::consts::PI] {
            let scaled = scale_expansion(&pair, s);
            let expected = pair_exact.clone() * exact(s);
            assert_eq!(
                expansion_exact_sum(&scaled),
                expected,
                "scale_expansion(pair, {s})"
            );
        }
    }

    #[test]
    fn diff_expansion_is_exact() {
        // Includes the specific lossy-subtraction pattern that exposed
        // the exact-fallback exactness bug: a large power-of-two magnitude
        // paired with a much smaller one, where fl(a-b) alone discards
        // information that diff_expansion must retain.
        let pairs = [
            (123.456, -789.012),
            (2.0_f64.powi(60), 1.0),
            (2.0_f64.powi(60), 3.0),
            (-2.0_f64.powi(55), 7.0),
            (0.0, 0.0),
            (5.0, 5.0),
        ];
        for (a, b) in pairs {
            let diff = diff_expansion(a, b);
            let expected = exact(a) - exact(b);
            assert_eq!(
                expansion_exact_sum(&diff),
                expected,
                "diff_expansion({a}, {b})"
            );
        }
    }

    #[test]
    fn product_of_expansions_is_exact() {
        // Multiplies two *derived* (diff) expansions together, the shape
        // orient2d_exact/orient3d_exact actually need.
        let e = diff_expansion(2.0_f64.powi(60), 1.0); // exact acx-style value
        let f = diff_expansion(3.0, 2.0_f64.powi(55)); // exact bcy-style value
        let product = product_of_expansions(&e, &f);
        let expected =
            (exact(2.0_f64.powi(60)) - exact(1.0)) * (exact(3.0) - exact(2.0_f64.powi(55)));
        assert_eq!(expansion_exact_sum(&product), expected);
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
