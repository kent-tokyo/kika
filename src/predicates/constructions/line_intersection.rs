use super::super::expansion::{
    expansion_sign, expansion_sum, product_of_expansions, scale_expansion,
};
use super::super::sign::Sign;
use crate::primitives::Point2;

/// The intersection point of line `AB` and line `CD`, **correctly rounded**
/// to the nearest representable `f64` (round-to-nearest-even on exact
/// ties) — ADR-004's exact/certified construction model, ending Phase 2's
/// `Proper`-crossing exactness gap.
///
/// "Correctly rounded" means: compute the true, infinite-precision
/// intersection coordinate, then return whichever `f64` is nearest to it —
/// the same guarantee IEEE-754 makes for a single arithmetic operation like
/// `a + b`, but here for a whole geometric construction. This is the
/// strongest guarantee possible while keeping [`Point2`] a plain `f64`
/// pair (ADR-004 chose this over a new exact-rational point type as the
/// conservative, backward-compatible option — see the ADR).
///
/// # Precondition
///
/// The two lines must be non-parallel. Not checked here — callers
/// establish this via a predicate first (e.g. `segment_intersection`'s
/// `Proper` classification, which guarantees it: opposite-sign straddling
/// on both sides is impossible for parallel lines).
///
/// # Derivation
///
/// Parametrize line `AB` as `P(t) = A + t(B - A)`. Let `d1 = orient2d(C, D,
/// A)` and `d2 = orient2d(C, D, B)` — the signed "distance" (twice signed
/// area) from `A` and `B` to line `CD`. `d(t) = d1 + t(d2 - d1)` is linear
/// in `t` (orient2d is affine in its third argument for fixed `C, D`), so
/// `d(t) = 0` (crossing line `CD`) at `t = d1 / (d1 - d2)`. Substituting:
///
/// ```text
/// P(t) = A + [d1/(d1-d2)] (B - A) = [d1*B - A*d2] / (d1 - d2)
/// ```
///
/// (the `A*d1` terms cancel). `d1`, `d2`, and hence the numerator and
/// denominator, are computed as *exact* expansions via the same
/// `orient2d`-exact-fallback machinery `orient2d` itself uses — see
/// `orient2d_expansion`. The division by `(d1 - d2)` is the one step that
/// cannot stay exact (the true quotient is generally irrational relative
/// to `f64`), so it is the one place `correctly_rounded_divide` is used.
///
/// # Known limitation: a verified-safe magnitude range, measured not assumed
///
/// This construction is effectively degree-3 in the input coordinates
/// (`d1`/`d2` are degree-2 cross products, scaled once more by a
/// coordinate) — *lower* degree than `incircle`'s degree-4 paraboloid-lift
/// determinant, so it was not obvious ahead of time which would have the
/// wider verified-safe magnitude range. Measured (not assumed) via
/// `tests/differential/line_intersection.rs`'s magnitude sweep against an
/// independent correctly-rounded-nearest-`f64` oracle: no failure observed
/// down through `2^-335` (`~1.4e-101`, 20 random crossings sampled per
/// exponent step) — comfortably *wider* than `incircle`'s documented
/// `~1e-70..1e70` (`docs/numerical-model.md`) — confirming
/// degree, not "is it a construction vs. a predicate", is what actually
/// governs the floor. Below the measured boundary,
/// `product_expansion`'s representability floor (`docs/numerical-model.md`
/// "exact-product representability floor") can be crossed, silently
/// degrading the "exact" claim — accepted and documented, not solved
/// (matches `incircle`/`insphere` precedent), pending Shewchuk-style
/// multi-tier adaptive precision (`tasks/todo.md` backlog). The large-
/// magnitude side is not independently swept here — see
/// `random_crossings_multiple_scales`'s `1e100` case for the largest
/// scale exercised.
pub(crate) fn line_intersection(a: Point2, b: Point2, c: Point2, d: Point2) -> Point2 {
    let d1 = orient2d_expansion(c, d, a);
    let d2 = orient2d_expansion(c, d, b);

    let denom = expansion_sum(&d1, &negate(&d2));
    let num_x = expansion_sum(
        &scale_expansion(&d1, b.x()),
        &negate(&scale_expansion(&d2, a.x())),
    );
    let num_y = expansion_sum(
        &scale_expansion(&d1, b.y()),
        &negate(&scale_expansion(&d2, a.y())),
    );

    let x = correctly_rounded_divide(&num_x, &denom);
    let y = correctly_rounded_divide(&num_y, &denom);
    Point2::new_unchecked(x, y)
}

/// The exact expansion for `orient2d(p, q, r)`'s determinant — the same
/// computation as `orient2d::orient2d_exact`, but returning the full
/// expansion instead of collapsing it to a `Sign`, since the construction
/// above needs the *value* to build a numerator/denominator from.
/// Duplicated rather than shared with `orient2d_exact` (which is private
/// to its own module and returns only a `Sign`) — a few lines, not worth
/// changing that module's return type for.
fn orient2d_expansion(p: Point2, q: Point2, r: Point2) -> Vec<f64> {
    use super::super::expansion::diff_expansion;
    let prx = diff_expansion(p.x(), r.x());
    let pry = diff_expansion(p.y(), r.y());
    let qrx = diff_expansion(q.x(), r.x());
    let qry = diff_expansion(q.y(), r.y());

    let left = product_of_expansions(&prx, &qry);
    let right = product_of_expansions(&pry, &qrx);
    expansion_sum(&left, &negate(&right))
}

fn negate(e: &[f64]) -> Vec<f64> {
    e.iter().map(|v| -v).collect()
}

/// The next representable `f64` strictly greater than `x` (standard
/// bit-pattern increment/decrement technique). `f64::next_up` covers this
/// but is not stable at this crate's MSRV (1.85; stabilized in 1.86).
/// `x` must be finite — the only inputs here are candidate quotients from
/// [`correctly_rounded_divide`], which are always finite by construction
/// (a finite `f64` division, refined by a measured 0-2 `f64` steps — see
/// that function's doc comment).
fn next_up(x: f64) -> f64 {
    if x == 0.0 {
        return f64::from_bits(1); // smallest positive subnormal
    }
    let bits = x.to_bits();
    f64::from_bits(if x > 0.0 { bits + 1 } else { bits - 1 })
}

fn next_down(x: f64) -> f64 {
    -next_up(-x)
}

/// The `f64` nearest to the exact value `num / denom` (both exact
/// expansions; `denom` nonzero), round-to-nearest-even on exact ties —
/// the same rounding rule IEEE-754 mandates for a single division.
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
/// neighbor, or is an exact tie (round to even). Looping after a step
/// handles the case where the initial guess was more than one ULP off.
///
/// The loop is bounded at 8 iterations as a safety net, not a relied-on
/// assumption: `divide_loop_iteration_bound_is_generous` measures the
/// actual worst case (ordinary random crossings plus deliberately
/// near-parallel ones, across magnitude scales from `1e-300` to `1e100`,
/// where the plain-`f64` initial guess is most exposed to catastrophic
/// cancellation) at 2 iterations — 4x below the bound. The
/// exhausted-loop fallback returns the last `q` without re-verifying it;
/// that is only acceptable while the measured margin stays comfortable, so
/// if it ever creeps toward 8 the bound (or the algorithm) needs
/// revisiting, not just re-trusting.
fn correctly_rounded_divide(num: &[f64], denom: &[f64]) -> f64 {
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
/// `correctly_rounded_divide` has actually needed, so the `0..8` bound above
/// is a measured safety margin rather than an assumed one — see
/// `divide_loop_iteration_bound_is_generous`. Compiled out entirely in
/// non-test builds.
#[cfg(test)]
fn record_iters(n: u32) {
    MAX_DIVIDE_ITERS.with(|c| c.set(c.get().max(n)));
}

#[cfg(not(test))]
#[inline(always)]
fn record_iters(_n: u32) {}

#[cfg(test)]
thread_local! {
    static MAX_DIVIDE_ITERS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

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
        use super::super::super::expansion::two_sum;

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
        // per-direction, not as one shared "the ULP" -- this is exactly
        // the asymmetry that would silently misround if they weren't.
        assert_exact_tie_rounds_to_even(4.0);
    }

    #[test]
    fn simple_crossing_matches_naive_computation() {
        let (a, b, c, d) = (p(0.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(4.0, 0.0));
        assert_eq!(line_intersection(a, b, c, d), p(2.0, 2.0));
    }

    #[test]
    fn exact_half_integer_result() {
        let (a, b, c, d) = (p(0.0, 0.0), p(3.0, 3.0), p(0.0, 3.0), p(3.0, 0.0));
        assert_eq!(line_intersection(a, b, c, d), p(1.5, 1.5));
    }

    struct Xorshift64(u64);
    impl Xorshift64 {
        fn next_u64(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn next_f64_in(&mut self, scale: f64) -> f64 {
            let bits = self.next_u64();
            let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
            (unit * 2.0 - 1.0) * scale
        }
    }

    /// Measures (doesn't assume) `correctly_rounded_divide`'s worst-case
    /// iteration count, including the case most likely to defeat a "1-2 ULP"
    /// initial guess: near-parallel lines, where `d1 - d2` is a small
    /// difference of close values (catastrophic cancellation in the plain
    /// `f64` sum used to seed `q`). Random ordinary crossings plus a sweep of
    /// near-parallel angles, both across several magnitude scales. If this
    /// ever creeps toward the `0..8` bound, the bound (or the algorithm) needs
    /// revisiting — see the doc comment on `correctly_rounded_divide`.
    #[test]
    fn divide_loop_iteration_bound_is_generous() {
        MAX_DIVIDE_ITERS.with(|c| c.set(0));
        let mut rng = Xorshift64(0x51ED_270D_1727_1A9B);

        for &scale in &[1.0_f64, 1e-6, 1e6, 1e30, 1e-30, 1e100, 1e-80, 1e-300] {
            // Ordinary random crossings.
            for _ in 0..200 {
                let cx = rng.next_f64_in(scale);
                let cy = rng.next_f64_in(scale);
                let r = scale.max(f64::MIN_POSITIVE) * (0.1 + rng.next_f64_in(1.0).abs());
                let a = p(cx - r, cy - r * (0.3 + rng.next_f64_in(1.0).abs()));
                let b = p(cx + r, cy + r * (0.3 + rng.next_f64_in(1.0).abs()));
                let c = p(cx - r * (0.3 + rng.next_f64_in(1.0).abs()), cy + r);
                let d = p(cx + r * (0.3 + rng.next_f64_in(1.0).abs()), cy - r);
                line_intersection(a, b, c, d);
            }
            // Near-parallel crossings: CD tilted by a tiny angle off AB, so
            // d1 and d2 are close in magnitude and `d1 - d2` is a small
            // difference of close values -- the case most likely to make the
            // plain-f64 initial guess land more than 1 ULP off.
            for k in 1..=200i32 {
                let tiny = scale.max(1.0) * 2.0_f64.powi(-k.min(60));
                let a = p(-scale, 0.0);
                let b = p(scale, 0.0);
                let c = p(-scale, tiny);
                let d = p(
                    scale,
                    -tiny + rng.next_f64_in(tiny.abs().max(f64::MIN_POSITIVE) * 1e-3),
                );
                if a == b || c == d {
                    continue;
                }
                line_intersection(a, b, c, d);
            }
        }

        let max_iters = MAX_DIVIDE_ITERS.with(|c| c.get());
        eprintln!("correctly_rounded_divide: measured max iterations = {max_iters}");
        assert!(
            max_iters <= 3,
            "correctly_rounded_divide used {max_iters} iterations on some input \
             -- the 0..8 bound's assumed headroom is thinner than measured, \
             investigate before trusting it"
        );
    }
}
