//! Minimized regression fixture for a real bug found during development.
//! See `tests/regression/orient2d.rs` for the convention.
//!
//! ## Found: a super-triangle-based Bowyer-Watson can silently drop a
//! triangle, even on ordinary (non-adversarial) input
//!
//! The first `delaunay2` implementation seeded Bowyer-Watson with a
//! synthetic "super-triangle" scaled to 20x the input's bounding-box
//! diagonal, stripped at the end. For 4 points — 3 forming a hull triangle,
//! 1 strictly interior — this produced only 2 triangles instead of the
//! topologically-required 3 (any triangulation of a triangle with one
//! interior point must be the 3-triangle fan; `2n - 2 - h = 2*4 - 2 - 3 =
//! 3` confirms it via Euler's formula too).
//!
//! Root cause: whether a super-triangle vertex "shields" a real edge from
//! getting its second real triangle depends on the *sign* of an `incircle`
//! test involving that synthetic vertex, and that sign is not scale-stable
//! — for this exact 4-point set, `incircle(super_vertex, A, K; B)` was
//! negative at a 20x scale and flipped positive only around 100x, with no
//! way to pick a universally-safe multiplier (the governing ratio is
//! bounding-box diagonal to *smallest relevant point spacing*, which is
//! unbounded). Found via property tests (`tests/differential/delaunay2.rs`'s
//! `watertight_and_matches_hull` cross-check against `convex_hull2`) on
//! ordinary random point clouds, not a constructed adversarial case, then
//! minimized by deleting points one at a time while the failure kept
//! reproducing.
//!
//! Fixed by replacing the super-triangle with a single symbolic "point at
//! infinity" (`GHOST` in `src/triangulation/delaunay2.rs`): a triangle with
//! exactly one ghost vertex reduces its circumcircle test to an exact
//! `orient2d` half-plane test against its one real edge (the limit of a
//! circle whose third point recedes to infinity), so no synthetic
//! coordinate — and no scale-dependent tradeoff — exists anywhere in the
//! algorithm.

use kika::{Point2, delaunay2};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[test]
fn interior_point_produces_the_full_three_triangle_fan() {
    // D, A, B form the hull triangle; K is strictly interior. Before the
    // fix, this returned 2 triangles (missing the A-K-B wedge) instead of
    // the topologically-required 3.
    let d = pt(-0.99936482236534, -0.6464743021054633);
    let a = pt(-0.33887256742679295, 0.42414938537581603);
    let k = pt(0.10589980596211346, 0.7185792656853698);
    let b = pt(0.2821279194842605, 0.8365490815110446);

    let t = delaunay2(&[d, a, k, b]);
    assert_eq!(t.len(), 3);

    let has_vertex =
        |tri: &kika::Triangle2, p: Point2| tri.a() == p || tri.b() == p || tri.c() == p;
    // Every triangle must include K (the interior point); with only 3
    // triangles fanning a single interior point, that's necessary and
    // sufficient to confirm the fan shape (no gap, no overlap).
    assert!(t.triangles().iter().all(|tri| has_vertex(tri, k)));
}
