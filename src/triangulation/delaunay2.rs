use std::collections::{HashMap, HashSet};

use super::ids::{EdgeId, FaceId, VertexId};
use crate::hull::{HullBoundaryPoints, convex_hull2, dedup_sorted};
use crate::predicates::{Orientation, Sign, incircle, orient2d};
use crate::primitives::{Point2, Triangle2};

/// A 2D Delaunay triangulation: a set of non-overlapping,
/// counterclockwise-wound [`Triangle2`]s whose union is the input's convex
/// hull, with no input point strictly inside any triangle's circumcircle,
/// plus the adjacency between them (§6B, ADR-006).
///
/// [`Triangulation2::triangles`] keeps its original flat, coordinate-only
/// view for callers that don't need topology. [`VertexId`]/[`EdgeId`]/
/// [`FaceId`] and the query methods below (`vertices`, `edges`, `faces`,
/// `edge_vertices`, `adjacent_faces`, `face_vertices`, `neighboring_faces`,
/// `boundary_edges`) expose the underlying indexed-triangle-adjacency
/// structure ADR-006 designed, needed for constrained Delaunay (Phase 6C)
/// and beyond.
///
/// This is a **static, post-construction snapshot** — there is no public
/// mutation API, so IDs are plain indices into fixed-size arrays, not the
/// generational handles a *mutating* structure would need. ADR-006's
/// generational-arena proposal is scoped to construction-time mutation
/// (Bowyer-Watson's own insertion loop, still internal and index-churning
/// — see `insert_point`), not this frozen view of its result.
#[derive(Debug, Clone, PartialEq)]
pub struct Triangulation2 {
    /// Coordinates, kept for `triangles()`'s pre-existing contract.
    triangles: Vec<Triangle2>,
    /// Canonical vertex list; `VertexId(i)` indexes here. Deliberately
    /// redundant with `triangles`'s own coordinates (see `faces` below).
    vertices: Vec<Point2>,
    /// Parallel to `triangles`: the same triangle as 3 `VertexId`s.
    /// Storing both this and `triangles` (coordinates) is deliberate, not
    /// an oversight — it keeps `triangles()`'s existing `&[Triangle2]`
    /// signature exactly as it was before this structure existed, at the
    /// cost of a few extra words per triangle.
    faces: Vec<[VertexId; 3]>,
    /// Parallel to `triangles`/`faces`: `face_neighbors[i][k]` is the face
    /// across the edge **opposite vertex `k`** of face `i` — edge 0 is
    /// `(faces[i][1], faces[i][2])` opposite `faces[i][0]`, edge 1 is
    /// `(faces[i][2], faces[i][0])` opposite `faces[i][1]`, edge 2 is
    /// `(faces[i][0], faces[i][1])` opposite `faces[i][2]` — `None` at the
    /// triangulation's outer boundary. See
    /// [`Triangulation2::neighboring_faces`].
    face_neighbors: Vec<[Option<FaceId>; 3]>,
    /// Canonical, deduplicated undirected edge list; `EdgeId(i)` indexes
    /// here and into `edge_faces`.
    edges: Vec<(VertexId, VertexId)>,
    /// Parallel to `edges`: the 1 (boundary) or 2 (interior) face(s)
    /// incident to that edge, in no particular order.
    edge_faces: Vec<[Option<FaceId>; 2]>,
}

impl Triangulation2 {
    fn empty() -> Self {
        Triangulation2 {
            triangles: Vec::new(),
            vertices: Vec::new(),
            faces: Vec::new(),
            face_neighbors: Vec::new(),
            edges: Vec::new(),
            edge_faces: Vec::new(),
        }
    }

    /// The triangulation's triangles (coordinates only), in no particular
    /// order.
    pub fn triangles(&self) -> &[Triangle2] {
        &self.triangles
    }

    /// The number of triangles.
    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    /// `true` iff there are no triangles (fewer than 3 non-collinear input
    /// points).
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// Every vertex, paired with its coordinate.
    pub fn vertices(&self) -> impl Iterator<Item = (VertexId, Point2)> + '_ {
        self.vertices
            .iter()
            .enumerate()
            .map(|(i, &p)| (VertexId(i as u32), p))
    }

    /// Every undirected edge's id.
    pub fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        (0..self.edges.len()).map(|i| EdgeId(i as u32))
    }

    /// Every face's id.
    pub fn faces(&self) -> impl Iterator<Item = FaceId> + '_ {
        (0..self.faces.len()).map(|i| FaceId(i as u32))
    }

    /// `edge`'s two endpoints, in no particular order.
    pub fn edge_vertices(&self, edge: EdgeId) -> (VertexId, VertexId) {
        self.edges[edge.0 as usize]
    }

    /// The 1 (boundary) or 2 (interior) face(s) incident to `edge`, in no
    /// particular order; `None` in the second slot at the boundary.
    pub fn adjacent_faces(&self, edge: EdgeId) -> [Option<FaceId>; 2] {
        self.edge_faces[edge.0 as usize]
    }

    /// `face`'s 3 vertices, counterclockwise.
    pub fn face_vertices(&self, face: FaceId) -> [VertexId; 3] {
        self.faces[face.0 as usize]
    }

    /// `face`'s neighbor across each edge; `None` at the triangulation's
    /// outer boundary.
    ///
    /// Index `k` in the returned array is the face across the edge
    /// **opposite** `face_vertices(face)[k]` (the edge *not* touching
    /// that vertex): index 0 is opposite vertex 0 (the edge between
    /// vertices 1 and 2), index 1 opposite vertex 1 (between vertices 2
    /// and 0), index 2 opposite vertex 2 (between vertices 0 and 1).
    /// `face_vertices` and `neighboring_faces` always agree on this
    /// convention for the same `face` — a caller walking adjacency (e.g.
    /// an edge-flip implementation) can rely on the same `k` indexing
    /// both.
    pub fn neighboring_faces(&self, face: FaceId) -> [Option<FaceId>; 3] {
        self.face_neighbors[face.0 as usize]
    }

    /// Every edge with exactly one incident face — the triangulation's
    /// outer boundary.
    pub fn boundary_edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        self.edges()
            .filter(move |&e| self.adjacent_faces(e)[1].is_none())
    }
}

/// Sentinel triangle-vertex index representing the single symbolic "point
/// at infinity" — never a real index into `pts` (`pts.len()` is always far
/// below `usize::MAX`).
const GHOST: usize = usize::MAX;

fn is_ghost(idx: usize) -> bool {
    idx == GHOST
}

/// The 2D Delaunay triangulation of `points`, via Bowyer-Watson incremental
/// insertion with a symbolic "point at infinity" standing in for a
/// synthetic bounding triangle.
///
/// Duplicate points (exact coordinate equality) are collapsed first, same
/// policy as [`crate::convex_hull2`]. Points are inserted in the same
/// canonical sorted order `convex_hull2` uses, not input order — see
/// "Determinism and cocircular points" below. Degenerate inputs (fewer than
/// 3 distinct points, or all points collinear) return an empty
/// triangulation rather than an error, matching this crate's usual
/// "degenerate is a valid, representable value" policy.
///
/// # Algorithm
///
/// The first 3 non-collinear points (in canonical sorted order) form the
/// initial real triangle; its 3 outer edges are each paired with a single
/// symbolic ghost vertex (`GHOST`, no coordinate) representing "point at
/// infinity", so the ghost has a closed triangle fan around it exactly like
/// any real interior point would. Each remaining point is then inserted in
/// turn via the standard cavity construction: every existing triangle whose
/// circumcircle strictly contains the new point is removed, opening a
/// star-shaped cavity that gets re-triangulated by connecting the new point
/// to every edge on the cavity's boundary. "Circumcircle contains the new
/// point" is evaluated by plain [`incircle`] when a triangle has no ghost
/// vertex, or reduces to a half-plane [`orient2d`] test against the
/// triangle's one real edge when it has exactly one (the limit of a circle
/// through a point receding to infinity) — see `is_bad`. A triangle can
/// never have more than one ghost vertex (proven by induction: the starting
/// triangles have at most one, and every new triangle is formed from an
/// existing triangle's edge plus a real point, which can add a ghost only
/// if the edge itself already carried one).
///
/// Once every point has been inserted, every triangle still carrying the
/// ghost vertex is dropped — **every vertex in the returned triangulation
/// is a value copied from the original input**, never a synthetic
/// coordinate, and (unlike a bounding-triangle approach) no arithmetic ever
/// touches a synthetic coordinate either, so there is no super-triangle
/// sizing tradeoff to document: the outer region is handled exactly, at any
/// input scale or aspect ratio (see `tests/differential/delaunay2.rs`'s
/// `near_collinear_cluster_with_a_far_outlier`).
///
/// # Determinism and cocircular points
///
/// Output is deterministic: points are canonically sorted before
/// insertion, so the result is a function of the input *set*, not its
/// order. This does **not** mean the triangulation is the unique
/// mathematically-canonical one for every input — when 4 or more points
/// are exactly cocircular, more than one triangulation satisfies the
/// empty-circumcircle property, and *which* one comes out depends on
/// insertion order (a tie-break rule, not a derived fact). This crate's
/// tie-break: a point exactly on a triangle's circumcircle boundary
/// (`Sign::Zero`) does not make that triangle "bad" — it is not removed.
/// Combined with the canonical sort, this makes the tie-break itself
/// deterministic, but a caller comparing this triangulation's diagonal
/// choice on a cocircular quad against another Delaunay implementation
/// should not expect them to agree.
pub fn delaunay2(points: &[Point2]) -> Triangulation2 {
    let pts = dedup_sorted(points);
    let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
    if hull.len() < 3 {
        return Triangulation2::empty();
    }

    // First 3 non-collinear points in sorted order. `pts[0]`/`pts[1]` fixed
    // and scanning forward always finds one: if every `pts[i]` were
    // collinear with `pts[0]`,`pts[1]`, the whole set would be collinear,
    // contradicting the hull check above.
    let mut ic = 2;
    while orient2d(pts[0], pts[1], pts[ic]) == Orientation::Collinear {
        ic += 1;
    }
    let (mut ia, mut ib) = (0, 1);
    if orient2d(pts[ia], pts[ib], pts[ic]) == Orientation::Clockwise {
        std::mem::swap(&mut ia, &mut ib);
    }

    // The real triangle plus a closed ghost fan around its 3 outer edges,
    // each stored so the ghost sits on the correct (outward) side — see
    // `is_bad`'s single-ghost case.
    let mut tris: Vec<[usize; 3]> = vec![
        [ia, ib, ic],
        [ib, ia, GHOST],
        [ic, ib, GHOST],
        [ia, ic, GHOST],
    ];

    for i in 0..pts.len() {
        if i == ia || i == ib || i == ic {
            continue;
        }
        insert_point(&mut tris, &pts, i);
    }

    build_topology(pts, tris)
}

/// Builds the public topology structure (§6B, ADR-006) from the raw
/// vertex-index triangle list Bowyer-Watson produces internally, once
/// every point has been inserted. `pts` becomes `Triangulation2::vertices`
/// directly (already deduplicated by the caller); every point in it ends
/// up as a vertex of at least one surviving real triangle (every inserted
/// point's cavity re-triangulation always creates at least one triangle
/// incident to it, and points are never later removed), so no further
/// filtering of `pts` itself is needed here.
fn build_topology(pts: Vec<Point2>, tris: Vec<[usize; 3]>) -> Triangulation2 {
    let real: Vec<[usize; 3]> = tris
        .into_iter()
        .filter(|t| t.iter().all(|&idx| !is_ghost(idx)))
        .collect();

    let faces: Vec<[VertexId; 3]> = real
        .iter()
        .map(|&[a, b, c]| [VertexId(a as u32), VertexId(b as u32), VertexId(c as u32)])
        .collect();

    assemble_triangulation(pts, faces)
}

/// Builds the full [`Triangulation2`] (coordinates, adjacency, edge
/// table) from a vertex list and a ghost-free face list. Shared between
/// Bowyer-Watson's output (`build_topology` above, after stripping ghost
/// triangles) and Phase 6C's constrained Delaunay (`super::cdt`, after
/// its edge-flip passes finish) — both end up needing exactly this same
/// "derive adjacency from a flat face list" step, and duplicating it would
/// mean two independent implementations of the same edge/neighbor-table
/// construction to keep in sync.
pub(super) fn assemble_triangulation(
    pts: Vec<Point2>,
    faces: Vec<[VertexId; 3]>,
) -> Triangulation2 {
    let triangles: Vec<Triangle2> = faces
        .iter()
        .map(|&[a, b, c]| Triangle2::new(pts[a.0 as usize], pts[b.0 as usize], pts[c.0 as usize]))
        .collect();

    // Canonical undirected-edge key: (min, max) by VertexId's own index,
    // so the same edge visited from either incident face (in either
    // winding position) hashes the same.
    let edge_key = |u: VertexId, v: VertexId| -> (u32, u32) {
        if u.0 <= v.0 { (u.0, v.0) } else { (v.0, u.0) }
    };

    let mut edge_index: HashMap<(u32, u32), usize> = HashMap::new();
    let mut edges: Vec<(VertexId, VertexId)> = Vec::new();
    // Every (FaceId, local_edge_index) bordering each canonical edge, in
    // insertion order -- feeds both `edge_faces` and `face_neighbors`
    // below. A well-formed planar triangulation never has more than 2 per
    // edge; `Triangulation2::validate_topology` checks this independently
    // rather than this builder asserting it.
    let mut edge_incidences: Vec<Vec<(FaceId, usize)>> = Vec::new();

    for (i, face) in faces.iter().enumerate() {
        let face_id = FaceId(i as u32);
        // Local edge k is opposite vertex k: k=0 -> (v1,v2), k=1 ->
        // (v2,v0), k=2 -> (v0,v1) -- see `Triangulation2::neighboring_faces`.
        let local_edges = [(face[1], face[2]), (face[2], face[0]), (face[0], face[1])];
        for (k, &(u, v)) in local_edges.iter().enumerate() {
            let key = edge_key(u, v);
            let idx = *edge_index.entry(key).or_insert_with(|| {
                edges.push((u, v));
                edge_incidences.push(Vec::new());
                edges.len() - 1
            });
            edge_incidences[idx].push((face_id, k));
        }
    }

    let mut edge_faces: Vec<[Option<FaceId>; 2]> = vec![[None, None]; edges.len()];
    let mut face_neighbors: Vec<[Option<FaceId>; 3]> = vec![[None, None, None]; faces.len()];
    for (idx, incidences) in edge_incidences.iter().enumerate() {
        for (slot, &(face_id, _)) in incidences.iter().take(2).enumerate() {
            edge_faces[idx][slot] = Some(face_id);
        }
        if incidences.len() == 2 {
            let (fa, ka) = incidences[0];
            let (fb, kb) = incidences[1];
            face_neighbors[fa.0 as usize][ka] = Some(fb);
            face_neighbors[fb.0 as usize][kb] = Some(fa);
        }
    }

    Triangulation2 {
        triangles,
        vertices: pts,
        faces,
        face_neighbors,
        edges,
        edge_faces,
    }
}

/// A structural invariant violated in a [`Triangulation2`], found by
/// [`Triangulation2::validate_topology`].
///
/// `pub` (not `pub(crate)`) so this crate's own `tests/` integration
/// suite and `fuzz/` targets — both, per Rust's crate-privacy rules,
/// external to `kika` for visibility purposes even though they live in
/// this repository — can reach it, but `#[doc(hidden)]`: not yet a real,
/// advertised public API commitment (ADR-006's "expose only when a real
/// consumer needs it" — promoted only as far as Rust's own visibility
/// rules require, not further).
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub enum TopologyError {
    /// A face is not counterclockwise (or is degenerate).
    FaceNotCcw(FaceId),
    /// An edge is bordered by a number of faces other than 1 or 2 —
    /// independently recomputed from `faces`, not read back from the
    /// cached `edges`/`edge_faces` tables (which is exactly what this
    /// check is verifying).
    NonManifoldEdge {
        u: VertexId,
        v: VertexId,
        incident_faces: usize,
    },
    /// `face_neighbors` isn't reciprocal for this edge: one side doesn't
    /// point back at the other.
    AdjacencyMismatch { face: FaceId, local_edge: usize },
    /// The triangle count doesn't satisfy Euler's formula `2n - 2 - h` for
    /// the (non-collinear) vertex set.
    EulerFormulaViolated { triangles: usize, expected: isize },
    /// An interior edge fails the local Delaunay (empty-circumcircle)
    /// property: one side's opposite vertex lies strictly inside the
    /// other side's circumcircle.
    NotLocallyDelaunay { u: VertexId, v: VertexId },
}

impl Triangulation2 {
    /// Checks every structural invariant this triangulation is supposed to
    /// satisfy: every face counterclockwise, every edge bordered by
    /// exactly 1 or 2 faces, `face_neighbors` reciprocal, Euler's formula,
    /// and every interior edge locally Delaunay (empty-circumcircle, via
    /// `incircle`). With no constrained edges yet (Phase 6C), "every
    /// interior edge" and "every *unconstrained* interior edge" coincide
    /// — this is the check Phase 6C's constrained Delaunay narrows to
    /// unconstrained edges only, not a new one.
    ///
    /// Returns every violation found, not just the first: a real
    /// regression often fails several checks at once from the same root
    /// cause, and seeing all of them aids diagnosis. `pub` +
    /// `#[doc(hidden)]` — see [`TopologyError`]'s doc comment for why.
    #[doc(hidden)]
    pub fn validate_topology(&self) -> Vec<TopologyError> {
        self.validate_topology_excluding(&|_| false)
    }

    /// Same checks as [`Triangulation2::validate_topology`], but the
    /// local-Delaunay check skips any edge for which `excluded` returns
    /// `true` — a constrained edge (Phase 6C) is allowed, expected even,
    /// to violate local-Delaunay, since it must never be flipped away
    /// regardless. `validate_topology` itself is exactly this with
    /// nothing excluded (no constraints ⇒ every edge unconstrained ⇒
    /// identical coverage to before this method existed).
    pub(crate) fn validate_topology_excluding(
        &self,
        excluded: &dyn Fn(EdgeId) -> bool,
    ) -> Vec<TopologyError> {
        let mut errors = Vec::new();

        for (i, tri) in self.triangles.iter().enumerate() {
            if tri.orientation() != Orientation::CounterClockwise {
                errors.push(TopologyError::FaceNotCcw(FaceId(i as u32)));
            }
        }

        // Independently recomputed edge incidence, from `faces` alone --
        // this check must not trust `edges`/`edge_faces`, since that is
        // exactly what it exists to verify.
        let mut incidence: HashMap<(u32, u32), Vec<(FaceId, usize)>> = HashMap::new();
        for (i, face) in self.faces.iter().enumerate() {
            let face_id = FaceId(i as u32);
            let local_edges = [(face[1], face[2]), (face[2], face[0]), (face[0], face[1])];
            for (k, &(u, v)) in local_edges.iter().enumerate() {
                let key = if u.0 <= v.0 { (u.0, v.0) } else { (v.0, u.0) };
                incidence.entry(key).or_default().push((face_id, k));
            }
        }

        // Maps a canonical vertex-pair key to its EdgeId, so the
        // caller-supplied `excluded` predicate (which takes an EdgeId) can
        // be checked here -- built from `self.edges`, independent of the
        // `incidence` map above.
        let edge_id_of: HashMap<(u32, u32), EdgeId> = self
            .edges
            .iter()
            .enumerate()
            .map(|(i, &(u, v))| {
                let key = if u.0 <= v.0 { (u.0, v.0) } else { (v.0, u.0) };
                (key, EdgeId(i as u32))
            })
            .collect();

        for (&(u, v), incident) in &incidence {
            if incident.len() != 1 && incident.len() != 2 {
                errors.push(TopologyError::NonManifoldEdge {
                    u: VertexId(u),
                    v: VertexId(v),
                    incident_faces: incident.len(),
                });
                continue;
            }
            if incident.len() == 2 {
                let (fa, ka) = incident[0];
                let (fb, kb) = incident[1];
                let claims_a = self.face_neighbors[fa.0 as usize][ka] == Some(fb);
                let claims_b = self.face_neighbors[fb.0 as usize][kb] == Some(fa);
                if !claims_a || !claims_b {
                    errors.push(TopologyError::AdjacencyMismatch {
                        face: fa,
                        local_edge: ka,
                    });
                }

                let is_excluded = edge_id_of.get(&(u, v)).is_some_and(|&id| excluded(id));
                if !is_excluded {
                    let opposite_b = self.vertices[self.faces[fb.0 as usize][kb].0 as usize];
                    let opposite_a = self.vertices[self.faces[fa.0 as usize][ka].0 as usize];
                    let tri_a = self.triangles[fa.0 as usize];
                    let tri_b = self.triangles[fb.0 as usize];
                    let a_contains_b = incircle(tri_a.a(), tri_a.b(), tri_a.c(), opposite_b);
                    let b_contains_a = incircle(tri_b.a(), tri_b.b(), tri_b.c(), opposite_a);
                    if a_contains_b == Sign::Positive || b_contains_a == Sign::Positive {
                        errors.push(TopologyError::NotLocallyDelaunay {
                            u: VertexId(u),
                            v: VertexId(v),
                        });
                    }
                }
            }
        }

        if !self.vertices.is_empty() {
            let hull = convex_hull2(&self.vertices, HullBoundaryPoints::KeepAllOnBoundary);
            if hull.orientation() != Orientation::Collinear {
                let n = self.vertices.len() as isize;
                let h = hull.len() as isize;
                let expected = 2 * n - 2 - h;
                if self.triangles.len() as isize != expected {
                    errors.push(TopologyError::EulerFormulaViolated {
                        triangles: self.triangles.len(),
                        expected,
                    });
                }
            }
        }

        errors
    }
}

/// Whether triangle `tri`'s circumcircle strictly contains `p`, handling
/// the (at most one) ghost vertex case as the limit of a circumcircle
/// receding to infinity: for CCW real edge `(u, v)` with the ghost as the
/// third ("far away") vertex, that limit is the half-plane strictly left of
/// `u -> v` — see `delaunay2`'s doc comment for why at most one ghost can
/// ever occur.
fn is_bad(pts: &[Point2], tri: [usize; 3], p: Point2) -> bool {
    let [a, b, c] = tri;
    match (is_ghost(a), is_ghost(b), is_ghost(c)) {
        (false, false, false) => incircle(pts[a], pts[b], pts[c], p) == Sign::Positive,
        (true, false, false) => orient2d(pts[b], pts[c], p) == Orientation::CounterClockwise,
        (false, true, false) => orient2d(pts[c], pts[a], p) == Orientation::CounterClockwise,
        (false, false, true) => orient2d(pts[a], pts[b], p) == Orientation::CounterClockwise,
        _ => unreachable!("a triangle can never carry more than one ghost vertex"),
    }
}

/// Inserts `pts[p_idx]` into `tris` via the Bowyer-Watson cavity
/// construction: find every triangle whose circumcircle strictly contains
/// the new point ("bad", see `is_bad`), remove them, and fan the resulting
/// cavity boundary to the new point.
///
/// The cavity boundary is found via directed-edge cancellation: every bad
/// triangle contributes its three CCW-ordered edges; an edge whose reverse
/// is *also* contributed by some (other) bad triangle is internal to the
/// cavity and cancels out, leaving only the boundary. This relies on the
/// bad-triangle set always being star-shaped around the new point (a
/// property of exact `incircle`/`orient2d` evaluation on a valid Delaunay
/// triangulation) — property-tested, not just assumed, in
/// `tests/differential/delaunay2.rs`.
fn insert_point(tris: &mut Vec<[usize; 3]>, pts: &[Point2], p_idx: usize) {
    let p = pts[p_idx];

    let bad: Vec<usize> = tris
        .iter()
        .enumerate()
        .filter(|&(_, &tri)| is_bad(pts, tri, p))
        .map(|(i, _)| i)
        .collect();

    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(bad.len() * 3);
    for &ti in &bad {
        let [a, b, c] = tris[ti];
        edges.push((a, b));
        edges.push((b, c));
        edges.push((c, a));
    }
    let edge_set: HashSet<(usize, usize)> = edges.iter().copied().collect();
    let boundary: Vec<(usize, usize)> = edges
        .into_iter()
        .filter(|&(u, v)| !edge_set.contains(&(v, u)))
        .collect();

    for &ti in bad.iter().rev() {
        tris.swap_remove(ti);
    }

    for (u, v) in boundary {
        tris.push([u, v, p_idx]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predicates::Orientation;
    use crate::primitives::PointTriangleRelation;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn all_ccw(t: &Triangulation2) {
        for tri in t.triangles() {
            assert_eq!(tri.orientation(), Orientation::CounterClockwise);
        }
    }

    /// `is_bad`'s single-ghost reduction was derived (and numerically
    /// checked against the old super-triangle limit) for exactly one
    /// vertex position; the other two follow the same rotational argument
    /// but were never independently verified. Rotating a triangle's vertex
    /// order can't change what triangle it represents, so all three
    /// single-ghost arms must agree for any rotation of the same
    /// (real, real, ghost) triangle.
    #[test]
    fn is_bad_single_ghost_arms_agree_under_rotation() {
        let mut rng = 0x1234_5678_9abc_def0_u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 11) as f64 * (1.0 / (1u64 << 53) as f64) * 2.0 - 1.0
        };
        for _ in 0..200 {
            let pts = vec![p(next(), next()), p(next(), next())];
            let query = p(next(), next());
            let (a, b) = (0usize, 1usize);
            let by_c = is_bad(&pts, [a, b, GHOST], query);
            let by_a = is_bad(&pts, [GHOST, a, b], query);
            let by_b = is_bad(&pts, [b, GHOST, a], query);
            assert_eq!(by_c, by_a, "ghost-at-c vs ghost-at-a disagree");
            assert_eq!(by_c, by_b, "ghost-at-c vs ghost-at-b disagree");
        }
    }

    /// The Delaunay property itself: no input point lies strictly inside
    /// any output triangle's circumcircle.
    fn empty_circumcircle_property(points: &[Point2], t: &Triangulation2) {
        for tri in t.triangles() {
            for &q in points {
                let sign = incircle(tri.a(), tri.b(), tri.c(), q);
                assert_ne!(
                    sign,
                    Sign::Positive,
                    "point {q:?} strictly inside circumcircle of {tri:?}"
                );
            }
        }
    }

    fn every_vertex_is_an_input_point(points: &[Point2], t: &Triangulation2) {
        for tri in t.triangles() {
            for v in [tri.a(), tri.b(), tri.c()] {
                assert!(points.contains(&v), "triangle vertex {v:?} not in input");
            }
        }
    }

    #[test]
    fn empty_input() {
        let t = delaunay2(&[]);
        assert!(t.is_empty());
    }

    #[test]
    fn one_and_two_points() {
        assert!(delaunay2(&[p(0.0, 0.0)]).is_empty());
        assert!(delaunay2(&[p(0.0, 0.0), p(1.0, 1.0)]).is_empty());
    }

    #[test]
    fn fully_collinear_input() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)];
        assert!(delaunay2(&pts).is_empty());
    }

    #[test]
    fn single_triangle() {
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)];
        let t = delaunay2(&pts);
        assert_eq!(t.len(), 1);
        all_ccw(&t);
        every_vertex_is_an_input_point(&pts, &t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn square_gives_two_triangles() {
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let t = delaunay2(&pts);
        assert_eq!(t.len(), 2);
        all_ccw(&t);
        every_vertex_is_an_input_point(&pts, &t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn square_with_center_point_gives_four_triangles() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        let t = delaunay2(&pts);
        // n=5, h=4 hull vertices, no 3 collinear: 2n - 2 - h = 4.
        assert_eq!(t.len(), 4);
        all_ccw(&t);
        every_vertex_is_an_input_point(&pts, &t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn point_on_interior_edge_splits_both_adjacent_triangles() {
        // Two triangles sharing edge (2,2)-(0,0)/(4,0)... concretely: a
        // square split by inserting a point exactly on its diagonal.
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0), // exactly on the (0,0)-(4,4) diagonal
        ];
        let t = delaunay2(&pts);
        // The point on the shared diagonal must split both triangles that
        // would otherwise meet there into 2 each: 4 total, matching the
        // 2n-2-h count above but for a different, degenerate-input reason.
        assert_eq!(t.len(), 4);
        all_ccw(&t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn point_on_hull_boundary_edge_splits_one_triangle() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(0.0, 4.0),
            p(2.0, 0.0), // exactly on the (0,0)-(4,0) hull edge
        ];
        let t = delaunay2(&pts);
        assert_eq!(t.len(), 2);
        all_ccw(&t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn cocircular_square_plus_center_is_stable_across_permutations() {
        // A square's 4 corners are exactly cocircular; the 5th point sits
        // off-center, breaking the tie among the corners but stressing the
        // Sign::Zero "not bad" rule for the corner-only circle.
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(1.0, 1.0),
        ];
        let base = delaunay2(&pts);
        empty_circumcircle_property(&pts, &base);
        all_ccw(&base);
        let mut shuffled = pts;
        shuffled.reverse();
        let other = delaunay2(&shuffled);
        assert_eq!(base.triangles(), other.triangles());
    }

    #[test]
    fn triangulation_covers_the_convex_hull() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(1.0, 1.0),
            p(3.0, 1.0),
            p(2.0, 3.0),
        ];
        let t = delaunay2(&pts);
        // Every input point must be inside-or-on some output triangle.
        for &q in &pts {
            let covered = t
                .triangles()
                .iter()
                .any(|tri| tri.relation_to(q) != PointTriangleRelation::Outside);
            assert!(covered, "point {q:?} not covered by any triangle");
        }
    }

    #[test]
    fn ghost_vertex_never_leaks_into_output() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(1.0, 1.0),
        ];
        let t = delaunay2(&pts);
        every_vertex_is_an_input_point(&pts, &t);
    }

    #[test]
    fn topology_empty_and_single_triangle_do_not_panic() {
        let empty = delaunay2(&[]);
        assert_eq!(empty.vertices().count(), 0);
        assert_eq!(empty.edges().count(), 0);
        assert_eq!(empty.faces().count(), 0);
        assert_eq!(empty.boundary_edges().count(), 0);
        assert!(empty.validate_topology().is_empty());

        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)];
        let t = delaunay2(&pts);
        assert_eq!(t.vertices().count(), 3);
        assert_eq!(t.faces().count(), 1);
        // A lone triangle: 3 edges, all boundary.
        assert_eq!(t.edges().count(), 3);
        assert_eq!(t.boundary_edges().count(), 3);
        let face = t.faces().next().unwrap();
        assert_eq!(t.neighboring_faces(face), [None, None, None]);
        assert!(t.validate_topology().is_empty());
    }

    #[test]
    fn topology_square_with_center_has_one_shared_edge_per_interior_pair() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        let t = delaunay2(&pts);
        assert_eq!(t.faces().count(), 4);
        // 4 hull-boundary edges + 4 interior spokes to the center = 8.
        assert_eq!(t.edges().count(), 8);
        assert_eq!(t.boundary_edges().count(), 4);

        // Every interior edge's two incident faces must each list the
        // other as a neighbor, at the shared edge's local index.
        for edge in t.edges() {
            let adj = t.adjacent_faces(edge);
            if let [Some(fa), Some(fb)] = adj {
                let neighbors_a = t.neighboring_faces(fa);
                let neighbors_b = t.neighboring_faces(fb);
                assert!(neighbors_a.contains(&Some(fb)));
                assert!(neighbors_b.contains(&Some(fa)));
            }
        }
        assert!(t.validate_topology().is_empty());
    }

    #[test]
    fn face_vertices_matches_triangles_coordinates() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        let t = delaunay2(&pts);
        let vertex_coords: std::collections::HashMap<VertexId, Point2> = t.vertices().collect();
        for (face, tri) in t.faces().zip(t.triangles()) {
            let [v0, v1, v2] = t.face_vertices(face);
            assert_eq!(vertex_coords[&v0], tri.a());
            assert_eq!(vertex_coords[&v1], tri.b());
            assert_eq!(vertex_coords[&v2], tri.c());
        }
    }

    /// Deliberately breaks `face_neighbors`' reciprocity to confirm
    /// `validate_topology` actually catches it, not just passes vacuously
    /// on well-formed input.
    #[test]
    fn validate_topology_catches_broken_adjacency_reciprocity() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        let mut t = delaunay2(&pts);
        assert!(t.validate_topology().is_empty());

        // Sever one direction of one interior adjacency.
        let broken = t.face_neighbors[0]
            .iter()
            .position(|n| n.is_some())
            .expect("triangle 0 has at least one interior neighbor");
        t.face_neighbors[0][broken] = None;

        let errors = t.validate_topology();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, TopologyError::AdjacencyMismatch { .. })),
            "expected an AdjacencyMismatch, got {errors:?}"
        );
    }
}
