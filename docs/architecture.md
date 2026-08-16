# Architecture

Status: reflects Phase 1 + Phase 2 (in progress). Updated per-phase, not
written ahead of the code it describes.

## Crate layout

Single crate (`kika`), per AGENTS.md §6. Split into crates only if
compile-time, optional-dependency boundaries, WASM packaging, API
stability, or reuse actually require it (§6) — not preemptively.

```text
src/
├── lib.rs               # public re-exports only
├── error.rs              # KikaError
├── primitives/           # Point2/3, Vector2/3, Segment2, Triangle2/3, Aabb2/3
│                          # + point-on-segment, point-in-triangle (query methods)
├── predicates/
│   ├── expansion.rs       # exact arithmetic core: two_sum, split, two_product,
│   │                      # expansion_sum, scale_expansion, product_of_expansions
│   ├── sign.rs             # Sign, Orientation enums
│   ├── orient2d.rs
│   ├── orient3d.rs
│   ├── incircle.rs
│   └── insphere.rs
└── intersections/
    └── segment2.rs         # segment_intersection_kind (predicate) /
                             # segment_intersection (construction), split per §4.2
```

`predicates/constructions/`, `polygon/`, `hull/`, `triangulation/`,
`topology/` are empty placeholders reserved by §6's tree until the phase
that fills them (Phase 2 (polygon/) onward).

## Layering (§4.2)

1. **Exact arithmetic core** (`predicates::expansion`) — no geometric
   meaning, just error-free floating-point transformations and
   nonoverlapping expansions. Reused by every predicate, and by future
   exact constructions (ADR-004).
2. **Exact Predicates** (`predicates::{orient2d,orient3d,incircle,insphere}`)
   — each is: compute a fast filtered estimate with a computed error bound;
   if inconclusive, recompute via the exact arithmetic core and take the
   sign of the resulting expansion's most significant component.
3. **Geometric queries built on predicates** (`primitives::{Segment2,
   Triangle2}::relation_to`) — compose one or more calls to layer 2 into a
   richer classification (point-on-segment, point-in-triangle). Each
   degenerate case (zero-length segment, collinear triangle) needs its
   *own* explicit handling — composing exact primitives does not
   automatically make the composition's edge cases exact; two of this
   layer's degenerate cases were wrong on first implementation and caught
   by testing, not derivation (see `docs/degeneracy-policy.md`).
4. **Intersections** (`intersections::segment2`) — the same
   compose-and-verify pattern as layer 3, one level more involved
   (AABB-reject fast path, then a branching decision tree over several
   layer-2/3 calls). Predicate (`segment_intersection_kind`) and
   construction (`segment_intersection`) are separate functions per §4.2
   — the predicate never divides or builds a new coordinate; only the
   `Proper`-crossing construction case does (and is documented as
   non-exact, Phase 5 territory — every other construction case reuses an
   original input coordinate exactly, since it corresponds to an actual
   shared point).
5. **Polygon, hull, triangulation, topology algorithms** do not exist yet
   (Phase 2's polygon type onward).

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

## Data flow for a composed query (segment intersection)

```text
segment_intersection_kind(s1, s2) -> SegmentIntersectionKind
        │
        ├─ Aabb2::overlaps fast-reject (no predicate calls at all)
        ├─ zero-length segment(s): explicit case, via Segment2::relation_to
        ├─ orient2d ×2..4 + Segment2::relation_to: classify
        └─ never divides, never builds a new Point2

segment_intersection(s1, s2) -> SegmentIntersection2   (separate call)
        │
        └─ re-derives the same classification, then:
           Proper        -> divides, builds a new (non-exact) Point2
           EndpointTouch,
           CollinearTouch,
           CollinearOverlap -> reuses original input point(s), exact
```
