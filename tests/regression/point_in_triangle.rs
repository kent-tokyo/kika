//! Minimized regression fixture for a real bug found during development.
//! See `tests/regression/orient2d.rs` for the convention.
//!
//! ## Found: degenerate triangle couldn't distinguish "on its span" from
//! "on the same line, far outside it"
//!
//! `Triangle2::relation_to`'s general algorithm (3 `orient2d` edge-side
//! checks) is correct for non-degenerate triangles, but for a degenerate
//! (collinear-vertex) triangle, all three checks are trivially
//! `Orientation::Collinear` for *any* point on the shared line —
//! regardless of whether that point is anywhere near the three vertices.
//! `t.relation_to((10.0, 0.0))` for a degenerate triangle spanning
//! `x ∈ [0, 2]` on the x-axis used to return `OnBoundary` (wrong — (10,
//! 0) is nowhere near the triangle) instead of `Outside`. Fixed in
//! `src/primitives/triangle2.rs` with an explicit degenerate case using
//! `Segment2::relation_to` (range membership) instead of relying on the
//! general orientation-based test. See `docs/degeneracy-policy.md`.

use kika::{Point2, PointTriangleRelation, Triangle2};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[test]
fn degenerate_triangle_point_far_outside_span_on_shared_line() {
    let t = Triangle2::new(pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0));

    // Before the fix, this incorrectly returned OnBoundary.
    assert_eq!(t.relation_to(pt(10.0, 0.0)), PointTriangleRelation::Outside);
    assert_eq!(t.relation_to(pt(-5.0, 0.0)), PointTriangleRelation::Outside);

    // Within the degenerate span: still OnBoundary.
    assert_eq!(
        t.relation_to(pt(0.5, 0.0)),
        PointTriangleRelation::OnBoundary
    );
    assert_eq!(
        t.relation_to(pt(0.0, 0.0)),
        PointTriangleRelation::OnBoundary
    );
    assert_eq!(
        t.relation_to(pt(2.0, 0.0)),
        PointTriangleRelation::OnBoundary
    );
}
