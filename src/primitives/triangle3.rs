use super::Point3;

/// A 3D triangle with vertices `a`, `b`, `c`, in the order given.
///
/// No orientation/winding method yet: unlike [`super::Triangle2`], a 3D
/// triangle's "orientation" isn't a single scalar sign the way
/// `orient2d` gives one — it needs a 4th reference point (`orient3d`) or
/// a normal-vector convention, neither of which Phase 2 (2D-scoped) has
/// a concrete use for yet. Added when a real caller needs it, not
/// speculatively.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle3 {
    a: Point3,
    b: Point3,
    c: Point3,
}

impl Triangle3 {
    /// Creates a triangle with vertices `a`, `b`, `c` in the order given.
    pub fn new(a: Point3, b: Point3, c: Point3) -> Self {
        Triangle3 { a, b, c }
    }

    /// The first vertex.
    #[inline]
    pub fn a(&self) -> Point3 {
        self.a
    }

    /// The second vertex.
    #[inline]
    pub fn b(&self) -> Point3 {
        self.b
    }

    /// The third vertex.
    #[inline]
    pub fn c(&self) -> Point3 {
        self.c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors() {
        let a = Point3::new(0.0, 0.0, 0.0).unwrap();
        let b = Point3::new(1.0, 0.0, 0.0).unwrap();
        let c = Point3::new(0.0, 1.0, 0.0).unwrap();
        let t = Triangle3::new(a, b, c);
        assert_eq!(t.a(), a);
        assert_eq!(t.b(), b);
        assert_eq!(t.c(), c);
    }
}
