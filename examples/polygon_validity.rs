//! `cargo run --example polygon_validity`

use kika::{Point2, Polygon2, PolygonBasicValidity};

fn main() {
    let square = Polygon2::new(
        [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]
            .into_iter()
            .map(|(x, y)| Point2::new(x, y).unwrap())
            .collect(),
    );
    println!(
        "square: {:?}, area = {}",
        square.basic_validity(),
        square.signed_area()
    );
    println!("self-intersection: {:?}", square.find_self_intersection());

    // A "bowtie" -- self-intersecting, still a structurally valid ring.
    let bowtie = Polygon2::new(
        [(0.0, 0.0), (2.0, 2.0), (2.0, 0.0), (0.0, 2.0)]
            .into_iter()
            .map(|(x, y)| Point2::new(x, y).unwrap())
            .collect(),
    );
    println!(
        "bowtie basic_validity: {:?} (structurally fine; self-intersecting anyway)",
        bowtie.basic_validity()
    );
    match bowtie.find_self_intersection() {
        Some(hit) => println!(
            "bowtie self-intersects: edges {} and {}, {:?}",
            hit.edge_a, hit.edge_b, hit.kind
        ),
        None => println!("bowtie: no self-intersection found"),
    }

    let degenerate = Polygon2::new(vec![Point2::new(0.0, 0.0).unwrap()]);
    assert_eq!(
        degenerate.basic_validity(),
        PolygonBasicValidity::TooFewVertices
    );
    println!("single-point polygon: {:?}", degenerate.basic_validity());
}
