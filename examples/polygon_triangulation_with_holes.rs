//! `cargo run --example polygon_triangulation_with_holes`
//!
//! Simple-polygon triangulation with holes (0.4.0), via
//! `triangulate_polygon_with_holes` -- generalizes `triangulate_polygon`
//! (see `examples/polygon_triangulation.rs`), using only the outer
//! boundary's and each hole's own vertices (no Steiner points).

use kika::{Orientation, Point2, Polygon2, triangulate_polygon_with_holes};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
    Polygon2::new(
        [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
            .into_iter()
            .map(|(x, y)| Point2::new(x, y).unwrap())
            .collect(),
    )
}

fn main() {
    // A 10x10 square with two separate 2x2 square holes cut out.
    let outer = rect(0.0, 0.0, 10.0, 10.0);
    let holes = [rect(1.0, 1.0, 3.0, 3.0), rect(6.0, 6.0, 8.0, 8.0)];

    let t = triangulate_polygon_with_holes(&outer, &holes).unwrap();
    println!("{} triangles:", t.len());

    // For `n` total vertices (outer + every hole) and `h` holes, a valid
    // input always triangulates to exactly `n + 2h - 2` triangles --
    // the hole-generalized form of `triangulate_polygon`'s `n - 2`.
    let n: usize = outer.len() + holes.iter().map(|h| h.len()).sum::<usize>();
    assert_eq!(t.len(), n + 2 * holes.len() - 2);

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
    }

    // Area is preserved: outer area minus every hole's area.
    let expected_area = 100.0 - 4.0 - 4.0;
    assert_eq!(total_area, expected_area);
    println!("total area: {total_area} (expected {expected_area})");
}
