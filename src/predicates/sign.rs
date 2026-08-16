/// The sign of an exact geometric quantity (e.g. a determinant).
///
/// Predicates return this instead of a raw floating-point value so callers
/// cannot accidentally compare a determinant to an ad-hoc epsilon (see
/// `docs/numerical-model.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// The quantity is exactly negative.
    Negative,
    /// The quantity is exactly zero.
    Zero,
    /// The quantity is exactly positive.
    Positive,
}

impl Sign {
    /// The sign of an `f64`, treating `0.0` and `-0.0` both as `Sign::Zero`.
    ///
    /// This is a raw sign extraction, not a filter — it says nothing about
    /// whether `value` itself is trustworthy. Callers use it either on a
    /// value already known exact (e.g. the leading term of a nonoverlapping
    /// expansion) or on a filtered value whose sign a separately-computed
    /// error bound has already proven correct.
    pub(crate) fn of(value: f64) -> Sign {
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
    /// The sequence turns clockwise.
    Clockwise,
    /// The sequence is degenerate (collinear).
    Collinear,
    /// The sequence turns counterclockwise.
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
