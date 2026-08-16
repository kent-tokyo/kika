use super::expansion::{diff_expansion, expansion_sign, expansion_sum, product_of_expansions};
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

    let bound = ORIENT3D_ERR_BOUND_FACTOR * (term_a.abs() + term_b.abs() + term_c.abs());

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
/// once-rounded `adx` etc. here would not be fully exact.
fn orient3d_exact(a: Point3, b: Point3, c: Point3, d: Point3) -> Sign {
    let adx = diff_expansion(a.x(), d.x());
    let ady = diff_expansion(a.y(), d.y());
    let adz = diff_expansion(a.z(), d.z());
    let bdx = diff_expansion(b.x(), d.x());
    let bdy = diff_expansion(b.y(), d.y());
    let bdz = diff_expansion(b.z(), d.z());
    let cdx = diff_expansion(c.x(), d.x());
    let cdy = diff_expansion(c.y(), d.y());
    let cdz = diff_expansion(c.z(), d.z());

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
