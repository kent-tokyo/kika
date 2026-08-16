# Degeneracy policy

Status: Phase 1 (predicates) + Phase 2 (2D primitive queries) subset. See
ADR-002 for full reasoning.

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

## Geometric-query degeneracies (Phase 2, implemented)

| Case | Behavior |
|---|---|
| Zero-length segment (`a == b`) | `Segment2::relation_to` returns `Endpoint` iff `p == a`, `NotOnSegment` otherwise — no "interior" to be on. |
| Degenerate (collinear-vertex) triangle | `Triangle2::relation_to` never returns `Inside` (zero area, nothing to be inside of). Returns `OnBoundary` iff `p` lies on the segment spanned by the three collinear vertices (checked via the union of the three point-pairs' segments, which always covers the full span regardless of vertex order), `Outside` otherwise. **Not** just "is `p` collinear with the vertices" — a point on the *same line* but far outside the vertices' span is `Outside`, not `OnBoundary`. This distinction was a real bug during development (the general 3-edge `orient2d` test alone can't tell the two cases apart, since all three checks are trivially `Collinear` for any point on that shared line): see `tests/regression/point_in_triangle.rs`. |
| Zero-length input segment(s), `segment_intersection_kind` | Not a separate classification variant. Both zero-length: `EndpointTouch` iff the two points are equal, else `None`. One zero-length: folds to `EndpointTouch`/`None` based on `Segment2::relation_to` against the other (real) segment — a degenerate segment's single point is trivially "its own endpoint". |
| Fewer than 3 polygon vertices | `Polygon2::orientation()` returns `Orientation::Collinear`; `Polygon2::signed_area()` returns `0.0`; `Polygon2::basic_validity()` returns `PolygonBasicValidity::TooFewVertices`. No panics — `polygon_orientation`/`signed_area` both explicitly short-circuit before any indexing that would need `n >= 1`. |
| All polygon vertices collinear (but `n >= 3`) | `Polygon2::orientation()` returns `Orientation::Collinear` (the exact shoelace-sum-of-expansions sign, not a float comparison — see `docs/numerical-model.md`); `basic_validity()` returns `PolygonBasicValidity::ZeroArea`. |
| Consecutive duplicate polygon vertices (including the wraparound edge) | `Polygon2::basic_validity()` returns `PolygonBasicValidity::ConsecutiveDuplicateVertices`, checked *before* the collinearity/zero-area check. Not treated as a self-intersection — that's a separate, more expensive check. |
| Adjacent polygon edges sharing an endpoint | `Polygon2::find_self_intersection()` explicitly excludes every adjacent edge pair (including the wraparound pair between the first and last edge) from the O(n²) check — the shared vertex is expected structure, not a self-intersection. Verified for both a convex polygon and a minimal triangle (where *every* edge pair is adjacent, so the check must return `None` for any triangle, degenerate or not). |

## Algorithm-level tie-breaking (not yet applicable)

Convex hull (Phase 3), Delaunay triangulation (Phase 4), and polygon
Boolean (Phase 6) all have cases with more than one topologically valid
output (e.g. Delaunay triangulation of cocircular points). Deterministic
tie-break rules for those cases will be added to this document when each
phase is implemented — not speculated here ahead of the algorithms.
