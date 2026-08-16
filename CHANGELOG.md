# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- Repository skeleton, dual `MIT OR Apache-2.0` license, ADR-001..005.
- Exact expansion arithmetic core (`two_sum`, `fast_two_sum`, `split`,
  `two_product`, `grow_expansion`, `expansion_sum`, `expansion_sign`),
  verified against a `num-rational` dev-dependency oracle.
- `Point2`, `Point3` finite-coordinate types; `Sign`, `Orientation` result
  enums; `KikaError`.
- `orient2d`: floating-point filter with a computed error bound, falling
  back to exact expansion arithmetic. Checked against an independent
  exact-rational oracle in `tests/differential/orient2d.rs`.
- `scale_expansion` (expansion × scalar) exact-arithmetic primitive.
- `orient3d`: same filter + exact-fallback design as `orient2d`. Checked
  against an independent exact-rational oracle in
  `tests/differential/orient3d.rs`.

### Fixed

- `orient2d`/`orient3d` exact fallback now builds coordinate differences
  as exact expansions from the original `Point2`/`Point3` coordinates
  (`diff_expansion`, `product_of_expansions`) instead of reusing the
  filter's once-rounded `f64` subtraction. The old behavior was only
  exact relative to that rounding, not the true input coordinates, and
  could return the wrong sign for calls mixing widely different
  coordinate magnitudes (e.g. `2^60` alongside small integers); see
  `tests/regression/orient2d.rs` and `docs/numerical-model.md`.
