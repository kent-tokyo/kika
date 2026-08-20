//! Minimized regression fixtures for real bugs found during development.
//! Per AGENTS.md §12/§18: fuzz/bug-hunting output gets reduced to a small
//! fixture and pinned here, not just left in a corpus or a differential
//! test's random sweep.
//!
//! ## Found: exact fallback was not exact relative to the original
//! coordinates
//!
//! `orient2d`'s exact fallback used to build on the filter's already
//! rounded `acx = a.x() - c.x()` (a single f64 subtraction), doing exact
//! arithmetic only from that point on. That is exact relative to the
//! *rounded* difference, not the original input coordinates — the
//! subtraction itself can discard information when the two coordinates
//! have very different magnitudes (e.g. `a.x() = 2^60`, `c.x() = 1.0`:
//! `fl(a.x() - c.x()) == 2^60`, the `-1.0` is silently lost). All four
//! cases below were found by a random search comparing this
//! once-rounded-then-exact computation against a fully exact computation
//! from the original coordinates; all four returned `Collinear` before
//! the fix (`src/predicates/orient2d.rs`, `orient2d_exact` now builds
//! `acx` etc. as exact expansions via `diff_expansion` from the start).
//! See `docs/numerical-model.md` "Known limitation: exactness starts at
//! the original coordinates".

use kika::{Orientation, Point2, delaunay2, orient2d};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// Checks `orient2d`'s antisymmetry across all 6 permutations of `(a, b,
/// c)`: the 3 cyclic permutations (`abc`, `bca`, `cab`) must all agree with
/// each other, the 3 transpositions (`acb`, `bac`, `cba`) must all agree
/// with each other, and the two groups must be opposite (or both
/// `Collinear`) — the standard antisymmetry of a determinant/signed area
/// under permutation of its rows, not just under a single swap.
fn assert_all_permutations_consistent(a: Point2, b: Point2, c: Point2) {
    let cyclic = [orient2d(a, b, c), orient2d(b, c, a), orient2d(c, a, b)];
    let transposed = [orient2d(a, c, b), orient2d(b, a, c), orient2d(c, b, a)];

    for &o in &cyclic[1..] {
        assert_eq!(
            o, cyclic[0],
            "cyclic permutations disagree: {cyclic:?} for ({a:?}, {b:?}, {c:?})"
        );
    }
    for &o in &transposed[1..] {
        assert_eq!(
            o, transposed[0],
            "transposed permutations disagree: {transposed:?} for ({a:?}, {b:?}, {c:?})"
        );
    }
    let expected_transposed = match cyclic[0] {
        Orientation::Clockwise => Orientation::CounterClockwise,
        Orientation::CounterClockwise => Orientation::Clockwise,
        Orientation::Collinear => Orientation::Collinear,
    };
    assert_eq!(
        transposed[0], expected_transposed,
        "transposition did not flip orientation: cyclic={cyclic:?} transposed={transposed:?}"
    );
}

#[test]
fn exact_fallback_uses_original_coordinates_not_rounded_differences() {
    let p50 = 2.0_f64.powi(50);
    let p60 = 2.0_f64.powi(60);

    let cases: &[(Point2, Point2, Point2, Orientation)] = &[
        (
            pt(-8.0, -9.0),
            pt(-p50, -p50),
            pt(-p60, -p60),
            Orientation::CounterClockwise,
        ),
        (
            pt(p60, -2.0),
            pt(p60, 9.0),
            pt(-p60, p60),
            Orientation::CounterClockwise,
        ),
        (
            pt(p50, -5.0),
            pt(p50, -6.0),
            pt(9.0, p60),
            Orientation::Clockwise,
        ),
        (
            pt(4.0, 7.0),
            pt(-3.0, 1.0),
            pt(-p60, p60),
            Orientation::Clockwise,
        ),
    ];

    for &(a, b, c, expected) in cases {
        let got = orient2d(a, b, c);
        assert_eq!(got, expected, "orient2d({a:?}, {b:?}, {c:?})");
        assert_ne!(
            got,
            Orientation::Collinear,
            "regression: this case used to incorrectly return Collinear"
        );
    }
}

/// ## Found: `orient2d` permutation-inconsistent at extreme mixed
/// coordinate magnitude, breaking `delaunay2()`'s "first 3 non-collinear
/// points" search
///
/// The 0.7.0 `delaunay2()` panic (`index out of bounds`), documented in
/// `CHANGELOG.md`'s 0.7.0 "Known issues" and fixed in 0.7.1. Two
/// independent overflow sites in the exact-fallback arithmetic core
/// (`predicates::expansion`) could each corrupt an exact-expansion
/// determinant with `NaN`, which `expansion_sign` silently read as
/// `Sign::Zero` (`Orientation::Collinear`) — breaking the antisymmetry
/// `delaunay2()`'s search relies on:
///
/// - Repro #1: `split()`'s `SPLITTER * a` step overflowed to `Infinity`
///   for `|a| > f64::MAX/SPLITTER ~= 1.34e300` (`p1`'s y-coordinate here).
///   Found by `fuzz/fuzz_targets/voronoi_geometry.rs`'s first-ever run.
/// - Repro #2: `two_sum`'s `a + b` (inside `diff_expansion`) overflowed
///   for opposite-sign coordinates each within a small factor of
///   `f64::MAX`, whose true difference itself exceeds `f64::MAX`. Found
///   independently while diagnosing repro #1.
///
/// Both fixed via `predicates::expansion::split`'s overflow-safe rescale
/// and the new `rescale_for_sign_only` helper — see their doc comments and
/// `docs/numerical-model.md`.
#[test]
fn permutation_consistent_at_extreme_mixed_magnitude() {
    // Repro #1.
    let p0 = pt(4.523334248222805e-282, 6.612169496581129e-281);
    let p1 = pt(3.2186699543901864e-57, -4.251746146807175e304);
    let p2 = pt(2.247760886104758e-307, 1.3683225479033359e-48);
    assert_all_permutations_consistent(p0, p1, p2);
    delaunay2(&[p0, p1, p2]);

    // Repro #2.
    let a = pt(1e308, 0.0);
    let b = pt(-1e308, 1e-10);
    let c = pt(0.0, 1e-10);
    assert_all_permutations_consistent(a, b, c);
    delaunay2(&[a, b, c]);
}
