use super::expansion::{diff_expansion, expansion_sign, expansion_sum, product_of_expansions};
use super::sign::Sign;
use crate::primitives::Point2;

/// Error-bound factor for `incircle`'s floating-point filter.
///
/// `incircle` lifts each point to `(dx, dy, dx^2+dy^2)` and computes the
/// same 3-term cofactor-sum structure as `orient3d`, but the third ("z")
/// coordinate is itself `fl(dx*dx + dy*dy)` — two more roundings before it
/// ever enters the cofactor computation. `24.0` is a deliberately generous
/// constant over orient3d's derived `~7` plus this extra ~2-3u, verified
/// empirically (not just derived) by `tests/differential/incircle.rs`.
///
/// The bound is **not** `|term_a|+|term_b|+|term_c|` (the post-subtraction
/// term magnitudes) — that was an earlier, wrong version of this filter
/// (and of `orient3d`'s), found by a differential test after it let a
/// wrong sign through: when e.g. `b` and `c` are both close to `d` but `d`
/// itself is far from the origin, `bdx≈cdx` and `bdz≈cdz`, so
/// `bdx*cdz - bdz*cdx` suffers catastrophic cancellation between two huge
/// (~1e89 in the found case) values. The *absolute* error introduced by
/// that cancellation is proportional to the **pre-subtraction** magnitudes
/// `|bdx*cdz| + |bdz*cdx|`, not to the (possibly much smaller,
/// post-cancellation) result — using the result's own magnitude as the
/// bound's scale silently underestimates the true uncertainty. The bound
/// below sums each cofactor's pre-subtraction magnitudes, scaled by the
/// outer row factor, matching how the filter's error actually
/// accumulates. See `docs/numerical-model.md` "Known limitation (fixed):
/// filter bound must use pre-cancellation magnitudes".
const INCIRCLE_ERR_BOUND_FACTOR: f64 = 24.0 * f64::EPSILON / 2.0;

/// The sign of `det [adx ady adz; bdx bdy bdz; cdx cdy cdz]` where
/// `Xdx = X.x()-d.x()`, `Xdy = X.y()-d.y()`, `Xdz = Xdx^2+Xdy^2` — the
/// standard "lift to the paraboloid" incircle determinant.
///
/// `Sign::Positive` means `d` lies inside the circle through `a`, `b`,
/// `c` when `a`, `b`, `c` are ordered counterclockwise (verified by the
/// doctest below: `d` at the center of a CCW unit-circle triangle is
/// `Positive`). `Sign::Zero` means the four points are cocircular (or the
/// three defining points are collinear, which degenerates the notion of
/// "circle"). Swapping any two of `a`, `b`, `c` flips the sign (row swap);
/// `d` does not participate in that antisymmetry the same way, since it
/// defines the paraboloid lift, not a determinant row.
///
/// Never panics. Same filter + exact-fallback design as `orient2d`; the
/// exact fallback builds every coordinate difference (and its square) as
/// an exact expansion straight from `a, b, c, d`, per
/// `docs/numerical-model.md` "Known limitation (fixed): exactness starts
/// at the original coordinates".
///
/// ```
/// use kika::{Point2, incircle, Sign};
///
/// let a = Point2::new(1.0, 0.0).unwrap();
/// let b = Point2::new(0.0, 1.0).unwrap();
/// let c = Point2::new(-1.0, 0.0).unwrap();
/// let center = Point2::new(0.0, 0.0).unwrap();
/// assert_eq!(incircle(a, b, c, center), Sign::Positive);
///
/// let far_outside = Point2::new(100.0, 100.0).unwrap();
/// assert_eq!(incircle(a, b, c, far_outside), Sign::Negative);
/// ```
pub fn incircle(a: Point2, b: Point2, c: Point2, d: Point2) -> Sign {
    let adx = a.x() - d.x();
    let ady = a.y() - d.y();
    let bdx = b.x() - d.x();
    let bdy = b.y() - d.y();
    let cdx = c.x() - d.x();
    let cdy = c.y() - d.y();

    let adz = adx * adx + ady * ady;
    let bdz = bdx * bdx + bdy * bdy;
    let cdz = cdx * cdx + cdy * cdy;

    let bdy_cdz = bdy * cdz;
    let bdz_cdy = bdz * cdy;
    let bdx_cdz = bdx * cdz;
    let bdz_cdx = bdz * cdx;
    let bdx_cdy = bdx * cdy;
    let bdy_cdx = bdy * cdx;

    let term_a = adx * (bdy_cdz - bdz_cdy);
    let term_b = ady * (bdx_cdz - bdz_cdx);
    let term_c = adz * (bdx_cdy - bdy_cdx);
    let det = term_a - term_b + term_c;

    let bound = INCIRCLE_ERR_BOUND_FACTOR
        * (adx.abs() * (bdy_cdz.abs() + bdz_cdy.abs())
            + ady.abs() * (bdx_cdz.abs() + bdz_cdx.abs())
            + adz.abs() * (bdx_cdy.abs() + bdy_cdx.abs()));

    if bound > 0.0 && det.abs() > bound {
        return Sign::of(det);
    }

    incircle_exact(a, b, c, d)
}

fn incircle_exact(a: Point2, b: Point2, c: Point2, d: Point2) -> Sign {
    let adx = diff_expansion(a.x(), d.x());
    let ady = diff_expansion(a.y(), d.y());
    let bdx = diff_expansion(b.x(), d.x());
    let bdy = diff_expansion(b.y(), d.y());
    let cdx = diff_expansion(c.x(), d.x());
    let cdy = diff_expansion(c.y(), d.y());

    let adz = expansion_sum(
        &product_of_expansions(&adx, &adx),
        &product_of_expansions(&ady, &ady),
    );
    let bdz = expansion_sum(
        &product_of_expansions(&bdx, &bdx),
        &product_of_expansions(&bdy, &bdy),
    );
    let cdz = expansion_sum(
        &product_of_expansions(&cdx, &cdx),
        &product_of_expansions(&cdy, &cdy),
    );

    let bc_yz = expansion_sum(
        &product_of_expansions(&bdy, &cdz),
        &negate(&product_of_expansions(&bdz, &cdy)),
    );
    let bc_xz = expansion_sum(
        &product_of_expansions(&bdx, &cdz),
        &negate(&product_of_expansions(&bdz, &cdx)),
    );
    let bc_xy = expansion_sum(
        &product_of_expansions(&bdx, &cdy),
        &negate(&product_of_expansions(&bdy, &cdx)),
    );

    let term_a = product_of_expansions(&adx, &bc_yz);
    let term_b = product_of_expansions(&ady, &bc_xz);
    let term_c = product_of_expansions(&adz, &bc_xy);

    let det = expansion_sum(&expansion_sum(&term_a, &negate(&term_b)), &term_c);
    expansion_sign(&det)
}

fn negate(e: &[f64]) -> Vec<f64> {
    e.iter().map(|v| -v).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn center_is_inside() {
        assert_eq!(
            incircle(pt(1.0, 0.0), pt(0.0, 1.0), pt(-1.0, 0.0), pt(0.0, 0.0)),
            Sign::Positive
        );
    }

    #[test]
    fn far_point_is_outside() {
        assert_eq!(
            incircle(pt(1.0, 0.0), pt(0.0, 1.0), pt(-1.0, 0.0), pt(100.0, 100.0)),
            Sign::Negative
        );
    }

    #[test]
    fn on_circle_is_zero() {
        assert_eq!(
            incircle(pt(1.0, 0.0), pt(0.0, 1.0), pt(-1.0, 0.0), pt(0.0, -1.0)),
            Sign::Zero
        );
    }

    #[test]
    fn all_four_points_collinear_is_zero() {
        // A "circle" through 3 collinear points degenerates to the line
        // itself extended to infinity; a 4th point on that same line is
        // on the degenerate circle too.
        assert_eq!(
            incircle(pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0), pt(0.5, 0.0)),
            Sign::Zero
        );
    }

    #[test]
    fn collinear_abc_with_d_off_line_is_not_zero() {
        // Collinear a,b,c does NOT make the predicate identically zero:
        // the degenerate "circle" (radius -> infinity) becomes a line,
        // and d off that line is on one definite side of it. Hand-verified
        // det=2.0 for this exact case.
        assert_eq!(
            incircle(pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0), pt(0.5, 1.0)),
            Sign::Positive
        );
    }

    #[test]
    fn duplicate_point_is_zero() {
        let p = pt(1.0, 2.0);
        assert_eq!(incircle(p, p, pt(3.0, 4.0), pt(5.0, 6.0)), Sign::Zero);
    }

    #[test]
    fn exact_fallback_does_not_panic_on_near_cocircular() {
        let a = pt(1e15, 0.0);
        let b = pt(0.0, 1e15);
        let c = pt(-1e15, 0.0);
        let d = pt(0.0, -1e15 + 1e-3);
        let _ = incircle(a, b, c, d);
    }
}
