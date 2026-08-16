//! Kika — robust computational geometry for Rust.
//!
//! Pre-alpha; see the repository README and `docs/` for scope and status.

mod error;
mod hull;
mod intersections;
mod polygon;
mod predicates;
mod primitives;
mod triangulation;

pub use error::KikaError;
pub use hull::{HullBoundaryPoints, convex_hull2};
pub use intersections::{
    SegmentIntersection2, SegmentIntersectionKind, segment_intersection, segment_intersection_kind,
};
pub use polygon::{Polygon2, PolygonBasicValidity, PolygonSelfIntersection};
pub use predicates::{Orientation, Sign, incircle, insphere, orient2d, orient3d};
pub use primitives::{
    Aabb2, Aabb3, Point2, Point3, PointSegmentRelation, PointTriangleRelation, Segment2, Triangle2,
    Triangle3, Vector2, Vector3,
};
pub use triangulation::{Triangulation2, delaunay2};
