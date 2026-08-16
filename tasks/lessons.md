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
