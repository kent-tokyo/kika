# Release checklist

0.3.0 release preparation in progress — 0.2.0 already shipped; 0.3.0 is a
robustness/compatibility follow-up (see `CHANGELOG.md`'s 0.3.0 entry).
`crates.io` publish, GitHub release, and the `git push`/tag that precede
them all require explicit user approval (AGENTS.md §19, `tasks/todo.md`'s
"Deferred pending explicit user approval") regardless of how much of this
checklist is green. See this checklist's checkboxes for exactly what has
and hasn't been verified as of the 0.3.0 version-bump commit.

## Before any release

- [x] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
      warnings`, `cargo test` (unit/adversarial/differential/regression/doc,
      274 tests) all pass — re-verified at the 0.3.0 version-bump commit
- [x] `cargo +1.85 test --all-features` (MSRV) passes
- [x] `cargo build --target wasm32-unknown-unknown --release` passes
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes (every public
      item documented — enforced by `#![warn(missing_docs)]` +
      clippy's `-D warnings`)
- [x] `cargo build --examples` passes (7 examples)
- [x] CI green on the actual push for the 9 bug-check-and-refactoring
      commits this 0.3.0 release is built on (`bf936c8`..`59f92b3`, all 5
      jobs green); the version-bump/CHANGELOG/`#[non_exhaustive]` commit(s)
      on top are only locally verified so far, not yet pushed
- [x] `cargo package --list` reviewed (92 files) — no accidental inclusion
      of scratch/dev-only files
- [x] `cargo publish --dry-run` passes (kika v0.3.0, 92 files, 183.1KiB
      compressed)
- [x] `CHANGELOG.md` has a dated entry for the release
      (`[0.3.0] - 2026-08-17`), not just `[Unreleased]`
- [x] Version bumped in `Cargo.toml` to `0.3.0`
- [x] `README.md`'s (and `README_ja.md`/`README_zh.md`'s) Status line
      updated to reference 0.3.0
- [x] `docs/compatibility.md` synced: 0.3.0 candidate framing, current
      test count, `#[non_exhaustive]` noted under Stability

## Not required before a release, but should not be silently missing

- [x] `examples/` build (`cargo build --examples`) — 7 examples build
      clean (regression risk if a public API change breaks an example
      without CI catching it; examples are not currently run in CI, only
      built via the normal `cargo build`/`clippy --all-targets` steps)

## What 0.3.0 actually changes vs. 0.2.0

Per `CHANGELOG.md`: two real bug fixes (`constrained_delaunay2` panicking
on degenerate point sets; `line_intersection` returning `NaN` at extreme/
mixed coordinate magnitudes), a doc-accuracy fix, two defensive/coverage
additions, several internal-only refactors, and — the reason this is
0.3.0 and not 0.2.1 — `#[non_exhaustive]` added to the four `Result`-style
public error enums, itself a breaking change for any 0.2.0 consumer with
an exhaustive `match` on one of them. No new features.

## Explicitly out of scope for 0.3.0

Same roadmap items deferred at 0.2.0 (polygon Boolean, exact Voronoi
construction, 3D triangulation, mesh Boolean/repair, surface
reconstruction, large-scale competitive benchmarking, long-duration
fuzzing, full CGAL differential verification) — none of this work started
between 0.2.0 and 0.3.0, which was scoped as a bug-check/robustness pass
only.
