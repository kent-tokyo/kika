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
