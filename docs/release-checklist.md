# Release checklist

0.7.0 release preparation in progress — 0.2.0 through 0.6.0 already
shipped; 0.7.0 adds `Voronoi2::vertex_point`/`edge_geometry` (see
`CHANGELOG.md`'s 0.7.0 entry). `crates.io` publish, GitHub release, and
the `git push`/tag that precede them all require explicit user approval
(AGENTS.md §19, `tasks/todo.md`'s "Deferred pending explicit user
approval") regardless of how much of this checklist is green. See this
checklist's checkboxes for exactly what has and hasn't been verified as
of the 0.7.0 version-bump commit.

## Before any release

- [x] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
      warnings` (both native and `--target wasm32-unknown-unknown`),
      `cargo test --all-features` (unit/adversarial/differential/
      regression/doc, 360 tests) all pass — re-verified at the 0.7.0
      version-bump commit
- [x] `cargo +1.85 test --all-features` (MSRV) passes
- [x] `cargo build --target wasm32-unknown-unknown --release` passes
- [x] `wasm-pack test --node --release` passes (10/10 tests, actual
      Node.js execution, not just a wasm32 build)
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes (every public
      item documented — enforced by `#![warn(missing_docs)]` +
      clippy's `-D warnings`)
- [x] `cargo build --examples` passes (10 examples, `examples/voronoi.rs`
      extended this release to also exercise `edge_geometry`)
- [x] CI green on the actual push for all 0.7.0-scoped commits: design +
      implementation (`dda8084`..`e43b125`), the same-day hardening round
      (`c0a4a35`..`7fcdf7e`) — all 10 jobs green on each of the 2 pushes;
      the version-bump/CHANGELOG/docs-sync/README-sync commits on top
      (`1896e52`..`016ef79`) are only locally verified so far, not yet
      pushed
- [x] `cargo package --list` reviewed (106 files) — via a local `git
      worktree` of the `README_ja.md`/`README_zh.md` sync commit
      (`016ef79`, not the dirty dev tree, which still has the
      pre-existing, not-this-session's `.gitignore` change and
      `.claude/`), confirming no accidental inclusion of scratch/dev-only
      files; specifically confirmed `ROADMAP.md` (internal, gitignored)
      and `.claude/` do **not** leak into the package
- [x] `cargo publish --dry-run` passes (kika v0.7.0, 1.0MiB / 303.0KiB
      compressed) — same clean worktree, no `--allow-dirty` needed
- [x] `CHANGELOG.md` has a dated entry for the release
      (`[0.7.0] - 2026-08-20`), not just `[Unreleased]` — including a
      "Known issues" entry for the pre-existing `delaunay2()` panic this
      release's own new fuzz target found (not fixed in 0.7.0,
      deliberately — unrelated subsystem, see below)
- [x] Version bumped in `Cargo.toml` to `0.7.0`
- [x] `README.md`'s (and `README_ja.md`/`README_zh.md`'s) Status line,
      "Implemented today" section, Minimal example, examples list,
      Maturity table, and Roadmap section updated to reference 0.7.0
- [x] `docs/compatibility.md` synced: 0.7.0 status framing, test count
      (360, was 333), the 2 Voronoi-geometry-round CI-green pushes
      recorded, `VoronoiGeometryError` added to the `#[non_exhaustive]`
      list, `VoronoiEdgeGeometry` documented as `#[non_exhaustive]` for
      the "scope, not necessity" reason (matching `VoronoiEdgeKind`'s own
      0.5.0 precedent), public API surface list updated, the
      `delaunay2()` known issue recorded
- [x] `docs/architecture.md` module tree gained `circumcenter.rs`/
      `rounding.rs` and an updated `voronoi.rs` description; its own
      Status line (stale since 0.4.0, never updated across 0.5.0/0.6.0
      either) fixed alongside the 0.7.0 update, not deferred further
- [x] `cargo deny check` green (advisories/bans/licenses/sources all ok)

## Not required before a release, but should not be silently missing

- [x] `examples/` build (`cargo build --examples`) — 10 examples build
      clean. `examples/voronoi.rs` now also prints and asserts
      finiteness on every cell edge's `edge_geometry` output (`Segment`
      endpoints or `Ray` origin/direction), not just topology
- [x] `cargo deny check` green with no new runtime dependency added for
      Voronoi geometry (the independent-oracle differential test reuses
      the existing `num-bigint`/`num-rational`/`num-traits`
      dev-dependencies `line_intersection.rs` already had)
- [x] The new `fuzz/fuzz_targets/voronoi_geometry.rs` (not part of the
      required test suite, but recorded here rather than silently
      omitted): raw `f64::from_bits` coordinates through
      `delaunay2` -> `voronoi2` -> `vertex_point`/`edge_geometry`,
      asserting finiteness and ray-direction non-zero-ness. Its first
      run found the `delaunay2()` panic named above — a real result, not
      a clean bill of health for that unrelated subsystem

## What 0.7.0 actually changes vs. 0.6.0

Per `CHANGELOG.md`: `Voronoi2::vertex_point` / `edge_geometry` — actual
coordinates (circumcenters) and edge geometry (segments/rays) on top of
0.5.0's topology-only Voronoi diagram. `VoronoiGeometryError` and
`VoronoiEdgeGeometry::{Segment { start, end }, Ray { origin, direction
}}`, both `#[non_exhaustive]`. `vertex_point` is a correctly-rounded
(ADR-004-style) circumcenter construction (`predicates::constructions::
circumcenter`, sharing `correctly_rounded_divide` with
`line_intersection` via the new `predicates::constructions::rounding`
module). `edge_geometry`'s `Ray::direction` is an unnormalized outward
vector, guaranteed finite and non-zero for any two distinct finite
Delaunay vertices including opposite-sign near-`f64::MAX` coordinates (a
fixed power-of-two rescale fallback for the rare case a plain coordinate
difference would overflow). `Err(NonFiniteCircumcenter)` for a
collinear/thin defining face (not fixable by rescaling alone — a
triangle's aspect ratio, not its coordinates' magnitude, drives this
overflow); `Err(InvalidTopology)` for an internal invariant this crate's
own construction never violates. Never a panic either way. No breaking
changes — purely additive at the crate root.

**Known issue, not fixed in 0.7.0**: `delaunay2()` can panic on 3 input
points with extreme, widely mixed coordinate magnitude (`orient2d`
itself returns permutation-inconsistent answers at that magnitude) — a
pre-existing bug, unrelated to Voronoi geometry, found by this release's
own new fuzz target. See `CHANGELOG.md`'s 0.7.0 "Known issues" entry and
`tasks/todo.md` for the full repro and root-cause record. Deliberately
not fixed as part of this release (a `predicates`-level investigation,
out of scope for the Voronoi-geometry work that found it) — whether this
should have blocked 0.7.0 or ship as a tracked known issue was an open
question raised to the user; the user's instruction to proceed with
release preparation is treated as accepting it as a tracked known issue.

## Explicitly out of scope for 0.7.0

Per `ROADMAP.md` (internal, untracked): Voronoi clipping and
nearest-neighbor query (0.5.0's own deferred scope, still deferred), a
spatial index or walking locator for `locate` (0.6.0's own deferred
scope, still deferred), nearest-site query (0.8.0), exact segment
arrangement kernel (0.9.0), polygon Boolean (0.10.0) — each its own
future release, deliberately not bundled into this one. Also unstarted:
3D triangulation, mesh Boolean/repair, surface reconstruction,
point-cloud processing, vertex deletion, Delaunay refinement, the full
CGAL differential harness (environment-blocked), long-duration fuzzing,
large-scale competitive performance benchmarking, and (this release's
own discovered work) fixing `delaunay2()`'s permutation-inconsistent-
`orient2d` panic at extreme mixed magnitude.
