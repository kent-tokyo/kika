use super::expansion::{
    det3_exact, det3_with_precancel_bound, diff_expansion, expansion_sum, negate,
    product_of_expansions, rescale_for_sign_only, sign_only_expansion_sign,
};
use super::sign::Sign;
use crate::primitives::Point3;

/// Error-bound factor for `insphere`'s floating-point filter.
///
/// `insphere` computes 4 signed terms `term_X = adX * M1j`, where each
/// `M1j` is itself a 3x3 cofactor determinant (same structure as
/// `orient3d`'s). The bound sums, for each of the 4 terms, the outer
/// factor times `M1j`'s own **pre-cancellation** magnitude (the sum of
/// `M1j`'s inner cofactor products' absolute values, not `M1j`'s
/// possibly-much-smaller post-cancellation value) — see
/// [`det3_with_precancel_bound`] and `docs/numerical-model.md` "Known
/// limitation (fixed): filter bound must use pre-cancellation
/// magnitudes" for why this is the only sound way to bound a nested
/// cofactor computation (this is the same lesson `orient3d`/`incircle`
/// already apply, extended one level deeper). `40.0` is a deliberately
/// generous constant, verified empirically (not just derived) by
/// `tests/differential/insphere.rs`.
const INSPHERE_ERR_BOUND_FACTOR: f64 = 40.0 * f64::EPSILON / 2.0;

/// The sign of the 4x4 "lift to the 4D paraboloid" determinant
/// `det [adx ady adz adw; bdx bdy bdz bdw; cdx cdy cdz cdw; ddx ddy ddz ddw]`
/// where `Xdx = X.x()-e.x()` etc. and `Xdw = Xdx^2+Xdy^2+Xdz^2`.
///
/// `Sign::Positive` means `e` lies inside the sphere through `a`, `b`,
/// `c`, `d` for the orientation verified by the doctest below (`e` at the
/// center of a sphere-inscribed tetrahedron is `Positive`). `Sign::Zero`
/// means the five points are cospherical (or `a,b,c,d` are coplanar,
/// which degenerates the notion of "sphere" the same way collinear
/// points degenerate `incircle`'s "circle" — see
/// `docs/degeneracy-policy.md`). Swapping any two of `a`, `b`, `c`, `d`
/// flips the sign (row swap); `e` defines the paraboloid lift, not a row.
///
/// Never panics. Same filter + exact-fallback design as `incircle`,
/// including its narrower-than-`orient3d` safe coordinate-magnitude range
/// (this predicate is degree 5, one worse than `incircle`'s degree 4) —
/// see `docs/numerical-model.md`.
///
/// ```
/// use kika::{Point3, insphere, Sign};
///
/// // Regular tetrahedron inscribed in the unit sphere, apex up.
/// let a = Point3::new(1.0, 0.0, -1.0 / 2.0_f64.sqrt()).unwrap();
/// let b = Point3::new(-1.0, 0.0, -1.0 / 2.0_f64.sqrt()).unwrap();
/// let c = Point3::new(0.0, 1.0, 1.0 / 2.0_f64.sqrt()).unwrap();
/// let d = Point3::new(0.0, -1.0, 1.0 / 2.0_f64.sqrt()).unwrap();
/// let center = Point3::new(0.0, 0.0, 0.0).unwrap();
/// assert_eq!(insphere(a, b, c, d, center), Sign::Positive);
///
/// let far_outside = Point3::new(100.0, 100.0, 100.0).unwrap();
/// assert_eq!(insphere(a, b, c, d, far_outside), Sign::Negative);
/// ```
pub fn insphere(a: Point3, b: Point3, c: Point3, d: Point3, e: Point3) -> Sign {
    let adx = a.x() - e.x();
    let ady = a.y() - e.y();
    let adz = a.z() - e.z();
    let bdx = b.x() - e.x();
    let bdy = b.y() - e.y();
    let bdz = b.z() - e.z();
    let cdx = c.x() - e.x();
    let cdy = c.y() - e.y();
    let cdz = c.z() - e.z();
    let ddx = d.x() - e.x();
    let ddy = d.y() - e.y();
    let ddz = d.z() - e.z();

    let adw = adx * adx + ady * ady + adz * adz;
    let bdw = bdx * bdx + bdy * bdy + bdz * bdz;
    let cdw = cdx * cdx + cdy * cdy + cdz * cdz;
    let ddw = ddx * ddx + ddy * ddy + ddz * ddz;

    let (m11, m11_bound) =
        det3_with_precancel_bound((bdy, bdz, bdw), (cdy, cdz, cdw), (ddy, ddz, ddw));
    let (m12, m12_bound) =
        det3_with_precancel_bound((bdx, bdz, bdw), (cdx, cdz, cdw), (ddx, ddz, ddw));
    let (m13, m13_bound) =
        det3_with_precancel_bound((bdx, bdy, bdw), (cdx, cdy, cdw), (ddx, ddy, ddw));
    let (m14, m14_bound) =
        det3_with_precancel_bound((bdx, bdy, bdz), (cdx, cdy, cdz), (ddx, ddy, ddz));

    let term_a = adx * m11;
    let term_b = ady * m12;
    let term_c = adz * m13;
    let term_d = adw * m14;
    let det = term_a - term_b + term_c - term_d;

    let bound = INSPHERE_ERR_BOUND_FACTOR
        * (adx.abs() * m11_bound
            + ady.abs() * m12_bound
            + adz.abs() * m13_bound
            + adw.abs() * m14_bound);

    if bound > 0.0 && det.abs() > bound {
        return Sign::of(det);
    }

    insphere_exact(a, b, c, d, e)
}

/// Exact fallback: coordinates are first routed through
/// [`rescale_for_sign_only`] — see that function's doc comment and
/// `orient2d_exact`'s.
fn insphere_exact(a: Point3, b: Point3, c: Point3, d: Point3, e: Point3) -> Sign {
    let [ax, ay, az, bx, by, bz, cx, cy, cz, dx, dy, dz, ex, ey, ez] = rescale_for_sign_only([
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
        e.x(),
        e.y(),
        e.z(),
    ]);
    let adx = diff_expansion(ax, ex);
    let ady = diff_expansion(ay, ey);
    let adz = diff_expansion(az, ez);
    let bdx = diff_expansion(bx, ex);
    let bdy = diff_expansion(by, ey);
    let bdz = diff_expansion(bz, ez);
    let cdx = diff_expansion(cx, ex);
    let cdy = diff_expansion(cy, ey);
    let cdz = diff_expansion(cz, ez);
    let ddx = diff_expansion(dx, ex);
    let ddy = diff_expansion(dy, ey);
    let ddz = diff_expansion(dz, ez);

    let square_sum = |x: &[f64], y: &[f64], z: &[f64]| -> Vec<f64> {
        expansion_sum(
            &expansion_sum(&product_of_expansions(x, x), &product_of_expansions(y, y)),
            &product_of_expansions(z, z),
        )
    };
    let adw = square_sum(&adx, &ady, &adz);
    let bdw = square_sum(&bdx, &bdy, &bdz);
    let cdw = square_sum(&cdx, &cdy, &cdz);
    let ddw = square_sum(&ddx, &ddy, &ddz);

    let m11 = det3_exact((&bdy, &bdz, &bdw), (&cdy, &cdz, &cdw), (&ddy, &ddz, &ddw));
    let m12 = det3_exact((&bdx, &bdz, &bdw), (&cdx, &cdz, &cdw), (&ddx, &ddz, &ddw));
    let m13 = det3_exact((&bdx, &bdy, &bdw), (&cdx, &cdy, &cdw), (&ddx, &ddy, &ddw));
    let m14 = det3_exact((&bdx, &bdy, &bdz), (&cdx, &cdy, &cdz), (&ddx, &ddy, &ddz));

    let term_a = product_of_expansions(&adx, &m11);
    let term_b = product_of_expansions(&ady, &m12);
    let term_c = product_of_expansions(&adz, &m13);
    let term_d = product_of_expansions(&adw, &m14);

    let det = expansion_sum(
        &expansion_sum(&expansion_sum(&term_a, &negate(&term_b)), &term_c),
        &negate(&term_d),
    );
    sign_only_expansion_sign(&det)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).unwrap()
    }

    fn regular_tetrahedron() -> (Point3, Point3, Point3, Point3) {
        let h = 1.0 / 2.0_f64.sqrt();
        (
            pt(1.0, 0.0, -h),
            pt(-1.0, 0.0, -h),
            pt(0.0, 1.0, h),
            pt(0.0, -1.0, h),
        )
    }

    #[test]
    fn center_is_inside() {
        let (a, b, c, d) = regular_tetrahedron();
        assert_eq!(insphere(a, b, c, d, pt(0.0, 0.0, 0.0)), Sign::Positive);
    }

    #[test]
    fn far_point_is_outside() {
        let (a, b, c, d) = regular_tetrahedron();
        assert_eq!(
            insphere(a, b, c, d, pt(100.0, 100.0, 100.0)),
            Sign::Negative
        );
    }

    #[test]
    fn duplicate_point_is_zero() {
        let p = pt(1.0, 2.0, 3.0);
        assert_eq!(
            insphere(
                p,
                p,
                pt(4.0, 5.0, 6.0),
                pt(7.0, 8.0, 9.0),
                pt(1.0, 1.0, 1.0)
            ),
            Sign::Zero
        );
    }

    #[test]
    fn coplanar_and_concyclic_abcd_is_zero() {
        // A square's corners are coplanar AND concyclic (lie on a common
        // circle within that plane) — the actual degenerate condition,
        // one dimension up from incircle's "collinear" case: a plane
        // intersects a sphere in a circle, so 4 coplanar points lie on
        // *some* sphere iff they're concyclic within that plane. Mere
        // coplanarity is NOT sufficient — see
        // `coplanar_but_not_concyclic_abcd_is_not_zero` below, which a
        // first (wrong) version of this test's name/doc claimed.
        assert_eq!(
            insphere(
                pt(0.0, 0.0, 0.0),
                pt(1.0, 0.0, 0.0),
                pt(0.0, 1.0, 0.0),
                pt(1.0, 1.0, 0.0),
                pt(0.5, 0.5, 1.0)
            ),
            Sign::Zero
        );
    }

    #[test]
    fn coplanar_but_not_concyclic_abcd_is_not_zero() {
        // A generic (non-concyclic) coplanar quadrilateral does NOT lie
        // on any common sphere, so insphere is generically nonzero here
        // — hand-verified det=-18 for this exact case, analogous to
        // incircle's collinear-abc-with-d-off-line behavior.
        assert_eq!(
            insphere(
                pt(0.0, 0.0, 0.0),
                pt(1.0, 0.0, 0.0),
                pt(0.0, 2.0, 0.0),
                pt(3.0, 3.0, 0.0),
                pt(0.5, 0.5, 1.0)
            ),
            Sign::Negative
        );
    }

    #[test]
    fn exact_fallback_does_not_panic_on_near_cospherical() {
        let (a, b, c, d) = regular_tetrahedron();
        let e = pt(0.0, 0.0, 1e-9);
        let _ = insphere(a, b, c, d, e);
    }
}
