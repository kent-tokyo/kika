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

## Constrained Delaunay degeneracies (Phase 6C, implemented)

`constrained_delaunay2`'s scope is deliberately narrow (see its doc
comment and `src/triangulation/cdt.rs`'s module doc comment); every
deviation from a plain PSLG-with-Steiner-points CDT is a typed
`CdtError`, never a panic or silent misbehavior.

| Case | Behavior |
|---|---|
| `points` has fewer than 3 elements, or is exactly collinear (`delaunay2`'s own degenerate-input case) | With an empty `constraints` list: `Ok`, wrapping the same empty `Triangulation2` `delaunay2` itself would return (0 triangles, 0 vertices) — matches `delaunay2`'s "degenerate is a valid, representable value" policy. With a non-empty `constraints` list: `CdtError::DegeneratePointSet` — no triangulation face exists for any constraint edge to become part of. See `tests/regression/cdt.rs`. |
| Two distinct constraint segments share exactly one endpoint | Allowed — not a crossing, checked exhaustively up front via the same `segment_intersection_kind` used everywhere else in the crate. See `shared_endpoint_constraints_are_allowed`. |
| Two distinct constraint segments properly cross (share a single interior point, endpoint of neither) | Rejected up front: `CdtError::ProperlyCrossingConstraints`. Automatic intersection-point generation is out of scope for this narrow version — see ADR-004's Phase 6 re-evaluation. |
| Two distinct constraint segments are collinear and overlap along a sub-segment | Rejected up front: `CdtError::CollinearOverlappingConstraints`. |
| A constraint segment passes exactly through a third, unrelated input vertex | No single triangulation edge can realize a segment through an intermediate vertex, and this scope does not auto-split it into two sub-constraints. The flip search exhausts its bound and returns `CdtError::ConstraintInsertionFailed`, not a wrong or partial result. |
| A constraint is already a Delaunay edge of the unconstrained triangulation | No-op: recognized immediately (`edge_exists` check before any flip), just marked constrained. See `constraint_already_a_delaunay_edge_is_a_noop_flip`. |
| A constraint's realized edge is not locally Delaunay | Kept anyway — constrained edges are never flip candidates during the unconstrained-Delaunay restoration pass (`restore_unconstrained_delaunay`'s `constrained_pairs` exclusion), and the topology validator (`validate_cdt_topology`) does not flag it. The plain `Triangulation2::validate_topology()` *would* still flag it if run directly on the constrained result — that's the expected, documented difference between the two validators, not a bug. See `constrained_edge_survives_even_when_not_locally_delaunay`. |
| Multiple constraints in one call, each needing its own edge flips | Each constraint's flip search defensively excludes every already-realized constraint from earlier in the same call as a flip candidate — see `crossing_faces`' doc comment for why this should be geometrically unreachable given the upfront non-crossing validation, and why the exclusion exists anyway as defense in depth. See `multiple_constraints_each_needing_a_flip_all_survive`. |
| Same point set and constraint set, different constraint insertion order | Same result — `constrained_delaunay2` builds from a canonically-sorted vertex order (inherited from `delaunay2`) and each constraint's flip search only depends on the current triangulation state, not accumulated insertion history. See `deterministic_regardless_of_constraint_order`. |

## Simple polygon triangulation degeneracies (Phase 6D, implemented)

`triangulate_polygon`'s scope is deliberately narrow (no holes, no Steiner
points — see its doc comment and `src/triangulation/polygon.rs`'s module
doc comment); every rejected input is a typed
`PolygonTriangulationError`, never a panic.

| Case | Behavior |
|---|---|
| Fewer than 3 vertices | `PolygonTriangulationError::TooFewVertices`, via `Polygon2::basic_validity`. |
| Two consecutive vertices exactly equal (zero-length edge) | `PolygonTriangulationError::DegenerateEdge`, via `Polygon2::basic_validity`. |
| Valid vertex count, no consecutive duplicates, but zero net signed area | `PolygonTriangulationError::ZeroArea`, via `Polygon2::basic_validity`. Note a symmetric self-crossing (bowtie) quadrilateral can *also* land here — its two lobes are congruent with opposite winding and cancel exactly — before self-intersection is even checked; this is still a correct rejection, just via a different variant than an asymmetric bowtie would hit. |
| Self-intersecting boundary (including a non-adjacent repeated vertex, caught as an `EndpointTouch`) | `PolygonTriangulationError::SelfIntersecting`, via `Polygon2::find_self_intersection`. Automatic splitting into simple sub-polygons is out of scope. |
| Clockwise input | Accepted — the interior/exterior flood fill (see below) is purely topological and orientation-agnostic; the one place winding matters (picking the correct starting face) explicitly branches on `Polygon2::orientation()`. See `clockwise_input_triangulates_the_same_region`. |
| Non-convex (reflex-vertex) polygon | The underlying CDT triangulates the full point set's convex hull; faces in the concave "pockets" between the polygon boundary and that hull are discarded by a topological flood fill seeded from one interior face (found via a single `orient2d` check against an existing triangle vertex — never a constructed point such as a centroid, which would reopen ADR-004's construction-exactness questions for no reason) and walking through every non-boundary edge. A simple polygon's interior is always a single connected region, so this reaches every interior face and no exterior one, even when there are several separate pockets. See `l_shape_discards_the_concave_pocket`, `plus_shape_discards_all_four_separate_pockets`, and `seed_edge_with_two_incident_faces_still_finds_the_interior_side` (the seed edge is a chord with 2 incident faces, not a hull edge with 1 — the case where the disambiguation is actually load-bearing). |
| Checking the result with `Triangulation2::validate_topology()` | Its Euler-characteristic check assumes the triangulation covers its own vertex set's full convex hull — true for `delaunay2`/`constrained_delaunay2`, generally **false** here for a non-convex polygon (the flood fill deliberately discards a proper subset of the hull). Expect `TopologyError::EulerFormulaViolated` on a non-convex result; every other check still holds. The applicable invariant instead: exactly `polygon.len() - 2` triangles, always, for any simple polygon triangulated with only its own vertices. |

## Algorithm-level tie-breaking (not yet applicable)

Polygon Boolean (Phase 6) has cases with more than one topologically valid
output. Deterministic tie-break rules for those cases will be added to this
document when that phase is implemented — not speculated here ahead of the
algorithm.
