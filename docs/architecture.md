# Architecture

Status: reflects Phase 1 only. Updated per-phase, not written ahead of the
code it describes.

## Crate layout

Single crate (`kika`), per AGENTS.md §6. Split into crates only if
compile-time, optional-dependency boundaries, WASM packaging, API
stability, or reuse actually require it (§6) — not preemptively.

```text
src/
├── lib.rs              # public re-exports only
├── error.rs             # KikaError
├── primitives/          # Point2, Point3 (Phase 1); Vector/Segment/Triangle/Aabb (Phase 2+)
└── predicates/
    ├── expansion.rs      # exact arithmetic core: two_sum, split, two_product, grow_expansion
    ├── sign.rs            # Sign, Orientation enums
    ├── orient2d.rs
    ├── orient3d.rs
    ├── incircle.rs
    └── insphere.rs
```

`predicates/constructions/`, `intersections/`, `polygon/`, `hull/`,
`triangulation/`, `topology/` are empty placeholders reserved by §6's tree
until the phase that fills them (Phase 2–6).

## Layering (§4.2)

1. **Exact arithmetic core** (`predicates::expansion`) — no geometric
   meaning, just error-free floating-point transformations and
   nonoverlapping expansions. Reused by every predicate, and by future
   exact constructions (ADR-004).
2. **Exact Predicates** (`predicates::{orient2d,orient3d,incircle,insphere}`)
   — each is: compute a fast filtered estimate with a computed error bound;
   if inconclusive, recompute via the exact arithmetic core and take the
   sign of the resulting expansion's most significant component.
3. **Constructions** and **Topology algorithms** do not exist yet
   (Phase 2+).

## Data flow for a predicate call

```text
Point2::new(x, y) -> Result<Point2, KikaError>   (finiteness checked here, once)
        │
        ▼
orient2d(a: Point2, b: Point2, c: Point2) -> Orientation   (never panics, never fails)
        │
        ├─ filter: f64 determinant + computed error bound → conclusive? return.
        └─ fallback: expansion-arithmetic exact determinant → sign of leading term.
```
