//! `cargo run --example voronoi`
//!
//! `Voronoi2` is topology-only: no vertex coordinates (circumcenters),
//! just which cells, vertices, and edges exist and how they connect. The
//! square's 4 corners are exactly cocircular -- `delaunay2` splits them
//! into 2 triangles along an arbitrary diagonal, but `voronoi2` merges
//! both triangles' circumcenters into a single Voronoi vertex rather than
//! exposing that diagonal choice as a spurious extra vertex/edge.

use kika::{Point2, VoronoiEdgeKind, delaunay2, voronoi2};

fn main() {
    let pts = [
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(4.0, 0.0).unwrap(),
        Point2::new(4.0, 4.0).unwrap(),
        Point2::new(0.0, 4.0).unwrap(),
        Point2::new(1.0, 1.0).unwrap(), // off-center interior point
    ];
    let voronoi = voronoi2(delaunay2(&pts));

    println!(
        "{} cells, {} Voronoi vertices, {} Voronoi edges",
        voronoi.cells().count(),
        voronoi.vertices().count(),
        voronoi.edges().count()
    );

    let mut unbounded_cells = 0;
    let mut bounded_cells = 0;
    for cell in voronoi.cells() {
        let site = voronoi.cell_site(cell);
        let unbounded = voronoi.cell_is_unbounded(cell);
        if unbounded {
            unbounded_cells += 1;
        } else {
            bounded_cells += 1;
        }
        let edges: Vec<_> = voronoi.cell_edges(cell).collect();
        println!(
            "cell for site {site:?}: {} boundary edges, unbounded={unbounded}",
            edges.len()
        );
        for edge in &edges {
            // Every edge cell_edges() returns for `cell` must itself
            // list `cell` among the 2 cells it separates.
            assert!(voronoi.edge_cells(*edge).contains(&cell));
            match voronoi.edge_kind(*edge) {
                VoronoiEdgeKind::Bounded { .. } => println!("    bounded edge"),
                VoronoiEdgeKind::Unbounded { .. } => println!("    unbounded ray"),
                // VoronoiEdgeKind is #[non_exhaustive]: a future variant
                // (e.g. a degenerate 1-2-site Line case) must not break
                // an existing match at compile time.
                _ => println!("    (unrecognized edge kind)"),
            }
        }
    }

    // The square's 4 corners are hull sites (unbounded cells); the
    // off-center interior point is the only bounded cell.
    assert_eq!(unbounded_cells, 4);
    assert_eq!(bounded_cells, 1);
    assert!(voronoi.validate_voronoi_topology().is_empty());
}
