use super::Point2;

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

impl Segment2 {
    pub fn new(a: Point2, b: Point2) -> Self {
        Segment2 { a, b }
    }

    #[inline]
    pub fn a(&self) -> Point2 {
        self.a
    }

    #[inline]
    pub fn b(&self) -> Point2 {
        self.b
    }

    /// `true` iff both endpoints are the same point (exact equality, see
    /// ADR-003's point equality policy).
    pub fn is_zero_length(&self) -> bool {
        self.a == self.b
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
}
