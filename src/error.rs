use core::fmt;

/// Error type for fallible Kika constructors.
///
/// Kika's predicates and algorithms never fail or panic; only construction
/// of a validated type (e.g. [`crate::Point2::new`]) can fail. See
/// `docs/adr/ADR-003-public-primitive-types.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KikaError {
    /// A coordinate was NaN or infinite.
    NonFiniteCoordinate,
}

impl fmt::Display for KikaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KikaError::NonFiniteCoordinate => {
                write!(f, "coordinate must be finite (not NaN or infinite)")
            }
        }
    }
}

impl core::error::Error for KikaError {}
