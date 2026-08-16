//! Property-based checks for `delaunay2`, in the same spirit as
//! `tests/differential/convex_hull2.rs`: a from-scratch exact
//! reimplementation would mostly re-exercise `incircle`/`orient2d`
//! (already proven against their own oracles). What actually needs
//! checking is the algorithm's structural output:
//!
//! - the empty-circumcircle property itself (the definition of Delaunay)
//! - every triangle is CCW and non-degenerate
//! - every returned vertex is an original input point (no ghost-vertex
//!   leak)
//! - the mesh is watertight: every non-hull edge is shared by exactly two
//!   triangles with opposite orientation; the unmatched edges are exactly
//!   the convex hull's boundary (cross-checked against `convex_hull2`) —
//!   this is the check that found a real bug (see
//!   `tests/regression/delaunay2.rs`)
//! - triangle count matches `2n - 2 - h` (Euler's formula), for any input
//!   including degenerate collinear points, using the `KeepAllOnBoundary`
//!   hull count for `h`
//! - determinism under input permutation

use std::collections::HashSet;

use kika::{HullBoundaryPoints, Point2, Sign, Triangulation2, convex_hull2, delaunay2, incircle};

struct Xorshift64(u64);
impl Xorshift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn next_f64_in(&mut self, scale: f64) -> f64 {
        let bits = self.next_u64();
        let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        (unit * 2.0 - 1.0) * scale
    }
    fn next_index(&mut self, len: usize) -> usize {
        (self.next_u64() as usize) % len
    }
}

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn key(p: Point2) -> (u64, u64) {
    (p.x().to_bits(), p.y().to_bits())
}

fn every_vertex_is_an_input_point(points: &[Point2], t: &Triangulation2) {
    for tri in t.triangles() {
        for v in [tri.a(), tri.b(), tri.c()] {
            assert!(points.contains(&v), "triangle vertex {v:?} not in input");
        }
    }
}

fn empty_circumcircle_property(points: &[Point2], t: &Triangulation2) {
    for tri in t.triangles() {
        for &q in points {
            assert_ne!(
                incircle(tri.a(), tri.b(), tri.c(), q),
                Sign::Positive,
                "point {q:?} strictly inside circumcircle of {tri:?}"
            );
        }
    }
}

/// Every directed edge is unique (no exact-duplicate triangle orientation),
/// and the set of edges without a matching reverse (unmatched = hull
/// boundary) exactly equals the convex hull's boundary edges.
///
/// Uses `KeepAllOnBoundary`, not `ExtremesOnly`: a collinear boundary point
/// still splits the triangulation's boundary into two edges (it's a real
/// vertex the triangulation must use), so comparing against the
/// strict-corners-only hull would flag that split as a mismatch even
/// though it's correct — see `triangle_count_matches_euler_formula` for
/// the same distinction.
fn watertight_and_matches_hull(points: &[Point2], t: &Triangulation2) {
    let mut edges = Vec::with_capacity(t.len() * 3);
    for tri in t.triangles() {
        let (a, b, c) = (key(tri.a()), key(tri.b()), key(tri.c()));
        edges.push((a, b));
        edges.push((b, c));
        edges.push((c, a));
    }
    let edge_set: HashSet<_> = edges.iter().copied().collect();
    assert_eq!(edges.len(), edge_set.len(), "duplicate directed edge");

    let unmatched: HashSet<_> = edge_set
        .iter()
        .copied()
        .filter(|&(u, v)| !edge_set.contains(&(v, u)))
        .collect();

    let hull = convex_hull2(points, HullBoundaryPoints::KeepAllOnBoundary);
    let hv = hull.vertices();
    let n = hv.len();
    let hull_edges: HashSet<_> = (0..n).map(|i| (key(hv[i]), key(hv[(i + 1) % n]))).collect();

    assert_eq!(
        unmatched, hull_edges,
        "unmatched triangulation edges don't match the convex hull boundary"
    );
}

fn check_all_properties(points: &[Point2]) {
    let t = delaunay2(points);
    if t.is_empty() {
        return;
    }
    // CCW-ness, edge-manifold incidence, adjacency reciprocity, Euler's
    // formula, and local-Delaunay are all `validate_topology`'s job now
    // (§6B, ADR-006) -- this used to be 2 separate ad hoc implementations
    // here (`all_ccw_and_nondegenerate`, `triangle_count_matches_euler_formula`)
    // duplicating exactly what the internal validator checks; ADR-006's
    // migration plan called for collapsing that duplication once the
    // validator existed, not keeping two copies of the same check.
    assert!(
        t.validate_topology().is_empty(),
        "validate_topology found violations: {:?}",
        t.validate_topology()
    );
    every_vertex_is_an_input_point(points, &t);
    empty_circumcircle_property(points, &t);
    watertight_and_matches_hull(points, &t);
}

fn shuffled(points: &[Point2], rng: &mut Xorshift64) -> Vec<Point2> {
    let mut v = points.to_vec();
    for i in (1..v.len()).rev() {
        let j = rng.next_index(i + 1);
        v.swap(i, j);
    }
    v
}

#[test]
fn basic_shapes_and_triangle_count_formula() {
    let pts = [
        pt(0.0, 0.0),
        pt(4.0, 0.0),
        pt(4.0, 4.0),
        pt(0.0, 4.0),
        pt(1.0, 1.0),
        pt(3.0, 1.0),
        pt(2.0, 3.0),
    ];
    check_all_properties(&pts);
}

#[test]
fn random_point_clouds() {
    let mut rng = Xorshift64(0xABCDEF0123456789);
    for &scale in &[1.0_f64, 1e-6, 1e6, 1e-30, 1e30] {
        for _ in 0..15 {
            let n = 5 + rng.next_index(20);
            let points: Vec<Point2> = (0..n)
                .map(|_| pt(rng.next_f64_in(scale), rng.next_f64_in(scale)))
                .collect();
            check_all_properties(&points);
        }
    }
}

#[test]
fn random_points_on_a_circle() {
    // Heavily cocircular input: exercises the Sign::Zero tie-break rule
    // across many simultaneous ties, not just one quad.
    let mut rng = Xorshift64(0x1032547698BADCFE);
    for &n in &[8usize, 30] {
        for _ in 0..10 {
            let r = 1.0 + rng.next_f64_in(1.0).abs();
            let points: Vec<Point2> = (0..n)
                .map(|i| {
                    let theta = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
                    pt(r * theta.cos(), r * theta.sin())
                })
                .collect();
            check_all_properties(&points);
        }
    }
}

#[test]
fn permutation_invariant() {
    let mut rng = Xorshift64(0x9E3779B97F4A7C15);
    let pts = [
        pt(0.0, 0.0),
        pt(4.0, 0.0),
        pt(4.0, 4.0),
        pt(0.0, 4.0),
        pt(1.0, 1.0),
        pt(3.0, 1.0),
        pt(2.0, 3.0),
    ];
    let base = delaunay2(&pts);
    for _ in 0..5 {
        let perm = shuffled(&pts, &mut rng);
        let other = delaunay2(&perm);
        assert_eq!(base.triangles(), other.triangles());
    }
}

#[test]
fn degenerate_inputs() {
    assert!(delaunay2(&[]).is_empty());
    assert!(delaunay2(&[pt(1.0, 1.0)]).is_empty());
    assert!(delaunay2(&[pt(1.0, 1.0), pt(2.0, 2.0)]).is_empty());
    let collinear = [pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 0.0), pt(3.0, 0.0)];
    assert!(delaunay2(&collinear).is_empty());
    let with_dup = [pt(0.0, 0.0), pt(0.0, 0.0), pt(4.0, 0.0), pt(0.0, 4.0)];
    check_all_properties(&with_dup);
}

#[test]
fn near_collinear_cluster_with_a_far_outlier() {
    // A cluster of points with tiny perpendicular spread relative to its
    // span, plus a far-off outlier: the case a super-triangle-based
    // implementation cannot handle at any fixed scale (unbounded
    // bbox-diagonal-to-point-spacing ratio), but the symbolic
    // point-at-infinity approach handles exactly, at any scale.
    for &eps in &[1e-3, 1e-6, 1e-9, 1e-12, 1e-60, 1e-200] {
        let span = 10.0;
        let mut pts: Vec<Point2> = (0..20)
            .map(|i| {
                let t = i as f64 / 19.0;
                let y = if i % 2 == 0 { 0.0 } else { eps };
                pt(t * span, y)
            })
            .collect();
        pts.push(pt(span / 2.0, 1000.0));

        check_all_properties(&pts);
    }
}
