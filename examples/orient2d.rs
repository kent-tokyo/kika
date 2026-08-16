//! `cargo run --example orient2d`
//!
//! The most basic robust predicate: which way does `a -> b -> c` turn?

use kika::{Orientation, Point2, orient2d};

fn main() {
    let a = Point2::new(0.0, 0.0).unwrap();
    let b = Point2::new(1.0, 0.0).unwrap();
    let c = Point2::new(0.0, 1.0).unwrap();

    match orient2d(a, b, c) {
        Orientation::CounterClockwise => println!("a -> b -> c turns left (CCW)"),
        Orientation::Clockwise => println!("a -> b -> c turns right (CW)"),
        Orientation::Collinear => println!("a, b, c are collinear"),
    }

    // Exact even when floating-point subtraction alone would round away
    // the true answer -- the whole point of a robust predicate.
    let near_collinear = Point2::new(1.0, 1e-16).unwrap();
    println!(
        "orient2d(a, b, near_collinear) = {:?}",
        orient2d(a, b, near_collinear)
    );
}
