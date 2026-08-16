use super::Point2;
use crate::predicates::{Orientation, orient2d};

/// A 2D line segment between two points.
///
/// No validation beyond what [`Point2`] already guarantees (finite
/// coordinates): a zero-length segment (`a == b`) is a valid,
/// representable `Segment2`, not rejected — degenerate segments are
/// handled explicitly by the algorithms that consume them (see
/// `docs/degeneracy-policy.md`), not disallowed at construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment2 {
    a: Point2,
    b: Point2,
}

/// Where a point lies relative to a [`Segment2`], per [`Segment2::relation_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointSegmentRelation {
    /// Exactly one of the segment's endpoints.
    Endpoint,
    /// Collinear with, and strictly between, the two endpoints.
    Interior,
    /// Not on the segment (off the line, or collinear but outside the
    /// endpoint range).
    NotOnSegment,
}

impl Segment2 {
    /// Creates a segment between `a` and `b`. `a == b` (zero-length) is
    /// allowed — see the type's doc comment.
    pub fn new(a: Point2, b: Point2) -> Self {
        Segment2 { a, b }
    }

    /// The first endpoint.
    #[inline]
    pub fn a(&self) -> Point2 {
        self.a
    }

    /// The second endpoint.
    #[inline]
    pub fn b(&self) -> Point2 {
        self.b
    }

    /// `true` iff both endpoints are the same point (exact equality, see
    /// ADR-003's point equality policy).
    pub fn is_zero_length(&self) -> bool {
        self.a == self.b
    }

    /// Classifies `p`'s position relative to this segment. Exact: built
    /// entirely from `orient2d` (collinearity) plus direct coordinate
    /// range comparisons on already-finite values (no arithmetic, so no
    /// new rounding) — never a distance/epsilon test.
    ///
    /// A zero-length segment is handled explicitly: `p` is `Endpoint` iff
    /// it equals that single point, `NotOnSegment` otherwise (there is no
    /// "interior" to be on).
    pub fn relation_to(&self, p: Point2) -> PointSegmentRelation {
        if self.is_zero_length() {
            return if p == self.a {
                PointSegmentRelation::Endpoint
            } else {
                PointSegmentRelation::NotOnSegment
            };
        }
        if orient2d(self.a, self.b, p) != Orientation::Collinear {
            return PointSegmentRelation::NotOnSegment;
        }
        if p == self.a || p == self.b {
            return PointSegmentRelation::Endpoint;
        }
        // p is exactly collinear with a non-degenerate a-b line: checking
        // whichever axis actually varies is sufficient to determine
        // betweenness (see src/primitives/segment2.rs module tests for
        // the differential coverage backing this).
        let within = if self.a.x() != self.b.x() {
            let (lo, hi) = (self.a.x().min(self.b.x()), self.a.x().max(self.b.x()));
            lo <= p.x() && p.x() <= hi
        } else {
            let (lo, hi) = (self.a.y().min(self.b.y()), self.a.y().max(self.b.y()));
            lo <= p.y() && p.y() <= hi
        };
        if within {
            PointSegmentRelation::Interior
        } else {
            PointSegmentRelation::NotOnSegment
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn accessors() {
        let s = Segment2::new(p(0.0, 0.0), p(1.0, 1.0));
        assert_eq!(s.a(), p(0.0, 0.0));
        assert_eq!(s.b(), p(1.0, 1.0));
    }

    #[test]
    fn zero_length_detection() {
        assert!(Segment2::new(p(1.0, 1.0), p(1.0, 1.0)).is_zero_length());
        assert!(!Segment2::new(p(0.0, 0.0), p(1.0, 1.0)).is_zero_length());
    }

    #[test]
    fn relation_to_basic_cases() {
        let s = Segment2::new(p(0.0, 0.0), p(4.0, 0.0));
        assert_eq!(s.relation_to(p(0.0, 0.0)), PointSegmentRelation::Endpoint);
        assert_eq!(s.relation_to(p(4.0, 0.0)), PointSegmentRelation::Endpoint);
        assert_eq!(s.relation_to(p(2.0, 0.0)), PointSegmentRelation::Interior);
        assert_eq!(
            s.relation_to(p(5.0, 0.0)),
            PointSegmentRelation::NotOnSegment
        );
        assert_eq!(
            s.relation_to(p(-1.0, 0.0)),
            PointSegmentRelation::NotOnSegment
        );
        assert_eq!(
            s.relation_to(p(2.0, 1.0)),
            PointSegmentRelation::NotOnSegment
        );
    }

    #[test]
    fn relation_to_vertical_segment() {
        // Exercises the a.x()==b.x() branch (range check falls back to y).
        let s = Segment2::new(p(3.0, -2.0), p(3.0, 5.0));
        assert_eq!(s.relation_to(p(3.0, 0.0)), PointSegmentRelation::Interior);
        assert_eq!(
            s.relation_to(p(3.0, 10.0)),
            PointSegmentRelation::NotOnSegment
        );
        assert_eq!(
            s.relation_to(p(4.0, 0.0)),
            PointSegmentRelation::NotOnSegment
        );
    }

    #[test]
    fn relation_to_zero_length_segment() {
        let s = Segment2::new(p(2.0, 2.0), p(2.0, 2.0));
        assert_eq!(s.relation_to(p(2.0, 2.0)), PointSegmentRelation::Endpoint);
        assert_eq!(
            s.relation_to(p(2.0, 3.0)),
            PointSegmentRelation::NotOnSegment
        );
    }

    #[test]
    fn relation_to_diagonal_and_extreme_scale() {
        for &scale in &[1.0_f64, 1e-100, 1e100] {
            let s = Segment2::new(p(0.0, 0.0), p(scale, scale));
            assert_eq!(
                s.relation_to(p(scale * 0.5, scale * 0.5)),
                PointSegmentRelation::Interior,
                "scale {scale}"
            );
            assert_eq!(
                s.relation_to(p(scale * 2.0, scale * 2.0)),
                PointSegmentRelation::NotOnSegment,
                "scale {scale}"
            );
        }
    }
}
