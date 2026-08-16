# Release checklist

Not run yet — no release has happened. `crates.io` publish and GitHub
release both require explicit user approval (AGENTS.md §19,
`tasks/todo.md`'s "Deferred pending explicit user approval") regardless of
how much of this checklist is green.

## Before any release

- [ ] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
      warnings`, `cargo test` (unit/adversarial/differential/regression/doc)
      all pass
- [ ] `cargo +1.85 test --all-features` (MSRV) passes
- [ ] `cargo build --target wasm32-unknown-unknown --release` passes
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes (every public
      item documented — enforced by `#![warn(missing_docs)]` +
      clippy's `-D warnings`)
- [ ] CI green on the actual push, not just local verification
- [ ] `cargo package --list` reviewed — no accidental inclusion of
      scratch/dev-only files
- [ ] `cargo publish --dry-run` passes
- [ ] `CHANGELOG.md` has a dated entry for the release, not just
      `[Unreleased]`
- [ ] Version bumped in `Cargo.toml`, consistent with what actually
      shipped (0.1.0 for today's predicate/intersection/hull/Delaunay
      scope; 0.2.0 once triangulation topology + constrained Delaunay +
      polygon triangulation land — see ADR-006 and `tasks/todo.md`)
- [ ] `README.md`'s Status line and Maturity table match what's actually
      shipping, not aspirational

## Not required before a release, but should not be silently missing

- [ ] `examples/` build and run (`cargo build --examples`) — regression
      risk if a public API change breaks an example without CI catching it
      (examples are not currently run in CI, only built via the normal
      `cargo build`/`clippy --all-targets` steps, which do compile them)

## Explicitly out of scope for v0.1/v0.2

Per `tasks/todo.md`'s roadmap: polygon Boolean, exact Voronoi
construction, 3D triangulation, mesh Boolean/repair, surface
reconstruction, large-scale competitive benchmarking, long-duration
fuzzing, full CGAL differential verification. None of these block a
release of what already exists.
