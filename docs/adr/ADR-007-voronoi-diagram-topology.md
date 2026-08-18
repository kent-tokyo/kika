# ADR-007: Voronoi diagram topology API (0.5.0)

Status: Decided and implemented, shipped in 0.5.0 (2026-08-19,
`v0.5.0` tag at `ef5cdda`). `src/triangulation/voronoi.rs`:
`Voronoi2`/`voronoi2` (cocircular-group union-find, canonical dense id
assignment, internal validator) and the query API through the ordered
`cell_edges()` boundary walk. No version bump, dependency, or
performance measurement beyond what's noted as still deferred below
(circumcenter coordinates, clipping, nearest-neighbor) was done under
this ADR — that remains accurate as of shipping, not just at proposal
time. See "Explicitly out of scope for 0.5.0" below.

## Context

`ROADMAP.md` (internal, gitignored) lists a Voronoi diagram topology API
as 0.5.0, the next scope-expanding release after 0.4.0's polygon-with-holes
work, and already sketches a rough `Voronoi2`/`VoronoiCellId`/
`VoronoiEdgeId`/`VoronoiVertexId` shape. It also already re-examined
`ADR-006-triangulation-adjacency-structure.md`'s rejection of quad-edge/DCEL
("outright... since no future Voronoi-diagram scope exists to revisit it
against") now that Voronoi is real scope, and found — by tracing every
operation the sketch needs against `Triangulation2`'s already-public
methods — that ordinary indexed-triangle adjacency is sufficient; **this
ADR does not reopen ADR-006**, it builds on that finding.

What neither the roadmap sketch nor ADR-006 resolved is the actual hard
problem: `delaunay2` (Phase 4) already documents that a cocircular point
set has more than one valid Delaunay triangulation, and picks one via a
deterministic-but-arbitrary tie-break (`Sign::Zero` is "not bad" — see
`delaunay2`'s own doc comment and `docs/degeneracy-policy.md`). A Voronoi
dual built naively face-by-face over *that specific* triangulation would
leak the tie-break into the output: a cocircular quad's two Delaunay
triangles (however the tie-break happened to split them) would produce two
separate Voronoi "vertices" joined by a spurious edge, when the true
Voronoi diagram has exactly one vertex there (the shared circumcenter) and
no such edge. This ADR's central decision is how to normalize an arbitrary
Delaunay triangulation into the one, tie-break-independent Voronoi topology
it's actually dual to — everything else here is comparatively mechanical.

## Scope for 0.5.0

**In scope**: Voronoi topology as the dual of an existing `Triangulation2`
— cells, edges, vertices as combinatorial objects with typed IDs and
correspondence back to `VertexId`/`EdgeId`/`FaceId`. No coordinates for
Voronoi vertices (no circumcenter construction).

**Explicitly out of scope for 0.5.0** (per the requester's constraint,
restated here so it's load-bearing, not just a closing footnote):
circumcenter/coordinate construction, clipped Voronoi (bounding a diagram
to a rectangle/polygon), weighted Voronoi (power diagrams), nearest-neighbor
queries, any new runtime dependency, performance measurement/optimization.
Also out of scope for *this round specifically*: writing any `src/` code,
`Cargo.toml` changes, pushing, or releasing — this document is a design
artifact only.

## Basic correspondence

| Delaunay | Voronoi | Relationship |
|---|---|---|
| `VertexId` (site) | `VoronoiCellId` | Bijection — every site has exactly one cell, always. |
| `EdgeId` (non-spurious) | `VoronoiEdgeId` | Partial injection — most Delaunay edges dual to exactly one Voronoi edge; edges internal to a cocircular group (defined below) dual to none. |
| A *group* of `FaceId`s | `VoronoiVertexId` | Many-to-one — one or more cocircular Delaunay faces collapse to one Voronoi vertex. |

The vertex↔cell and (filtered) edge↔edge correspondences are structurally
trivial given `Triangulation2`'s existing API (confirmed by the ROADMAP
sketch's own trace, restated in "Rejected alternatives" below). The
face-group↔vertex correspondence is the one genuinely new piece of
algorithm this ADR designs.

## The central problem: normalizing to canonical Voronoi topology

### Cocircular face grouping

**Rule**: two Delaunay faces `f1`, `f2` sharing an edge belong to the same
Voronoi-vertex group iff their four defining points are exactly cocircular.
Concretely, for shared edge `e` with `(u, v) = edge_vertices(e)`, `a` = the
vertex of `f1` not on `e`, `b` = the vertex of `f2` not on `e`:

```text
cocircular(f1, f2)  :=  incircle(u, v, a, b) == Sign::Zero
```

`incircle`'s own doc comment confirms `Sign::Zero` means exactly "the four
points are cocircular" — and confirms swapping any two of the first three
arguments only flips `Positive`/`Negative`, never changes `Zero`-ness, so
the winding/order `u, v, a` is passed in doesn't matter for this test; only
consistency (same `u`, `v` from the shared edge, `a` from `f1`, `b` from
`f2`) matters.

**Grouping**: run union-find over `FaceId` (`0..faces.len()`), unioning
`f1`/`f2` for every interior edge where `cocircular(f1, f2)` holds. Each
resulting connected component is one `VoronoiVertexId`.

**Correctness argument (why pairwise-adjacent testing is sufficient for
arbitrarily large cocircular groups, not just size-4 quads)**: three
non-collinear points determine a circle uniquely. If `cocircular(f1, f2)`
holds via shared edge `(u,v)`, then `circle(u,v,a) == circle(u,v,b)` — both
equal the one circle all four points lie on. If `f2` (`= (u,v,b)`, in some
vertex order) is also adjacent to `f3` across a *different* one of its own
edges — necessarily an edge drawn from `{u,v,b}`, since those are `f2`'s
only three vertices — and `cocircular(f2, f3)` holds, then `f3`'s opposite
vertex `c` lies on `circle` of *that* shared edge's two vertices plus `b`
(or `u`, or `v`) — which is the same circle as before, by the same
uniqueness argument. Induction over the connected component's BFS/DFS
order shows every face in the component shares the *one* circle the first
edge established. This is a proof, not an assumption — but per this
project's own "measure it, don't just derive it" discipline (see
`tasks/lessons.md` repeatedly), it should still be exercised by a property
test with a genuinely large (5+) cocircular cluster before being trusted
in practice (listed under "Assumptions to prove or test" below).

### Excluding spurious edges

Once faces are grouped, classify every Delaunay `EdgeId`:

- **Boundary** (1 incident face `f`): dual to exactly one **unbounded**
  Voronoi edge, finite endpoint = `f`'s group.
- **Interior, different groups** (`group(f1) != group(f2)`): dual to
  exactly one **bounded** Voronoi edge between the two groups.
- **Interior, same group** (`group(f1) == group(f2)`): **excluded** — no
  Voronoi edge minted at all. This is exactly the "spurious diagonal from
  the cocircular tie-break" case the roadmap sketch named; it disappears
  by construction rather than needing a separate filter step bolted on
  afterward.

## Unbounded topology

No synthetic large coordinates, no "point at infinity" sentinel coordinate
(this crate already has and rejected that pattern once, for a different
reason — `delaunay2`'s single symbolic ghost *vertex*, which is an internal
Bowyer-Watson construction detail stripped before `Triangulation2` is ever
public, not something a Voronoi edge should reinvent or rely on).

```rust
/// #[non_exhaustive]: 0.5.0 only ever produces `Bounded`/`Unbounded` (its
/// scope is ≥3 non-collinear sites, where every Voronoi edge is either a
/// finite segment or a single-direction ray). A future release supporting
/// 1-2 site input (see "Revisit when") would need a `Line` variant — both
/// directions infinite, no finite endpoint at all — which doesn't fit
/// either existing variant. Marking this non_exhaustive now means that
/// addition is non-breaking later, the same lesson 0.2.0's `CdtError`
/// already taught this crate the hard way (see `CHANGELOG.md`'s 0.3.0
/// entry) — except applied here to a *classification* enum, not a
/// `Result` error type. This sharpens the criterion
/// `tasks/lessons.md` recorded for that case (`Result`-error vs. closed
/// classification): the real question is whether the variant set is
/// complete by mathematical necessity forever (`Sign`, `Orientation` —
/// a real number's sign has exactly 3 possibilities, full stop) or only
/// complete under the *current* problem scope (`VoronoiEdgeKind` — closed
/// for "≥3 non-collinear sites" but not closed in general). The latter
/// gets `#[non_exhaustive]` too, regardless of whether it's an error type.
#[non_exhaustive]
pub enum VoronoiEdgeKind {
    /// Both sides finite — a genuine segment between two Voronoi vertices.
    Bounded { vertices: [VoronoiVertexId; 2] },
    /// One finite side; the other extends to infinity. The direction/ray
    /// itself is not represented (topology-only, no coordinates) — only
    /// which finite vertex it leaves from.
    Unbounded { finite_vertex: VoronoiVertexId },
}
```

The "which hull edge does this unbounded edge come from" requirement does
**not** need a separate field on `VoronoiEdgeKind` itself — see "Internal
data structures" below: `VoronoiEdge.source_edge` (provenance) and
`VoronoiEdge.cells` (the two sites, always present regardless of
`Bounded`/`Unbounded`) together already cover it, and `cells` also serves
"the owned-type self-sufficiency" requirement from review (below) — an
`Unbounded` edge's `cells` pair already names both sites of its
originating hull edge directly, no dereference through `source_edge`
required.

## ID design

```rust
/// Wraps the corresponding Delaunay VertexId -- a true bijection, so no
/// separate dense counter is needed. See VertexId's own doc comment for
/// the cross-triangulation validity caveat, which applies identically
/// here (valid only for the Triangulation2 the owning Voronoi2 was built
/// from).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiCellId(pub(super) VertexId);

/// A genuinely new dense id -- Voronoi vertices are a many-to-one merge
/// of Delaunay faces, not a bijection with any existing id type. Dense
/// index assigned by canonical key, not by union-find root or face-scan
/// order -- see "Construction algorithm".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiVertexId(pub(super) u32);

/// A genuinely new dense id -- see "Rejected alternatives" for why this
/// isn't just a filtered reuse of EdgeId. Dense index assigned by
/// canonical key, same discipline as VoronoiVertexId -- see
/// "Construction algorithm".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiEdgeId(pub(super) u32);
```

Same privacy convention as `VertexId`/`EdgeId`/`FaceId`: `pub(super)` inner
field (opaque outside the crate, constructible only from within
`triangulation::*`), same derive list (`Debug, Clone, Copy, PartialEq, Eq,
Hash`).

**Canonical, not incidental, numbering.** `VoronoiVertexId`/`VoronoiEdgeId`
being *dense* only says their values are `0..count` with no gaps — it says
nothing about *which* group gets `0` vs. `1`. Left to union-find's own
root selection or face-scan order, two differently-triangulated (but
topologically identical) `Triangulation2`s of the same point set could
produce the same `Voronoi2` *shape* with different `VoronoiVertexId`/
`VoronoiEdgeId` *numbers* — a real gap for something claiming to be
"the Delaunay tie-break's canonical, independent dual." Closed by
assigning dense ids from a canonical sort key derived from site identity
(`VertexId`), not from internal Delaunay bookkeeping — see "Construction
algorithm" for the exact keys. **Compatibility note**: this guarantees
*same input, same Kika version → same numbering*, not persistent
cross-version id stability as part of the public contract — the same
scope limitation this crate's other opaque ids already carry implicitly,
made explicit here since canonical-by-construction numbering could
otherwise read as a stronger promise than intended.

## Public API (pseudo-Rust — illustrative, not implementation)

```rust
pub fn voronoi2(triangulation: Triangulation2) -> Voronoi2;

impl Voronoi2 {
    // -- cells (bijection with Delaunay vertices/sites) --
    pub fn cells(&self) -> impl Iterator<Item = VoronoiCellId> + '_;
    pub fn cell_site(&self, cell: VoronoiCellId) -> VertexId;
    pub fn neighboring_cells(&self, cell: VoronoiCellId) -> impl Iterator<Item = VoronoiCellId> + '_;
    pub fn is_unbounded(&self, cell: VoronoiCellId) -> bool;

    // -- edges --
    pub fn edges(&self) -> impl Iterator<Item = VoronoiEdgeId> + '_;
    pub fn dual_delaunay_edge(&self, edge: VoronoiEdgeId) -> EdgeId;
    pub fn edge_kind(&self, edge: VoronoiEdgeId) -> VoronoiEdgeKind;
    /// The two cells this edge separates. Stored directly on the owned
    /// `VoronoiEdge` record (see "Internal data structures") rather than
    /// re-derived through `dual_delaunay_edge` each call -- see "Owned
    /// self-sufficiency" below for why.
    pub fn edge_cells(&self, edge: VoronoiEdgeId) -> [VoronoiCellId; 2];

    // -- vertices --
    pub fn vertices(&self) -> impl Iterator<Item = VoronoiVertexId> + '_;
    /// The (one or more, for a cocircular group) Delaunay faces this
    /// Voronoi vertex merges.
    pub fn delaunay_faces(&self, vertex: VoronoiVertexId) -> &[FaceId];

    // -- escape hatch --
    pub fn triangulation(&self) -> &Triangulation2;
}
```

Every existing id type gets a correspondence accessor as required:
`VertexId` via `cell_site`, `EdgeId` via `dual_delaunay_edge`, `FaceId` via
`delaunay_faces`. `neighboring_cells`/`is_unbounded` are derived on demand
from `Triangulation2`'s own `edges()`/`edge_vertices()`/`boundary_edges()`
(no separate per-cell adjacency list stored — see "Internal data
structures"), matching how the rest of this crate scans adjacency on
demand rather than pre-indexing it (`triangulate_polygon`'s flood fill,
`cdt.rs`'s face-scanning helpers).

## Internal data structures

```rust
pub struct Voronoi2 {
    delaunay: Triangulation2,
    /// Dense, indexed by FaceId.raw() as usize -- which Voronoi vertex
    /// group each Delaunay face belongs to.
    face_group: Vec<VoronoiVertexId>,
    /// Dense, indexed by VoronoiVertexId.0 as usize -- the inverse of
    /// face_group, precomputed once for delaunay_faces().
    group_faces: Vec<Vec<FaceId>>,
    /// Dense, indexed by VoronoiEdgeId.0 as usize -- every surviving
    /// (non-spurious) Voronoi edge.
    edges: Vec<VoronoiEdge>,
}

pub struct VoronoiEdge {
    /// The two cells this edge separates. Present and meaningful for
    /// *both* Bounded and Unbounded edges alike (an edge's two cells and
    /// its own finite/infinite endpoint topology are orthogonal
    /// questions) -- the canonical semantic content of "which sites does
    /// this edge separate," stored directly rather than only reachable
    /// by dereferencing `source_edge` through a live `Triangulation2`.
    pub cells: [VoronoiCellId; 2],
    /// Provenance only: which Delaunay edge this is dual to. Useful for
    /// cross-referencing back into `Triangulation2` (e.g. `triangulation()`)
    /// when it's available, but `cells`/`kind` are already the complete,
    /// self-contained semantic fact -- nothing about interpreting this
    /// edge *requires* re-resolving `source_edge`.
    pub source_edge: EdgeId,
    pub kind: VoronoiEdgeKind,
}
```

### Owned self-sufficiency

Review flagged a real gap in the original sketch (`VoronoiEdgeRecord {
dual: EdgeId, endpoints: VoronoiEdgeEndpoints }`, `edge_cells` derived by
re-resolving `dual` through `self.delaunay` on every call): while
*functionally* correct as long as a `&Voronoi2` is in hand (which it
always is, for every method call, under the owned design), it made the
edge's *canonical* fact — which two sites it separates — only indirectly
reachable, through what's structurally provenance data. Two consequences
that motivate storing `cells` directly instead:

1. **Canonical-key sorting (below) needs `cells` as sortable data
   anyway** — computing it fresh from `source_edge` on every comparison
   during construction, rather than once and storing it, is pure
   overhead for no benefit.
2. **Forward-compatible for any future API that yields a detached,
   owned `VoronoiEdge`** (e.g. an `edges().map(|id| voronoi.edge(id))`-style
   accessor returning the record by value rather than by id) — with
   `cells` stored directly, such a value is immediately self-describing;
   with only `source_edge`, it would need the caller to *also* hold onto
   `&Voronoi2`/`&Triangulation2` just to interpret a value they already
   have in hand.

`source_edge` is kept, explicitly demoted to provenance/debugging value —
dropping it entirely would lose real information (which specific Delaunay
edge produced this, useful for visualization/debugging against the
underlying `Triangulation2`), it's just no longer load-bearing for
interpreting the edge's own meaning.

### Construction algorithm (for the design record, not code to write yet)

1. Union-find over `0..faces.len()`; for each interior edge (2 incident
   faces), union them iff `cocircular(f1, f2)` (defined above).
2. Resolve to canonical groups (connected components of `FaceId`).
3. **Canonical `VoronoiVertexId` assignment.** For each group, compute its
   canonical key: the sorted, deduplicated set of every `VertexId` across
   all member faces (i.e. the full set of Delaunay sites incident to that
   Voronoi vertex — not the union-find root, not the minimum member
   `FaceId`, not any other internal-bookkeeping artifact). Sort all groups
   by this key (lexicographic over each group's sorted `VertexId.raw()`
   list) and assign `VoronoiVertexId(0), (1), (2), ...` in that order.
   This is what makes "which cocircular diagonal `delaunay2` happened to
   pick doesn't matter" hold for the actual **numbering**, not just the
   topology-up-to-isomorphism: two differently-triangulated Delaunay
   inputs of the *same* point set now provably produce identical
   `Voronoi2` output, id-for-id, not merely isomorphic output.
4. Build `face_group` (dense, size `faces.len()`) and `group_faces`
   (inverse) from the canonically-numbered groups.
5. For each `EdgeId`, classify per "Excluding spurious edges" above.
   **Canonical `VoronoiEdgeId` assignment**: for each surviving edge,
   compute its canonical key as the sorted pair of its two `VoronoiCellId`s
   (by underlying `VertexId.raw()`) — sort all surviving edges by this key
   and assign `VoronoiEdgeId(0), (1), (2), ...` in that order, same
   discipline as step 3, for the same reason.
6. Iterate `delaunay.edges()`/`faces()` themselves in their own natural
   (already-deterministic) order throughout construction — no `HashMap`
   iteration anywhere in this pass (the canonical-key sort in steps 3/5
   is what supplies determinism at the numbering level; an incidental
   `HashMap`-iteration-order dependency anywhere else in the pass would
   silently undermine it).

## Ownership and lifetime

**Option A — owned** (recommended):

```rust
pub struct Voronoi2 {
    delaunay: Triangulation2,  // owned, moved in at construction
    // ...
}
```

**Option B — borrowed**:

```rust
pub struct Voronoi2<'a> {
    delaunay: &'a Triangulation2,
    // ...
}
```

| | Option A (owned) | Option B (borrowed) |
|---|---|---|
| Lifetime parameter | None — propagates nowhere | `'a` on `Voronoi2` and everything that holds one |
| Precedent in this crate | `ConstrainedTriangulation2` owns its `Triangulation2` outright (confirmed: `struct ConstrainedTriangulation2 { triangulation: Triangulation2, constrained_edges: HashSet<EdgeId> }`) | None — no existing type in this crate borrows another of the crate's own structural types |
| Derived-data storage | `face_group`/`group_faces`/`edges` owned regardless | Same — these are Voronoi-specific, `Triangulation2` has no concept of them, so even the borrowed option must own them. Borrowing only avoids duplicating `Triangulation2`'s own fields, not the actually-new bookkeeping. |
| Staleness risk | None — `Triangulation2` is documented as "a static, post-construction snapshot" with no mutation API, so a moved-in copy can never drift from a hypothetical external one | None currently, for the same reason — but *would* become a real risk if `Triangulation2` ever gained a mutation API (noted in "Revisit when") |
| Cost | One extra `Triangulation2` clone worth of memory if the caller also needs the original (`Triangulation2: Clone`) | None, but see above — the savings are smaller than they look |

**Recommendation: Option A.** The borrowed option's main theoretical
saving (avoid duplicating vertex/face/edge arrays) is real but bounded —
`Triangulation2` is already documented as an immutable snapshot, so a
duplicate is exactly as valid forever as the original, not a staleness
risk — while its cost (a lifetime parameter propagating into every type
and function signature that holds or returns a `Voronoi2`) is a genuine,
permanent ergonomic tax with no precedent anywhere else in this crate.
`ConstrainedTriangulation2` already made this exact call for an analogous
situation (composition, not borrowing) and nothing about Voronoi's shape
argues for a different answer.

## Determinism and invariants

All of the following should hold, and are addressed by construction (not
left to be true "by luck"):

1. **Input point order doesn't matter.** Inherited from `delaunay2`'s own
   documented order-independence (canonical sort before insertion) —
   `Voronoi2` adds no new order-dependence as long as construction avoids
   `HashMap` iteration (see algorithm step 6).
2. **Independent of which cocircular diagonal `delaunay2` picked.**
   Guaranteed by the cocircular-grouping correctness argument above: any
   valid tie-break triangulation of the same cocircular group produces the
   same connected component. Strengthened by the canonical-key id
   assignment (algorithm steps 3/5): the same connected component also
   gets the same `VoronoiVertexId` *number* across differently-triangulated
   inputs of the same point set, not merely an isomorphic one — see
   "Canonical-topology property tests".
3. **Exactly one `VoronoiCellId` per Delaunay site.** True by construction
   — `VoronoiCellId` wraps `VertexId` directly, no filtering.
4. **Exactly one `VoronoiEdgeId` per non-degenerate interior Delaunay
   edge.** True by construction — `union()` is only called when
   `cocircular(f1,f2)` holds, so a genuinely non-cocircular interior edge's
   two faces are never merged, and its classification always falls into
   the "different groups" (bounded) case.
5. **Spurious cocircular-tie-break edges excluded.** True by construction
   — the "same group" case is the exclusion rule itself.
6. **Hull edges correspond to unbounded edges.** True by construction —
   every boundary edge (1 incident face) is classified `Unbounded`.

## Degenerate input policy (0.5.0)

| Case | Behavior |
|---|---|
| Empty input | `Triangulation2::empty()` has 0 vertices, 0 faces, 0 edges — `Voronoi2` built from it has 0 cells, 0 vertices, 0 edges. |
| 1 point | **Important, non-obvious inherited limitation**: `delaunay2` returns `Triangulation2::empty()` for *any* input with fewer than 3 non-collinear points — confirmed by reading `delaunay2`'s own source (`if hull.len() < 3 { return Triangulation2::empty(); }`, and `Triangulation2::empty()` sets `vertices: Vec::new()`, discarding the input points entirely, not just the faces). A single site's Voronoi diagram is mathematically well-defined (one cell, the whole unbounded plane) — but since this ADR builds `Voronoi2` strictly as the dual of an existing `Triangulation2`, and that `Triangulation2` has already discarded the site, `Voronoi2` shows 0 cells too. Not fixed in this design — see "Explicitly out of scope" and "Revisit when". |
| 2 points | Same as 1 point — `hull.len() < 3`, `Triangulation2::empty()`, 0 cells. A 2-site Voronoi diagram (two unbounded half-plane cells split by the perpendicular bisector) is also mathematically well-defined and also not exposed, for the same reason. |
| All points exactly collinear (3+) | Same as above — `convex_hull2`'s `hull.len() < 3` check catches this case too (a collinear set's hull is a degenerate 2-point extremes-only line), so `Triangulation2::empty()`, 0 cells. |
| Duplicate points (exact coordinate equality) | Already collapsed by `delaunay2` itself (`dedup_sorted`) before `Triangulation2` is ever built — `Voronoi2` never sees a duplicate site; no special-casing needed. |
| 4 or more points exactly cocircular | Handled by the cocircular-grouping algorithm above — collapses to one shared `VoronoiVertexId` regardless of which Delaunay diagonal(s) the tie-break picked. The maximal case (**all** input points on one common circle, "fan" triangulation) is a clean, exact test of this: every Delaunay face is pairwise-cocircular with its neighbors (they all share the one circle), so the whole triangulation collapses to **exactly one** `VoronoiVertexId`, with every hull edge becoming an `Unbounded` edge radiating from it and **zero** `Bounded` edges — which is also the literal true Voronoi diagram of `n` cocircular sites (all perpendicular bisectors between cocircular points pass through their shared circle's center). See "Acceptance tests". |
| A point exactly on a hull edge (collinear with two hull vertices, but the full set isn't all-collinear) | No special-casing needed: the point is still a genuine `Triangulation2` vertex (Bowyer-Watson inserts it normally), and its own two edges along that hull line are themselves boundary edges (1 incident face each) by the existing `boundary_edges()` definition — so `is_unbounded` on its cell already returns `true` automatically, with no Voronoi-specific collinearity check required. |

**This ADR does not change `delaunay2`'s or `Triangulation2`'s own
degenerate-input policy.** The 1-point/2-point/all-collinear gap above is
a real, known limitation of building Voronoi strictly as an existing
`Triangulation2`'s dual, named explicitly rather than silently — see
"Revisit when" for the alternative (an independent raw-points
constructor) this design deliberately doesn't take on for 0.5.0.

## Rejected alternatives

- **Quad-edge / DCEL structure.** Already investigated in `ROADMAP.md`
  (not part of this ADR's own work, cited for completeness): every
  operation this design needs is already available from
  `Triangulation2`'s public, already-implemented, already-tested adjacency
  API (`vertices`, `edges`, `edge_vertices`, `adjacent_faces`,
  `face_vertices`, `boundary_edges`). `ADR-006`'s quad-edge rejection
  stands; this design does not reopen it.
- **Naive face = vertex model (no cocircular grouping).** The obvious
  first idea — one `VoronoiVertexId` per `FaceId`, no merging. Rejected:
  produces a spurious extra vertex and edge for every cocircular group,
  and (worse) makes the result depend on `delaunay2`'s arbitrary tie-break
  choice, violating "independent of which cocircular diagonal was
  picked" — the whole reason this ADR exists as more than a one-paragraph
  wrapper.
- **Borrowed `Voronoi2<'a>(&'a Triangulation2)`.** See "Ownership and
  lifetime" above — real but bounded savings, permanent ergonomic cost,
  no precedent in this crate, and `ConstrainedTriangulation2` already
  chose ownership for an analogous case.
- **Reusing `EdgeId` directly as `VoronoiEdgeId`** (since the map from
  Delaunay edge to Voronoi edge is already an injection — no edge produces
  more than one Voronoi edge). Considered: would make
  `dual_delaunay_edge` the identity function and save one dense-array
  allocation. Rejected in favor of a fresh dense `VoronoiEdgeId`: `EdgeId`
  space isn't dense once spurious edges are excluded (iterating "all
  Voronoi edges" would mean filtering every Delaunay edge every time,
  unless the exclusion set is precomputed anyway — at which point the
  savings mostly disappear), and it breaks symmetry with `VoronoiVertexId`
  (which *cannot* reuse `FaceId` directly, being many-to-one) — having one
  Voronoi id type reuse a Delaunay one and the other not would be a more
  surprising, less consistent API than minting both fresh, matching this
  crate's existing convention of dense, purpose-specific id types
  (`VertexId`/`EdgeId`/`FaceId` themselves, none of which alias each
  other despite `Triangulation2` obviously having numeric relationships
  between their counts).
- **Independent `VoronoiCellId` counter (not wrapping `VertexId`).**
  Rejected — cell↔site is a proven, total bijection once `Triangulation2`
  is fixed; a separate counter would need its own indirection table for
  zero benefit over a direct wrapper.
- **Eager circumcenter (coordinate) computation for Voronoi vertices.**
  Rejected for 0.5.0 — this is a *new certified construction* problem in
  its own right (a merged cocircular group's shared circumcenter, exact/
  correctly-rounded), the same category of work `ADR-004`'s Phase 5
  section treats as deliberate, separately-verified work, not something
  to bundle casually into a structural change. See "Revisit when".
- **Deriving `edge_cells` on demand via `source_edge` instead of storing
  `cells` directly on `VoronoiEdge`.** This was this ADR's own first
  draft, revised after review: functionally correct under the owned
  design (every method call has `&self.delaunay` available), but left
  the edge's actual canonical fact — which two sites it separates — only
  indirectly reachable through what is structurally provenance data, and
  meant re-deriving it on every canonical-key sort comparison during
  construction for no benefit. See "Owned self-sufficiency" above.

## Compatibility risk

- This design adds only new public items (`Voronoi2`, `VoronoiEdge`, three
  new id types, `VoronoiEdgeKind`, one free function) — zero risk to any
  existing public API.
- `VoronoiEdgeKind` is `#[non_exhaustive]` from the start (see "Unbounded
  topology") — a future `Line` variant for 1-2 site support is additive,
  not a repeat of the 0.2.0→0.3.0 `CdtError` situation.
- No fallible constructor is proposed (`voronoi2` always succeeds — even
  degenerate input just produces an empty `Voronoi2`, matching
  `delaunay2`'s own "degenerate is a valid, representable value" policy),
  so no new error enum is needed for 0.5.0, and therefore no
  `#[non_exhaustive]` question arises yet for this feature specifically.
  If a Voronoi-specific internal validator (analogous to `TopologyError`)
  is added later for testing, it should follow the same
  `#[doc(hidden)]` + `#[non_exhaustive]` pattern `TopologyError` already
  established, for consistency, not because this design requires it now.
- `VoronoiCellId`/`VoronoiEdgeId`/`VoronoiVertexId` are opaque
  (`pub(super)` inner field, same as the existing three id types) — no
  compatibility surface beyond their existence and the accessor methods
  above.

## Assumptions to prove or test before implementation

1. **Cocircular-group transitivity** (the "3 points determine a circle"
   argument above) — proven here, but should additionally be exercised by
   a property test with a genuinely large (5+) cocircular cluster, not
   just the minimal 4-point case, before being trusted in the actual
   implementation.
2. **`incircle` self-consistency** — the same deterministic exact
   predicate, called on the same coordinates during construction as
   during any later re-verification, must agree with itself. Expected to
   be trivially true (no floating-point-adjacent subtlety here — this
   isn't comparing two *different* representations of the same
   quantity), but named explicitly so a future reader doesn't wonder
   about it.
3. **Deterministic `VoronoiVertexId`/`VoronoiEdgeId` assignment order** —
   must be a function of the `Triangulation2`'s own structure (site
   identity) alone, not of union-find's internal representative-selection
   or any `HashMap`-iteration order. Construction algorithm steps 3/5/6
   above name the specific discipline (canonical site-set/cell-pair sort
   keys; never iterate a `HashMap`) — worth a dedicated property test using the same
   `assemble_triangulation`-based direct-construction technique as the
   diagonal-independence acceptance test below (two differently-built
   `Triangulation2`s that are topologically equivalent but not
   byte-identical in construction order) to confirm the id assignment
   itself doesn't leak a construction-order artifact.
4. **The all-cocircular "fan" case collapses to exactly one vertex, zero
   bounded edges** — stated as a degenerate-input row above; should be a
   concrete test, not just an argued property.
5. **Hull-collinear-point cells resolve `is_unbounded` correctly without
   special-casing** — argued above from the existing `boundary_edges()`
   definition; should be a concrete test (a point exactly on a hull edge,
   confirm its cell reports `is_unbounded() == true`).

## Acceptance tests (0.5.0)

- Empty input, 1 point, 2 points, all-collinear input → `Voronoi2` with 0
  cells/edges/vertices in every case (matches the degenerate-input table).
- A simple non-degenerate configuration (e.g. 4 points, no cocircularity)
  → cell count == vertex count of the `Triangulation2`; edge count ==
  interior edge count of the `Triangulation2`; every hull edge maps to an
  `Unbounded` Voronoi edge and every interior edge to a `Bounded` one.
- A single cocircular quad (4 points exactly on one circle) → exactly one
  `VoronoiVertexId` regardless of which diagonal was used. **Important
  test-construction note**: `delaunay2` canonically sorts before
  insertion — its own doc comment and
  `cocircular_square_plus_center_is_stable_across_permutations` already
  prove the result is a deterministic function of the input *set*, not
  order, so simply reordering the caller's array can **never** exercise
  both diagonals of the same quad through `delaunay2` itself. Actually
  testing diagonal-independence requires directly constructing both valid
  triangulations of the same 4-point cocircular quad (e.g. via
  `assemble_triangulation`, reachable from `voronoi.rs` as a sibling
  `triangulation::*` module) and confirming both produce identical
  `Voronoi2` topology despite differing in which Delaunay diagonal was
  used.
- A larger (5+) cocircular cluster → exactly one `VoronoiVertexId`, not
  one per triangle (the transitivity property test named above) — same
  direct-construction note applies if exercising more than one valid
  triangulation of the cluster.
- The maximal all-cocircular "fan" case (every input point on one common
  circle) → exactly one `VoronoiVertexId`, zero `Bounded` edges, every
  hull edge `Unbounded`.
- A point exactly on a hull edge (collinear with two hull vertices,
  overall set not all-collinear) → that vertex's cell reports
  `is_unbounded() == true`.
- Regression guard, not a new property: `Voronoi2`'s own construction
  pass must not *add* order-dependence beyond what `delaunay2` already
  eliminates — same point set through `delaunay2` twice with different
  input orderings (already guaranteed identical `Triangulation2` output)
  should of course still produce identical `Voronoi2`; this mainly guards
  against an implementation accidentally introducing `HashMap`-iteration-order
  dependence in the grouping pass (see "Assumptions to prove or test" #3),
  not against `delaunay2` itself misbehaving.
- Every `VoronoiCellId` round-trips through `cell_site`/back to a real
  `Triangulation2` vertex; every `VoronoiEdgeId` round-trips through
  `dual_delaunay_edge`/`edge_kind`/`edge_cells` consistently with the
  underlying `Triangulation2`'s own `edge_vertices`/`adjacent_faces`;
  every `VoronoiVertexId`'s `delaunay_faces` are mutually cocircular
  (or the group is a single face) and pairwise Delaunay-adjacent
  (connected, not just individually cocircular with some common
  reference — i.e. actually re-derive the connected-component property
  from the stored group, not just trust construction).

### Canonical-topology property tests (most important — flagged by review)

The tests above check individual invariants; these check the actual claim
this ADR is built on — that `Voronoi2` is genuinely independent of *which*
legal Delaunay triangulation of a cocircular point set it was built from,
compared as **canonical representations**, not raw ids (raw
`VoronoiVertexId`/`VoronoiEdgeId` values are only guaranteed equal across
differently-triangulated-but-equal inputs *because* of the canonical-key
assignment above — the comparison should still be done at the canonical
level, both to test that guarantee itself and to keep the test
meaningful even if the canonical-key scheme changes later). Canonical
representation for comparison purposes:

- A cell, as its site: `cell_site(cell)`.
- A Voronoi vertex, as the sorted set of incident sites:
  `{cell_site(c) : c ∈ cells sharing an edge with this vertex}`, or
  equivalently the sorted union of `Triangulation2::face_vertices` over
  `delaunay_faces(vertex)` — matches the same key used for canonical id
  assignment, so this doubles as a direct test of that assignment too.
- A Voronoi edge, as `(sorted [cell_site; 2], kind-shape)` where
  "kind-shape" distinguishes `Bounded`/`Unbounded` but not specific
  `VoronoiVertexId` values (those are only meaningful *within* one
  `Voronoi2`, not across two independently constructed ones being
  compared).

**Constructing multiple legal triangulations of the same cocircular point
set directly** (not through `delaunay2`, which is deterministic and would
only ever produce one — see the diagonal-independence acceptance test
above): build each variant via `assemble_triangulation` from the same
vertex list with different face lists.

- **4 cocircular points**: both of the two possible diagonals.
- **5 cocircular points**: multiple fan triangulations (from each of the 5
  points) and at least one non-fan ("zigzag") triangulation.
- **6-10 cocircular points**: generate a diverse sample by legal edge
  flips. For an input where *every* point is on one shared circle, every
  internal diagonal is flippable without leaving the cocircular-Delaunay
  family — flipping diagonal `(a,c)` of quad `a,b,c,d` to `(b,d)` keeps
  all four points on the same shared circle, so `incircle(...) == Zero`
  continues to hold either way. Start from one triangulation (e.g. a fan)
  and apply a random sequence of legal flips to generate several
  combinatorially different triangulations of the same point set.
- For every variant of the same point set: build `Voronoi2`, canonicalize,
  and assert **all variants produce an identical canonical representation**
  — same cell set, same set of (incident-site-set) Voronoi vertices, same
  set of (site-pair, kind-shape) Voronoi edges.

**Mixed/partial cocircularity** (not everything cocircular at once):

- A larger, otherwise-generic random point cloud with one embedded
  4-point-exactly-cocircular cluster → only that cluster's faces merge;
  every other Voronoi vertex stays a singleton (one Delaunay face each).
- Two separate, independent cocircular clusters within one point cloud
  (different circles, non-overlapping) → each merges independently into
  its own `VoronoiVertexId`; confirm they do **not** merge into each
  other and nothing about one cluster's grouping leaks into the other's.
- **Near-cocircular but not exactly cocircular** input (`incircle != Zero`
  by a small but nonzero margin — e.g. take an exactly-cocircular
  4-point configuration and perturb one coordinate by the smallest
  representable step, `f64::next_up`/`next_down` or equivalent): confirm
  **no merge happens**. This is the negative-case complement to every
  positive cocircular test above, and the one that would catch a real bug
  (an accidentally epsilon-based or otherwise non-exact comparison
  standing in for the strict `Sign::Zero` test) rather than a merely
  incomplete one — the whole design's correctness rests on `incircle`'s
  exactness, and this is the test that would fail first if that were
  ever compromised.

## Recommended additional API (optional for 0.5.0)

```rust
impl Voronoi2 {
    /// A cell's bounding edges, in order: cyclic for a bounded cell,
    /// ray-to-ray for an unbounded one (i.e. starting and ending at its
    /// two Unbounded edges, with every Bounded edge in between in
    /// walk order).
    pub fn cell_edges(&self, cell: VoronoiCellId) -> impl Iterator<Item = VoronoiEdgeId> + '_;
}
```

Ordered (not just unordered-set) cell boundaries are genuinely valuable
for a topology-only API to not read as "just an adjacency graph" — and
should be derivable from the existing Delaunay vertex-star structure
(walking `neighboring_faces`/`face_vertices`'s documented "index `k`
opposite vertex `k`" local-edge convention around a shared vertex).
`Triangulation2` doesn't currently expose an *ordered* vertex-star walk
directly, though (only unordered `edges()`/`neighboring_faces()`
scanning), so implementing this correctly is real, nontrivial work — not
required for 0.5.0's minimum viable API (nothing else in this ADR depends
on it), but valuable enough to record here rather than lose. Add if the
implementation cost turns out low once the rest of this design is coded;
defer past 0.5.0 otherwise, same "don't over-fix ahead of a real need"
discipline as everything else this ADR defers.

## Explicitly out of scope for 0.5.0

Circumcenter/coordinate construction for Voronoi vertices; clipped
Voronoi (bounding to a rectangle or polygon); weighted/power Voronoi;
nearest-neighbor queries; any new runtime dependency; performance
measurement or optimization work of any kind. Also out of scope for this
specific design round: writing `src/` code, `Cargo.toml` changes, a
version bump, `push`, or a release — this ADR is a stopping point for
review, not a plan to execute immediately afterward.

## Module placement

`src/triangulation/voronoi.rs`, sibling to `delaunay2.rs`/`cdt.rs`/
`polygon.rs`/`ids.rs` under the existing `triangulation/` module,
consistent with `docs/architecture.md`'s one-file-per-algorithm layout
(itself stale relative to Phase 6 and worth a documentation pass
independent of this ADR — not addressed here). Re-exported through
`src/triangulation/mod.rs` and `src/lib.rs`, matching the existing
pattern for `CdtError`/`PolygonTriangulationError` and friends.

## Revisit when

- **Circumcenter coordinates are actually needed.** A separate, dedicated
  certified-construction design (ADR-004-style: filter + exact fallback,
  or a lazily-exact representation if chained with future arrangement
  work) — not an extension bolted onto this topology-only structure.
- **The 1-point/2-point/all-collinear gap in "Degenerate input policy"
  becomes a real problem for an actual caller.** The fix would be an
  alternate `Voronoi2` constructor that accepts raw points directly
  (bypassing `Triangulation2::empty()`'s vertex-discarding policy) rather
  than only `voronoi2(triangulation: Triangulation2)` — deliberately not
  designed here, since it's added complexity for what's presently a
  corner case with no known caller need (AGENTS.md §9 Phase 0: don't
  over-fix ahead of a real need).
- **`Triangulation2` ever gains a mutation API.** The "Option A (owned)"
  recommendation's "no staleness risk" argument rests entirely on
  `Triangulation2` being documented as an immutable, static,
  post-construction snapshot (per its own doc comment) — if that ever
  changes, this ADR's ownership section needs re-examination, the same
  way `ADR-006`'s own "ID stability" section already flags an analogous
  future dependency on `Triangulation2` staying immutable.
