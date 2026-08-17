# Compatibility

Status: Phase 1-5 complete; Phase 6A-6D complete (narrow-scope
constrained Delaunay and simple-polygon triangulation — see
`README.md`'s Maturity table). Polygon Boolean and later Phase 6 work
not started, deliberately deferred.

## Platforms

* `aarch64-apple-darwin` — full test suite (259 tests across unit,
  differential, adversarial, regression, doctests) run locally on this
  target during development, not just built.
* `x86_64`, `aarch64` Linux/macOS/Windows — `.github/workflows/ci.yml`
  runs the full test suite on `ubuntu-latest`, `macos-latest` (Apple
  Silicon on current GitHub-hosted runners), `windows-latest`. Confirmed
  green on an actual push to `kent-tokyo/kika` (all jobs: fmt, clippy,
  test matrix, MSRV, wasm32 build, `cargo doc`, `cargo deny`) as of
  Phase 5's push — Phase 6A-6D's later commits are verified locally
  (this same suite, plus `cargo bench --bench sanity`) but not yet
  pushed, so CI has not run on them yet; see `tasks/todo.md`.
* `wasm32-unknown-unknown` — library **builds** successfully, verified
  both locally and in CI (`cargo build --target wasm32-unknown-unknown
  --lib`). Tests do **not** run under wasm32 — that needs a WASM test
  runner (`wasm-bindgen-test` + a JS engine, or `wasmtime`), not set up
  in Phase 1. This matters specifically for the exact-arithmetic core's
  correctness claims (ADR-001's argument that Rust never contracts
  `+`/`-`/`*` into FMA is a language-level guarantee, not empirically
  re-verified under wasm32) — a known gap, not silently assumed fine;
  see `tasks/todo.md`.

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

Pre-1.0. No stability guarantees yet. Public API surface as of Phase 6D:
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
`triangulate_polygon` (Phase 6D).
