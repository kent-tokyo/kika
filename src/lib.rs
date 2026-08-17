//! Kika — a robust 2D kernel for Rust: exact predicates, Delaunay
//! triangulation, constrained Delaunay triangulation, and simple-polygon
//! triangulation.
//!
//! Pre-1.0, no stability guarantees yet; see the repository README and
//! `docs/` for exact scope (each triangulation feature is narrow-scope —
//! see the [Maturity table](https://github.com/kent-tokyo/kika#maturity))
//! and status.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

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
pub use polygon::{PointPolygonRelation, Polygon2, PolygonBasicValidity, PolygonSelfIntersection};
pub use predicates::{Orientation, Sign, incircle, insphere, orient2d, orient3d};
pub use primitives::{
    Aabb2, Aabb3, Point2, Point3, PointSegmentRelation, PointTriangleRelation, Segment2, Triangle2,
    Triangle3, Vector2, Vector3,
};
pub use triangulation::{
    CdtError, ConstrainedTriangulation2, EdgeId, FaceId, PolygonTriangulationError, Triangulation2,
    VertexId, constrained_delaunay2, delaunay2, triangulate_polygon,
    triangulate_polygon_with_holes, validate_cdt_topology,
};
