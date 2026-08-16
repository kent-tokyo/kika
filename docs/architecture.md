# Architecture

Status: reflects Phase 1 + Phase 2 + Phase 3 + Phase 4 (complete). Updated
per-phase, not written ahead of the code it describes.

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
├── intersections/
│   └── segment2.rs         # segment_intersection_kind (predicate) /
│                            # segment_intersection (construction), split per §4.2
├── polygon/
│   └── polygon2.rs          # Polygon2: signed_area (f64), orientation (exact),
│                             # basic_validity, find_self_intersection
├── hull/
│   └── convex_hull2.rs       # convex_hull2: Andrew monotone chain, built
│                              # entirely from orient2d — no new coordinates
│                              # constructed, so the whole algorithm is exact
└── triangulation/
    └── delaunay2.rs           # delaunay2: Bowyer-Watson incremental insertion,
                                # a single symbolic "point at infinity" ghost
                                # vertex instead of a synthetic bounding triangle
                                # — no new coordinates constructed here either
```

`predicates/constructions/`, `topology/` are empty placeholders reserved by
§6's tree until the phase that fills them (Phase 5 onward).

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
5. **Polygon** (`polygon::Polygon2`) — the same layering *within* one
   type: `signed_area()` is a plain `f64` construction (a number, not a
   sign — no exactness claim, matches `segment_intersection`'s `Proper`
   case); `orientation()` is a genuine exact predicate, reusing the same
   `expansion`/`merge_all` machinery as layer 1/2 to sum every edge's
   shoelace term exactly rather than trust a running `f64` sum (which
   could round through cancellation for a near-degenerate polygon).
   `basic_validity()`/`find_self_intersection()` compose layers 2–4, same
   as layer 3.
6. **Convex hull** (`hull::convex_hull2`) — Andrew monotone chain, built
   entirely from `orient2d` turn tests plus an input sort. Unlike layers 3–5,
   this algorithm's output is *fully exact*, not just its component
   predicate calls: every returned vertex is copied from an original input
   `Point2`, never a computed/interpolated coordinate — there is nothing
   here analogous to `segment_intersection`'s non-exact `Proper` case. The
   fully collinear input case is detected explicitly with its own `orient2d`
   precheck up front, rather than inferred from the chain construction's
   output length — a length-based heuristic (e.g. "the lower chain used
   every point") is not reliable, since a genuinely 2D "valley" point set
   can legitimately do the same thing without being collinear (ruled out by
   a concrete counterexample during design; see `docs/degeneracy-policy.md`
   and `tasks/lessons.md`).
7. **Delaunay triangulation** (`triangulation::delaunay2`) — Bowyer-Watson
   incremental insertion. Also fully exact, like layer 6, but by a
   different means: rather than a synthetic bounding coordinate,
   "outside the current triangulation" is represented by a single
   symbolic ghost vertex with no coordinate at all, always maintaining a
   closed triangle fan around it (exactly like any real interior point
   would). A triangle carrying the ghost reduces its circumcircle test to
   an exact `orient2d` half-plane check against its one real edge. An
   earlier design used a synthetic "super-triangle" coordinate instead;
   that version passed every hand-written unit test but a property test
   on ordinary (non-adversarial) random input found it silently dropping
   a triangle, because whether a super-triangle vertex shields a real
   edge is scale-dependent with no universally-safe multiplier. See
   `docs/numerical-model.md`'s Phase 4 section, `tasks/lessons.md`, and
   `tests/regression/delaunay2.rs` for the full trail.
8. **Topology algorithms** do not exist yet (Phase 6 onward).

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
