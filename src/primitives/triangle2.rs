use super::Point2;
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
}
