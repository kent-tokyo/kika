//! Adversarial and property tests for `insphere`. See
//! `tests/adversarial/orient2d.rs` for the rationale.

use kika::{Point3, Sign, insphere};

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

/// Swapping two of the four defining points (a, b, c, d) flips the sign
/// (determinant row swap); e is not a row.
#[test]
fn swap_abcd_flips_sign() {
    let (a, b, c, d) = regular_tetrahedron();
    let e = pt(0.0, 0.0, 0.0);
    assert_eq!(insphere(a, b, c, d, e).negate(), insphere(b, a, c, d, e));
    assert_eq!(insphere(a, b, c, d, e).negate(), insphere(a, c, b, d, e));
    assert_eq!(insphere(a, b, c, d, e).negate(), insphere(d, b, c, a, e));
}

/// Translation by an exactly-representable vector must not change the
/// sign (power-of-two coordinates/translations: no new rounding).
#[test]
fn translation_invariance() {
    let (a, b, c, d) = (
        pt(8.0, 0.0, -8.0),
        pt(-8.0, 0.0, -8.0),
        pt(0.0, 8.0, 8.0),
        pt(0.0, -8.0, 8.0),
    );
    let e = pt(0.0, 0.0, 0.0);
    let want = insphere(a, b, c, d, e);
    for &(tx, ty, tz) in &[(32.0, 0.0, 0.0), (0.0, -64.0, 16.0), (16.0, 16.0, -16.0)] {
        let t = |p: Point3| pt(p.x() + tx, p.y() + ty, p.z() + tz);
        assert_eq!(
            insphere(t(a), t(b), t(c), t(d), t(e)),
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
        pt(8.0, 0.0, -8.0),
        pt(-8.0, 0.0, -8.0),
        pt(0.0, 8.0, 8.0),
        pt(0.0, -8.0, 8.0),
    );
    let e = pt(0.0, 0.0, 0.0);
    let want = insphere(a, b, c, d, e);
    for &s in &[0.5_f64, 2.0, 1024.0, 1.0 / 1024.0] {
        let scale = |p: Point3| pt(p.x() * s, p.y() * s, p.z() * s);
        assert_eq!(
            insphere(scale(a), scale(b), scale(c), scale(d), scale(e)),
            want,
            "scale {s}"
        );
    }
}

#[test]
fn cospherical_at_various_scales() {
    // 5 of the 6 octahedron vertices (+-scale,0,0),(0,+-scale,0),(0,0,+-scale):
    // all exactly at distance `scale` from the origin, with exactly
    // representable (rational) coordinates — unlike a sqrt()-based
    // construction, which would only be cospherical up to sqrt's
    // rounding error, not exactly (a real trap this test fell into
    // during development: insphere correctly reported Positive, not
    // Zero, for a sqrt()-based "cospherical" configuration that wasn't
    // actually exactly cospherical once sqrt's rounding was accounted
    // for — see tasks/lessons.md).
    for &scale in &[1.0_f64, 1e-10, 1e10] {
        let a = pt(scale, 0.0, 0.0);
        let b = pt(-scale, 0.0, 0.0);
        let c = pt(0.0, scale, 0.0);
        let d = pt(0.0, 0.0, scale);
        let e = pt(0.0, -scale, 0.0);
        assert_eq!(insphere(a, b, c, d, e), Sign::Zero);
    }
}

#[test]
fn duplicate_point_variants_are_zero() {
    let p = pt(1.0, 1.0, 1.0);
    let q = pt(2.0, 3.0, 4.0);
    let r = pt(5.0, -1.0, 2.0);
    let e = pt(0.0, 0.0, 0.0);
    assert_eq!(insphere(p, p, q, r, e), Sign::Zero);
    assert_eq!(insphere(q, p, p, r, e), Sign::Zero);
    assert_eq!(insphere(q, r, p, p, e), Sign::Zero);
}

#[test]
fn near_subnormal_scale_does_not_panic() {
    let scale = 1e-30_f64;
    let (a, b, c, d) = regular_tetrahedron();
    let scaled = |p: Point3| pt(p.x() * scale, p.y() * scale, p.z() * scale);
    let e = pt(0.0, 0.0, 0.0);
    // See docs/numerical-model.md "Known limitation: incircle/insphere
    // have a narrower safe magnitude range" — correctness at this scale
    // is not asserted here, only that it doesn't panic.
    let _ = insphere(scaled(a), scaled(b), scaled(c), scaled(d), e);
}

#[test]
fn extreme_large_scale_does_not_panic() {
    let scale = 1e30_f64;
    let (a, b, c, d) = regular_tetrahedron();
    let scaled = |p: Point3| pt(p.x() * scale, p.y() * scale, p.z() * scale);
    let e = pt(0.0, 0.0, 0.0);
    let _ = insphere(scaled(a), scaled(b), scaled(c), scaled(d), e);
}

/// Mixed huge + tiny coordinates in one call, mirroring the shape of the
/// 0.7.1 `delaunay2()` panic's repro triples
/// (`tests/regression/orient2d.rs`) rather than the *uniform* extreme
/// magnitude `extreme_large_scale_does_not_panic` above already covers.
///
/// As with `incircle` (see its own `mixed_huge_and_tiny_magnitude_does_not_panic`),
/// this can't be pushed to `rescale_for_sign_only`'s own trigger threshold
/// (`f64::MAX/4`): `insphere` sums three squared differences internally
/// (`adw = adx^2 + ady^2 + adz^2`), an even narrower structural
/// product-ceiling than `incircle`'s. `1e30` (matching
/// `extreme_large_scale_does_not_panic`'s own magnitude) stays within
/// insphere's actual safe range while still creating genuine intra-call
/// magnitude spread against the tiny sibling coordinates.
#[test]
fn mixed_huge_and_tiny_magnitude_does_not_panic() {
    let a = pt(1e30, 0.0, 0.0);
    let b = pt(-1e30, 1e-10, 0.0);
    let c = pt(0.0, 1e-10, 1e-10);
    let d = pt(0.0, 0.0, 1e-10);
    let e = pt(0.0, 0.0, 0.0);
    let _ = insphere(a, b, c, d, e);
}
