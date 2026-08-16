# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- Repository skeleton, dual `MIT OR Apache-2.0` license, ADR-001..005.
- Exact expansion arithmetic core (`predicates::expansion`, internal):
  `two_sum`, `split`, `two_product`, `product_expansion`,
  `diff_expansion`, `expansion_sum`, `scale_expansion`,
  `product_of_expansions`, `expansion_sign` — verified against a
  `num-rational` dev-dependency oracle, never used from production code
  (ADR-005).
- `Point2`, `Point3` finite-coordinate types; `Sign`, `Orientation` result
  enums; `KikaError`.
- `orient2d`, `orient3d`, `incircle`, `insphere`: exact-sign geometric
  predicates. Each uses a floating-point filter with a *computed* error
  bound (never a fixed epsilon), falling back to exact expansion
  arithmetic — built from the original coordinates, not a once-rounded
  intermediate — when the filter is inconclusive. `incircle`/`insphere`
  have narrower verified-safe coordinate-magnitude ranges than
  `orient2d`/`orient3d` (higher polynomial degree from the paraboloid
  lift); see `docs/numerical-model.md`. Each checked against an
  independent exact-rational oracle in `tests/differential/`.
- CI (`.github/workflows/ci.yml`): `cargo fmt --check`, clippy
  (`-D warnings`), test matrix (Linux/macOS/Windows), MSRV (1.85) check,
  `wasm32-unknown-unknown` build, `cargo doc` (warnings denied),
  `cargo-deny` (license + security-advisory check, `deny.toml`).
- `Vector2`, `Vector3` finite-coordinate displacement types, with
  `Point ± Vector -> Point` / `Point - Point -> Vector` affine arithmetic
  and vector `+`/`-`/negate/`* f64`. Point equality policy formalized as
  exact coordinate equality (ADR-003).

### Fixed

*(bugs found and fixed during this same initial implementation, before
any release — noted for the record per AGENTS.md §18/§20, not because
anything public regressed)*

- `orient2d`/`orient3d` exact fallback used to reuse the filter's
  once-rounded coordinate difference (e.g. `a.x()-c.x()` as a plain `f64`
  subtraction) instead of recomputing it exactly from the original
  coordinates. Could return the wrong sign for calls mixing widely
  different coordinate magnitudes (e.g. `2^60` alongside small integers);
  see `tests/regression/orient2d.rs` and `docs/numerical-model.md`.
- `orient3d`/`incircle` floating-point filters used to bound their error
  using each cofactor's post-subtraction term magnitude instead of the
  pre-subtraction magnitudes, which could silently underestimate the true
  error when an inner cofactor subtraction cancelled catastrophically;
  see `tests/regression/incircle.rs` and `docs/numerical-model.md`.
- Expansion combination (`expansion_sum`/`scale_expansion`/
  `product_of_expansions`) used to fold pieces left-to-right into a
  single growing accumulator, which is O(count²) regardless of how fast
  each individual merge step is. Made `insphere`'s exact fallback take
  16s/call on degenerate inputs. Fixed with a linear-time `expansion_sum`
  (merge-by-magnitude + single `two_sum` cascade) plus balanced
  binary-tree combination instead of a linear fold; see
  `docs/numerical-model.md`.
