//! Not yet called by anything (that lands in the next commit, wiring it
//! into `Voronoi2::vertex_point`/`edge_geometry` per ADR-009) --
//! `#![allow(dead_code)]` deliberately, matching ADR-007 Phase 7A's own
//! precedent for landing internal construction machinery ahead of its
//! public consumer as its own reviewable, independently-tested step.

#![allow(dead_code)]

use super::super::expansion::{
    diff_expansion, expansion_sign, expansion_sum, negate, product_of_expansions, scale_expansion,
};
use super::super::sign::Sign;
use super::rounding::correctly_rounded_divide;
use crate::primitives::Point2;

/// The circumcenter of triangle `(a, b, c)`, **correctly rounded** to the
/// nearest representable `f64` (round-to-nearest-even on exact ties) —
/// the same "float+certificate" guarantee `line_intersection` established
/// (ADR-004 Phase 5), applied to a different construction
/// (`docs/adr/ADR-009-voronoi-geometry.md`).
///
/// Returns `None`, never a panic and never a silently wrong
/// large-magnitude coordinate, when the true circumcenter is not a finite
/// `f64` — either because `a`, `b`, `c` are exactly collinear (no
/// circumcircle exists at all), or because the triangle is thin enough
/// that its true (finite, well-defined) circumradius overflows `f64`'s
/// representable range. This differs fundamentally from
/// `line_intersection`'s failure mode: a line intersection's coordinate is
/// bounded by the input points' own magnitude, so power-of-two rescaling
/// alone eliminates its overflow case. A circumcenter is not bounded that
/// way — three points spanning a small, fixed region can still be made
/// arbitrarily close to collinear, driving the circumradius arbitrarily
/// large independent of input magnitude — so rescaling (still applied
/// below, for the separate large-*input*-magnitude case it does solve)
/// cannot by itself guarantee a finite result; see the ADR's
/// "`VoronoiGeometryError`: why it's needed here and wasn't needed by
/// `line_intersection`" section.
///
/// # Derivation
///
/// Translate to vertex `a`'s frame: `dx1 = b.x-a.x`, `dy1 = b.y-a.y`,
/// `dx2 = c.x-a.x`, `dy2 = c.y-a.y`. Then (standard circumcenter formula):
///
/// ```text
/// d  = 2 * (dx1*dy2 - dy1*dx2)     -- note: d/2 is exactly orient2d(a,b,c)'s
///                                  -- own determinant value, same shape
/// ux = (dy2*(dx1^2+dy1^2) - dy1*(dx2^2+dy2^2)) / d
/// uy = (dx1*(dx2^2+dy2^2) - dx2*(dx1^2+dy1^2)) / d
/// circumcenter = (a.x + ux, a.y + uy)
/// ```
///
/// Rounding `ux`/`uy` and then adding to `a.x()`/`a.y()` in plain `f64`
/// would round *twice*. Following `line_intersection`'s own derivation
/// (its "the `A*d1` terms cancel" trick), `a`'s coordinate is folded into
/// one shared numerator per axis before dividing once:
///
/// ```text
/// circumcenter.x = [a.x*d + dy2*(dx1^2+dy1^2) - dy1*(dx2^2+dy2^2)] / d
/// circumcenter.y = [a.y*d + dx1*(dx2^2+dy2^2) - dx2*(dx1^2+dy1^2)] / d
/// ```
///
/// `d` is a degree-2 exact expansion (built the same way `orient2d`'s own
/// exact fallback builds its determinant — `diff_expansion` from the
/// *original* coordinates, never a once-rounded `f64` difference, then
/// `product_of_expansions`). Each numerator is degree 3 — the same degree
/// as `line_intersection`'s own numerator, built the same way (squared
/// terms via `product_of_expansions(dx, dx)`, never a squared rounded
/// `f64` — the "exactness starts at the original coordinates" lesson
/// `docs/numerical-model.md` generalized from `orient2d`/`orient3d` to
/// `incircle`/`insphere`). One [`correctly_rounded_divide`] call per
/// output coordinate.
///
/// # `d` can be exactly zero
///
/// A genuine Delaunay face has `orient2d(a,b,c) != Sign::Zero` — but that
/// is an invariant this function's caller is *supposed* to maintain, not
/// one to trust blindly (matching `docs/adr/ADR-008-point-location.md`'s
/// posture: `Triangulation2::validate_topology()` is a test-only
/// diagnostic, never a construction-time gate). `expansion_sign(&d)` is
/// checked explicitly before dividing; `Sign::Zero` returns `None` rather
/// than dividing by an exact zero.
///
/// # Magnitude range
///
/// Same rescale-by-power-of-two guard `line_intersection` uses for large
/// *input* coordinates (an exact, lossless operation — an exponent shift,
/// no rounding). This does not rescue the near-degenerate (thin-triangle)
/// overflow case described above; only the explicit finiteness check
/// after scaling back does. See `docs/numerical-model.md` (to be updated
/// once this construction's own magnitude range is measured, per the
/// ADR's "Assumptions to prove" list) for the expected `line_intersection`-like
/// (not `incircle`-like) range, given the matching degree-3/degree-2
/// shape.
pub(crate) fn circumcenter(a: Point2, b: Point2, c: Point2) -> Option<Point2> {
    let max_coord = [a.x(), a.y(), b.x(), b.y(), c.x(), c.y()]
        .into_iter()
        .fold(0.0_f64, |acc, v| acc.max(v.abs()));

    if max_coord <= RESCALE_THRESHOLD {
        return circumcenter_raw(a, b, c);
    }

    let k = max_coord.log2().ceil() as i32;
    let s = 2f64.powi(-k);
    let scale = |p: Point2| Point2::new_unchecked(p.x() * s, p.y() * s);
    let scaled = circumcenter_raw(scale(a), scale(b), scale(c))?;
    let inv_s = 2f64.powi(k);
    let x = scaled.x() * inv_s;
    let y = scaled.y() * inv_s;
    if x.is_finite() && y.is_finite() {
        Some(Point2::new_unchecked(x, y))
    } else {
        None
    }
}

/// Coordinate-magnitude threshold above which [`circumcenter`] rescales
/// its inputs. Same value and reasoning as `line_intersection`'s own
/// `RESCALE_THRESHOLD`: this construction has the same degree-3-numerator
/// shape, so `(1e90)^3 = 1e270` stays well under `f64::MAX` for the
/// worst-case intermediate product here too.
const RESCALE_THRESHOLD: f64 = 1e90;

fn circumcenter_raw(a: Point2, b: Point2, c: Point2) -> Option<Point2> {
    let dx1 = diff_expansion(b.x(), a.x());
    let dy1 = diff_expansion(b.y(), a.y());
    let dx2 = diff_expansion(c.x(), a.x());
    let dy2 = diff_expansion(c.y(), a.y());

    let dx1_dy2 = product_of_expansions(&dx1, &dy2);
    let dy1_dx2 = product_of_expansions(&dy1, &dx2);
    let half_d = expansion_sum(&dx1_dy2, &negate(&dy1_dx2));
    let d = scale_expansion(&half_d, 2.0); // exact: doubling never rounds

    if expansion_sign(&d) == Sign::Zero {
        return None;
    }

    let sq1 = expansion_sum(
        &product_of_expansions(&dx1, &dx1),
        &product_of_expansions(&dy1, &dy1),
    );
    let sq2 = expansion_sum(
        &product_of_expansions(&dx2, &dx2),
        &product_of_expansions(&dy2, &dy2),
    );

    let num_x = expansion_sum(
        &expansion_sum(
            &scale_expansion(&d, a.x()),
            &product_of_expansions(&dy2, &sq1),
        ),
        &negate(&product_of_expansions(&dy1, &sq2)),
    );
    let num_y = expansion_sum(
        &expansion_sum(
            &scale_expansion(&d, a.y()),
            &product_of_expansions(&dx1, &sq2),
        ),
        &negate(&product_of_expansions(&dx2, &sq1)),
    );

    let x = correctly_rounded_divide(&num_x, &d);
    let y = correctly_rounded_divide(&num_y, &d);

    if x.is_finite() && y.is_finite() {
        Some(Point2::new_unchecked(x, y))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::super::rounding::MAX_DIVIDE_ITERS;
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn right_triangle_circumcenter_is_hypotenuse_midpoint() {
        // Circumcenter of a right triangle is the midpoint of its
        // hypotenuse -- a standard, independently-known fact, not derived
        // from this module's own formula.
        assert_eq!(
            circumcenter(p(0.0, 0.0), p(2.0, 0.0), p(0.0, 2.0)),
            Some(p(1.0, 1.0))
        );
        assert_eq!(
            circumcenter(p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)),
            Some(p(2.0, 2.0))
        );
    }

    #[test]
    fn equilateral_triangle_circumcenter_matches_hand_computation() {
        // Equilateral triangle side 2, centered so the hand-computed
        // circumcenter is a "nice" number: vertices (0,0), (2,0),
        // (1, sqrt(3)) -- circumradius = 2/sqrt(3), circumcenter =
        // (1, 1/sqrt(3)).
        let sqrt3 = 3.0_f64.sqrt();
        let got = circumcenter(p(0.0, 0.0), p(2.0, 0.0), p(1.0, sqrt3)).unwrap();
        assert!((got.x() - 1.0).abs() < 1e-12);
        assert!((got.y() - 1.0 / sqrt3).abs() < 1e-12);
    }

    #[test]
    fn order_independent_up_to_which_vertex_is_a() {
        // The circumcenter is a property of the triangle, not of which
        // vertex plays "a" in the translated-frame derivation -- all 3
        // rotations must agree.
        let (a, b, c) = (p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0));
        let v1 = circumcenter(a, b, c);
        let v2 = circumcenter(b, c, a);
        let v3 = circumcenter(c, a, b);
        assert_eq!(v1, v2);
        assert_eq!(v2, v3);
    }

    #[test]
    fn exactly_collinear_points_return_none() {
        assert_eq!(circumcenter(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)), None);
        assert_eq!(circumcenter(p(1.0, 1.0), p(2.0, 2.0), p(5.0, 5.0)), None);
        // Duplicate point is a degenerate (zero-area) "triangle", same as
        // collinear.
        assert_eq!(circumcenter(p(1.0, 1.0), p(1.0, 1.0), p(2.0, 3.0)), None);
    }

    /// A concrete, reproducible thin-triangle overflow case, chosen (per
    /// `docs/adr/ADR-009-voronoi-geometry.md`'s assumption-to-prove #6) to
    /// sit comfortably *above* the `~1.7e-292` exact-product
    /// representability floor documented in `docs/numerical-model.md` --
    /// unlike a subnormal-`eps`-based fixture (tried first while writing
    /// this test; rejected once `orient2d` itself reported `Collinear` on
    /// it, confirming that regime is below the floor for *every*
    /// predicate in this crate, not a circumcenter-specific issue): a
    /// wide (`L=1e75`), extremely flat (`h=1e-170`) isoceles triangle.
    /// `d = 2*L*h ~ 2e-95` and every intermediate product involved stay
    /// far above the representability floor, so `orient2d(a,b,c)` (and
    /// `delaunay2`) correctly recognize this as a genuine, non-degenerate
    /// triangle -- confirmed below, not assumed -- while the true
    /// circumcenter's `y` coordinate (`~ -L^3/(4d) ~ -1.25e300`... and
    /// beyond, past `f64::MAX`) genuinely overflows. Empirically swept
    /// (a handful of `L`/`h` combinations, see this ADR's own record) to
    /// find a value pair that overflows cleanly rather than landing near
    /// the transition boundary.
    #[test]
    fn thin_triangle_overflow_returns_none_not_a_panic() {
        let l = 1e75;
        let h = 1e-170;
        let a = p(0.0, 0.0);
        let b = p(l, 0.0);
        let c = p(l / 2.0, h);
        assert_eq!(
            crate::predicates::orient2d(a, b, c),
            crate::predicates::Orientation::CounterClockwise,
            "fixture must be a genuine, non-degenerate triangle, not an accidentally-collinear one"
        );
        assert_eq!(circumcenter(a, b, c), None);
    }

    /// Sanity check that thin-but-*not*-overflowing triangles still work:
    /// as the triangle gets thinner, the circumcenter moves further away
    /// but stays finite and, once it stops moving detectably at `f64`
    /// precision, stable.
    #[test]
    fn thin_but_representable_triangle_is_some_and_finite() {
        for exp in [1, 10, 50, 100, 200, 250] {
            let eps = 2f64.powi(-exp);
            let got = circumcenter(p(0.0, 0.0), p(1.0, 0.0), p(0.5, eps));
            assert!(
                got.is_some(),
                "expected Some at eps=2^-{exp}, got None (overflowed sooner than expected)"
            );
            let got = got.unwrap();
            assert!(got.x().is_finite() && got.y().is_finite());
        }
    }

    /// Verifies the overflow-ceiling analysis in this module's doc
    /// comment: sweeps *uniform* coordinate magnitude, mirroring
    /// `line_intersection`'s own `extreme_uniform_magnitude_is_finite`.
    #[test]
    fn extreme_uniform_magnitude_is_finite_or_none_not_a_panic() {
        for exp in [90, 100, 120, 150, 200, 250, 300] {
            let scale = 10f64.powi(exp);
            let got = circumcenter(p(0.0, 0.0), p(scale, 0.0), p(0.0, scale));
            if let Some(pt) = got {
                assert!(
                    pt.x().is_finite() && pt.y().is_finite(),
                    "circumcenter produced a non-finite Point2 at scale 1e{exp} -- should be None instead"
                );
            }
            // None is an acceptable outcome at large scale (this
            // particular triangle's circumradius grows with scale too) --
            // the point of this test is "never panics, never a bad
            // Point2", not "always succeeds at any scale".
        }
    }

    /// Measures (doesn't assume) `correctly_rounded_divide`'s worst-case
    /// iteration count for *this* construction's own numerator shape --
    /// `line_intersection`'s own measured "2 iterations" does not
    /// transfer automatically (see `docs/adr/ADR-009-voronoi-geometry.md`).
    /// Includes the cancellation family that construction's own sweep
    /// doesn't cover: a circumcenter near the origin with vertices far
    /// from it, where `a.x*d` and the offset terms are large and nearly
    /// cancel, stressing the plain-`f64` initial guess the same way
    /// `line_intersection`'s near-parallel-lines sweep stressed *its*
    /// seed.
    #[test]
    fn divide_loop_iteration_bound_is_generous() {
        MAX_DIVIDE_ITERS.with(|c| c.set(0));
        let mut rng = Xorshift64(0x1EE7_C1CC_1E77_5EED_u64);

        // Ordinary random triangles across several magnitude scales.
        for &scale in &[1.0_f64, 1e-6, 1e6, 1e30, 1e-30, 1e60, 1e-60] {
            for _ in 0..300 {
                let cx = rng.next_f64_in(scale);
                let cy = rng.next_f64_in(scale);
                let r = scale.max(f64::MIN_POSITIVE) * (0.1 + rng.next_f64_in(1.0).abs());
                let a = p(cx - r, cy - r * (0.3 + rng.next_f64_in(1.0).abs()));
                let b = p(cx + r, cy + r * (0.3 + rng.next_f64_in(1.0).abs()));
                let c = p(cx + r * rng.next_f64_in(1.0), cy + r);
                if is_nondegenerate(a, b, c) {
                    circumcenter(a, b, c);
                }
            }
        }

        // Origin-centered circumcenter, vertices far away: a triangle
        // inscribed in a circle centered at (0,0) with large radius --
        // `a.x*d` and the squared-offset terms are all large and nearly
        // cancel against each other in the numerator's plain-f64 sum,
        // the cancellation mode `line_intersection`'s own sweep never
        // exercises.
        for &radius in &[1e3, 1e6, 1e10, 1e20, 1e40, 1e60, 1e80] {
            for i in 0..300u32 {
                let theta_a = (i as f64) * 0.618_033_988_75 * std::f64::consts::TAU;
                let theta_b = theta_a + 1.9 + rng.next_f64_in(0.3);
                let theta_c = theta_a + 4.0 + rng.next_f64_in(0.3);
                let a = p(radius * theta_a.cos(), radius * theta_a.sin());
                let b = p(radius * theta_b.cos(), radius * theta_b.sin());
                let c = p(radius * theta_c.cos(), radius * theta_c.sin());
                if is_nondegenerate(a, b, c) {
                    circumcenter(a, b, c);
                }
            }
        }

        let max_iters = MAX_DIVIDE_ITERS.with(|c| c.get());
        eprintln!("circumcenter: measured max correctly_rounded_divide iterations = {max_iters}");
        // Measured at 2 -- matching line_intersection's own measured worst
        // case exactly, consistent with both constructions sharing the
        // same degree-3/degree-2 numerator/denominator shape. `<= 3`
        // leaves the same comfortable margin line_intersection's own test
        // asserts, not the full 0..8 bound.
        assert!(
            max_iters <= 3,
            "correctly_rounded_divide used {max_iters} iterations on some circumcenter input \
             -- re-examine before trusting the 0..8 bound for this construction"
        );
    }

    fn is_nondegenerate(a: Point2, b: Point2, c: Point2) -> bool {
        let dx1 = b.x() - a.x();
        let dy1 = b.y() - a.y();
        let dx2 = c.x() - a.x();
        let dy2 = c.y() - a.y();
        let d = dx1 * dy2 - dy1 * dx2;
        d.is_finite() && d != 0.0
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
}
