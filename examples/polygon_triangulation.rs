//! `cargo run --example polygon_triangulation`
//!
//! Simple-polygon triangulation via `constrained_delaunay2`, using only
//! the polygon's own vertices (no Steiner points).

use kika::{Orientation, Point2, Polygon2, triangulate_polygon};

fn main() {
    // An L-shaped (non-convex) hexagon.
    let poly = Polygon2::new(
        [
            (0.0, 0.0),
            (4.0, 0.0),
            (4.0, 2.0),
            (2.0, 2.0),
            (2.0, 4.0),
            (0.0, 4.0),
        ]
        .into_iter()
        .map(|(x, y)| Point2::new(x, y).unwrap())
        .collect(),
    );

    let t = triangulate_polygon(&poly).unwrap();
    println!("{} triangles:", t.len());

    // A simple polygon triangulated with only its own vertices always has
    // exactly `polygon.len() - 2` triangles -- true for both convex and
    // (as here) non-convex input, since the concave "pocket" faces
    // outside the polygon are discarded before this count is taken.
    assert_eq!(t.len(), poly.len() - 2);

    let mut total_area = 0.0;
    for tri in t.triangles() {
        assert_eq!(
            tri.orientation(),
            Orientation::CounterClockwise,
            "every output triangle must be CCW"
        );
        let (a, b, c) = (tri.a(), tri.b(), tri.c());
        total_area +=
            ((b.x() - a.x()) * (c.y() - a.y()) - (c.x() - a.x()) * (b.y() - a.y())).abs() / 2.0;
        println!(
            "  ({}, {}) ({}, {}) ({}, {})",
            a.x(),
            a.y(),
            b.x(),
            b.y(),
            c.x(),
            c.y(),
        );
    }

    // Area is preserved: the L-shape is a 4x4 square minus its 2x2
    // missing corner.
    let expected_area = 16.0 - 4.0;
    assert_eq!(total_area, expected_area);
    println!("total area: {total_area} (expected {expected_area})");
}
