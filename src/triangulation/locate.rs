//! Point location: which vertex, edge, or face of a [`Triangulation2`]
//! (if any) a query point falls on. See
//! `docs/adr/ADR-008-point-location.md` for the full design and
//! correctness argument.

use super::delaunay2::Triangulation2;
use super::ids::{EdgeId, FaceId, VertexId};
use crate::primitives::{Point2, PointSegmentRelation, PointTriangleRelation, Segment2};

/// Where a query point falls relative to a [`Triangulation2`].
///
/// Closed (not `#[non_exhaustive]`): these 4 variants are exactly the
/// closure of [`Triangulation2`]'s own already-closed id vocabulary
/// (`VertexId`/`EdgeId`/`FaceId`) plus the necessary "miss" case -- a
/// 2-simplicial complex has only 0-cells, 1-cells, and 2-cells, so no
/// further variant is possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointLocation {
    /// The point exactly equals a vertex's coordinate.
    Vertex(VertexId),
    /// The point is on a bounded edge (hull or interior), not at either
    /// endpoint.
    Edge(EdgeId),
    /// The point is strictly inside exactly one face.
    Face(FaceId),
    /// The point is not covered by any face -- outside the triangulated
    /// domain. This is *not* the same as "outside the convex hull": for
    /// a [`super::triangulate_polygon_with_holes`] result, a point
    /// geometrically inside the outer ring but inside a hole is also
    /// `Outside`, since no face covers it.
    Outside,
}

impl Triangulation2 {
    /// Locates `point` against this triangulation's faces, edges, and
    /// vertices.
    ///
    /// A linear scan over every face -- performance is not part of this
    /// method's contract, and this signature does not need to change if
    /// a faster (e.g. walking) locator replaces the scan later. The
    /// result never depends on face iteration order: `Inside` regions
    /// are pairwise disjoint by construction, a shared interior edge's
    /// two incident faces always resolve to the same `EdgeId` (edges are
    /// deduplicated by canonical vertex pair), and a shared vertex always
    /// resolves to the same `VertexId` (no `Triangulation2` ever has
    /// duplicate-coordinate vertices).
    ///
    /// Always succeeds -- an empty triangulation (fewer than 3 points, or
    /// all input exactly collinear) returns `Outside` for every query
    /// point, never a panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use kika::{Point2, PointLocation, delaunay2};
    ///
    /// let pts = [
    ///     Point2::new(0.0, 0.0).unwrap(),
    ///     Point2::new(4.0, 0.0).unwrap(),
    ///     Point2::new(0.0, 4.0).unwrap(),
    /// ];
    /// let t = delaunay2(&pts);
    ///
    /// // Every input point locates to its own vertex.
    /// let (v0, _) = t.vertices().next().unwrap();
    /// assert_eq!(t.locate(pts[0]), PointLocation::Vertex(v0));
    ///
    /// // A point strictly inside the triangle locates to its one face.
    /// assert!(matches!(
    ///     t.locate(Point2::new(1.0, 1.0).unwrap()),
    ///     PointLocation::Face(_)
    /// ));
    ///
    /// // A point outside the hull.
    /// assert_eq!(t.locate(Point2::new(10.0, 10.0).unwrap()), PointLocation::Outside);
    /// ```
    ///
    /// For a [`super::ConstrainedTriangulation2`], locate through its
    /// existing [`super::ConstrainedTriangulation2::triangulation`]
    /// accessor: `cdt.triangulation().locate(point)`.
    pub fn locate(&self, point: Point2) -> PointLocation {
        for (face, tri) in self.faces().zip(self.triangles()) {
            match tri.relation_to(point) {
                PointTriangleRelation::Inside => return PointLocation::Face(face),
                PointTriangleRelation::Outside => continue,
                PointTriangleRelation::OnBoundary => {
                    let [v0, v1, v2] = self.face_vertices(face);
                    let corners = [(v0, tri.a()), (v1, tri.b()), (v2, tri.c())];
                    for &(i, j) in &[(0usize, 1usize), (1, 2), (2, 0)] {
                        let (ua, pa) = corners[i];
                        let (ub, pb) = corners[j];
                        match Segment2::new(pa, pb).relation_to(point) {
                            PointSegmentRelation::Endpoint => {
                                let vid = if point == pa { ua } else { ub };
                                return PointLocation::Vertex(vid);
                            }
                            PointSegmentRelation::Interior => {
                                let edge = self.edges().find(|&e| {
                                    let (x, y) = self.edge_vertices(e);
                                    (x == ua && y == ub) || (x == ub && y == ua)
                                });
                                if let Some(e) = edge {
                                    return PointLocation::Edge(e);
                                }
                            }
                            PointSegmentRelation::NotOnSegment => {}
                        }
                    }
                    // Provably unreachable for a non-degenerate CCW face
                    // (ADR-008 "OnBoundary implies a bounded edge") --
                    // continue rather than trust that as a panic gate.
                }
            }
        }
        PointLocation::Outside
    }
}
