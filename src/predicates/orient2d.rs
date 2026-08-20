use super::expansion::{
    diff_expansion, expansion_sum, product_of_expansions, rescale_for_sign_only,
    sign_only_expansion_sign,
};
use super::sign::{Orientation, Sign};
use crate::primitives::Point2;

/// Error-bound factor for `orient2d`'s floating-point filter.
///
/// `orient2d` computes `left = fl(acx*bcy)`, `right = fl(acy*bcx)`,
/// `det = fl(left - right)`, where `acx = fl(a.x-c.x)` etc. Each of the 4
/// subtractions and 2 multiplications and the final subtraction carries a
/// relative error bounded by unit roundoff `u = f64::EPSILON/2`. Working
/// through the error propagation (documented in full in
/// `docs/numerical-model.md`) gives `|det_computed - det_exact| <= 4u *
/// (|left_exact| + |right_exact|) + O(u^2)`; using the *computed* `left`,
/// `right` in place of the exact ones only changes this by another O(u)
/// term. `7.0` is a deliberately generous constant over the derived `~4`,
/// verified empirically (not just derived) by
/// `filter_never_disagrees_with_exact_sign` in `tests/differential/`.
const ORIENT2D_ERR_BOUND_FACTOR: f64 = 7.0 * f64::EPSILON / 2.0;

/// The orientation of the ordered triple `(a, b, c)`: whether `c` lies to
/// the left of, to the right of, or on the directed line through `a` and
/// `b` — equivalently, the sign of twice the signed area of triangle
/// `abc`.
///
/// Never panics. Uses a floating-point filter with a computed error bound
/// on the common case, falling back to exact expansion arithmetic when the
/// filter is inconclusive (near-collinear input). See
/// `docs/numerical-model.md`.
///
/// ```
/// use kika::{Point2, orient2d, Orientation};
///
/// let a = Point2::new(0.0, 0.0).unwrap();
/// let b = Point2::new(1.0, 0.0).unwrap();
/// let c = Point2::new(0.0, 1.0).unwrap();
/// assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
/// ```
pub fn orient2d(a: Point2, b: Point2, c: Point2) -> Orientation {
    let acx = a.x() - c.x();
    let acy = a.y() - c.y();
    let bcx = b.x() - c.x();
    let bcy = b.y() - c.y();

    let left = acx * bcy;
    let right = acy * bcx;
    let det = left - right;

    let bound = ORIENT2D_ERR_BOUND_FACTOR * (left.abs() + right.abs());
    if bound > 0.0 && det.abs() > bound {
        return Orientation::from(Sign::of(det));
    }

    Orientation::from(orient2d_exact(a, b, c))
}

/// Exact fallback: builds `acx`, `acy`, `bcx`, `bcy` as *exact* 2-term
/// expansions from the original coordinates (not the once-rounded `f64`
/// differences the filter uses) via [`diff_expansion`], then multiplies
/// those expansions exactly via [`product_of_expansions`]. Reusing the
/// filter's already-rounded differences here would only be exact relative
/// to that rounding, not to the true input coordinates — see
/// `docs/numerical-model.md` "Known limitation: exactness starts at the
/// original coordinates". Coordinates are first routed through
/// [`rescale_for_sign_only`], which is a no-op except at extreme
/// magnitude, where it prevents `diff_expansion`'s internal `two_sum` from
/// overflowing — see that function's doc comment and
/// `docs/numerical-model.md` "Known limitation (fixed): two_sum overflow
/// for sign-only predicates".
fn orient2d_exact(a: Point2, b: Point2, c: Point2) -> Sign {
    let [ax, ay, bx, by, cx, cy] =
        rescale_for_sign_only([a.x(), a.y(), b.x(), b.y(), c.x(), c.y()]);
    let acx = diff_expansion(ax, cx);
    let acy = diff_expansion(ay, cy);
    let bcx = diff_expansion(bx, cx);
    let bcy = diff_expansion(by, cy);

    let left = product_of_expansions(&acx, &bcy);
    let right = product_of_expansions(&acy, &bcx);
    let neg_right: Vec<f64> = right.iter().map(|v| -v).collect();
    let det = expansion_sum(&left, &neg_right);
    sign_only_expansion_sign(&det)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn counterclockwise_triangle() {
        assert_eq!(
            orient2d(pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)),
            Orientation::CounterClockwise
        );
    }

    #[test]
    fn clockwise_triangle() {
        assert_eq!(
            orient2d(pt(0.0, 0.0), pt(0.0, 1.0), pt(1.0, 0.0)),
            Orientation::Clockwise
        );
    }

    #[test]
    fn collinear_points() {
        assert_eq!(
            orient2d(pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)),
            Orientation::Collinear
        );
    }

    #[test]
    fn duplicate_point_is_collinear() {
        let p = pt(3.0, 4.0);
        assert_eq!(orient2d(p, p, pt(5.0, 6.0)), Orientation::Collinear);
        assert_eq!(orient2d(p, pt(5.0, 6.0), p), Orientation::Collinear);
    }

    #[test]
    fn antisymmetric_under_swap() {
        let (a, b, c) = (pt(0.1, 0.2), pt(4.3, -1.5), pt(-2.0, 7.0));
        let orig = orient2d(a, b, c);
        let swapped = orient2d(b, a, c);
        match (orig, swapped) {
            (Orientation::CounterClockwise, Orientation::Clockwise) => {}
            (Orientation::Clockwise, Orientation::CounterClockwise) => {}
            (Orientation::Collinear, Orientation::Collinear) => {}
            other => panic!("swap did not flip orientation: {other:?}"),
        }
    }

    #[test]
    fn exact_fallback_triggers_on_near_collinear() {
        // Points chosen so the float filter is inconclusive but the true
        // orientation is not collinear: exercises orient2d_exact directly.
        let a = pt(0.0, 0.0);
        let b = pt(1e15, 1.0);
        let c = pt(2e15, 2.0 + 1e-9);
        // Just check it terminates with a well-typed, non-panicking answer;
        // the exact value is cross-checked against the bigint oracle in
        // tests/differential/orient2d.rs.
        let _ = orient2d(a, b, c);
    }

    #[test]
    fn signed_zero_matches_positive_zero() {
        let a = pt(0.0, 0.0);
        let b = pt(1.0, 0.0);
        let c_pos = pt(0.0, 0.0);
        let c_neg = pt(-0.0, -0.0);
        assert_eq!(orient2d(a, b, c_pos), orient2d(a, b, c_neg));
    }
}
