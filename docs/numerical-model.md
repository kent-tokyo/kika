# Numerical model

Status: Phase 1 (predicates). Extended in later phases as constructions are
added.

## Input

Public geometric types (`Point2`, `Point3`) hold `f64` coordinates and can
only be constructed via `Point2::new`/`Point3::new`, which reject NaN and
infinite values (`KikaError::NonFiniteCoordinate`). See ADR-003. This is the
"validated finite coordinate type" option from AGENTS.md §7.1, chosen over
`Result`-returning predicate functions.

Internal code may assume all `f64` values it sees from a constructed
`Point2`/`Point3` are finite.

## Exact arithmetic core (`predicates::expansion`)

Built from Dekker's (1971) error-free transformations, as popularized for
robust predicates by Shewchuk (1997) — implemented from the published
algorithm description, not copied from any specific codebase (see
ADR-005).

* `two_sum(a, b) -> (hi, lo)`: `hi = fl(a+b)`, and `hi + lo == a + b` exactly
  (as real numbers), for **any** `f64` `a`, `b` (no magnitude ordering
  required). 6 flops.
* `fast_two_sum(a, b) -> (hi, lo)`: same postcondition, 3 flops, but
  requires `|a| >= |b|`.
* `split(a) -> (hi, lo)`: splits a 53-bit mantissa into two ~26-bit halves
  whose product with another split value can be summed without rounding.
  Uses splitter `2^27 + 1`.
* `two_product(a, b) -> (hi, lo)`: `hi = fl(a*b)`, and `hi + lo == a * b`
  exactly, for any `f64` `a`, `b`, built from two `split` calls and 17
  flops.

**Why not `f64::mul_add`:** see ADR-001. In short: exactness of an
FMA-based `two_product` depends on the platform FMA/software-fallback being
correctly rounded, which we did not want as a portability assumption across
x86_64/aarch64/wasm32. The split-based `two_product` uses only `+`, `-`,
`*`, which Rust never silently contracts into fused operations — so it is
exact and portable without relying on the FMA question at all.

`grow_expansion(e, b) -> Vec<f64>`: adds a single `f64` `b` to a
nonoverlapping expansion `e` (a `Vec<f64>` sorted by increasing magnitude,
components pairwise nonoverlapping in the Shewchuk sense), producing a new
nonoverlapping expansion with the same sum, length `|e| + 1`.

**Sign of an expansion:** for a nonoverlapping expansion in increasing
order of magnitude, the sign of the total sum equals the sign of the last
(most significant) nonzero component. This lets us extract an exact sign
without a final (potentially rounding) summation step. This invariant is
covered by a dedicated test (`expansion::tests::sign_matches_leading_term`),
not just asserted.

## Filter: computed error bound, not a fixed epsilon

Each predicate first computes its determinant directly in `f64` (the
"fast path"). Alongside it, it computes an error bound that is a linear
function of the *magnitudes of the actual input coordinates and
intermediate products* — never a constant. The general form (Shewchuk
1997, §3–5) is:

```text
error_bound = error_bound_factor * sum_of_absolute_values_of_terms
```

where `error_bound_factor` is a small constant derived from `f64` machine
epsilon (`2^-53`) and the number of arithmetic operations in the specific
determinant formula (more operations → more accumulated rounding → a
larger, but still derived, factor). The derivation for each predicate is
recorded next to its filter constant in source (`predicates/orientNd.rs`
etc.) and summarized below as each predicate is implemented:

* `orient2d`: implemented. See `ORIENT2D_ERR_BOUND_FACTOR` in
  `src/predicates/orient2d.rs` for the full derivation (`~4u *
  (|left|+|right|)`, generous constant `7u`). Verified, not just derived:
  `tests/differential/orient2d.rs` checks predicate output against an
  independent `num-rational` oracle across random points, extreme and
  mixed scales, and inputs deliberately walked across the filter's
  conclusive/inconclusive boundary near collinearity.
* `orient3d`: implemented. See `ORIENT3D_ERR_BOUND_FACTOR` in
  `src/predicates/orient3d.rs`. Exact fallback uses `scale_expansion`
  (new primitive: an expansion times a scalar, needed for the
  triple-product terms of a 3x3 determinant) on top of the same
  `product_expansion`/`expansion_sum`/`expansion_sign` building blocks.
  Verified against an independent oracle in
  `tests/differential/orient3d.rs`.
* `incircle`, `insphere`: same pattern, added as each lands.

If `|determinant| > error_bound`, the sign of `determinant` is provably the
true sign and is returned without any fallback. Otherwise, the exact
fallback (above) runs.

## Known limitation (fixed): exactness starts at the original coordinates

Found during development, not a present-tense limitation — recorded here
because it shaped the exact-fallback design and is exactly the kind of
mistake worth guarding against when `incircle`/`insphere` are added.

The first version of `orient2d_exact`/`orient3d_exact` reused the
filter's already-computed `acx = a.x() - c.x()` (a single, possibly
rounding, `f64` subtraction) and only went exact *from that point on*
(building `product_expansion(acx, bcy)` etc.). That is exact relative to
the *rounded* difference, not the original coordinates: `fl(a-b)` for two
arbitrary finite `f64`s can require more than 53 bits to represent
exactly and therefore loses information (concretely: `a.x()=2^60`,
`c.x()=1.0` gives `fl(a.x()-c.x()) == 2^60`, discarding the `-1.0`
entirely — confirmed with `two_sum`, whose second component is `-1.0`,
not `0.0`).

A 2,000,000-trial random search comparing this once-rounded-then-exact
sign against a fully exact sign (computed from the original coordinates
via exact rational arithmetic) found 1,327 disagreements, all requiring
wide dynamic range *within a single predicate call* (e.g. coordinates
around `2^60` alongside small integers) — same-scale random inputs never
exposed it, which is why the original differential test suite (same-scale
generators) passed despite the bug.

**Fix:** `orient2d_exact`/`orient3d_exact` now build every coordinate
difference as an exact 2-component expansion via `diff_expansion` (`=
two_sum(a, -b)`, exact for *any* finite `a`, `b` — no exponent-range
restriction, unlike `two_product`) directly from the original
`Point2`/`Point3` coordinates, and multiply those expansions together via
the new `product_of_expansions` primitive (distributes `scale_expansion`
over each component of the second expansion, mirroring how
`scale_expansion` itself distributes `product_expansion`). The filter
path is unchanged — it was already derived treating the true (unrounded)
coordinate difference as ground truth, so its error bound already covers
the subtraction's own rounding; only the fallback's starting point
changed. See `tests/regression/orient2d.rs` for the pinned minimized
cases, and `tests/differential/*.rs`'s `mixed_intra_call_magnitude` tests
for the regression-class coverage.

**Consequence for `incircle`/`insphere`:** their lifted coordinate
(`adz = adx^2 + ady^2` and higher) must be built the same way — as an
exact expansion of an exact expansion (`product_of_expansions(adx, adx)`
summed with `product_of_expansions(ady, ady)`), never a squared rounded
`f64` — or they inherit this bug in a compounded form (a rounded square
of an already-rounded difference).

## Known limitation: exact-product representability floor

`two_product` (and by extension `grow_expansion`/`expansion_sum` built on
it) represents an exact product as two `f64` values, `hi` and `lo`. This
representation has a hard, algorithm-independent floor: it requires the
true rounding error of `hi = fl(a*b)` — a value of magnitude roughly
`ulp(hi)`, i.e. about `2^-53` relative to `hi` — to itself be exactly
representable as a single `f64`. Since the smallest representable `f64`
magnitude at all (a subnormal) is `2^-1074`, and the error term's own
representable precision costs another `~2^-53` relative to *its* leading
bit, the combined requirement works out to:

```text
|a * b| >= 2^-968  (~1.7e-292)
```

Below that, no two-`f64` exact-product representation can be fully exact —
**verified empirically to affect a correctly-rounded FMA-based
`two_product` identically to this split-based one** (both were checked
against an exact-rational oracle down to product magnitudes of `1e-320`;
both start losing exactness at the same `~1e-292` threshold). This is not
a consequence of choosing split over FMA (ADR-001's FMA-portability
argument stands on its own, independent of this finding) — it is inherent
to representing an exact product as a pair of `f64`s at all.

**Practical impact:** for `orient2d`/`orient3d`/`incircle`/`insphere`,
this only matters on the rare exact-fallback path (the filter already
handles the overwhelming majority of inputs). Exactness on that fallback
path is guaranteed for coordinate differences with magnitude down to
roughly `1e-140` each (worst case: two such values multiplied together
give `~1e-280`, comfortably above the `1e-292` floor). Genuinely
subnormal input coordinates (magnitude below `f64::MIN_POSITIVE`,
`~2.2e-308`) are below this floor; Kika does not claim exact-fallback
correctness for them. It does still guarantee the universal API contract
(no panic, no NaN/Infinity output from finite input) —
`expansion::tests::two_product_true_subnormal_operand_does_not_panic`
covers this. No real-world coordinate system operates anywhere near this
boundary (`1e-140` is many orders of magnitude below the Planck length in
any physical unit), so this is not expected to matter in practice; it is
documented rather than silently assumed away, per AGENTS.md §20.

**Upgrade path**, if a future use case ever needs it: rescale operands by
an exact power of two before calling `two_product` when either is outside
the safe band, then rescale the result back — power-of-two scaling is
lossless in IEEE-754 up to its own overflow/underflow limits, and would
push the floor down by roughly the scaling exponent. Not implemented in
Phase 1 (no known need).

## What is *not* claimed

This is a two-stage (filter + exact) model, not Shewchuk's three-stage
adaptive-precision scheme (see ADR-001). We do not claim "adaptive
precision" anywhere in code or docs. Fallback-rate measurements (§13) will
be reported once benchmarking exists, not assumed.
