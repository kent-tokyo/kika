# Compatibility

Status: Phase 1.

## Platforms (CI-verified, §16)

* `x86_64` — Linux, macOS, Windows
* `aarch64` — best-effort per §16 ("aarch64可能範囲")
* `wasm32-unknown-unknown` — library build verified in CI; no OS-specific
  code exists in Phase 1, so no WASM-specific gaps are currently known.

## MSRV

Rust 1.85 (edition 2024). Fixed in `Cargo.toml` (`rust-version`) and
checked in CI.

## CGAL

Kika does not link against or depend on CGAL. CGAL is used only as an
external differential-test oracle in a separate comparison program, never
part of the `kika` crate itself (§10).

## Stability

Pre-1.0. No stability guarantees yet. Public API surface as of Phase 1:
`Point2`, `Point3`, `Sign`, `Orientation`, `KikaError`, `orient2d`,
`orient3d`, `incircle`, `insphere`.
