//! Simple-polygon triangulation via Phase 6C's constrained Delaunay
//! (Phase 6D), narrow scope per this session's explicit direction: no
//! holes, no self-intersecting input (typed error instead), no Steiner
//! points -- every output vertex is one of the input polygon's own
//! vertices.

use std::collections::{HashMap, HashSet, VecDeque};

use super::Triangulation2;
use super::cdt::constrained_delaunay2;
#[cfg(test)]
use super::delaunay2::TopologyError;
use super::delaunay2::assemble_triangulation;
use super::ids::{FaceId, VertexId};
use crate::polygon::{Polygon2, PolygonBasicValidity, PolygonSelfIntersection};
use crate::predicates::{Orientation, orient2d};
use crate::primitives::Point2;

/// Why [`triangulate_polygon`] rejected an input or failed to build a
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonTriangulationError {
    /// Fewer than 3 vertices — see [`PolygonBasicValidity::TooFewVertices`].
    TooFewVertices,
    /// Two consecutive vertices are exactly equal (a zero-length edge) —
    /// see [`PolygonBasicValidity::ConsecutiveDuplicateVertices`].
    DegenerateEdge,
    /// At least 3 vertices, no consecutive duplicates, but the exact
    /// signed area is zero anyway — see [`PolygonBasicValidity::ZeroArea`].
    ZeroArea,
    /// The polygon boundary self-intersects (a non-adjacent repeated
    /// vertex counts, via `PolygonSelfIntersection`'s `EndpointTouch`
    /// case). Automatic splitting into simple sub-polygons is out of
    /// scope — see the module doc comment.
    SelfIntersecting(PolygonSelfIntersection),
    /// The underlying constrained Delaunay triangulation could not
    /// realize the polygon boundary, or the interior-side flood fill
    /// couldn't find a starting face. Not expected to be reachable given
    /// this function's own upfront validation (every boundary edge is
    /// non-crossing and between distinct vertices, by construction, once
    /// [`PolygonTriangulationError::SelfIntersecting`] and
    /// [`PolygonTriangulationError::DegenerateEdge`] have already been
    /// ruled out) — returned defensively rather than panicking or
    /// unwrapping, in case that reasoning is ever wrong.
    ConstraintInsertionFailed,
}

/// Triangulates a simple polygon (no holes, no self-intersections) using
/// only its own input vertices — no Steiner points (see the module doc
/// comment for full scope). Accepts both CCW and CW input.
///
/// # Algorithm
///
/// 1. Reject the same structural degeneracies [`Polygon2::basic_validity`]
///    and [`Polygon2::find_self_intersection`] already check for, as typed
///    errors, before touching any triangulation.
/// 2. Build the constrained Delaunay triangulation of the polygon's own
///    vertices ([`super::constrained_delaunay2`]), constraining every
///    polygon edge. This triangulates the polygon's full *convex hull*,
///    which for a non-convex polygon includes extra "outside" triangles
///    filling its concave pockets.
/// 3. Discard those outside triangles by a purely topological flood fill:
///    starting from one interior seed face (identified via a single
///    [`orient2d`] check against the polygon's own
///    [`Polygon2::orientation`], reusing an existing triangle vertex —
///    never constructing a new coordinate such as a centroid, which would
///    reopen ADR-004's construction-exactness questions for no reason),
///    walk to every other face reachable without crossing a constrained
///    (boundary) edge. A simple polygon's interior is always a single
///    connected region (the discrete Jordan curve guarantee), so this
///    always reaches every interior face and no exterior ones.
///
/// # A caveat for callers checking the result with [`Triangulation2::validate_topology`]
///
/// That validator's Euler-characteristic check assumes the triangulation
/// covers its own vertex set's full convex hull — true for `delaunay2`
/// and [`super::constrained_delaunay2`]'s output, but generally **false**
/// here for a non-convex polygon: step 3 above deliberately discards the
/// concave-pocket faces, leaving a proper subset of the hull. Expect
/// `TopologyError::EulerFormulaViolated` on a non-convex result — every
/// other check (CCW, manifold edges, adjacency reciprocity) still holds.
/// The applicable invariant instead is: a simple polygon triangulated
/// with only its own vertices always has exactly `polygon.len() - 2`
/// triangles.
pub fn triangulate_polygon(
    polygon: &Polygon2,
) -> Result<Triangulation2, PolygonTriangulationError> {
    match polygon.basic_validity() {
        PolygonBasicValidity::TooFewVertices => {
            return Err(PolygonTriangulationError::TooFewVertices);
        }
        PolygonBasicValidity::ConsecutiveDuplicateVertices => {
            return Err(PolygonTriangulationError::DegenerateEdge);
        }
        PolygonBasicValidity::ZeroArea => {
            return Err(PolygonTriangulationError::ZeroArea);
        }
        PolygonBasicValidity::Valid => {}
    }
    if let Some(found) = polygon.find_self_intersection() {
        return Err(PolygonTriangulationError::SelfIntersecting(found));
    }

    let points = polygon.vertices();
    let n = points.len();
    let constraints: Vec<(usize, usize)> = (0..n).map(|i| (i, (i + 1) % n)).collect();
    let cdt = constrained_delaunay2(points, &constraints)
        .map_err(|_| PolygonTriangulationError::ConstraintInsertionFailed)?;

    let coord: HashMap<VertexId, Point2> = cdt.triangulation().vertices().collect();

    let seed = interior_seed_face(&cdt, &coord, points[0], points[1], polygon.orientation())
        .ok_or(PolygonTriangulationError::ConstraintInsertionFailed)?;
    let interior = flood_fill_interior(&cdt, seed);

    let vertex_pos: Vec<Point2> = cdt.triangulation().vertices().map(|(_, p)| p).collect();
    let faces: Vec<[VertexId; 3]> = cdt
        .triangulation()
        .faces()
        .filter(|f| interior.contains(f))
        .map(|f| cdt.triangulation().face_vertices(f))
        .collect();

    Ok(assemble_triangulation(vertex_pos, faces))
}

/// The one triangulation face, incident to the directed boundary edge
/// `(p0, p1)` (the polygon's own first edge, in its own vertex order),
/// that lies on `orientation`'s interior side — found via a single
/// [`orient2d`] check per candidate face against that face's own apex
/// vertex, never a constructed point.
fn interior_seed_face(
    cdt: &super::ConstrainedTriangulation2,
    coord: &HashMap<VertexId, Point2>,
    p0: Point2,
    p1: Point2,
    orientation: Orientation,
) -> Option<FaceId> {
    let vertex_of = |p: Point2| coord.iter().find(|&(_, &q)| q == p).map(|(&id, _)| id);
    let u = vertex_of(p0)?;
    let v = vertex_of(p1)?;
    let edge = cdt.triangulation().edges().find(|&e| {
        let (a, b) = cdt.triangulation().edge_vertices(e);
        (a == u && b == v) || (a == v && b == u)
    })?;

    for face in cdt
        .triangulation()
        .adjacent_faces(edge)
        .into_iter()
        .flatten()
    {
        let tri = cdt.triangulation().face_vertices(face);
        let Some(&apex) = tri.iter().find(|&&x| x != u && x != v) else {
            continue;
        };
        let side = orient2d(p0, p1, coord[&apex]);
        let is_interior = match orientation {
            Orientation::CounterClockwise => side == Orientation::CounterClockwise,
            Orientation::Clockwise => side == Orientation::Clockwise,
            // Unreachable from `triangulate_polygon`: a Collinear polygon
            // orientation means zero area, already rejected as
            // `PolygonTriangulationError::ZeroArea` before this is called.
            Orientation::Collinear => false,
        };
        if is_interior {
            return Some(face);
        }
    }
    None
}

/// Every face reachable from `seed` without crossing a constrained
/// (polygon boundary) edge.
fn flood_fill_interior(cdt: &super::ConstrainedTriangulation2, seed: FaceId) -> HashSet<FaceId> {
    let mut links: HashMap<FaceId, Vec<FaceId>> = HashMap::new();
    for edge in cdt.triangulation().edges() {
        if cdt.is_constrained(edge) {
            continue;
        }
        if let [Some(fa), Some(fb)] = cdt.triangulation().adjacent_faces(edge) {
            links.entry(fa).or_default().push(fb);
            links.entry(fb).or_default().push(fa);
        }
    }

    let mut interior = HashSet::new();
    let mut queue = VecDeque::new();
    interior.insert(seed);
    queue.push_back(seed);
    while let Some(face) = queue.pop_front() {
        for &neighbor in links.get(&face).into_iter().flatten() {
            if interior.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    interior
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    /// Plain (not exact) shoelace area, for area-conservation sanity
    /// checks only -- not a claim about `Triangle2` needing this method
    /// itself (it doesn't have one; predicates stay separate from
    /// constructions per this crate's own split).
    fn tri_area(tri: crate::primitives::Triangle2) -> f64 {
        let (a, b, c) = (tri.a(), tri.b(), tri.c());
        ((b.x() - a.x()) * (c.y() - a.y()) - (c.x() - a.x()) * (b.y() - a.y())).abs() / 2.0
    }

    /// [`Triangulation2::validate_topology`]'s checks, except
    /// `EulerFormulaViolated` -- that check assumes the triangulation
    /// covers its own vertex set's full convex hull (true for
    /// `delaunay2`/`constrained_delaunay2`, false for
    /// `triangulate_polygon`'s output on a non-convex polygon, which
    /// deliberately triangulates only the interior, a proper subset of
    /// the hull). Every other check (CCW, manifold edges, adjacency
    /// reciprocity, local-Delaunay on untouched interior edges) still
    /// applies unchanged.
    fn validate_ignoring_hull_coverage(t: &Triangulation2) -> Vec<TopologyError> {
        t.validate_topology()
            .into_iter()
            .filter(|e| !matches!(e, TopologyError::EulerFormulaViolated { .. }))
            .collect()
    }

    #[test]
    fn triangle_triangulates_to_itself() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)]);
        let t = triangulate_polygon(&poly).unwrap();
        assert_eq!(t.len(), 1);
        assert!(t.validate_topology().is_empty());
    }

    #[test]
    fn convex_square_triangulates_to_two_triangles() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]);
        let t = triangulate_polygon(&poly).unwrap();
        assert_eq!(t.len(), 2);
        assert!(t.validate_topology().is_empty());
        for tri in t.triangles() {
            assert_eq!(tri.orientation(), Orientation::CounterClockwise);
        }
    }

    #[test]
    fn clockwise_input_triangulates_the_same_region() {
        let ccw = Polygon2::new(vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]);
        let cw = Polygon2::new(vec![p(0.0, 0.0), p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0)]);
        assert_eq!(ccw.orientation(), Orientation::CounterClockwise);
        assert_eq!(cw.orientation(), Orientation::Clockwise);

        let t_ccw = triangulate_polygon(&ccw).unwrap();
        let t_cw = triangulate_polygon(&cw).unwrap();
        assert_eq!(t_ccw.len(), 2);
        assert_eq!(t_cw.len(), 2);
        let area =
            |t: &Triangulation2| -> f64 { t.triangles().iter().map(|&tri| tri_area(tri)).sum() };
        assert_eq!(area(&t_ccw), 16.0);
        assert_eq!(area(&t_cw), 16.0);
    }

    /// An L-shaped (non-convex) hexagon: the concave notch's "outside"
    /// pocket must be discarded, not included as a triangle.
    #[test]
    fn l_shape_discards_the_concave_pocket() {
        let poly = Polygon2::new(vec![
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 2.0),
            p(2.0, 2.0),
            p(2.0, 4.0),
            p(0.0, 4.0),
        ]);
        let expected_area = 12.0; // 4x4 square minus the 2x2 missing corner
        let t = triangulate_polygon(&poly).unwrap();
        let errs = validate_ignoring_hull_coverage(&t);
        assert!(errs.is_empty(), "{errs:?} triangles={:?}", t.triangles());
        // A simple polygon triangulation with only input vertices always
        // has exactly n - 2 triangles -- this is the check that actually
        // catches an outside pocket wrongly kept or an inside face wrongly
        // dropped, in place of `validate_topology`'s hull-coverage-based
        // Euler check (not applicable here -- see
        // `validate_ignoring_hull_coverage`'s doc comment).
        assert_eq!(t.len(), poly.len() - 2);
        let area: f64 = t.triangles().iter().map(|&tri| tri_area(tri)).sum();
        assert_eq!(area, expected_area);
        for tri in t.triangles() {
            assert_eq!(tri.orientation(), Orientation::CounterClockwise);
        }
        // Every output vertex must be one of the 6 input vertices -- no
        // Steiner points.
        for v in t.vertices() {
            assert!(poly.vertices().contains(&v.1));
        }
    }

    #[test]
    fn too_few_vertices_is_rejected() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(1.0, 0.0)]);
        assert_eq!(
            triangulate_polygon(&poly),
            Err(PolygonTriangulationError::TooFewVertices)
        );
    }

    #[test]
    fn consecutive_duplicate_vertex_is_rejected() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)]);
        assert_eq!(
            triangulate_polygon(&poly),
            Err(PolygonTriangulationError::DegenerateEdge)
        );
    }

    #[test]
    fn collinear_zero_area_is_rejected() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)]);
        assert_eq!(
            triangulate_polygon(&poly),
            Err(PolygonTriangulationError::ZeroArea)
        );
    }

    #[test]
    fn self_intersecting_bowtie_is_rejected() {
        // A symmetric square-diagonal bowtie has zero *net* signed area
        // (its two lobes are congruent with opposite winding and cancel
        // exactly), which would be caught by `basic_validity`'s ZeroArea
        // check before ever reaching self-intersection detection -- this
        // shape is asymmetric so its two lobes don't cancel, genuinely
        // exercising the SelfIntersecting path.
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(6.0, 6.0), p(6.0, 0.0), p(0.0, 2.0)]);
        assert_ne!(poly.basic_validity(), PolygonBasicValidity::ZeroArea);
        match triangulate_polygon(&poly) {
            Err(PolygonTriangulationError::SelfIntersecting(found)) => {
                assert_eq!(found.edge_a, 0);
                assert_eq!(found.edge_b, 2);
            }
            other => panic!("expected SelfIntersecting, got {other:?}"),
        }
    }

    #[test]
    fn deterministic_regardless_of_starting_vertex() {
        // Same polygon (same vertex set, same boundary edge set), vertex
        // list rotated to a different starting point -- must produce the
        // exact same triangle set, not just the same total area. Rotating
        // by 3 also happens to move the reflex vertex (2,2) to index 0,
        // so `poly_b`'s seed edge (index 0-1) is a chord with 2 incident
        // faces, not a hull edge with 1 -- see
        // `seed_edge_with_two_incident_faces_still_finds_the_interior_side`
        // for that same property tested explicitly.
        let poly_a = Polygon2::new(vec![
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 2.0),
            p(2.0, 2.0),
            p(2.0, 4.0),
            p(0.0, 4.0),
        ]);
        let poly_b = Polygon2::new(vec![
            p(2.0, 2.0),
            p(2.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 2.0),
        ]);
        let t_a = triangulate_polygon(&poly_a).unwrap();
        let t_b = triangulate_polygon(&poly_b).unwrap();
        assert_eq!(t_a.triangles(), t_b.triangles());
    }

    /// The polygon's first edge (the flood fill's seed edge) is a chord
    /// of the full point set's convex hull -- 2 incident faces, one
    /// inside the L, one in the discarded pocket -- not a hull edge (1
    /// incident face, where interior-side disambiguation is trivial).
    /// Exercises the actual `orient2d` disambiguation in
    /// `interior_seed_face`, which every other test so far has bypassed
    /// by having a hull edge at index 0.
    #[test]
    fn seed_edge_with_two_incident_faces_still_finds_the_interior_side() {
        let poly = Polygon2::new(vec![
            p(4.0, 2.0),
            p(2.0, 2.0),
            p(2.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 0.0),
            p(4.0, 0.0),
        ]);
        let t = triangulate_polygon(&poly).unwrap();
        assert_eq!(t.len(), poly.len() - 2);
        let area: f64 = t.triangles().iter().map(|&tri| tri_area(tri)).sum();
        assert_eq!(area, 12.0);
        let errs = validate_ignoring_hull_coverage(&t);
        assert!(errs.is_empty(), "{errs:?}");
    }

    /// A plus/cross outline: 4 separate concave notches, each carving out
    /// its own disconnected "outside" pocket in the full point set's
    /// convex hull (an octagon). Checks the flood fill discards every
    /// pocket, not just the first one it happens to reach.
    #[test]
    fn plus_shape_discards_all_four_separate_pockets() {
        let poly = Polygon2::new(vec![
            p(2.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 2.0),
            p(6.0, 2.0),
            p(6.0, 4.0),
            p(4.0, 4.0),
            p(4.0, 6.0),
            p(2.0, 6.0),
            p(2.0, 4.0),
            p(0.0, 4.0),
            p(0.0, 2.0),
            p(2.0, 2.0),
        ]);
        assert_eq!(poly.orientation(), Orientation::CounterClockwise);
        let t = triangulate_polygon(&poly).unwrap();
        assert_eq!(t.len(), poly.len() - 2);
        let area: f64 = t.triangles().iter().map(|&tri| tri_area(tri)).sum();
        assert_eq!(area, 20.0); // 6x6 bounding box minus 4 corner 2x2 squares
        let errs = validate_ignoring_hull_coverage(&t);
        assert!(errs.is_empty(), "{errs:?}");
    }
}
