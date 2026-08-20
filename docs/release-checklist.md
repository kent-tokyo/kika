# Release checklist

0.7.1 release preparation in progress — 0.2.0 through 0.7.0 already
shipped; 0.7.1 is a bug-fix patch for the `delaunay2()` permutation-
inconsistency panic 0.7.0 shipped as a documented known issue (see
`CHANGELOG.md`'s `[0.7.1]` entry). `crates.io` publish, GitHub release,
and the `git push`/tag that precede them all require explicit user
approval (AGENTS.md §19, `tasks/todo.md`'s "Deferred pending explicit
user approval") regardless of how much of this checklist is green. See
this checklist's checkboxes for exactly what has and hasn't been
verified as of the 0.7.1 version-bump commit.

## Before any release

- [x] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
      warnings` (both native and `--target wasm32-unknown-unknown`),
      `cargo test --all-features` (unit/adversarial/differential/
      regression/doc, 369 tests, up from 360 at 0.7.0) all pass —
      verified at the 0.7.1 version-bump commit
- [x] `cargo +1.85 test --all-features` (MSRV) passes
- [x] `cargo build --target wasm32-unknown-unknown --release` passes
- [x] `wasm-pack test --node --release` passes (10/10 tests, actual
      Node.js execution, not just a wasm32 build)
- [x] `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` passes (every public
      item documented — enforced by `#![warn(missing_docs)]` +
      clippy's `-D warnings`)
- [x] `cargo build --examples` passes (10 examples, unchanged this
      release — 0.7.1 touches no public API)
- [ ] CI green on the actual push for the 0.7.1-scoped commit(s) — not
      yet pushed as of this checklist update; the checks above were run
      locally against the dirty working tree, not a clean worktree of a
      pushed commit
- [ ] `cargo package --list` reviewed for no accidental inclusion of
      scratch/dev-only files (`ROADMAP.md`, `.claude/` — both must **not**
      leak into the package) — deferred to after commit, per 0.7.0's own
      precedent of doing this from a clean worktree rather than the dirty
      dev tree
- [ ] `cargo publish --dry-run` passes — same reason, deferred to after
      commit (a dirty tree needs `--allow-dirty`, which this project
      avoids using for a real dry-run check)
- [x] `CHANGELOG.md` has a dated entry for the release
      (`[0.7.1] - 2026-08-20`), not just `[Unreleased]` — a "Fixed" entry
      for the `delaunay2()` panic, explicitly leaving the 0.7.0 "Known
      issues" entry above it untouched (accurate history of what was
      known at that release)
- [x] Version bumped in `Cargo.toml` to `0.7.1`
- [x] `README.md` (and `README_ja.md`/`README_zh.md`, all three, not just
      the English original): Status line updated to 0.7.1, the
      `delaunay2` bullet's "Known issue (0.7.0, unfixed)" note removed
      (now fixed), release-history line updated to "0.2.0 through 0.7.1"
- [x] `docs/compatibility.md` synced: 0.7.1 status framing, test count
      (369, was 360), the `delaunay2()` known issue's "Fixed in 0.7.1"
      status recorded with a pointer to the diagnosis, wasm-suite
      comparison count updated
- [x] `docs/architecture.md` Status line updated to 0.7.1; the ADR-009
      hardening-round paragraph's `delaunay2()` mention updated to point
      at the 0.7.1 fix instead of leaving it as "not fixed as part of
      this work" with no forward pointer
- [x] `docs/numerical-model.md`: new "Known limitation (fixed): split()
      overflow and two_sum overflow for sign-only predicates" section
      (full diagnosis, fix, and threshold derivations) plus a new "Known
      limitation: exact-product representability ceiling" section for
      the deliberately deferred harder case — cross-referenced with the
      existing floor/incircle-insphere-range sections rather than
      duplicating their numbers
- [x] `tasks/todo.md`: the "Discovered, not fixed (delaunay2() panics...)"
      section resolved to "Done (0.7.1: fix the delaunay2()
      permutation-inconsistency panic)"; two new, smaller "Discovered,
      not fixed" entries added for the two still-deferred cases found
      while diagnosing and fixing this one
- [x] `cargo deny check` green (advisories/bans/licenses/sources all ok)

## Not required before a release, but should not be silently missing

- [x] `examples/` build (`cargo build --examples`) — 10 examples build
      clean, unchanged this release (no public API surface touched)
- [x] `cargo deny check` green with no new runtime *or* dev dependency
      added for this fix (no new arithmetic/testing infrastructure needed
      beyond what 0.7.0 already had)
- [ ] `cargo fuzz run voronoi_geometry` (or `predicate_input_bytes`) for a
      short session, to add fuzzer-level confidence beyond the direct
      reproduction below — **not run**: this environment's `cargo-fuzz`
      requires a nightly toolchain, not installed here, and installing one
      was judged out of scope for this fix. Mitigated by direct,
      exhaustive verification against the real public API instead (see
      below) — both original fuzz-found repro coordinates and an
      independently-found second repro, checked across all 6 permutations
      of `orient2d` and through a bare `delaunay2()` call, in both debug
      and release builds
- [x] Both repro triples (the original fuzz-found one and a second,
      independently found while diagnosing it) verified directly against
      the patched public API — not just via the test suite — in both
      debug and release profiles, confirming: permutation-consistent
      `orient2d` output, no `delaunay2()` panic, and (for the separate,
      deliberately deferred product-ceiling case) unchanged release-mode
      behavior (silently self-consistent-but-wrong, matching pre-fix
      behavior — the "never panics in release" contract is preserved for
      that still-open case too)

## What 0.7.1 actually changes vs. 0.7.0

Per `CHANGELOG.md`: fixes the `delaunay2()` panic (`index out of bounds`)
on 3 input points with extreme, widely mixed coordinate magnitude,
documented as a known issue at 0.7.0. Root-caused to two independent
overflow sites in the exact-fallback arithmetic core
(`predicates::expansion`): `split()`'s `SPLITTER * a` overflow for
`|a| > f64::MAX/SPLITTER ~= 1.34e300`, and `two_sum`'s `a + b` overflow
for opposite-sign coordinates near `f64::MAX`. Both silently produced a
`NaN` that `expansion_sign` read as `Sign::Zero`
(`Orientation::Collinear`), breaking `orient2d`'s permutation-consistency
guarantee. Fixed via a magnitude-safe `split()` and a new
`rescale_for_sign_only` helper for the 4 sign-only predicates
(`orient2d`/`orient3d`/`incircle`/`insphere`); `circumcenter`/
`line_intersection` are unaffected (they need real magnitudes back, kept
their own existing approach). `expansion_sign` gained a debug-only NaN
guard, scoped to a new `sign_only_expansion_sign` wrapper so it doesn't
break `circumcenter`/`line_intersection`'s own, different, already-correct
overflow handling. No public API signature changes — pure bug fix,
correctly a patch release.

**Two structurally different, harder cases remain deliberately
deferred**, not fixed this round: `two_product`'s own `a * b` overflow
when both operands are independently `> ~sqrt(f64::MAX) ~= 1.34e154` (a
genuine representability ceiling, not fixable by rescaling alone — needs
a different arithmetic architecture), and a narrower `split()` residual
within `~2^-26` of `±f64::MAX` itself (a rounding-carry limit). Neither
causes a panic in release builds; both are documented in
`docs/numerical-model.md` and tracked in `tasks/todo.md`.

## Explicitly out of scope for 0.7.1

Per `ROADMAP.md` (internal, untracked): 0.8.0's nearest-site query and
everything after it in the roadmap — this round was scheduled ahead of
0.8.0 specifically because it's a correctness fix to the predicates layer
0.8.0 would otherwise build on top of, not itself new roadmap scope. Also
out of scope, deliberately: the two deferred representability-limit cases
above (need a different arithmetic architecture, not a patch-sized fix);
an actual `cargo fuzz` run (nightly toolchain not available in this
environment; mitigated by direct verification, see above); a playground/
visualization tool (raised and discussed this session as a good next
investment, but explicitly deferred to its own round after this fix,
per the user's own prioritization).
