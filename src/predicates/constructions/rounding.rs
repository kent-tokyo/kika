use super::super::expansion::{expansion_sign, expansion_sum, negate, scale_expansion};
use super::super::sign::Sign;

/// The next representable `f64` strictly greater than `x` (standard
/// bit-pattern increment/decrement technique). `f64::next_up` covers this
/// but is not stable at this crate's MSRV (1.85; stabilized in 1.86).
/// `x` must be finite — the only inputs here are candidate quotients from
/// [`correctly_rounded_divide`], which are always finite by construction
/// (a finite `f64` division, refined by a measured, bounded number of
/// `f64` steps — see that function's doc comment).
pub(super) fn next_up(x: f64) -> f64 {
    if x == 0.0 {
        return f64::from_bits(1); // smallest positive subnormal
    }
    let bits = x.to_bits();
    f64::from_bits(if x > 0.0 { bits + 1 } else { bits - 1 })
}

pub(super) fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

/// The `f64` nearest to the exact value `num / denom` (both exact
/// expansions; `denom` nonzero — checked by every caller before invoking
/// this, not re-checked here), round-to-nearest-even on exact ties — the
/// same rounding rule IEEE-754 mandates for a single division.
///
/// Shared by every correctly-rounded construction in this module
/// (`line_intersection`, `circumcenter`) — each builds its own
/// numerator/denominator as exact expansions from its own geometry, then
/// calls this once per output coordinate. Originally implemented inside
/// `line_intersection.rs` alone; extracted here once a second construction
/// needed the identical rounding step, rather than duplicating ~50 lines
/// of nontrivial, already-tested logic (see `docs/adr/ADR-009-voronoi-geometry.md`'s
/// "`correctly_rounded_divide`: reuse or duplicate?").
///
/// Algorithm: start from an ordinary `f64` division of each expansion's
/// (inexact) sum as a good initial guess `q`, then compute the *exact*
/// residual `r = num - q*denom` via the expansion machinery. If `r` is
/// exactly zero, `q` is the exact answer. Otherwise `r`'s sign (combined
/// with `denom`'s sign) says which direction the true quotient lies from
/// `q`; comparing `|r|` against `|denom| * half_ulp` (half the distance
/// from `q` to its neighbor *in that direction* — asymmetric at power-of-two
/// boundaries, so computed separately per direction rather than as one
/// "the" ULP) says whether `q` already rounds correctly, must step to that
/// neighbor, or lands on an exact tie (resolved by round-to-even on `q`'s
/// mantissa LSB). Looping after a step handles the case where the initial
/// guess was more than one ULP off.
///
/// The loop is bounded at 8 iterations as a safety net, not a relied-on
/// assumption. Each caller measures its own worst case for its own
/// numerator/denominator shape (a cancellation-prone initial guess for one
/// construction's geometry doesn't necessarily predict another's) — see
/// `line_intersection`'s and `circumcenter`'s own
/// `divide_loop_iteration_bound_is_generous`-style tests. The
/// exhausted-loop fallback returns the last `q` without re-verifying it;
/// that is only acceptable while every caller's measured margin stays
/// comfortable, so if any measurement ever creeps toward 8 the bound (or
/// the algorithm) needs revisiting, not just re-trusting.
pub(super) fn correctly_rounded_divide(num: &[f64], denom: &[f64]) -> f64 {
    let denom_sign = expansion_sign(denom);
    let mut q: f64 = num.iter().sum::<f64>() / denom.iter().sum::<f64>();

    for iter in 0..8u32 {
        let r = expansion_sum(num, &negate(&scale_expansion(denom, q)));
        let r_sign = expansion_sign(&r);
        if r_sign == Sign::Zero {
            record_iters(iter);
            return q;
        }

        let quotient_positive = r_sign == denom_sign;
        let neighbor = if quotient_positive {
            next_up(q)
        } else {
            next_down(q)
        };
        let half_ulp = 0.5 * (neighbor - q).abs();

        let r_abs = if r_sign == Sign::Positive {
            r
        } else {
            negate(&r)
        };
        let denom_abs = if denom_sign == Sign::Positive {
            denom.to_vec()
        } else {
            negate(denom)
        };
        let threshold = scale_expansion(&denom_abs, half_ulp);
        match expansion_sign(&expansion_sum(&r_abs, &negate(&threshold))) {
            Sign::Negative => {
                record_iters(iter);
                return q;
            }
            Sign::Positive => q = neighbor,
            Sign::Zero => {
                record_iters(iter);
                let q_is_even = (q.to_bits() & 1) == 0;
                return if q_is_even { q } else { neighbor };
            }
        }
    }
    record_iters(8);
    q
}

/// Test-only hook recording the worst-case iteration count
/// `correctly_rounded_divide` has actually needed, so each caller's `0..8`
/// bound is a measured safety margin rather than an assumed one. Compiled
/// out entirely in non-test builds. Each `#[test]` runs on its own thread
/// by default under Rust's test harness, so this `thread_local` naturally
/// isolates measurements per test — a `line_intersection` test and a
/// `circumcenter` test measuring concurrently never interfere.
#[cfg(test)]
pub(super) fn record_iters(n: u32) {
    MAX_DIVIDE_ITERS.with(|c| c.set(c.get().max(n)));
}

#[cfg(not(test))]
#[inline(always)]
fn record_iters(_n: u32) {}

#[cfg(test)]
thread_local! {
    pub(super) static MAX_DIVIDE_ITERS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::super::super::expansion::two_sum;
    use super::*;

    #[test]
    fn next_up_down_basic() {
        assert!(next_up(1.0) > 1.0);
        assert_eq!(next_down(next_up(1.0)), 1.0);
        assert_eq!(next_up(0.0), f64::from_bits(1));
        assert_eq!(next_up(-0.0), f64::from_bits(1));
        assert_eq!(next_down(0.0), -f64::from_bits(1));
        assert!(next_up(-1.0) > -1.0);
        assert_eq!(next_down(next_up(-1.0)), -1.0);
    }

    /// Deliberately constructs an exact tie for `correctly_rounded_divide`
    /// — the true quotient exactly halfway between `q0` and one neighbor —
    /// by building `num` as the exact expansion for `(q0 + neighbor) / 2`
    /// (via `two_sum`, exact; then `scale_expansion` by `0.5`, exact since
    /// halving never rounds) and `denom = [1.0]`, so `num/denom` equals
    /// that midpoint exactly, not merely approximately. Covers both
    /// round-to-even directions and a power-of-two boundary (`4.0`), where
    /// the ULP below and above differ — the exact case
    /// `correctly_rounded_divide`'s half-ULP-per-direction logic exists
    /// for.
    fn assert_exact_tie_rounds_to_even(q0: f64) {
        for neighbor in [next_up(q0), next_down(q0)] {
            let (hi, lo) = two_sum(q0, neighbor);
            let midpoint = scale_expansion(&[lo, hi], 0.5);
            let got = correctly_rounded_divide(&midpoint, &[1.0]);
            let expected = if (q0.to_bits() & 1) == 0 {
                q0
            } else {
                neighbor
            };
            assert_eq!(
                got, expected,
                "tie between {q0} and {neighbor} should round to the even one"
            );
        }
    }

    #[test]
    fn exact_tie_rounds_to_even_normal_range() {
        // 3.0's mantissa LSB is 1 (odd); its neighbors are both "more even"
        // in one direction or the other -- exercises both tie directions.
        assert_exact_tie_rounds_to_even(3.0);
    }

    #[test]
    fn exact_tie_rounds_to_even_at_power_of_two_boundary() {
        // 4.0 sits at a binade boundary: ulp above (2^-50) is double ulp
        // below (2^-51). half_ulp_above/half_ulp_below must be computed
        // per-direction, not as one shared "the" ULP -- this is exactly
        // the asymmetry that would silently misround if they weren't.
        assert_exact_tie_rounds_to_even(4.0);
    }
}
