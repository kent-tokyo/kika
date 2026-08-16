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

## Convex hull degeneracies (Phase 3, implemented)

| Case | Behavior |
|---|---|
| Empty input | `convex_hull2` returns a `Polygon2` with 0 vertices. |
| Single distinct point | Returns that one point, regardless of `boundary`. |
| Two distinct points | Returns both, regardless of `boundary`. |
| Duplicate points (exact coordinate equality, incl. `-0.0`/`0.0`) | Collapsed before hulling via a sort+dedup pass; do not affect the result. The sort comparator normalizes `-0.0` to `0.0` so it agrees with `Point2`'s `PartialEq` — a plain `total_cmp`-based sort without that normalization can place `PartialEq`-equal points non-adjacently (found during design, before writing the hull code; see `tasks/lessons.md`), which would make a consecutive-element `dedup()` miss the duplicate. |
| All input points exactly collinear (≥3 distinct points) | The general lower/upper monotone-chain construction is **not** run — applied naively to a fully collinear set, both chains independently retain every point (nothing ever triggers a pop in either direction), producing a self-retracing result like `[A,B,C,D,C,B]` for 4 collinear points. Detected via an explicit `orient2d` check against the two lexicographic extremes before running the chains (found by hand-tracing the algorithm during design). `ExtremesOnly` returns just the two extremes; `KeepAllOnBoundary` returns every distinct point once, in sorted order, with **no** duplicated closing point. |
| `KeepAllOnBoundary` result for a fully collinear input | The returned `Polygon2`'s implicit closing edge (last vertex back to first) retraces the same line as every other edge; `Polygon2::find_self_intersection()` will report overlaps on it. This is a documented consequence of representing a zero-width hull as a vertex ring, not a bug — see the doc comment on `convex_hull2`. |
| Collinear boundary point on an otherwise non-degenerate hull (e.g. a point on a square's edge, between two corners) | `ExtremesOnly` drops it (only strict corners survive); `KeepAllOnBoundary` keeps it. |
| Strictly interior point | Dropped under both `boundary` modes. |
| Points forming a "valley" (all necessary corners on one monotone chain, e.g. samples of `y = x²`) | **Not** treated as collinear — ruled out explicitly during design as a false-positive risk for a length-based collinearity heuristic (`lower chain used every point` does *not* imply collinear; verified with this exact counterexample before choosing the `orient2d`-based precheck instead). See `tests/differential/convex_hull2.rs`'s `valley_shape_is_not_treated_as_collinear`. |

Output is always counterclockwise-wound and starts at the lexicographically
smallest input point (by `(x, y)`), independent of input order — verified by
permutation-invariance property tests
(`tests/differential/convex_hull2.rs`).

## Delaunay triangulation degeneracies (Phase 4, implemented)

| Case | Behavior |
|---|---|
| Fewer than 3 distinct points, or all points collinear | `delaunay2` returns an empty `Triangulation2` (0 triangles) — no valid 2D triangulation exists (matches `convex_hull2`'s `hull.len() < 3` check). |
| Duplicate points (exact coordinate equality) | Collapsed before triangulating, via the same `hull::dedup_sorted` pass `convex_hull2` uses; do not affect the result. |
| Point exactly on an *interior* edge shared by two triangles | Both adjacent triangles are invalidated (a point on a chord is geometrically inside both circles that have that chord — a chord-of-a-circle argument, not a special case in the code) and correctly split into 4 total, covering the same area with no gap or overlap. See `tests/differential/delaunay2.rs`'s `point_on_interior_edge_splits_both_adjacent_triangles`. |
| Point exactly on a *hull-boundary* edge | Splits the one triangle touching that edge into 2. See `point_on_hull_boundary_edge_splits_one_triangle`. |
| 4 or more points exactly cocircular | More than one triangulation satisfies the empty-circumcircle (Delaunay) property simultaneously — there is no single "the" Delaunay triangulation for a cocircular point set. **Tie-break rule**: a point exactly on a triangle's circumcircle boundary (`Sign::Zero` from `incircle`) does not make that triangle "bad" — it is not removed/replaced. Combined with `delaunay2`'s canonical sort-before-insertion, this makes the result a deterministic function of the input *set* (not insertion order) — but it is **not** the mathematically-canonical or unique triangulation, and does not necessarily match another Delaunay implementation's choice on the same cocircular input (e.g. which diagonal a cocircular quad's two triangles use). See `tests/differential/delaunay2.rs`'s `random_points_on_a_circle` and `tests/regression/delaunay2.rs`. |
| Near-collinear point cluster plus a far-off point | Handled exactly, at any scale — see `docs/numerical-model.md`'s Phase 4 section for why this was a real, found bug (not just a theoretical concern) in an earlier super-triangle-based design, and how the symbolic single-ghost-vertex fix removes the scale dependency entirely. |

## Algorithm-level tie-breaking (not yet applicable)

Polygon Boolean (Phase 6) has cases with more than one topologically valid
output. Deterministic tie-break rules for those cases will be added to this
document when that phase is implemented — not speculated here ahead of the
algorithm.
