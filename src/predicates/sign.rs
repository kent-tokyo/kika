/// The sign of an exact geometric quantity (e.g. a determinant).
///
/// Predicates return this instead of a raw floating-point value so callers
/// cannot accidentally compare a determinant to an ad-hoc epsilon (see
/// `docs/numerical-model.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    Negative,
    Zero,
    Positive,
}

impl Sign {
    /// The sign of an `f64`, treating `0.0` and `-0.0` both as `Sign::Zero`.
    ///
    /// Only meaningful for values that are *already known to be exactly
    /// correct* (e.g. the leading term of a nonoverlapping expansion) —
    /// this is not a filter or a rounding decision.
    pub(crate) fn of_exact(value: f64) -> Sign {
        if value > 0.0 {
            Sign::Positive
        } else if value < 0.0 {
            Sign::Negative
        } else {
            Sign::Zero
        }
    }

    /// Flips `Positive` <-> `Negative`, leaves `Zero` unchanged.
    pub fn negate(self) -> Sign {
        match self {
            Sign::Negative => Sign::Positive,
            Sign::Zero => Sign::Zero,
            Sign::Positive => Sign::Negative,
        }
    }
}

/// The orientation of an ordered sequence of 2D points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Clockwise,
    Collinear,
    CounterClockwise,
}

impl From<Sign> for Orientation {
    fn from(sign: Sign) -> Self {
        match sign {
            Sign::Negative => Orientation::Clockwise,
            Sign::Zero => Orientation::Collinear,
            Sign::Positive => Orientation::CounterClockwise,
        }
    }
}
