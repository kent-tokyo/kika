use super::Point2;
use crate::error::KikaError;
use core::ops::{Add, Mul, Neg, Sub};

/// A 2D displacement with finite `f64` components.
///
/// `Vector2` can only be constructed via [`Vector2::new`] (or by
/// subtracting two [`Point2`]s), which reject NaN and infinite
/// components. Arithmetic between already-finite `Vector2`/`Point2`
/// values (`+`, `-`, scaling) does not re-validate its result — the same
/// convention every numeric type in Rust follows (e.g. `f64` addition
/// doesn't return `Result`) — so it is possible, in principle, for two
/// astronomically large finite inputs to add to an infinite result. This
/// is not treated as a boundary: [`Point2::new`]/[`Vector2::new`] are the
/// finiteness boundary (ADR-003), not every subsequent arithmetic op.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector2 {
    x: f64,
    y: f64,
}

impl Vector2 {
    /// Creates a new vector, rejecting NaN and infinite components.
    ///
    /// ```
    /// use kika::Vector2;
    /// assert!(Vector2::new(1.0, 2.0).is_ok());
    /// assert!(Vector2::new(f64::NAN, 0.0).is_err());
    /// ```
    pub fn new(x: f64, y: f64) -> Result<Self, KikaError> {
        if x.is_finite() && y.is_finite() {
            Ok(Vector2 { x, y })
        } else {
            Err(KikaError::NonFiniteCoordinate)
        }
    }

    #[inline]
    pub fn x(&self) -> f64 {
        self.x
    }

    #[inline]
    pub fn y(&self) -> f64 {
        self.y
    }
}

impl Add for Vector2 {
    type Output = Vector2;
    fn add(self, rhs: Vector2) -> Vector2 {
        Vector2 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vector2 {
    type Output = Vector2;
    fn sub(self, rhs: Vector2) -> Vector2 {
        Vector2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Neg for Vector2 {
    type Output = Vector2;
    fn neg(self) -> Vector2 {
        Vector2 {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl Mul<f64> for Vector2 {
    type Output = Vector2;
    fn mul(self, rhs: f64) -> Vector2 {
        Vector2 {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Add<Vector2> for Point2 {
    type Output = Point2;
    fn add(self, rhs: Vector2) -> Point2 {
        Point2::new_unchecked(self.x() + rhs.x, self.y() + rhs.y)
    }
}

impl Sub<Vector2> for Point2 {
    type Output = Point2;
    fn sub(self, rhs: Vector2) -> Point2 {
        Point2::new_unchecked(self.x() - rhs.x, self.y() - rhs.y)
    }
}

impl Sub for Point2 {
    type Output = Vector2;
    fn sub(self, rhs: Point2) -> Vector2 {
        Vector2 {
            x: self.x() - rhs.x(),
            y: self.y() - rhs.y(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vector2 {
        Vector2::new(x, y).unwrap()
    }
    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn rejects_non_finite() {
        assert_eq!(
            Vector2::new(f64::NAN, 0.0),
            Err(KikaError::NonFiniteCoordinate)
        );
        assert_eq!(
            Vector2::new(0.0, f64::INFINITY),
            Err(KikaError::NonFiniteCoordinate)
        );
    }

    #[test]
    fn point_vector_affine_arithmetic() {
        assert_eq!(p(1.0, 2.0) + v(3.0, 4.0), p(4.0, 6.0));
        assert_eq!(p(4.0, 6.0) - v(3.0, 4.0), p(1.0, 2.0));
        assert_eq!(p(4.0, 6.0) - p(1.0, 2.0), v(3.0, 4.0));
    }

    #[test]
    fn vector_arithmetic() {
        assert_eq!(v(1.0, 2.0) + v(3.0, 4.0), v(4.0, 6.0));
        assert_eq!(v(3.0, 4.0) - v(1.0, 2.0), v(2.0, 2.0));
        assert_eq!(-v(1.0, -2.0), v(-1.0, 2.0));
        assert_eq!(v(1.0, 2.0) * 3.0, v(3.0, 6.0));
    }
}
