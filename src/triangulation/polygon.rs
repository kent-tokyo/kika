//! Simple-polygon triangulation via Phase 6C's constrained Delaunay
//! (Phase 6D), narrow scope per this session's explicit direction: no
//! holes, no self-intersecting input (typed error instead), no Steiner
//! points -- every output vertex is one of the input polygon's own
//! vertices.
//!
//! [`triangulate_polygon_with_holes`] (0.4.0) lifts the "no holes"
//! restriction -- see its own doc comment for scope.

use std::collections::{HashMap, HashSet, VecDeque};

use super::Triangulation2;
use super::cdt::constrained_delaunay2;
#[cfg(test)]
use super::delaunay2::TopologyError;
use super::delaunay2::assemble_triangulation;
use super::ids::{FaceId, VertexId};
use crate::intersections::{SegmentIntersectionKind, segment_intersection_kind};
use crate::polygon::{
    PointPolygonRelation, Polygon2, PolygonBasicValidity, PolygonSelfIntersection,
};
use crate::predicates::{Orientation, orient2d};
use crate::primitives::Point2;

/// Why [`triangulate_polygon`] rejected an input or failed to build a
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// A hole ring itself fails the same cheap structural checks the
    /// outer boundary must pass — see [`PolygonBasicValidity`]. Checked
    /// before any relationship between the hole and the outer boundary
    /// or other holes. `(hole_index, validity)`.
    InvalidHole(usize, PolygonBasicValidity),
    /// A hole ring self-intersects — `(hole_index, found)`.
    HoleSelfIntersecting(usize, PolygonSelfIntersection),
    /// A hole's boundary intersects the outer boundary — touching or
    /// crossing, never fully contained — `(hole_index, kind)`. `kind` is
    /// never [`SegmentIntersectionKind::None`] (that case is
    /// [`PolygonTriangulationError::HoleOutsideOuter`] instead, or no
    /// error at all if the hole is properly contained).
    HoleIntersectsOuter(usize, SegmentIntersectionKind),
    /// A hole lies entirely outside the outer boundary: no intersection
    /// with it, and not contained by it either — `hole_index`.
    HoleOutsideOuter(usize),
    /// Two holes' boundaries intersect — touching or crossing —
    /// `(hole_a, hole_b, kind)` with `hole_a < hole_b`.
    HolesIntersect(usize, usize, SegmentIntersectionKind),
    /// One hole is entirely nested inside another (an "island" case,
    /// semantically a hole-within-a-hole) — `(inner_hole, outer_hole)`.
    /// Not supported in this scope; a typed error rather than silent
    /// partial support. See [`triangulate_polygon_with_holes`]'s doc
    /// comment.
    NestedHole(usize, usize),
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
///
/// # Examples
///
/// ```
/// use kika::{Point2, Polygon2, triangulate_polygon};
///
/// let square = Polygon2::new(vec![
///     Point2::new(0.0, 0.0).unwrap(),
///     Point2::new(4.0, 0.0).unwrap(),
///     Point2::new(4.0, 4.0).unwrap(),
///     Point2::new(0.0, 4.0).unwrap(),
/// ]);
/// let t = triangulate_polygon(&square).unwrap();
///
/// // A simple polygon triangulated with only its own vertices always has
/// // exactly `polygon.len() - 2` triangles.
/// assert_eq!(t.len(), square.len() - 2);
/// ```
///
/// See `examples/polygon_triangulation.rs` for a runnable version
/// (`cargo run --example polygon_triangulation`).
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

    // Defensive: re-verify the invariant this function's own doc comment
    // promises (exactly `polygon.len() - 2` triangles) before returning
    // `Ok`, rather than trusting the flood fill's result unchecked --
    // not expected to be reachable given a simple polygon input, but
    // cheap, and this is exactly the shape of postcondition check that
    // was missing before `insert_constraint_edge`'s queue-empty bug (see
    // `cdt.rs`).
    if faces.len() != n - 2 {
        return Err(PolygonTriangulationError::ConstraintInsertionFailed);
    }

    Ok(assemble_triangulation(vertex_pos, faces))
}

/// Triangulates a simple polygon with holes: `outer`'s interior, minus
/// each hole's interior. No Steiner points (every output vertex is one
/// of `outer`'s or a hole's own vertices) — see the module doc comment.
/// Accepts any mix of CW/CCW winding for `outer` and each hole
/// independently; orientation is normalized internally.
///
/// # Algorithm
///
/// Generalizes [`triangulate_polygon`]'s own algorithm rather than using
/// a different one: constrain every edge of `outer` *and* every edge of
/// every hole in one [`constrained_delaunay2`] call over the combined
/// vertex set, then flood-fill from a seed face on `outer`'s interior
/// side, stopping at any constrained edge. A hole's boundary is just
/// more constrained edges from the flood fill's point of view — exactly
/// the same mechanism [`triangulate_polygon`] already uses to discard its
/// own concave pockets on a non-convex `outer`, generalized for free: the
/// fill can no more cross into a hole's interior than it can cross back
/// out of `outer`'s own boundary. No new construction, no new coordinate
/// — see ADR-004.
///
/// # Rejected input
///
/// `outer` is checked exactly like [`triangulate_polygon`]'s own input
/// (too few vertices, a degenerate edge, zero area, self-intersection).
/// Each hole is checked the same way
/// ([`PolygonTriangulationError::InvalidHole`] /
/// [`PolygonTriangulationError::HoleSelfIntersecting`]), then against
/// `outer` and every other hole: a hole must be entirely contained in
/// `outer`, touching neither `outer`'s boundary nor any other hole's —
/// touching or crossing either is a typed error
/// ([`PolygonTriangulationError::HoleIntersectsOuter`] /
/// [`PolygonTriangulationError::HolesIntersect`]), and so is a hole lying
/// entirely outside `outer`
/// ([`PolygonTriangulationError::HoleOutsideOuter`]). A hole nested
/// inside another hole (an "island" case) is out of scope, rejected as
/// [`PolygonTriangulationError::NestedHole`] rather than partially
/// supported.
///
/// # Acceptance criteria
///
/// For `n` total vertices (`outer` plus every hole) and `h` holes, a
/// valid input triangulates to exactly `n + 2h - 2` triangles (the
/// standard result for triangulating a polygonal domain with holes and no
/// Steiner points; reduces to [`triangulate_polygon`]'s own `n - 2` at
/// `h = 0`) — checked defensively before returning `Ok`, the same
/// postcondition discipline as [`triangulate_polygon`].
///
/// # Examples
///
/// ```
/// use kika::{Point2, Polygon2, triangulate_polygon_with_holes};
///
/// let outer = Polygon2::new(vec![
///     Point2::new(0.0, 0.0).unwrap(),
///     Point2::new(4.0, 0.0).unwrap(),
///     Point2::new(4.0, 4.0).unwrap(),
///     Point2::new(0.0, 4.0).unwrap(),
/// ]);
/// let hole = Polygon2::new(vec![
///     Point2::new(1.0, 1.0).unwrap(),
///     Point2::new(1.0, 2.0).unwrap(),
///     Point2::new(2.0, 2.0).unwrap(),
///     Point2::new(2.0, 1.0).unwrap(),
/// ]);
/// let t = triangulate_polygon_with_holes(&outer, &[hole]).unwrap();
///
/// // n + 2h - 2 = 8 total vertices + 2*1 hole - 2 = 8 triangles.
/// assert_eq!(t.len(), 8);
/// ```
pub fn triangulate_polygon_with_holes(
    outer: &Polygon2,
    holes: &[Polygon2],
) -> Result<Triangulation2, PolygonTriangulationError> {
    match outer.basic_validity() {
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
    if let Some(found) = outer.find_self_intersection() {
        return Err(PolygonTriangulationError::SelfIntersecting(found));
    }
    validate_holes(outer, holes)?;

    let mut points: Vec<Point2> = outer.vertices().to_vec();
    let mut hole_offsets = Vec::with_capacity(holes.len());
    for hole in holes {
        hole_offsets.push(points.len());
        points.extend_from_slice(hole.vertices());
    }

    let n_outer = outer.len();
    let mut constraints: Vec<(usize, usize)> =
        (0..n_outer).map(|i| (i, (i + 1) % n_outer)).collect();
    for (hole, &offset) in holes.iter().zip(&hole_offsets) {
        let n_hole = hole.len();
        constraints.extend((0..n_hole).map(|i| (offset + i, offset + (i + 1) % n_hole)));
    }

    let cdt = constrained_delaunay2(&points, &constraints)
        .map_err(|_| PolygonTriangulationError::ConstraintInsertionFailed)?;

    let coord: HashMap<VertexId, Point2> = cdt.triangulation().vertices().collect();

    let seed = interior_seed_face(
        &cdt,
        &coord,
        outer.vertices()[0],
        outer.vertices()[1],
        outer.orientation(),
    )
    .ok_or(PolygonTriangulationError::ConstraintInsertionFailed)?;
    let interior = flood_fill_interior(&cdt, seed);

    let vertex_pos: Vec<Point2> = cdt.triangulation().vertices().map(|(_, p)| p).collect();
    let faces: Vec<[VertexId; 3]> = cdt
        .triangulation()
        .faces()
        .filter(|f| interior.contains(f))
        .map(|f| cdt.triangulation().face_vertices(f))
        .collect();

    // Defensive: same postcondition discipline as `triangulate_polygon`
    // (see its own comment at the analogous check) -- `n + 2h - 2` is the
    // hole-generalized triangle count, not just `n - 2`.
    let expected_faces = points.len() + 2 * holes.len() - 2;
    if faces.len() != expected_faces {
        return Err(PolygonTriangulationError::ConstraintInsertionFailed);
    }

    Ok(assemble_triangulation(vertex_pos, faces))
}

/// The first found intersection (any kind other than `None`) between one
/// of `a`'s edges and one of `b`'s edges, if any. No adjacency exclusion
/// needed (unlike [`Polygon2::find_self_intersection`]'s within-one-ring
/// check) since `a` and `b` are different rings — no edge from `a` is
/// ever "the same edge" as one from `b`.
fn find_ring_intersection(a: &Polygon2, b: &Polygon2) -> Option<SegmentIntersectionKind> {
    for i in 0..a.len() {
        for j in 0..b.len() {
            let kind = segment_intersection_kind(a.edge(i), b.edge(j));
            if kind != SegmentIntersectionKind::None {
                return Some(kind);
            }
        }
    }
    None
}

/// Whether `container` fully encloses `inner` — **assumes**
/// `find_ring_intersection(container, inner)` is already known to be
/// `None` (no shared or crossing point), so a single vertex of `inner`
/// suffices: if it's on `container`'s interior side, every other vertex
/// of `inner` must be too (its own boundary never touches `container`'s,
/// by the caller's precondition), so the whole ring can't have crossed
/// back outside without an intersection the precondition already ruled
/// out.
fn ring_contains_ring(container: &Polygon2, inner: &Polygon2) -> bool {
    inner
        .vertices()
        .first()
        .is_some_and(|&v| container.relation_to(v) == PointPolygonRelation::Inside)
}

/// Validates `holes` against each other and against `outer` (already
/// assumed to have passed the same structural/self-intersection checks
/// [`triangulate_polygon`] itself performs on its own input — this
/// function does not re-check `outer` in isolation, only its
/// relationship to each hole). See
/// [`triangulate_polygon_with_holes`]'s doc comment for exactly what
/// input shape this accepts and rejects.
fn validate_holes(outer: &Polygon2, holes: &[Polygon2]) -> Result<(), PolygonTriangulationError> {
    for (i, hole) in holes.iter().enumerate() {
        match hole.basic_validity() {
            PolygonBasicValidity::Valid => {}
            other => return Err(PolygonTriangulationError::InvalidHole(i, other)),
        }
        if let Some(found) = hole.find_self_intersection() {
            return Err(PolygonTriangulationError::HoleSelfIntersecting(i, found));
        }
    }

    for (i, hole) in holes.iter().enumerate() {
        match find_ring_intersection(outer, hole) {
            Some(kind) => return Err(PolygonTriangulationError::HoleIntersectsOuter(i, kind)),
            None if !ring_contains_ring(outer, hole) => {
                return Err(PolygonTriangulationError::HoleOutsideOuter(i));
            }
            None => {}
        }
    }

    for i in 0..holes.len() {
        for j in (i + 1)..holes.len() {
            match find_ring_intersection(&holes[i], &holes[j]) {
                Some(kind) => return Err(PolygonTriangulationError::HolesIntersect(i, j, kind)),
                None if ring_contains_ring(&holes[j], &holes[i]) => {
                    return Err(PolygonTriangulationError::NestedHole(i, j));
                }
                None if ring_contains_ring(&holes[i], &holes[j]) => {
                    return Err(PolygonTriangulationError::NestedHole(j, i));
                }
                None => {}
            }
        }
    }

    Ok(())
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

    // -- triangulate_polygon_with_holes ------------------------------

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
        Polygon2::new(vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)])
    }

    fn rect_cw(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
        Polygon2::new(vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0)])
    }

    fn tri_centroid(tri: crate::primitives::Triangle2) -> Point2 {
        let (a, b, c) = (tri.a(), tri.b(), tri.c());
        p((a.x() + b.x() + c.x()) / 3.0, (a.y() + b.y() + c.y()) / 3.0)
    }

    fn assert_boundary_edges_preserved(ring: &Polygon2, t: &Triangulation2) {
        let vid_of = |pt: Point2| -> VertexId {
            t.vertices()
                .find(|&(_, q)| q == pt)
                .unwrap_or_else(|| panic!("{pt:?} not a triangulation vertex"))
                .0
        };
        for i in 0..ring.len() {
            let edge = ring.edge(i);
            let (u, v) = (vid_of(edge.a()), vid_of(edge.b()));
            let exists = t.edges().any(|e| {
                let (a, b) = t.edge_vertices(e);
                (a == u && b == v) || (a == v && b == u)
            });
            assert!(exists, "boundary edge {i} ({edge:?}) missing from result");
        }
    }

    /// Every acceptance criterion from `ROADMAP.md`'s 0.4.0 section: triangle
    /// count (`n + 2h - 2`), every triangle CCW, area conservation (outer
    /// area minus every hole's area), every triangle's centroid inside
    /// `outer` and outside every hole, every outer/hole boundary edge
    /// surviving, and topology validity (ignoring the same hull-coverage
    /// caveat `triangulate_polygon`'s own tests already document).
    fn assert_valid_holes_result(outer: &Polygon2, holes: &[Polygon2], t: &Triangulation2) {
        let n: usize = outer.len() + holes.iter().map(|h| h.len()).sum::<usize>();
        let h = holes.len();
        assert_eq!(t.len(), n + 2 * h - 2, "triangle count");

        for tri in t.triangles() {
            assert_eq!(
                tri.orientation(),
                Orientation::CounterClockwise,
                "not CCW: {tri:?}"
            );
        }

        let outer_area = outer.signed_area().abs();
        let holes_area: f64 = holes.iter().map(|hh| hh.signed_area().abs()).sum();
        let total_area: f64 = t.triangles().iter().map(|&tri| tri_area(tri)).sum();
        let want_area = outer_area - holes_area;
        assert!(
            (total_area - want_area).abs() < 1e-6,
            "area conservation: got {total_area}, want {want_area}"
        );

        for &tri in t.triangles() {
            let c = tri_centroid(tri);
            assert_eq!(
                outer.relation_to(c),
                PointPolygonRelation::Inside,
                "triangle centroid not inside outer: {tri:?}"
            );
            for hh in holes {
                assert_eq!(
                    hh.relation_to(c),
                    PointPolygonRelation::Outside,
                    "triangle centroid inside a hole: {tri:?}"
                );
            }
        }

        assert_boundary_edges_preserved(outer, t);
        for hh in holes {
            assert_boundary_edges_preserved(hh, t);
        }

        let errs = validate_ignoring_hull_coverage(t);
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn square_with_a_square_hole() {
        let outer = rect(0.0, 0.0, 4.0, 4.0);
        let holes = [rect(1.0, 1.0, 2.0, 2.0)];
        let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();
        assert_valid_holes_result(&outer, &holes, &t);
    }

    #[test]
    fn multiple_holes() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        let holes = [rect(1.0, 1.0, 3.0, 3.0), rect(6.0, 6.0, 8.0, 8.0)];
        let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();
        assert_valid_holes_result(&outer, &holes, &t);
    }

    /// A U-shaped (non-convex) outer boundary -- a wide notch cut from the
    /// top leaves a narrow-ish bridge of material at the bottom -- with a
    /// hole placed in that bridge, combining `triangulate_polygon`'s
    /// existing non-convex handling with the new hole-discarding path.
    #[test]
    fn narrow_channel_outer_boundary_with_a_hole() {
        let outer = Polygon2::new(vec![
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(7.0, 10.0),
            p(7.0, 3.0),
            p(3.0, 3.0),
            p(3.0, 10.0),
            p(0.0, 10.0),
        ]);
        let holes = [rect(4.0, 0.5, 6.0, 1.5)];
        let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();
        assert_valid_holes_result(&outer, &holes, &t);
    }

    #[test]
    fn hole_very_close_to_the_outer_boundary() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        let holes = [rect(0.1, 0.1, 2.0, 2.0)];
        let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();
        assert_valid_holes_result(&outer, &holes, &t);
    }

    #[test]
    fn cw_outer_ccw_hole() {
        let outer = rect_cw(0.0, 0.0, 4.0, 4.0);
        assert_eq!(outer.orientation(), Orientation::Clockwise);
        let holes = [rect(1.0, 1.0, 2.0, 2.0)];
        assert_eq!(holes[0].orientation(), Orientation::CounterClockwise);
        let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();
        assert_valid_holes_result(&outer, &holes, &t);
    }

    #[test]
    fn ccw_outer_cw_hole() {
        let outer = rect(0.0, 0.0, 4.0, 4.0);
        assert_eq!(outer.orientation(), Orientation::CounterClockwise);
        let holes = [rect_cw(1.0, 1.0, 2.0, 2.0)];
        assert_eq!(holes[0].orientation(), Orientation::Clockwise);
        let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();
        assert_valid_holes_result(&outer, &holes, &t);
    }

    #[test]
    fn hole_entirely_outside_outer_is_rejected() {
        let outer = rect(0.0, 0.0, 4.0, 4.0);
        let holes = [rect(10.0, 10.0, 12.0, 12.0)];
        assert_eq!(
            triangulate_polygon_with_holes(&outer, &holes),
            Err(PolygonTriangulationError::HoleOutsideOuter(0))
        );
    }

    #[test]
    fn hole_touching_the_outer_boundary_is_rejected() {
        let outer = rect(0.0, 0.0, 4.0, 4.0);
        // Diamond with one vertex exactly on outer's bottom edge (2,0),
        // not collinear with either of the diamond's own adjacent edges --
        // a clean single-point touch, not an overlapping-edge case.
        let holes = [Polygon2::new(vec![
            p(2.0, 0.0),
            p(3.0, 1.0),
            p(2.0, 2.0),
            p(1.0, 1.0),
        ])];
        match triangulate_polygon_with_holes(&outer, &holes) {
            Err(PolygonTriangulationError::HoleIntersectsOuter(
                0,
                SegmentIntersectionKind::EndpointTouch,
            )) => {}
            other => panic!("expected HoleIntersectsOuter(0, EndpointTouch), got {other:?}"),
        }
    }

    #[test]
    fn hole_crossing_the_outer_boundary_is_rejected() {
        let outer = rect(0.0, 0.0, 4.0, 4.0);
        // Straddles outer's (0,0) corner: partly inside, partly outside.
        let holes = [rect(-1.0, -1.0, 1.0, 1.0)];
        match triangulate_polygon_with_holes(&outer, &holes) {
            Err(PolygonTriangulationError::HoleIntersectsOuter(
                0,
                SegmentIntersectionKind::Proper,
            )) => {}
            other => panic!("expected HoleIntersectsOuter(0, Proper), got {other:?}"),
        }
    }

    #[test]
    fn holes_touching_each_other_is_rejected() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        // hole1's right edge (x=3) is collinear with and fully overlaps
        // hole2's left edge (x=3, y in 1..3) -- but `find_ring_intersection`
        // reports the *first* found intersection in edge-index order (same
        // "not necessarily the geometric first" convention as
        // `Polygon2::find_self_intersection`), and hole1's *bottom* edge
        // touches hole2's bottom edge at the single point (3,1) first, so
        // that's the kind actually returned, not the fully-overlapping
        // vertical edge pair checked later.
        let holes = [rect(1.0, 1.0, 3.0, 3.0), rect(3.0, 1.0, 5.0, 3.0)];
        match triangulate_polygon_with_holes(&outer, &holes) {
            Err(PolygonTriangulationError::HolesIntersect(
                0,
                1,
                SegmentIntersectionKind::CollinearTouch,
            )) => {}
            other => panic!("expected HolesIntersect(0, 1, CollinearTouch), got {other:?}"),
        }
    }

    #[test]
    fn holes_crossing_each_other_is_rejected() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        let holes = [rect(1.0, 1.0, 3.0, 3.0), rect(2.0, 2.0, 4.0, 4.0)];
        match triangulate_polygon_with_holes(&outer, &holes) {
            Err(PolygonTriangulationError::HolesIntersect(
                0,
                1,
                SegmentIntersectionKind::Proper,
            )) => {}
            other => panic!("expected HolesIntersect(0, 1, Proper), got {other:?}"),
        }
    }

    #[test]
    fn nested_hole_is_rejected() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        let holes = [rect(1.0, 1.0, 5.0, 5.0), rect(2.0, 2.0, 3.0, 3.0)];
        assert_eq!(
            triangulate_polygon_with_holes(&outer, &holes),
            Err(PolygonTriangulationError::NestedHole(1, 0))
        );
    }

    #[test]
    fn hole_with_too_few_vertices_is_rejected() {
        let outer = rect(0.0, 0.0, 4.0, 4.0);
        let holes = [Polygon2::new(vec![p(1.0, 1.0), p(2.0, 1.0)])];
        assert_eq!(
            triangulate_polygon_with_holes(&outer, &holes),
            Err(PolygonTriangulationError::InvalidHole(
                0,
                PolygonBasicValidity::TooFewVertices
            ))
        );
    }

    #[test]
    fn self_intersecting_hole_is_rejected() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        // Same asymmetric-bowtie shape as `self_intersecting_bowtie_is_rejected`,
        // translated inside `outer`.
        let holes = [Polygon2::new(vec![
            p(1.0, 1.0),
            p(7.0, 7.0),
            p(7.0, 1.0),
            p(1.0, 3.0),
        ])];
        match triangulate_polygon_with_holes(&outer, &holes) {
            Err(PolygonTriangulationError::HoleSelfIntersecting(0, found)) => {
                assert_eq!(found.edge_a, 0);
                assert_eq!(found.edge_b, 2);
            }
            other => panic!("expected HoleSelfIntersecting, got {other:?}"),
        }
    }

    #[test]
    fn deterministic_regardless_of_hole_starting_vertex() {
        let outer = rect(0.0, 0.0, 10.0, 10.0);
        let hole_a = rect(2.0, 2.0, 4.0, 4.0);
        // Same ring, rotated to a different starting vertex.
        let hole_b = Polygon2::new(vec![p(4.0, 4.0), p(2.0, 4.0), p(2.0, 2.0), p(4.0, 2.0)]);
        let t_a = triangulate_polygon_with_holes(&outer, &[hole_a]).unwrap();
        let t_b = triangulate_polygon_with_holes(&outer, &[hole_b]).unwrap();
        assert_eq!(t_a.triangles(), t_b.triangles());
    }

    #[test]
    fn no_holes_matches_triangulate_polygon() {
        let outer = Polygon2::new(vec![
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 2.0),
            p(2.0, 2.0),
            p(2.0, 4.0),
            p(0.0, 4.0),
        ]);
        let a = triangulate_polygon(&outer).unwrap();
        let b = triangulate_polygon_with_holes(&outer, &[]).unwrap();
        assert_eq!(a.triangles(), b.triangles());
    }
}
