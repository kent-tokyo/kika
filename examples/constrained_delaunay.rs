//! `cargo run --example constrained_delaunay`
//!
//! Plain `delaunay2` always picks whichever diagonal of a convex quad is
//! locally Delaunay -- it can't be told to keep a specific edge.
//! `constrained_delaunay2` can: every edge passed in `constraints` is
//! guaranteed present in the result, even where flipping it away would
//! otherwise be the Delaunay choice.

use kika::{EdgeId, Point2, Triangulation2, VertexId, constrained_delaunay2, delaunay2};

fn main() {
    // An asymmetric convex quad -- one of its two diagonals is naturally
    // Delaunay, the other isn't.
    let pts = [
        Point2::new(0.0, 0.0).unwrap(),  // a
        Point2::new(5.0, 1.0).unwrap(),  // b
        Point2::new(4.0, 4.0).unwrap(),  // c
        Point2::new(-1.0, 3.0).unwrap(), // d
    ];

    let natural = delaunay2(&pts);
    println!("plain delaunay2:          {} triangles", natural.len());

    // Constrain diagonal a-c regardless of which diagonal is naturally
    // Delaunay.
    let constraints = [(0usize, 2usize)];
    let cdt = constrained_delaunay2(&pts, &constraints).unwrap();
    println!(
        "constrained_delaunay2:    {} triangles",
        cdt.triangulation().len()
    );

    // Every constraint edge must be present in the result, and marked as
    // constrained (never flipped away, even if not locally Delaunay).
    for &(i, j) in &constraints {
        let edge = find_edge(cdt.triangulation(), pts[i], pts[j])
            .expect("constraint edge must exist in the result");
        assert!(
            cdt.is_constrained(edge),
            "constraint edge must be marked constrained"
        );
        println!("constraint ({i}, {j}): present and marked constrained");
    }
}

fn find_edge(t: &Triangulation2, a: Point2, b: Point2) -> Option<EdgeId> {
    let coord = |id: VertexId| t.vertices().find(|&(vid, _)| vid == id).unwrap().1;
    t.edges().find(|&e| {
        let (u, v) = t.edge_vertices(e);
        let (pu, pv) = (coord(u), coord(v));
        (pu == a && pv == b) || (pu == b && pv == a)
    })
}
