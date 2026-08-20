use super::expansion::{
    det3_exact, det3_with_precancel_bound, diff_expansion, rescale_for_sign_only,
    sign_only_expansion_sign,
};
use super::sign::Sign;
use crate::primitives::Point3;

/// Error-bound factor for `orient3d`'s floating-point filter.
///
/// `orient3d` computes 3 signed terms `term_X = adX * (P*Q - R*S)`, each
/// carrying roughly the same ~4u relative error as `orient2d`'s
/// determinant (see `ORIENT2D_ERR_BOUND_FACTOR`) from the inner
/// 2-multiply-1-subtract sub-determinant, plus ~1u more from the outer
/// multiply by `adX`; summing the 3 terms (2 more additions) adds ~2u
/// more. `12.0` is a deliberately generous constant over the derived
/// `~7`, verified empirically (not just derived) by
/// `tests/differential/orient3d.rs`.
///
/// The bound below sums each cofactor's **pre-subtraction** magnitudes
/// (`|bdy_cdz|+|bdz_cdy|` etc.), scaled by the outer row factor — not
/// `|term_a|+|term_b|+|term_c|` (the post-subtraction term magnitudes).
/// Using post-subtraction magnitudes was an earlier, wrong version of
/// this bound (and of `incircle`'s, where the same mistake was caught by
/// a differential test — see `docs/numerical-model.md` "Known limitation
/// (fixed): filter bound must use pre-cancellation magnitudes"): when the
/// inner subtraction itself suffers catastrophic cancellation (e.g. `b`
/// and `c` both roughly collinear with `d`), the true error is
/// proportional to the pre-cancellation magnitudes, not to the smaller
/// post-cancellation result.
const ORIENT3D_ERR_BOUND_FACTOR: f64 = 12.0 * f64::EPSILON / 2.0;

/// The sign of `det [a-d; b-d; c-d]` (the determinant with rows `a-d`,
/// `b-d`, `c-d`), proportional to the signed volume of the tetrahedron
/// `(a, b, c, d)`. `Sign::Zero` means the four points are coplanar.
///
/// The sign convention is exactly this determinant's algebraic sign — see
/// the doctest below for a concrete, verified example rather than a
/// spatial ("above"/"below") description, which is easy to get backwards
/// in prose. Swapping any two of the four arguments flips the sign
/// (standard determinant antisymmetry), covered by
/// `tests/adversarial/orient3d.rs`.
///
/// Never panics. Same filter + exact-fallback design as `orient2d`; see
/// `docs/numerical-model.md`.
///
/// ```
/// use kika::{Point3, orient3d, Sign};
///
/// let a = Point3::new(0.0, 0.0, 0.0).unwrap();
/// let b = Point3::new(1.0, 0.0, 0.0).unwrap();
/// let c = Point3::new(0.0, 1.0, 0.0).unwrap();
/// let d = Point3::new(0.0, 0.0, 1.0).unwrap();
/// assert_eq!(orient3d(a, b, c, d), Sign::Negative);
/// // Swapping b and c flips the sign, as for any determinant row swap.
/// assert_eq!(orient3d(a, c, b, d), Sign::Positive);
/// ```
pub fn orient3d(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
    let adx = a.x() - d.x();
    let ady = a.y() - d.y();
    let adz = a.z() - d.z();
    let bdx = b.x() - d.x();
    let bdy = b.y() - d.y();
    let bdz = b.z() - d.z();
    let cdx = c.x() - d.x();
    let cdy = c.y() - d.y();
    let cdz = c.z() - d.z();

    let (det, precancel_bound) =
        det3_with_precancel_bound((adx, ady, adz), (bdx, bdy, bdz), (cdx, cdy, cdz));
    let bound = ORIENT3D_ERR_BOUND_FACTOR * precancel_bound;

    if bound > 0.0 && det.abs() > bound {
        return Sign::of(det);
    }

    orient3d_exact(a, b, c, d)
}

/// Exact fallback: builds every `Xdy`-style term as an *exact* expansion
/// from the original coordinates via [`diff_expansion`], then multiplies
/// with [`product_of_expansions`] throughout. See `orient2d_exact`'s doc
/// comment and `docs/numerical-model.md` "Known limitation: exactness
/// starts at the original coordinates" for why reusing the filter's
/// once-rounded `adx` etc. here would not be fully exact. Coordinates are
/// first routed through [`rescale_for_sign_only`] — see that function's
/// doc comment.
fn orient3d_exact(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
    let [ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz] = rescale_for_sign_only([
        a.x(),
        a.y(),
        a.z(),
        b.x(),
        b.y(),
        b.z(),
        c.x(),
        c.y(),
        c.z(),
        d.x(),
        d.y(),
        d.z(),
    ]);
    let adx = diff_expansion(ax, dx);
    let ady = diff_expansion(ay, dy);
    let adz = diff_expansion(az, dz);
    let bdx = diff_expansion(bx, dx);
    let bdy = diff_expansion(by, dy);
    let bdz = diff_expansion(bz, dz);
    let cdx = diff_expansion(cx, dx);
    let cdy = diff_expansion(cy, dy);
    let cdz = diff_expansion(cz, dz);

    let det = det3_exact((&adx, &ady, &adz), (&bdx, &bdy, &bdz), (&cdx, &cdy, &cdz));
    sign_only_expansion_sign(&det)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).unwrap()
    }

    #[test]
    fn positive_tetrahedron() {
        // det [a-d; b-d; c-d] = +1, hand-verified (see orient3d doctest
        // for the mirror case).
        assert_eq!(
            orient3d(
                pt(0.0, 0.0, 0.0),
                pt(0.0, 1.0, 0.0),
                pt(1.0, 0.0, 0.0),
                pt(0.0, 0.0, 1.0)
            ),
            Sign::Positive
        );
    }

    #[test]
    fn negative_tetrahedron() {
        // det [a-d; b-d; c-d] = -1, hand-verified.
        assert_eq!(
            orient3d(
                pt(0.0, 0.0, 0.0),
                pt(1.0, 0.0, 0.0),
                pt(0.0, 1.0, 0.0),
                pt(0.0, 0.0, 1.0)
            ),
            Sign::Negative
        );
    }

    #[test]
    fn coplanar_points() {
        assert_eq!(
            orient3d(
                pt(0.0, 0.0, 0.0),
                pt(1.0, 0.0, 0.0),
                pt(0.0, 1.0, 0.0),
                pt(1.0, 1.0, 0.0)
            ),
            Sign::Zero
        );
    }

    #[test]
    fn duplicate_point_is_coplanar() {
        let p = pt(1.0, 2.0, 3.0);
        assert_eq!(
            orient3d(p, p, pt(4.0, 5.0, 6.0), pt(7.0, 8.0, 9.0)),
            Sign::Zero
        );
    }

    #[test]
    fn exact_fallback_does_not_panic_on_near_coplanar() {
        let a = pt(0.0, 0.0, 0.0);
        let b = pt(1e15, 1.0, 0.0);
        let c = pt(0.0, 1e15, 1.0);
        let d = pt(1e15, 1e15, 2.0 + 1e-9);
        let _ = orient3d(a, b, c, d);
    }
}
