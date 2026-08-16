# Degeneracy policy

Status: Phase 1 subset only. See ADR-002 for full reasoning.

## Predicate-level degeneracies (Phase 1, implemented)

| Case | Predicate behavior |
|---|---|
| Three collinear points | `orient2d` returns `Orientation::Collinear` |
| Four coplanar points | `orient3d` returns `Sign::Zero` |
| Four cocircular points (non-collinear) | `incircle` returns `Sign::Zero` |
| Five cospherical points (non-coplanar) | `insphere` returns `Sign::Zero` |
| Duplicate/repeated input point | Falls out of the determinant being exactly zero; no special-cased code path; covered by tests |
| Signed zero (`-0.0`) | Treated identically to `0.0`; covered by adversarial tests |
| Subnormal coordinates | Handled by the same filter+exact-fallback path as normal floats; covered by adversarial tests |

These require no tie-break rule: a predicate's sign is a mathematical fact
about its inputs, not a choice among multiple valid answers.

## Algorithm-level tie-breaking (not yet applicable)

Convex hull (Phase 3), Delaunay triangulation (Phase 4), and polygon
Boolean (Phase 6) all have cases with more than one topologically valid
output (e.g. Delaunay triangulation of cocircular points). Deterministic
tie-break rules for those cases will be added to this document when each
phase is implemented — not speculated here ahead of the algorithms.
