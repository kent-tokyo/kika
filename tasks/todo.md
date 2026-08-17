# Todo

## Done (Phase 0 + Phase 1 + Phase 2 + Phase 3 + Phase 4 + Phase 5 + Phase 6A + Phase 6B + Phase 6C + Phase 6D)

- [x] Phase 0: name-collision check, ecosystem survey, ADR-001..005
- [x] Expansion arithmetic core (`two_sum`, `split`, `two_product`,
      `product_expansion`, `diff_expansion`, `expansion_sum`,
      `scale_expansion`, `product_of_expansions`, `expansion_sign`,
      `merge_all`)
- [x] `orient2d`, `orient3d`, `incircle`, `insphere` — filter + exact
      fallback, each checked against an independent exact-rational oracle
- [x] CI workflow (`.github/workflows/ci.yml`): fmt, clippy, test matrix
      (Linux/macOS/Windows), MSRV (1.85), wasm32 build, `cargo doc`,
      `cargo deny` (license + advisory check, `deny.toml`). Confirmed
      green on an actual push, not just locally, once a GitHub remote
      (`kent-tokyo/kika`) was created and the first 3 phase commits
      were pushed — previously only locally verified.
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
- [x] `triangulation::delaunay2` (Bowyer-Watson): `Triangulation2`, exact
      throughout via a single symbolic "point at infinity" ghost vertex
      instead of a synthetic bounding-triangle coordinate (no scale-
      dependent tradeoff anywhere — verified down to a `1e-200`
      perpendicular cluster spread). Cocircular-point tie-break
      documented (`Sign::Zero` circumcircle boundary is not "bad").
      Checked via structural property tests (empty-circumcircle property,
      CCW/non-degenerate triangles, watertight mesh matching the convex
      hull, Euler's formula, permutation invariance) — see
      `tests/differential/delaunay2.rs`.
- [x] A real bug, this one caught by property testing on ordinary
      (non-adversarial) input rather than by hand-tracing or an
      adversarial construction: an initial `delaunay2` design using a
      synthetic super-triangle silently dropped a triangle for a plain
      4-point input, because whether the super-triangle shields a real
      edge is scale-dependent with no safe fixed multiplier. Fixed by
      removing the synthetic coordinate entirely (single ghost vertex,
      see above) — see `tasks/lessons.md` for the full diagnostic trail,
      including a design mistake in the first fix attempt (three ghosts
      instead of one) caught by hand-tracing before it shipped.
- [x] ADR-004 decided: kept `Point2` a plain `f64` pair, chose
      `float+certificate` (correctly-rounded division from exact
      expansions) over a new exact-coordinate type — see the ADR's
      "Decision for Phase 5" section for the rejected alternatives and why.
- [x] `predicates::line_intersection` (internal): the crate's first
      exact/certified construction, closing `segment_intersection`'s
      `Proper`-case exactness gap. Reuses `orient2d`'s exact-fallback
      machinery for the numerator/denominator (degree 3, not a fresh
      determinant); `correctly_rounded_divide` resolves the one
      unavoidable division to the provably nearest `f64`. Verified against
      an independent `BigRational` "correctly-rounded nearest `f64`"
      oracle in `tests/differential/line_intersection.rs` (magnitude
      scales, mixed-magnitude inputs, an empirical floor sweep down to
      `2^-335`) — see `docs/numerical-model.md`.
- [x] A wrong a priori assumption, caught by measurement before it shipped
      as documentation: assumed the construction's safe magnitude range
      would be *narrower* than `incircle`'s (more multiplications felt
      riskier); the empirical floor sweep showed it's *wider* — degree (3
      vs. `incircle`'s 4), not "predicate vs. construction", governs the
      floor. See `tasks/lessons.md`.
- [x] The refinement loop's iteration bound (`0..8`) was unverified when
      first written — advisor review flagged it as the same class of risk
      as the super-triangle scale constant (an unverified assumption on a
      correctness-critical path). Measured via
      `divide_loop_iteration_bound_is_generous`: worst case observed is 2
      iterations (ordinary + deliberately near-parallel crossings across
      `1e-300`..`1e100`), 4x below the bound — see `tasks/lessons.md`.
- [x] Phase 6A: release-quality polish — `#![forbid(unsafe_code)]`,
      `#![warn(missing_docs)]` (all 52 previously-undocumented public
      items now documented), `examples/` (5 runnable examples), package
      metadata (`homepage`/`documentation`), `docs/release-checklist.md`,
      README maturity table, and fixed several trust-affecting staleness
      issues found by re-reading the crate's own public-facing docs:
      `Cargo.toml`'s `repository` pointed at a nonexistent org, and
      `README.md`/`docs/compatibility.md` both described the CGAL
      differential-test harness and CI as further along than they
      actually were.
- [x] Phase 6B: `Triangulation2` adjacency structure (ADR-006, indexed
      triangle adjacency — not half-edge/quad-edge). `VertexId`/`EdgeId`/
      `FaceId` plus `vertices`/`edges`/`faces`/`edge_vertices`/
      `adjacent_faces`/`face_vertices`/`neighboring_faces`/
      `boundary_edges`, all `pub`, additive to the existing `triangles()`
      contract. Internal `validate_topology` (CCW, edge-manifold
      incidence recomputed independently rather than trusting its own
      cached tables, adjacency reciprocity, Euler's formula, per-edge
      local-Delaunay) is `pub` + `#[doc(hidden)]` — not `pub(crate)`,
      since this repository's own `tests/` and `fuzz/` are separate
      crates for Rust visibility purposes and couldn't otherwise reach it.
      A static, post-construction snapshot: no generational-ID arena
      needed (ADR-006's arena proposal is scoped to construction-time
      mutation, which this phase didn't touch — `insert_point` is
      unchanged). Deliberately caught a self-inflicted stale-build-cache
      false negative during development (a real code path silently wasn't
      being recompiled) by re-testing after a clean rebuild rather than
      trusting the first red result at face value.
- [x] fuzz targets (§12), first pass — 4 libFuzzer targets under `fuzz/`
      (`segment_intersection`, `convex_hull`, `delaunay_insert`,
      `triangulation_topology_validator`), prioritizing the combinatorial
      algorithms over predicates (already covered by thick differential/
      adversarial suites). Inputs map to a small-integer coordinate grid
      rather than raw byte-to-`f64`, deliberately: continuous random floats
      almost never produce the duplicate/collinear/cocircular
      configurations that stress combinatorial logic, so a grid makes those
      common instead of vanishingly rare — see `fuzz/fuzz_targets/common.rs`.
      Short bounded runs only (60-90s each, ~1.65M total executions), not
      unbounded/nightly-scale fuzzing per AGENTS.md §11's "重い測定を通常
      の開発ループで繰り返さない" — no crashes found across all 4 targets,
      including `triangulation_topology_validator`'s edge-connectivity
      (every edge used by exactly 1 or 2 triangles) and Euler's-formula
      checks. Remaining `predicate input bytes`/`polygon parser`/
      `polygon validity` targets from AGENTS.md §12's full list not yet
      added — out of scope for this pass, which targeted the topology/
      algorithm layer specifically.
- [x] Phase 6C: constrained Delaunay (narrow scope — non-crossing
      constraints between existing input vertices only; no automatic
      constraint splitting, Steiner points, refinement, or quality
      meshing). `constrained_delaunay2`/`ConstrainedTriangulation2`/
      `CdtError`. Confirms ADR-004's Phase 6 re-evaluation prediction:
      segment recovery is done entirely by flipping existing Delaunay
      edges, never building a new coordinate — CDT needed zero new
      construction machinery. Both flip passes (constraint recovery,
      unconstrained-Delaunay restoration) are bounded
      (`4 * face_count + 16`, measured — worst case 9/3 flips across a
      spread of random test configurations, well under the ~72 bound for
      those sizes) rather than looping to convergence unbounded, matching
      Phase 5's `correctly_rounded_divide` discipline. A candidate flip
      edge is defensively excluded if it's already a realized constraint
      from an earlier constraint in the same call (belt-and-suspenders:
      the upfront pairwise non-crossing validation should already make
      this unreachable, but `crossing_faces` no longer trusts that
      argument silently) — added after advisor review flagged the gap and
      a dedicated multi-constraint test
      (`multiple_constraints_each_needing_a_flip_all_survive`) confirmed
      the fix. 15 unit tests, including the load-bearing
      `constrained_edge_survives_even_when_not_locally_delaunay`
      (proves the exclusion logic actually matters, not vacuously true).
- [x] Phase 6D: simple-polygon triangulation via Phase 6C's CDT
      (`triangulate_polygon`/`PolygonTriangulationError`). No holes, no
      Steiner points, self-intersecting input rejected as a typed error
      (via the same `Polygon2::basic_validity`/`find_self_intersection`
      checks `Polygon2` already had). Constrain every polygon edge via
      CDT, then discard the concave-pocket faces via a purely topological
      flood fill from one interior seed face — identified by a single
      `orient2d` check against an existing triangle vertex, never a
      constructed point. Accepts both CCW and CW input; deterministic
      regardless of starting vertex (verified by comparing the full
      triangle set, not just total area). Advisor review flagged that the
      initial test suite only ever exercised a seed edge with 1 incident
      face (a hull edge, trivially unambiguous) — added
      `seed_edge_with_two_incident_faces_still_finds_the_interior_side`
      (seed edge is a chord, 2 incident faces, disambiguation actually
      load-bearing) and `plus_shape_discards_all_four_separate_pockets`
      (4 disconnected pockets, not just 1). Also found and documented:
      `Triangulation2::validate_topology()`'s Euler-characteristic check
      assumes full convex-hull coverage — false for a non-convex
      polygon's output — see `docs/degeneracy-policy.md`.

## Known gaps, not yet closed (see docs/compatibility.md)

- [ ] The 4 fuzz targets added so far ran clean on short (60-90s) local
      runs only — no coverage-guided corpus persisted across runs, no
      nightly/long-duration run performed yet, no `predicate input bytes`/
      `polygon parser`/`polygon validity` targets from AGENTS.md §12's list
- [ ] wasm32: build verified, but no test execution under wasm32 (needs
      `wasm-bindgen-test`/`wasmtime`) — the "Rust never contracts +/-/*
      into FMA" argument in ADR-001 is a language guarantee, not
      re-verified empirically on this target
- [ ] `incircle`/`insphere` safe-magnitude-range bounds
      (`docs/numerical-model.md`) are empirically-checked, not tightly
      derived on the floor side
## Backlog (later phases, not started)

- [ ] Phase 6 (polygon Boolean, overlay): ADR-004's Phase 6 re-evaluation
      found Phase 6b/overlay needs a lazily-exact representation,
      expansion-backed homogeneous coordinates leading, rational-backed as
      an approval-gated fallback — neither implemented, both explicitly
      left open pending the overlay algorithm's actual needs. Not started,
      deliberately after 6C/6D per the user's explicit sequencing.
- [ ] CGAL differential-test harness (separate program, §10) — currently
      environment-blocked, not just unstarted: CGAL/pkg-config are not
      installed in this development environment
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
- [ ] Any new runtime (non-dev) dependency, including specifically:
      `num-bigint`/`num-rational` (or similar) promoted from dev-only
      (ADR-005) to a genuine runtime dependency, as the fallback if
      expansion-backed homogeneous coordinates prove insufficient for
      Phase 6b's polygon-overlay construction needs — see ADR-004's
      "Phase 6 re-evaluation" section
