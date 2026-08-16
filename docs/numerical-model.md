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
* `split(a) -> (hi, lo)`: splits a 53-bit mantissa into two ~26-bit halves
  whose product with another split value can be summed without rounding.
  Uses splitter `2^27 + 1`.
* `two_product(a, b) -> (hi, lo)`: `hi = fl(a*b)`, and `hi + lo == a * b`
  exactly, for any `f64` `a`, `b`, built from two `split` calls and 17
  flops.
* `product_expansion`/`diff_expansion`: the 2-component nonoverlapping
  expansion for `a*b` / `a-b`.

**Why not `f64::mul_add`:** see ADR-001. In short: exactness of an
FMA-based `two_product` depends on the platform FMA/software-fallback being
correctly rounded, which we did not want as a portability assumption across
x86_64/aarch64/wasm32. The split-based `two_product` uses only `+`, `-`,
`*`, which Rust never silently contracts into fused operations — so it is
exact and portable without relying on the FMA question at all.

**Combining expansions**, all built on `expansion_sum(base, addend) ->
Vec<f64>` (merges two nonoverlapping expansions into one, same total
sum): `scale_expansion` (expansion × scalar), `product_of_expansions`
(expansion × expansion — needed once exactness must start at the
original coordinates rather than a once-rounded intermediate, see below).
`expansion_sum` itself is O(base.len()+addend.len()): merge the two
(already magnitude-sorted) inputs by magnitude, then a single cascading
`two_sum` pass over the merged sequence (Shewchuk's "linear-time
expansion sum") — merging by magnitude, not just cascading, is what keeps
the *nonoverlapping* postcondition needed for the sign trick below, not
just the sum. `scale_expansion`/`product_of_expansions` combine their
per-component pieces via `merge_all`, a **balanced binary-tree merge**,
not a left-to-right fold: folding into a single growing accumulator costs
O(count²) even with a linear `expansion_sum`, since each fold step's cost
is proportional to the accumulator's current (ever-growing) size —
halving the piece count at each tree level instead gives O(total_size ×
log(count)). This is not a micro-optimization: the naive fold made
`insphere`'s exact fallback take **16 seconds per call** on a degenerate
input before the fix (degree-5 nesting compounds the quadratic cost
across several levels) — see "Known limitation (fixed): naive expansion
merging is quadratic" below.

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
* `incircle`: implemented. See `INCIRCLE_ERR_BOUND_FACTOR` in
  `src/predicates/incircle.rs`. Lifts each point to `(dx, dy,
  dx^2+dy^2)`; the exact fallback builds the lifted `z` coordinate as an
  exact expansion of an exact expansion (`product_of_expansions(dx, dx)`
  summed with `product_of_expansions(dy, dy)`), not a squared rounded
  `f64` — see "Known limitation (fixed): exactness starts at the original
  coordinates" below for why that distinction matters. Verified against
  an independent oracle in `tests/differential/incircle.rs`.
* `insphere`: implemented. See `INSPHERE_ERR_BOUND_FACTOR` in
  `src/predicates/insphere.rs`. The 4x4 "lift to the 4D paraboloid"
  determinant, expanded along the first row into four 3x3 cofactors
  (`det3_with_precancel_bound` for the filter, `det3_exact` for the
  fallback) — the same nested-cofactor structure as `orient3d`/
  `incircle`, one level deeper. Both the filter-bound fix and the
  exactness-from-original-coordinates fix generalized to this predicate
  without needing new adjustments, confirmed by differential testing
  (including the same `mixed_intra_call_magnitude` and cancellation-stress
  generator classes used to originally find those two bugs). Verified
  against an independent oracle in `tests/differential/insphere.rs`.

If `|determinant| > error_bound`, the sign of `determinant` is provably the
true sign and is returned without any fallback. Otherwise, the exact
fallback (above) runs.

## Known limitation (fixed): filter bound must use pre-cancellation magnitudes

Found while implementing `incircle`, then confirmed as a latent flaw in
`orient3d` too (same cofactor structure, not yet triggered by that
predicate's differential tests when found, but structurally identical).

Both predicates compute 3 signed terms of the shape `term_X = adX * (P*Q
- R*S)`. The first version of both filters bounded the error using
`|term_a|+|term_b|+|term_c|` — the **post**-subtraction term magnitudes.
That is wrong whenever the inner subtraction `P*Q - R*S` itself suffers
catastrophic cancellation (e.g. `incircle` with two of the three defining
points both far from `d` in roughly the same direction: `P*Q` and `R*S`
are individually huge and nearly equal). The true absolute error
introduced by that cancellation is proportional to the **pre**-subtraction
magnitudes `|P*Q|+|R*S|`, not to the — possibly much smaller —
post-cancellation result. Using the smaller value silently underestimates
the bound, which can let a wrong sign through as "the filter is
conclusive."

Found via `tests/differential/incircle.rs`'s `mixed_intra_call_magnitude`
generator; root-caused by comparing every intermediate expansion of
`incircle_exact` against the bigint oracle step by step (all were exact —
the bug was entirely in the filter, not the fallback). Fixed by summing
each cofactor's pre-subtraction magnitudes, scaled by the outer row
factor:

```text
bound = FACTOR * (|adX| * (|P*Q| + |R*S|) + ... for each of the 3 rows)
```

`orient2d`'s bound was never wrong this way — its determinant has no
*inner* subtraction to worry about (`left = acx*bcy`, `right = acy*bcx`
are direct products, and `det = left - right` is the only subtraction,
already bounded by the correct `|left|+|right|`). The flaw only appears
in predicates whose cofactor expansion has an inner 2-term subtraction
before the outer row multiply — `orient3d` and `incircle` today,
`insphere` by the same structure. Pinned as regression fixtures:
`tests/regression/incircle.rs`, `tests/differential/orient3d.rs`'s
`cofactor_cancellation_stress`.

## Known limitation (fixed): naive expansion merging is quadratic

Found while implementing `insphere`: its exact fallback took **16
seconds per call** on degenerate (duplicate-point / coplanar) inputs —
completely impractical for a differential test suite running hundreds of
cases, let alone real use.

The first versions of `expansion_sum`, `scale_expansion`, and
`product_of_expansions` combined pieces by folding left-to-right into a
single growing accumulator (`result = combine(result, next_piece)` in a
loop). Even if each individual `combine` step were O(1), an
ever-growing-accumulator fold over `M` pieces costs O(M²) — and the
original `expansion_sum` was itself O(n×m) per call (repeated
single-element injection), compounding the problem further. `incircle`
(degree 4) and below didn't show this because their expansions stayed
short enough for the constants to not matter; `insphere`'s degree-5
nesting (multiple levels of `product_of_expansions` calling
`scale_expansion` calling `expansion_sum`) compounds expansion length
multiplicatively across levels, reaching thousands of components and
making the quadratic cost dominate.

Fixed in two parts, both now load-bearing:

1. `expansion_sum(base, addend)` is now O(base.len()+addend.len()):
   merge the two (already magnitude-sorted) expansions by magnitude,
   then a single cascading `two_sum` pass (Shewchuk's actual "linear-time
   expansion sum" — not a novel technique, just not what the first
   implementation used).
2. `scale_expansion`/`product_of_expansions` now combine their
   per-component pieces via `merge_all`, a **balanced binary-tree
   merge**, instead of a linear fold. This is necessary *in addition to*
   (1): a linear-time merge per step still costs O(M²) total if called M
   times against an ever-growing accumulator; halving the piece count at
   each tree level gives O(total_size × log(M)) instead.

Verified: the same degenerate `insphere` calls that took 16s each now
complete in under 30ms combined; the full differential/adversarial/
regression suite (100+ tests across all four predicates, many with
hundreds of random iterations) runs in a few seconds. `fast_two_sum` and
`grow_expansion` (superseded by `expansion_sum(e, &[b])` for
single-element injection) were removed as now-unused code rather than
kept "just in case" — see `tasks/lessons.md`.

## Known limitation: incircle/insphere have a narrower safe magnitude range

`incircle`'s determinant is **degree 4** in the coordinate differences
(the paraboloid lift squares two of the three matrix columns, then a
cofactor multiplies a degree-1 row factor by a degree-1-times-degree-2
sub-determinant) — not degree 2/3 like `orient2d`/`orient3d`. For
uniformly-scaled coordinates of magnitude `M`, intermediate products
reach `~M^4`. Both the filter (plain `f64`, can overflow to `Infinity`)
and the exact fallback (each expansion component is still a single `f64`
— exact arithmetic adds precision, not exponent range, see "exact-product
representability floor" above) are bounded by `f64`'s representable
exponent range, giving:

* **Ceiling** (overflow): `M^4 < f64::MAX (~1.8e308)` ⟹ `M < ~1.16e77`.
* **Floor** (the two-`f64`-product exactness floor, `~1.7e-292`,
  compounded through a degree-4 chain rather than a single `two_product`):
  empirically, safe well above `M ~ 1e-70`; not tightly derived, treated
  conservatively.

Kika does not claim exact-fallback correctness for `incircle` inputs
outside roughly `[1e-70, 1e70]` coordinate-difference magnitude (a
generous margin inside the derived ceiling/floor); it does still
guarantee the universal API contract (no panic, no NaN/Infinity from
finite input) — see `tests/adversarial/incircle.rs`'s
`near_subnormal_scale_does_not_panic` / `extreme_large_scale_does_not_panic`.

`insphere` is **degree 5** (one column squared, three linear, cofactor
expansion multiplies a degree-1 row factor by a degree-1×degree-2
sub-cofactor): `M^5 < f64::MAX` ⟹ `M < ~4.6e61` for the ceiling; the
floor compounds even further through one more nesting level than
`incircle`. Kika does not claim exact-fallback correctness for `insphere`
inputs outside roughly `[1e-30, 1e30]` (verified empirically via
`tests/adversarial/insphere.rs`'s non-panic checks and
`tests/differential/insphere.rs`'s test generators, not tightly derived —
narrower than `incircle`'s range as expected, with a large safety margin
below the theoretical `~4.6e61` ceiling since the floor side of a
degree-5 chain is harder to bound tightly by hand). As with the
two-`f64`-product floor, no real-world coordinate system operates
anywhere near either boundary.

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

## Phase 2: composed queries are exact predicates, but not all their output is

`Segment2::relation_to`, `Triangle2::relation_to`, and
`segment_intersection_kind` are exact — they only ever compare
already-computed predicate results and raw input coordinates, no new
arithmetic. `segment_intersection`'s construction side is **not**
uniformly exact: `EndpointTouch`/`CollinearTouch`/`CollinearOverlap`
reuse an original input coordinate directly (exact, by definition — the
shared point *is* one of the four inputs), but `Proper` computes a new
coordinate via ordinary `f64` parametric line-line interpolation, with no
exactness guarantee. This is intentional, not an oversight: ADR-004
explicitly defers a real exact/certified construction strategy to Phase
5, and Phase 2's own scope note says not to skip ahead of it. Until then,
a `Proper` intersection point may carry ordinary floating-point rounding
error, and — in astronomically extreme, near-parallel-line inputs — is
not even guaranteed finite (see `proper_intersection_point`'s doc
comment in `src/intersections/segment2.rs`).

## What is *not* claimed

This is a two-stage (filter + exact) model, not Shewchuk's three-stage
adaptive-precision scheme (see ADR-001). We do not claim "adaptive
precision" anywhere in code or docs. Fallback-rate measurements (§13) will
be reported once benchmarking exists, not assumed.
