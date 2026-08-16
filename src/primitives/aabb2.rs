use super::{Point2, Segment2};

/// An axis-aligned bounding box in 2D: `min.x() <= max.x()` and
/// `min.y() <= max.y()`, enforced by construction (there is no path to
/// build an `Aabb2` with an inverted or otherwise invalid extent).
///
/// Exists primarily as a fast, exact rejection test ahead of expensive
/// exact predicates (§9 Phase 2 "segment bounding-box rejection") — a
/// non-overlapping pair of AABBs proves their segments/shapes cannot
/// intersect, without needing `orient2d` at all. Component comparisons
/// here are exact (`f64 <=`/`>=` on already-finite values), not an
/// epsilon test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb2 {
    min: Point2,
    max: Point2,
}

impl Aabb2 {
    /// Builds the AABB containing exactly `p` and `q`, in either order.
    pub fn from_points(p: Point2, q: Point2) -> Self {
        Aabb2 {
            min: Point2::new_unchecked(p.x().min(q.x()), p.y().min(q.y())),
            max: Point2::new_unchecked(p.x().max(q.x()), p.y().max(q.y())),
        }
    }

    /// Builds the AABB containing exactly `s`'s two endpoints.
    pub fn from_segment(s: Segment2) -> Self {
        Self::from_points(s.a(), s.b())
    }

    /// The box's lower-left corner (component-wise minimum).
    #[inline]
    pub fn min(&self) -> Point2 {
        self.min
    }

    /// The box's upper-right corner (component-wise maximum).
    #[inline]
    pub fn max(&self) -> Point2 {
        self.max
    }

    /// `true` iff the two boxes share at least one point, including
    /// touching at an edge or corner.
    pub fn overlaps(&self, other: &Aabb2) -> bool {
        self.min.x() <= other.max.x()
            && other.min.x() <= self.max.x()
            && self.min.y() <= other.max.y()
            && other.min.y() <= self.max.y()
    }

    /// `true` iff `p` lies inside the box, boundary inclusive.
    pub fn contains_point(&self, p: Point2) -> bool {
        self.min.x() <= p.x()
            && p.x() <= self.max.x()
            && self.min.y() <= p.y()
            && p.y() <= self.max.y()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn from_points_normalizes_order() {
        let a = Aabb2::from_points(p(3.0, 4.0), p(1.0, -2.0));
        assert_eq!(a.min(), p(1.0, -2.0));
        assert_eq!(a.max(), p(3.0, 4.0));
    }

    #[test]
    fn overlap_cases() {
        let a = Aabb2::from_points(p(0.0, 0.0), p(2.0, 2.0));
        let touching = Aabb2::from_points(p(2.0, 0.0), p(4.0, 2.0));
        let disjoint = Aabb2::from_points(p(3.0, 3.0), p(4.0, 4.0));
        assert!(a.overlaps(&touching));
        assert!(!a.overlaps(&disjoint));
        assert!(!disjoint.overlaps(&a));
    }

    #[test]
    fn contains_point_boundary_inclusive() {
        let a = Aabb2::from_points(p(0.0, 0.0), p(2.0, 2.0));
        assert!(a.contains_point(p(0.0, 0.0)));
        assert!(a.contains_point(p(2.0, 2.0)));
        assert!(a.contains_point(p(1.0, 1.0)));
        assert!(!a.contains_point(p(2.1, 1.0)));
    }

    #[test]
    fn zero_length_segment_gives_point_aabb() {
        let s = Segment2::new(p(1.0, 1.0), p(1.0, 1.0));
        let a = Aabb2::from_segment(s);
        assert_eq!(a.min(), a.max());
    }
}
