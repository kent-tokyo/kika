# Release checklist

0.5.0 release preparation in progress — 0.2.0 through 0.4.0 already
shipped; 0.5.0 adds topology-only Voronoi diagram support (see
`CHANGELOG.md`'s 0.5.0 entry). `crates.io` publish, GitHub release, and
the `git push`/tag that precede them all require explicit user approval
(AGENTS.md §19, `tasks/todo.md`'s "Deferred pending explicit user
approval") regardless of how much of this checklist is green. See this
checklist's checkboxes for exactly what has and hasn't been verified as
of the 0.5.0 version-bump commit.

## Before any release

- [x] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
      warnings` (both native and `--target wasm32-unknown-unknown`),
      `cargo test --all-features` (unit/adversarial/differential/
      regression/doc, 320 tests) all pass — re-verified at the 0.5.0
      version-bump commit
- [x] `cargo +1.85 test --all-features` (MSRV) passes
- [x] `cargo build --target wasm32-unknown-unknown --release` passes
- [x] `wasm-pack test --node --release` passes (10/10 tests, actual
      Node.js execution, not just a wasm32 build)
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes (every public
      item documented — enforced by `#![warn(missing_docs)]` +
      clippy's `-D warnings`)
- [x] `cargo build --examples` passes (9 examples, `examples/voronoi.rs`
      new this release)
- [x] CI green on the actual push for all 0.5.0 commits: Phase 7A
      (`b9702c1`..`7050f9f`), Phase 7B (`18c8d6e`..`92a4d4b`), Phase 7C
      (`45a91d1`..`159cf56`) — all 10 jobs green on each of the 3 pushes;
      the version-bump/CHANGELOG/README-sync commits on top
      (`7978207`..`3573075`) are only locally verified so far, not yet
      pushed
- [x] `cargo package --list` reviewed (99 files) — no accidental
      inclusion of scratch/dev-only files; specifically confirmed
      `ROADMAP.md` (internal, gitignored) does **not** leak into the
      package. One local-only artifact (`.claude/scheduled_tasks.lock`,
      this development session's own tooling state, never git-tracked)
      appeared in the local dry-run listing — not a real concern, since
      the actual `cargo publish` runs via `.github/workflows/publish.yml`
      against a fresh `actions/checkout`, where that file never exists;
      confirmed by reading the workflow rather than assumed
- [x] `cargo publish --dry-run` passes (kika v0.5.0, 841.6KiB / 243.5KiB
      compressed)
- [x] `CHANGELOG.md` has a dated entry for the release
      (`[0.5.0] - 2026-08-19`), not just `[Unreleased]`
- [x] Version bumped in `Cargo.toml` to `0.5.0`
- [x] `README.md`'s (and `README_ja.md`/`README_zh.md`'s) Status line,
      "Implemented today" section, Minimal example, examples list,
      Maturity table, and Roadmap section updated to reference 0.5.0 —
      including fixing a pre-existing translation-drift issue in
      `README_ja.md`/`README_zh.md`'s Roadmap closing sentence (still
      claimed 0.2.0/0.3.0/0.4.0 release verification "hasn't happened
      yet", stale since all three shipped)
- [x] `docs/compatibility.md` synced: 0.5.0 candidate framing, the 3
      Voronoi-phase CI-green pushes recorded, public API surface list
      extended
- [x] `docs/degeneracy-policy.md` gained a Voronoi diagram topology
      section — each row backed by an actual run (1/2/collinear-point
      input, a collinear hull stretch, partial cocircularity, a
      near-cocircular-but-not point, a single-triangle cell's
      `cell_edges()`), not just derived from the general design
- [x] `cargo deny check` green (advisories/bans/licenses/sources all ok)

## Not required before a release, but should not be silently missing

- [x] `examples/` build (`cargo build --examples`) — 9 examples build
      clean (regression risk if a public API change breaks an example
      without CI catching it; examples are not currently run in CI, only
      built via the normal `cargo build`/`clippy --all-targets` steps).
      `examples/voronoi.rs` is self-checking (asserts bounded/unbounded
      cell counts, `edge_cells()`/`cell_edges()` consistency, a clean
      `validate_voronoi_topology()`), matching `constrained_delaunay.rs`'s
      precedent, not just printing output
- [x] `cargo deny check` green with the existing `wasm-bindgen-test`
      (`wasm32`-only dev-dependency) in the dependency tree — no new
      dependency added for Voronoi topology support

## What 0.5.0 actually changes vs. 0.4.0

Per `CHANGELOG.md`: `Voronoi2`/`voronoi2` — a topology-only Voronoi
diagram (no vertex coordinates/circumcenters, clipping, or
nearest-neighbor query), the dual of an existing `Triangulation2`.
Cocircular Delaunay faces are merged via union-find keyed on
`incircle(...) == Sign::Zero` so `delaunay2`'s own tie-break can't leak
into the output as a spurious extra vertex/edge; dense ids are assigned
by sorting on a canonical site-identity key, not union-find root or scan
order, so differently-triangulated-but-topologically-equal inputs
produce identical output. Query API: `cells`/`vertices`/`edges`,
`cell_site`, `neighboring_cells`, `cell_is_unbounded`, `edge_cells`,
`edge_kind`, `dual_delaunay_edge`, `vertex_delaunay_faces`, and
`cell_edges` (an ordered counterclockwise boundary walk, built entirely
from `Triangulation2`'s existing face adjacency — no new data model). No
breaking changes — purely additive at the crate root.

## Explicitly out of scope for 0.5.0

Per `ROADMAP.md` (internal, untracked): Voronoi vertex coordinates
(circumcenters), clipping, nearest-neighbor query — `cell_edges()`
covers the ordering need those would otherwise require a coordinate for.
Also per the original 0.5.0 scoping in `ROADMAP.md`: point-location/
spatial-query API (0.6.0), exact segment arrangement kernel (0.7.0),
polygon Boolean (0.8.0) — each its own future release, deliberately not
bundled into this one. Also unstarted: 3D triangulation, mesh
Boolean/repair, surface reconstruction, point-cloud processing, vertex
deletion, Delaunay refinement, the full CGAL differential harness
(environment-blocked), long-duration fuzzing, large-scale competitive
performance benchmarking.
