# Todo

## Done (0.7.0 release preparation)

- [x] User approved starting release preparation (2026-08-20, "go to
      0.7.0 release preparation") — the standing roadmap-pacing policy's
      required explicit go-ahead, following push+CI-green on both the
      implementation (`dda8084`..`e43b125`) and the same-day hardening
      round (`c0a4a35`..`7fcdf7e`).
- [x] `CHANGELOG.md`: `[0.7.0] - 2026-08-20` entry — `Voronoi2::
      vertex_point`/`edge_geometry`, `VoronoiGeometryError`,
      `VoronoiEdgeGeometry`, plus a "Known issues" entry for the
      pre-existing `delaunay2()` panic the new fuzz target found (not
      fixed this round, deliberately — unrelated subsystem).
- [x] `Cargo.toml` version bumped to `0.7.0`.
- [x] `docs/architecture.md`: Status line (was stale since 0.4.0 — never
      updated across 0.5.0/0.6.0 either, caught and fixed here too, not
      just for 0.7.0), module tree gained `circumcenter.rs`/`rounding.rs`
      and an updated `voronoi.rs` description, an ADR-009 pointer
      paragraph matching ADR-007/ADR-008's own precedent.
- [x] `docs/compatibility.md`: status paragraph, test count (360, was
      333), the 2 pushes making up 0.7.0's work recorded, `#[non_exhaustive]`
      list gained `VoronoiGeometryError`, `VoronoiEdgeGeometry` documented
      as `#[non_exhaustive]` for the "scope, not necessity" reason
      (matching `VoronoiEdgeKind`'s own precedent), public API surface
      list updated, the `delaunay2()` known issue recorded.
- [x] `README.md` (+ `README_ja.md`/`README_zh.md`, translated, not just
      the English original): Status line, `Voronoi2` bullet rewritten for
      the geometry addition, a known-issue note on the `delaunay2` bullet,
      a new doctested "Voronoi diagram geometry" minimal-example snippet
      (reusing `vertex_point`'s own real doctest, not a new untested one),
      `examples/voronoi.rs` extended to also print/verify `edge_geometry`
      output, Maturity table row, Roadmap section, Stability section's
      `#[non_exhaustive]` list.
- [ ] `docs/release-checklist.md` rewrite for 0.7.0 — in progress.

## Done (0.7.1: fix the delaunay2() permutation-inconsistency panic)

- [x] Root-caused, empirically (not just argued): two independent
      overflow sites in `predicates::expansion`, confirmed against the
      real public API before writing any fix.
      - `split()`'s `SPLITTER * a` (`SPLITTER ~= 2^27`) overflows to
        `±Infinity` for `|a| > f64::MAX/SPLITTER ~= 1.34e300`, then
        `hi = c - a_big` becomes `Infinity - Infinity = NaN`. This is
        the original fuzz-found repro's exact mechanism (`p1`'s
        y-coordinate, `~4.25e304`, from the minimal repro below).
      - `two_sum`'s `a + b` (inside `diff_expansion`) overflows for
        opposite-sign coordinates each within a small factor of
        `f64::MAX`, whose true difference itself exceeds `f64::MAX`.
        Found independently while diagnosing the above:
        `a=(1e308,0), b=(-1e308,1e-10), c=(0,1e-10)` is also
        permutation-inconsistent through the real `orient2d`.
      Both silently produce `NaN` that `expansion_sign` reads as
      `Sign::Zero` (`Orientation::Collinear`) — `NaN != 0.0` is `true`
      in Rust, so an unguarded `NaN` component reaches `Sign::of`, which
      falls through both `> 0.0` and `< 0.0` to `Sign::Zero`.
- [x] **Fixed** `delaunay2()` panics (`index out of bounds`) on 3 points
      with widely mixed magnitude. Minimal repro (each
      `Point2::new(x, y).unwrap()`):
      `p0 = (4.523334248222805e-282, 6.612169496581129e-281)`,
      `p1 = (3.2186699543901864e-57, -4.251746146807175e304)`,
      `p2 = (2.247760886104758e-307, 1.3683225479033359e-48)` — pinned in
      `tests/regression/orient2d.rs`'s
      `permutation_consistent_at_extreme_mixed_magnitude`, alongside the
      second repro above.
- [x] `split()` (`src/predicates/expansion.rs`) made overflow-safe for
      any finite input: above the threshold, recursively splits a
      `2^-100`-rescaled copy and scales the result back — exact, since
      power-of-two multiplication never loses precision short of
      overflow/underflow. Gated on `a.is_finite()` to avoid infinite
      recursion for an already-`Infinity` input from an unrelated
      overflow upstream (found via a pre-existing `circumcenter` test
      that deliberately constructs one) — `Infinity * 2^-100` is still
      `Infinity`, so without the guard the threshold check never stops
      triggering.
- [x] New `rescale_for_sign_only` helper (`predicates::expansion`), used
      only by `orient2d_exact`/`orient3d_exact`/`incircle_exact`/
      `insphere_exact` — **not** pushed into `diff_expansion` itself,
      since `circumcenter`/`line_intersection` also call it and need the
      real magnitude back, not just a sign. Rescales *every* coordinate
      in one predicate call by a fixed `0.25` factor above a fixed
      `f64::MAX/4` threshold, never restored — a determinant is
      homogeneous of positive degree in its coordinates, so any positive
      uniform rescale preserves its sign. Rescaling only the one
      overflowing diff (mirroring `voronoi::edge_vector`'s single-vector
      rescale too literally) was considered and rejected: it would
      desync that diff's scale from its siblings and corrupt the
      surrounding product's sign — the same bug class being fixed.
      Likewise rejected `circumcenter`/`line_intersection`'s own dynamic
      "normalize max coordinate to 1.0" rescale: for a call with a huge
      coordinate and a genuinely tiny sibling (exactly this bug's
      shape), that dynamic a shift would flush the tiny sibling to zero.
- [x] `expansion_sign` given a debug-only NaN guard, but *only* via a new
      `sign_only_expansion_sign` wrapper used by the 4 sign-only
      predicates — not inside `expansion_sign` itself. Learned the hard
      way: `circumcenter`/`line_intersection` (and `correctly_rounded_divide`)
      legitimately drive an intermediate expansion non-finite while
      computing a true result that itself doesn't fit in `f64` (e.g. a
      thin triangle's circumcenter), and already handle this correctly
      via their own final `.is_finite()` check — a shared assert in
      `expansion_sign` broke those already-passing tests
      (`thin_triangle_overflow_returns_none_not_a_panic` and others).
      Compiles out in release, so release-mode behavior for any
      still-deferred case (below) is unchanged.
- [x] Two pre-existing tests turned out to already be probing past this
      round's actual safe range and needed their own fix, not a weaker
      assert: `tests/adversarial/orient3d.rs`'s `extreme_large_scale_does_not_panic`
      used `1e150` (copied from `orient2d`'s own test) without accounting
      for `orient3d` being degree-3, not degree-2 — its real structural
      ceiling is `~5.65e102`; fixed to `1e90` (matching
      `circumcenter`/`line_intersection`'s own `RESCALE_THRESHOLD`
      precedent for degree-3 shapes). `tests/differential/line_intersection.rs`'s
      `magnitude_ceiling_sweep` swept coordinates up to `2^1020` through
      `segment_intersection`, which classifies via `orient2d` internally
      and — at uniform magnitude with no tiny sibling coordinate — was
      always going to cross `orient2d`'s own degree-2 ceiling
      (`~2^512.6`) well before `2^1020`; capped at `2^500`, still 158
      orders of magnitude past what the test actually exists to verify
      (`line_intersection`'s own degree-3 numerator fix).

## Discovered, not fixed (two deliberately deferred representability limits)

- [ ] **`two_product`'s `a * b` overflows when *both* operands are
      independently `> ~sqrt(f64::MAX) ~= 1.34e154`** — e.g.
      `p0=(1e300,1e300), p1=(-1e300,1e300), p2=(0,0)` (a valid, large,
      non-degenerate right triangle) returns `Orientation::Collinear`
      self-consistently across all 6 permutations of `orient2d` — wrong,
      but not permutation-*inconsistent*, so it doesn't panic
      `delaunay2()` and isn't the bug the 0.7.1 round above fixed. A
      genuine representability ceiling, structurally symmetric to the
      already-documented `~1.7e-292` small-value floor — not fixable by
      rescaling alone (rescaling shifts the whole magnitude range, it
      doesn't compress the *span* between a huge and a tiny coordinate in
      the same call, which is exactly what's needed here). A real fix
      needs a different arithmetic architecture (e.g. variable-precision/
      Shewchuk-adaptive expansions), out of scope for a patch release.
      `incircle`/`insphere` reach a *much* narrower version of this same
      ceiling even sooner, via their own internal squaring
      (`adz = adx^2 + ady^2`, etc.) — confirmed empirically while writing
      this round's `tests/adversarial/incircle.rs`/`insphere.rs` spot
      checks, which had to stay at `1e70`/`1e30` respectively (matching
      each predicate's own pre-existing `extreme_large_scale_does_not_panic`
      magnitude) rather than the `1e308` used for `orient2d`/`orient3d`'s
      equivalent checks.
- [ ] **`split()`'s own narrower residual**: for `|a|` within roughly
      `2^-26` of `f64::MAX` itself (a band `~4.3e300` wide at the very
      top of `f64`'s range), the correctly-rounded result would need a
      nonexistent exponent 1024, so `split` returns `hi = Infinity`
      regardless of rescale factor — a rounding-carry limit intrinsic to
      representing the result at all, not specific to this round's fix.
      `split` itself never produces `NaN` here (verified, not assumed —
      `split_near_f64_max_does_not_panic`), but `two_product` built on
      top of it still can, via `Infinity * 0.0` (exactly zero is
      `split`'s own `lo` for plenty of ordinary values, e.g.
      `split(1.0) == (1.0, 0.0)`) — never a panic either way. Structurally
      distinct from the item above (a rounding-carry/subtraction limit,
      not a multiplication-overflow one), and much narrower.
- [ ] Both documented in `docs/numerical-model.md`. Expected fallout:
      `fuzz/fuzz_targets/predicate_input_bytes.rs` (raw `f64::from_bits`
      coordinates) may now hit either case and panic via the new
      `sign_only_expansion_sign` debug assert, in debug/fuzz builds only
      — not a new bug introduced by this round, don't re-triage as one.

## Done (ADR-009 0.7.0 hardening round: ray-direction finiteness, InvalidTopology, fuzz target)

- [x] Fixed a real wrong-side bug in `outward_ray_direction` (found via
      self-review before this round, pushed separately as `e43b125`):
      the original implementation matched this ADR's own first-draft
      pseudocode literally (`orient2d(u,v,u+perp)`), which silently
      returns the wrong direction for a large-offset/short-edge input
      (the `u+perp` add rounds `perp` away). Fixed by deciding the side
      from `perp`'s fixed rotation relation to `edge` directly, never
      materializing `u+perp` at all.
- [x] **Ray-direction finiteness guarantee**: `edge_vector` (new free
      function, `src/triangulation/voronoi.rs`) replaces a plain
      `Point2 - Point2` in `outward_ray_direction` — falls back to a
      fixed `2^-600`-rescaled difference when the plain one overflows
      (only possible for opposite-sign, near-`f64::MAX` endpoints on some
      axis), never scaled back. Proven finite and non-zero for any two
      distinct finite points; verified directly (`edge_vector_finite_and_
      nonzero_at_opposite_sign_near_f64_max`) and end-to-end
      (`outward_ray_direction_correct_for_third_vertex_on_either_side`,
      exact-cross-product side check on both branches of the `orient2d`
      choice).
- [x] **`VoronoiGeometryError::InvalidTopology`** (new variant, enum
      already `#[non_exhaustive]` so not a SemVer event pre-release):
      replaces two internal `.expect()` panics — `canonical_representative_
      face`'s empty-group case (reachable via direct `Voronoi2` field
      corruption, tested: `empty_face_group_is_a_typed_error_not_a_panic`)
      and `edge_geometry`'s "`Unbounded` edge's source has no incident
      face" case (not reachable via any legitimate `Voronoi2`/
      `Triangulation2` construction path — `Triangulation2`'s own
      invariant guarantees every listed edge has >= 1 incident face, and
      its fields are private to a sibling module — so a
      `#[cfg(test)] pub(super) clear_adjacent_faces_for_test` corruption
      hook was added to `delaunay2.rs` specifically to reach it; tested:
      `unbounded_edge_missing_incident_face_is_a_typed_error_not_a_panic`).
- [x] `fuzz/fuzz_targets/voronoi_geometry.rs` (registered in
      `fuzz/Cargo.toml`): raw `f64::from_bits` coordinates (not the
      small-integer grid the topology-validator targets use) through
      `delaunay2` -> `voronoi2` -> `vertex_point`/`edge_geometry` on
      every vertex/edge, asserting finiteness (`Point2`/`Segment`/`Ray`
      origin) and ray-direction non-zero-ness; `Err(VoronoiGeometryError)`
      accepted as a correct rejection. Found the `delaunay2()` bug above
      on its first run — a real result, not a clean-bill-of-health.
- [x] Doc sync: `docs/adr/ADR-009-voronoi-geometry.md`'s "Determining the
      ray direction" section revised to describe the fixed algorithm (not
      the original buggy pseudocode) with a "Revised after implementation"
      note explaining both bugs; `VoronoiGeometryError` section documents
      `InvalidTopology`; Status line updated (pushed, CI green at
      `e43b125`, hardening round recorded). `docs/degeneracy-policy.md`
      gained 3 new rows (finite-direction guarantee, the discovered
      `orient2d` inconsistency, `InvalidTopology`).
- [x] **Not done, deliberately**: the `delaunay2()` bug above (separate
      round), `README`/`CHANGELOG.md`/`docs/compatibility.md` sync,
      version bump, publish, tag, release.

## Done (ADR-009 0.7.0: Voronoi vertex/edge geometry, implementation)

- [x] User approved the ADR and implementation start (2026-08-20, "approve
      the ADR (and implementation start)") — the standing roadmap-pacing
      policy's required explicit go-ahead for both the design round
      (already had one) and starting implementation.
- [x] `f995079` refactor(predicates): extract `correctly_rounded_divide`
      (+ `next_up`/`next_down` + its test-only iteration-count
      instrumentation) from `line_intersection.rs` into
      `predicates::constructions::rounding`, `pub(super)` so both
      `line_intersection` and the new `circumcenter` can call it.
      Behavior-preserving, verified: `line_intersection`'s full existing
      test suite (unit + differential) passed unchanged through the new
      call site before this commit landed.
- [x] `e1a05f0` feat(predicates): add `circumcenter` (`Option<Point2>`,
      `predicates::constructions::circumcenter.rs`) — same
      correctly-rounded construction discipline as `line_intersection`
      (ADR-004 Phase 5), a different (but same-degree) formula. Own test
      suite: hand-computed circumcenters (right/equilateral triangles),
      exact-collinear rejection, a reproducible thin-triangle overflow
      case, a divide-loop iteration measurement (2, matching
      `line_intersection`'s own). Landed with `#![allow(dead_code)]`
      (not yet wired to any public API), matching ADR-007 Phase 7A's own
      precedent, as its own commit.
- [x] **Real bug caught while building the overflow test fixture**: a
      first attempt used `c=(0.5, eps)` with `eps` the smallest positive
      subnormal `f64` — `orient2d(a,b,c)` itself reported `Collinear` on
      it (confirmed via a throwaway debug test), meaning that fixture sat
      *below* the `~1.7e-292` exact-product representability floor
      documented in `docs/numerical-model.md` for *every* predicate in
      this crate, not a circumcenter-specific issue — it would have
      tested exactness breakdown in general, not this construction's own
      genuine unbounded-output failure mode. Replaced with an empirically
      swept, verified-safe fixture (`a=(0,0)`, `b=(L,0)`, `c=(L/2,h)`,
      `L=1e75`, `h=1e-170` — both individually far above the
      representability floor, `orient2d` confirmed `CounterClockwise`)
      that genuinely overflows for the right reason (aspect ratio, not
      magnitude-floor breakdown).
- [x] `824ef7d` feat(triangulation): wire up `Voronoi2::vertex_point`/
      `edge_geometry`, `VoronoiGeometryError`/`VoronoiEdgeGeometry`
      (both `#[non_exhaustive]`), re-exported at `triangulation::mod.rs`
      and the crate root (ADR-007 precedent, explicitly checked — the
      first ADR-009 draft didn't say this, added after review). Canonical
      cocircular-group representative face: lexicographically-smallest
      sorted `VertexId` triple (site-identity-keyed, not `FaceId`/scan
      order — matters below the exact-computation floor, where the
      "any member gives the same answer" argument no longer strictly
      holds). Ray direction: `orient2d`-based sign, no division,
      deliberately unnormalized (no `sqrt` anywhere in this crate).
      `Triangulation2` gained a small `pub(super) vertex_point(VertexId)
      -> Point2` accessor (no prior single-vertex lookup existed);
      `Vector2` gained `pub(crate) new_unchecked`, mirroring `Point2`'s.
- [x] Independent `BigRational` oracle (`tests/differential/voronoi_geometry.rs`,
      circumcenter formula re-derived from scratch): random/mixed-magnitude
      triangles, magnitude floor sweep (measured `2^-335`, *identical* to
      `line_intersection`'s own measured floor — confirms both share the
      same degree-3/degree-2 magnitude behavior, not just a similar one,
      resolving ADR-009's own "Assumptions to prove" #1), magnitude
      ceiling sweep past `RESCALE_THRESHOLD` (`1e90`).
- [x] `docs/numerical-model.md` gained a Phase 6 section recording the
      above measurements; `docs/degeneracy-policy.md` gained a
      Voronoi-geometry degenerate-case table — both implementation-round
      deliverables the ADR itself named in advance, not an afterthought.
      `docs/adr/ADR-009-voronoi-geometry.md`'s own Status line updated to
      "Decided and implemented... not yet pushed or released."
- [x] 348 → 354 tests (220 → 225 unit incl. `circumcenter`'s own 8 +
      `rounding`'s relocated 3, 64 → 70 differential incl. 6 new). Full
      quality bar green at every one of the 3 commits above (not just the
      final state): `cargo fmt --check`, `clippy --all-targets
      --all-features -- -D warnings` (native + `wasm32-unknown-unknown`),
      `cargo test --all-features`, `cargo +1.85 test --all-features`
      (MSRV), `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`,
      `cargo deny check` — verified via `git stash push --keep-index -u`
      isolating each commit's actual tree before testing it, not just
      trusting the final combined state was fine.
- [x] **Not done, deliberately**: `README.md`/`README_ja.md`/
      `README_zh.md`/`CHANGELOG.md`/`docs/compatibility.md` sync, version
      bump, `wasm-pack test --node` re-run per commit (ran once against
      the final combined state instead — 10/10 passing), `push`, publish,
      tag, release. Per this project's own "release preparation is its
      own separate round" precedent (0.4.0 through 0.6.0) — these 3
      commits are local only.
- [x] **Not done, deliberately**: no fuzz target added for the new
      `circumcenter` construction, unlike the precedent set by
      `voronoi_topology_validator` (post-0.5.0) and `point_location`
      (post-0.6.0) — deferred to release prep like the rest of this list,
      not an oversight.

## Done (ADR-009 hardening: post-implementation self-review fixes)

- [x] `docs/architecture.md`'s Voronoi-topology paragraph still said
      "Vertex/edge geometry ... is not yet done" — stale as of `824ef7d`.
      Fixed the one sentence (distinct from the still-deferred full
      release-prep sync above, which covers the module tree, not this
      factual claim).
- [x] `VoronoiEdgeGeometry::Segment { start, end }`: the pair's order is
      derived from internal Delaunay face-adjacency order, not sorted by
      any canonical key (unlike `VoronoiEdgeId`/`VoronoiVertexId`, and
      unlike `canonical_edges`' own `endpoints.sort()` precedent in the
      ADR-007 test helpers). Documented as an explicit non-guarantee on
      the variant rather than left implicit.
- [x] **Real bug found and fixed, not just documented**:
      `outward_ray_direction` built a `candidate = pu + perp` `Point2` via
      `Point2 + Vector2`'s unchecked add, purely to test which side of it
      `orient2d` reported — but that add has no finiteness check, so for
      near-`f64::MAX`-magnitude input coordinates `candidate` could
      silently become non-finite, feeding a non-finite `Point2` into
      `orient2d` and picking a ray direction by coin flip instead of
      erroring. Root-cause fix: `perp` (`edge` rotated +90°) is
      *mathematically* always on `orient2d`'s `CounterClockwise` side of
      `pu`/`pv` regardless of magnitude (`cross(edge, perp) = edge.x² +
      edge.y² >= 0`), so the side can be decided without ever
      constructing `candidate` at all. Removes the overflow path
      entirely rather than adding a guard around it. Regression test
      (`ray_direction_correct_side_survives_large_offset_precision_loss`)
      verified against the reverted-to-buggy code first — confirmed it
      fails there before confirming it passes fixed.

## Done (ADR-009: Voronoi vertex/edge geometry design — design only, not implemented)

- [x] `docs/adr/ADR-009-voronoi-geometry.md` — full design for 0.7.0's
      Voronoi geometry API (`Voronoi2::vertex_point`/`edge_geometry`,
      `VoronoiGeometryError`, `VoronoiEdgeGeometry`), following
      `line_intersection`'s correctly-rounded-construction discipline
      (ADR-004 Phase 5) rather than inventing a new one: circumcenter as
      a degree-2-denominator/degree-3-numerator `correctly_rounded_divide`
      construction (same shape as `line_intersection`, `a`'s coordinate
      folded into the numerator to avoid double rounding), ray direction
      as an unnormalized (no `sqrt` anywhere in this crate) exact-difference
      vector with an `orient2d`-based sign choice.
- [x] Central open questions ROADMAP.md's own 0.7.0 design-question list
      named, each answered: (1) correctly-rounded circumcenter
      construction, following `line_intersection`'s precedent; (2) the
      typed error case — `VoronoiGeometryError::NonFiniteCircumcenter`,
      needed because a triangle's circumradius is unbounded relative to
      input magnitude as it approaches collinear (unlike
      `line_intersection`'s ceiling, rescaling cannot fix this — only a
      post-division finiteness check can); (3) unbounded-edge ray
      direction, derived from the dual hull edge via `orient2d` sign
      selection, no division; (4) canonical-per-group circumcenter —
      argued correctly-rounded output is identical regardless of which
      member face supplies it (provable within the exact-arithmetic
      range), with the representative-face pick itself keyed by
      site-identity (lexicographically-smallest sorted `VertexId` triple)
      rather than `FaceId`/scan order, so the choice stays canonical even
      below that range — matching ADR-007's own canonical-id discipline
      one layer deeper.
- [x] Reviewed and revised before finalizing (self-review caught 6 real
      gaps in the first draft): the divide-loop 8-iteration bound needs
      re-measuring against circumcenter's own cancellation shape
      (`line_intersection`'s "2 iterations" result doesn't transfer), the
      ray-direction coordinate difference is correctly-rounded not exact
      (a precision overclaim in the first draft), the group-representative
      pick needed to be site-identity-keyed rather than `FaceId::raw()`
      order, a concrete unit-scale overflow fixture
      (`a=(0,0)`,`b=(1,0)`,`c=(0.5,ε)`) replaces a vague "extreme
      magnitude" claim, `d`-is-zero is checked explicitly rather than
      trusted from the CCW-face invariant (matching ADR-008's
      `validate_topology`-is-test-only posture), and the shared
      construction is placed in `predicates::constructions` (reusable by
      0.8.0/0.9.0) rather than defaulted into the `triangulation` module.
- [x] **Not done, deliberately**: no `src/` code, no `Cargo.toml` change,
      no version bump, no dependency, no performance work. Matches
      ADR-007's own "design round, not an automatic follow-on to
      implementation" precedent — starting implementation remains its
      own separate decision, per `ROADMAP.md`'s "stop after each
      release/round" rule and the standing roadmap-pacing policy (see
      memory).

## Done (post-0.6.0: ROADMAP staged plan through 1.0.0, point_location fuzz target)

- [x] `ROADMAP.md` (internal, gitignored) staged-plan revised: 0.7.0
      Voronoi geometry (circumcenters/rays for the topology-only Voronoi
      diagram 0.5.0 shipped) and 0.8.0 exact nearest-site query inserted
      ahead of the arrangement kernel (now 0.9.0, split into 4 phases)
      and polygon Boolean (now 0.10.0); 2D API hardening renumbered to
      0.11.x with an expanded checklist; 1.0.0 criteria expanded to name
      Voronoi geometry/nearest-site query/arrangement explicitly. Fixed
      one stale claim while merging in the new list: "wasm test
      execution is build-only" — actually shipped (Node.js, 0.4.0);
      narrowed the still-open item to browser execution specifically.
      No commit needed (file is gitignored).
- [x] `fuzz/fuzz_targets/point_location.rs` (commit `b8edad5`) — exercises
      `Triangulation2::locate` across many small-integer-grid point
      clouds and query points, checking the same postcondition
      `tests/differential/locate.rs` verifies against an independent
      oracle at small fixed scale, but against the crate's own
      (already-verified) primitives at fuzz scale instead. Matches the
      `voronoi_topology_validator` precedent (added the same way, same
      shape, after 0.5.0). Ran clean: 65,792 executions / 60s, no
      crashes. fmt/clippy clean on the fuzz crate.
- [x] `docs/degeneracy-policy.md` (commit `30ed670`) gained a point
      location degeneracies section (vertex/edge/hole-boundary/
      hole-interior hits), matching the table format the Voronoi section
      already used.
- [x] `benches/sanity.rs` (commit `124b1b5`) gained a `locate` sanity
      check (n=100/300/1000, timing only, no correctness assertion
      beyond what the existing test suite already covers — matching this
      file's own catastrophic-regression-only precedent).
- [x] All 4 of the above (`b8edad5`..`124b1b5`) are local commits only,
      not yet pushed — same "local only, until the next round" pattern
      the 0.5.0 `voronoi_topology_validator` fuzz-target follow-up used.
      Re-verified together as of 2026-08-20 (this session): `cargo fmt
      --check`, `clippy --all-targets --all-features -- -D warnings`
      (native and separately `--target wasm32-unknown-unknown`), `cargo
      test --all-features` (native), `cargo +1.85 test --all-features`
      (MSRV), `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`,
      `wasm-pack test --node --release` (10/10), `cargo deny check`,
      `cargo build --examples`, `cargo bench --bench sanity` all pass on
      the current working tree. Pushing remains its own explicit decision
      (see this file's "Deferred pending explicit user approval" section
      and `ROADMAP.md`'s release process note) — not done here.

## Done (0.6.0 release: published)

- [x] `v0.6.0` tag at `db6d04c967b61ea54aa045d9b01daca9d8710e34`
      (`docs(changelog): fix 0.6.0 release date to 2026-08-20 (JST)`),
      pushed; CI green on that push (run `32309643905`) and on the
      `Publish` workflow_dispatch run immediately after (`32309825918`).
      GitHub Release "Kika 0.6.0" published 2026-08-19T22:44:17Z.
      crates.io confirmed via the actual API response (not just a
      dry-run): `max_version`/`newest_version`/`default_version` all
      `"0.6.0"`, not yanked. docs.rs build confirmed live (HTTP 200 at
      `docs.rs/kika/0.6.0/kika/`).
- [x] Re-verified directly against the published artifacts this session
      (2026-08-20), not just re-reading the earlier release-checklist
      run: downloaded crates.io source for `kika-0.6.0` confirmed free of
      `.claude/`/`ROADMAP.md`; a fresh external fixture crate (`kika =
      "0.6.0"` pulled from crates.io, not a path/git dependency) built
      and ran `delaunay2`/`Triangulation2::locate` against the real
      published crate, confirming both a `Vertex` hit and an `Outside`
      miss classify correctly.
- [x] `origin/main` == the `v0.6.0` tag's peeled commit, both
      `db6d04c967b61ea54aa045d9b01daca9d8710e34`, confirmed via `git
      ls-remote --tags origin` (peeled with `^{}`) and `git log
      origin/main -1`. Local `main` is *ahead* of that — by the 4
      post-release hardening commits noted above plus this session's own
      doc-only commits, all not yet pushed (see this session's note
      above on pushing remaining its own explicit decision) — unlike the
      0.5.0 precedent entry above, written when nothing was outstanding
      at release time.

## Done (ADR-008 0.6.0: Triangulation2::locate, Round 1 + 2)

- [x] **Round 1** (`2d994eb`, `2364f6e`, `e1bb9ba`, pushed, CI green):
      `docs/adr/ADR-008-point-location.md` written before implementation
      (matches the ADR-006/007 convention). `PointLocation::{Vertex,
      Edge, Face, Outside}`, closed (not `#[non_exhaustive]`) since its
      4 variants are the closure of `Triangulation2`'s own already-closed
      `VertexId`/`EdgeId`/`FaceId` vocabulary plus the necessary miss
      case. `locate()`: `O(F)` scan over `faces().zip(triangles())`
      (index-parallel by construction, no coordinate lookup table),
      `Segment2::relation_to` over a face's 3 actual edges to
      disambiguate an `OnBoundary` hit into `Vertex`/`Edge` — proof this
      always finds a match for a non-degenerate CCW face is in the ADR.
      Never panics (the 2 fallthrough spots that might look like
      `unreachable!()` candidates both `continue`, since
      `validate_topology()` is a test-only diagnostic, never a
      construction-time gate). A plan-agent review before implementation
      caught 2 real corrections (drop the coordinate table in favor of
      zipping `faces()`/`triangles()`; never panic) baked into the
      design before any code was written.
- [x] **Round 2** (`98e3ecb`, `1216571`, pushed, CI green): explicit
      `O(F)` complexity statement in the doc comment; a
      shared-interior-edge order-independence test (2
      differently-face-ordered `assemble_triangulation` instances,
      same edge result either way, compared by endpoint coordinates
      since raw `EdgeId` is independently numbered per instance); an
      outer-vs-hole classification test (caught a real test-fixture
      mistake before landing: the first candidate point (7,7) sits
      exactly on a genuine diagonal edge this triangulation draws from
      (10,10) to a hole corner, verified via a throwaway debug example
      before picking (7,2)); and an independent BigRational oracle
      (`tests/differential/locate.rs`, duplicating
      `point_in_triangle.rs`'s oracle machinery) checking `locate`'s
      actual aggregation/dispatch logic and its full postcondition
      (Vertex/Edge/Face/Outside) directly against the oracle, not this
      crate's own `Triangle2::relation_to`/`Segment2::relation_to`
      (which would only re-test internal consistency).
- [x] 333 tests total (was 320 pre-0.6.0): 212 unit (+10), 64
      differential (+2), 35 adversarial, 7 regression, 15 doctests (+1).
- [x] **Not done, deliberately**: walking locator, spatial index,
      nearest-neighbor query, `ConstrainedTriangulation2`-specific
      forwarding method (use `cdt.triangulation().locate(p)`), new
      dependencies, performance optimization/measurement beyond what's
      already stated as out of scope in the ADR.

## Done (post-0.5.0: voronoi_topology_validator fuzz target)

- [x] `fuzz/fuzz_targets/voronoi_topology_validator.rs` — the same
      small-integer-grid `common::points_from` fixture
      `triangulation_topology_validator` uses (it produces cocircular
      and collinear configurations often), calling
      `Voronoi2::validate_voronoi_topology` on `voronoi2(delaunay2(...))`.
      Not on any pre-existing required-target list (AGENTS.md §12's list
      predates Voronoi entirely) — added because it directly matches
      this crate's own established per-algorithm fuzz-target pattern for
      newly-shipped topology-validating code, same shape as
      `delaunay_insert`/`triangulation_topology_validator`/
      `polygon_validity`. Ran clean: 143,971 executions / 60s, no
      crashes. `cargo fmt`/`clippy` clean on the fuzz crate. Commit
      `a0ca5fa`, local only.

## Done (0.5.0 release: published)

- [x] Release preparation (5 commits, `7978207`..`ef5cdda`): version
      bump, `CHANGELOG.md` `[0.5.0] - 2026-08-19` entry, `README.md` ×3
      languages synced (also fixed a pre-existing translation-drift
      issue in `README_ja.md`/`README_zh.md`'s Roadmap closing
      sentence), `docs/degeneracy-policy.md` gained a Voronoi
      degeneracies table (each row backed by an actual run, not just
      derived), `docs/compatibility.md` synced, `examples/voronoi.rs`
      added (self-checking, matching `constrained_delaunay.rs`'s
      precedent), `docs/release-checklist.md` rewritten with real
      verification results (320 tests, MSRV, wasm native+node, `cargo
      deny`, `cargo package --list`, `cargo publish --dry-run`).
- [x] Pushed, CI green (all 10 jobs, run `32183491104`).
- [x] Clean-worktree re-verification via a genuine fresh clone (not
      just `--allow-dirty` on the dev tree): `cargo package --list` (98
      files), `cargo publish --dry-run`, confirmed `.claude/` and
      `ROADMAP.md` both absent -- this caught a real, if
      environment-local, issue: the dev tree's own dry-run had
      `.claude/scheduled_tasks.lock` (this session's tooling state,
      never git-tracked) leak into its local package listing. Traced to
      root cause via `.github/workflows/publish.yml` (fresh
      `actions/checkout`, so that file never exists there) rather than
      assumed harmless.
- [x] Published via `publish.yml` (`workflow_dispatch`), confirmed on
      crates.io (`max_version`/`newest_version` both `0.5.0`).
      Downloaded and inspected the actual published tarball directly
      (not just the dry-run) -- 98 files, `.claude`/`ROADMAP.md` both
      confirmed absent from the real artifact.
- [x] Fresh external fixture (`kika = "0.5.0"` pulled from crates.io,
      not a path/git dependency) confirmed: `Voronoi2` construction,
      bounded/unbounded cell detection, `cell_edges()`,
      `validate_voronoi_topology()` clean, and -- the one that actually
      matters -- a cocircular square exposes no spurious tie-break
      diagonal edge (1 Voronoi vertex, 4 edges, all `Unbounded`).
- [x] docs.rs build confirmed live (`Voronoi2`/`voronoi2` visible in the
      generated page at `docs.rs/kika/0.5.0/kika/`).
- [x] `v0.5.0` tag created and pushed at `ef5cdda65692fba2446442c16f23b426ffbe9b8d`;
      GitHub Release "Kika 0.5.0" created. Final SHA check: local main =
      origin/main = peeled `v0.5.0` tag, all matching.
- [x] `ROADMAP.md` (internal, gitignored) updated to reflect the actual
      shipped state -- no git commit needed for that file.

## Done (ADR-007 Phase 7C: Voronoi cell_edges + 0.5.0 readiness check)

- [x] Investigated before implementing (per instruction): does
      `cell_edges()` reduce cleanly to existing `Triangulation2` face
      adjacency, no new half-edge structure? Worked out and hand-verified
      (concrete square example, traced before writing code) that
      rotating a site's incident faces in a fixed direction visits them
      in the same order the geometric cell boundary would; a step whose
      Delaunay edge was excluded (same cocircular group) simply has no
      entry in `edges`, which is the skip signal — no special-casing.
      Proved cocircular merging can't split one cell's walk into two
      disconnected runs of the same `VoronoiVertexId` (shared-vertex +
      both circles' defining triples force one common circle, always
      caught by `voronoi2`'s exhaustive adjacent-pair testing).
      Conclusion: natural, no half-edge structure needed — implemented.
      Commit `45a91d1`.
- [x] First draft's cycle-detection loop reused its own ending position
      as the forward walk's start; wrong for the interior/cyclic case
      (needs the *original* starting face) and would have silently
      dropped one edge per interior cell. Caught by hand-tracing before
      any code was written, not by a test.
- [x] `VoronoiTopologyError::EdgeNotInExactlyTwoCellBoundaries` — every
      edge reachable from exactly 2 cells' walks. Documented explicitly
      as a coverage check (catches a dropped step), not an ordering
      check, after review flagged that the lookup it's built on is
      symmetric by construction and so cannot detect wrong-order or
      duplicate emissions on its own.
- [x] Tests: cocircular square/5-/8-point n-gons (2 rays per cell, no
      interior edge survives); a hand-verified exact edge order on a
      partially-cocircular fixture; a near-cocircular-but-not quad
      (confirms the exact predicate doesn't merge a close miss); a
      60-point fixed-seed cloud checking `edge_cells()` consistency and
      — the discriminating check — that consecutive edges in each cell's
      walk share exactly one Voronoi vertex, cyclically for interior
      cells and linearly (no wraparound) for hull cells. The hull-cell
      half of that check was originally missing (first/last-ray-only
      assertion would pass a walk with the middle reversed) — added
      after review.
- [x] 0.5.0 readiness check: `ROADMAP.md` (internal)'s "design approved,
      implementation not started" 0.5.0 section was stale, rewritten to
      reflect Phase 7A/7B/7C completion (confirms the section's own
      original scoping call — "0.5.0 does not need to expose an actual
      `Point2` coordinate" — held exactly, `cell_edges()` included, no
      coordinate construction anywhere). `docs/architecture.md`'s module
      tree and its 2 stale "implementation not started" mentions fixed.
      `CHANGELOG.md`'s `[Unreleased]` section populated with the full
      feature summary (no version header/date -- that's a release-time
      action, not done here).
- [x] **Not done, deliberately**: circumcenter coordinates, clipping,
      nearest-neighbor, performance optimization, new dependencies,
      competitive benchmarks, version bump, publish, tag, release. Not
      pushed — local commit only, per instruction.

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
