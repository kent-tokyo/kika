use std::collections::HashSet;

use crate::hull::{HullBoundaryPoints, convex_hull2, dedup_sorted};
use crate::predicates::{Orientation, Sign, incircle, orient2d};
use crate::primitives::{Point2, Triangle2};

/// A 2D Delaunay triangulation: a flat list of non-overlapping,
/// counterclockwise-wound [`Triangle2`]s whose union is the input's convex
/// hull, with no input point strictly inside any triangle's circumcircle.
///
/// No adjacency/topology is exposed — just the triangle list. A structured
/// (half-edge or similar) representation is deferred until a consumer
/// actually needs neighbor queries (§6: split into a real structure only
/// when required, not preemptively).
#[derive(Debug, Clone, PartialEq)]
pub struct Triangulation2 {
    triangles: Vec<Triangle2>,
}

impl Triangulation2 {
    pub fn triangles(&self) -> &[Triangle2] {
        &self.triangles
    }

    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }
}

/// Sentinel triangle-vertex index representing the single symbolic "point
/// at infinity" — never a real index into `pts` (`pts.len()` is always far
/// below `usize::MAX`).
const GHOST: usize = usize::MAX;

fn is_ghost(idx: usize) -> bool {
    idx == GHOST
}

/// The 2D Delaunay triangulation of `points`, via Bowyer-Watson incremental
/// insertion with a symbolic "point at infinity" standing in for a
/// synthetic bounding triangle.
///
/// Duplicate points (exact coordinate equality) are collapsed first, same
/// policy as [`crate::convex_hull2`]. Points are inserted in the same
/// canonical sorted order `convex_hull2` uses, not input order — see
/// "Determinism and cocircular points" below. Degenerate inputs (fewer than
/// 3 distinct points, or all points collinear) return an empty
/// triangulation rather than an error, matching this crate's usual
/// "degenerate is a valid, representable value" policy.
///
/// # Algorithm
///
/// The first 3 non-collinear points (in canonical sorted order) form the
/// initial real triangle; its 3 outer edges are each paired with a single
/// symbolic ghost vertex (`GHOST`, no coordinate) representing "point at
/// infinity", so the ghost has a closed triangle fan around it exactly like
/// any real interior point would. Each remaining point is then inserted in
/// turn via the standard cavity construction: every existing triangle whose
/// circumcircle strictly contains the new point is removed, opening a
/// star-shaped cavity that gets re-triangulated by connecting the new point
/// to every edge on the cavity's boundary. "Circumcircle contains the new
/// point" is evaluated by plain [`incircle`] when a triangle has no ghost
/// vertex, or reduces to a half-plane [`orient2d`] test against the
/// triangle's one real edge when it has exactly one (the limit of a circle
/// through a point receding to infinity) — see `is_bad`. A triangle can
/// never have more than one ghost vertex (proven by induction: the starting
/// triangles have at most one, and every new triangle is formed from an
/// existing triangle's edge plus a real point, which can add a ghost only
/// if the edge itself already carried one).
///
/// Once every point has been inserted, every triangle still carrying the
/// ghost vertex is dropped — **every vertex in the returned triangulation
/// is a value copied from the original input**, never a synthetic
/// coordinate, and (unlike a bounding-triangle approach) no arithmetic ever
/// touches a synthetic coordinate either, so there is no super-triangle
/// sizing tradeoff to document: the outer region is handled exactly, at any
/// input scale or aspect ratio (see `tests/differential/delaunay2.rs`'s
/// `near_collinear_cluster_with_a_far_outlier`).
///
/// # Determinism and cocircular points
///
/// Output is deterministic: points are canonically sorted before
/// insertion, so the result is a function of the input *set*, not its
/// order. This does **not** mean the triangulation is the unique
/// mathematically-canonical one for every input — when 4 or more points
/// are exactly cocircular, more than one triangulation satisfies the
/// empty-circumcircle property, and *which* one comes out depends on
/// insertion order (a tie-break rule, not a derived fact). This crate's
/// tie-break: a point exactly on a triangle's circumcircle boundary
/// (`Sign::Zero`) does not make that triangle "bad" — it is not removed.
/// Combined with the canonical sort, this makes the tie-break itself
/// deterministic, but a caller comparing this triangulation's diagonal
/// choice on a cocircular quad against another Delaunay implementation
/// should not expect them to agree.
pub fn delaunay2(points: &[Point2]) -> Triangulation2 {
    let pts = dedup_sorted(points);
    let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
    if hull.len() < 3 {
        return Triangulation2 {
            triangles: Vec::new(),
        };
    }

    // First 3 non-collinear points in sorted order. `pts[0]`/`pts[1]` fixed
    // and scanning forward always finds one: if every `pts[i]` were
    // collinear with `pts[0]`,`pts[1]`, the whole set would be collinear,
    // contradicting the hull check above.
    let mut ic = 2;
    while orient2d(pts[0], pts[1], pts[ic]) == Orientation::Collinear {
        ic += 1;
    }
    let (mut ia, mut ib) = (0, 1);
    if orient2d(pts[ia], pts[ib], pts[ic]) == Orientation::Clockwise {
        std::mem::swap(&mut ia, &mut ib);
    }

    // The real triangle plus a closed ghost fan around its 3 outer edges,
    // each stored so the ghost sits on the correct (outward) side — see
    // `is_bad`'s single-ghost case.
    let mut tris: Vec<[usize; 3]> = vec![
        [ia, ib, ic],
        [ib, ia, GHOST],
        [ic, ib, GHOST],
        [ia, ic, GHOST],
    ];

    for i in 0..pts.len() {
        if i == ia || i == ib || i == ic {
            continue;
        }
        insert_point(&mut tris, &pts, i);
    }

    let triangles = tris
        .into_iter()
        .filter(|t| t.iter().all(|&idx| !is_ghost(idx)))
        .map(|[a, b, c]| Triangle2::new(pts[a], pts[b], pts[c]))
        .collect();

    Triangulation2 { triangles }
}

/// Whether triangle `tri`'s circumcircle strictly contains `p`, handling
/// the (at most one) ghost vertex case as the limit of a circumcircle
/// receding to infinity: for CCW real edge `(u, v)` with the ghost as the
/// third ("far away") vertex, that limit is the half-plane strictly left of
/// `u -> v` — see `delaunay2`'s doc comment for why at most one ghost can
/// ever occur.
fn is_bad(pts: &[Point2], tri: [usize; 3], p: Point2) -> bool {
    let [a, b, c] = tri;
    match (is_ghost(a), is_ghost(b), is_ghost(c)) {
        (false, false, false) => incircle(pts[a], pts[b], pts[c], p) == Sign::Positive,
        (true, false, false) => orient2d(pts[b], pts[c], p) == Orientation::CounterClockwise,
        (false, true, false) => orient2d(pts[c], pts[a], p) == Orientation::CounterClockwise,
        (false, false, true) => orient2d(pts[a], pts[b], p) == Orientation::CounterClockwise,
        _ => unreachable!("a triangle can never carry more than one ghost vertex"),
    }
}

/// Inserts `pts[p_idx]` into `tris` via the Bowyer-Watson cavity
/// construction: find every triangle whose circumcircle strictly contains
/// the new point ("bad", see `is_bad`), remove them, and fan the resulting
/// cavity boundary to the new point.
///
/// The cavity boundary is found via directed-edge cancellation: every bad
/// triangle contributes its three CCW-ordered edges; an edge whose reverse
/// is *also* contributed by some (other) bad triangle is internal to the
/// cavity and cancels out, leaving only the boundary. This relies on the
/// bad-triangle set always being star-shaped around the new point (a
/// property of exact `incircle`/`orient2d` evaluation on a valid Delaunay
/// triangulation) — property-tested, not just assumed, in
/// `tests/differential/delaunay2.rs`.
fn insert_point(tris: &mut Vec<[usize; 3]>, pts: &[Point2], p_idx: usize) {
    let p = pts[p_idx];

    let bad: Vec<usize> = tris
        .iter()
        .enumerate()
        .filter(|&(_, &tri)| is_bad(pts, tri, p))
        .map(|(i, _)| i)
        .collect();

    let mut edges: Vec<(usize, usize)> = Vec::with_capacity(bad.len() * 3);
    for &ti in &bad {
        let [a, b, c] = tris[ti];
        edges.push((a, b));
        edges.push((b, c));
        edges.push((c, a));
    }
    let edge_set: HashSet<(usize, usize)> = edges.iter().copied().collect();
    let boundary: Vec<(usize, usize)> = edges
        .into_iter()
        .filter(|&(u, v)| !edge_set.contains(&(v, u)))
        .collect();

    for &ti in bad.iter().rev() {
        tris.swap_remove(ti);
    }

    for (u, v) in boundary {
        tris.push([u, v, p_idx]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predicates::Orientation;
    use crate::primitives::PointTriangleRelation;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn all_ccw(t: &Triangulation2) {
        for tri in t.triangles() {
            assert_eq!(tri.orientation(), Orientation::CounterClockwise);
        }
    }

    /// `is_bad`'s single-ghost reduction was derived (and numerically
    /// checked against the old super-triangle limit) for exactly one
    /// vertex position; the other two follow the same rotational argument
    /// but were never independently verified. Rotating a triangle's vertex
    /// order can't change what triangle it represents, so all three
    /// single-ghost arms must agree for any rotation of the same
    /// (real, real, ghost) triangle.
    #[test]
    fn is_bad_single_ghost_arms_agree_under_rotation() {
        let mut rng = 0x1234_5678_9abc_def0_u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 11) as f64 * (1.0 / (1u64 << 53) as f64) * 2.0 - 1.0
        };
        for _ in 0..200 {
            let pts = vec![p(next(), next()), p(next(), next())];
            let query = p(next(), next());
            let (a, b) = (0usize, 1usize);
            let by_c = is_bad(&pts, [a, b, GHOST], query);
            let by_a = is_bad(&pts, [GHOST, a, b], query);
            let by_b = is_bad(&pts, [b, GHOST, a], query);
            assert_eq!(by_c, by_a, "ghost-at-c vs ghost-at-a disagree");
            assert_eq!(by_c, by_b, "ghost-at-c vs ghost-at-b disagree");
        }
    }

    /// The Delaunay property itself: no input point lies strictly inside
    /// any output triangle's circumcircle.
    fn empty_circumcircle_property(points: &[Point2], t: &Triangulation2) {
        for tri in t.triangles() {
            for &q in points {
                let sign = incircle(tri.a(), tri.b(), tri.c(), q);
                assert_ne!(
                    sign,
                    Sign::Positive,
                    "point {q:?} strictly inside circumcircle of {tri:?}"
                );
            }
        }
    }

    fn every_vertex_is_an_input_point(points: &[Point2], t: &Triangulation2) {
        for tri in t.triangles() {
            for v in [tri.a(), tri.b(), tri.c()] {
                assert!(points.contains(&v), "triangle vertex {v:?} not in input");
            }
        }
    }

    #[test]
    fn empty_input() {
        let t = delaunay2(&[]);
        assert!(t.is_empty());
    }

    #[test]
    fn one_and_two_points() {
        assert!(delaunay2(&[p(0.0, 0.0)]).is_empty());
        assert!(delaunay2(&[p(0.0, 0.0), p(1.0, 1.0)]).is_empty());
    }

    #[test]
    fn fully_collinear_input() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)];
        assert!(delaunay2(&pts).is_empty());
    }

    #[test]
    fn single_triangle() {
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)];
        let t = delaunay2(&pts);
        assert_eq!(t.len(), 1);
        all_ccw(&t);
        every_vertex_is_an_input_point(&pts, &t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn square_gives_two_triangles() {
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let t = delaunay2(&pts);
        assert_eq!(t.len(), 2);
        all_ccw(&t);
        every_vertex_is_an_input_point(&pts, &t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn square_with_center_point_gives_four_triangles() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        let t = delaunay2(&pts);
        // n=5, h=4 hull vertices, no 3 collinear: 2n - 2 - h = 4.
        assert_eq!(t.len(), 4);
        all_ccw(&t);
        every_vertex_is_an_input_point(&pts, &t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn point_on_interior_edge_splits_both_adjacent_triangles() {
        // Two triangles sharing edge (2,2)-(0,0)/(4,0)... concretely: a
        // square split by inserting a point exactly on its diagonal.
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0), // exactly on the (0,0)-(4,4) diagonal
        ];
        let t = delaunay2(&pts);
        // The point on the shared diagonal must split both triangles that
        // would otherwise meet there into 2 each: 4 total, matching the
        // 2n-2-h count above but for a different, degenerate-input reason.
        assert_eq!(t.len(), 4);
        all_ccw(&t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn point_on_hull_boundary_edge_splits_one_triangle() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(0.0, 4.0),
            p(2.0, 0.0), // exactly on the (0,0)-(4,0) hull edge
        ];
        let t = delaunay2(&pts);
        assert_eq!(t.len(), 2);
        all_ccw(&t);
        empty_circumcircle_property(&pts, &t);
    }

    #[test]
    fn cocircular_square_plus_center_is_stable_across_permutations() {
        // A square's 4 corners are exactly cocircular; the 5th point sits
        // off-center, breaking the tie among the corners but stressing the
        // Sign::Zero "not bad" rule for the corner-only circle.
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(1.0, 1.0),
        ];
        let base = delaunay2(&pts);
        empty_circumcircle_property(&pts, &base);
        all_ccw(&base);
        let mut shuffled = pts;
        shuffled.reverse();
        let other = delaunay2(&shuffled);
        assert_eq!(base.triangles(), other.triangles());
    }

    #[test]
    fn triangulation_covers_the_convex_hull() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(1.0, 1.0),
            p(3.0, 1.0),
            p(2.0, 3.0),
        ];
        let t = delaunay2(&pts);
        // Every input point must be inside-or-on some output triangle.
        for &q in &pts {
            let covered = t
                .triangles()
                .iter()
                .any(|tri| tri.relation_to(q) != PointTriangleRelation::Outside);
            assert!(covered, "point {q:?} not covered by any triangle");
        }
    }

    #[test]
    fn ghost_vertex_never_leaks_into_output() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(1.0, 1.0),
        ];
        let t = delaunay2(&pts);
        every_vertex_is_an_input_point(&pts, &t);
    }
}
