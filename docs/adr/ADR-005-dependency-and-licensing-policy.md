# ADR-005: Dependency and licensing policy

Status: Accepted

## Name collision check (§9 Phase 0, item 1)

Checked 2026-08-16:

* crates.io: `kika` — unclaimed (`GET /api/v1/crates/kika` → 404 "crate
  `kika` does not exist").
* GitHub: no organization/repository providing a Rust computational
  geometry library named exactly `kika`. A GitHub user named `kika` exists
  (unrelated font repo) and unrelated repos containing the substring "kika"
  exist (e.g. `kikar-hamedina`, `Kikakuka`); none conflict with a
  `kika` crate namespace.
* npm: `kika` — unclaimed (404).
* PyPI: `kika` — unclaimed (404).

No blocking collision found. `kika` is used as the crate name.

## Existing Rust ecosystem survey (§9 Phase 0, item 2)

| Crate | Relevance | Reuse decision |
|---|---|---|
| `geo` / `geo-types` | 2D geometry types + algorithms, huge adoption | Not a dependency. Kika's `Point2` etc. are intentionally minimal and carry the finite-input invariant (ADR-003); `geo-types` does not. Interop adapter is a possible future *optional* feature, not a Phase 1 concern. |
| `spade` | Robust 2D Delaunay/CDT in Rust, uses `robust`-style predicates internally | Reference for Phase 4 algorithm choice, not a dependency. Reimplementing predicates ourselves is the explicit point of this project (§1: predicates/exact fallback are the product, not incidental plumbing). |
| `parry` | 2D/3D collision/query library (rapier ecosystem) | Not a dependency — pulls in a much larger surface (bounding volume hierarchies, shape casting) than Kika needs. Worth re-checking once Kika has AABBs/BVH needs. |
| `nalgebra` | Linear algebra | Not a dependency for Phase 1 (predicates need only scalar `f64` arithmetic). An optional `nalgebra`/`glam` interop adapter is explicitly anticipated by §14 but not built until a concrete downstream need exists. |
| `rgeometry` | Computational geometry with a focus on certified/robust algorithms | Closest philosophical neighbor. Not a dependency (same reasoning as `spade`) — Kika's predicate layer is the thing being built, not imported. |
| `robust` (Shewchuk-style predicates, `georust`) | Existing Rust port of Shewchuk's adaptive predicates | Not a dependency and not copied from. Kika deliberately implements its own expansion arithmetic (ADR-001) as the core deliverable of Phase 1; using `robust` directly would defeat the purpose of this project. Algorithmic technique (Shewchuk 1997 paper) is the shared ancestor, not this crate's source. |

No CGAL source, GPL code, or license-unclear code has been copied,
translated, or ported into Kika, per AGENTS.md §3. Where an algorithm
matches published literature (Dekker 1971 error-free transformations,
Shewchuk 1997 adaptive precision predicates), it is implemented from the
published technique description, not from any specific implementation's
source code.

## Runtime dependency policy

* Zero runtime dependencies in Phase 1. The predicate core needs nothing
  beyond `core`/`std` `f64` arithmetic.
* `num-bigint` + `num-rational` are **dev-dependencies only**, used
  exclusively by the differential-test oracle (`tests/differential/`) to
  independently verify predicate signs via exact rational arithmetic. They
  are never imported by `src/`.
* Any future runtime dependency addition must record: license, maintenance
  status, WASM (`wasm32-unknown-unknown`) support, MSRV compatibility,
  `unsafe` usage, and transitive dependency count, per §14.
* GMP/MPFR/C-FFI are not used, per §3/§14.

## License

Kika is dual-licensed `MIT OR Apache-2.0`, per §3. `LICENSE-MIT` and
`LICENSE-APACHE` are included at the repository root; `Cargo.toml` declares
`license = "MIT OR Apache-2.0"`.
