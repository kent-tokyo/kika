//! `cargo run --example convex_hull`

use kika::{HullBoundaryPoints, Point2, convex_hull2};

fn main() {
    let points: Vec<Point2> = [
        (0.0, 0.0),
        (2.0, 0.0),
        (2.0, 2.0),
        (0.0, 2.0),
        (1.0, 1.0), // strictly interior -- must not appear in the hull
        (1.0, 0.0), // collinear with two hull corners
    ]
    .into_iter()
    .map(|(x, y)| Point2::new(x, y).unwrap())
    .collect();

    let strict = convex_hull2(&points, HullBoundaryPoints::ExtremesOnly);
    println!("strict corners only: {} vertices", strict.len());
    for v in strict.vertices() {
        println!("  ({}, {})", v.x(), v.y());
    }

    let with_boundary = convex_hull2(&points, HullBoundaryPoints::KeepAllOnBoundary);
    println!(
        "including collinear boundary points: {} vertices",
        with_boundary.len()
    );
}
