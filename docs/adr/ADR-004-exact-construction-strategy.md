# ADR-004: Exact construction strategy

Status: Accepted for Phase 1 (predicates only); decided for Phase 5

## Context

AGENTS.md §4.2 distinguishes exact **predicates** (a correct sign) from
exact/certified **constructions** (a correctly-generated coordinate, e.g. a
segment intersection point). §9 Phase 5 requires this ADR to select a
construction model before polygon Boolean / constrained Delaunay (Phase 6)
is attempted, and explicitly forbids skipping Phase 5.

## Decision for now

Phase 1 needs no construction model — predicates return `Sign`/`Orientation`
enums, never coordinates. We are not selecting a construction strategy
(rational coordinates vs. homogeneous coordinates vs. lazy exact numbers vs.
filtered constructions vs. exact expression DAG vs. float+certificate) yet,
because doing so before Phase 2/3 intersection code exists would be a
speculative commitment (§9 Phase 0: "実装前に設計を過剰に固定する必要は
ありません").

The internal exact-arithmetic building block introduced in Phase 1 —
nonoverlapping floating-point expansions (ADR-001) — is a strong candidate
for the eventual construction model (it is exactly what "filtered
constructions" / "exact expression DAG with float leaves" are built from),
but this ADR does not commit to that yet.

## What Phase 1 does commit to

* The expansion-arithmetic primitives (`two_sum`, `two_product`,
  `grow_expansion`) are implemented as a small, reusable internal module
  (`src/predicates/expansion.rs`), not inlined per-predicate, specifically
  so they can be reused for constructions later without duplication.
* No production code path depends on arbitrary-precision big integers or
  rationals. `num-bigint`/`num-rational` are dev-dependencies used only by
  the differential-test oracle (ADR-005), never reachable from the public
  API.

## Decision for Phase 5

Phase 2 turned out to need exactly one construction: the `Proper`-crossing
point of two segments (`SegmentIntersection2::Point`, previously computed by
plain `f64` linear interpolation — an approximation, not a certified value).
That is the concrete case this ADR resolves against, per "Revisit when"
below.

**Chosen: keep `Point2` a plain `f64` pair; make the *value* correctly
rounded ("float+certificate", using the existing expansion machinery)
instead of introducing a new exact-coordinate type.**

Concretely: the intersection coordinate is derived as an exact expansion
(reusing `orient2d`'s own exact-fallback determinant as `d1`/`d2` — see
`src/predicates/constructions/line_intersection.rs`'s derivation doc
comment — which keeps the numerator/denominator at degree 3, not a fresh,
higher-degree line-line determinant), then rounded to the nearest `f64` via
`correctly_rounded_divide`: an ordinary `f64` division for an initial guess,
refined against the *exact* residual until the result is provably the
correctly-rounded (round-to-nearest-even) `f64` closest to the true value.
This is the same guarantee IEEE-754 gives a single `a / b`, extended to a
whole geometric construction.

Rejected alternatives, in the order the ADR's original candidate list
raised them:

* **Rational coordinates / homogeneous coordinates** — would make `Point2`
  exact-but-unbounded-size, a public-API/back-compat break for every
  downstream consumer, to solve a problem (one construction, one call site)
  that doesn't need it yet.
* **Lazy exact numbers** — same unbounded-representation cost, plus a new
  public numeric type; no construction in this crate yet needs deferred
  exactness across a *chain* of operations, only a single correctly-rounded
  result per call.
* **Exact expression DAG** — general machinery for composing many exact
  constructions; overkill for the one construction that exists today, and
  premature before Phase 6 reveals what else is needed (§9 Phase 0's
  "don't over-fix the design before it's needed" principle, the same
  reasoning that deferred this ADR past Phase 1 in the first place).
* **Filtered constructions without a correctly-rounded final step** — closer
  to what was chosen, but "close enough" filtering (a fast float path plus
  a fallback that's merely *more* accurate, not *provably* nearest) doesn't
  meet AGENTS.md §4.2's certified-construction bar. The implementation goes
  the extra step to a proven-correctly-rounded result instead.

Why this is the conservative, reversible choice: zero new public API
surface (`Point2`'s shape and the `SegmentIntersection2` API contract are
unchanged — only *strengthened*, from approximate to correctly rounded),
zero new dependencies (reuses Phase 1's expansion primitives exactly as
ADR-001 anticipated them being reused), and it does not foreclose adopting
one of the rejected alternatives later if Phase 6 (constrained Delaunay,
polygon Boolean) turns out to need exactness *across* a chain of
constructions rather than one coordinate at a time — see "Revisit when"
below.

Verified, not assumed: `tests/differential/line_intersection.rs` checks the
result against an independently reimplemented `BigRational` oracle
(comparing the candidate `f64` against both representable neighbors, per
the round-to-nearest-even definition — not just "close"), across magnitude
scales, mixed-magnitude inputs, and an empirical floor sweep. See
`docs/numerical-model.md`'s Phase 5 section for the measured magnitude
range and `tasks/lessons.md` for a lesson on a wrong a priori assumption
about that range's direction that measurement corrected.

## Revisit when

Phase 6 (constrained Delaunay, polygon Boolean) reveals what construction
needs it actually has. If those need exactness chained across multiple
constructions (not just one correctly-rounded coordinate per call, as
`line_intersection` needed), this ADR should be re-opened again rather than
assuming the float+certificate model automatically extends — the same
"decide when the real need is known, not speculatively" principle that
deferred this ADR past Phase 1 and past Phase 2 through Phase 4.
