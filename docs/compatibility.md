# Compatibility

Status: Phase 1.

## Platforms

* `aarch64-apple-darwin` — full test suite (109 tests across unit,
  differential, adversarial, regression, doctests) run locally on this
  target during development, not just built.
* `x86_64`, `aarch64` Linux/macOS/Windows — `.github/workflows/ci.yml`
  runs the full test suite on `ubuntu-latest`, `macos-latest` (Apple
  Silicon on current GitHub-hosted runners), `windows-latest`. Not yet
  verified by an actual CI run (workflow added but not yet exercised by
  a push/PR) — treat as "should work" until a real CI run confirms it.
* `wasm32-unknown-unknown` — library **builds** successfully, verified
  both locally and in CI (`cargo build --target wasm32-unknown-unknown
  --lib`). Tests do **not** run under wasm32 — that needs a WASM test
  runner (`wasm-bindgen-test` + a JS engine, or `wasmtime`), not set up
  in Phase 1. This matters specifically for the exact-arithmetic core's
  correctness claims (ADR-001's argument that Rust never contracts
  `+`/`-`/`*` into FMA is a language-level guarantee, not empirically
  re-verified under wasm32) — a known gap, not silently assumed fine;
  see `tasks/todo.md`.

## MSRV

Rust 1.85 (edition 2024). Fixed in `Cargo.toml` (`rust-version`).
Actually verified locally (`cargo +1.85 test --all-features`, full suite
green), not just declared; also checked in CI.

## CGAL

Kika does not link against or depend on CGAL. CGAL is used only as an
external differential-test oracle in a separate comparison program, never
part of the `kika` crate itself (§10).

## Stability

Pre-1.0. No stability guarantees yet. Public API surface as of Phase 1:
`Point2`, `Point3`, `Sign`, `Orientation`, `KikaError`, `orient2d`,
`orient3d`, `incircle`, `insphere`.
