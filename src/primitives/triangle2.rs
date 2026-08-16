use super::{Point2, PointSegmentRelation, Segment2};
use crate::predicates::{Orientation, orient2d};

/// A 2D triangle with vertices `a`, `b`, `c`, in the order given (no
/// implied winding — a degenerate, collinear-vertex triangle is a valid,
/// representable `Triangle2`, not rejected).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle2 {
    a: Point2,
    b: Point2,
    c: Point2,
}

impl Triangle2 {
    pub fn new(a: Point2, b: Point2, c: Point2) -> Self {
        Triangle2 { a, b, c }
    }

    #[inline]
    pub fn a(&self) -> Point2 {
        self.a
    }

    #[inline]
    pub fn b(&self) -> Point2 {
        self.b
    }

    #[inline]
    pub fn c(&self) -> Point2 {
        self.c
    }

    /// The triangle's winding: `orient2d(a, b, c)`. `Orientation::Collinear`
    /// means the three vertices are degenerate (don't form a real
    /// triangle).
    pub fn orientation(&self) -> Orientation {
        orient2d(self.a, self.b, self.c)
    }

    /// Classifies `p`'s position relative to this triangle. Exact: the
    /// standard "same side of every edge" test, built entirely from
    /// `orient2d` — works for either winding (CW or CCW).
    ///
    /// A degenerate (collinear-vertex) triangle needs its own case: with
    /// `a`, `b`, `c` collinear, all three edge checks are trivially
    /// `Collinear` for *any* `p` on that shared line, regardless of
    /// whether `p` actually falls within the span the three points cover
    /// — the general test alone cannot tell "on the degenerate triangle"
    /// from "on the same line, way outside it" apart (caught by a test
    /// with `p` far outside that span on the shared line, initially
    /// wrongly classified `OnBoundary`). So: a degenerate triangle has no
    /// interior, and `p` is `OnBoundary` iff it lies on at least one of
    /// the three point-pairs' segments (whose union always covers the
    /// full span of three collinear points, regardless of their order
    /// along the line) — otherwise `Outside`.
    pub fn relation_to(&self, p: Point2) -> PointTriangleRelation {
        if self.orientation() == Orientation::Collinear {
            let on_any_edge = [
                Segment2::new(self.a, self.b),
                Segment2::new(self.b, self.c),
                Segment2::new(self.c, self.a),
            ]
            .iter()
            .any(|edge| edge.relation_to(p) != PointSegmentRelation::NotOnSegment);
            return if on_any_edge {
                PointTriangleRelation::OnBoundary
            } else {
                PointTriangleRelation::Outside
            };
        }

        let edges = [
            orient2d(self.a, self.b, p),
            orient2d(self.b, self.c, p),
            orient2d(self.c, self.a, p),
        ];
        let has_ccw = edges.contains(&Orientation::CounterClockwise);
        let has_cw = edges.contains(&Orientation::Clockwise);

        if has_ccw && has_cw {
            PointTriangleRelation::Outside
        } else if edges.contains(&Orientation::Collinear) {
            PointTriangleRelation::OnBoundary
        } else {
            PointTriangleRelation::Inside
        }
    }
}

/// Where a point lies relative to a [`Triangle2`], per
/// [`Triangle2::relation_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointTriangleRelation {
    Inside,
    /// On an edge (including exactly at a vertex).
    OnBoundary,
    Outside,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn orientation_matches_orient2d() {
        let t = Triangle2::new(p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0));
        assert_eq!(t.orientation(), Orientation::CounterClockwise);
    }

    #[test]
    fn degenerate_triangle_is_collinear() {
        let t = Triangle2::new(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0));
        assert_eq!(t.orientation(), Orientation::Collinear);
    }

    #[test]
    fn relation_to_ccw_triangle() {
        let t = Triangle2::new(p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0));
        assert_eq!(t.relation_to(p(1.0, 1.0)), PointTriangleRelation::Inside);
        assert_eq!(
            t.relation_to(p(0.0, 0.0)),
            PointTriangleRelation::OnBoundary
        );
        assert_eq!(
            t.relation_to(p(2.0, 0.0)),
            PointTriangleRelation::OnBoundary
        );
        assert_eq!(
            t.relation_to(p(2.0, 2.0)),
            PointTriangleRelation::OnBoundary
        );
        assert_eq!(t.relation_to(p(-1.0, -1.0)), PointTriangleRelation::Outside);
        assert_eq!(t.relation_to(p(5.0, 5.0)), PointTriangleRelation::Outside);
        assert_eq!(t.relation_to(p(3.0, 3.0)), PointTriangleRelation::Outside);
        assert_eq!(t.relation_to(p(2.0, -1.0)), PointTriangleRelation::Outside);
    }

    #[test]
    fn relation_to_cw_triangle_same_as_ccw() {
        // Same triangle, opposite winding: relation_to must not care.
        let t = Triangle2::new(p(0.0, 0.0), p(0.0, 4.0), p(4.0, 0.0));
        assert_eq!(t.relation_to(p(1.0, 1.0)), PointTriangleRelation::Inside);
        assert_eq!(t.relation_to(p(5.0, 5.0)), PointTriangleRelation::Outside);
    }

    #[test]
    fn degenerate_triangle_never_contains_off_line_point() {
        let t = Triangle2::new(p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0));
        assert_eq!(
            t.relation_to(p(0.5, 0.0)),
            PointTriangleRelation::OnBoundary
        );
        assert_eq!(t.relation_to(p(0.5, 1.0)), PointTriangleRelation::Outside);
        assert_eq!(t.relation_to(p(10.0, 0.0)), PointTriangleRelation::Outside);
    }
}
