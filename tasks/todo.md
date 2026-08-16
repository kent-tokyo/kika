# Todo

## Done (Phase 0 + Phase 1 + Phase 2 + Phase 3)

- [x] Phase 0: name-collision check, ecosystem survey, ADR-001..005
- [x] Expansion arithmetic core (`two_sum`, `split`, `two_product`,
      `product_expansion`, `diff_expansion`, `expansion_sum`,
      `scale_expansion`, `product_of_expansions`, `expansion_sign`,
      `merge_all`)
- [x] `orient2d`, `orient3d`, `incircle`, `insphere` — filter + exact
      fallback, each checked against an independent exact-rational oracle
- [x] CI workflow (`.github/workflows/ci.yml`): fmt, clippy, test matrix
      (Linux/macOS/Windows), MSRV (1.85), wasm32 build, `cargo doc`,
      `cargo deny` (license + advisory check, `deny.toml`)
- [x] `Vector2`/`Vector3`, `Segment2`, `Triangle2`/`Triangle3`,
      `Aabb2`/`Aabb3`; point equality policy formalized (ADR-003)
- [x] `Segment2::relation_to` (point-on-segment), `Triangle2::relation_to`
      (point-in-triangle), `Triangle2::orientation`
- [x] `segment_intersection_kind` / `segment_intersection` (robust 2D
      segment intersection, predicate/construction split)
- [x] `Polygon2`: `signed_area`, `orientation` (exact), `basic_validity`,
      `find_self_intersection`
- [x] Six real bugs found and fixed during implementation (see
      `tasks/lessons.md` for the diagnostic trail):
      1. exact fallback wasn't exact relative to the original coordinates
      2. `orient3d`/`incircle` filter bound used post-cancellation
         magnitudes
      3. naive expansion merging was O(count²), making `insphere`'s
         exact fallback take 16s/call
      4. `Triangle2::relation_to` couldn't tell "within a degenerate
         triangle's span" from "same line, far outside it"
      5. (test-authoring trap, not a library bug) `sqrt()`-based
         "exactly cospherical" test coordinates weren't actually exact
      6. (doc bug, caught before writing) assumed insphere's coplanar
         case was analogous to incircle's collinear case; verified
         first, found it was wrong (needs concyclic, not just coplanar)
      7. (design-time bugs, caught by hand-tracing/review before writing
         code) Phase 3's naive monotone chain self-retraces on fully
         collinear input in "keep all boundary" mode; a proposed
         post-hoc collinearity heuristic (chain length) has a false
         positive on "valley" point sets; a `total_cmp` sort without
         signed-zero normalization can make `dedup()` miss a real
         duplicate — see `tasks/lessons.md`
- [x] `hull::convex_hull2` (Andrew monotone chain): `HullBoundaryPoints`
      (`ExtremesOnly`/`KeepAllOnBoundary`), CCW output starting at the
      lexicographically smallest input point, exact throughout (every
      returned vertex is a copied input coordinate — no division, no
      interpolation, unlike `segment_intersection`'s `Proper` case).
      Checked via structural property tests (containment, hull vertices
      are input points, convexity/winding, permutation invariance,
      idempotence) against `orient2d`/`Segment2::relation_to`, not a
      from-scratch `BigRational` reimplementation — see
      `tests/differential/convex_hull2.rs`'s module doc for why.

## Known gaps, not yet closed (see docs/compatibility.md)

- [ ] CI workflow added but not yet exercised by an actual push/PR run —
      "should pass" based on local verification, not CI-confirmed
- [ ] wasm32: build verified, but no test execution under wasm32 (needs
      `wasm-bindgen-test`/`wasmtime`) — the "Rust never contracts +/-/*
      into FMA" argument in ADR-001 is a language guarantee, not
      re-verified empirically on this target
- [ ] `incircle`/`insphere` safe-magnitude-range bounds
      (`docs/numerical-model.md`) are empirically-checked, not tightly
      derived on the floor side
- [ ] `segment_intersection`'s `Proper` case is not a certified/exact
      construction (ordinary `f64` parametric interpolation) — Phase 5
      territory, not skipped ahead of, but a real gap for callers who
      need exact intersection coordinates today

## Backlog (later phases, not started)

- [ ] Phase 4: 2D Delaunay triangulation
- [ ] Phase 5: exact construction model (re-open ADR-004; this is also
      where `segment_intersection`'s `Proper` case gets a real fix)
- [ ] Phase 6: constrained Delaunay, polygon Boolean
- [ ] CGAL differential-test harness (separate program, §10)
- [ ] fuzz targets (§12) — none yet; differential/regression tests so far
      are hand-written and randomized, not coverage-guided fuzzing
- [ ] benches (§13) — predicate fast-path/fallback rate measurement not
      yet built; no performance numbers exist beyond the ad hoc timing
      used to catch and confirm the O(count²) merge bug
- [ ] Shewchuk-style multi-tier adaptive precision (ADR-001 "Revisit"),
      gated on measured fallback rate from real (non-adversarial) usage
- [ ] `Polygon2::orientation()` has no fast float filter ahead of the
      exact path (ponytail-documented simplification in
      `predicates::polygon2`) — add one if profiling ever shows it
      matters for large polygons

## Deferred pending explicit user approval (§19)

- [ ] crates.io publish
- [ ] GitHub release / repo visibility change
- [ ] Any new runtime (non-dev) dependency
