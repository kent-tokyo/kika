# Lessons

Notes on decisions that took real investigation, so they aren't re-litigated.

- Chose Dekker split-based `two_product` over FMA-based (`f64::mul_add`)
  for the exact arithmetic core, specifically to avoid depending on FMA
  correct-rounding guarantees on wasm32-unknown-unknown (no native FMA).
  Rust does not contract separate `+`/`-`/`*` into fused ops, which is what
  makes the split-based approach portable. See ADR-001.
- Empirically verified (against an exact-rational oracle, and against a
  correctly-rounded-FMA emulation) that exact two-`f64` product
  representation has a hard floor around `|a*b| < 1.7e-292`, independent
  of split-vs-FMA. First guess at the threshold (naive `2^-1021` from a
  single 53-bit headroom argument) was wrong by ~50 bits; the real
  requirement is two rounds of 53-bit headroom (product rounding, then the
  error term's own precision), landing near `2^-968`. Don't trust a
  first-pass error-bound derivation without checking it against measured
  data — see `docs/numerical-model.md` "Known limitation".
- Real bug, caught before release: `orient2d_exact`/`orient3d_exact`
  reused the filter's once-rounded coordinate difference (`a.x()-c.x()`
  as a plain f64) instead of recomputing it exactly. Same-scale
  differential test generators (all of `a`,`b`,`c` drawn from one scale
  bucket) never exposed it — only a generator mixing wildly different
  magnitudes *within a single call* (e.g. `2^60` next to small integers)
  did, found via a 2M-trial random search outside the normal test suite.
  Lesson: a differential test suite's coverage is defined by its
  generators; same-scale-only generators systematically miss
  intra-call dynamic-range bugs. See `docs/numerical-model.md` "Known
  limitation (fixed): exactness starts at the original coordinates" and
  `tests/regression/orient2d.rs`.
- Second real bug, same session: `incircle`'s (and latently `orient3d`'s)
  filter bound used post-subtraction term magnitudes
  (`|term_a|+|term_b|+|term_c|`) instead of pre-subtraction cofactor
  magnitudes. Wrong whenever the *inner* subtraction inside a cofactor
  term cancels catastrophically — orient2d never had this flaw because
  its determinant has no inner subtraction, only the outer one, which was
  already bounded correctly. When a filter's determinant formula has more
  than one subtraction in its dependency chain, each one needs its own
  pre-subtraction-magnitude bound term, not just the outermost one.
  Diagnosed by checking `incircle_exact`'s intermediate expansions
  against the bigint oracle step by step first (all exact — ruled out the
  fallback), which pointed straight at the filter. See
  `docs/numerical-model.md` "Known limitation (fixed): filter bound must
  use pre-cancellation magnitudes".
- Not a bug, a test-authoring trap: an `insphere` adversarial test used
  `1.0 / 2.0_f64.sqrt()`-based coordinates intended to be exactly
  cospherical, and got `Positive` instead of the expected `Zero`.
  Verified against the exact-rational oracle that `Positive` was
  correct: `sqrt()` is irrational, so its `f64` rounding means the
  constructed points are only cospherical to ~1e-15 relative precision,
  not exactly — and the whole point of an exact predicate is to notice
  that. Any test asserting an *exact* degeneracy (collinear, coplanar,
  cospherical, cocircular) needs exactly-representable (integer/rational)
  coordinates, never `sqrt`/`sin`/`cos`-derived ones.
- Performance bug, found implementing `insphere`: exact fallback took
  16s/call on degenerate inputs. Root cause was combining N small
  expansion pieces via a left-to-right fold into one growing accumulator
  — O(N²) regardless of how fast each individual merge step is, since
  each step's cost scales with the accumulator's current size. Fixing
  `expansion_sum` itself to be O(n+m) (merge-by-magnitude + single
  `two_sum` cascade, instead of repeated single-element injection) was
  necessary but not sufficient; `scale_expansion`/`product_of_expansions`
  also needed to switch from a linear fold to a balanced binary-tree
  merge. `incircle` (degree 4) never showed this because its expansions
  stayed short enough for the constant factor to not matter — `insphere`
  (degree 5, one more nesting level) crossed the threshold where it does.
  The two ~10-line, well-tested "obviously fine" primitives (`fast_two_sum`,
  `grow_expansion`) became fully unused once `expansion_sum` no longer
  needed single-element injection, and were deleted rather than kept
  "just in case" — see `docs/numerical-model.md` "Known limitation
  (fixed): naive expansion merging is quadratic".
- Documentation bug caught by verifying before writing, not after: while
  writing `docs/degeneracy-policy.md` for `insphere`, assumed by analogy
  with `incircle` ("collinear a,b,c ⟹ zero for d off-line is false, but
  d-on-line ⟹ zero") that "coplanar a,b,c,d ⟹ zero for e off-plane" would
  be the corresponding false claim and "e on-plane ⟹ zero" the true one.
  Checked numerically before committing the doc: a square's corners
  (coplanar *and* concyclic) do give zero for any `e`, but a generic
  (non-concyclic) coplanar quadrilateral gives a *nonzero* determinant
  even for `e` exactly on the plane. The real degenerate condition is
  coplanar-and-concyclic-within-the-plane, not mere coplanarity — the
  existing `insphere` unit test (`coplanar_abcd_is_zero`, using a square)
  was accidentally passing for the wrong reason. Lesson: an analogy
  between two predicates' degenerate cases is a hypothesis, not a fact —
  check it the same way as any other numerical claim before writing it
  into docs or tests, even when it "obviously" generalizes.
- Phase 2 bug, same shape as the insphere-coplanar one: `Triangle2::relation_to`'s
  general algorithm (3 `orient2d` edge-side checks) silently breaks for a
  degenerate (collinear-vertex) triangle, because all three checks are
  trivially `Collinear` for *any* point on the shared line — the test
  can't tell "within the triangle's degenerate span" from "same line,
  miles away". Caught by a test (`p` far outside the span on the shared
  line) written specifically to probe the degenerate case, not by
  assuming the general algorithm would handle it — the exact assumption
  that broke `incircle` earlier this session. Lesson reinforced: a
  predicate built by composing several exact sub-predicates (`orient2d`
  ×3 here) needs its *own* degenerate-case analysis; composing exact
  parts doesn't automatically make the composition's edge cases exact.
  See `docs/degeneracy-policy.md` and `tests/regression/point_in_triangle.rs`.
- Phase 3 design-time bug, caught by hand-tracing before writing any code:
  the standard Andrew monotone-chain algorithm, applied naively to a fully
  collinear point set in "keep all boundary points" mode, produces a
  self-retracing/duplicated result (e.g. `[A,B,C,D,C,B]` for 4 collinear
  points) — both the lower and upper chains independently retain every
  point, since nothing ever triggers a pop in either direction. Fixed by
  detecting full collinearity explicitly (an `orient2d` check against the
  two lexicographic extremes) before running the chain construction at
  all, rather than letting the general algorithm hit the case. See
  `docs/degeneracy-policy.md` and `src/hull/convex_hull2.rs`.
- Phase 3, a second design-time check that paid off: considered detecting
  the fully collinear case *after* the fact instead, from the chain
  lengths (e.g. "the lower chain absorbed every point"), which would avoid
  the extra `orient2d` sweep. Verified by hand-checking a "valley" point
  set (samples of `y = x^2`, genuinely 2D, not collinear) that this
  specific heuristic gives a false positive — a convex/concave curve
  legitimately puts every point on one monotone chain while the other
  stays trivial. Kept the explicit precheck instead; see
  `tests/differential/convex_hull2.rs`'s `valley_shape_is_not_treated_as_collinear`.
  Lesson: a proposed simplification that trades an explicit check for an
  inferred one needs the same "verify before trusting" treatment as any
  other numerical claim — a plausible-sounding heuristic is a hypothesis,
  not a fact, even when it comes from a second opinion.
- Phase 3 bug, caught during review before writing the implementation: a
  planned `total_cmp`-based sort+dedup for collapsing duplicate input
  points didn't account for `total_cmp` treating `-0.0 < 0.0` while
  `Point2`'s `PartialEq` (and IEEE-754) treat them as equal. A
  consecutive-element `dedup()` after such a sort can miss a real
  duplicate if a third point's other coordinate happens to sort between
  the `-0.0`/`0.0` copies (e.g. `(-0.0,5.0)`, `(0.0,3.0)`, `(0.0,5.0)` —
  the two equal points land 2 apart, not adjacent). Fixed by normalizing
  `-0.0` to `0.0` in the sort comparator itself, so the sort's notion of
  "equal" matches `PartialEq`'s exactly and equal points are always
  adjacent. See `src/hull/convex_hull2.rs`'s `normalize_zero` and the
  regression-style unit test `signed_zero_duplicate_collapses`.
- Phase 4, real bug found in production (would have shipped without
  property testing): a first `delaunay2` implementation seeded
  Bowyer-Watson with a synthetic "super-triangle" (bounding-box-derived,
  scaled 20x), stripped at the end. Passed every hand-written unit test —
  including several targeting degenerate cases — but a property test on
  *ordinary* random point clouds (not an adversarial construction) found
  it silently dropping a triangle: 2 instead of the topologically-required
  3 for a 4-point input (3 hull, 1 interior). The general lesson, sharper
  than "pick a bigger constant": **a synthetic coordinate introduced for
  algorithmic convenience carries a scale-dependent correctness condition
  that no fixed constant satisfies** — the governing ratio here was
  bounding-box diagonal to smallest relevant point spacing, which is
  unbounded for an unlucky (not even adversarial) input, so the same trap
  is waiting for any future phase tempted to reach for a bounding box or
  similar synthetic construct (Phase 6 polygon Boolean is a likely
  candidate). Fixed by removing the synthetic coordinate entirely — a
  single symbolic "point at infinity" ghost vertex with an exact
  `orient2d`-based reduction, no coordinate, no scale dependency. See
  `docs/numerical-model.md`'s Phase 4 section and
  `tests/regression/delaunay2.rs` for the full diagnostic trail
  (property test → minimized 4-point counterexample → root cause →
  redesign → re-verify).
- Phase 4, a design mistake caught and corrected mid-flight by a second
  opinion: an initial plan for the ghost-vertex fix used *three* separate
  ghost points (mirroring the removed super-triangle's three corners) with
  the rule "a triangle with 2 or more ghosts is always invalidated by any
  new point." Implementing and hand-tracing this showed it discards
  already-inserted real points (a triangle fan entirely around one real
  point, made of three 2-ghost triangles, gets *completely* replaced by
  the next insertion regardless of where that point actually lands, since
  "always bad" doesn't check position). The correct model uses a *single*
  conceptual point at infinity — matching the well-established
  Guibas-Stolfi formulation — so at most one ghost vertex can ever appear
  in any triangle, provable by induction, no unconditional "always bad"
  rule needed anywhere. Lesson: verify a design change by hand-tracing its
  simplest non-trivial case (here: what happens on the *second* point
  insertion) before trusting it generalizes, the same discipline as
  verifying any other algorithmic claim in this project.
- Phase 4, a test-helper bug found twice in one session, in two different
  property checks: comparing a triangulation's structure against
  `convex_hull2(points, HullBoundaryPoints::ExtremesOnly)` is wrong
  whenever the input has a collinear boundary point — that point is a
  real triangulation vertex (splitting what would otherwise be one hull
  edge into two), so `ExtremesOnly`'s strict-corners-only count
  undercounts both the expected triangle count (`2n - 2 - h`, Euler's
  formula) and the expected set of "unmatched" (hull-boundary) mesh
  edges. Both checks needed `HullBoundaryPoints::KeepAllOnBoundary`
  instead. Not a bug in `delaunay2` either time — a reminder that a test
  oracle built from one of this crate's own APIs needs the *same* degree
  of "which mode actually matches what I'm checking" scrutiny as the
  production code it's verifying, especially when two sibling checks
  (triangle count, edge-matching) both silently made the same wrong
  choice.
- Phase 5, a wrong a priori assumption caught by measuring before writing
  it down as fact: assumed `line_intersection`'s safe magnitude range
  would be *narrower* than `incircle`'s, reasoning that its extra
  multiplication (`d1`/`d2` scaled once more by a coordinate, on top of
  the degree-2 cross product) would make things worse than `incircle`'s
  already-narrow range. The actual empirical sweep
  (`tests/differential/line_intersection.rs`'s `magnitude_floor_sweep`)
  found the opposite: safe down through `2^-335`, comfortably *wider* than
  `incircle`'s `~1e-70`. The real governing factor is polynomial *degree*
  (`line_intersection` is degree 3; `incircle` is degree 4, from its
  paraboloid lift), not "one more multiply feels riskier" intuition, and
  not "predicate vs. construction". This is the same discipline as the
  `1.7e-292` representability-floor lesson above (first-pass derivation
  wrong by ~50 bits, corrected by measurement) — applied here to a
  direction-of-effect guess instead of a numeric constant. Caught before
  it shipped as documentation, not after: the doc comment was corrected to
  state the measured fact once the sweep contradicted the draft.
- Phase 5, a real risk caught by advisor review rather than by testing:
  `correctly_rounded_divide`'s refinement loop was bounded at 8 iterations
  "as a safety net", with a doc comment claiming the initial `f64`-division
  guess would need "at most a few steps" to refine — asserted, not
  measured. Advisor review pointed out this is the same shape of mistake
  as the Phase 4 super-triangle scale constant: an unverified assumption on
  a path whose entire purpose is a correctness guarantee, where silently
  returning a wrong-but-finite answer on loop exhaustion is exactly the
  failure mode this whole construction exists to prevent. Fixed by adding
  a `#[cfg(test)]`-only instrumented counter and a dedicated test
  (`divide_loop_iteration_bound_is_generous`) exercising both ordinary
  random crossings and deliberately near-parallel ones (small `d1-d2`,
  the case most likely to defeat the initial guess via catastrophic
  cancellation) across magnitude scales from `1e-300` to `1e100`: measured
  worst case is 2 iterations, 4x below the bound. A `debug_assert!` on the
  loop-exhaustion fallback was considered and rejected — `debug_assertions`
  are controlled by the *consuming* crate's build profile, not just this
  crate's own test runs, so it would have introduced a new panic path
  reachable from the public `segment_intersection` API in any downstream
  consumer's ordinary (non-release) build, conflicting with AGENTS.md's
  "no undocumented panics in public API" mandate for an already
  near-unreachable case. Lesson: "the loop won't realistically hit its
  bound" is exactly the kind of claim this project's whole methodology
  says to measure, not assert — and even once measured favorably, a
  defensive check's *reachability* (test-only vs. shipped-to-downstream)
  needs the same scrutiny as the check itself.
- Real bug, found not by the unit test suite but by the Phase 6D sanity
  benchmark's larger inputs: Phase 6C's `insert_constraint_edge`
  described itself as "the standard Sloan-style algorithm, simplified —
  rescan every iteration and flip whichever crossing edge is currently
  flippable, instead of maintaining a persistent queue." That framing was
  wrong in a way 14 passing unit tests (all on ≤8-point grids or short
  constraints) never caught: it isn't a slower version of the same
  algorithm, it's a *different, non-terminating* one. Always picking
  "whichever flippable edge sorts first this scan" can settle into a
  2-cycle — flip edge A, its replacement is still crossing and still
  sorts first next scan, flip it back, repeat — with the crossing-edge
  count never shrinking, confirmed by instrumenting the loop and watching
  `crossing.len()` and each candidate's flippability oscillate between
  exactly two states for ~2300 iterations before hitting the flip bound.
  A single, otherwise-unremarkable long constraint in a 300-point random
  cloud (`benches/sanity.rs`) reproduced it on the first try; no
  degenerate collinearity involved. Fixed by implementing the actual
  standard algorithm: a persistent FIFO queue of crossing edges, popped
  one at a time, flipped if convex (requeuing the fresh diagonal only if
  it's still crossing) or pushed to the back to retry if not yet convex —
  relying on the fact that a flip changes the *existence* of exactly the
  popped edge and its replacement, so no other edge's crossing status can
  change and the queue never needs a full rescan. See
  `tests/regression/cdt.rs` and `src/triangulation/cdt.rs`'s
  `insert_constraint_edge` doc comment. Lesson: a doc comment that
  describes an implementation as "the standard algorithm, just simplified
  for efficiency" is itself a claim that needs checking against the
  actual algorithm's termination argument, not just against small-input
  test results — and a bounded-loop safety net (the flip-count bound)
  caught the *symptom* (returned a typed error instead of hanging or
  corrupting state) but not the *root cause*, which only a benchmark
  exercising realistically-sized, non-adversarial input surfaced.
- Second bug in the same rewrite, caught by advisor review rather than
  execution: the queue-based fix above changed the queue-empty exit to
  "return `Ok(())`" without re-checking that the constraint edge actually
  exists. For a constraint whose segment passes exactly through a third
  input vertex, edges incident to that vertex classify as
  `EndpointTouch`/`CollinearTouch` (never `Proper`) and so never enter
  the crossing queue at all — meaning the queue can drain to empty while
  the constraint itself was never realized, silently returning success
  with the edge missing from `constrained_edges`. A `ConstraintInsertionFailed`
  test for exactly this input (three collinear points, constraint
  spanning the outer two) passed on the first write — not because the
  bug wasn't real, but because the fix's own doc comment already
  described the intended check and the code happened to have it by the
  time the test ran. Reverted the one-line check and re-ran the same
  test to confirm it actually fails without the fix (`Ok(...)` with
  `constrained_edges: {}`) before trusting the green result. Lesson:
  a passing regression test for a rewrite's edge case is only evidence
  if you've watched it fail against the pre-fix code — otherwise it may
  be validating that the fix exists, not that it was necessary.
- Real bug, found by review of `constrained_delaunay2` rather than by new
  test-writing: it called `delaunay2(points)` and looked up every input
  point's `VertexId` by coordinate, ending in `.expect("every input point
  has a VertexId: duplicates were already rejected")`. That message
  states one precondition (no duplicates) but the code actually depended
  on a second, unstated one inherited from `delaunay2`'s own documented
  contract: a non-degenerate point set. `delaunay2` returns an *empty*
  `Triangulation2` for fewer than 3 points or an all-collinear set, by
  design, not an error — so the `.expect` panicked for every point
  whenever that held, even with zero constraints (a single point was
  enough). Fixed by checking `triangulation.is_empty()` right after the
  `delaunay2` call, before the coordinate lookup ever runs: `Ok` wrapping
  the same empty triangulation for an empty `constraints` list, a new
  `CdtError::DegeneratePointSet` otherwise. Lesson: an `.expect` message
  naming one precondition is only as trustworthy as *every* precondition
  the surrounding code actually relies on being enumerated — this one
  silently inherited a second precondition from a callee's own
  carefully-documented return-value contract (`delaunay2`'s doc comment,
  `docs/degeneracy-policy.md`) that was never cross-checked against this
  caller. Confirmed by writing the regression tests against the unfixed
  code first and watching all 5 panic at the exact `.expect` line before
  applying the fix. See `tests/regression/cdt.rs`.
- Real bug, found by review then confirmed empirically rather than
  assumed: `line_intersection`'s doc comment explicitly flagged the
  large-magnitude side as "not independently swept" — a documented gap,
  not a claimed guarantee. A review pass reasoned through the actual
  arithmetic (the degree-3 numerator `d1*b.x()` etc. can overflow
  `f64::MAX`) and predicted two distinct failure mechanisms: uniform-magnitude
  overflow around `~5.6e102`, and a sharper mixed-magnitude one (`K²·M`
  overflowing even when both `K` and `M` individually sit far below that
  threshold) that a first-pass analysis missed entirely and that changes
  the *shape* of any correct fix (a joint condition, not a scalar "safe up
  to X" claim). Both were then confirmed real, not just plausible, by two
  targeted tests (`extreme_uniform_magnitude_is_finite`,
  `extreme_mixed_magnitude_is_finite`) that reproduced actual `NaN` output
  before any fix existed. Fixed with exact power-of-two rescaling — a
  technique this crate had already named as the known upgrade path for
  the analogous *floor*-side problem, just never applied to the ceiling
  side. Lesson: a documented "not swept" limitation is not the same as
  "assumed safe" — it's an open question, and the second (mixed-magnitude)
  failure mode here was only found by actually deriving the overflow
  condition algebraically, not by pattern-matching against the first
  (uniform-magnitude) one that seemed like the obvious story.
- Design decision, 0.3.0: which public enums get `#[non_exhaustive]`.
  Prompted by `CdtError::DegeneratePointSet` (see the entry above) landing
  on an enum that wasn't `#[non_exhaustive]` in 0.2.0 — itself a breaking
  change for any consumer with an exhaustive `match`, which is why that
  fix shipped as 0.3.0 rather than 0.2.1. Inventoried all 11 public enums
  (`grep -rn "^pub enum" src/`) and split them on one question: is this
  the `E` in a `Result<T, E>` (or diagnostic-list equivalent), or is it a
  classification of a mathematically/geometrically closed outcome set?
  `KikaError`, `CdtError`, `PolygonTriangulationError`, and (doc-hidden,
  but still technically `pub`) `TopologyError` got `#[non_exhaustive]` —
  each is "why did this fallible operation reject/fail," and `CdtError`
  already proved that set grows. `Sign`, `Orientation`,
  `PointSegmentRelation`, `PointTriangleRelation`,
  `SegmentIntersectionKind`, `SegmentIntersection2`,
  `PolygonBasicValidity`, `HullBoundaryPoints` were left exhaustive: none
  are `Result` error types (no `Error` impl, not returned as `Err`), and
  each enumerates a fixed, complete geometric/structural classification by
  construction — `SegmentIntersectionKind`'s own doc comment even says so
  explicitly ("a zero-length input segment is not a separate variant ...
  folds into `EndpointTouch`/`None`"). Lesson for the next public enum
  this crate adds: ask "is this a `Result` error, or a closed
  classification" first — the answer settles `#[non_exhaustive]`
  immediately, no case-by-case guessing needed. See `CHANGELOG.md`'s
  0.3.0 entry.
