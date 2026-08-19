# Kika

**English** | [日本語](README_ja.md) | [简体中文](README_zh.md)

**Kika — Robust Computational Geometry for Rust.** A pure-Rust,
memory-safe alternative to [CGAL](https://www.cgal.org/) for developers who
want robust geometric predicates without CMake, Boost, or a hard GMP/MPFR
dependency.

Kika ("幾何", Japanese for "geometry") is a Rust library building toward
robust 2D/3D computational geometry: exact predicates with adaptive/exact
fallback arithmetic, and — in later phases — triangulation, hull, and
polygon algorithms built on top of that foundation.

Status: **pre-alpha (Phase 1-5 and Phase 6A-6D complete).** As of 0.6.0,
Kika is a robust 2D kernel with exact predicates, 2D convex hull, Delaunay
triangulation, constrained Delaunay triangulation (narrow scope),
simple-polygon triangulation with or without holes, Voronoi diagram
topology (no vertex coordinates yet), and point location — see
[Implemented today](#implemented-today) and the
[Maturity](#maturity) table below for exactly what that does and doesn't
cover. No stability guarantees yet. See [Roadmap](#roadmap) for what
doesn't exist yet — **Kika is not a CGAL replacement**, it is a robust
kernel a future one could be built on.

## Why not just use CGAL?

CGAL is the mature, comprehensive reference implementation for
computational geometry. Kika's plan is to test its predicate layer against
it as an external oracle during development (§10) — **not done yet**: the
comparison program is unbuilt and currently environment-blocked (no
CGAL/pkg-config available in this project's development environment), see
[`docs/compatibility.md`](docs/compatibility.md) and
[`tasks/todo.md`](tasks/todo.md). Today's exactness claims are verified
against independent `num-bigint`/`num-rational` oracles instead (see
[Implemented today](#implemented-today) below), not against CGAL. Pulling
CGAL into a Rust project at all means a C++ toolchain, CMake, Boost, and
usually GMP/MPFR — friction that's real for `cargo build`-only workflows,
WASM targets, and teams that want a pure-Rust dependency tree.

Kika's bet, in order:

| | CGAL | Kika |
|---|---|---|
| Language | C++ | Pure Rust |
| Build | CMake + Boost | `cargo build` |
| Big-number dependency | GMP/MPFR (typically required) | Not required at runtime (dev-only, for test oracles) |
| Memory safety | Manual | Enforced by the compiler (`unsafe` disallowed by default, see `docs/architecture.md`) |
| WASM target | Not practical | `wasm32-unknown-unknown`, CI-checked |
| License | GPL / commercial | MIT OR Apache-2.0 |
| Feature breadth today | Very large, decades mature | Deliberately small — see [Implemented today](#implemented-today) |

If you need mesh Booleans, NURBS, or a CAD kernel today, that's CGAL, not
Kika. If you want a small, robust, panic-free predicate layer to build on
in pure Rust, that's what Phase 1 of Kika is.

## Implemented today

* `Point2`, `Point3` — finite-coordinate points (`f64`). Construction
  validates and rejects NaN/infinity; once constructed, always finite.
  Equality is exact coordinate equality (no tolerance) — see ADR-003.
* `Vector2`, `Vector3` — finite-coordinate displacement vectors, with the
  standard point/vector affine arithmetic (`Point ± Vector -> Point`,
  `Point - Point -> Vector`, vector `+`/`-`/`-` (negate)/`* f64`).
* `Segment2`, `Triangle2`, `Triangle3`, `Aabb2`, `Aabb3` — plain data
  types over the above (no extra validation beyond what `Point2`/`Point3`
  already guarantee); zero-length segments and degenerate
  (collinear-vertex) triangles are valid, representable values, not
  rejected. `Aabb2`/`Aabb3` give an exact, `orient2d`-free `overlaps()`
  fast-reject test.
* `Segment2::relation_to`, `Triangle2::orientation`/`relation_to` —
  exact point-on-segment and point-in-triangle predicates, built entirely
  from `orient2d`. Each degenerate case (zero-length segment, collinear
  triangle) is handled explicitly, not assumed to fall out of the general
  algorithm — one case initially didn't and was caught by testing, see
  `docs/degeneracy-policy.md`. Checked against an independent
  exact-rational oracle in `tests/differential/`.
* `segment_intersection_kind` / `segment_intersection` — robust 2D
  segment intersection, classification and coordinate construction
  deliberately kept as separate functions (§4.2): classification
  (`None`/`Proper`/`EndpointTouch`/`CollinearTouch`/`CollinearOverlap`)
  never divides or builds a new coordinate, with an `Aabb2`-based
  fast-reject ahead of any predicate call. Construction is exact for every
  case, including `Proper` (the one case that needs a genuinely new
  coordinate, correctly rounded as of Phase 5) — see
  [Exact predicates vs. exact constructions](#exact-predicates-vs-exact-constructions).
  Checked against an independent exact-rational oracle in
  `tests/differential/`.
* `Polygon2` — a vertex ring, implicitly closed (no repeated first/last
  vertex). `signed_area()` (plain `f64`, a construction) and
  `orientation()` (exact — sums every edge's shoelace term via the same
  exact-expansion machinery the core predicates use, not a running `f64`
  sum) are kept separate on purpose, same split as everywhere else.
  `basic_validity()` covers the cheap structural checks (vertex count,
  consecutive-duplicate vertices, zero area); `find_self_intersection()`
  is the separate, O(n²) check across non-adjacent edges (adjacent edges'
  shared vertex is correctly never reported as a self-intersection).
* `Sign`, `Orientation` — meaningful enums returned by predicates (never a
  raw determinant, never an ambiguous `bool`).
* `orient2d` — exact-sign 2D orientation predicate. Uses a fast
  floating-point filter with a *computed* error bound (never a fixed
  epsilon) and falls back to exact expansion-arithmetic evaluation when
  the filter is inconclusive. Checked against an independent exact-rational
  oracle in `tests/differential/`. See
  [`docs/numerical-model.md`](docs/numerical-model.md).
* `orient3d` — exact-sign tetrahedron-orientation predicate. Same
  filter + exact-fallback design as `orient2d`. Checked against an
  independent exact-rational oracle in `tests/differential/`.
* `incircle` — exact-sign point-in-circumcircle predicate. Same
  filter + exact-fallback design, with a narrower verified-safe
  coordinate-magnitude range (`~1e-70`..`~1e70`) than `orient2d`/
  `orient3d` due to its higher polynomial degree — see
  [`docs/numerical-model.md`](docs/numerical-model.md). Checked against an
  independent exact-rational oracle in `tests/differential/`.
* `insphere` — exact-sign point-in-circumsphere predicate, the 3D analog
  of `incircle`. Same filter + exact-fallback design, with an even
  narrower verified-safe coordinate-magnitude range (`~1e-30`..`~1e30`)
  than `incircle` — see [`docs/numerical-model.md`](docs/numerical-model.md).
  Checked against an independent exact-rational oracle in
  `tests/differential/`.

* `convex_hull2` / `HullBoundaryPoints` — 2D convex hull via Andrew's
  monotone chain. `ExtremesOnly` (default) keeps only strict corners;
  `KeepAllOnBoundary` also keeps boundary points collinear with their
  neighbors. Counterclockwise output, starting at the lexicographically
  smallest input point, independent of input order; duplicate input points
  are collapsed first. Fully exact — every returned vertex is copied from
  an original input `Point2`, since the algorithm is built entirely from
  `orient2d` with no interpolation or division anywhere. Degenerate inputs
  (0/1/2 points, all-collinear) are handled explicitly, not left to the
  general algorithm — see
  [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md).

* `delaunay2` / `Triangulation2` — 2D Delaunay triangulation via
  Bowyer-Watson incremental insertion. Fully exact, like `convex_hull2`:
  "outside the triangulation" is represented by a single symbolic ghost
  vertex (no coordinate), not a synthetic bounding triangle, so there is
  no scale-dependent tradeoff to work around — verified down to a
  perpendicular cluster spread of `1e-200` relative to a span of `10.0`.
  Cocircular points are a real tie among multiple valid triangulations,
  not a single "correct" answer; the deterministic tie-break rule is
  documented in [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)
  alongside every other degenerate case (collinear boundary points, points
  exactly on an existing edge).
* `Triangulation2`'s adjacency structure — `VertexId`/`EdgeId`/`FaceId`
  and `vertices`/`edges`/`faces`/`edge_vertices`/`adjacent_faces`/
  `face_vertices`/`neighboring_faces`/`boundary_edges`, a **static,
  post-construction snapshot** of the indexed-triangle-adjacency structure
  (no half-edge/quad-edge generality, per ADR-006's comparison).
  `triangles()` keeps its original coordinate-only contract unchanged; the
  new methods are purely additive.
* `constrained_delaunay2` / `ConstrainedTriangulation2` — 2D constrained
  Delaunay triangulation, deliberately narrow scope (Phase 6C): only
  non-crossing constraint edges between *existing* input vertices, no
  automatic intersection/Steiner-point generation, no refinement. Built
  entirely by flipping existing Delaunay edges via the crate's own
  `orient2d`/`incircle`/`segment_intersection_kind` predicates — ADR-004's
  Phase 6 re-evaluation predicted CDT needs **no new construction**, and
  the implementation confirms it: not one new coordinate is ever built.
  Constraint recovery and Delaunay restoration are each bounded (never an
  unbounded loop); `CdtError` reports crossing/collinear constraints,
  algorithm exhaustion, and a degenerate point set (fewer than 3 points, or
  all collinear) as typed errors, never a panic.
* `triangulate_polygon` — simple-polygon triangulation (Phase 6D), built
  on Phase 6C's CDT: constrain every polygon edge, then discard the
  concave-pocket faces outside the polygon (for a non-convex input) via a
  purely topological flood fill from one interior seed face — never a
  constructed point such as a centroid. No Steiner points (every output
  vertex is one of the polygon's own), self-intersecting input rejected
  as a typed `PolygonTriangulationError`, both CCW and CW input accepted,
  deterministic. See [`docs/degeneracy-policy.md`](docs/degeneracy-policy.md)
  for the full scope table, including a caveat for checking the result
  with `Triangulation2::validate_topology()` (its Euler-characteristic
  check assumes full convex-hull coverage, which a non-convex polygon's
  triangulation deliberately doesn't have).
* `triangulate_polygon_with_holes` — polygon triangulation with holes,
  generalizing `triangulate_polygon`'s own algorithm rather than a new
  one: a hole's boundary is just more constrained edges the same flood
  fill stops at. A hole nested inside another hole is out of scope
  (typed error, not partial support); every other rejected input (a hole
  outside the boundary, touching or crossing it, or touching/crossing
  another hole) is likewise a typed `PolygonTriangulationError`, never a
  panic. `Polygon2::relation_to`/`PointPolygonRelation` (an exact
  point-in-polygon predicate, new alongside this) backs the hole-containment
  check.
* `Voronoi2` / `voronoi2` — a topology-only Voronoi diagram (0.5.0), the
  dual of an existing `Triangulation2`: no vertex coordinates
  (circumcenters), clipping, or nearest-neighbor query yet, deliberately
  deferred. Delaunay's own cocircular tie-break (above) can split a
  cocircular point cluster across more than one triangle; `voronoi2`
  merges the affected faces via union-find keyed on
  `incircle(...) == Sign::Zero` so that arbitrary choice never leaks out
  as a spurious extra Voronoi vertex or edge — verified by feeding the
  *same* cocircular point set through multiple different triangulations
  and checking for identical, not merely isomorphic, output. Query API:
  `cells`/`vertices`/`edges`, `cell_site`, `neighboring_cells`,
  `cell_is_unbounded`, `edge_cells`, `edge_kind`, `dual_delaunay_edge`,
  `vertex_delaunay_faces`, and `cell_edges` — an ordered counterclockwise
  walk of a cell's boundary (closed cycle for a bounded/interior-site
  cell, a linear sequence between two rays for an unbounded/hull-site
  cell), built entirely from `Triangulation2`'s existing face adjacency,
  no new data model. See
  [`docs/adr/ADR-007-voronoi-diagram-topology.md`](docs/adr/ADR-007-voronoi-diagram-topology.md).
* `Triangulation2::locate` / `PointLocation` — point location (0.6.0):
  `PointLocation::{Vertex(VertexId), Edge(EdgeId), Face(FaceId), Outside}`,
  a closed enum (not `#[non_exhaustive]` — its 4 variants are exactly the
  closure of `Triangulation2`'s own already-closed id vocabulary plus the
  necessary miss case). `Outside` means "not covered by any face," not
  "outside the convex hull": a point inside a
  `triangulate_polygon_with_holes` hole is also `Outside`. `O(F)` — a
  linear scan over every face, not a spatial index; performance is
  deliberately not part of this release's contract, so a faster locator
  can replace the scan later without the signature changing. Never
  panics, including on an empty triangulation. Verified against an
  independent BigRational oracle covering the actual aggregation/dispatch
  logic across faces, not just this crate's own
  `Triangle2::relation_to`/`Segment2::relation_to`. See
  [`docs/adr/ADR-008-point-location.md`](docs/adr/ADR-008-point-location.md).

All four predicates complete v0.1's robust-predicate scope; the primitives,
intersections, polygon, convex hull, and Delaunay triangulation above
complete Phases 2 through 4. `segment_intersection`'s `Proper`-crossing
point construction (below) completes Phase 5, and the adjacency structure,
constrained Delaunay triangulation, and simple-polygon triangulation above
complete Phase 6A-6D. Voronoi diagram *topology* (0.5.0) and point
location (0.6.0) are both implemented (above); everything past this
point — polygon Boolean, a spatial index/walking locator,
nearest-neighbor query, and Voronoi vertex *coordinates* (circumcenters)
specifically — is later, see [Roadmap](#roadmap).

* `predicates::line_intersection` (used internally by
  `segment_intersection`'s `Proper` case) — the first exact/certified
  **construction** in the crate, per ADR-004. Returns the correctly-rounded
  (round-to-nearest-even on exact ties) `f64` nearest to the true
  intersection coordinate, not an approximation — the same guarantee
  IEEE-754 makes for a single arithmetic operation, extended to a whole
  geometric construction. `Point2` stays a plain `f64` pair; no new public
  type, no new dependency. Verified against an independent `BigRational`
  "is this the correctly-rounded nearest `f64`" oracle across magnitude
  scales, mixed-magnitude inputs, and an empirical floor sweep — see
  [`docs/numerical-model.md`](docs/numerical-model.md).

## Exact predicates vs. exact constructions

Kika's predicates (`orient2d` etc.) guarantee a mathematically correct
**sign**. Guaranteeing that a *generated coordinate* is exact is a
separate problem ("construction") — as of Phase 5, one case is solved:
`segment_intersection`'s `Proper`-crossing point is now a correctly-rounded
construction (`predicates::line_intersection`, ADR-004), unlike the naive
`f64` interpolation it used to be. It joins the other cases that function
can return (`EndpointTouch`/`CollinearTouch`/`CollinearOverlap`), which
were already exact by reusing an original input coordinate directly. See
[`docs/architecture.md`](docs/architecture.md) §4.2 and ADR-004. Do not
assume constructions implemented in later phases (Phase 6: constrained
Delaunay, polygon Boolean) carry the same exactness guarantee until their
own docs say so.

## Degenerate cases

Collinear/coplanar/cocircular/cospherical points, duplicate points, signed
zero, and subnormal coordinates are handled explicitly and tested, not
treated as "rare enough to ignore." See
[`docs/degeneracy-policy.md`](docs/degeneracy-policy.md).

## Minimal example

```rust
use kika::{Point2, orient2d, Orientation};

let a = Point2::new(0.0, 0.0).unwrap();
let b = Point2::new(1.0, 0.0).unwrap();
let c = Point2::new(0.0, 1.0).unwrap();

assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
```

Constrained Delaunay — every constraint edge is guaranteed present in the
result, even where flipping it away would otherwise be the Delaunay choice
(this exact snippet is doctested — `cargo test --doc` — as
[`constrained_delaunay2`'s own doc example](src/triangulation/cdt.rs)):

```rust
use kika::{Point2, constrained_delaunay2};

let pts = [
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(4.0, 4.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
];
let constraints = [(0, 2)]; // one diagonal of the square
let cdt = constrained_delaunay2(&pts, &constraints).unwrap();

let constrained_edge_count = cdt
    .triangulation()
    .edges()
    .filter(|&e| cdt.is_constrained(e))
    .count();
assert_eq!(constrained_edge_count, constraints.len());
```

Simple-polygon triangulation — no Steiner points, built on the
constrained Delaunay above (also doctested, as
[`triangulate_polygon`'s own doc example](src/triangulation/polygon.rs)).
`triangulate_polygon` covers a single boundary ring (no holes);
`triangulate_polygon_with_holes` extends the same algorithm to a boundary
plus zero or more hole rings (still no Steiner points, no new
construction):

```rust
use kika::{Point2, Polygon2, triangulate_polygon};

let square = Polygon2::new(vec![
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(4.0, 4.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
]);
let t = triangulate_polygon(&square).unwrap();

// A simple polygon triangulated with only its own vertices always has
// exactly `polygon.len() - 2` triangles.
assert_eq!(t.len(), square.len() - 2);
```

Voronoi diagram topology — the dual of a `Triangulation2`, no vertex
coordinates (also doctested, as
[`voronoi2`'s own doc example](src/triangulation/voronoi.rs)):

```rust
use kika::{Point2, VoronoiEdgeKind, delaunay2, voronoi2};

let pts = [
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
];
let voronoi = voronoi2(delaunay2(&pts));

// One cell per site, one Voronoi vertex (the triangle's circumcenter),
// and 3 unbounded rays -- no interior Delaunay edge to exclude.
assert_eq!(voronoi.cells().count(), 3);
assert_eq!(voronoi.vertices().count(), 1);
for edge in voronoi.edges() {
    assert!(matches!(
        voronoi.edge_kind(edge),
        VoronoiEdgeKind::Unbounded { .. }
    ));
}
```

Point location — `O(F)`, no spatial index (also doctested, as
[`locate`'s own doc example](src/triangulation/locate.rs)):

```rust
use kika::{Point2, PointLocation, delaunay2};

let pts = [
    Point2::new(0.0, 0.0).unwrap(),
    Point2::new(4.0, 0.0).unwrap(),
    Point2::new(0.0, 4.0).unwrap(),
];
let t = delaunay2(&pts);

// Every input point locates to its own vertex.
let (v0, _) = t.vertices().next().unwrap();
assert_eq!(t.locate(pts[0]), PointLocation::Vertex(v0));

// A point strictly inside the triangle locates to its one face.
assert!(matches!(
    t.locate(Point2::new(1.0, 1.0).unwrap()),
    PointLocation::Face(_)
));

// A point outside the hull.
assert_eq!(t.locate(Point2::new(10.0, 10.0).unwrap()), PointLocation::Outside);
```

More, runnable via `cargo run --example <name>`, in [`examples/`](examples/):

* [`orient2d`](examples/orient2d.rs) — the basic turn predicate
* [`segment_intersection`](examples/segment_intersection.rs) — classify and
  construct a segment crossing
* [`convex_hull`](examples/convex_hull.rs) — `ExtremesOnly` vs
  `KeepAllOnBoundary`
* [`delaunay`](examples/delaunay.rs) — 2D Delaunay triangulation
* [`polygon_validity`](examples/polygon_validity.rs) — `basic_validity` and
  `find_self_intersection`
* [`constrained_delaunay`](examples/constrained_delaunay.rs) — forcing a
  specific (possibly non-Delaunay) edge to survive
* [`polygon_triangulation`](examples/polygon_triangulation.rs) — a
  non-convex polygon, with triangle-count/CCW/area checks
* [`polygon_triangulation_with_holes`](examples/polygon_triangulation_with_holes.rs) —
  a boundary with two separate holes cut out
* [`voronoi`](examples/voronoi.rs) — a cocircular square plus an
  off-center interior point, bounded vs. unbounded cells
* [`locate`](examples/locate.rs) — vertex/edge/face/outside
  classification, including a hole's interior vs. its boundary

## WASM

The predicate core has no OS or platform-specific code and builds for
`wasm32-unknown-unknown`; this is checked in CI. No WASM-specific bindings
(`wasm-bindgen` etc.) exist yet.

## Difference from CGAL

Kika does not link CGAL and shares no source with it. CGAL is *planned* to
be used only as an external, separate differential-test oracle during
development (§10 of the project's development instructions) — never as a
runtime or build dependency of the `kika` crate — but that comparison
program does not exist yet; see [Why not just use CGAL?](#why-not-just-use-cgal)
above.

## Stability

Pre-1.0, no semver guarantees. The public `Kernel` trait design described
in some computational-geometry libraries (CGAL included) is explicitly not
being finalized yet — see ADR-004. As of 0.3.0, the public `Result`-style
error enums (`KikaError`, `CdtError`, `PolygonTriangulationError`) are
`#[non_exhaustive]`, so a future variant addition won't break a downstream
`match` that already has a wildcard arm — see `CHANGELOG.md`.

## Maturity

| Feature | Status |
|---|---|
| Predicates (`orient2d`, `orient3d`, `incircle`, `insphere`) | Stable enough for evaluation — filter + exact fallback, checked against independent oracles |
| Segment intersection | Implemented — classification exact, `Proper` construction correctly rounded (ADR-004) |
| Convex hull | Implemented — fully exact |
| Delaunay triangulation | Implemented — fully exact, no synthetic coordinates |
| Triangulation adjacency (vertex/edge/face queries) | Implemented — `VertexId`/`EdgeId`/`FaceId`, neighbor/boundary queries, internal topology validator (ADR-006) |
| Constrained Delaunay | Implemented — narrow scope: non-crossing constraints between existing vertices only, no Steiner points (Phase 6C) |
| Simple polygon triangulation | Implemented — no Steiner points, self-intersecting input rejected (Phase 6D); holes supported (0.4.0, `triangulate_polygon_with_holes`) — nested holes out of scope, typed error |
| Voronoi diagram | Implemented — topology only (0.5.0): cells/vertices/edges, ordered `cell_edges()` boundary walk; no vertex coordinates (circumcenters), clipping, or nearest-neighbor query yet |
| Point location | Implemented — `Triangulation2::locate` (0.6.0), `O(F)` linear scan, verified against an independent BigRational oracle; no spatial index/walking locator or nearest-neighbor query yet |
| Polygon Boolean | Not implemented — exactness model still open, see ADR-004 |
| 3D mesh operations | Not implemented |

## License

Licensed under either of

* MIT license ([LICENSE-MIT](LICENSE-MIT))
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Roadmap

Phase 1 (robust predicates), Phase 2 (2D primitives and intersections),
Phase 3 (2D convex hull), Phase 4 (2D Delaunay triangulation), Phase 5
(certified/exact constructions — an exact `Proper` segment-intersection
point), Phase 6A-6D (triangulation adjacency, narrow-scope constrained
Delaunay, narrow-scope simple-polygon triangulation), Voronoi diagram
*topology* (0.5.0), and point location (0.6.0) are complete. Not yet
implemented: Voronoi vertex *coordinates* (circumcenters), clipping, and
nearest-neighbor query; a spatial index/walking locator for `locate`;
polygon/mesh Boolean; vertex deletion; Delaunay refinement; mesh repair;
surface reconstruction; point-cloud processing. See
[`tasks/todo.md`](tasks/todo.md) for the phased backlog and
[`docs/release-checklist.md`](docs/release-checklist.md) for what's
verified before each `crates.io`/GitHub release (0.2.0 through 0.5.0 have
all shipped, 0.6.0 in preparation; see `CHANGELOG.md`).
