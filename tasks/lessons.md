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
