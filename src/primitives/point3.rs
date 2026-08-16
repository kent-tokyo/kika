use crate::error::KikaError;

/// A 3D point with finite `f64` coordinates.
///
/// `Point3` can only be constructed via [`Point3::new`], which rejects NaN
/// and infinite coordinates. Once constructed, its coordinates are
/// guaranteed finite for its lifetime (see
/// `docs/adr/ADR-003-public-primitive-types.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Point3 {
    /// Creates a new point, rejecting NaN and infinite coordinates.
    ///
    /// ```
    /// use kika::Point3;
    /// assert!(Point3::new(1.0, 2.0, 3.0).is_ok());
    /// assert!(Point3::new(f64::NAN, 0.0, 0.0).is_err());
    /// ```
    pub fn new(x: f64, y: f64, z: f64) -> Result<Self, KikaError> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Point3 { x, y, z })
        } else {
            Err(KikaError::NonFiniteCoordinate)
        }
    }

    /// Builds a point without validating finiteness. See
    /// `Point2::new_unchecked`'s doc comment; same rationale.
    pub(crate) fn new_unchecked(x: f64, y: f64, z: f64) -> Self {
        Point3 { x, y, z }
    }

    /// The x coordinate.
    #[inline]
    pub fn x(&self) -> f64 {
        self.x
    }

    /// The y coordinate.
    #[inline]
    pub fn y(&self) -> f64 {
        self.y
    }

    /// The z coordinate.
    #[inline]
    pub fn z(&self) -> f64 {
        self.z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite() {
        assert_eq!(
            Point3::new(f64::NAN, 0.0, 0.0),
            Err(KikaError::NonFiniteCoordinate)
        );
        assert_eq!(
            Point3::new(0.0, f64::INFINITY, 0.0),
            Err(KikaError::NonFiniteCoordinate)
        );
        assert_eq!(
            Point3::new(0.0, 0.0, f64::NEG_INFINITY),
            Err(KikaError::NonFiniteCoordinate)
        );
    }

    #[test]
    fn accepts_finite() {
        let p = Point3::new(1.5, -2.5, 3.0).unwrap();
        assert_eq!(p.x(), 1.5);
        assert_eq!(p.y(), -2.5);
        assert_eq!(p.z(), 3.0);
    }

    /// Equality policy (ADR-003 "Phase 2: point equality policy"): exact
    /// coordinate equality, no tolerance. Signed zero compares equal, per
    /// IEEE-754, matching how the predicates already treat it.
    #[test]
    fn equality_is_exact_and_signed_zero_matches() {
        assert_eq!(Point3::new(1.0, 2.0, 3.0), Point3::new(1.0, 2.0, 3.0));
        assert_ne!(Point3::new(1.0, 2.0, 3.0), Point3::new(1.0, 2.0, 3.0000001));
        assert_eq!(Point3::new(0.0, -0.0, 0.0), Point3::new(-0.0, 0.0, -0.0));
    }
}
