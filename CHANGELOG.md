# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- `Voronoi2` / `voronoi2`: a topology-only Voronoi diagram, the dual of
  an existing `Triangulation2` — no coordinates (circumcenters),
  clipping, or nearest-neighbor query, deliberately deferred. Cocircular
  Delaunay faces (which `delaunay2`'s own documented tie-break can split
  across more than one triangle) are merged via union-find keyed on
  `incircle(...) == Sign::Zero`, so an arbitrary Delaunay tie-break can't
  leak into the output as spurious extra vertices or edges; every dense
  id (`VoronoiVertexId`/`VoronoiEdgeId`) is assigned by sorting on a
  canonical site-identity key rather than union-find root or scan order,
  so two differently-triangulated-but-topologically-equal inputs produce
  identical, not merely isomorphic, output.

  Query API: `cells()`/`vertices()`/`edges()`, `cell_site()`,
  `neighboring_cells()`, `cell_is_unbounded()`, `edge_cells()`,
  `edge_kind()`, `dual_delaunay_edge()`, `vertex_delaunay_faces()`, and
  `cell_edges()` — an ordered counterclockwise walk of a cell's boundary
  edges (a closed cycle for a bounded/interior-site cell, a linear
  sequence between the two rays for an unbounded/hull-site cell), built
  entirely from `Triangulation2`'s existing face adjacency, no new data
  model. See `docs/adr/ADR-007-voronoi-diagram-topology.md` for the full
  design and correctness argument.

## [0.4.0] - 2026-08-18

Polygon triangulation with holes, plus a wasm32 execution-testing gap
closed. No breaking changes — every new enum variant lands on an already
`#[non_exhaustive]` enum (as of 0.3.0).

### Added

- `Polygon2::relation_to` / `PointPolygonRelation`: exact point-in-polygon
  predicate (crossing-number/ray-casting, built entirely from `orient2d`
  and `Segment2::relation_to` — no new coordinate construction). Works for
  any simple polygon, convex or not, unlike `Triangle2::relation_to`'s
  "same side of every edge" test, which only works because a triangle is
  always convex. Verified against an independent exact-rational
  winding-number oracle (a different algorithm class from the production
  even-odd test) in `tests/differential/point_in_polygon.rs`.

- `triangulate_polygon_with_holes`: simple-polygon triangulation with
  hole support, generalizing `triangulate_polygon`'s existing algorithm
  rather than using a new one — a hole's boundary is just more
  constrained edges from the same flood-fill's point of view that already
  discards `triangulate_polygon`'s own concave pockets. No Steiner
  points, no new construction. `PolygonTriangulationError` (already
  `#[non_exhaustive]` as of 0.3.0) gains 6 new variants covering hole
  rejection: `InvalidHole`, `HoleSelfIntersecting`,
  `HoleIntersectsOuter`, `HoleOutsideOuter`, `HolesIntersect`,
  `NestedHole` (a hole nested inside another hole — an "island" case —
  is out of scope, rejected as a typed error rather than partially
  supported). See the function's own doc comment for the full algorithm,
  rejected-input list, and the `n + 2h - 2` triangle-count acceptance
  criterion.

### Changed

- wasm32 is now **executed**, not just built: `tests/wasm.rs` runs 10
  load-bearing cases (one per major subsystem — predicates, segment
  intersection, the 0.3.0 `line_intersection` overflow fix, Delaunay,
  degenerate CDT, both polygon-triangulation entry points) under an
  actual Node.js runtime via `wasm-pack test --node --release`, both
  locally and in a new independent CI job (`wasm-test-node`; the
  existing build-only `wasm` job is unchanged). Closes the gap noted in
  0.3.0's `docs/compatibility.md` entry: ADR-001's "Rust never contracts
  `+`/`-`/`*` into FMA" argument, load-bearing for the exact-arithmetic
  core, is now empirically confirmed on this target, not just assumed
  from a successful build. `wasm-bindgen-test` added as a `wasm32`-only
  dev-dependency — never propagates to downstream crates or the normal
  build (same isolation as the existing `num-bigint`/`num-rational`
  oracle dev-dependencies).

## [0.3.0] - 2026-08-17

A robustness/compatibility release following a general bug-check and
refactoring pass over 0.2.0: two real bugs fixed (one of them the reason
this is 0.3.0 and not 0.2.1 — see Changed below), a couple of doc-accuracy
and defensive-check gaps closed, and several purely-internal
deduplications. No new features.

### Changed

- **Breaking:** `KikaError`, `CdtError`, `PolygonTriangulationError`, and
  `TopologyError` (the latter `#[doc(hidden)]`, not an advertised API
  commitment, but still technically `pub`) are now `#[non_exhaustive]`.
  Any downstream `match` on these enums without a wildcard (`_`) arm will
  no longer compile. This is what forces 0.3.0 rather than 0.2.1: the
  `CdtError::DegeneratePointSet` addition below is itself a variant
  addition to an enum that was *not* `#[non_exhaustive]` in 0.2.0, which
  is already a breaking change for any 0.2.0 consumer with an exhaustive
  match — marking these enums `#[non_exhaustive]` now is the fix that
  prevents the *next* variant addition from repeating that break. Applied
  only to `Result`-style "why did this fallible operation fail" enums
  (all four either implement/are intended to implement
  `core::error::Error`, or serve the equivalent diagnostic role); enums
  that classify a mathematically or geometrically closed set of outcomes
  (`Sign`, `Orientation`, `PointSegmentRelation`, `PointTriangleRelation`,
  `SegmentIntersectionKind`, `SegmentIntersection2`,
  `PolygonBasicValidity`, `HullBoundaryPoints`) were deliberately left
  exhaustive — none of them are `Result` error types, and their variant
  sets are complete by construction, not expected to grow.

- **Internal, no public API change:** shared the 3×3 cofactor
  determinant-with-precancellation-bound pattern (`det3_with_precancel_bound`/
  `det3_exact`) and the 4×-duplicated `negate()` helper across
  `orient3d.rs`/`incircle.rs`/`insphere.rs`/`line_intersection.rs` into
  `predicates::expansion` (~130 duplicated lines removed); extracted
  `point_in_collinear_range` (shared by `Segment2::relation_to` and
  segment-intersection classification) and removed redundant `orient2d`
  recomputation in `classify()`'s `EndpointTouch` checks; extracted
  `validate_constraints` from `constrained_delaunay2`; consolidated
  `cdt.rs`'s four separate face-scanning loops (`edge_exists`,
  `crossing_edges`, `adjacent_faces_of_edge`,
  `find_first_bad_unconstrained_edge`) onto one shared scan. All verified
  behavior-preserving by the existing test suite; the face-scan
  consolidation additionally verified via bit-identical measured flip
  counts before/after.

### Fixed

- `constrained_delaunay2` panicked (via an internal `.expect(...)`) on any
  degenerate point set — fewer than 3 points, or all points exactly
  collinear — even with an empty `constraints` list, because `delaunay2`
  returns an *empty* `Triangulation2` for that input by its own documented
  policy, leaving nothing for the coordinate-to-`VertexId` lookup to find.
  Fixed by checking for an empty triangulation immediately after the
  `delaunay2` call: an empty `constraints` list now returns `Ok` wrapping
  that same empty triangulation; a non-empty one returns the new
  `CdtError::DegeneratePointSet` (no triangulation face exists for a
  constraint to become an edge of). See `tests/regression/cdt.rs` and
  `docs/degeneracy-policy.md`'s CDT table.

- `predicates::line_intersection` (used internally by `segment_intersection`'s
  `Proper` case) could return a non-finite (`NaN`) `Point2`, breaking the
  crate-wide "a constructed `Point2` is always finite" invariant, at
  coordinate magnitudes the construction's degree-3 numerator overflows
  `f64::MAX` — uniform-magnitude inputs around `~5.6e102`, and a sharper
  mixed-magnitude case (segments at different scales `K`/`M`, where the
  relevant quantity `K²·M` can overflow even when both scales individually
  sit far below that threshold). Previously documented only as "not
  independently swept", not fixed. Both mechanisms confirmed by dedicated
  tests before the fix (`NaN` reproduced at uniform `1e103` and mixed
  `k=1e130, m=1e100`). Fixed with exact power-of-two rescaling (lossless —
  an exponent shift, no rounding) applied whenever any input coordinate
  exceeds `1e90`; verified both finite and still correctly rounded (against
  the same `BigRational` oracle used elsewhere) up through `~3.3e150`. No
  public API change. See `docs/numerical-model.md`'s Phase 5 section and
  `tasks/lessons.md`.

- `Polygon2::edge`'s doc comment claimed it panics when `self.len() < 2`;
  false for `len() == 1`, where `edge(0)` wraps (`(0+1) % 1 == 0`) and
  returns a degenerate zero-length `Segment2` instead of panicking (the
  real panic condition is only `i >= self.len()`, including the `len()
  == 0` case via plain indexing). Doc-only fix, no behavior change — the
  only caller (`find_self_intersection`) already used statically-safe
  indices. See `docs/degeneracy-policy.md`.

### Added

- Defensive postcondition check in `triangulate_polygon`: returns
  `PolygonTriangulationError::ConstraintInsertionFailed` if the built
  triangle count doesn't match the documented `polygon.len() - 2`
  guarantee, instead of trusting the loop's result unchecked. No failure
  found triggering this in stress testing — a safety net, not a fix for
  an observed bug.
- Doc comment on `restore_unconstrained_delaunay` explaining why its
  rescan loop terminates (paraboloid-lift / monotonic-volume argument for
  Lawson flips), plus a new test exercising it in the
  multi-constraint-in-one-call mode `triangulate_polygon` actually uses
  (previously only single-constraint calls were covered).

## [0.2.0] - 2026-08-17

A robust 2D kernel with exact predicates, 2D convex hull, Delaunay
triangulation, constrained Delaunay triangulation (narrow scope), and
simple-polygon triangulation (narrow scope). This is the first tagged
release — everything below, from the initial predicate core through
Phase 6D, ships as 0.2.0.

### Added

- Runnable examples for constrained Delaunay and simple-polygon
  triangulation: `examples/constrained_delaunay.rs` (forces a specific
  non-Delaunay diagonal, asserts it survives and is marked constrained)
  and `examples/polygon_triangulation.rs` (triangulates a non-convex
  L-shaped polygon, asserts triangle count/CCW/area conservation) — 7
  runnable examples total. Matching `# Examples` doctests added to
  `constrained_delaunay2`'s and `triangulate_polygon`'s own doc comments
  (10 doctests total, up from 8), mirrored as short snippets in README's
  Minimal example section so the README can't silently drift from what's
  actually compiled and run.

- Small-scale, fixed-seed sanity benchmarks (`benches/sanity.rs`,
  `cargo bench --bench sanity`, `harness = false` so it runs on stable):
  `delaunay2`/`constrained_delaunay2`/`triangulate_polygon` at
  n=100/300/1000, checking triangle counts and topology validity with a
  generous (not competitive) time ceiling. Performance has not yet been
  optimized — this exists to catch a catastrophic algorithmic regression,
  not to make a speed claim.

### Fixed

- **Two distinct defects in constrained Delaunay segment recovery**,
  both in Phase 6C's `insert_constraint_edge`, found by the sanity
  benchmark above on a single long constraint in a 300-point random
  cloud (no degenerate collinearity involved) — every existing unit test
  used inputs too small to reach either.
  1. **Loud, wrong: could oscillate forever instead of converging.** The
     original rescan-and-pick-first approach could settle into a
     2-cycle — flip an edge, its replacement is still crossing and still
     sorts first next scan, flip it back, repeat — never converging
     until it exhausted the flip bound and returned
     `CdtError::ConstraintInsertionFailed` for a perfectly realizable
     constraint. Fixed by implementing the actual standard Sloan-style
     algorithm: a persistent FIFO queue of crossing edges instead of a
     full rescan each iteration.
  2. **Quiet, wrong: introduced by the queue-based rewrite itself.** The
     rewrite's queue-empty exit returned `Ok(())` without confirming the
     constraint edge actually exists. For a constraint whose segment
     passes exactly through a third input vertex (edges incident to that
     vertex never enter the crossing queue at all — they classify as
     `EndpointTouch`/`CollinearTouch`, never `Proper`), the queue could
     drain to empty while the constraint itself was never realized,
     silently returning success with the constraint missing from
     `constrained_edges`. Fixed by checking `edge_exists` before
     declaring success on an empty queue. Caught during review, before
     being caught empirically: reverting the one-line fix reproduced
     exactly this — `Ok(...)` with `constrained_edges: {}`.
  See `tests/regression/cdt.rs` (one test per defect) and
  `tasks/lessons.md`. Measured flip/pass counts on the existing test
  suite (max 7 insertion flips / 10 insertion passes / 3 restore flips,
  vs. the previously-reported 9/3 flip-only numbers — all well under the
  ~72 bound for those input sizes) now track total loop passes
  separately from flip count, since the queue-based algorithm's `bound`
  limits passes (flips plus not-yet-flippable retries), not flips alone.

- Simple polygon triangulation (Phase 6D): `triangulate_polygon`,
  `PolygonTriangulationError`. Built on Phase 6C's CDT: constrain every
  polygon edge, then discard the concave-pocket faces outside the polygon
  (for non-convex input) via a purely topological flood fill from one
  interior seed face — found via a single `orient2d` check against an
  existing triangle vertex, never a constructed point such as a centroid.
  No holes, no Steiner points (every output vertex is one of the
  polygon's own); self-intersecting input (including a non-adjacent
  repeated vertex) is a typed error, never a panic. Accepts both CCW and
  CW input — the flood fill is orientation-agnostic; only the seed-face
  selection branches on the polygon's own winding. Deterministic
  regardless of which vertex the input starts at. Found via review: the
  flood fill's seed-face selection needed to correctly disambiguate when
  the seed edge has 2 incident faces (a chord of the full point set's
  hull), not just the trivial 1-incident-face (hull edge) case — added a
  dedicated test for that branch, plus one with 4 separate discarded
  pockets (a plus/cross shape), after the initial test suite happened to
  only exercise hull-edge seeds. Also found and documented a real
  semantic gap: `Triangulation2::validate_topology()`'s Euler-formula
  check assumes full convex-hull coverage, true for
  `delaunay2`/`constrained_delaunay2` but generally false for a
  non-convex polygon's triangulation (a proper subset of the hull) — see
  `docs/degeneracy-policy.md`.

- Constrained Delaunay triangulation (Phase 6C): `constrained_delaunay2`,
  `ConstrainedTriangulation2`, `CdtError`. Deliberately narrow scope: only
  non-crossing constraint edges between existing input vertices (crossing
  or collinear-overlapping constraints are a typed error, checked
  exhaustively up front); no automatic intersection/Steiner-point
  generation, refinement, or quality meshing. Implemented entirely by
  flipping existing Delaunay edges (segment recovery, then bounded
  restoration of local Delaunay-ness on unconstrained edges) — ADR-004's
  Phase 6 re-evaluation predicted CDT needs no new construction at all,
  and the implementation confirms it, touching no construction primitive.
  Both flip passes are bounded (`4 * face_count + 16`, the same
  measured-not-assumed discipline as Phase 5's `correctly_rounded_divide`
  loop bound — measured worst case across a spread of random test
  configurations: 9 insertion flips / 3 restore flips, well under the
  ~72 bound) rather than looping to convergence with no ceiling; hitting
  it is `CdtError::ConstraintInsertionFailed`, never a hang or a silently
  wrong result. No panics anywhere in the public API.

- Release-quality polish pass (Phase 6A): `#![forbid(unsafe_code)]` and
  `#![warn(missing_docs)]` at the crate root (every public item now
  documented, enforced going forward by CI's existing `-D warnings`);
  `examples/` with 5 runnable examples (`orient2d`, `segment_intersection`,
  `convex_hull`, `delaunay`, `polygon_validity`); `Cargo.toml`
  `homepage`/`documentation` metadata; a maturity table and corrected
  CGAL-oracle/CI-status wording in `README.md` (both previously described
  work that hadn't actually happened yet); `docs/release-checklist.md`.
  Also fixes `Cargo.toml`'s `repository` field (was `kika-rs/kika`, a
  nonexistent org, now the actual `kent-tokyo/kika` remote) and
  `docs/compatibility.md`'s stale "CI not yet exercised"/Phase-1-era test
  count and public API list.

- `Triangulation2` adjacency structure (Phase 6B, ADR-006): `VertexId`,
  `EdgeId`, `FaceId` and `vertices`/`edges`/`faces`/`edge_vertices`/
  `adjacent_faces`/`face_vertices`/`neighboring_faces`/`boundary_edges` —
  a static, post-construction indexed-triangle-adjacency snapshot (no
  half-edge/quad-edge generality; `flip`/constraint-marking deferred to
  Phase 6C). `triangles()`'s original coordinate-only contract is
  unchanged; the new methods are purely additive, built alongside the
  existing coordinate list rather than replacing it. Includes an internal
  `validate_topology` (CCW, edge-manifold incidence, adjacency
  reciprocity, Euler's formula, per-edge local-Delaunay), which
  `tests/differential/delaunay2.rs` and
  `fuzz/fuzz_targets/triangulation_topology_validator.rs` now call
  directly instead of duplicating the same checks ad hoc.

- Repository skeleton, dual `MIT OR Apache-2.0` license, ADR-001..005.
- Exact expansion arithmetic core (`predicates::expansion`, internal):
  `two_sum`, `split`, `two_product`, `product_expansion`,
  `diff_expansion`, `expansion_sum`, `scale_expansion`,
  `product_of_expansions`, `expansion_sign` — verified against a
  `num-rational` dev-dependency oracle, never used from production code
  (ADR-005).
- `Point2`, `Point3` finite-coordinate types; `Sign`, `Orientation` result
  enums; `KikaError`.
- `orient2d`, `orient3d`, `incircle`, `insphere`: exact-sign geometric
  predicates. Each uses a floating-point filter with a *computed* error
  bound (never a fixed epsilon), falling back to exact expansion
  arithmetic — built from the original coordinates, not a once-rounded
  intermediate — when the filter is inconclusive. `incircle`/`insphere`
  have narrower verified-safe coordinate-magnitude ranges than
  `orient2d`/`orient3d` (higher polynomial degree from the paraboloid
  lift); see `docs/numerical-model.md`. Each checked against an
  independent exact-rational oracle in `tests/differential/`.
- CI (`.github/workflows/ci.yml`): `cargo fmt --check`, clippy
  (`-D warnings`), test matrix (Linux/macOS/Windows), MSRV (1.85) check,
  `wasm32-unknown-unknown` build, `cargo doc` (warnings denied),
  `cargo-deny` (license + security-advisory check, `deny.toml`).
- `Vector2`, `Vector3` finite-coordinate displacement types, with
  `Point ± Vector -> Point` / `Point - Point -> Vector` affine arithmetic
  and vector `+`/`-`/negate/`* f64`. Point equality policy formalized as
  exact coordinate equality (ADR-003).
- `Segment2`, `Triangle2`, `Triangle3`, `Aabb2`, `Aabb3` primitive types.
  `Triangle2::orientation()`; `Aabb2`/`Aabb3::overlaps()` (exact, no
  predicate calls — a fast bounding-box rejection test).
- `Segment2::relation_to` (point-on-segment), `Triangle2::relation_to`
  (point-in-triangle): exact predicates built from `orient2d`, each with
  explicit degenerate-case handling (zero-length segment; collinear
  triangle). Checked against independent exact-rational oracles in
  `tests/differential/`.
- `segment_intersection_kind` / `segment_intersection`: robust 2D segment
  intersection, classification (`None`/`Proper`/`EndpointTouch`/
  `CollinearTouch`/`CollinearOverlap`) and coordinate construction kept
  as separate functions per §4.2. Classification never divides or builds
  a new coordinate (an `Aabb2` fast-reject runs before any predicate
  call); construction is exact except for `Proper`, which needs a
  genuinely new coordinate (ordinary `f64` interpolation, not certified —
  Phase 5 territory). Checked against an independent exact-rational
  reimplementation of the whole decision tree (not just the underlying
  `orient2d` calls) in `tests/differential/`.
- `Polygon2`: implicitly-closed vertex ring. `signed_area()` (plain
  `f64`) and `orientation()` (exact — sums every edge's shoelace term via
  the same exact-expansion machinery `orient2d` etc. use, not a running
  `f64` sum) kept separate per §4.2. `basic_validity()` (vertex count,
  consecutive-duplicate vertices, zero area) and the separate, O(n²)
  `find_self_intersection()` (correctly excludes adjacent edges' shared
  vertex from being reported as a self-intersection).

  This completes Phase 2 (2D Primitives and Intersections).
- `convex_hull2` / `HullBoundaryPoints`: 2D convex hull via Andrew's
  monotone chain. `ExtremesOnly` (default) keeps only strict corners;
  `KeepAllOnBoundary` also keeps boundary points collinear with their
  neighbors. Output is counterclockwise, starts at the lexicographically
  smallest input point, and is independent of input order. Duplicate input
  points (exact coordinate equality) are collapsed before hulling. Fully
  exact: every returned vertex is copied from an original input `Point2`,
  never a computed coordinate — no `Proper`-style non-exactness case exists
  here, since the algorithm is built entirely from `orient2d`. Checked via
  structural property tests (input-point containment, convex winding,
  permutation invariance, idempotence), not a from-scratch exact
  reimplementation — see `tests/differential/convex_hull2.rs`.

  This completes Phase 3 (2D Convex Hull).
- `delaunay2` / `Triangulation2`: 2D Delaunay triangulation via Bowyer-Watson
  incremental insertion. "Outside the triangulation" is represented by a
  single symbolic ghost vertex (no coordinate), not a synthetic bounding
  triangle — a triangle carrying the ghost reduces its circumcircle test to
  an exact `orient2d` half-plane check against its one real edge. Fully
  exact like `convex_hull2`: every returned vertex is copied from an
  original input `Point2`, and unlike a bounding-triangle approach there is
  no scale-dependent tradeoff anywhere in the algorithm (verified down to a
  perpendicular cluster spread of `1e-200` relative to a span of `10.0`).
  Cocircular points (no unique Delaunay triangulation exists) get a
  documented, deterministic tie-break: a point exactly on a circumcircle
  boundary does not invalidate that triangle. Checked via structural
  property tests (empty-circumcircle property, CCW/non-degenerate
  triangles, watertight mesh matching the convex hull boundary, Euler's
  formula `2n - 2 - h`, permutation invariance) — see
  `tests/differential/delaunay2.rs`.

  This completes Phase 4 (2D Delaunay Triangulation).
- `predicates::line_intersection` (internal, used by `segment_intersection`'s
  `Proper` case): the crate's first exact/certified **construction**
  (ADR-004, decided). Returns the correctly-rounded (round-to-nearest-even
  on exact ties) `f64` nearest to the true line-line intersection
  coordinate, ending the `Proper`-case exactness gap noted in Phase 2's
  entry above. `Point2` stays a plain `f64` pair (`float+certificate` model
  chosen over a new exact-coordinate type, per ADR-004): the numerator and
  denominator of the parametric crossing formula are built as exact
  expansions reusing `orient2d`'s own exact-fallback machinery, and the
  final division is resolved to the correctly-rounded result by comparing
  the exact residual against a per-direction half-ULP threshold. No new
  public API, no new dependency. Verified against an independent
  `BigRational` "is this the correctly-rounded nearest `f64`" oracle
  (magnitude scales, mixed-magnitude inputs, an empirical magnitude-floor
  sweep, and a measured — not assumed — bound on the refinement loop's
  worst-case iteration count) in `tests/differential/line_intersection.rs`;
  see `docs/numerical-model.md`.

  This completes Phase 5 (Exact/Certified Constructions).

### Fixed

*(bugs found and fixed during this same initial implementation, before
any release — noted for the record per AGENTS.md §18/§20, not because
anything public regressed)*

- `orient2d`/`orient3d` exact fallback used to reuse the filter's
  once-rounded coordinate difference (e.g. `a.x()-c.x()` as a plain `f64`
  subtraction) instead of recomputing it exactly from the original
  coordinates. Could return the wrong sign for calls mixing widely
  different coordinate magnitudes (e.g. `2^60` alongside small integers);
  see `tests/regression/orient2d.rs` and `docs/numerical-model.md`.
- `orient3d`/`incircle` floating-point filters used to bound their error
  using each cofactor's post-subtraction term magnitude instead of the
  pre-subtraction magnitudes, which could silently underestimate the true
  error when an inner cofactor subtraction cancelled catastrophically;
  see `tests/regression/incircle.rs` and `docs/numerical-model.md`.
- Expansion combination (`expansion_sum`/`scale_expansion`/
  `product_of_expansions`) used to fold pieces left-to-right into a
  single growing accumulator, which is O(count²) regardless of how fast
  each individual merge step is. Made `insphere`'s exact fallback take
  16s/call on degenerate inputs. Fixed with a linear-time `expansion_sum`
  (merge-by-magnitude + single `two_sum` cascade) plus balanced
  binary-tree combination instead of a linear fold; see
  `docs/numerical-model.md`.
- `Triangle2::relation_to` (point-in-triangle) used to incorrectly return
  `OnBoundary` for a point far outside a degenerate (collinear-vertex)
  triangle's span but still on the same shared line — the general
  3-edge `orient2d` test can't distinguish the two cases for a degenerate
  triangle, since all three checks are trivially `Collinear` for any
  point on that line. Fixed with an explicit degenerate case using
  `Segment2::relation_to` (exact range membership); see
  `tests/regression/point_in_triangle.rs` and `docs/degeneracy-policy.md`.
- An early `delaunay2` implementation seeded Bowyer-Watson with a synthetic
  "super-triangle" (a bounding-box-derived coordinate, scaled and stripped
  from the output). Passed every hand-written unit test, but a property
  test on ordinary random point clouds found it silently dropping a
  triangle (2 instead of the topologically-required 3 for a 4-point input:
  3 hull, 1 interior) — whether a super-triangle vertex shields a real edge
  from its second real triangle is scale-dependent, with no universally
  safe multiplier (the governing ratio, bounding-box diagonal to smallest
  relevant point spacing, is unbounded). Fixed by replacing the synthetic
  coordinate with a single symbolic "point at infinity" ghost vertex and an
  exact `orient2d`-based reduction; see `tests/regression/delaunay2.rs` and
  `docs/numerical-model.md`.
