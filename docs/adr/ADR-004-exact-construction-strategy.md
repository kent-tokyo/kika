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

## Phase 6 re-evaluation (pre-design, before implementation)

Re-opened per "Revisit when" below, ahead of Phase 6 implementation, to
check whether the Phase 5 decision above satisfies polygon Boolean's actual
requirements: vertex identity across independently-computed intersection
points, consistent ordering of multiple intersections along a shared edge,
safe re-evaluation of predicates against constructed points, and overall
topological consistency of the resulting arrangement. This section records
a finding, not a new decision — no representation is chosen here; see
"What remains open" below.

**Most load-bearing finding: Phase 6a (constrained Delaunay) needs no new
construction at all.** A constrained Delaunay triangulation exists for any
PSLG (planar straight-line graph) using only the *input* vertices — segment
recovery is done by edge-flipping between existing points, not by
constructing new intersection coordinates (the standard CDT theorem; no
Steiner points required for straight-line constraints). CDT therefore
operates purely on already-exact input vertices via `orient2d`/`incircle`,
exactly like unconstrained Delaunay (Phase 4) already does. **The Phase 5
decision is untouched for Phase 6a — nothing here reopens it.** Only Phase
6b (polygon Boolean's overlay step, which genuinely constructs new
segment-segment crossing points between the two input polygons) is in
scope for the rest of this section.

**Vertex identity holds for exactly-concurrent inputs.** If two different
segment pairs happen to cross at the exact same true point (e.g. three
concurrent lines), `correctly_rounded_divide` returns the `f64` nearest to
that one true value for *both* pairwise computations — round-to-nearest is
a deterministic function of the true real number, independent of which
arithmetic path produced it. Two computations of the same true value agree.
This is a real guarantee the current construction already provides, not a
gap.

**Two real gaps, both structural, neither solved by correct rounding alone:**

1. **Ordering along a shared edge is not guaranteed to survive rounding.**
   `line_intersection` correctly rounds its `x` and `y` output
   *independently*. The returned point is therefore generally not exactly
   *on* either input line — it is the nearest representable lattice point
   to a point that was. Two intersections that are close together along a
   shared edge can, after independent per-axis rounding, compare in the
   opposite order from their true parametric order along that edge. This
   is not an exotic edge case avoided in practice; it is a structural
   consequence of rounding `x` and `y` separately instead of rounding a
   single parametric position.
2. **Re-predicate evaluation against a rounded point is not guaranteed
   consistent with the rest of an exact arrangement** — the classical
   snap-rounding problem (Hobby 1999; Fortune; CGAL's own distinction
   between exact-predicates-inexact-constructions and
   exact-predicates-*exact*-constructions kernels). Correct rounding
   guarantees the output is the nearest `f64` to *that one* true value; it
   makes no guarantee that re-evaluating a predicate against that rounded
   point — say, against some third, unrelated edge passing very close to
   the true (unrounded) intersection — still agrees with what exact
   arithmetic on the true value would have said. Accuracy and consistency
   are different properties; Phase 5 solved the first, not the second.

**What would close these gaps, and what it would cost:** both gaps trace to
the same root cause — rounding to `f64` before all consistency-relevant
comparisons are done, rather than after. The fix is a lazily-exact
intermediate representation for overlay's constructed points, carried
through the arrangement-construction phase and only collapsed to `f64` (via
the existing `correctly_rounded_divide`) once each point's role is settled.
Two candidates, both from ADR-004's original list:

* **Expansion-backed homogeneous coordinates (leading candidate, zero new
  dependencies).** Represent a constructed point as `(num_x, num_y, denom)`
  — the same exact expansions `line_intersection` already builds, just left
  unevaluated instead of divided. Comparisons (ordering, equality,
  predicate evaluation against other expansion-backed or plain points) are
  done exactly via cross-multiplication using the existing expansion
  primitives (`product_of_expansions`, `expansion_sum`, `expansion_sign`) —
  no rounding anywhere until final output. This reuses 100% of Phase 1/5's
  existing machinery; ADR-005's zero-runtime-dependency posture is
  unaffected.
  * **This is not a claim that expansions make chained construction fully
    exact for free.** Expansions are closed under `+`, `-`, `×` but *not*
    `÷` — the same reason `line_intersection` needed
    `correctly_rounded_divide` in the first place. A chained construction
    (an intersection whose input is itself a previously-constructed,
    still-unevaluated point) combines two `(num, denom)` ratios by
    multiplying denominators, so component count grows with chain depth,
    and each product is still subject to `product_expansion`'s measured
    representability floor (`~1.7e-292`, `docs/numerical-model.md`). This
    is a real, measurable ceiling, not a solved problem — it needs
    measuring at Phase 6b implementation time, the same "measure it, don't
    assume it" discipline this crate applied to Phase 5's loop-iteration
    bound and magnitude floor.
* **Rational-backed construction (fallback, requires approval).** If
  expansion-backed homogeneous coordinates prove insufficient in practice
  (e.g. chain-depth growth becomes a real performance problem, to be
  checked by the benchmarking pass already planned), the documented
  fallback is `num-bigint`/`num-rational` as a genuine **runtime**
  dependency for overlay's internal bookkeeping — not the dev-only,
  test-oracle-isolated use ADR-005 already permits. This is a new runtime
  dependency and stays pending explicit user approval per AGENTS.md §19;
  recorded in `tasks/todo.md`'s "Deferred pending explicit user approval"
  list, not just in this prose.

**What remains open, deliberately:** this section does not choose between
expansion-backed homogeneous coordinates and rational-backed construction,
and does not design the overlay algorithm that would consume either. That
is Phase 6b's own decision, to be made when Phase 6b's actual algorithm
(and whether it turns out to need the DCEL-style structure ADR-006 also
left open for the same reason) is concretely known — repeating, one level
deeper, the exact discipline that produced this section in the first
place.

## Revisit when

Phase 6b (polygon Boolean's overlay step) implementation begins and reveals
its actual construction and consistency needs. At that point, choose
between expansion-backed homogeneous coordinates and (if approved)
rational-backed construction for overlay's intermediate points — informed
by what Phase 6b's algorithm actually does, not decided speculatively here.
Phase 6a (constrained Delaunay) needs no revisit: per the finding above, it
uses no new construction and the Phase 5 decision already covers it.
