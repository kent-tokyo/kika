//! Adversarial and property tests for `incircle`. See
//! `tests/adversarial/orient2d.rs` for the rationale.

use kika::{Point2, Sign, incircle};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// Swapping two of the three defining points (a, b, c) flips the sign
/// (determinant row swap); d is not a row and does not participate in
/// this antisymmetry.
#[test]
fn swap_abc_flips_sign() {
    let (a, b, c, d) = (pt(1.0, 0.0), pt(0.0, 1.0), pt(-1.0, 0.0), pt(0.0, 0.0));
    assert_eq!(incircle(a, b, c, d).negate(), incircle(b, a, c, d));
    assert_eq!(incircle(a, b, c, d).negate(), incircle(a, c, b, d));
}

/// Translation by an exactly-representable vector must not change the
/// sign (power-of-two coordinates/translations: no new rounding).
#[test]
fn translation_invariance() {
    let (a, b, c, d) = (pt(8.0, 0.0), pt(0.0, 8.0), pt(-8.0, 0.0), pt(0.0, 0.0));
    let want = incircle(a, b, c, d);
    for &(tx, ty) in &[(32.0, 0.0), (0.0, -64.0), (16.0, 16.0)] {
        let t = |p: Point2| pt(p.x() + tx, p.y() + ty);
        assert_eq!(
            incircle(t(a), t(b), t(c), t(d)),
            want,
            "translation ({tx},{ty})"
        );
    }
}

/// Positive uniform scaling must not change the sign (power-of-two
/// scales: no new rounding).
#[test]
fn positive_uniform_scale_invariance() {
    let (a, b, c, d) = (pt(8.0, 0.0), pt(0.0, 8.0), pt(-8.0, 0.0), pt(0.0, 0.0));
    let want = incircle(a, b, c, d);
    for &s in &[0.5_f64, 2.0, 1024.0, 1.0 / 1024.0] {
        let scale = |p: Point2| pt(p.x() * s, p.y() * s);
        assert_eq!(
            incircle(scale(a), scale(b), scale(c), scale(d)),
            want,
            "scale {s}"
        );
    }
}

#[test]
fn cocircular_at_various_scales() {
    for &scale in &[1.0_f64, 1e-20, 1e20] {
        let a = pt(scale, 0.0);
        let b = pt(0.0, scale);
        let c = pt(-scale, 0.0);
        let d = pt(0.0, -scale);
        assert_eq!(incircle(a, b, c, d), Sign::Zero);
    }
}

#[test]
fn duplicate_point_variants_are_zero() {
    let p = pt(1.0, 1.0);
    let q = pt(2.0, 3.0);
    let r = pt(5.0, -1.0);
    assert_eq!(incircle(p, p, q, r), Sign::Zero);
    assert_eq!(incircle(q, p, p, r), Sign::Zero);
    assert_eq!(incircle(q, r, p, p), Sign::Zero);
}

#[test]
fn near_subnormal_scale_does_not_panic() {
    let scale = 1e-70_f64;
    let a = pt(scale, 0.0);
    let b = pt(0.0, scale);
    let c = pt(-scale, 0.0);
    let d = pt(0.0, 0.0);
    // See docs/numerical-model.md "Known limitation: incircle/insphere
    // have a narrower safe magnitude range" — correctness at this scale
    // is not asserted here, only that it doesn't panic.
    let _ = incircle(a, b, c, d);
}

#[test]
fn extreme_large_scale_does_not_panic() {
    let scale = 1e70_f64;
    let a = pt(scale, 0.0);
    let b = pt(0.0, scale);
    let c = pt(-scale, 0.0);
    let d = pt(0.0, 0.0);
    let _ = incircle(a, b, c, d);
}

/// Mixed huge + tiny coordinates in one call, mirroring the shape of the
/// 0.7.1 `delaunay2()` panic's repro triples
/// (`tests/regression/orient2d.rs`) rather than the *uniform* extreme
/// magnitude `extreme_large_scale_does_not_panic` above already covers.
///
/// Unlike `orient2d`/`orient3d`, this can't be pushed all the way to
/// `rescale_for_sign_only`'s own trigger threshold (`f64::MAX/4`):
/// `incircle` squares each coordinate difference internally
/// (`adz = adx^2 + ady^2`), so its structural product-ceiling
/// (`|adx| < f64::MAX.sqrt() ~= 1.34e154`, else `adx^2` itself overflows)
/// is reached at a *much* lower magnitude than the two_sum-overflow fix's
/// own trigger — confirmed empirically: `1e308` here hits that squaring
/// ceiling (a distinct, deliberately deferred limitation, same as
/// `extreme_large_scale_does_not_panic`'s own uniform-magnitude case)
/// before the two_sum fix would ever engage. `1e70` (matching
/// `extreme_large_scale_does_not_panic`'s own magnitude) stays within
/// incircle's actual safe range while still creating genuine intra-call
/// magnitude spread against the tiny sibling coordinates.
#[test]
fn mixed_huge_and_tiny_magnitude_does_not_panic() {
    let a = pt(1e70, 0.0);
    let b = pt(-1e70, 1e-10);
    let c = pt(0.0, 1e-10);
    let d = pt(0.0, 0.0);
    let _ = incircle(a, b, c, d);
}
