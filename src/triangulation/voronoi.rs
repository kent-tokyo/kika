//! Voronoi diagram topology: the dual of a [`Triangulation2`], built
//! without generating any Voronoi coordinates.
//!
//! Phase 7A (ADR-007) built the internal data model, the [`voronoi2`]
//! constructor, and an internal validator. Phase 7B adds the public query
//! API below (`cells`, `vertices`, `edges`, and the accessors on each).
//! Still no coordinates (circumcenters), clipping, nearest-neighbor, or
//! ordered `cell_edges` walk -- those remain later phases. See
//! `docs/adr/ADR-007-voronoi-diagram-topology.md` for the full design and
//! the correctness argument this module implements.

use super::delaunay2::Triangulation2;
use super::ids::{EdgeId, FaceId, VertexId};
use crate::predicates::{Sign, incircle};

/// A Voronoi cell, identified with the [`VertexId`] of the Delaunay site
/// it surrounds -- a true bijection, so no separate cell table is kept.
///
/// Valid only for the [`Voronoi2`] that produced it -- comparing or
/// indexing with a handle from a different `Voronoi2` is not checked and
/// will silently look up the wrong element or panic (same convention as
/// [`VertexId`]'s own cross-triangulation caveat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiCellId(pub(super) VertexId);

/// A Voronoi vertex: the dual of one connected group of Delaunay faces
/// merged by cocircularity (§"Cocircular tie-break normalization",
/// ADR-007). Usually one face, but every face in a cocircular cluster
/// (e.g. all faces of a triangulated square) shares a single
/// `VoronoiVertexId`, since they share a single true circumcenter.
///
/// See [`VoronoiCellId`]'s doc comment for the cross-`Voronoi2` caveat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiVertexId(pub(super) u32);

/// A Voronoi edge: the dual of a Delaunay edge whose two sides fall in
/// different face groups (an excluded same-group Delaunay edge is a
/// spurious artifact of the input triangulation's tie-break, not a real
/// Voronoi edge -- see [`voronoi2`]).
///
/// See [`VoronoiCellId`]'s doc comment for the cross-`Voronoi2` caveat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiEdgeId(pub(super) u32);

/// Whether a [`VoronoiEdge`] is a finite segment between two Voronoi
/// vertices, or an infinite ray from one Voronoi vertex out to infinity.
///
/// `#[non_exhaustive]`: closed only for the current problem scope (≥3
/// non-collinear sites). Degenerate inputs (0-2 sites, or all sites
/// collinear) will need a future `Line`-shaped variant this crate doesn't
/// yet support, unlike e.g. [`Sign`], which is mathematically complete
/// forever.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoronoiEdgeKind {
    /// A finite segment between two Voronoi vertices.
    Bounded {
        /// The segment's two endpoints.
        vertices: [VoronoiVertexId; 2],
    },
    /// An infinite ray from `finite_vertex` out to infinity, dual to a
    /// convex-hull edge of the Delaunay triangulation.
    Unbounded {
        /// The ray's single finite endpoint.
        finite_vertex: VoronoiVertexId,
    },
}

/// One edge of a Voronoi diagram: the two cells it separates, its shape
/// (bounded segment or unbounded ray), and the Delaunay edge it is dual
/// to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiEdge {
    /// The two cells (sites) this edge separates.
    pub cells: [VoronoiCellId; 2],
    /// The dual Delaunay edge, kept as provenance/debugging metadata --
    /// not needed to interpret `cells`/`kind`, which are self-sufficient.
    pub source_edge: EdgeId,
    /// Whether this edge is a finite segment or an infinite ray.
    pub kind: VoronoiEdgeKind,
}

/// A 2D Voronoi diagram's topology: the dual of a [`Triangulation2`],
/// owned by value (no lifetime parameter), matching
/// [`super::ConstrainedTriangulation2`]'s precedent.
///
/// Carries no coordinates for its own vertices -- only which cells,
/// vertices, and edges exist and how they connect (query API below).
/// Circumcenter computation and clipping are a later phase (ADR-007
/// Phase 7C).
#[derive(Debug, Clone, PartialEq)]
pub struct Voronoi2 {
    delaunay: Triangulation2,
    /// Dense, indexed by `FaceId::raw()`: which Voronoi vertex (face
    /// group) each Delaunay face belongs to.
    face_group: Vec<VoronoiVertexId>,
    /// Dense, indexed by `VoronoiVertexId`'s inner `u32`: the inverse of
    /// `face_group`, each group's member faces (sorted by `FaceId::raw()`
    /// for this instance's own internal determinism).
    group_faces: Vec<Vec<FaceId>>,
    /// Dense, indexed by `VoronoiEdgeId`'s inner `u32`, in canonical
    /// (cell-pair-key-sorted) order.
    edges: Vec<VoronoiEdge>,
}

/// Union-find (disjoint-set) over Delaunay `FaceId`s, path-compressed
/// only -- no union-by-rank. Face counts here are small enough that this
/// is not a bottleneck, and performance tuning is explicitly out of scope
/// for this phase (ADR-007 Phase 7A).
struct UnionFind {
    parent: Vec<u32>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind {
            parent: (0..n as u32).collect(),
        }
    }

    fn find(&mut self, x: u32) -> u32 {
        if self.parent[x as usize] != x {
            let root = self.find(self.parent[x as usize]);
            self.parent[x as usize] = root;
        }
        self.parent[x as usize]
    }

    fn union(&mut self, a: u32, b: u32) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra != rb {
            self.parent[ra as usize] = rb;
        }
    }
}

/// Every distinct `VertexId` among `faces`' vertices, sorted and
/// deduplicated -- the canonical key a face group is sorted and assigned
/// a dense [`VoronoiVertexId`] by (ADR-007 "Canonical dense ID
/// assignment").
fn group_key(delaunay: &Triangulation2, faces: &[FaceId]) -> Vec<u32> {
    let mut verts: Vec<u32> = faces
        .iter()
        .flat_map(|&f| delaunay.face_vertices(f))
        .map(VertexId::raw)
        .collect();
    verts.sort_unstable();
    verts.dedup();
    verts
}

/// The sorted pair of `VertexId::raw()` a [`VoronoiEdge`]'s `cells`
/// resolve to -- the canonical key a Voronoi edge is sorted and assigned
/// a dense [`VoronoiEdgeId`] by.
fn edge_key(cells: [VoronoiCellId; 2]) -> [u32; 2] {
    let mut k = [cells[0].0.raw(), cells[1].0.raw()];
    k.sort_unstable();
    k
}

/// A triangle's third vertex, given the other two (an edge it shares with
/// a neighboring face). Panics only if `face` doesn't actually have `u`
/// and `v` among its vertices -- which cannot happen for a `face`/`edge`
/// pair drawn from `delaunay.adjacent_faces(edge)`, an invariant
/// `Triangulation2` itself is responsible for and already validated by
/// (`TopologyError::AdjacencyMismatch`).
fn third_vertex(delaunay: &Triangulation2, face: FaceId, u: VertexId, v: VertexId) -> VertexId {
    delaunay
        .face_vertices(face)
        .into_iter()
        .find(|&w| w != u && w != v)
        .expect("a triangle's vertices must include a 3rd vertex besides its shared edge's two")
}

/// Builds the [`Voronoi2`] topology dual to `delaunay`.
///
/// Adjacent Delaunay faces whose shared edge is cocircular with both
/// opposite vertices (`incircle(..) == Sign::Zero`) are merged via
/// union-find into one Voronoi vertex, so an arbitrary Delaunay
/// tie-break among cocircular sites (`delaunay2`'s own documented
/// behavior) never leaks into the Voronoi topology as spurious extra
/// vertices/edges. Every dense ID (`VoronoiVertexId`, `VoronoiEdgeId`) is
/// assigned by sorting on a canonical key derived from site identity, not
/// from union-find root values or face-scan order, so two differently
/// triangulated-but-topologically-equal inputs produce identical (not
/// merely isomorphic) output. Always succeeds -- no fallible variant is
/// needed, since every input, including degenerate ones, has a
/// well-defined (possibly empty) dual.
///
/// # Examples
///
/// ```
/// use kika::{Point2, VoronoiEdgeKind, delaunay2, voronoi2};
///
/// let pts = [
///     Point2::new(0.0, 0.0).unwrap(),
///     Point2::new(4.0, 0.0).unwrap(),
///     Point2::new(0.0, 4.0).unwrap(),
/// ];
/// let voronoi = voronoi2(delaunay2(&pts));
///
/// // One cell per site, one Voronoi vertex (the triangle's circumcenter),
/// // and 3 unbounded rays -- no interior Delaunay edge to exclude.
/// assert_eq!(voronoi.cells().count(), 3);
/// assert_eq!(voronoi.vertices().count(), 1);
/// for edge in voronoi.edges() {
///     assert!(matches!(
///         voronoi.edge_kind(edge),
///         VoronoiEdgeKind::Unbounded { .. }
///     ));
/// }
/// ```
pub fn voronoi2(delaunay: Triangulation2) -> Voronoi2 {
    let n_faces = delaunay.faces().count();
    let mut uf = UnionFind::new(n_faces);
    let coord: Vec<_> = delaunay.vertices().map(|(_, p)| p).collect();

    for edge in delaunay.edges() {
        if let [Some(f1), Some(f2)] = delaunay.adjacent_faces(edge) {
            let (u, v) = delaunay.edge_vertices(edge);
            let a = third_vertex(&delaunay, f1, u, v);
            let b = third_vertex(&delaunay, f2, u, v);
            let cocircular = incircle(
                coord[u.raw() as usize],
                coord[v.raw() as usize],
                coord[a.raw() as usize],
                coord[b.raw() as usize],
            ) == Sign::Zero;
            if cocircular {
                uf.union(f1.raw(), f2.raw());
            }
        }
    }

    let mut by_root: std::collections::HashMap<u32, Vec<FaceId>> = std::collections::HashMap::new();
    for f in delaunay.faces() {
        by_root.entry(uf.find(f.raw())).or_default().push(f);
    }

    // Canonical order: sort groups by their site-identity key. Iteration
    // order over `by_root` (a HashMap) never leaks into the result --
    // only the *content* of each group's key determines its final
    // position.
    let mut keyed_groups: Vec<(Vec<u32>, Vec<FaceId>)> = by_root
        .into_values()
        .map(|mut faces| {
            faces.sort_unstable_by_key(|f| f.raw());
            (group_key(&delaunay, &faces), faces)
        })
        .collect();
    keyed_groups.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut face_group = vec![VoronoiVertexId(0); n_faces];
    let mut group_faces = Vec::with_capacity(keyed_groups.len());
    for (i, (_, faces)) in keyed_groups.into_iter().enumerate() {
        for &f in &faces {
            face_group[f.raw() as usize] = VoronoiVertexId(i as u32);
        }
        group_faces.push(faces);
    }

    let mut raw_edges: Vec<VoronoiEdge> = Vec::new();
    for edge in delaunay.edges() {
        let (u, v) = delaunay.edge_vertices(edge);
        let cells = [VoronoiCellId(u), VoronoiCellId(v)];
        match delaunay.adjacent_faces(edge) {
            [Some(f), None] | [None, Some(f)] => {
                raw_edges.push(VoronoiEdge {
                    cells,
                    source_edge: edge,
                    kind: VoronoiEdgeKind::Unbounded {
                        finite_vertex: face_group[f.raw() as usize],
                    },
                });
            }
            [Some(f1), Some(f2)] => {
                let g1 = face_group[f1.raw() as usize];
                let g2 = face_group[f2.raw() as usize];
                if g1 != g2 {
                    raw_edges.push(VoronoiEdge {
                        cells,
                        source_edge: edge,
                        kind: VoronoiEdgeKind::Bounded { vertices: [g1, g2] },
                    });
                }
                // else: both sides in the same cocircular group -- a
                // spurious artifact of the Delaunay tie-break, excluded.
            }
            [None, None] => unreachable!("a Triangulation2 edge always has at least one face"),
        }
    }

    raw_edges.sort_unstable_by_key(|e| edge_key(e.cells));

    Voronoi2 {
        delaunay,
        face_group,
        group_faces,
        edges: raw_edges,
    }
}

impl Voronoi2 {
    /// Every cell -- one per Delaunay site, a bijection with
    /// [`Triangulation2::vertices`].
    pub fn cells(&self) -> impl Iterator<Item = VoronoiCellId> + '_ {
        self.delaunay.vertices().map(|(v, _)| VoronoiCellId(v))
    }

    /// Every Voronoi vertex (one per cocircular-merged Delaunay face
    /// group).
    pub fn vertices(&self) -> impl Iterator<Item = VoronoiVertexId> + '_ {
        (0..self.group_faces.len()).map(|i| VoronoiVertexId(i as u32))
    }

    /// Every Voronoi edge.
    pub fn edges(&self) -> impl Iterator<Item = VoronoiEdgeId> + '_ {
        (0..self.edges.len()).map(|i| VoronoiEdgeId(i as u32))
    }

    /// The Delaunay site `cell` surrounds.
    pub fn cell_site(&self, cell: VoronoiCellId) -> VertexId {
        cell.0
    }

    /// Every cell adjacent to `cell` across a Voronoi edge -- the other
    /// endpoint of every edge incident to `cell` (an excluded,
    /// same-group Delaunay edge never contributes one, since it never
    /// became a `VoronoiEdge` at all).
    ///
    /// # Examples
    ///
    /// ```
    /// use kika::{Point2, delaunay2, voronoi2};
    ///
    /// let pts = [
    ///     Point2::new(0.0, 0.0).unwrap(),
    ///     Point2::new(4.0, 0.0).unwrap(),
    ///     Point2::new(4.0, 4.0).unwrap(),
    ///     Point2::new(0.0, 4.0).unwrap(),
    /// ];
    /// let voronoi = voronoi2(delaunay2(&pts));
    /// let cell = voronoi.cells().next().unwrap();
    ///
    /// // A square's corners are all hull vertices -- every cell is unbounded.
    /// assert!(voronoi.cell_is_unbounded(cell));
    /// // Each corner is Voronoi-adjacent to the 2 corners sharing a square
    /// // edge, not to the diagonally-opposite corner (that Delaunay edge
    /// // is cocircular with this one and was excluded).
    /// assert_eq!(voronoi.neighboring_cells(cell).count(), 2);
    /// ```
    pub fn neighboring_cells(
        &self,
        cell: VoronoiCellId,
    ) -> impl Iterator<Item = VoronoiCellId> + '_ {
        self.edges.iter().filter_map(move |e| {
            if e.cells[0] == cell {
                Some(e.cells[1])
            } else if e.cells[1] == cell {
                Some(e.cells[0])
            } else {
                None
            }
        })
    }

    /// `true` iff `cell` has at least one `Unbounded` edge -- equivalently,
    /// iff its site lies on the Delaunay triangulation's convex hull. A
    /// hull site always has a boundary Delaunay edge, and a boundary edge
    /// (only ever 1 incident face) can never be excluded as same-group,
    /// so this can never disagree with the hull-membership definition.
    pub fn cell_is_unbounded(&self, cell: VoronoiCellId) -> bool {
        self.edges.iter().any(|e| {
            (e.cells[0] == cell || e.cells[1] == cell)
                && matches!(e.kind, VoronoiEdgeKind::Unbounded { .. })
        })
    }

    /// `edge`'s two cells.
    pub fn edge_cells(&self, edge: VoronoiEdgeId) -> [VoronoiCellId; 2] {
        self.edges[edge.0 as usize].cells
    }

    /// `edge`'s shape: a finite segment between two Voronoi vertices, or
    /// an infinite ray from one.
    pub fn edge_kind(&self, edge: VoronoiEdgeId) -> &VoronoiEdgeKind {
        &self.edges[edge.0 as usize].kind
    }

    /// The Delaunay edge `edge` is dual to -- provenance/debugging
    /// metadata, not needed to interpret `edge`'s cells or kind.
    pub fn dual_delaunay_edge(&self, edge: VoronoiEdgeId) -> EdgeId {
        self.edges[edge.0 as usize].source_edge
    }

    /// Every Delaunay face merged into `vertex`'s group -- one face for
    /// an ordinary Voronoi vertex, more than one where cocircular faces
    /// were merged (ADR-007).
    pub fn vertex_delaunay_faces(&self, vertex: VoronoiVertexId) -> &[FaceId] {
        &self.group_faces[vertex.0 as usize]
    }

    /// `cell`'s boundary edges in counterclockwise order: a closed cycle
    /// for a bounded cell (interior site), or a linear sequence from one
    /// `Unbounded` ray to the other for an unbounded cell (hull site).
    ///
    /// Derived entirely from `Triangulation2::face_vertices`/
    /// `neighboring_faces` (the existing Delaunay face-adjacency
    /// structure, no new data model) plus the already-computed `edges`:
    /// walking the faces incident to `cell`'s site in a fixed rotational
    /// direction visits the same faces the geometric Voronoi cell
    /// boundary would, in order, and a step whose Delaunay edge was
    /// excluded as a same-cocircular-group artifact (§`voronoi2`) simply
    /// has no entry in `edges` to find -- which is exactly the signal to
    /// skip it and continue the walk across the merged group, no special
    /// casing needed. Cocircular merging cannot split a cell's walk into
    /// disconnected pieces of the same `VoronoiVertexId`: if two faces
    /// incident to `cell`'s site land in the same group, every face
    /// between them in the local rotation must too (their shared vertex
    /// plus both circles' defining triples force one common circle,
    /// which `voronoi2`'s exhaustive adjacent-pair testing always
    /// catches) -- so no `VoronoiVertexId` can appear from two separate,
    /// non-adjacent stretches of one cell's boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// use kika::{Point2, delaunay2, voronoi2};
    ///
    /// let pts = [
    ///     Point2::new(0.0, 0.0).unwrap(),
    ///     Point2::new(4.0, 0.0).unwrap(),
    ///     Point2::new(0.0, 4.0).unwrap(),
    /// ];
    /// let voronoi = voronoi2(delaunay2(&pts));
    /// let cell = voronoi.cells().next().unwrap();
    ///
    /// // A single triangle's cell has exactly 2 unbounded rays -- no
    /// // interior edge to complete a cycle.
    /// assert_eq!(voronoi.cell_edges(cell).count(), 2);
    /// ```
    pub fn cell_edges(&self, cell: VoronoiCellId) -> impl Iterator<Item = VoronoiEdgeId> + '_ {
        let s = cell.0;
        let mut result = Vec::new();

        if let Some(f0) = self
            .delaunay
            .faces()
            .find(|&f| self.delaunay.face_vertices(f).contains(&s))
        {
            let i0 = self.local_index(f0, s);

            // Rotate backward from f0: either it returns to f0 itself
            // (s is fully surrounded -- a closed cycle), or it hits the
            // hull boundary (None), landing on the fan's true start.
            // Verified by hand for a degree-3 interior umbrella and a
            // 2-triangle hull fan (a square's corner) before writing
            // this -- the cycle case must restart the forward walk from
            // the *original* f0/i0, not wherever this loop's `face` ends
            // up, since the loop only exists to classify hull vs.
            // interior, not to locate a meaningful position on a cycle.
            // (An earlier draft used the loop's ending position for both
            // cases and silently dropped one step per cell on every
            // interior vertex -- caught by hand-tracing before it ever
            // reached a test.)
            let mut face = f0;
            let mut i = i0;
            let mut is_cycle = false;
            loop {
                let (_, next) = self.backward_step(face, i);
                match next {
                    None => break,
                    Some(nf) if nf == f0 => {
                        is_cycle = true;
                        break;
                    }
                    Some(nf) => {
                        face = nf;
                        i = self.local_index(nf, s);
                    }
                }
            }
            let (start_face, start_i) = if is_cycle { (f0, i0) } else { (face, i) };

            let mut face = start_face;
            let mut i = start_i;

            if !is_cycle {
                let (entry_other, _) = self.backward_step(face, i);
                if let Some(id) = self.edge_id_between(s, entry_other) {
                    result.push(id);
                }
            }

            loop {
                let (other, next) = self.forward_step(face, i);
                if let Some(id) = self.edge_id_between(s, other) {
                    result.push(id);
                }
                match next {
                    Some(nf) if is_cycle && nf == start_face => break,
                    Some(nf) => {
                        face = nf;
                        i = self.local_index(nf, s);
                    }
                    None => break,
                }
            }
        }

        result.into_iter()
    }

    /// `v`'s index within `face`'s 3 vertices. Panics only if `face`
    /// isn't actually incident to `v` -- never true for any call site in
    /// this file, which only ever passes a `face` already known (by
    /// construction) to contain `v`.
    fn local_index(&self, face: FaceId, v: VertexId) -> usize {
        self.delaunay
            .face_vertices(face)
            .iter()
            .position(|&w| w == v)
            .expect("`face` must be incident to `v`")
    }

    /// One rotation step around `face_vertices(face)[i]`, crossing the
    /// edge to the *previous* CCW vertex -- verified against a concrete
    /// square (corner site, diagonal split into 2 CCW triangles) to
    /// trace true counterclockwise order, not assumed from the index
    /// arithmetic alone. Returns the edge's other endpoint and the
    /// neighboring face across it (`None` at the hull boundary).
    fn forward_step(&self, face: FaceId, i: usize) -> (VertexId, Option<FaceId>) {
        let other = self.delaunay.face_vertices(face)[(i + 2) % 3];
        let next = self.delaunay.neighboring_faces(face)[(i + 1) % 3];
        (other, next)
    }

    /// The reverse of [`Self::forward_step`]: crosses the edge to the
    /// *next* CCW vertex.
    fn backward_step(&self, face: FaceId, i: usize) -> (VertexId, Option<FaceId>) {
        let other = self.delaunay.face_vertices(face)[(i + 1) % 3];
        let next = self.delaunay.neighboring_faces(face)[(i + 2) % 3];
        (other, next)
    }

    /// The `VoronoiEdgeId` of the surviving edge between sites `a` and
    /// `b`, if any -- `None` means the Delaunay edge between them was
    /// excluded as a same-cocircular-group artifact (`voronoi2`), not
    /// that no such Delaunay edge exists.
    fn edge_id_between(&self, a: VertexId, b: VertexId) -> Option<VoronoiEdgeId> {
        let target = [VoronoiCellId(a), VoronoiCellId(b)];
        self.edges
            .iter()
            .position(|e| e.cells == target || e.cells == [target[1], target[0]])
            .map(|idx| VoronoiEdgeId(idx as u32))
    }
}

/// A structural invariant violated in a [`Voronoi2`], found by
/// `validate_voronoi_topology`. Mirrors
/// [`super::delaunay2::TopologyError`]'s role for `Triangulation2`:
/// `#[doc(hidden)]` `pub` so this crate's own `tests/`/`fuzz/` can reach
/// it without being a real, advertised public API commitment yet.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum VoronoiTopologyError {
    /// `face_group[face]` and `group_faces` don't agree with each other
    /// as inverses.
    FaceGroupNotInverse {
        /// The face whose group membership is inconsistent.
        face: FaceId,
    },
    /// `group_faces` (the `VoronoiVertexId` space) is not sorted by its
    /// canonical site-identity key at this position.
    GroupsNotCanonicallyOrdered {
        /// The index at which canonical order first breaks.
        at: usize,
    },
    /// `edges` (the `VoronoiEdgeId` space) is not sorted by its canonical
    /// cell-pair key at this position.
    EdgesNotCanonicallyOrdered {
        /// The index at which canonical order first breaks.
        at: usize,
    },
    /// A Delaunay edge that should contribute a Voronoi edge (a hull
    /// edge, or an interior edge between two different groups) has no
    /// corresponding entry in `edges`.
    MissingVoronoiEdge {
        /// The Delaunay edge with no dual entry.
        source_edge: EdgeId,
    },
    /// An entry in `edges` doesn't match what its `source_edge` and the
    /// current `face_group` table independently say it should be.
    EdgeMismatch {
        /// The Delaunay edge whose dual entry is wrong.
        source_edge: EdgeId,
    },
    /// A Voronoi edge's two `cells` are the same cell, not two distinct
    /// sites.
    NonDistinctEdgeCells {
        /// The offending edge.
        edge: VoronoiEdgeId,
    },
    /// A `Bounded` edge's two endpoint vertices are the same Voronoi
    /// vertex, not two distinct groups.
    NonDistinctBoundedVertices {
        /// The offending edge.
        edge: VoronoiEdgeId,
    },
    /// An `Unbounded` edge's dual Delaunay edge is not actually a
    /// convex-hull edge (recomputed from `delaunay.adjacent_faces`
    /// independently of `edges`' own cached `kind`).
    UnboundedEdgeNotHullDual {
        /// The offending edge.
        edge: VoronoiEdgeId,
    },
    /// The same Delaunay face appears more than once in one Voronoi
    /// vertex's face group.
    DuplicateFaceInGroup {
        /// The group with a duplicate entry.
        vertex: VoronoiVertexId,
        /// The duplicated face.
        face: FaceId,
    },
    /// A Voronoi edge doesn't appear in exactly 2 cells' `cell_edges()`
    /// sequences. This is a coverage check, not an ordering one: since
    /// `cell_edges` finds an edge via `edge_id_between(s, other)` -- a
    /// lookup keyed on `edges`' own `cells` pair -- any edge with
    /// `cells == [a, b]` is *found* by construction when walking `a` and
    /// when walking `b`, so a healthy walk always yields exactly 2 by
    /// that lookup alone. What this actually catches is a walk that
    /// *drops* a step it should have visited (leaving the edge under 2,
    /// possibly 0) -- it cannot detect two edges emitted in the wrong
    /// relative order, or a duplicate emission alongside a correct one.
    /// Order is verified by test (`cell_edges_interior_cell_is_...` and
    /// its hull-cell counterpart), not by this check.
    EdgeNotInExactlyTwoCellBoundaries {
        /// The offending edge.
        edge: VoronoiEdgeId,
        /// How many cells' boundaries it actually appeared in.
        count: usize,
    },
}

impl Voronoi2 {
    /// Checks every structural invariant `voronoi2` is supposed to
    /// establish: `face_group`/`group_faces` are mutual inverses with no
    /// duplicate face in any one group, both `group_faces` and `edges`
    /// are canonically (not incidentally) ordered, `edges` matches what
    /// independently recomputing each Delaunay edge's classification
    /// against `face_group` says it should be (which also covers "no
    /// same-component edge is ever exposed" -- a same-group interior
    /// edge's independently recomputed classification is `None`, so any
    /// entry for it at all is an `EdgeMismatch`), every edge's two cells
    /// are distinct, every `Bounded` edge's two vertices are distinct,
    /// and every `Unbounded` edge's dual is actually a hull edge.
    ///
    /// Two invariants ADR-007 also names have no corresponding check
    /// here. "Every site has exactly one cell" needs none:
    /// `VoronoiCellId` is a direct `VertexId` wrapper with no separate
    /// cell table to desync in the first place. "The neighboring-cells
    /// relation is symmetric" is asserted by a test instead (see
    /// `neighboring_and_unbounded_queries_on_the_mixed_fixture`), not a
    /// validator check: `neighboring_cells` derives its answer fresh from
    /// `edges`' own unordered `[cell_a, cell_b]` pairs on every call, so
    /// it reads symmetrically regardless of what `edges` contains -- the
    /// data shape admits no asymmetric entry to inject, the way the 4
    /// checks above can each be tested by directly corrupting a
    /// constructed `Voronoi2`'s private fields.
    ///
    /// Returns every violation found, not just the first.
    #[doc(hidden)]
    pub fn validate_voronoi_topology(&self) -> Vec<VoronoiTopologyError> {
        let mut errors = Vec::new();

        for (i, faces) in self.group_faces.iter().enumerate() {
            for &f in faces {
                if self.face_group[f.raw() as usize] != VoronoiVertexId(i as u32) {
                    errors.push(VoronoiTopologyError::FaceGroupNotInverse { face: f });
                }
            }
        }
        for f in self.delaunay.faces() {
            let g = self.face_group[f.raw() as usize];
            if !self.group_faces[g.0 as usize].contains(&f) {
                errors.push(VoronoiTopologyError::FaceGroupNotInverse { face: f });
            }
        }

        for (i, w) in self.group_faces.windows(2).enumerate() {
            let ka = group_key(&self.delaunay, &w[0]);
            let kb = group_key(&self.delaunay, &w[1]);
            if ka >= kb {
                errors.push(VoronoiTopologyError::GroupsNotCanonicallyOrdered { at: i + 1 });
            }
        }
        for (i, w) in self.edges.windows(2).enumerate() {
            if edge_key(w[0].cells) >= edge_key(w[1].cells) {
                errors.push(VoronoiTopologyError::EdgesNotCanonicallyOrdered { at: i + 1 });
            }
        }

        for source_edge in self.delaunay.edges() {
            let (u, v) = self.delaunay.edge_vertices(source_edge);
            let expected_cells = [VoronoiCellId(u), VoronoiCellId(v)];
            let expected_kind = match self.delaunay.adjacent_faces(source_edge) {
                [Some(f), None] | [None, Some(f)] => Some(VoronoiEdgeKind::Unbounded {
                    finite_vertex: self.face_group[f.raw() as usize],
                }),
                [Some(f1), Some(f2)] => {
                    let g1 = self.face_group[f1.raw() as usize];
                    let g2 = self.face_group[f2.raw() as usize];
                    if g1 != g2 {
                        Some(VoronoiEdgeKind::Bounded { vertices: [g1, g2] })
                    } else {
                        None
                    }
                }
                [None, None] => None,
            };

            let actual = self.edges.iter().find(|e| e.source_edge == source_edge);
            match (expected_kind, actual) {
                (None, Some(_)) => errors.push(VoronoiTopologyError::EdgeMismatch { source_edge }),
                (Some(_), None) => {
                    errors.push(VoronoiTopologyError::MissingVoronoiEdge { source_edge })
                }
                (Some(expected), Some(actual)) => {
                    if actual.cells != expected_cells || actual.kind != expected {
                        errors.push(VoronoiTopologyError::EdgeMismatch { source_edge });
                    }
                }
                (None, None) => {}
            }
        }

        for (i, faces) in self.group_faces.iter().enumerate() {
            let mut seen = std::collections::HashSet::new();
            for &f in faces {
                if !seen.insert(f) {
                    errors.push(VoronoiTopologyError::DuplicateFaceInGroup {
                        vertex: VoronoiVertexId(i as u32),
                        face: f,
                    });
                }
            }
        }

        for (i, e) in self.edges.iter().enumerate() {
            let edge = VoronoiEdgeId(i as u32);
            if e.cells[0] == e.cells[1] {
                errors.push(VoronoiTopologyError::NonDistinctEdgeCells { edge });
            }
            match e.kind {
                VoronoiEdgeKind::Bounded { vertices } => {
                    if vertices[0] == vertices[1] {
                        errors.push(VoronoiTopologyError::NonDistinctBoundedVertices { edge });
                    }
                }
                VoronoiEdgeKind::Unbounded { .. } => {
                    let is_hull_edge = matches!(
                        self.delaunay.adjacent_faces(e.source_edge),
                        [Some(_), None] | [None, Some(_)]
                    );
                    if !is_hull_edge {
                        errors.push(VoronoiTopologyError::UnboundedEdgeNotHullDual { edge });
                    }
                }
            }
        }

        // cell_edges()'s own coverage: every edge must be reachable from
        // exactly 2 cells' walks, catching a dropped step (see
        // EdgeNotInExactlyTwoCellBoundaries's doc comment for why this
        // is coverage, not ordering, verification). O(cells x average
        // degree), the first check here costing more than near-linear
        // -- fine for a validator, not something to call from a hot
        // path.
        let mut boundary_counts = vec![0usize; self.edges.len()];
        for cell in self.cells() {
            for edge in self.cell_edges(cell) {
                boundary_counts[edge.0 as usize] += 1;
            }
        }
        for (i, &count) in boundary_counts.iter().enumerate() {
            if count != 2 {
                errors.push(VoronoiTopologyError::EdgeNotInExactlyTwoCellBoundaries {
                    edge: VoronoiEdgeId(i as u32),
                    count,
                });
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::super::delaunay2::{assemble_triangulation, delaunay2};
    use super::*;
    use crate::primitives::Point2;

    fn pt(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn single_triangle_has_one_group_and_three_unbounded_edges() {
        let pts = vec![pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0)];
        let v = voronoi2(delaunay2(&pts));

        assert_eq!(v.group_faces.len(), 1);
        assert_eq!(v.edges.len(), 3);
        assert!(
            v.edges
                .iter()
                .all(|e| matches!(e.kind, VoronoiEdgeKind::Unbounded { .. }))
        );
        assert!(v.validate_voronoi_topology().is_empty());
    }

    #[test]
    fn axis_aligned_square_merges_both_faces_into_one_group() {
        // Exactly cocircular under exact incircle: any 3 corners' circle
        // passes exactly through the 4th, a pure algebraic identity for
        // a square -- not merely numerically close.
        let pts = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.0)];
        let v = voronoi2(delaunay2(&pts));

        assert_eq!(
            v.group_faces.len(),
            1,
            "the diagonal split by delaunay2's tie-break must not leak as 2 Voronoi vertices"
        );
        assert_eq!(
            v.edges.len(),
            4,
            "the 4 hull edges are Unbounded; the interior diagonal is same-group and excluded"
        );
        assert!(
            v.edges
                .iter()
                .all(|e| matches!(e.kind, VoronoiEdgeKind::Unbounded { .. }))
        );
        assert!(v.validate_voronoi_topology().is_empty());
    }

    #[test]
    fn generic_position_random_points_pass_validation() {
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

        let mut rng = Xorshift64(0x9e3779b97f4a7c15);
        let pts: Vec<Point2> = (0..60)
            .map(|_| pt(rng.next_f64_in(100.0), rng.next_f64_in(100.0)))
            .collect();
        let v = voronoi2(delaunay2(&pts));

        assert!(!v.group_faces.is_empty());
        assert_eq!(v.validate_voronoi_topology(), Vec::new());

        // cell_is_unbounded must agree with an independent recomputation
        // from the Delaunay hull, not just always be true -- every other
        // test in this file uses a fully-convex point set (every site on
        // the hull), so a 60-point generic-position cloud is the only
        // fixture that actually has interior sites to distinguish.
        let hull_sites: std::collections::HashSet<VertexId> = v
            .delaunay
            .boundary_edges()
            .flat_map(|e| {
                let (a, b) = v.delaunay.edge_vertices(e);
                [a, b]
            })
            .collect();
        let mut saw_interior_cell = false;
        for cell in v.cells() {
            let expected_unbounded = hull_sites.contains(&v.cell_site(cell));
            assert_eq!(v.cell_is_unbounded(cell), expected_unbounded);
            if !expected_unbounded {
                saw_interior_cell = true;
                assert!(
                    v.neighboring_cells(cell).count() >= 3,
                    "an interior Voronoi cell in generic position has at least 3 neighbors"
                );
            }
        }
        assert!(
            saw_interior_cell,
            "a 60-point generic-position cloud must have at least one interior site"
        );
    }

    // -- Canonical-topology normalization tests (ADR-007) --
    //
    // delaunay2() canonically sorts its input before insertion, so it is
    // a deterministic function of the input *set*, not order -- it can
    // never itself be coaxed into picking a different diagonal among
    // cocircular points. Different triangulations of the same cocircular
    // point set are instead built directly via assemble_triangulation,
    // using the exact same `pts` (and so the exact same VertexId
    // numbering) each time -- which lets the tests check that canonical
    // ID assignment makes differently-triangulated-but-topologically-
    // equal inputs produce *identical*, not merely isomorphic, output.

    /// 12 exact lattice points on the circle x^2+y^2=25, in CCW angular
    /// order -- small integers, so `incircle` is exactly (not just
    /// numerically) zero for any 4 of them.
    fn cocircular_lattice_points(n: usize) -> Vec<Point2> {
        const ALL: [(f64, f64); 12] = [
            (5.0, 0.0),
            (4.0, 3.0),
            (3.0, 4.0),
            (0.0, 5.0),
            (-3.0, 4.0),
            (-4.0, 3.0),
            (-5.0, 0.0),
            (-4.0, -3.0),
            (-3.0, -4.0),
            (0.0, -5.0),
            (3.0, -4.0),
            (4.0, -3.0),
        ];
        assert!(n <= ALL.len());
        ALL[..n].iter().map(|&(x, y)| pt(x, y)).collect()
    }

    /// Fans `n` CCW-ordered convex points (indices `0..n`) out from
    /// vertex `start`: triangles `(start, start+i, start+i+1)` for
    /// `i = 1..=n-2`, indices mod `n`. A different `start` picks a
    /// different, but equally valid, triangulation of the same point set.
    fn fan_from(n: usize, start: usize) -> Vec<[VertexId; 3]> {
        (1..n - 1)
            .map(|i| {
                let a = start % n;
                let b = (start + i) % n;
                let c = (start + i + 1) % n;
                [VertexId(a as u32), VertexId(b as u32), VertexId(c as u32)]
            })
            .collect()
    }

    /// A face group's canonical key (sorted-dedup site `VertexId`s),
    /// recomputed independently of any incidental group/scan order.
    fn group_site_set(v: &Voronoi2, g: VoronoiVertexId) -> Vec<u32> {
        group_key(&v.delaunay, &v.group_faces[g.0 as usize])
    }

    /// Every group's canonical key, in `VoronoiVertexId` order.
    fn canonical_groups(v: &Voronoi2) -> Vec<Vec<u32>> {
        v.group_faces
            .iter()
            .map(|faces| group_key(&v.delaunay, faces))
            .collect()
    }

    /// Every edge's canonical representation: the sorted site pair it
    /// separates, and its endpoint(s) translated from (per-instance)
    /// `VoronoiVertexId` to (instance-independent) site sets -- exactly
    /// the "cell-as-site, vertex-as-sorted-incident-site-set" comparison
    /// ADR-007 specifies, dropping the incidental `source_edge`/raw-id
    /// fields entirely.
    fn canonical_edges(v: &Voronoi2) -> Vec<(Vec<u32>, Vec<Vec<u32>>)> {
        let mut out: Vec<_> = v
            .edges
            .iter()
            .map(|e| {
                let cell_key = edge_key(e.cells).to_vec();
                let mut endpoints: Vec<Vec<u32>> = match e.kind {
                    VoronoiEdgeKind::Bounded { vertices } => {
                        vertices.iter().map(|&g| group_site_set(v, g)).collect()
                    }
                    VoronoiEdgeKind::Unbounded { finite_vertex } => {
                        vec![group_site_set(v, finite_vertex)]
                    }
                };
                endpoints.sort();
                (cell_key, endpoints)
            })
            .collect();
        out.sort();
        out
    }

    /// Asserts a fully cocircular convex `n`-gon collapses to exactly one
    /// Voronoi vertex with `n` Unbounded hull edges and no interior
    /// (Bounded) edges -- true regardless of which triangulation of it
    /// was fed in, since every pair of adjacent faces in any
    /// triangulation of mutually cocircular points tests cocircular.
    fn assert_single_cocircular_group(v: &Voronoi2, n_sites: usize) {
        assert_eq!(v.group_faces.len(), 1);
        assert_eq!(v.edges.len(), n_sites);
        assert!(
            v.edges
                .iter()
                .all(|e| matches!(e.kind, VoronoiEdgeKind::Unbounded { .. }))
        );
        assert!(v.validate_voronoi_topology().is_empty());
    }

    #[test]
    fn square_both_diagonals_produce_identical_canonical_topology() {
        let pts = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.0)];

        let diagonal_a = voronoi2(assemble_triangulation(pts.clone(), fan_from(4, 0)));
        let diagonal_b = voronoi2(assemble_triangulation(pts, fan_from(4, 1)));

        assert_single_cocircular_group(&diagonal_a, 4);
        assert_single_cocircular_group(&diagonal_b, 4);
        assert_eq!(canonical_groups(&diagonal_a), canonical_groups(&diagonal_b));
        assert_eq!(canonical_edges(&diagonal_a), canonical_edges(&diagonal_b));
    }

    #[test]
    fn five_cocircular_points_multiple_fans_produce_identical_canonical_topology() {
        let pts = cocircular_lattice_points(5);

        let fan_0 = voronoi2(assemble_triangulation(pts.clone(), fan_from(5, 0)));
        let fan_2 = voronoi2(assemble_triangulation(pts, fan_from(5, 2)));

        assert_single_cocircular_group(&fan_0, 5);
        assert_single_cocircular_group(&fan_2, 5);
        assert_eq!(canonical_groups(&fan_0), canonical_groups(&fan_2));
        assert_eq!(canonical_edges(&fan_0), canonical_edges(&fan_2));
    }

    #[test]
    fn eight_cocircular_points_multiple_fans_produce_identical_canonical_topology() {
        let pts = cocircular_lattice_points(8);

        let fan_0 = voronoi2(assemble_triangulation(pts.clone(), fan_from(8, 0)));
        let fan_3 = voronoi2(assemble_triangulation(pts.clone(), fan_from(8, 3)));
        let fan_6 = voronoi2(assemble_triangulation(pts, fan_from(8, 6)));

        for v in [&fan_0, &fan_3, &fan_6] {
            assert_single_cocircular_group(v, 8);
        }
        assert_eq!(canonical_groups(&fan_0), canonical_groups(&fan_3));
        assert_eq!(canonical_groups(&fan_0), canonical_groups(&fan_6));
        assert_eq!(canonical_edges(&fan_0), canonical_edges(&fan_3));
        assert_eq!(canonical_edges(&fan_0), canonical_edges(&fan_6));
    }

    #[test]
    fn cocircular_cluster_plus_outlier_mixes_bounded_and_excluded_edges() {
        // A cocircular square (p0,p1,p2,p3), split by diagonal p0-p2,
        // plus a far-outside point p4 pulled in as an "ear" against edge
        // p1-p2. Unlike every other test in this file, exclusion here is
        // *partial*: p0-p2 (shared by the two square faces) is cocircular
        // and excluded, but p1-p2 (shared by a square face and the ear)
        // is not -- p4 is nowhere near the square's circumcircle -- so it
        // must survive as a genuine Bounded edge. A construction that
        // over-merges (e.g. unions any adjacent pair regardless of the
        // incircle test) or under-excludes (keeps p0-p2) would pass every
        // other test in this file but fail this one.
        let p0 = pt(0.0, 0.0);
        let p1 = pt(2.0, 0.0);
        let p2 = pt(2.0, 2.0);
        let p3 = pt(0.0, 2.0);
        let p4 = pt(6.0, 1.0);
        let pts = vec![p0, p1, p2, p3, p4];
        let faces = vec![
            [VertexId(1), VertexId(4), VertexId(2)], // ear: (p1, p4, p2)
            [VertexId(0), VertexId(1), VertexId(2)], // square half sharing p1-p2 and p0-p2
            [VertexId(0), VertexId(2), VertexId(3)], // square half sharing p0-p2
        ];
        let v = voronoi2(assemble_triangulation(pts, faces));

        assert_eq!(
            v.group_faces.len(),
            2,
            "the square's 2 faces merge (cocircular); the ear stays its own group"
        );
        assert_eq!(
            v.edges.len(),
            6,
            "7 Delaunay edges (5 hull + 2 interior) minus 1 excluded same-group diagonal"
        );

        let bounded: Vec<_> = v
            .edges
            .iter()
            .filter(|e| matches!(e.kind, VoronoiEdgeKind::Bounded { .. }))
            .collect();
        assert_eq!(
            bounded.len(),
            1,
            "only p1-p2 (square face vs. ear) is a genuine, non-cocircular interior edge"
        );
        match bounded[0].kind {
            VoronoiEdgeKind::Bounded { vertices } => {
                assert_ne!(
                    vertices[0], vertices[1],
                    "a Bounded edge must separate 2 distinct groups"
                );
            }
            VoronoiEdgeKind::Unbounded { .. } => unreachable!(),
        }
        assert_eq!(
            v.edges.len() - bounded.len(),
            5,
            "the remaining 5 edges are the pentagon's hull, all Unbounded"
        );
        assert!(v.validate_voronoi_topology().is_empty());
    }

    // -- Query API (Phase 7B) --

    #[test]
    fn query_api_matches_underlying_edge_and_group_data() {
        let pts = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.0)];
        let v = voronoi2(delaunay2(&pts));

        assert_eq!(v.cells().count(), v.delaunay.vertices().count());
        assert_eq!(v.vertices().count(), v.group_faces.len());
        assert_eq!(v.edges().count(), v.edges.len());

        for (i, id) in v.edges().enumerate() {
            let raw = &v.edges[i];
            assert_eq!(v.edge_cells(id), raw.cells);
            assert_eq!(*v.edge_kind(id), raw.kind);
            assert_eq!(v.dual_delaunay_edge(id), raw.source_edge);
        }

        for (i, id) in v.vertices().enumerate() {
            assert_eq!(v.vertex_delaunay_faces(id), v.group_faces[i].as_slice());
        }

        for cell in v.cells() {
            assert_eq!(cell.0, v.cell_site(cell));
        }
    }

    #[test]
    fn neighboring_and_unbounded_queries_on_the_mixed_fixture() {
        // Same construction as
        // cocircular_cluster_plus_outlier_mixes_bounded_and_excluded_edges:
        // a cocircular square (p0..p3) plus a far outlier p4 pulled in as
        // an ear against edge p1-p2.
        let pts = vec![
            pt(0.0, 0.0),
            pt(2.0, 0.0),
            pt(2.0, 2.0),
            pt(0.0, 2.0),
            pt(6.0, 1.0),
        ];
        let faces = vec![
            [VertexId(1), VertexId(4), VertexId(2)],
            [VertexId(0), VertexId(1), VertexId(2)],
            [VertexId(0), VertexId(2), VertexId(3)],
        ];
        let v = voronoi2(assemble_triangulation(pts, faces));
        let p0 = VoronoiCellId(VertexId(0));

        assert!(
            v.cell_is_unbounded(p0),
            "p0 is a hull vertex of the pentagon"
        );
        let neighbor_sites: Vec<VertexId> =
            v.neighboring_cells(p0).map(|c| v.cell_site(c)).collect();
        assert_eq!(
            neighbor_sites.len(),
            2,
            "p0-p2 is cocircular and excluded, so p2 must not appear as a neighbor"
        );
        assert!(neighbor_sites.contains(&VertexId(1)));
        assert!(neighbor_sites.contains(&VertexId(3)));

        // Symmetry: p0 is a neighbor of each of its own neighbors.
        for cell in v.cells() {
            for neighbor in v.neighboring_cells(cell).collect::<Vec<_>>() {
                assert!(v.neighboring_cells(neighbor).any(|c| c == cell));
            }
        }
    }

    // -- Validator extension (Phase 7B) --
    //
    // voronoi2() itself never produces any of these 4 violations -- these
    // tests deliberately corrupt a valid Voronoi2's private fields (only
    // possible from within this module) to prove each check's *logic* is
    // right, as a regression guard, not because the constructor is
    // currently suspected of being wrong.

    #[test]
    fn validator_catches_deliberately_corrupted_invariants() {
        let pts = vec![
            pt(0.0, 0.0),
            pt(2.0, 0.0),
            pt(2.0, 2.0),
            pt(0.0, 2.0),
            pt(6.0, 1.0),
        ];
        let faces = vec![
            [VertexId(1), VertexId(4), VertexId(2)],
            [VertexId(0), VertexId(1), VertexId(2)],
            [VertexId(0), VertexId(2), VertexId(3)],
        ];
        let valid = voronoi2(assemble_triangulation(pts, faces));
        assert!(valid.validate_voronoi_topology().is_empty());

        let bounded_i = valid
            .edges
            .iter()
            .position(|e| matches!(e.kind, VoronoiEdgeKind::Bounded { .. }))
            .expect("the fixture has exactly one Bounded edge (p1-p2)");
        let unbounded_i = valid
            .edges
            .iter()
            .position(|e| matches!(e.kind, VoronoiEdgeKind::Unbounded { .. }))
            .expect("the fixture has 5 Unbounded hull edges");
        let excluded_interior_edge = valid
            .delaunay
            .edges()
            .find(|&e| {
                matches!(valid.delaunay.adjacent_faces(e), [Some(_), Some(_)])
                    && !valid.edges.iter().any(|ve| ve.source_edge == e)
            })
            .expect("the square's cocircular diagonal (p0-p2) is excluded and interior");

        let mut broken = valid.clone();
        broken.edges[bounded_i].cells[1] = broken.edges[bounded_i].cells[0];
        assert!(broken.validate_voronoi_topology().contains(
            &VoronoiTopologyError::NonDistinctEdgeCells {
                edge: VoronoiEdgeId(bounded_i as u32)
            }
        ));

        let mut broken = valid.clone();
        if let VoronoiEdgeKind::Bounded { vertices } = &mut broken.edges[bounded_i].kind {
            vertices[1] = vertices[0];
        }
        assert!(broken.validate_voronoi_topology().contains(
            &VoronoiTopologyError::NonDistinctBoundedVertices {
                edge: VoronoiEdgeId(bounded_i as u32)
            }
        ));

        let mut broken = valid.clone();
        broken.edges[unbounded_i].source_edge = excluded_interior_edge;
        assert!(broken.validate_voronoi_topology().contains(
            &VoronoiTopologyError::UnboundedEdgeNotHullDual {
                edge: VoronoiEdgeId(unbounded_i as u32)
            }
        ));

        let mut broken = valid.clone();
        let dup = broken.group_faces[0][0];
        broken.group_faces[0].push(dup);
        assert!(broken.validate_voronoi_topology().contains(
            &VoronoiTopologyError::DuplicateFaceInGroup {
                vertex: VoronoiVertexId(0),
                face: dup,
            }
        ));
    }

    // -- cell_edges (Phase 7C) --

    fn other_cell_site(v: &Voronoi2, e: VoronoiEdgeId, this: VoronoiCellId) -> VertexId {
        let [a, b] = v.edge_cells(e);
        v.cell_site(if a == this { b } else { a })
    }

    #[test]
    fn cell_edges_square_corner_is_exactly_two_rays() {
        // Every prior "fully cocircular" fixture in this file collapses
        // the same way for cell_edges: the excluded diagonal leaves each
        // corner with exactly its 2 hull rays and no Bounded edge.
        let pts = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.0)];
        let v = voronoi2(delaunay2(&pts));
        for cell in v.cells() {
            let edges: Vec<_> = v.cell_edges(cell).collect();
            assert_eq!(edges.len(), 2);
            assert!(
                edges
                    .iter()
                    .all(|&e| matches!(v.edge_kind(e), VoronoiEdgeKind::Unbounded { .. }))
            );
        }
        assert!(v.validate_voronoi_topology().is_empty());
    }

    #[test]
    fn cell_edges_cocircular_ngon_multiple_triangulations_all_two_rays() {
        for &n in &[5usize, 8] {
            let pts = cocircular_lattice_points(n);
            for start in [0, n / 2] {
                let v = voronoi2(assemble_triangulation(pts.clone(), fan_from(n, start)));
                for cell in v.cells() {
                    assert_eq!(
                        v.cell_edges(cell).count(),
                        2,
                        "n={n} start={start}: every site's cell must be exactly 2 rays \
                         once the whole n-gon merges into one Voronoi vertex"
                    );
                }
                assert!(v.validate_voronoi_topology().is_empty());
            }
        }
    }

    #[test]
    fn cell_edges_partially_cocircular_fixture_matches_hand_verified_order() {
        // Same fixture as
        // cocircular_cluster_plus_outlier_mixes_bounded_and_excluded_edges:
        // a cocircular square (p0..p3) plus a far outlier p4 pulled in as
        // an ear against edge p1-p2.
        let pts = vec![
            pt(0.0, 0.0),
            pt(2.0, 0.0),
            pt(2.0, 2.0),
            pt(0.0, 2.0),
            pt(6.0, 1.0),
        ];
        let faces = vec![
            [VertexId(1), VertexId(4), VertexId(2)],
            [VertexId(0), VertexId(1), VertexId(2)],
            [VertexId(0), VertexId(2), VertexId(3)],
        ];
        let v = voronoi2(assemble_triangulation(pts, faces));

        // p0: a pure square corner untouched by p4 -- 2 rays, same shape
        // as cell_edges_square_corner_is_exactly_two_rays.
        let p0 = VoronoiCellId(VertexId(0));
        let p0_edges: Vec<_> = v.cell_edges(p0).collect();
        assert_eq!(p0_edges.len(), 2);
        assert_eq!(other_cell_site(&v, p0_edges[0], p0), VertexId(1));
        assert_eq!(other_cell_site(&v, p0_edges[1], p0), VertexId(3));

        // p1: hand-verified CCW order against p1's actual neighbor
        // directions -- p4 at ~14 deg, p2 at 90 deg, p0 at 180 deg from
        // p1=(2,0). Entry ray to p4, a genuine Bounded edge to p2 (not
        // excluded -- p4 is nowhere near the square's circumcircle),
        // exit ray to p0.
        let p1 = VoronoiCellId(VertexId(1));
        let p1_edges: Vec<_> = v.cell_edges(p1).collect();
        assert_eq!(p1_edges.len(), 3);
        assert_eq!(other_cell_site(&v, p1_edges[0], p1), VertexId(4));
        assert!(matches!(
            v.edge_kind(p1_edges[0]),
            VoronoiEdgeKind::Unbounded { .. }
        ));
        assert_eq!(other_cell_site(&v, p1_edges[1], p1), VertexId(2));
        assert!(matches!(
            v.edge_kind(p1_edges[1]),
            VoronoiEdgeKind::Bounded { .. }
        ));
        assert_eq!(other_cell_site(&v, p1_edges[2], p1), VertexId(0));
        assert!(matches!(
            v.edge_kind(p1_edges[2]),
            VoronoiEdgeKind::Unbounded { .. }
        ));

        // edge_cells() consistency, for every cell -- the user's
        // explicit minimum requirement.
        for cell in v.cells() {
            for e in v.cell_edges(cell) {
                assert!(v.edge_cells(e).contains(&cell));
            }
        }
        assert!(v.validate_voronoi_topology().is_empty());
    }

    #[test]
    fn cell_edges_near_cocircular_but_not_does_not_merge() {
        // p3 nudged 0.001 off the circle through p0,p1,p2 -- close, but
        // the exact incircle predicate must not treat it as cocircular.
        let pts = vec![pt(0.0, 0.0), pt(2.0, 0.0), pt(2.0, 2.0), pt(0.0, 2.001)];
        let faces = vec![
            [VertexId(0), VertexId(1), VertexId(2)],
            [VertexId(0), VertexId(2), VertexId(3)],
        ];
        let v = voronoi2(assemble_triangulation(pts, faces));

        assert_eq!(
            v.group_faces.len(),
            2,
            "a near-miss cocircular quad must not merge into one Voronoi vertex"
        );
        let p0_edges: Vec<_> = v.cell_edges(VoronoiCellId(VertexId(0))).collect();
        assert_eq!(
            p0_edges.len(),
            3,
            "p0's diagonal to p2 is now a genuine Bounded edge, not excluded"
        );
        assert!(
            p0_edges
                .iter()
                .any(|&e| matches!(v.edge_kind(e), VoronoiEdgeKind::Bounded { .. }))
        );
        assert!(v.validate_voronoi_topology().is_empty());
    }

    #[test]
    fn cell_edges_interior_cell_is_cyclically_consistent_on_a_60_point_cloud() {
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

        let mut rng = Xorshift64(0x00c0_ffee_1234_5678);
        let pts: Vec<Point2> = (0..60)
            .map(|_| pt(rng.next_f64_in(100.0), rng.next_f64_in(100.0)))
            .collect();
        let v = voronoi2(delaunay2(&pts));
        assert!(v.validate_voronoi_topology().is_empty());

        let hull_sites: std::collections::HashSet<VertexId> = v
            .delaunay
            .boundary_edges()
            .flat_map(|e| {
                let (a, b) = v.delaunay.edge_vertices(e);
                [a, b]
            })
            .collect();

        let mut checked_interior_cell = false;
        for cell in v.cells() {
            let edges: Vec<_> = v.cell_edges(cell).collect();
            for &e in &edges {
                assert!(v.edge_cells(e).contains(&cell));
            }

            if hull_sites.contains(&v.cell_site(cell)) {
                assert!(edges.len() >= 2);
                assert!(matches!(
                    v.edge_kind(edges[0]),
                    VoronoiEdgeKind::Unbounded { .. }
                ));
                assert!(matches!(
                    v.edge_kind(*edges.last().unwrap()),
                    VoronoiEdgeKind::Unbounded { .. }
                ));
                // Same ordering property as the interior/cyclic case,
                // but *not* wrapping past the last edge back to the
                // first -- a hull cell's sequence is linear, not a
                // closed loop. Without this, a walk that emitted the
                // right multiset of edges with the middle ones out of
                // order (or reversed) would still pass the first/last
                // check above.
                let endpoint_set = |e: VoronoiEdgeId| -> Vec<VoronoiVertexId> {
                    match v.edge_kind(e) {
                        VoronoiEdgeKind::Bounded { vertices } => vertices.to_vec(),
                        VoronoiEdgeKind::Unbounded { finite_vertex } => vec![*finite_vertex],
                    }
                };
                for w in 0..edges.len() - 1 {
                    let a = endpoint_set(edges[w]);
                    let b = endpoint_set(edges[w + 1]);
                    let shared = a.iter().filter(|x| b.contains(x)).count();
                    assert_eq!(
                        shared, 1,
                        "consecutive hull cell_edges must share exactly one Voronoi vertex, in order"
                    );
                }
            } else {
                // The check that actually distinguishes a correctly
                // *ordered* walk from one that emits the right *set* in
                // the wrong order: consecutive edges (cyclically,
                // wrapping past the last back to the first) must share
                // exactly one Voronoi vertex.
                checked_interior_cell = true;
                assert!(
                    edges.len() >= 3,
                    "an interior Voronoi cell needs at least 3 edges to close"
                );
                let endpoints = |e: VoronoiEdgeId| -> [VoronoiVertexId; 2] {
                    match v.edge_kind(e) {
                        VoronoiEdgeKind::Bounded { vertices } => *vertices,
                        VoronoiEdgeKind::Unbounded { .. } => {
                            panic!("an interior cell cannot have an Unbounded edge")
                        }
                    }
                };
                for w in 0..edges.len() {
                    let a = endpoints(edges[w]);
                    let b = endpoints(edges[(w + 1) % edges.len()]);
                    let shared = a.iter().filter(|x| b.contains(x)).count();
                    assert_eq!(
                        shared, 1,
                        "consecutive cell_edges must share exactly one Voronoi vertex, in order"
                    );
                }
            }
        }
        assert!(
            checked_interior_cell,
            "a 60-point generic-position cloud must have at least one interior site"
        );
    }
}
