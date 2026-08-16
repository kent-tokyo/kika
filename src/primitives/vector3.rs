use super::Point3;
use crate::error::KikaError;
use core::ops::{Add, Mul, Neg, Sub};

/// A 3D displacement with finite `f64` components. See [`super::Vector2`]'s
/// doc comment for the arithmetic-doesn't-re-validate convention.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vector3 {
    /// Creates a new vector, rejecting NaN and infinite components.
    ///
    /// ```
    /// use kika::Vector3;
    /// assert!(Vector3::new(1.0, 2.0, 3.0).is_ok());
    /// assert!(Vector3::new(f64::NAN, 0.0, 0.0).is_err());
    /// ```
    pub fn new(x: f64, y: f64, z: f64) -> Result<Self, KikaError> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Vector3 { x, y, z })
        } else {
            Err(KikaError::NonFiniteCoordinate)
        }
    }

    /// The x component.
    #[inline]
    pub fn x(&self) -> f64 {
        self.x
    }

    /// The y component.
    #[inline]
    pub fn y(&self) -> f64 {
        self.y
    }

    /// The z component.
    #[inline]
    pub fn z(&self) -> f64 {
        self.z
    }
}

impl Add for Vector3 {
    type Output = Vector3;
    fn add(self, rhs: Vector3) -> Vector3 {
        Vector3 {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Vector3 {
    type Output = Vector3;
    fn sub(self, rhs: Vector3) -> Vector3 {
        Vector3 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Neg for Vector3 {
    type Output = Vector3;
    fn neg(self) -> Vector3 {
        Vector3 {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

impl Mul<f64> for Vector3 {
    type Output = Vector3;
    fn mul(self, rhs: f64) -> Vector3 {
        Vector3 {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl Add<Vector3> for Point3 {
    type Output = Point3;
    fn add(self, rhs: Vector3) -> Point3 {
        Point3::new_unchecked(self.x() + rhs.x, self.y() + rhs.y, self.z() + rhs.z)
    }
}

impl Sub<Vector3> for Point3 {
    type Output = Point3;
    fn sub(self, rhs: Vector3) -> Point3 {
        Point3::new_unchecked(self.x() - rhs.x, self.y() - rhs.y, self.z() - rhs.z)
    }
}

impl Sub for Point3 {
    type Output = Vector3;
    fn sub(self, rhs: Point3) -> Vector3 {
        Vector3 {
            x: self.x() - rhs.x(),
            y: self.y() - rhs.y(),
            z: self.z() - rhs.z(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> Vector3 {
        Vector3::new(x, y, z).unwrap()
    }
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).unwrap()
    }

    #[test]
    fn rejects_non_finite() {
        assert_eq!(
            Vector3::new(f64::NAN, 0.0, 0.0),
            Err(KikaError::NonFiniteCoordinate)
        );
    }

    #[test]
    fn point_vector_affine_arithmetic() {
        assert_eq!(p(1.0, 2.0, 3.0) + v(1.0, 1.0, 1.0), p(2.0, 3.0, 4.0));
        assert_eq!(p(2.0, 3.0, 4.0) - v(1.0, 1.0, 1.0), p(1.0, 2.0, 3.0));
        assert_eq!(p(2.0, 3.0, 4.0) - p(1.0, 2.0, 3.0), v(1.0, 1.0, 1.0));
    }

    #[test]
    fn vector_arithmetic() {
        assert_eq!(v(1.0, 2.0, 3.0) + v(1.0, 1.0, 1.0), v(2.0, 3.0, 4.0));
        assert_eq!(-v(1.0, -2.0, 3.0), v(-1.0, 2.0, -3.0));
        assert_eq!(v(1.0, 2.0, 3.0) * 2.0, v(2.0, 4.0, 6.0));
    }
}
