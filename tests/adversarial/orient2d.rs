//! Adversarial and property tests for `orient2d`, per AGENTS.md §11's
//! required property list and §9 Phase 1's degeneracy categories. These
//! are self-consistency checks against the public API only — differential
//! comparison against the independent bigint oracle lives in
//! `tests/differential/orient2d.rs`.

use kika::{Orientation, Point2, orient2d};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn flip(o: Orientation) -> Orientation {
    match o {
        Orientation::Clockwise => Orientation::CounterClockwise,
        Orientation::CounterClockwise => Orientation::Clockwise,
        Orientation::Collinear => Orientation::Collinear,
    }
}

/// `orient2d(a,b,c) == -orient2d(b,a,c)` (AGENTS.md §11).
#[test]
fn swap_ab_flips_sign() {
    let triples = [
        (pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)),
        (pt(-3.0, 5.0), pt(2.0, -1.0), pt(7.0, 7.0)),
        (pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)), // collinear
        (pt(1.0, 1.0), pt(1.0, 1.0), pt(2.0, 3.0)), // duplicate
    ];
    for (a, b, c) in triples {
        assert_eq!(orient2d(a, b, c), flip(orient2d(b, a, c)));
    }
}

/// Translation by an exactly-representable vector must not change the
/// sign. Uses power-of-two coordinates and translations so the sum is
/// computed without any new rounding, isolating the invariance property
/// itself from unrelated floating-point representation effects.
#[test]
fn translation_invariance() {
    let base = [
        (pt(0.0, 0.0), pt(8.0, 0.0), pt(0.0, 8.0)),
        (pt(-4.0, 2.0), pt(4.0, -2.0), pt(1.0, 16.0)),
    ];
    let translations = [(0.0, 0.0), (32.0, 0.0), (0.0, -64.0), (16.0, 16.0)];
    for (a, b, c) in base {
        let want = orient2d(a, b, c);
        for &(tx, ty) in &translations {
            let ta = pt(a.x() + tx, a.y() + ty);
            let tb = pt(b.x() + tx, b.y() + ty);
            let tc = pt(c.x() + tx, c.y() + ty);
            assert_eq!(orient2d(ta, tb, tc), want, "translation ({tx}, {ty})");
        }
    }
}

/// Positive uniform scaling must not change the sign. Uses power-of-two
/// scales so multiplication introduces no new rounding.
#[test]
fn positive_uniform_scale_invariance() {
    let base = [
        (pt(0.0, 0.0), pt(8.0, 0.0), pt(0.0, 8.0)),
        (pt(-4.0, 2.0), pt(4.0, -2.0), pt(1.0, 16.0)),
    ];
    let scales = [0.5_f64, 2.0, 1024.0, 1.0 / 1024.0];
    for (a, b, c) in base {
        let want = orient2d(a, b, c);
        for &s in &scales {
            let sa = pt(a.x() * s, a.y() * s);
            let sb = pt(b.x() * s, b.y() * s);
            let sc = pt(c.x() * s, c.y() * s);
            assert_eq!(orient2d(sa, sb, sc), want, "scale {s}");
        }
    }
}

#[test]
fn all_points_collinear_various_scales() {
    for &scale in &[1.0_f64, 1e-20, 1e20] {
        let a = pt(0.0, 0.0);
        let b = pt(scale, scale);
        let c = pt(2.0 * scale, 2.0 * scale);
        assert_eq!(orient2d(a, b, c), Orientation::Collinear);
    }
}

#[test]
fn zero_length_segment_is_collinear() {
    let p = pt(1.0, 1.0);
    assert_eq!(orient2d(p, p, pt(2.0, 2.0)), Orientation::Collinear);
    assert_eq!(orient2d(p, pt(2.0, 2.0), p), Orientation::Collinear);
    assert_eq!(orient2d(pt(2.0, 2.0), p, p), Orientation::Collinear);
}

/// Tiny-but-safe coordinate differences (see
/// `docs/numerical-model.md` "Known limitation" for why "safe" is bounded)
/// must not panic and must agree with a straightforward sign check.
#[test]
fn near_subnormal_scale_does_not_panic() {
    let scale = 1e-140_f64;
    let a = pt(0.0, 0.0);
    let b = pt(scale, 0.0);
    let c = pt(0.0, scale);
    assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
    assert_eq!(orient2d(b, a, c), Orientation::Clockwise);
}

#[test]
fn extreme_large_scale_does_not_panic() {
    let scale = 1e150_f64;
    let a = pt(0.0, 0.0);
    let b = pt(scale, 0.0);
    let c = pt(0.0, scale);
    assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
}

/// Full antisymmetry under all 6 permutations of `(a, b, c)` — a genuine
/// strengthening of `swap_ab_flips_sign` above, which only checks a single
/// transposition. The 3 cyclic permutations must all agree with each
/// other; the 3 (odd) transpositions must all agree with each other and be
/// the flip of the cyclic group. Found wanting for exactly this property
/// at extreme mixed magnitude — see
/// `tests/regression/orient2d.rs`'s `permutation_consistent_at_extreme_mixed_magnitude`
/// for the specific bug this generalizes from. Magnitudes here stay
/// `<= 1e150`, comfortably below `orient2d`'s own degree-2 structural
/// ceiling (`f64::MAX^(1/2) ~= 1.34e154`) — the still-deferred
/// out-of-scope product-ceiling case, not this property, is covered
/// separately (see `docs/numerical-model.md`).
#[test]
fn six_permutation_consistency_general() {
    let triples = [
        (pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)),
        (pt(-3.0, 5.0), pt(2.0, -1.0), pt(7.0, 7.0)),
        (pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0)), // collinear
        (pt(1e-140, 0.0), pt(0.0, 1e140), pt(1.0, 1.0)), // mixed magnitude
        (pt(1e150, 0.0), pt(0.0, 1e150), pt(1.0, -1.0)), // near degree-2 ceiling
    ];
    for (a, b, c) in triples {
        let cyclic = [orient2d(a, b, c), orient2d(b, c, a), orient2d(c, a, b)];
        let transposed = [orient2d(a, c, b), orient2d(b, a, c), orient2d(c, b, a)];
        for &o in &cyclic[1..] {
            assert_eq!(
                o, cyclic[0],
                "cyclic permutations disagree for ({a:?},{b:?},{c:?})"
            );
        }
        for &o in &transposed[1..] {
            assert_eq!(
                o, transposed[0],
                "transposed permutations disagree for ({a:?},{b:?},{c:?})"
            );
        }
        assert_eq!(transposed[0], flip(cyclic[0]));
    }
}
