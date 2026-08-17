# Release checklist

0.4.0 release preparation in progress — 0.2.0 and 0.3.0 already shipped;
0.4.0 adds polygon triangulation with holes plus wasm32 execution testing
(see `CHANGELOG.md`'s 0.4.0 entry). `crates.io` publish, GitHub release,
and the `git push`/tag that precede them all require explicit user
approval (AGENTS.md §19, `tasks/todo.md`'s "Deferred pending explicit user
approval") regardless of how much of this checklist is green. See this
checklist's checkboxes for exactly what has and hasn't been verified as of
the 0.4.0 version-bump commit.

## Before any release

- [x] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
      warnings` (both native and `--target wasm32-unknown-unknown`),
      `cargo test` (unit/adversarial/differential/regression/doc, 302
      tests) all pass — re-verified at the 0.4.0 version-bump commit
- [x] `cargo +1.85 test --all-features` (MSRV) passes
- [x] `cargo build --target wasm32-unknown-unknown --release` passes
- [x] `wasm-pack test --node --release` passes (10/10 tests, actual
      Node.js execution, not just a wasm32 build)
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes (every public
      item documented — enforced by `#![warn(missing_docs)]` +
      clippy's `-D warnings`)
- [x] `cargo build --examples` passes (8 examples)
- [x] CI green on the actual push for all 0.4.0 commits
      (`4e54804`..`d6bf971`, all 10 jobs green — including
      `wasm-test-node`'s first real CI run); the version-bump/CHANGELOG
      commit(s) on top are only locally verified so far, not yet pushed
- [ ] `cargo package --list` reviewed — no accidental inclusion of
      scratch/dev-only files
- [ ] `cargo publish --dry-run` passes
- [x] `CHANGELOG.md` has a dated entry for the release
      (`[0.4.0] - 2026-08-18`), not just `[Unreleased]`
- [x] Version bumped in `Cargo.toml` to `0.4.0`
- [x] `README.md`'s (and `README_ja.md`/`README_zh.md`'s) Status line,
      "Implemented today" section, Maturity table, and examples list
      updated to reference 0.4.0 — including closing translation drift
      found in `README_ja.md`/`README_zh.md` (the `triangulate_polygon_with_holes`
      paragraph and its Maturity-table row hadn't been translated yet)
- [x] `docs/compatibility.md` synced: 0.4.0 candidate framing, current
      test count, wasm32 execution-testing status, public API surface list

## Not required before a release, but should not be silently missing

- [x] `examples/` build (`cargo build --examples`) — 8 examples build
      clean (regression risk if a public API change breaks an example
      without CI catching it; examples are not currently run in CI, only
      built via the normal `cargo build`/`clippy --all-targets` steps)
- [x] `cargo deny check` green with the new `wasm-bindgen-test`
      (`wasm32`-only dev-dependency) in the dependency tree

## What 0.4.0 actually changes vs. 0.3.0

Per `CHANGELOG.md`: `triangulate_polygon_with_holes` (generalizes
`triangulate_polygon`'s existing algorithm, no new construction) plus its
supporting `Polygon2::relation_to`/`PointPolygonRelation` predicate; 6 new
`PolygonTriangulationError` variants (additive — non-breaking, thanks to
0.3.0's `#[non_exhaustive]`); wasm32 test execution under Node.js (not
just a build), via a new `wasm32`-only `wasm-bindgen-test` dev-dependency
and an independent `wasm-test-node` CI job. No breaking changes.

## Explicitly out of scope for 0.4.0

Per `ROADMAP.md` (internal, untracked): Voronoi diagram topology API
(0.5.0), point-location/spatial-query API (0.6.0), exact segment
arrangement kernel (0.7.0), polygon Boolean (0.8.0) — each its own
future release, deliberately not bundled into this one. Also unstarted:
3D triangulation, mesh Boolean/repair, surface reconstruction,
point-cloud processing, vertex deletion, Delaunay refinement, the full
CGAL differential harness (environment-blocked), long-duration fuzzing,
large-scale competitive performance benchmarking.
