//! `cargo run --example locate`
//!
//! `Triangulation2::locate` classifies a query point against a
//! triangulation's vertices, edges, and faces -- O(F) linear scan, not a
//! spatial index (see the method's own doc comment). `Outside` means
//! "not covered by any face," not "outside the convex hull": a point
//! inside a `triangulate_polygon_with_holes` hole is also `Outside`.

use kika::{
    Point2, PointLocation, Polygon2, Triangulation2, delaunay2, triangulate_polygon_with_holes,
};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Polygon2 {
    Polygon2::new(vec![pt(x0, y0), pt(x1, y0), pt(x1, y1), pt(x0, y1)])
}

fn report(t: &Triangulation2, label: &str, q: Point2) -> PointLocation {
    let loc = t.locate(q);
    println!("  {label:<28} locate({q:?}) = {loc:?}");
    loc
}

fn main() {
    // A single triangle -- enough to demonstrate all 4 classifications
    // without a hole.
    let pts = [pt(0.0, 0.0), pt(4.0, 0.0), pt(0.0, 4.0)];
    let t = delaunay2(&pts);
    println!("Single triangle:");

    let vertex_id = t.vertices().find(|&(_, p)| p == pt(0.0, 0.0)).unwrap().0;
    assert_eq!(
        report(&t, "vertex (0,0)", pt(0.0, 0.0)),
        PointLocation::Vertex(vertex_id)
    );
    assert!(matches!(
        report(&t, "edge midpoint (2,0)", pt(2.0, 0.0)),
        PointLocation::Edge(_)
    ));
    assert!(matches!(
        report(&t, "interior point (1,1)", pt(1.0, 1.0)),
        PointLocation::Face(_)
    ));
    assert_eq!(
        report(&t, "outside the hull (10,10)", pt(10.0, 10.0)),
        PointLocation::Outside
    );

    // A square with one square hole cut out -- demonstrates Outside's
    // real meaning: hole interior is Outside despite being geometrically
    // inside the outer ring, and the hole's own boundary is a real Edge.
    let outer = rect(0.0, 0.0, 10.0, 10.0);
    let holes = [rect(1.0, 1.0, 3.0, 3.0)];
    let with_hole = triangulate_polygon_with_holes(&outer, &holes).unwrap();
    println!("\nSquare with a square hole:");

    assert!(matches!(
        report(&with_hole, "inside outer, outside hole (7,2)", pt(7.0, 2.0)),
        PointLocation::Face(_)
    ));
    assert_eq!(
        report(&with_hole, "inside the hole (2,2)", pt(2.0, 2.0)),
        PointLocation::Outside
    );
    assert!(matches!(
        report(&with_hole, "hole boundary (2,1)", pt(2.0, 1.0)),
        PointLocation::Edge(_)
    ));

    println!("\nAll assertions passed.");
}
