# Degeneracy policy

Status: Phase 1 subset only. See ADR-002 for full reasoning.

## Predicate-level degeneracies (Phase 1, implemented)

| Case | Predicate behavior |
|---|---|
| Three collinear points | `orient2d` returns `Orientation::Collinear` |
| Four coplanar points | `orient3d` returns `Sign::Zero` |
| Four cocircular points (`a,b,c` non-collinear) | `incircle` returns `Sign::Zero` |
| `a,b,c` collinear, `d` also on that line | `incircle` returns `Sign::Zero` (the "circle" through 3 collinear points degenerates to the line, extended to infinity; a 4th point on that same line is on the degenerate circle) |
| `a,b,c` collinear, `d` off that line | `incircle` returns a **nonzero** sign indicating which side of the line `d` is on. Not a special case in the code — a direct, hand-verified consequence of the determinant formula, covered by `incircle::tests::collinear_abc_with_d_off_line_is_not_zero`. Do not assume "defining points collinear" implies zero. |
| Five cospherical points (`a,b,c,d` non-coplanar) | `insphere` returns `Sign::Zero` |
| `a,b,c,d` coplanar **and** concyclic within that plane, `e` also on the same circle | `insphere` returns `Sign::Zero` — a plane meets a sphere in a circle, so 4 coplanar points lie on *some* sphere only if they're additionally concyclic within their shared plane. This is the real analog of `incircle`'s collinear case, **not** mere coplanarity — see next row. |
| `a,b,c,d` coplanar but **not** concyclic within that plane | `insphere` returns a **nonzero** sign for generic `e`. A first version of this document (and of `insphere`'s own test suite) incorrectly assumed coplanarity alone was the degenerate case, by analogy with `incircle`'s collinear-implies-zero; a square's corners are coplanar *and* concyclic (making that specific test pass for the wrong reason), but a generic coplanar quadrilateral is not concyclic and gives a nonzero determinant — hand-verified, covered by `insphere::tests::coplanar_but_not_concyclic_abcd_is_not_zero`. `incircle`'s collinear-abc case has no such trap: collinearity of a,b,c *is* the full 2D degenerate condition (proven by a column-factoring argument — collinear points reduce two of the determinant's three columns to identical values, which doesn't happen for merely-coplanar, non-concyclic points in the 3D case). |
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
