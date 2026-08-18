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
| `Polygon2::edge` on a single-vertex polygon (`len() == 1`) | `edge(0)` does not panic — `(0 + 1) % 1` wraps back to index `0`, returning the degenerate zero-length segment `Segment2::new(v, v)`. `edge(i)` panics only for `i >= len()` (including every `i` when `len() == 0`, since no index is ever in range) — ordinary slice-indexing semantics, not a distinct `len() < 2` condition. |
| All polygon vertices collinear (but `n >= 3`) | `Polygon2::orientation()` returns `Orientation::Collinear` (the exact shoelace-sum-of-expansions sign, not a float comparison — see `docs/numerical-model.md`); `basic_validity()` returns `PolygonBasicValidity::ZeroArea`. |
| Consecutive duplicate polygon vertices (including the wraparound edge) | `Polygon2::basic_validity()` returns `PolygonBasicValidity::ConsecutiveDuplicateVertices`, checked *before* the collinearity/zero-area check. Not treated as a self-intersection — that's a separate, more expensive check. |
| Adjacent polygon edges sharing an endpoint | `Polygon2::find_self_intersection()` explicitly excludes every adjacent edge pair (including the wraparound pair between the first and last edge) from the O(n²) check — the shared vertex is expected structure, not a self-intersection. Verified for both a convex polygon and a minimal triangle (where *every* edge pair is adjacent, so the check must return `None` for any triangle, degenerate or not). |
| `Polygon2::relation_to` on an empty, single-vertex, or otherwise degenerate/self-intersecting ring | Total, never panics — the crossing-number loop is well-defined for any vertex count (0 vertices: no edges, always `Outside`; 1 vertex: the single degenerate zero-length "edge" only ever returns `OnBoundary` for that exact point) and for a self-intersecting ring (mechanically well-defined, but "inside" then no longer corresponds to enclosed area — check `find_self_intersection` first if that matters, per the method's own doc comment). |

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

## Polygon-with-holes triangulation degeneracies (0.4.0, implemented)

`triangulate_polygon_with_holes` generalizes `triangulate_polygon` above
(same underlying algorithm — see its doc comment) rather than adding a
separate one; every rejected input is a typed `PolygonTriangulationError`,
never a panic. `outer` itself is checked exactly like `triangulate_polygon`'s
own input (the four cases in the table above apply unchanged); this table
covers only the hole-specific cases.

| Case | Behavior |
|---|---|
| A hole ring itself fails `Polygon2::basic_validity` (too few vertices, a degenerate edge, or zero area) | `PolygonTriangulationError::InvalidHole(hole_index, validity)` — checked before any relationship between the hole and `outer` or other holes. |
| A hole ring self-intersects | `PolygonTriangulationError::HoleSelfIntersecting(hole_index, found)`. |
| A hole's boundary touches or crosses `outer`'s boundary | `PolygonTriangulationError::HoleIntersectsOuter(hole_index, kind)`, `kind` from `SegmentIntersectionKind` (never `None`). Detected via an all-edge-pairs check between the hole and `outer` — the specific `kind` reported is the *first* pair found in edge-index order, same "not necessarily the geometric first" convention as `Polygon2::find_self_intersection`, not necessarily the most prominent intersection. |
| A hole lies entirely outside `outer` (no intersection with it, and not contained) | `PolygonTriangulationError::HoleOutsideOuter(hole_index)`. Disambiguated from proper containment via a single `Polygon2::relation_to` check on one hole vertex once the all-edge-pairs check confirms zero intersections — sound because a ring that never touches or crosses `outer`'s boundary can't have some vertices inside and others outside without an intersection in between. |
| Two holes' boundaries touch or cross | `PolygonTriangulationError::HolesIntersect(hole_a, hole_b, kind)`, `hole_a < hole_b`, same all-edge-pairs/first-found convention as the hole-vs-outer case. |
| One hole entirely nested inside another (an "island" case) | `PolygonTriangulationError::NestedHole(inner_hole, outer_hole)`. Out of scope for 0.4.0 — a clean typed error rather than partial/silent support. Checked both directions (each hole tested for containment in the other) since which one is "inside" isn't known upfront. |
| Any mix of CW/CCW winding, `outer` and each hole independently | Accepted — same orientation-agnostic flood fill as `triangulate_polygon`, generalized: a hole's boundary is just more constrained edges the fill stops at, exactly like `outer`'s own boundary or (for a non-convex `outer`) its concave-pocket edges. See `cw_outer_ccw_hole`/`ccw_outer_cw_hole`. |
| Checking the result's triangle count | `n + 2h - 2` (total vertices across `outer` and every hole, `h` holes) — the hole-generalized form of `triangulate_polygon`'s `polygon.len() - 2`, reducing to it at `h = 0`. Checked defensively before returning `Ok`, same postcondition discipline as `triangulate_polygon`. |

## Voronoi diagram topology degeneracies (0.5.0, implemented)

`voronoi2`'s scope is deliberately topology-only: no coordinates
(circumcenters), so there is no rounding/construction-exactness question
to answer here at all — every case below is either "how many
cells/vertices/edges exist and how do they connect", not "where".

| Case | Behavior |
|---|---|
| Fewer than 3 distinct points, or all points collinear (`delaunay2`'s own degenerate-input case) | `voronoi2` returns a fully empty `Voronoi2` — 0 cells, 0 vertices, 0 edges, never a panic. Verified directly (not just derived from `delaunay2`'s own empty-`Triangulation2` policy): `cells()`/`vertices()`/`edges()` all yield 0 for 1 point, 2 points, and 3+ exactly collinear points. |
| A collinear stretch on the convex hull (e.g. 3+ points on one straight hull edge, with other points making the overall set non-degenerate) | No special case needed or present — `delaunay2`'s own `HullBoundaryPoints::ExtremesOnly` seeding only affects which points bootstrap the initial hull fan, not which points end up as real triangulation vertices; every input point becomes a real Delaunay vertex (and so a real Voronoi cell) regardless. Verified directly: a 3-point collinear stretch plus one off-line point produces 4 cells, all correctly connected, `cell_edges()` well-defined for the "flat" middle point, `validate_voronoi_topology()` clean. |
| 4 or more points exactly cocircular | The central case this module exists for (§"Cocircular tie-break normalization", ADR-007): merged into a single Voronoi vertex via union-find keyed on `incircle(...) == Sign::Zero`, regardless of which diagonal `delaunay2`'s own tie-break happened to choose — verified by feeding the *same* cocircular point set through multiple different hand-built triangulations (`assemble_triangulation`, since `delaunay2` itself can never be coaxed into picking a different diagonal for a fixed point set) and checking for identical, not merely isomorphic, canonical output. |
| A cocircular cluster adjacent to a non-cocircular point | Partial exclusion, not all-or-nothing: only the interior Delaunay edge(s) *within* the cocircular cluster are excluded as spurious; an edge between a cluster face and an unrelated face survives as a genuine `Bounded` Voronoi edge. Verified with a fixture combining both in one triangulation (a cocircular square plus one far outlier). |
| Near-cocircular but not exactly (e.g. one point nudged a small, exactly-representable distance off the true circle) | Not merged — `incircle`'s exact (expansion-arithmetic) evaluation is decisive, not a tolerance/epsilon comparison, so a close miss produces a genuine `Bounded` edge rather than being swept into the same group. |
| `cell_edges()` on a cell with only 1 incident Delaunay face (e.g. any cell in a single-triangle triangulation) | Exactly 2 `Unbounded` edges (both of the site's hull-boundary edges), not 1 or 3 — the walk's entry and exit rays coincide with the same single face's two boundary edges. |
| `cell_edges()`'s ordering under cocircular merging | Never splits into two disconnected runs of the same `VoronoiVertexId` — proven, not just tested: if two faces incident to one site land in the same cocircular group, every face between them in the local rotation must too (their shared vertex plus both circles' defining triples force one common circle, always caught by `voronoi2`'s exhaustive adjacent-pair testing, never just a spanning-structure subset of it). |

## Algorithm-level tie-breaking (not yet applicable)

Polygon Boolean (Phase 6) has cases with more than one topologically valid
output. Deterministic tie-break rules for those cases will be added to this
document when that phase is implemented — not speculated here ahead of the
algorithm.
