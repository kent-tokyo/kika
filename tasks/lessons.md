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
