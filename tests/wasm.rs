//! WASM execution tests (not just build). Runs under `wasm-pack test
//! --node --release` (Node.js is the default execution target for
//! `wasm-bindgen-test` -- no `wasm_bindgen_test_configure!` call needed;
//! that macro is only for opting into `run_in_browser` instead, which
//! this file deliberately doesn't use) -- deliberately gated to `wasm32`
//! only: `wasm-bindgen-test` is a
//! `wasm32`-only dev-dependency (`Cargo.toml`'s
//! `[target.'cfg(target_arch = "wasm32")'.dev-dependencies]`), so this
//! whole file compiles to an empty, trivially-passing test binary on every
//! other target rather than failing to find the crate.
//!
//! `docs/compatibility.md` previously noted wasm32 as "builds, not
//! executed" -- ADR-001's "Rust never contracts +/-/* into FMA" argument
//! (load-bearing for the exact-arithmetic core's correctness) is a
//! language-level guarantee, not something that needed re-verifying per
//! target, but actually running the predicate/construction/triangulation
//! code under a real wasm32 runtime is still worth doing once, not just
//! assumed from a successful build. This file closes that gap with a
//! deliberately small set of load-bearing cases -- one per major
//! subsystem, not a port of the 274 native tests (native coverage is
//! already exhaustive; this exists to catch wasm32-*specific*
//! codegen/execution differences, which a handful of representative cases
//! is enough to surface).

#![cfg(target_arch = "wasm32")]

use kika::{
    CdtError, Orientation, Point2, Point3, Polygon2, Segment2, SegmentIntersection2, Sign,
    constrained_delaunay2, delaunay2, incircle, insphere, orient2d, orient3d, segment_intersection,
    triangulate_polygon, triangulate_polygon_with_holes,
};
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn orient2d_basic_sign() {
    let a = Point2::new(0.0, 0.0).unwrap();
    let b = Point2::new(1.0, 0.0).unwrap();
    let c = Point2::new(0.0, 1.0).unwrap();
    assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
    assert_eq!(orient2d(a, c, b), Orientation::Clockwise);
}

#[wasm_bindgen_test]
fn orient3d_basic_sign() {
    let a = Point3::new(0.0, 0.0, 0.0).unwrap();
    let b = Point3::new(1.0, 0.0, 0.0).unwrap();
    let c = Point3::new(0.0, 1.0, 0.0).unwrap();
    let d = Point3::new(0.0, 0.0, 1.0).unwrap();
    // Same sign convention exercised in the native unit tests.
    assert_ne!(orient3d(a, b, c, d), Sign::Zero);
}

#[wasm_bindgen_test]
fn incircle_basic_case() {
    let a = Point2::new(0.0, 0.0).unwrap();
    let b = Point2::new(4.0, 0.0).unwrap();
    let c = Point2::new(0.0, 4.0).unwrap();
    let inside = Point2::new(1.0, 1.0).unwrap();
    let outside = Point2::new(10.0, 10.0).unwrap();
    assert_eq!(incircle(a, b, c, inside), Sign::Positive);
    assert_eq!(incircle(a, b, c, outside), Sign::Negative);
}

#[wasm_bindgen_test]
fn insphere_basic_case() {
    // `insphere`'s inside/outside sign is orientation-dependent on the
    // order of a/b/c/d (see its doc comment: "swapping any two flips the
    // sign") -- this ordering's `orient3d` is `Negative`, so outside is
    // `Positive` and inside is `Negative` here, not the other way around.
    let a = Point3::new(0.0, 0.0, 0.0).unwrap();
    let b = Point3::new(4.0, 0.0, 0.0).unwrap();
    let c = Point3::new(0.0, 4.0, 0.0).unwrap();
    let d = Point3::new(0.0, 0.0, 4.0).unwrap();
    let outside = Point3::new(10.0, 10.0, 10.0).unwrap();
    let inside = Point3::new(1.0, 1.0, 1.0).unwrap();
    assert_eq!(insphere(a, b, c, d, outside), Sign::Positive);
    assert_eq!(insphere(a, b, c, d, inside), Sign::Negative);
}

#[wasm_bindgen_test]
fn segment_intersection_returns_finite_point() {
    let s1 = Segment2::new(
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(4.0, 4.0).unwrap(),
    );
    let s2 = Segment2::new(
        Point2::new(0.0, 4.0).unwrap(),
        Point2::new(4.0, 0.0).unwrap(),
    );
    match segment_intersection(s1, s2) {
        SegmentIntersection2::Point(p) => {
            assert!(p.x().is_finite() && p.y().is_finite());
            assert_eq!(p, Point2::new(2.0, 2.0).unwrap());
        }
        other => panic!("expected a Proper point, got {other:?}"),
    }
}

/// Extreme/mixed-magnitude regression case for the 0.3.0 `line_intersection`
/// overflow fix (`CHANGELOG.md`) -- same shape as
/// `tests/adversarial/segment_intersection.rs`'s
/// `extreme_large_scale_proper_point_is_finite`, run here under an actual
/// wasm32 runtime rather than assumed to behave identically to native.
#[wasm_bindgen_test]
fn extreme_magnitude_line_intersection_is_finite() {
    for &scale in &[1e103_f64, 1e140] {
        let s1 = Segment2::new(
            Point2::new(-scale, 0.0).unwrap(),
            Point2::new(scale, 0.0).unwrap(),
        );
        let s2 = Segment2::new(
            Point2::new(-scale, -scale).unwrap(),
            Point2::new(scale, scale).unwrap(),
        );
        if let SegmentIntersection2::Point(p) = segment_intersection(s1, s2) {
            assert!(
                p.x().is_finite() && p.y().is_finite(),
                "non-finite Proper intersection at scale {scale}: {p:?}"
            );
        }
    }
}

#[wasm_bindgen_test]
fn delaunay_basic_triangle_count_and_topology() {
    let pts = [
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(4.0, 0.0).unwrap(),
        Point2::new(4.0, 4.0).unwrap(),
        Point2::new(0.0, 4.0).unwrap(),
        Point2::new(2.0, 2.0).unwrap(),
    ];
    let t = delaunay2(&pts);
    assert!(t.validate_topology().is_empty());
    assert!(!t.is_empty());
}

#[wasm_bindgen_test]
fn degenerate_cdt_is_a_typed_error_not_a_panic() {
    let pts = [
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(1.0, 0.0).unwrap(),
    ];
    assert_eq!(
        constrained_delaunay2(&pts, &[(0, 1)]),
        Err(CdtError::DegeneratePointSet)
    );
}

#[wasm_bindgen_test]
fn polygon_triangulation_happy_path() {
    let square = Polygon2::new(vec![
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(4.0, 0.0).unwrap(),
        Point2::new(4.0, 4.0).unwrap(),
        Point2::new(0.0, 4.0).unwrap(),
    ]);
    let t = triangulate_polygon(&square).unwrap();
    assert_eq!(t.len(), square.len() - 2);
}

#[wasm_bindgen_test]
fn polygon_triangulation_with_holes_happy_path() {
    let outer = Polygon2::new(vec![
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(4.0, 0.0).unwrap(),
        Point2::new(4.0, 4.0).unwrap(),
        Point2::new(0.0, 4.0).unwrap(),
    ]);
    let hole = Polygon2::new(vec![
        Point2::new(1.0, 1.0).unwrap(),
        Point2::new(1.0, 2.0).unwrap(),
        Point2::new(2.0, 2.0).unwrap(),
        Point2::new(2.0, 1.0).unwrap(),
    ]);
    let t = triangulate_polygon_with_holes(&outer, &[hole]).unwrap();
    assert_eq!(t.len(), 8); // n + 2h - 2 = 8 + 2*1 - 2
}
