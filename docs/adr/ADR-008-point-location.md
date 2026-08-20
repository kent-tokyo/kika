# ADR-008: Point location (`Triangulation2::locate`) for 0.6.0

Status: Decided and implemented, shipped in 0.6.0 (2026-08-20, `v0.6.0`
tag at `db6d04c`). `ROADMAP.md` (internal, gitignored) already specified
0.6.0 as "Spatial query API," centered on point location, with the
target signature this ADR implements unchanged. No new dependency, no
walking-locator optimization, no nearest-neighbor query was done under
this ADR — see "Explicitly out of scope" below.

## Context

`Triangulation2` (Delaunay), `ConstrainedTriangulation2` (via its
`triangulation()` accessor), and `Voronoi2` (whose cells are dual to
`Triangulation2`'s vertices) can all be *built*, but a caller has no way
to ask "which face/edge/vertex of this triangulation contains point
`p`?" without re-deriving it by hand from `faces()`/`face_vertices()`
and the primitive predicates directly. `ROADMAP.md` already scopes this
precisely:

```rust
pub enum PointLocation {
    Vertex(VertexId),
    Edge(EdgeId),
    Face(FaceId),
    Outside,
}
impl Triangulation2 {
    pub fn locate(&self, point: Point2) -> PointLocation;
}
```

with an explicit performance note: a linear-scan-over-faces
implementation is fine for 0.6.0 — performance is not part of the public
contract, and this signature doesn't need to change later to swap in a
walking locator over the existing adjacency. Nearest-neighbor query is
explicitly excluded from this release even with spare capacity.

Unlike 0.5.0's Voronoi topology work, this problem has no competing
structural approaches to weigh (no quad-edge-vs-indexed-adjacency-style
decision, no cocircular-tie-break-style normalization problem) — it is
one correct algorithm assembled from already-exact, already-existing
primitives (`Triangle2::relation_to`, `Segment2::relation_to`, both
built on `orient2d`). This ADR is correspondingly shorter than ADR-007;
its job is to record the disambiguation algorithm's correctness
argument and the degenerate-case table, not to weigh alternatives.

## Design

### Algorithm

`Triangulation2::faces()` and `Triangulation2::triangles()` are
index-parallel by construction (`assemble_triangulation` builds
`triangles` by iterating `faces` in order; pinned by the existing test
`face_vertices_matches_triangles_coordinates`). `locate` zips them
directly — no coordinate lookup table, no `VertexId`-keyed map:

```rust
for (face, tri) in self.faces().zip(self.triangles()) {
    match tri.relation_to(point) {
        PointTriangleRelation::Inside => return PointLocation::Face(face),
        PointTriangleRelation::Outside => continue,
        PointTriangleRelation::OnBoundary => {
            // Disambiguate vertex vs. edge via Segment2::relation_to on
            // the face's 3 actual edges (see "OnBoundary implies a
            // bounded edge" below for why this always finds a match for
            // a non-degenerate CCW face) -- never a distance comparison,
            // see "Rejected alternative" below.
            // ... returns Vertex(id) or Edge(id) ...
        }
    }
}
PointLocation::Outside
```

The `EdgeId`-by-vertex-pair lookup (given a `Segment2::relation_to`
`Interior` match, find the crate-wide `EdgeId` for that vertex pair)
reuses an existing pattern verbatim:
`src/triangulation/polygon.rs`'s `cdt.triangulation().edges().find(|&e|
{ let (a,b) = ...edge_vertices(e); (a==u&&b==v)||(a==v&&b==u) })`. Not a
new technique.

**Never panics.** The two spots that might look like `unreachable!()`/
`.expect()` candidates — no edge matched inside the `OnBoundary` arm, or
the vertex-pair scan finding no `EdgeId` — both `continue` to the next
face (falling through to `Outside` if nothing else matches) rather than
panic. `Triangulation2::validate_topology()` is a test-only diagnostic
(every call site across the whole crate is inside `#[cfg(test)]`), never
a construction-time gate — "every face is a proper CCW triangle" is not
an invariant a public query method should be allowed to panic on if it
is ever violated by a future bug.

### OnBoundary implies a bounded edge (why there's no wrong-edge risk)

For a non-degenerate CCW triangle, `Triangle2::relation_to` returning
`OnBoundary` can only be produced by a point genuinely on one of the 3
*bounded* edges (including its endpoints) — never on an edge's line
extended past its actual segment. A point on edge AB's line but beyond
the segment necessarily falls on opposite sides of the *other* two
edges' lines (a triangle's 3 edges only meet at its 3 vertices), which
forces both a clockwise and a counterclockwise result among the 3
`orient2d` calls `relation_to` combines — and that combination is
exactly what makes it return `Outside`, not `OnBoundary`. So a
`Segment2::relation_to` pass over the face's 3 actual (bounded) edges is
guaranteed to find a match whenever `Triangle2::relation_to` already
said `OnBoundary`, for any valid face.

**Rejected alternative**: resolving vertex-vs-edge via nearest-distance
comparison to the 3 vertices. This crate has no exact distance predicate
— only `orient2d`/`incircle` — so squared-distance comparison on `f64`
coordinates rounds, reintroducing exactly the tolerance-shaped bug this
crate's entire design (adaptive-precision exact predicates, zero
epsilon comparisons anywhere) exists to avoid. `Segment2::relation_to`
is exact; distance comparison would not be.

### `PointLocation` is a closed enum, not `#[non_exhaustive]`

Its 4 variants are exactly the closure of `Triangulation2`'s own
already-closed id vocabulary (`VertexId | EdgeId | FaceId`, from
`src/triangulation/ids.rs`) plus the necessary "miss" case. A
2-simplicial complex has only 0-cells, 1-cells, and 2-cells — there is
no fourth topological kind a triangulation could ever grow, so no future
variant is conceivable. Matches `PointTriangleRelation`/
`PointSegmentRelation`'s own precedent (both closed enums), unlike
`VoronoiEdgeKind` (`#[non_exhaustive]`, because Voronoi diagrams
genuinely need a topological kind — an unbounded ray — that degenerate
1-2-site input can't express with the vocabulary that exists today).
This is the same "closed by mathematical necessity forever, vs. closed
only under the current problem scope" test ADR-007 established for
`VoronoiEdgeKind`, applied here and landing on the opposite answer for a
principled reason, not by default.

### `Outside` means "not covered by any face," not "outside the convex hull"

For a `triangulate_polygon_with_holes` result, a point geometrically
inside the outer ring but inside a hole correctly returns `Outside` — no
face covers it, since hole interiors are flood-filled away from the
underlying CDT. A point exactly on a hole's boundary correctly resolves
to `Edge` (a real `boundary_edges()` member with exactly one incident
face, same as any other hull-style boundary edge). Documented explicitly
on `locate` itself so a caller doesn't assume `Outside` implies
hull-exterior.

### The result does not depend on face iteration order

This is the property the ROADMAP's "swap in a walking locator later, the
signature doesn't need to change" note relies on, and it's worth stating
why rather than leaving it as an unstated assumption a future
"reorder/optimize the scan" change could silently break:

- `Inside` triangle interiors are pairwise disjoint by construction (a
  valid triangulation never has overlapping faces).
- A point on a shared interior edge makes *both* incident faces report
  `OnBoundary`, and both resolve to the same `EdgeId` regardless of scan
  order, since `edges` is deduplicated by canonical vertex pair in
  `assemble_triangulation` — there is only one `EdgeId` for that pair to
  find.
- A point at a shared vertex resolves to the same `VertexId` regardless
  of which incident face or edge is examined first, since no
  `Triangulation2` ever contains duplicate-coordinate vertices:
  `delaunay2()` dedupes via `dedup_sorted` before construction, and
  `constrained_delaunay2()` rejects duplicate points outright as a typed
  `CdtError`.

### No forwarding method on `ConstrainedTriangulation2`

`cdt.triangulation().locate(p)` already works today via the existing
`pub fn triangulation(&self) -> &Triangulation2` accessor. `ROADMAP.md`
specs only `Triangulation2::locate`; a forwarding method on
`ConstrainedTriangulation2` would be an unrequested convenience wrapper
not asked for by the roadmap or any known caller. The pattern is shown
in `locate`'s own doctest instead.

## Degenerate cases

| Case | `PointLocation` |
|---|---|
| Point exactly equals a vertex's coordinate | `Vertex(that id)` |
| Point on a bounded edge (hull or interior), not at either endpoint | `Edge(that id)` |
| Point strictly inside exactly one face | `Face(that id)` |
| Point outside the triangulated domain (hull-exterior, or inside a `triangulate_polygon_with_holes` hole) | `Outside` |
| Empty `Triangulation2` (fewer than 3 points, or all input exactly collinear) | `Outside` for any query point, never a panic. Inherited, not a new decision: `Triangulation2::empty()` wipes `vertices` too, so even a query point exactly equal to one of the degenerate input's own collinear points still returns `Outside`. |
| 3+ collinear points forming part of the convex hull (a "flat" middle point) | Resolves like any other vertex/edge — no special-casing needed or present, worth an explicit test given Voronoi's own precedent (ADR-007) of this being worth checking directly rather than assumed fine. |

## Explicitly out of scope for this round

Walking locator or any other performance optimization (`ROADMAP.md` is
explicit: "performance isn't part of the public contract"),
nearest-neighbor query (`ROADMAP.md` is explicit: don't add this even
with spare capacity), a `ConstrainedTriangulation2`-specific forwarding
method, new dependencies, version bump, dated `CHANGELOG.md` entry,
release preparation, push, publish, tag, GitHub Release. Voronoi vertex
coordinates/clipping/nearest-neighbor (0.5.0's own deferred scope) and
Polygon Boolean (a later release) are unrelated to this ADR.
