# Architecture

Status: reflects Phase 1 through Phase 6D, plus 0.3.0 (robustness) and
0.4.0 (polygon triangulation with holes) — current as of 0.4.0, the
latest shipped release. Updated per-phase/per-release, not written ahead
of the code it describes (this file fell behind that discipline for a
while — it described only through Phase 4 despite Phase 6D and two
releases already having shipped — caught and corrected here; see
`tasks/lessons.md` if a similar staleness turns up again).

## Crate layout

Single crate (`kika`), per AGENTS.md §6. Split into crates only if
compile-time, optional-dependency boundaries, WASM packaging, API
stability, or reuse actually require it (§6) — not preemptively.

```text
src/
├── lib.rs                    # public re-exports only
├── error.rs                   # KikaError
├── primitives/                # Point2/3, Vector2/3, Segment2, Triangle2/3, Aabb2/3
│                               # + point-on-segment, point-in-triangle (query methods)
│                               # segment2.rs also exposes point_in_collinear_range
│                               # pub(crate) -- shared with intersections::segment2
├── predicates/
│   ├── expansion.rs            # exact arithmetic core: two_sum, split, two_product,
│   │                           # expansion_sum, scale_expansion, product_of_expansions;
│   │                           # also the shared det3_with_precancel_bound/det3_exact/
│   │                           # negate cofactor helpers (hoisted here in 0.3.0 --
│   │                           # previously ~130 duplicated lines across orient3d/
│   │                           # incircle/insphere/line_intersection)
│   ├── sign.rs                  # Sign, Orientation enums
│   ├── orient2d.rs
│   ├── orient3d.rs
│   ├── incircle.rs
│   ├── insphere.rs
│   ├── polygon2.rs               # polygon_orientation (pub(crate); backs Polygon2::orientation)
│   └── constructions/
│       └── line_intersection.rs   # Phase 5: the crate's first exact/certified
│                                   # *construction* -- correctly_rounded_divide,
│                                   # plus 0.3.0's exact power-of-two rescaling fix
│                                   # for extreme/mixed-magnitude overflow
├── intersections/
│   └── segment2.rs               # segment_intersection_kind (predicate) /
│                                  # segment_intersection (construction), split per §4.2
├── polygon/
│   └── polygon2.rs                # Polygon2: signed_area (f64), orientation (exact),
│                                   # basic_validity, find_self_intersection, and
│                                   # (0.4.0) relation_to/PointPolygonRelation --
│                                   # exact point-in-polygon, backs hole containment
├── hull/
│   └── convex_hull2.rs             # convex_hull2: Andrew monotone chain, built
│                                    # entirely from orient2d — no new coordinates
│                                    # constructed, so the whole algorithm is exact
└── triangulation/
    ├── ids.rs                       # VertexId/EdgeId/FaceId (Phase 6B) -- opaque,
    │                                 # pub(super)-field newtypes indexing
    │                                 # Triangulation2's parallel arrays
    ├── delaunay2.rs                  # delaunay2/Triangulation2: Bowyer-Watson
    │                                 # incremental insertion (Phase 4), plus the
    │                                 # indexed-triangle adjacency structure and
    │                                 # TopologyError validator (Phase 6B, ADR-006)
    ├── cdt.rs                        # constrained_delaunay2/ConstrainedTriangulation2/
    │                                 # CdtError (Phase 6C) -- segment recovery via
    │                                 # pure edge-flipping, no new construction
    ├── polygon.rs                    # triangulate_polygon (Phase 6D) and
    │                                  # triangulate_polygon_with_holes (0.4.0) /
    │                                  # PolygonTriangulationError
    ├── voronoi.rs                    # Voronoi2/voronoi2 (0.5.0, ADR-007) --
    │                                  # topology-only dual of Triangulation2:
    │                                  # cocircular face grouping via union-find,
    │                                  # canonical dense id assignment, the
    │                                  # cells/vertices/edges query API, and the
    │                                  # ordered cell_edges() boundary walk. No
    │                                  # coordinates (circumcenter), clipping, or
    │                                  # nearest-neighbor yet -- deliberately
    │                                  # deferred, see the ADR
    └── locate.rs                     # Triangulation2::locate/PointLocation
                                       # (0.6.0, ADR-008) -- O(F) linear scan over
                                       # faces()/triangles() (index-parallel by
                                       # construction), Segment2::relation_to to
                                       # disambiguate an OnBoundary hit into
                                       # Vertex/Edge. No spatial index or
                                       # nearest-neighbor yet -- deliberately
                                       # deferred, see the ADR
```

`docs/adr/ADR-007-voronoi-diagram-topology.md` designed
`triangulation::voronoi` (above); implementation shipped in full across
3 phases (7A: data model + constructor + validator; 7B: public query
API; 7C: `cell_edges()`), each phase's own correctness argument recorded
in the module's doc comments rather than repeated here.

`docs/adr/ADR-008-point-location.md` designed `triangulation::locate`
(above); implementation shipped in 2 rounds (Round 1: the algorithm and
its correctness argument; Round 2: a shared-interior-edge
order-independence test, an outer-vs-hole classification test, and an
independent BigRational oracle covering `locate`'s actual aggregation
logic rather than re-testing its own primitives).

## Error enums are `#[non_exhaustive]` (0.3.0)

`KikaError`, `CdtError`, `PolygonTriangulationError`, and (doc-hidden)
`TopologyError` are all `#[non_exhaustive]` as of 0.3.0 — new variants
(e.g. `CdtError::DegeneratePointSet`, or `PolygonTriangulationError`'s six
0.4.0 hole-rejection variants) are additive, not breaking. See
`CHANGELOG.md`'s 0.3.0 entry and `tasks/lessons.md`'s criterion for which
enums get this treatment (`Result`-style error enums, not closed
geometric/structural classifications like `Sign`/`Orientation`/
`SegmentIntersectionKind`).

## Layering (§4.2)

1. **Exact arithmetic core** (`predicates::expansion`) — no geometric
   meaning, just error-free floating-point transformations and
   nonoverlapping expansions. Reused by every predicate, and by future
   exact constructions (ADR-004).
2. **Exact Predicates** (`predicates::{orient2d,orient3d,incircle,insphere}`)
   — each is: compute a fast filtered estimate with a computed error bound;
   if inconclusive, recompute via the exact arithmetic core and take the
   sign of the resulting expansion's most significant component.
3. **Geometric queries built on predicates** (`primitives::{Segment2,
   Triangle2}::relation_to`) — compose one or more calls to layer 2 into a
   richer classification (point-on-segment, point-in-triangle). Each
   degenerate case (zero-length segment, collinear triangle) needs its
   *own* explicit handling — composing exact primitives does not
   automatically make the composition's edge cases exact; two of this
   layer's degenerate cases were wrong on first implementation and caught
   by testing, not derivation (see `docs/degeneracy-policy.md`).
4. **Intersections** (`intersections::segment2`) — the same
   compose-and-verify pattern as layer 3, one level more involved
   (AABB-reject fast path, then a branching decision tree over several
   layer-2/3 calls). Predicate (`segment_intersection_kind`) and
   construction (`segment_intersection`) are separate functions per §4.2
   — the predicate never divides or builds a new coordinate; only the
   `Proper`-crossing construction case does, and (as of Phase 5, layer 8
   below) that division is *correctly rounded*, not merely approximate —
   every other construction case reuses an original input coordinate
   exactly, since it corresponds to an actual shared point.
5. **Polygon** (`polygon::Polygon2`) — the same layering *within* one
   type: `signed_area()` is a plain `f64` construction (a number, not a
   sign — no exactness claim, matches `segment_intersection`'s `Proper`
   case); `orientation()` is a genuine exact predicate, reusing the same
   `expansion`/`merge_all` machinery as layer 1/2 to sum every edge's
   shoelace term exactly rather than trust a running `f64` sum (which
   could round through cancellation for a near-degenerate polygon).
   `basic_validity()`/`find_self_intersection()` compose layers 2–4, same
   as layer 3.
6. **Convex hull** (`hull::convex_hull2`) — Andrew monotone chain, built
   entirely from `orient2d` turn tests plus an input sort. Unlike layers 3–5,
   this algorithm's output is *fully exact*, not just its component
   predicate calls: every returned vertex is copied from an original input
   `Point2`, never a computed/interpolated coordinate — there is nothing
   here analogous to `segment_intersection`'s non-exact `Proper` case. The
   fully collinear input case is detected explicitly with its own `orient2d`
   precheck up front, rather than inferred from the chain construction's
   output length — a length-based heuristic (e.g. "the lower chain used
   every point") is not reliable, since a genuinely 2D "valley" point set
   can legitimately do the same thing without being collinear (ruled out by
   a concrete counterexample during design; see `docs/degeneracy-policy.md`
   and `tasks/lessons.md`).
7. **Delaunay triangulation** (`triangulation::delaunay2`) — Bowyer-Watson
   incremental insertion. Also fully exact, like layer 6, but by a
   different means: rather than a synthetic bounding coordinate,
   "outside the current triangulation" is represented by a single
   symbolic ghost vertex with no coordinate at all, always maintaining a
   closed triangle fan around it (exactly like any real interior point
   would). A triangle carrying the ghost reduces its circumcircle test to
   an exact `orient2d` half-plane check against its one real edge. An
   earlier design used a synthetic "super-triangle" coordinate instead;
   that version passed every hand-written unit test but a property test
   on ordinary (non-adversarial) random input found it silently dropping
   a triangle, because whether a super-triangle vertex shields a real
   edge is scale-dependent with no universally-safe multiplier. See
   `docs/numerical-model.md`'s Phase 4 section, `tasks/lessons.md`, and
   `tests/regression/delaunay2.rs` for the full trail.
8. **Exact/certified constructions** (`predicates::constructions::line_intersection`,
   Phase 5, ADR-004) — the crate's first layer that builds a genuinely new
   coordinate rather than only classifying existing ones (closing
   `segment_intersection`'s `Proper`-case exactness gap from layer 4).
   `Point2` stays a plain `f64` pair (`float+certificate`, not a new
   exact-coordinate type — ADR-004's decision); the numerator/denominator
   are built as exact expansions reusing `orient2d`'s own machinery, and
   the final division is resolved to the *correctly-rounded* nearest
   `f64` (`correctly_rounded_divide`) rather than an ordinary,
   possibly-off-by-one-ULP division. 0.3.0 found and fixed a real gap
   here: extreme or mixed-magnitude inputs could overflow the numerator
   to `NaN`, breaking the crate-wide "a constructed `Point2` is always
   finite" invariant — fixed with exact power-of-two rescaling (lossless,
   an exponent shift), verified correctly rounded up through `~3.3e150`.
9. **Triangulation adjacency** (`triangulation::{ids,delaunay2}`, Phase
   6B, ADR-006) — `Triangulation2` gains `VertexId`/`EdgeId`/`FaceId` and
   query methods (`vertices`, `edges`, `faces`, `edge_vertices`,
   `adjacent_faces`, `face_vertices`, `neighboring_faces`,
   `boundary_edges`) over its existing coordinate-only `triangles()`
   contract — a static, post-construction snapshot (no mutation API), so
   plain dense-array indices suffice for ids, no generational-arena
   complexity needed. Indexed-triangle adjacency was chosen over
   half-edge/DCEL or quad-edge (ADR-006's own comparison) as the smaller,
   additive change sufficient for what Phase 6 actually needs.
10. **Constrained Delaunay** (`triangulation::cdt`, Phase 6C) — segment
    recovery and local-Delaunay restoration done *entirely* by flipping
    existing Delaunay edges, confirming ADR-004's Phase 6 re-evaluation
    prediction that this needs no new construction at all (not one new
    coordinate is ever built). Both flip passes are bounded
    (`4 * face_count + 16`, measured not assumed), never looping to
    convergence with no ceiling; `CdtError` (`#[non_exhaustive]` since
    0.3.0) reports crossing/collinear constraints, algorithm exhaustion,
    and degenerate point sets as typed errors, never a panic.
11. **Polygon triangulation** (`triangulation::polygon`, Phase 6D +
    0.4.0) — `triangulate_polygon` builds on layer 10's CDT: constrain
    every polygon edge, then discard concave-pocket faces (for a
    non-convex boundary) via a purely topological flood fill from one
    interior seed face. `triangulate_polygon_with_holes` (0.4.0)
    generalizes the same algorithm rather than using a new one — a
    hole's boundary is just more constrained edges the same flood fill
    already stops at, so hole-interior faces get discarded by the exact
    same mechanism that discards a non-convex boundary's own concave
    pockets. Backed by layer 5's 0.4.0 addition, `Polygon2::relation_to`
    (exact point-in-polygon), for hole-containment validation.

`docs/adr/ADR-007-voronoi-diagram-topology.md` designs a further layer —
Voronoi topology as `Triangulation2`'s dual — for 0.5.0; implemented in
full as `triangulation::voronoi` (see the module tree above), release
not yet done (see `ROADMAP.md`, internal).

## Data flow for a predicate call

```text
Point2::new(x, y) -> Result<Point2, KikaError>   (finiteness checked here, once)
        │
        ▼
orient2d(a: Point2, b: Point2, c: Point2) -> Orientation   (never panics, never fails)
        │
        ├─ filter: f64 determinant + computed error bound → conclusive? return.
        └─ fallback: expansion-arithmetic exact determinant → sign of leading term.
```

## Data flow for a composed query (segment intersection)

```text
segment_intersection_kind(s1, s2) -> SegmentIntersectionKind
        │
        ├─ Aabb2::overlaps fast-reject (no predicate calls at all)
        ├─ zero-length segment(s): explicit case, via Segment2::relation_to
        ├─ orient2d ×2..4 + Segment2::relation_to: classify
        └─ never divides, never builds a new Point2

segment_intersection(s1, s2) -> SegmentIntersection2   (separate call)
        │
        └─ re-derives the same classification, then:
           Proper        -> divides, builds a new (non-exact) Point2
           EndpointTouch,
           CollinearTouch,
           CollinearOverlap -> reuses original input point(s), exact
```
