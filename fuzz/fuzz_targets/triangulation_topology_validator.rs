#![no_main]

use kika::{HullBoundaryPoints, Orientation, Point2, convex_hull2, delaunay2};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

fn distinct_count(pts: &[Point2]) -> usize {
    let mut seen: Vec<Point2> = Vec::new();
    for &p in pts {
        if !seen.contains(&p) {
            seen.push(p);
        }
    }
    seen.len()
}

/// Unordered edge key: `(min, max)` by `total_cmp` on `(x, y)`, so the same
/// edge visited from either triangle winding order compares equal.
fn edge_key(a: Point2, b: Point2) -> ((f64, f64), (f64, f64)) {
    let ax = (a.x(), a.y());
    let bx = (b.x(), b.y());
    if ax.0.total_cmp(&bx.0).then(ax.1.total_cmp(&bx.1)) == std::cmp::Ordering::Greater {
        (bx, ax)
    } else {
        (ax, bx)
    }
}

// Topology validator: every edge in the triangulation must be shared by
// exactly 1 (hull boundary) or 2 (interior) triangles, never 0 or 3+ — a
// direct check of AGENTS.md §11's "triangulationの辺接続が整合する"
// property. For genuinely 2D (non-collinear) inputs, also checks Euler's
// formula `triangles == 2n - 2 - h` (`n` = distinct points, `h` = convex
// hull boundary point count including collinear-with-neighbor points, per
// `tests/differential/delaunay2.rs`'s established convention — using
// `ExtremesOnly` there instead would undercount `h` whenever a collinear
// boundary point exists, exactly the test-helper bug this crate's own
// `tasks/lessons.md` already records finding twice by hand).
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 30);
    if pts.len() < 3 {
        return;
    }

    let triangulation = delaunay2(&pts);
    let triangles = triangulation.triangles();

    let mut edges: Vec<((f64, f64), (f64, f64))> = Vec::with_capacity(triangles.len() * 3);
    for tri in triangles {
        assert_eq!(tri.orientation(), Orientation::CounterClockwise);
        edges.push(edge_key(tri.a(), tri.b()));
        edges.push(edge_key(tri.b(), tri.c()));
        edges.push(edge_key(tri.c(), tri.a()));
    }
    for e in &edges {
        let uses = edges.iter().filter(|&x| x == e).count();
        assert!(
            uses == 1 || uses == 2,
            "edge {e:?} used by {uses} triangles (expected 1 or 2) in triangulation of {pts:?}"
        );
    }

    let hull = convex_hull2(&pts, HullBoundaryPoints::KeepAllOnBoundary);
    if hull.orientation() != Orientation::Collinear {
        let n = distinct_count(&pts) as isize;
        let h = hull.vertices().len() as isize;
        let expected = 2 * n - 2 - h;
        assert_eq!(
            triangles.len() as isize,
            expected,
            "Euler's formula violated: {} triangles, expected 2*{n} - 2 - {h} = {expected}, points {pts:?}",
            triangles.len()
        );
    }
});
