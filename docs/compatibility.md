# Compatibility

Status: Phase 1-5 complete; Phase 6A-6D complete. As of the 0.3.0
candidate (0.2.0 already shipped; 0.3.0 is a robustness/compatibility
follow-up — see `CHANGELOG.md`), Kika is a robust 2D kernel with exact
predicates, 2D convex hull, Delaunay triangulation, constrained Delaunay
triangulation (narrow scope), and simple-polygon triangulation (narrow
scope) — see `README.md`'s Maturity table for exactly what each covers.
Polygon Boolean and later Phase 6 work not started, deliberately
deferred.

## Platforms

* `aarch64-apple-darwin` — full test suite (274 tests across unit,
  differential, adversarial, regression, doctests) run locally on this
  target during development, not just built.
* `x86_64`, `aarch64` Linux/macOS/Windows — `.github/workflows/ci.yml`
  runs the full test suite on `ubuntu-latest`, `macos-latest` (Apple
  Silicon on current GitHub-hosted runners), `windows-latest`. Confirmed
  green on actual pushes to `kent-tokyo/kika` (all jobs: fmt, clippy,
  test matrix, MSRV, wasm32 build, `cargo doc`, `cargo deny`) as of
  Phase 5's push, again after Phase 6A-6D + the CDT bug-fix commit landed
  (commit `d5e755a`, all 9 jobs green), and again after the 0.3.0
  bug-check-and-refactoring commits landed (`bf936c8`..`59f92b3`, all 5
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
  is unchanged). Not a port of the full 274-test native suite — deliberately
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

Pre-1.0. No stability guarantees yet. As of 0.3.0, the four `Result`-style
error enums (`KikaError`, `CdtError`, `PolygonTriangulationError`, and
`#[doc(hidden)]` `TopologyError`) are `#[non_exhaustive]`, so new variants
can be added without breaking a downstream `match` that already includes a
wildcard arm — see `CHANGELOG.md`'s 0.3.0 entry for why this didn't
already hold in 0.2.0. Classification enums that are not `Result` errors
(`Sign`, `Orientation`, `PointSegmentRelation`, `PointTriangleRelation`,
`SegmentIntersectionKind`, `SegmentIntersection2`, `PolygonBasicValidity`,
`HullBoundaryPoints`) remain exhaustive; their variant sets are complete
by construction.

Public API surface as of Phase 6D:
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
`triangulate_polygon_with_holes` (0.4.0, in progress — see
`ROADMAP.md`, untracked/internal).
