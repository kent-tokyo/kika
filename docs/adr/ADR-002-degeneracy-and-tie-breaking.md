# ADR-002: Degeneracy and deterministic tie-breaking

Status: Accepted (v0.1 Phase 1 scope; extended in later phases)

## Context

AGENTS.md §7.3 requires explicit, documented handling of degenerate cases
and deterministic tie-breaking wherever multiple topologically valid outputs
exist (e.g. non-unique Delaunay triangulations on cocircular points).

## Decision (Phase 1 scope)

Phase 1 only covers the four predicates. The degeneracy cases relevant at
this stage are the ones a predicate's *sign* must classify correctly and
deterministically:

* `orient2d`: three points exactly collinear → `Orientation::Collinear`
  (`Sign::Zero`), regardless of input order (subject to antisymmetry:
  swapping two arguments flips CW/CCW but collinearity is order-invariant
  under any permutation).
* `orient3d`: four points exactly coplanar → `Sign::Zero`.
* `incircle`: four points exactly cocircular (and non-collinear) →
  `Sign::Zero`.
* `insphere`: five points exactly cospherical (and non-coplanar) →
  `Sign::Zero`.
* Duplicate points (two or more input points identical): predicates must
  still return a well-defined, non-panicking answer. `orient2d`/`orient3d`
  with a repeated point is always collinear/coplanar (the enclosed simplex
  is degenerate) — this falls out of the determinant being exactly zero and
  requires no special-case code.
* Signed zero (`-0.0` vs `0.0`) and subnormal coordinates: must not change
  the predicate's sign relative to the mathematically equal `0.0`/normal
  value. Covered by adversarial tests in Phase 1's acceptance criteria.

No coordinate-scale-dependent or input-order-dependent tie-break is needed
for predicate signs themselves — a determinant sign is a mathematical fact
about the input points, not a choice.

## Deferred to later phases

Tie-breaking *policy* (as opposed to correctness) becomes relevant once an
algorithm must pick one output among several valid ones, e.g.:

* Convex hull (Phase 3): collinear points on the boundary — keep or drop.
* Delaunay triangulation (Phase 4): cocircular points admit more than one
  valid triangulation; a deterministic tie-break (stable vertex ID order)
  is required and will be recorded in `docs/degeneracy-policy.md` when
  Phase 4 is implemented.
* Polygon Boolean (Phase 6): overlapping/degenerate edges.

`docs/degeneracy-policy.md` is created now as a stub and will be filled in
per-phase, not written speculatively ahead of the algorithms it governs.

## Consequences

* Predicate degeneracy handling requires no separate code path: it is a
  direct, tested consequence of computing the exact sign of the determinant.
* Algorithm-level tie-breaking rules are out of scope until the phase that
  needs them, avoiding speculative policy design (AGENTS.md §9 Phase 0:
  "設計を過剰に固定する必要はありません").
