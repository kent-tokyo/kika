use crate::error::KikaError;

/// A 2D point with finite `f64` coordinates.
///
/// `Point2` can only be constructed via [`Point2::new`], which rejects NaN
/// and infinite coordinates. Once constructed, its coordinates are
/// guaranteed finite for its lifetime (see
/// `docs/adr/ADR-003-public-primitive-types.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    x: f64,
    y: f64,
}

impl Point2 {
    /// Creates a new point, rejecting NaN and infinite coordinates.
    ///
    /// ```
    /// use kika::Point2;
    /// assert!(Point2::new(1.0, 2.0).is_ok());
    /// assert!(Point2::new(f64::NAN, 0.0).is_err());
    /// ```
    pub fn new(x: f64, y: f64) -> Result<Self, KikaError> {
        if x.is_finite() && y.is_finite() {
            Ok(Point2 { x, y })
        } else {
            Err(KikaError::NonFiniteCoordinate)
        }
    }

    /// Builds a point without validating finiteness. For internal use by
    /// arithmetic (`Point2 + Vector2` etc.) operating on already-finite
    /// operands — see `Vector2`'s module doc for why arithmetic doesn't
    /// re-validate. Not exposed publicly: `new` is the only finiteness
    /// boundary (ADR-003).
    pub(crate) fn new_unchecked(x: f64, y: f64) -> Self {
        Point2 { x, y }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite() {
        assert_eq!(
            Point2::new(f64::NAN, 0.0),
            Err(KikaError::NonFiniteCoordinate)
        );
        assert_eq!(
            Point2::new(0.0, f64::NAN),
            Err(KikaError::NonFiniteCoordinate)
        );
        assert_eq!(
            Point2::new(f64::INFINITY, 0.0),
            Err(KikaError::NonFiniteCoordinate)
        );
        assert_eq!(
            Point2::new(0.0, f64::NEG_INFINITY),
            Err(KikaError::NonFiniteCoordinate)
        );
    }

    #[test]
    fn accepts_finite() {
        let p = Point2::new(1.5, -2.5).unwrap();
        assert_eq!(p.x(), 1.5);
        assert_eq!(p.y(), -2.5);
    }

    /// Equality policy (ADR-003 "Phase 2: point equality policy"): exact
    /// coordinate equality, no tolerance. Signed zero compares equal, per
    /// IEEE-754, matching how the predicates already treat it.
    #[test]
    fn equality_is_exact_and_signed_zero_matches() {
        assert_eq!(Point2::new(1.0, 2.0), Point2::new(1.0, 2.0));
        assert_ne!(Point2::new(1.0, 2.0), Point2::new(1.0, 2.0000001));
        assert_eq!(Point2::new(0.0, -0.0), Point2::new(-0.0, 0.0));
    }
}
