use super::Point3;

/// An axis-aligned bounding box in 3D. See [`super::Aabb2`]'s doc
/// comment; same design, one more axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb3 {
    min: Point3,
    max: Point3,
}

impl Aabb3 {
    pub fn from_points(p: Point3, q: Point3) -> Self {
        Aabb3 {
            min: Point3::new_unchecked(p.x().min(q.x()), p.y().min(q.y()), p.z().min(q.z())),
            max: Point3::new_unchecked(p.x().max(q.x()), p.y().max(q.y()), p.z().max(q.z())),
        }
    }

    #[inline]
    pub fn min(&self) -> Point3 {
        self.min
    }

    #[inline]
    pub fn max(&self) -> Point3 {
        self.max
    }

    pub fn overlaps(&self, other: &Aabb3) -> bool {
        self.min.x() <= other.max.x()
            && other.min.x() <= self.max.x()
            && self.min.y() <= other.max.y()
            && other.min.y() <= self.max.y()
            && self.min.z() <= other.max.z()
            && other.min.z() <= self.max.z()
    }

    pub fn contains_point(&self, p: Point3) -> bool {
        self.min.x() <= p.x()
            && p.x() <= self.max.x()
            && self.min.y() <= p.y()
            && p.y() <= self.max.y()
            && self.min.z() <= p.z()
            && p.z() <= self.max.z()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).unwrap()
    }

    #[test]
    fn from_points_normalizes_order() {
        let a = Aabb3::from_points(p(3.0, 4.0, -1.0), p(1.0, -2.0, 5.0));
        assert_eq!(a.min(), p(1.0, -2.0, -1.0));
        assert_eq!(a.max(), p(3.0, 4.0, 5.0));
    }

    #[test]
    fn overlap_cases() {
        let a = Aabb3::from_points(p(0.0, 0.0, 0.0), p(2.0, 2.0, 2.0));
        let touching = Aabb3::from_points(p(2.0, 0.0, 0.0), p(4.0, 2.0, 2.0));
        let disjoint = Aabb3::from_points(p(3.0, 3.0, 3.0), p(4.0, 4.0, 4.0));
        assert!(a.overlaps(&touching));
        assert!(!a.overlaps(&disjoint));
    }

    #[test]
    fn contains_point_boundary_inclusive() {
        let a = Aabb3::from_points(p(0.0, 0.0, 0.0), p(2.0, 2.0, 2.0));
        assert!(a.contains_point(p(0.0, 0.0, 0.0)));
        assert!(a.contains_point(p(2.0, 2.0, 2.0)));
        assert!(!a.contains_point(p(2.1, 1.0, 1.0)));
    }
}
