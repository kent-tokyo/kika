//! `cargo run --example segment_intersection`
//!
//! Classification (`segment_intersection_kind`) and construction
//! (`segment_intersection`) are separate calls by design -- classifying
//! never divides or builds a new coordinate.

use kika::{
    Point2, Segment2, SegmentIntersection2, segment_intersection, segment_intersection_kind,
};

fn main() {
    let s1 = Segment2::new(
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(4.0, 4.0).unwrap(),
    );
    let s2 = Segment2::new(
        Point2::new(0.0, 4.0).unwrap(),
        Point2::new(4.0, 0.0).unwrap(),
    );

    println!("kind: {:?}", segment_intersection_kind(s1, s2));

    match segment_intersection(s1, s2) {
        SegmentIntersection2::Point(p) => {
            // Correctly rounded to the nearest f64 to the true crossing
            // point (ADR-004) -- not a naive approximation.
            println!("crossing point: ({}, {})", p.x(), p.y());
        }
        SegmentIntersection2::Overlap(seg) => println!("collinear overlap: {seg:?}"),
        SegmentIntersection2::None => println!("no intersection"),
    }
}
