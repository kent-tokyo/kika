//! `cargo run --example delaunay`

use kika::{Point2, delaunay2};

fn main() {
    let points: Vec<Point2> = [
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 4.0),
        (0.0, 4.0),
        (2.0, 2.0), // interior point
    ]
    .into_iter()
    .map(|(x, y)| Point2::new(x, y).unwrap())
    .collect();

    let triangulation = delaunay2(&points);
    println!("{} triangles:", triangulation.len());
    for tri in triangulation.triangles() {
        println!(
            "  ({}, {}) ({}, {}) ({}, {})  orientation={:?}",
            tri.a().x(),
            tri.a().y(),
            tri.b().x(),
            tri.b().y(),
            tri.c().x(),
            tri.c().y(),
            tri.orientation(),
        );
    }
}
