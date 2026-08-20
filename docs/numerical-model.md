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

## Known limitation (fixed): split() overflow and two_sum overflow for sign-only predicates

0.7.0 shipped with a real bug at the opposite (large-magnitude) end of
this arithmetic core from the floor above: `delaunay2()` could panic
(`index out of bounds`) on 3 points with extreme, widely mixed coordinate
magnitude, because `orient2d` returned *permutation-inconsistent* answers
— breaking the antisymmetry `delaunay2()`'s "first 3 non-collinear
points" search relies on. Fixed in 0.7.1; the diagnosis (found by
`fuzz/fuzz_targets/voronoi_geometry.rs`) and fix are recorded here since
they're a correction to this document's own prior account of this
arithmetic core's safe range.

Two independent overflow sites, both silently producing a `NaN` that
`expansion_sign` read as `Sign::Zero` (`Orientation::Collinear`) rather
than surfacing:

1. **`split()`'s `SPLITTER * a`** (`SPLITTER ~= 2^27`) overflows to
   `±Infinity` for `|a| > f64::MAX/SPLITTER ~= 1.34e300`, then
   `hi = c - a_big` becomes `Infinity - Infinity = NaN`. This is the
   original repro's mechanism: `p1 = (3.2186699543901864e-57,
   -4.251746146807175e304)`'s y-coordinate exceeds the threshold. Fixed
   by making `split` recursively split a `2^-100`-rescaled copy of `a`
   above the threshold, then scale the result back — exact, since
   power-of-two multiplication never loses precision short of
   overflow/underflow, and `2^-100` alone brings any finite `a` (up to
   `f64::MAX`) safely under the threshold in one step.
2. **`two_sum`'s `a + b`** (inside `diff_expansion`) overflows when two
   coordinates are opposite-sign and each within a small factor of
   `f64::MAX`, so their true difference itself exceeds `f64::MAX` — a
   genuine representability limit (the exact result has no finite `f64`
   value at all), not an algorithmic artifact like (1). Found
   independently while diagnosing the above:
   `a=(1e308,0), b=(-1e308,1e-10), c=(0,1e-10)` is also
   permutation-inconsistent. Fixed via a new `rescale_for_sign_only`
   helper, used only by `orient2d_exact`/`orient3d_exact`/
   `incircle_exact`/`insphere_exact` — **not** pushed into
   `diff_expansion` itself, since `circumcenter`/`line_intersection` also
   build on it and need the real magnitude back, not just a sign (see
   "Phase 6" below for their own, different rescale-and-restore
   approach). Above a fixed `f64::MAX/4` threshold,
   `rescale_for_sign_only` rescales *every* coordinate in one predicate
   call by a fixed `0.25` factor, never restored: a determinant is
   homogeneous of positive degree in its coordinates, so any positive
   uniform rescale preserves its sign. Rescaling only the one
   overflowing difference — not every coordinate — was tried first and
   rejected: `orient2d`'s determinant multiplies differences from
   *different* coordinate pairs (`acx*bcy - acy*bcx`), so an
   inconsistently-scaled diff corrupts the surrounding product's sign,
   reintroducing the same bug class.

Both repros are pinned in `tests/regression/orient2d.rs`'s
`permutation_consistent_at_extreme_mixed_magnitude`, checked across all 6
permutations (not just one swap). `expansion_sign` also gained a
debug-only NaN guard, but scoped to a new `sign_only_expansion_sign`
wrapper used only by the 4 sign-only predicates — a shared assert inside
`expansion_sign` itself broke `circumcenter`/`line_intersection`'s own,
different (and already correct) way of detecting an unrepresentable
*result* via a final `.is_finite()` check.

### Residual: `split()`'s narrow rounding-carry ceiling near `±f64::MAX`

For `|a|` within roughly `2^-26` of `f64::MAX` itself (a band `~4.3e300`
wide at the very top of `f64`'s range), the correctly-rounded result
would need a nonexistent exponent 1024, so `split` still returns
`hi = Infinity` there regardless of rescale factor — a rounding-carry
limit intrinsic to representing the result at all, not specific to the
`2^-100` rescale choice above. Verified (not assumed): `split` itself
never produces `NaN` in this band (`hi = Infinity`, `lo` stays finite —
`split_near_f64_max_does_not_panic`), but `two_product` built on top of
it still can, via `Infinity * 0.0` (`split`'s own `lo` half is exactly
`0.0` for plenty of ordinary values, e.g. `split(1.0) == (1.0, 0.0)`) —
never a panic either way. Structurally distinct from the ceiling below (a
subtraction/rounding-carry limit, not a multiplication-overflow one), and
much narrower; tracked in `tasks/todo.md`, not chased further here.

## Known limitation: exact-product representability ceiling

The symmetric large-magnitude counterpart to the floor above: `two_product`'s
own `hi = a * b` overflows when **both** operands are independently
larger than roughly `sqrt(f64::MAX) ~= 1.34e154`. Unlike the two overflow
sites just fixed, this is a genuine representability ceiling, not an
algorithmic artifact or a genuinely-unrepresentable-difference case —
the true product itself has no finite `f64` value, and no amount of
rescaling fixes it (rescaling shifts the whole magnitude range; it
doesn't compress the *span* between a huge and a tiny coordinate in the
same predicate call, which the two 0.7.1 fixes above both rely on).

**Practical impact:** confirmed via the real public API,
`p0=(1e300,1e300), p1=(-1e300,1e300), p2=(0,0)` — a valid, large,
non-degenerate right triangle — returns `Orientation::Collinear`
self-consistently across all 6 permutations of `orient2d`. Wrong, but not
permutation-*inconsistent* (unlike the bugs fixed above), so it doesn't
panic `delaunay2()`, and `sign_only_expansion_sign`'s debug-only NaN
guard will now assert on it in debug/fuzz builds — expected, not a
regression (see `tasks/todo.md`). This `~1.34e154` figure is `orient2d`'s
own (degree-2, `M^2 < f64::MAX`); `incircle`/`insphere` reach the *same*
underlying ceiling far sooner, at their own, already-documented, lower
degree-4/degree-5 thresholds (`~1.16e77`/`~4.6e61` — see "Known
limitation: incircle/insphere have a narrower safe magnitude range"
above), confirmed empirically while writing this round's
`tests/adversarial/incircle.rs`/`insphere.rs` mixed-magnitude spot checks
(`1e308`, safe for `orient2d`/`orient3d`, hits this ceiling immediately
for `incircle`/`insphere` via their own internal squaring).

**Upgrade path**, if a future use case ever needs it: a different
arithmetic architecture (e.g. variable-precision or Shewchuk-adaptive
expansions, tracking a shared exponent rather than representing every
component as a full-range `f64`) — a larger undertaking than a patch
release, tracked in `tasks/todo.md`, not implemented here.

## Phase 2: composed queries are exact predicates, but not all their output is

`Segment2::relation_to`, `Triangle2::relation_to`, and
`segment_intersection_kind` are exact — they only ever compare
already-computed predicate results and raw input coordinates, no new
arithmetic. `segment_intersection`'s construction side was **not**
uniformly exact at the time Phase 2 shipped: `EndpointTouch`/
`CollinearTouch`/`CollinearOverlap` reuse an original input coordinate
directly (exact, by definition — the shared point *is* one of the four
inputs), but `Proper` computed a new coordinate via ordinary `f64`
parametric line-line interpolation, with no exactness guarantee. That gap
was intentional, not an oversight — ADR-004 explicitly deferred a real
exact/certified construction strategy to Phase 5 — and is now closed: see
"Phase 5: `Proper` intersection is now a correctly-rounded construction"
below.

`Polygon2::orientation()` is the exception among Phase 2's query methods
in the other direction: it's not just composing existing exact
predicates, it independently reuses the exact-arithmetic core directly
(`predicates::expansion`'s `product_expansion`/`expansion_sum`/
`merge_all`/`expansion_sign`) to sum every edge's shoelace term
(`x_i*y_j - x_j*y_i`) into one exact expansion before taking its sign —
a running `f64` sum here could round through cancellation for a
near-degenerate polygon (many vertices, small net area) the same way a
naive predicate could. Unlike `orient2d`/`orient3d`/`incircle`/
`insphere`, it has no fast floating-point filter ahead of the exact path
— a deliberate, documented simplification (`predicates::polygon2`'s doc
comment), not the O(count²) naive-merge mistake from the "naive expansion
merging" section above: the summation itself is already the O(n log n)
balanced `merge_all`, just always exact rather than filter-then-exact.
`Polygon2::signed_area()` is the plain, non-exact numeric counterpart
(the actual `f64` area value, not just its sign) — same
predicate/construction split as everywhere else in Phase 2.

## Phase 3: convex hull is fully exact, unlike most Phase 2 constructions

`convex_hull2` is the first algorithm in the crate whose entire output is
exact, not just its component predicate calls' signs. It uses `orient2d`
for every turn test (both the monotone-chain scan and the collinearity
precheck) and a `total_cmp`-based sort for ordering, but constructs no new
coordinate anywhere — every vertex in the returned `Polygon2` is a value
copied directly from the input slice. There is no analog of
`segment_intersection`'s non-exact `Proper` case here, and no ADR-004
deferral applies: "exact construction" for a hull is trivial because a hull
never needs to compute an intersection or interpolation, only select a
subset of its input.

The one place this phase's design needed care was **which** subset to
select for a fully collinear input, not how to compute a new coordinate:
see `docs/degeneracy-policy.md`'s convex-hull table and `tasks/lessons.md`
for the self-retracing-chain bug found (and the false-positive detection
heuristic ruled out) while designing it, before any code existed.

## Phase 4: Delaunay triangulation avoids synthetic coordinates entirely

`delaunay2` (Bowyer-Watson incremental insertion) has the same exactness
property as `convex_hull2`: every vertex in the returned `Triangulation2`
is copied directly from the input, never computed. The first
implementation attempt did *not* have this property cleanly — it seeded
the algorithm with a synthetic "super-triangle" (three coordinates derived
from the input's bounding box, scaled by a fixed multiplier, stripped from
the output at the end). That version passed every hand-written unit test,
including several specifically targeting degenerate cases, but a property
test on *ordinary* random point clouds (not a constructed adversarial
case) found a real bug: for a 4-point input (3 forming a hull triangle, 1
strictly interior), it produced 2 triangles instead of the
topologically-required 3, silently dropping a triangle.

Root cause: whether a super-triangle vertex "shields" a real internal edge
from getting its second real triangle is governed by the *sign* of an
`incircle` test involving that synthetic vertex, and that sign is not
scale-stable. For the minimized 4-point case, the relevant `incircle`
value was negative at a 20x-bounding-box-diagonal scale and flipped
positive only around 100x — with no universally-safe multiplier, because
the governing ratio is bounding-box diagonal to *smallest relevant point
spacing*, which is unbounded (a tight cluster of points can sit anywhere
inside an arbitrarily large bounding box). This is a sharper version of
the "near-collinear cluster has unbounded circumradius" problem — it does
not require an adversarially-constructed input, only an unlucky one.

Fixed by removing the synthetic coordinate entirely: `delaunay2` bootstraps
from the first 3 non-collinear *real* points (found by scanning the
canonically-sorted input) and represents "outside the current triangulation"
with a single symbolic ghost vertex (no coordinate at all) that always has
a closed triangle fan around it, exactly like a real interior point would.
A triangle with the ghost as one of its three vertices reduces its
circumcircle test to an exact `orient2d` half-plane test against its one
real edge (the limit of a circle whose third point recedes to infinity) —
see `is_bad` in `src/triangulation/delaunay2.rs`. No arithmetic anywhere in
the algorithm ever touches a coordinate that isn't a real input point, so
there is no scale-dependent tradeoff left to document: the fix is exact,
not merely "less likely to fail" (verified down to a perpendicular spread
of `1e-200` relative to a span of `10.0` in
`tests/differential/delaunay2.rs`'s `near_collinear_cluster_with_a_far_outlier`,
and against the original minimized failing case in
`tests/regression/delaunay2.rs`).

Degenerate cases (collinear boundary points, cocircular points) are
handled explicitly — see `docs/degeneracy-policy.md`'s Delaunay
triangulation table.

## Phase 5: `Proper` intersection is now a correctly-rounded construction

ADR-004 is decided (see the ADR): `Point2` stays a plain `f64` pair, and
`segment_intersection`'s `Proper` case now returns the **correctly rounded**
(round-to-nearest-even on exact ties) `f64` nearest to the true,
infinite-precision intersection coordinate — the same guarantee IEEE-754
gives a single arithmetic operation, extended to a whole geometric
construction. Implemented in `src/predicates/constructions/line_intersection.rs`.

**Construction.** Parametrizing line `AB` as `P(t) = A + t(B-A)` and letting
`d1 = orient2d(C,D,A)`, `d2 = orient2d(C,D,B)` (the signed twice-area from
each endpoint to line `CD`), the crossing point is `P = [d1*B - d2*A] /
(d1-d2)` (see the function's doc comment for the full derivation). `d1`,
`d2`, and hence the numerator/denominator, are built as *exact* expansions
reusing `orient2d`'s own exact-fallback determinant machinery — the same
`diff_expansion`/`product_of_expansions`/`expansion_sum` primitives as every
other exact fallback in this file, no new arithmetic primitive needed. The
division by `(d1-d2)` is the one step that cannot stay exact (the true
quotient is generally irrational relative to `f64`); `correctly_rounded_divide`
handles it: an ordinary `f64` division seeds an initial guess `q`, then the
*exact* residual `r = num - q*denom` (via the same expansion machinery)
determines whether `q` already rounds correctly, must step to its
neighbor, or lands on an exact tie (resolved by round-to-even on `q`'s
mantissa LSB) — comparing `|r|` against `|denom| * half_ulp`, computed
per-direction since ULP is asymmetric at power-of-two boundaries.

**Loop bound, measured not assumed.** The refinement loop is capped at 8
iterations as a safety net. `divide_loop_iteration_bound_is_generous`
(`src/predicates/constructions/line_intersection.rs`) measures the actual
worst case — ordinary random crossings plus deliberately near-parallel ones
(where `d1-d2` is a small difference of close values, the case most likely
to defeat the plain-`f64` initial guess via catastrophic cancellation),
across magnitude scales from `1e-300` to `1e100` — at 2 iterations, 4x below
the bound. This was checked, not assumed: an unverified iteration bound on a
function whose entire purpose is a correctness guarantee would be the same
class of risk as the super-triangle scale constant from Phase 4.

**Magnitude range, measured wider than expected.** This construction is
degree 3 in the input coordinates (`d1`/`d2` are degree-2 cross products,
scaled once more by a coordinate) — lower degree than `incircle`'s degree-4
determinant. It was not obvious ahead of time whether that would make its
safe range narrower or wider; the first draft of this document assumed
narrower (more multiplications feels riskier) and was wrong.
`tests/differential/line_intersection.rs`'s `magnitude_floor_sweep` measured
it directly against an independent `BigRational` "is this the
correctly-rounded nearest `f64`" oracle: no failure observed down through
`2^-335` (`~1.4e-101`, 50 random crossings sampled per exponent step) —
wider than `incircle`'s documented `~1e-70` floor above, not narrower.
Degree, not "construction vs. predicate", governs the floor. Below the
measured boundary, the same `product_expansion` representability floor
documented above applies (silently degrading the "exact" claim, not
solved, matching `incircle`/`insphere` precedent). See `tasks/lessons.md`
for the meta-lesson about predicting this the wrong way round.

**Ceiling, found and fixed, not just documented.** Unlike the floor above,
the large-magnitude side was a real, confirmed bug, not a documented
limitation. The degree-3 numerator (`d1*b.x()` etc.) overflows `f64::MAX`
for *uniform*-magnitude inputs around `~5.6e102`; for *mixed*-magnitude
inputs (segments `AB`/`CD` at different scales `K`/`M`) the relevant
quantity is `K²·M` (or `M²·K`), which can overflow even when both `K` and
`M` individually sit far below that uniform threshold — so no single-scalar
"safe up to magnitude X" claim could ever have been correct.
`extreme_uniform_magnitude_is_finite` and `extreme_mixed_magnitude_is_finite`
(`src/predicates/constructions/line_intersection.rs`) reproduced non-finite
(`NaN`) output from exactly this mechanism. Fixed by rescaling all four
input points by an exact power of two (lossless — an exponent shift, no
rounding) whenever any coordinate exceeds `RESCALE_THRESHOLD` (`1e90`),
computing on the rescaled points, then scaling the correctly-rounded result
back by the same power of two — the same technique the floor-side
limitation above already named as the known (then-unimplemented) upgrade
path, applied here in the other direction. `tests/differential/line_intersection.rs`'s
`magnitude_ceiling_sweep` confirms both finiteness *and* correctness
(against the same `BigRational` oracle) up through `2^500` (`~3.3e150`) —
comfortably past the old `~5.6e102` failure point, up to where
`segment_intersection`'s own classification stops reliably returning
`Proper` at all, not a remaining limitation of this construction itself.

**Known gap, unchanged:** the precondition (non-parallel lines) is
established by the caller (`segment_intersection`'s `Proper` classification
already guarantees this), not re-checked here.

## Phase 6 (ADR-009): circumcenter — same construction shape as Phase 5, a genuinely new failure mode

`predicates::constructions::circumcenter` (`Voronoi2::vertex_point`/
`edge_geometry`, 0.7.0) is the first new correctly-rounded construction
since Phase 5's `line_intersection`. Same shape, reusing the same shared
machinery (`correctly_rounded_divide`, extracted out of
`line_intersection.rs` into `predicates::constructions::rounding` once a
second construction needed it — see the ADR's own "reuse or duplicate?"
section): a degree-2 exact-expansion denominator (`d`, literally
`2 * orient2d(a,b,c)`'s own determinant), a degree-3 exact-expansion
numerator per output coordinate (vertex `a`'s coordinate folded in to
avoid double-rounding, mirroring `line_intersection`'s own "the `A*d1`
terms cancel" trick), one `correctly_rounded_divide` call per coordinate.

**Magnitude range, measured identical to Phase 5's, not just similarly
wide.** `tests/differential/voronoi_geometry.rs`'s `magnitude_floor_sweep`
found no failure down through `2^-335` (`~1.4e-101`) against an
independent `BigRational` oracle — the *exact* same boundary
`line_intersection`'s own `magnitude_floor_sweep` measures, consistent
with both constructions sharing the identical degree-3/degree-2 shape (see
"Magnitude range, measured wider than expected" above for why degree, not
"is it a predicate or a construction", governs this). The
`magnitude_ceiling_sweep` companion test confirms both finiteness and
correctness up through uniform coordinate magnitude `1e150`, past
`circumcenter`'s own `RESCALE_THRESHOLD` (`1e90`, same value and
reasoning as `line_intersection`'s).

**Divide-loop iteration bound, also measured identical to Phase 5's.**
`circumcenter`'s own `divide_loop_iteration_bound_is_generous` test
(`src/predicates/constructions/circumcenter.rs`) measures 2 iterations
worst case — including a cancellation family Phase 5's own sweep doesn't
cover (a circumcenter near the origin with vertices far from it, where
`a.x*d` and the offset terms are large and nearly cancel in the plain-`f64`
initial guess) — matching `line_intersection`'s own measured "2
iterations" exactly, not just falling within the shared `0..8` safety
bound by coincidence.

**A genuinely new failure mode, not present in Phase 5 at all.** Unlike
`line_intersection`, whose overflow ceiling is entirely a function of
*input* coordinate magnitude (fixed by rescaling), `circumcenter`'s true
output can diverge to infinity for perfectly ordinary, bounded input
coordinates: a triangle's circumradius is unbounded as it approaches
collinear, independent of how small the triangle's own coordinates are.
Confirmed concretely and reproducibly — not just argued — by
`thin_triangle_overflow_returns_none_not_a_panic`: `a=(0,0)`,
`b=(L,0)`, `c=(L/2,h)` with `L=1e75`, `h=1e-170` (both values
individually far above the `~1.7e-292` exact-product representability
floor below this document, and `orient2d(a,b,c)` independently confirmed
`CounterClockwise` — a genuine, non-degenerate triangle, not an
accidentally-collinear fixture) already overflows `f64::MAX`. Rescaling
does not help here: scaling all three points by `s` scales the true
(already-overflowing) circumcenter by `s` too, so scaling the result back
after the fact just re-overflows again — only the explicit
finiteness check after scale-back catches this, which is why
`circumcenter`/`vertex_point`/`edge_geometry` are fallible
(`Option`/`Result`) where `line_intersection` is not. A first attempt at
this fixture used a subnormal `eps` for `c`'s `y` coordinate
(`c=(0.5,eps)`, `eps` the smallest positive `f64`) — rejected once
`orient2d(a,b,c)` itself reported `Collinear` on it: that regime sits
*below* the representability floor for every predicate in this crate, not
a circumcenter-specific issue, so it would have tested the wrong thing
(exactness breakdown generally, not this construction's own genuine
unbounded-output failure mode).

## What is *not* claimed

This is a two-stage (filter + exact) model, not Shewchuk's three-stage
adaptive-precision scheme (see ADR-001). We do not claim "adaptive
precision" anywhere in code or docs. Fallback-rate measurements (§13) will
be reported once benchmarking exists, not assumed.
