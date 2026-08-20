# Compatibility

Status: Phase 1-5 complete; Phase 6A-6D complete. As of 0.7.1, shipped
2026-08-20 (0.2.0 through 0.7.1 all released; 0.7.0 added
`Voronoi2::vertex_point`/`edge_geometry` — Voronoi vertex/edge geometry
on top of 0.5.0's topology — see `CHANGELOG.md`), Kika is a robust 2D
kernel with exact predicates, 2D convex hull, Delaunay triangulation,
constrained Delaunay triangulation (narrow scope), simple-polygon
triangulation with or without holes, a Voronoi diagram with both
topology and geometry, and point location — see `README.md`'s Maturity
table for exactly what each covers. Voronoi clipping, nearest-neighbor
query, a spatial index/walking locator for `locate`, polygon Boolean,
and later roadmap work not started, deliberately deferred.

0.7.0 shipped `fuzz/fuzz_targets/voronoi_geometry.rs`, which found (on
its first run) a real, pre-existing panic in `delaunay2()` itself —
`orient2d` could return permutation-inconsistent answers at extreme
mixed coordinate magnitude, breaking an antisymmetry assumption
`delaunay2()` relies on — unrelated to Voronoi geometry, shipped as a
documented 0.7.0 known issue rather than blocking that release.
**Fixed in 0.7.1**: two independent overflow sites in the exact-fallback
arithmetic core (`predicates::expansion`), both producing a silent `NaN`
that broke `orient2d`'s permutation-consistency guarantee. See
`CHANGELOG.md`'s `[0.7.1]` entry and `docs/numerical-model.md`'s "Known
limitation (fixed): split() overflow and two_sum overflow for sign-only
predicates" for the full diagnosis and fix. A structurally different,
harder case remains deliberately deferred — not a permutation-consistency
bug, doesn't panic, needs a different arithmetic architecture to fix —
tracked in `tasks/todo.md` and `docs/numerical-model.md`'s "Known
limitation: exact-product representability ceiling".

## Platforms

* `aarch64-apple-darwin` — full test suite (369 tests across unit,
  differential, adversarial, regression, doctests — plus 10 more under
  `wasm-pack test`, see below) run locally on this target during
  development, not just built.
* `x86_64`, `aarch64` Linux/macOS/Windows — `.github/workflows/ci.yml`
  runs the full test suite on `ubuntu-latest`, `macos-latest` (Apple
  Silicon on current GitHub-hosted runners), `windows-latest`. Confirmed
  green on actual pushes to `kent-tokyo/kika` (all jobs: fmt, clippy,
  test matrix, MSRV, wasm32 build, `cargo doc`, `cargo deny`) as of
  Phase 5's push, again after Phase 6A-6D + the CDT bug-fix commit landed
  (commit `d5e755a`, all 9 jobs green), again after the 0.3.0
  bug-check-and-refactoring commits landed (`bf936c8`..`59f92b3`, all 5
  jobs green), again after the 0.4.0 polygon-with-holes and wasm
  execution-testing commits landed (`4e54804`..`d6bf971`, all 10 jobs
  green, including the new `wasm-test-node` job's first real CI run), and
  again across the 3 pushes making up 0.5.0's Voronoi topology work
  (Phase 7A `b9702c1`..`7050f9f`, Phase 7B `18c8d6e`..`92a4d4b`, Phase 7C
  `45a91d1`..`159cf56`, all 10 jobs green on each push), again across the
  2 pushes making up 0.6.0's point-location work (Round 1
  `2d994eb`..`e1bb9ba`, Round 2 `98e3ecb`..`1216571`, all 10 jobs green
  on each push), and again across the 2 pushes making up 0.7.0's Voronoi
  geometry work (design + implementation `dda8084`..`e43b125`, all 10
  jobs green; the same-day hardening round `c0a4a35`..`7fcdf7e`, all 10
  jobs green) — CI-confirmed, not just locally verified.
* `wasm32-unknown-unknown` — library **builds** successfully, verified
  both locally and in CI (`cargo build --target wasm32-unknown-unknown
  --lib`, the `wasm` job). As of this session, also **executed**, not
  just built: `tests/wasm.rs` (`wasm-bindgen-test`, a `wasm32`-only
  dev-dependency — see `Cargo.toml`'s
  `[target.'cfg(target_arch = "wasm32")'.dev-dependencies]`) runs 10
  load-bearing cases — one per major subsystem (`orient2d`/`orient3d`
  sign, `incircle`/`insphere` basic cases, `segment_intersection`
  finiteness, the 0.3.0 extreme/mixed-magnitude `line_intersection`
  regression case, `delaunay2` triangle count/topology, degenerate CDT
  as a typed error not a panic, `triangulate_polygon`/
  `triangulate_polygon_with_holes` happy paths) — under an actual Node.js
  runtime via `wasm-pack test --node --release`, both locally and in CI
  (the separate `wasm-test-node` job; the existing `wasm` build-only job
  is unchanged). Not a port of the full 369-test native suite — deliberately
  small, to catch wasm32-*specific* codegen/execution divergence, not to
  duplicate coverage the native suite already has exhaustively. This
  closes the gap that mattered specifically for the exact-arithmetic
  core's correctness claims (ADR-001's argument that Rust never
  contracts `+`/`-`/`*` into FMA is a language-level guarantee, previously
  not empirically re-verified under wasm32, now is).

## MSRV

Rust 1.85 (edition 2024). Fixed in `Cargo.toml` (`rust-version`).
Actually verified locally (`cargo +1.85 test --all-features`, full suite
green), not just declared; also checked in CI.

## CGAL

Kika does not link against or depend on CGAL. CGAL is used only as an
external differential-test oracle in a separate comparison program (not
yet built — currently environment-blocked: CGAL/pkg-config are not
installed in this development environment, see `tasks/todo.md`), never
part of the `kika` crate itself (§10).

## Stability

Pre-1.0. No stability guarantees yet. As of 0.3.0, `Result`-style error
enums are `#[non_exhaustive]`, so new variants can be added without
breaking a downstream `match` that already includes a wildcard arm — see
`CHANGELOG.md`'s 0.3.0 entry for why this didn't already hold in 0.2.0:
`KikaError`, `CdtError`, `PolygonTriangulationError`,
`#[doc(hidden)]` `TopologyError`, and (0.7.0) `VoronoiGeometryError`.
`VoronoiEdgeGeometry` (0.7.0) is also `#[non_exhaustive]` despite not
being an error enum — same reasoning as `VoronoiEdgeKind`'s own
precedent (0.5.0): closed under "≥3 non-collinear sites" scope, not
closed by mathematical necessity, so a future degenerate case (e.g. a
1-2-site `Line` variant) can be added without breaking an existing
`match`. Classification enums that are not `Result` errors and don't
carry this "scope, not necessity" caveat (`Sign`, `Orientation`,
`PointSegmentRelation`, `PointTriangleRelation`, `SegmentIntersectionKind`,
`SegmentIntersection2`, `PolygonBasicValidity`, `HullBoundaryPoints`,
`PointLocation`) remain exhaustive; their variant sets are complete by
construction. `PointLocation` (0.6.0) is closed for a specific, stated
reason (ADR-008), not by default: its 4 variants are exactly the closure
of `Triangulation2`'s own already-closed `VertexId`/`EdgeId`/`FaceId`
vocabulary plus the necessary miss case — a 2-simplicial complex has
only 0-, 1-, and 2-cells, so no further variant is possible.

Public API surface as of 0.7.0:
`Point2`, `Point3`, `Vector2`, `Vector3`, `Segment2`, `Triangle2`,
`Triangle3`, `Aabb2`, `Aabb3`, `Sign`, `Orientation`, `KikaError`,
`orient2d`, `orient3d`, `incircle`, `insphere`,
`PointSegmentRelation`, `PointTriangleRelation`, `SegmentIntersectionKind`,
`SegmentIntersection2`, `segment_intersection_kind`, `segment_intersection`,
`Polygon2`, `PolygonBasicValidity`, `PolygonSelfIntersection`,
`HullBoundaryPoints`, `convex_hull2`, `Triangulation2`, `delaunay2`,
`VertexId`, `EdgeId`, `FaceId` (Phase 6B adjacency structure),
`ConstrainedTriangulation2`, `CdtError`, `constrained_delaunay2`,
`validate_cdt_topology` (Phase 6C), `PolygonTriangulationError`,
`triangulate_polygon` (Phase 6D), `PointPolygonRelation`,
`triangulate_polygon_with_holes` (0.4.0), `Voronoi2`, `voronoi2`,
`VoronoiCellId`, `VoronoiVertexId`, `VoronoiEdgeId`, `VoronoiEdgeKind`,
`VoronoiEdge` (0.5.0, ADR-007) — topology only, no coordinate type added
— `PointLocation` (0.6.0, ADR-008) — `Triangulation2::locate` itself is
a method, not a separate re-export — and (0.7.0, ADR-009)
`VoronoiGeometryError`, `VoronoiEdgeGeometry` —
`Voronoi2::vertex_point`/`edge_geometry` are likewise methods, not
separate re-exports.
