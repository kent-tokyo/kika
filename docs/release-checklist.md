# Release checklist

0.6.0 release preparation in progress — 0.2.0 through 0.5.0 already
shipped; 0.6.0 adds `Triangulation2::locate`/`PointLocation` point
location (see `CHANGELOG.md`'s 0.6.0 entry). `crates.io` publish, GitHub
release, and the `git push`/tag that precede them all require explicit
user approval (AGENTS.md §19, `tasks/todo.md`'s "Deferred pending
explicit user approval") regardless of how much of this checklist is
green. See this checklist's checkboxes for exactly what has and hasn't
been verified as of the 0.6.0 version-bump commit.

## Before any release

- [x] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
      warnings` (both native and `--target wasm32-unknown-unknown`),
      `cargo test --all-features` (unit/adversarial/differential/
      regression/doc, 333 tests) all pass — re-verified at the 0.6.0
      version-bump commit
- [x] `cargo +1.85 test --all-features` (MSRV) passes
- [x] `cargo build --target wasm32-unknown-unknown --release` passes
- [x] `wasm-pack test --node --release` passes (10/10 tests, actual
      Node.js execution, not just a wasm32 build)
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes (every public
      item documented — enforced by `#![warn(missing_docs)]` +
      clippy's `-D warnings`)
- [x] `cargo build --examples` passes (10 examples, `examples/locate.rs`
      new this release)
- [x] CI green on the actual push for all 0.6.0 commits: Round 1
      (`2d994eb`..`e1bb9ba`), Round 2 (`98e3ecb`..`1216571`) — all 10
      jobs green on each of the 2 pushes; the version-bump/CHANGELOG/
      README-sync commits on top (`5277e58`..`e572396`) are only locally
      verified so far, not yet pushed
- [x] `cargo package --list` reviewed (102 files) — via a local `git
      worktree` of this exact commit (not the dirty dev tree, which
      still has this session's own `.claude/scheduled_tasks.lock` and
      would need `--allow-dirty` to even run), confirming no accidental
      inclusion of scratch/dev-only files; specifically confirmed
      `ROADMAP.md` (internal, gitignored) and `.claude/` do **not** leak
      into the package
- [x] `cargo publish --dry-run` passes (kika v0.6.0, 899.1KiB / 259.7KiB
      compressed) — same clean worktree, no `--allow-dirty` needed
- [x] `CHANGELOG.md` has a dated entry for the release
      (`[0.6.0] - 2026-08-19`), not just `[Unreleased]`
- [x] Version bumped in `Cargo.toml` to `0.6.0`
- [x] `README.md`'s (and `README_ja.md`/`README_zh.md`'s) Status line,
      "Implemented today" section, Minimal example, examples list,
      Maturity table, and Roadmap section updated to reference 0.6.0
- [x] `docs/compatibility.md` synced: 0.6.0 candidate framing, test
      count (333, was 302), the 2 point-location-round CI-green pushes
      recorded, `PointLocation` added to the exhaustive-enums list with
      its closed-enum rationale, public API surface list updated
- [x] `docs/architecture.md` module tree gained `locate.rs` and its
      ADR-008 pointer
- [x] `cargo deny check` green (advisories/bans/licenses/sources all ok)

## Not required before a release, but should not be silently missing

- [x] `examples/` build (`cargo build --examples`) — 10 examples build
      clean. `examples/locate.rs` is self-checking (asserts all 6
      classifications: Vertex, Edge, Face, hull-exterior Outside,
      hole-interior Outside, hole-boundary Edge), matching
      `voronoi.rs`/`constrained_delaunay.rs`'s precedent, not just
      printing output
- [x] `cargo deny check` green with no new dependency added for point
      location (the independent-oracle differential test reuses the
      existing `num-bigint`/`num-rational`/`num-traits` dev-dependencies
      `point_in_triangle.rs` already had)

## What 0.6.0 actually changes vs. 0.5.0

Per `CHANGELOG.md`: `Triangulation2::locate` / `PointLocation` — point
location against a triangulation's vertices, edges, and faces.
`PointLocation::{Vertex(VertexId), Edge(EdgeId), Face(FaceId), Outside}`,
a closed enum (not `#[non_exhaustive]`, since its 4 variants are exactly
the closure of `Triangulation2`'s own already-closed id vocabulary plus
the necessary miss case). `Outside` means "not covered by any face," not
"outside the convex hull" — a point inside a
`triangulate_polygon_with_holes` hole is also `Outside`. `O(F)`, a linear
scan, not a spatial index; performance deliberately not part of the
public contract for this release. Never panics, including on an empty
triangulation. Verified against an independent BigRational oracle
covering the actual aggregation/dispatch logic across faces, not just
this crate's own `Triangle2::relation_to`/`Segment2::relation_to`. No
breaking changes — purely additive at the crate root.

## Explicitly out of scope for 0.6.0

Per `ROADMAP.md` (internal, untracked): a spatial index or walking
locator for `locate` (explicitly deferred until a real measured need
exists — the signature doesn't need to change to add one),
nearest-neighbor query (ROADMAP explicitly says not to add this even
with spare capacity in the same release), any
`ConstrainedTriangulation2`-specific forwarding method (use
`cdt.triangulation().locate(p)`), exact segment arrangement kernel
(0.7.0), polygon Boolean (0.8.0) — each its own future release,
deliberately not bundled into this one. Also unstarted: Voronoi vertex
coordinates (circumcenters)/clipping (0.5.0's own deferred scope), 3D
triangulation, mesh Boolean/repair, surface reconstruction, point-cloud
processing, vertex deletion, Delaunay refinement, the full CGAL
differential harness (environment-blocked), long-duration fuzzing,
large-scale competitive performance benchmarking.
