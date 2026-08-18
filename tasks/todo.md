# Todo

## Done (ADR-007 Phase 7B: Voronoi public query API)

- [x] `Voronoi2`/`voronoi2()`/`VoronoiCellId`/`VoronoiVertexId`/
      `VoronoiEdgeId`/`VoronoiEdgeKind`/`VoronoiEdge` re-exported at
      `triangulation::mod.rs` and the crate root. Query API:
      `cells()`/`vertices()`/`edges()`, `cell_site()`, `edge_cells()`,
      `edge_kind()`, `dual_delaunay_edge()`, `vertex_delaunay_faces()`
      (direct accessors), `neighboring_cells()`/`cell_is_unbounded()`
      (derived from `edges` each call). Out-of-range/cross-instance id
      handling mirrors `VertexId`/`EdgeId`/`FaceId`'s existing
      unchecked-indexing convention. Commit `18c8d6e`.
- [x] `cell_edges()` (ordered cyclic cell-boundary walk) deliberately
      **not** implemented: unbounded cells have no closed ring to walk,
      and the crate has no existing "faces around a vertex" primitive to
      build it from — a real design task, deferred to Phase 7C rather
      than rushed in under this phase's scope.
- [x] Validator extended (`a994384`) with 4 new checks: distinct edge
      cells, distinct `Bounded` vertices, `Unbounded` edges dual to an
      actual hull edge, no duplicate face within one vertex's group. Of
      ADR-007's requested invariant list, "same-component edges never
      exposed" was already covered by Phase 7A's checks; "one cell per
      site" needed no check (`VoronoiCellId` is a pass-through wrapper,
      no separate table to desync); "neighboring is symmetric" is
      asserted by a test instead of a validator check, since
      `neighboring_cells` reads `edges`' unordered pairs symmetrically by
      construction — the data shape admits no asymmetric entry to inject,
      unlike the 4 checks that were added. A negative test deliberately
      corrupts a valid `Voronoi2`'s private fields to confirm each of the
      4 new checks actually fires, not just that valid input passes.
- [x] `cell_is_unbounded`/`neighboring_cells` initially had no test
      distinguishing an interior cell from a hull cell — every fixture in
      the file used a fully-convex point set, so a stub always returning
      `true` would have passed. Fixed by extending the existing 60-point
      generic-position test: `cell_is_unbounded` is checked against an
      independent recomputation from `delaunay.boundary_edges()` for
      every cell, and an interior cell's `neighboring_cells` count is
      asserted `>= 3`.
- [x] rustdoc examples on `voronoi2()` and `neighboring_cells()`; query
      API round-trip test against internal struct data; symmetry test on
      the mixed cocircular-cluster-plus-outlier fixture. Verified at
      every commit: fmt, clippy (native + `wasm32-unknown-unknown`, both
      `-D warnings`), full test suite incl. doctests, MSRV (1.85),
      `cargo doc` (`-D warnings`), `wasm-pack test --node --release`.
- [x] **Not done, deliberately (Phase 7C)**: `cell_edges()`, circumcenter
      coordinates, clipping, nearest-neighbor, performance work, new
      dependencies, version bump, release. Not pushed — local commits
      only, per instruction.

## Done (ADR-007 Phase 7A: Voronoi topology construction, internal only)

- [x] `src/triangulation/voronoi.rs` — `VoronoiCellId`/`VoronoiVertexId`/
      `VoronoiEdgeId`, `VoronoiEdgeKind` (`#[non_exhaustive]`),
      `VoronoiEdge`, owned `Voronoi2`, and the `voronoi2()` constructor:
      union-find groups cocircular-adjacent Delaunay faces
      (`incircle(...) == Sign::Zero`), same-group Delaunay edges are
      excluded as spurious tie-break artifacts, and dense
      `VoronoiVertexId`/`VoronoiEdgeId` are assigned by sorting on a
      canonical site-identity key (not union-find root or scan order).
      Two commits: `b9702c1` (data model + constructor + internal
      `validate_voronoi_topology` validator + smoke tests),
      `d161636` (canonical-topology normalization tests: a square's
      both diagonals, and 5-/8-point exactly-cocircular integer-lattice
      point sets under multiple fan triangulations built directly via
      `assemble_triangulation`, since `delaunay2()` can never itself be
      made to pick a different diagonal for a fixed point set).
- [x] `#![allow(dead_code)]` at the module level, deliberately: nothing
      outside this file's own tests calls into it yet — no query API,
      no re-export from `triangulation::mod.rs`/`lib.rs`, no
      circumcenter, no clipping. Full fmt/clippy (native +
      `wasm32-unknown-unknown`, both `-D warnings`)/test suite pass at
      each commit.
- [x] **Not done, deliberately (Phase 7B/7C)**: public query API
      (`cells()`/`cell_site()`/`edges()`/etc.), circumcenter, clipping,
      nearest-neighbor, `cell_edges()`, performance work, new
      dependencies, version bump, release. Not pushed to `origin/main`
      — local commits only, per instruction.

## Done (ADR-007: Voronoi diagram topology design — design only, not implemented)

- [x] `docs/adr/ADR-007-voronoi-diagram-topology.md` — full design for
      0.5.0's Voronoi topology API, reviewed and approved (two rounds:
      initial design, then three specific revisions — owned `VoronoiEdge`
      storing `cells` directly rather than only re-derivable through
      `source_edge`; canonical (site-identity-keyed, not union-find-root-
      or scan-order-keyed) dense id assignment for `VoronoiVertexId`/
      `VoronoiEdgeId`; `VoronoiEdgeKind` marked `#[non_exhaustive]` up
      front for a future 1-2-site `Line` variant).
- [x] Central problem solved: cocircular Delaunay faces (which
      `delaunay2`'s own documented tie-break can split across more than
      one triangle) are grouped via union-find keyed on
      `incircle(...) == Sign::Zero`, with a from-scratch transitivity
      proof (three points determine a circle) for why pairwise-adjacent
      testing correctly captures arbitrarily large cocircular clusters,
      not just isolated 4-point quads.
- [x] **Not done, deliberately**: no `src/` code, no `Cargo.toml` change,
      no version bump, no dependency, no performance work. Starting
      0.5.0 implementation itself remains its own separate decision, not
      an automatic follow-on from the design being approved — see
      `ROADMAP.md` (internal)'s own "stop after each release/round"
      rule, still in effect.

## Done (fuzz: predicate_input_bytes target)

- [x] Added `fuzz/fuzz_targets/predicate_input_bytes.rs` — the last
      applicable target from AGENTS.md §12's original list.
      Raw-bit-pattern (`f64::from_bits`) fuzzing of
      `orient2d`/`orient3d`/`incircle`/`insphere`, complementing the
      existing small-integer-grid targets (which stress degenerate
      *configurations*) with raw magnitude/bit-pattern diversity (`NaN`,
      infinity, subnormals, full range) — `Point2::new`/`Point3::new`'s
      own finite-coordinate validation is exercised the same way. Ran
      clean: 40,224 executions / 90s, no crashes.
- [x] `polygon parser`, AGENTS.md §12's remaining unimplemented target,
      confirmed inapplicable rather than left silently unstarted: this
      crate never grew a text/byte-format polygon parser (`Polygon2` is
      built directly from `Vec<Point2>`, no WKT/GeoJSON/etc. surface
      exists) — fuzzing it would mean building a parser expressly to
      fuzz it, backwards from the point of fuzzing existing attack
      surface. Noted here so it doesn't get silently retried.

## Done (wasm32 test execution, not just build)

- [x] Added `wasm-bindgen-test` as a `wasm32`-only dev-dependency
      (`Cargo.toml`'s `[target.'cfg(target_arch = "wasm32")'.dev-dependencies]`
      — never propagates to downstream crates or the normal build,
      matching the existing `num-bigint`/`num-rational` dev-only
      isolation, ADR-005). `tests/wasm.rs`: 10 load-bearing
      `#[wasm_bindgen_test]` cases (one per major subsystem — see
      `docs/compatibility.md` for the exact list), verified passing
      under `wasm-pack test --node --release` (Node.js), not just
      `cargo build --target wasm32-unknown-unknown`. New independent CI
      job `wasm-test-node` (the existing build-only `wasm` job is
      unchanged, not replaced).
- [x] Found and fixed a real bug in the new test itself while writing
      it, not a wasm32 discrepancy: `insphere_basic_case`'s first draft
      assumed "outside the sphere" always means `Sign::Negative`, but
      `insphere`'s sign convention is orientation-dependent on the
      a/b/c/d vertex order (its own doc comment: swapping any two flips
      the sign) — confirmed by reproducing the exact same result
      natively before touching the test, ruling out a platform
      difference. See `tests/wasm.rs`'s comment on that test.
- [x] `docs/compatibility.md` and this file updated from "builds, not
      executed" to "executed under Node.js" for wasm32.

## Done (0.4.0: polygon triangulation with holes, wasm32 execution testing — released 2026-08-18)

- [x] `Polygon2::relation_to`/`PointPolygonRelation`: exact point-in-polygon
      predicate (crossing-number/ray-casting via `orient2d` +
      `Segment2::relation_to`, no new coordinate). Verified against an
      independent exact-rational *winding-number* oracle (deliberately a
      different algorithm class from the production even-odd test) in
      `tests/differential/point_in_polygon.rs`. Caught and fixed a real
      test-generator bug along the way (not a `relation_to` bug): the
      angle-sort-around-centroid technique for building random simple
      polygons silently produces a self-intersecting ring under extreme
      intra-ring magnitude mixing — see `lessons.md`.
- [x] `triangulate_polygon_with_holes`: generalizes `triangulate_polygon`'s
      existing algorithm (a hole's boundary is just more constrained
      edges the same flood fill already stops at) rather than a new one.
      `PolygonTriangulationError` (`#[non_exhaustive]` since 0.3.0, so
      this is non-breaking) gains `InvalidHole`, `HoleSelfIntersecting`,
      `HoleIntersectsOuter`, `HoleOutsideOuter`, `HolesIntersect`,
      `NestedHole`. Nested holes ("island" case) out of scope, typed
      error. Verified against all 9 fixtures + acceptance criteria from
      `ROADMAP.md`'s (internal) 0.4.0 spec — see that file for exactly
      what shipped vs. what's still deliberately deferred.
- [x] `wasm-bindgen-test` added as a `wasm32`-only dev-dependency;
      `tests/wasm.rs` runs 10 load-bearing cases under actual Node.js
      execution (`wasm-pack test --node --release`), not just a wasm32
      build; new independent CI job `wasm-test-node`.
- [x] CHANGELOG, `docs/degeneracy-policy.md`, `docs/compatibility.md`,
      README (all 3 languages — closed real translation drift in
      `README_ja.md`/`README_zh.md`, which had never gotten the
      `triangulate_polygon_with_holes` paragraph), and two new runnable
      examples updated to match.
- [x] **Released**: `Cargo.toml` bumped to 0.4.0, `CHANGELOG.md`
      `[0.4.0] - 2026-08-18`, pushed, CI green (10/10 jobs including
      `wasm-test-node`'s first real run), published to crates.io,
      confirmed via a fresh fixture build against the published version
      (including a nested-hole rejection check), `v0.4.0` tag + GitHub
      Release, all SHAs (local/origin/tag) consistent.

## Done (0.3.0: bug-check-and-refactoring pass + release)

- [x] `constrained_delaunay2` panicked on any degenerate point set (fewer
      than 3 points, or all collinear), even with zero constraints — fixed
      with the new `CdtError::DegeneratePointSet`. See `tests/regression/cdt.rs`
      and the `.expect`-precondition-enumeration lesson in `lessons.md`.
- [x] `predicates::line_intersection` could return non-finite (`NaN`) at
      extreme (~5.6e102+) or mixed-magnitude coordinates — fixed via exact
      power-of-two rescaling, verified correctly rounded to ~3.3e150.
- [x] `triangulate_polygon` defensive postcondition check (triangle count
      matches `polygon.len() - 2` before returning `Ok`); `Polygon2::edge`
      doc-accuracy fix (`len() == 1` doesn't panic); documented
      `restore_unconstrained_delaunay`'s termination argument plus a
      multi-constraint-mode test (matching `triangulate_polygon`'s actual
      usage, previously only single-constraint-covered).
- [x] Internal refactors, no public API/behavior change: shared
      `det3_with_precancel_bound`/`det3_exact`/`negate` across 4 predicate
      files (~130 duplicated lines); extracted `point_in_collinear_range`;
      extracted `validate_constraints` from `constrained_delaunay2`;
      consolidated `cdt.rs`'s 4 face-scanning loops onto one shared scan.
      Considered and *reverted* a 5th refactor (FIFO-counter struct
      consolidation) after the actual diff showed it added more code than
      it removed — see `lessons.md` if this comes up again.
- [x] `#[non_exhaustive]` added to `KikaError`/`CdtError`/
      `PolygonTriangulationError`/`TopologyError` (Result-style error
      enums only — closed classification enums like `Sign`/`Orientation`
      left exhaustive). This is why the release is 0.3.0, not 0.2.1 — see
      `lessons.md`'s "which public enums get `#[non_exhaustive]`" entry
      for the criterion to reuse next time a public enum is added.
- [x] **0.3.0 published**: `cargo publish` (via the repo's `publish.yml`
      `workflow_dispatch`), confirmed live on crates.io, verified with a
      fresh fixture crate built against the published version (including
      confirming `#[non_exhaustive]` actually rejects a non-wildcard
      `match` from outside the crate), docs.rs build green, `v0.3.0` tag
      pushed at the published commit, GitHub Release published, all SHAs
      (local `main`/`origin/main`/peeled tag) confirmed matching.

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
      (`4 * face_count + 16`, well under the ~72 bound for the sizes
      tested at the time — since superseded, see the sanity-benchmark
      entry below, which found and fixed a real bug this measurement's
      small (~8 point) grids never exercised) rather than looping to
      convergence unbounded, matching Phase 5's `correctly_rounded_divide`
      discipline. A candidate flip
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
- [x] Small-scale sanity benchmarks (`benches/sanity.rs`, fixed seed,
      n=100/300/1000, `cargo bench --bench sanity`): triangle counts and
      topology validity for `delaunay2`/`constrained_delaunay2`/
      `triangulate_polygon`, generous (not competitive) time ceilings —
      no performance optimization done, per the user's explicit scope.
      Found and fixed a real bug along the way:
      `insert_constraint_edge`'s original rescan-and-pick-first crossing-
      edge selection could oscillate in a 2-cycle instead of converging,
      on an ordinary (non-degenerate) long constraint in a 300-point
      random cloud — every existing unit test used inputs too small to
      exercise it. Fixed with a persistent FIFO queue (the actual
      standard Sloan-style algorithm, which the code's own prior doc
      comment had mistakenly described itself as already being). See
      `tests/regression/cdt.rs` and `tasks/lessons.md`.

## Known gaps, not yet closed (see docs/compatibility.md)

- [ ] All 6 fuzz targets that map onto something this crate actually has
      (`segment_intersection`, `convex_hull`, `delaunay_insert`,
      `triangulation_topology_validator`, `polygon_validity`,
      `predicate_input_bytes`) ran clean on short (60-90s) local runs
      only — no coverage-guided corpus persisted across runs, no
      nightly/long-duration run performed yet. AGENTS.md §12's original
      list is now fully addressed: its 7th item, `polygon parser`,
      doesn't apply — this crate never grew a text/byte-format parser to
      fuzz (see the "Done" section above).
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
      environment-blocked, not just unstarted: `pkg-config` is now
      installed (re-checked), but CGAL itself still isn't, and installing
      it would mean pulling in a large C++ dependency stack (Boost,
      GMP, MPFR) via Homebrew — a real environment change, not a small
      reversible one, so not done without explicit approval
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

- [x] crates.io publish — done for 0.2.0 and 0.3.0. As of 0.3.0, kicking
      off a release round (i.e. deciding "we're releasing now") still
      needs explicit approval, but once that round's commits are pushed
      and CI is green, the rest of the sequence (publish, crates.io
      verification via a fresh fixture, docs.rs check, tag, GitHub
      Release, SHA consistency check) runs without a separate approval
      per step — see `docs/release-checklist.md` and ROADMAP.md
      (untracked, internal) for the standing policy.
- [x] GitHub release / repo visibility change — `v0.2.0` and `v0.3.0`
      releases both published; repo visibility unchanged (still whatever
      it was before, not touched by this policy).
- [ ] Any new runtime (non-dev) dependency, including specifically:
      `num-bigint`/`num-rational` (or similar) promoted from dev-only
      (ADR-005) to a genuine runtime dependency, as the fallback if
      expansion-backed homogeneous coordinates prove insufficient for
      Phase 6b's polygon-overlay construction needs — see ADR-004's
      "Phase 6 re-evaluation" section
