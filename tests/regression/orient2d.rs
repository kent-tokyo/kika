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

use kika::{Orientation, Point2, orient2d};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
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
