# ADR-004: Exact construction strategy

Status: Accepted for Phase 1 (predicates only); re-opened before Phase 5

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

## Revisit when

Before Phase 5 begins (exact constructions for segment intersection etc.),
this ADR must be re-opened and a real decision recorded, informed by
whichever intersection cases Phase 2 actually produces.
