//! Adversarial and property tests for `orient3d`. See
//! `tests/adversarial/orient2d.rs` for the rationale; mirrors it for 3D.

use kika::{Point3, Sign, orient3d};

fn pt(x: f64, y: f64, z: f64) -> Point3 {
    Point3::new(x, y, z).unwrap()
}

/// Swapping any two arguments flips the sign (determinant row swap).
#[test]
fn swap_flips_sign() {
    let (a, b, c, d) = (
        pt(0.0, 0.0, 0.0),
        pt(1.0, 0.0, 0.0),
        pt(0.0, 1.0, 0.0),
        pt(0.0, 0.0, 1.0),
    );
    assert_eq!(orient3d(a, b, c, d).negate(), orient3d(b, a, c, d));
    assert_eq!(orient3d(a, b, c, d).negate(), orient3d(a, c, b, d));
    assert_eq!(orient3d(a, b, c, d).negate(), orient3d(d, b, c, a));
}

/// Translation by an exactly-representable vector must not change the
/// sign (power-of-two coordinates/translations: no new rounding).
#[test]
fn translation_invariance() {
    let (a, b, c, d) = (
        pt(0.0, 0.0, 0.0),
        pt(8.0, 0.0, 0.0),
        pt(0.0, 8.0, 0.0),
        pt(0.0, 0.0, 8.0),
    );
    let want = orient3d(a, b, c, d);
    for &(tx, ty, tz) in &[(32.0, 0.0, 0.0), (0.0, -64.0, 16.0), (16.0, 16.0, -16.0)] {
        let t = |p: Point3| pt(p.x() + tx, p.y() + ty, p.z() + tz);
        assert_eq!(
            orient3d(t(a), t(b), t(c), t(d)),
            want,
            "translation ({tx},{ty},{tz})"
        );
    }
}

/// Positive uniform scaling must not change the sign (power-of-two
/// scales: no new rounding).
#[test]
fn positive_uniform_scale_invariance() {
    let (a, b, c, d) = (
        pt(0.0, 0.0, 0.0),
        pt(8.0, 0.0, 0.0),
        pt(0.0, 8.0, 0.0),
        pt(0.0, 0.0, 8.0),
    );
    let want = orient3d(a, b, c, d);
    for &s in &[0.5_f64, 2.0, 1024.0, 1.0 / 1024.0] {
        let scale = |p: Point3| pt(p.x() * s, p.y() * s, p.z() * s);
        assert_eq!(
            orient3d(scale(a), scale(b), scale(c), scale(d)),
            want,
            "scale {s}"
        );
    }
}

#[test]
fn coplanar_at_various_scales() {
    for &scale in &[1.0_f64, 1e-20, 1e20] {
        let a = pt(0.0, 0.0, 0.0);
        let b = pt(scale, 0.0, 0.0);
        let c = pt(0.0, scale, 0.0);
        let d = pt(scale, scale, 0.0);
        assert_eq!(orient3d(a, b, c, d), Sign::Zero);
    }
}

#[test]
fn duplicate_point_variants_are_coplanar() {
    let p = pt(1.0, 1.0, 1.0);
    let q = pt(2.0, 3.0, 4.0);
    let r = pt(5.0, -1.0, 2.0);
    assert_eq!(orient3d(p, p, q, r), Sign::Zero);
    assert_eq!(orient3d(q, p, p, r), Sign::Zero);
    assert_eq!(orient3d(q, r, p, p), Sign::Zero);
}

#[test]
fn near_subnormal_scale_does_not_panic() {
    let scale = 1e-140_f64;
    let a = pt(0.0, 0.0, 0.0);
    let b = pt(scale, 0.0, 0.0);
    let c = pt(0.0, scale, 0.0);
    let d = pt(0.0, 0.0, scale);
    // Just require a well-typed, non-panicking result here; sign
    // correctness at this scale is covered by the bigint-oracle
    // differential tests, not re-derived by hand in this file.
    let _ = orient3d(a, b, c, d);
}

#[test]
fn extreme_large_scale_does_not_panic() {
    // `1e150` (matching `orient2d`'s own extreme-scale test) is *not* safe
    // here: `orient3d` is a degree-3 determinant (triple products of
    // coordinate differences), so its structural safe ceiling is
    // `f64::MAX^(1/3) ~= 5.65e102`, well below orient2d's degree-2 ceiling
    // of `f64::MAX^(1/2) ~= 1.34e154`. `1e90` mirrors the same margin this
    // crate's `circumcenter`/`line_intersection` already use as their own
    // `RESCALE_THRESHOLD` for degree-3 constructions. Past that ceiling,
    // this predicate's exact fallback can hit the documented, deliberately
    // deferred `two_product` overflow (`docs/numerical-model.md` "Known
    // limitation: exact-product representability ceiling") rather than the
    // extreme-*mixed*-magnitude bug this crate's 0.7.1 round fixed.
    let scale = 1e90_f64;
    let a = pt(0.0, 0.0, 0.0);
    let b = pt(scale, 0.0, 0.0);
    let c = pt(0.0, scale, 0.0);
    let d = pt(0.0, 0.0, scale);
    let _ = orient3d(a, b, c, d);
}

/// Mixed huge + tiny coordinates in one call, mirroring the shape of the
/// 0.7.1 `delaunay2()` panic's repro triples
/// (`tests/regression/orient2d.rs`) rather than the *uniform* extreme
/// magnitude `extreme_large_scale_does_not_panic` above already covers —
/// exercises `rescale_for_sign_only`'s trigger threshold (`f64::MAX/4`)
/// without approaching the separate, deliberately deferred
/// `two_product` product-ceiling (see `docs/numerical-model.md`), since
/// only one coordinate here is ever huge at a time.
#[test]
fn mixed_huge_and_tiny_magnitude_does_not_panic() {
    let a = pt(1e308, 0.0, 0.0);
    let b = pt(-1e308, 1e-10, 0.0);
    let c = pt(0.0, 1e-10, 1e-10);
    let d = pt(0.0, 0.0, 1e-10);
    let _ = orient3d(a, b, c, d);
}
