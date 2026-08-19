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
    /// **O(F)**, where `F` is the number of faces -- a linear scan, not
    /// a spatial index or walking locator. (Precisely O(F + E) for the
    /// face scan plus the at-most-once edge lookup on an `OnBoundary`
    /// hit, but a planar triangulation always has `E = O(F)`, so this
    /// reduces to `O(F)`.) Performance is not part of this method's
    /// contract; this is stated explicitly so "there's a `locate` API"
    /// is never mistaken for "there's a fast point-location index" --
    /// a walking locator can replace the scan later, without this
    /// signature needing to change, once a real measured need for it
    /// exists. The result never depends on face iteration order: `Inside` regions
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

#[cfg(test)]
mod tests {
    use super::super::cdt::constrained_delaunay2;
    use super::super::delaunay2::{assemble_triangulation, delaunay2};
    use super::*;
    use crate::polygon::Polygon2;
    use crate::triangulation::triangulate_polygon_with_holes;

    fn pt(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
        Polygon2::new(vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)])
    }

    #[test]
    fn locate_on_a_single_triangle() {
        let pts = vec![pt(0.0, 0.0), pt(4.0, 0.0), pt(0.0, 4.0)];
        let t = delaunay2(&pts);

        // Every input point locates to its own vertex. delaunay2()
        // canonically sorts points before insertion, so match by
        // coordinate, not input-array index.
        for &p in &pts {
            let expected = t.vertices().find(|&(_, q)| q == p).unwrap().0;
            assert_eq!(t.locate(p), PointLocation::Vertex(expected));
        }

        // Exact-integer edge midpoints locate to their edge.
        for mid in [pt(2.0, 0.0), pt(0.0, 2.0), pt(2.0, 2.0)] {
            assert!(matches!(t.locate(mid), PointLocation::Edge(_)));
        }

        // Strictly interior point.
        assert!(matches!(t.locate(pt(1.0, 1.0)), PointLocation::Face(_)));

        // Clearly outside the hull.
        assert_eq!(t.locate(pt(10.0, 10.0)), PointLocation::Outside);
    }

    #[test]
    fn locate_every_vertex_in_a_generic_position_cloud() {
        struct Xorshift64(u64);
        impl Xorshift64 {
            fn next_u64(&mut self) -> u64 {
                let mut x = self.0;
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                self.0 = x;
                x
            }
            fn next_f64_in(&mut self, scale: f64) -> f64 {
                let bits = self.next_u64();
                let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
                (unit * 2.0 - 1.0) * scale
            }
        }

        let mut rng = Xorshift64(0x1ace_1de5_a5cf_1de5);
        let pts: Vec<Point2> = (0..60)
            .map(|_| pt(rng.next_f64_in(100.0), rng.next_f64_in(100.0)))
            .collect();
        let t = delaunay2(&pts);

        for (id, p) in t.vertices() {
            assert_eq!(
                t.locate(p),
                PointLocation::Vertex(id),
                "every triangulation vertex must locate to itself"
            );
        }
    }

    #[test]
    fn locate_hull_edge_vs_interior_edge_incidence() {
        // A square plus an off-center interior point -- the square's 4
        // hull edges each have exactly 1 incident face; the 4 edges from
        // the interior point to each corner are interior (2 incident
        // faces each).
        let pts = vec![
            pt(0.0, 0.0),
            pt(4.0, 0.0),
            pt(4.0, 4.0),
            pt(0.0, 4.0),
            pt(1.0, 1.0),
        ];
        let t = delaunay2(&pts);

        let hull_mid = pt(2.0, 0.0); // midpoint of the bottom hull edge
        match t.locate(hull_mid) {
            PointLocation::Edge(e) => {
                let incident = t.adjacent_faces(e).iter().filter(|f| f.is_some()).count();
                assert_eq!(incident, 1, "a hull edge has exactly 1 incident face");
            }
            other => panic!("expected Edge, got {other:?}"),
        }

        // The interior point itself must locate to a Vertex with more
        // than 2 incident faces (it's surrounded, not on the hull).
        let interior_vertex = t
            .vertices()
            .find(|&(_, p)| p == pt(1.0, 1.0))
            .expect("interior point must be a vertex")
            .0;
        assert_eq!(
            t.locate(pt(1.0, 1.0)),
            PointLocation::Vertex(interior_vertex)
        );
    }

    #[test]
    fn locate_on_empty_triangulation_returns_outside_never_panics() {
        for pts in [
            vec![],
            vec![pt(0.0, 0.0)],
            vec![pt(0.0, 0.0), pt(1.0, 0.0)],
            vec![pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0)], // exactly collinear
        ] {
            let t = delaunay2(&pts);
            assert!(t.is_empty());
            // Query points include one exactly equal to a degenerate
            // input point itself -- still Outside, since
            // Triangulation2::empty() records no vertices at all.
            for q in [pt(0.0, 0.0), pt(5.0, 5.0), pt(-1.0, -1.0)] {
                assert_eq!(t.locate(q), PointLocation::Outside);
            }
        }
    }

    #[test]
    fn locate_collinear_hull_stretch_flat_middle_point() {
        // 3 collinear points on the bottom edge plus one off-line point
        // -- the middle collinear point (1,0) is a genuine vertex, not
        // skipped or merged, mirroring ADR-007's own precedent of
        // testing this case explicitly for Voronoi rather than assuming
        // it's fine.
        let pts = vec![pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0), pt(1.0, 1.0)];
        let t = delaunay2(&pts);
        let flat_middle = t
            .vertices()
            .find(|&(_, p)| p == pt(1.0, 0.0))
            .expect("the flat middle point must still be a real vertex")
            .0;
        assert_eq!(t.locate(pt(1.0, 0.0)), PointLocation::Vertex(flat_middle));
    }

    #[test]
    fn locate_with_holes() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        let holes = [rect(1.0, 1.0, 3.0, 3.0)];
        let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();

        // Inside the outer ring and outside the hole: a real face.
        // (Not (7,7)/(8,8): those happen to sit exactly on the diagonal
        // edge this triangulation draws from (10,10) to hole-corner
        // (1,1), along y=x -- verified directly before picking (7,2).)
        assert!(matches!(t.locate(pt(7.0, 2.0)), PointLocation::Face(_)));

        // Inside the hole: not covered by any face, Outside -- even
        // though it's geometrically inside the outer ring.
        assert_eq!(t.locate(pt(2.0, 2.0)), PointLocation::Outside);

        // On the hole's own boundary: a real edge.
        assert!(matches!(t.locate(pt(2.0, 1.0)), PointLocation::Edge(_)));
    }

    #[test]
    fn locate_shared_interior_edge_is_order_independent() {
        // A cocircular-free square split by diagonal (0,2): the diagonal
        // is a genuine interior edge shared by both faces. Build the
        // same triangulation with the 2 faces in reversed relative
        // order (a distinct FaceId assignment for the same topology) and
        // confirm a point on that shared edge resolves to the exact
        // same EdgeId either way -- locate()'s own scan order must not
        // leak into the result.
        let pts = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.0)];
        let forward = assemble_triangulation(
            pts.clone(),
            vec![
                [VertexId(0), VertexId(1), VertexId(2)],
                [VertexId(0), VertexId(2), VertexId(3)],
            ],
        );
        let reversed = assemble_triangulation(
            pts,
            vec![
                [VertexId(0), VertexId(2), VertexId(3)],
                [VertexId(0), VertexId(1), VertexId(2)],
            ],
        );

        let on_diagonal = pt(1.0, 1.0);
        let forward_edge = forward.locate(on_diagonal);
        let reversed_edge = reversed.locate(on_diagonal);
        assert!(matches!(forward_edge, PointLocation::Edge(_)));

        // Compare by endpoint coordinates, not raw EdgeId -- the two
        // instances number their edges independently (arbitrary per
        // construction, same convention as VertexId/EdgeId/FaceId
        // generally), but must agree on *which* edge (by geometry).
        let endpoints = |t: &Triangulation2, loc: PointLocation| -> (Point2, Point2) {
            let PointLocation::Edge(e) = loc else {
                panic!("expected Edge, got {loc:?}")
            };
            let (u, v) = t.edge_vertices(e);
            let cu = t.vertices().find(|&(id, _)| id == u).unwrap().1;
            let cv = t.vertices().find(|&(id, _)| id == v).unwrap().1;
            if (cu.x(), cu.y()) <= (cv.x(), cv.y()) {
                (cu, cv)
            } else {
                (cv, cu)
            }
        };
        assert_eq!(
            endpoints(&forward, forward_edge),
            endpoints(&reversed, reversed_edge)
        );
    }

    #[test]
    fn locate_through_constrained_triangulation_accessor() {
        let pts = vec![pt(0.0, 0.0), pt(4.0, 0.0), pt(4.0, 4.0), pt(0.0, 4.0)];
        // Constrained diagonal (0,0)-(4,4): pick a query point clearly
        // off that line (not (1,1), which sits exactly on it).
        let cdt = constrained_delaunay2(&pts, &[(0, 2)]).unwrap();
        assert!(matches!(
            cdt.triangulation().locate(pt(3.0, 1.0)),
            PointLocation::Face(_)
        ));
    }

    #[test]
    fn locate_agrees_with_underlying_triangle_relation_to() {
        // Differential-style check against the primitive locate() is
        // assembled from: locate(p) != Outside iff some triangle's own
        // relation_to(p) != Outside.
        struct Xorshift64(u64);
        impl Xorshift64 {
            fn next_u64(&mut self) -> u64 {
                let mut x = self.0;
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                self.0 = x;
                x
            }
            fn next_f64_in(&mut self, scale: f64) -> f64 {
                let bits = self.next_u64();
                let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
                (unit * 2.0 - 1.0) * scale
            }
        }

        let mut rng = Xorshift64(0xfeed_face_dead_beef);
        let pts: Vec<Point2> = (0..40)
            .map(|_| pt(rng.next_f64_in(50.0), rng.next_f64_in(50.0)))
            .collect();
        let t = delaunay2(&pts);

        for _ in 0..200 {
            let q = pt(rng.next_f64_in(60.0), rng.next_f64_in(60.0));
            let located = t.locate(q) != PointLocation::Outside;
            let covered = t
                .triangles()
                .iter()
                .any(|tri| tri.relation_to(q) != crate::primitives::PointTriangleRelation::Outside);
            assert_eq!(located, covered, "locate/relation_to disagreement at {q:?}");
        }
    }

    #[test]
    fn locate_direct_assemble_triangulation_fixture_matches_face_vertices() {
        // Sanity check on the faces()/triangles() index-parallel
        // contract locate() depends on, built directly via
        // assemble_triangulation rather than delaunay2 to make the face
        // list explicit.
        let pts = vec![pt(0.0, 0.0), pt(4.0, 0.0), pt(4.0, 4.0), pt(0.0, 4.0)];
        let faces = vec![
            [VertexId(0), VertexId(1), VertexId(2)],
            [VertexId(0), VertexId(2), VertexId(3)],
        ];
        let t = assemble_triangulation(pts, faces);
        for (face, tri) in t.faces().zip(t.triangles()) {
            let [v0, v1, v2] = t.face_vertices(face);
            let expected: Vec<Point2> = vec![v0, v1, v2]
                .into_iter()
                .map(|v| t.vertices().nth(v.raw() as usize).unwrap().1)
                .collect();
            assert_eq!(vec![tri.a(), tri.b(), tri.c()], expected);
        }
    }
}
