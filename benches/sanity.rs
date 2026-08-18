//! Small-scale, fixed-seed sanity benchmarks (n=100/300/1000).
//!
//! **Performance has not yet been optimized.** These are small
//! deterministic sanity checks, not a competitive or micro-optimized
//! benchmark suite — full AGENTS.md §13 scope (predicate fast-path rate,
//! adaptive/exact fallback rate, allocation count, peak memory, WASM
//! timing) is a separate, not-yet-built backlog item (`tasks/todo.md`).
//! This exists only to confirm completion (triangle counts, topology
//! validity) and catch a catastrophic algorithmic regression (e.g. an
//! accidental O(n^3) path), via a generous time ceiling — not to make any
//! precise or competitive timing claim.
//!
//! `harness = false` in `Cargo.toml` (a plain `fn main()`, not the
//! nightly-only `#[bench]` attribute) so this runs on stable, matching
//! the crate's MSRV — run via `cargo bench --bench sanity`.
//!
//! `constrained_delaunay2`/`triangulate_polygon` are visibly slower per
//! point than plain `delaunay2` here — not a regression, but the
//! already-documented O(n) coordinate-lookup-per-input-point
//! (`constrained_delaunay2`'s `vertex_of_coord`) and O(n) per-constraint
//! adjacency scans (`insert_constraint_edge`/`crossing_edges`), each
//! called O(n) times for an n-vertex polygon's full boundary constraint
//! set — an accepted O(n²)-ish characteristic for this narrow scope's
//! expected input sizes (`docs/adr/ADR-004-exact-construction-strategy.md`,
//! `src/triangulation/cdt.rs`'s module doc comment), not something this
//! bench exists to flag. `triangulate_polygon`'s time ceiling is set
//! accordingly higher than `delaunay2`/`constrained_delaunay2`'s.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use kika::{Point2, Polygon2, constrained_delaunay2, delaunay2, triangulate_polygon, voronoi2};

const TIME_CEILING: Duration = Duration::from_secs(5);
const POLYGON_TIME_CEILING: Duration = Duration::from_secs(30);

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// `n` distinct random points in `[0, scale] x [0, scale]`, deterministic
/// given `seed`.
fn random_points(seed: u64, n: usize, scale: f64) -> Vec<Point2> {
    let mut state = seed;
    let mut seen = HashSet::new();
    let mut pts = Vec::with_capacity(n);
    while pts.len() < n {
        let x = ((xorshift(&mut state) % 1_000_000) as f64 / 1_000_000.0) * scale;
        let y = ((xorshift(&mut state) % 1_000_000) as f64 / 1_000_000.0) * scale;
        if seen.insert((x.to_bits(), y.to_bits())) {
            pts.push(Point2::new(x, y).unwrap());
        }
    }
    pts
}

/// A `k`-pointed star (`2k` vertices, alternating outer/inner ring) — `k`
/// convex vertices, `k` reflex vertices, and `k` separate concave pockets
/// between the star's own boundary and its point set's convex hull, so
/// `triangulate_polygon`'s flood fill has real (not just trivial, as a
/// convex polygon would give) discard work to do at scale.
fn star_polygon(k: usize, outer_r: f64, inner_r: f64) -> Polygon2 {
    let mut pts = Vec::with_capacity(2 * k);
    for i in 0..k {
        let theta_outer = 2.0 * std::f64::consts::PI * (i as f64) / (k as f64);
        let theta_inner = theta_outer + std::f64::consts::PI / (k as f64);
        pts.push(Point2::new(outer_r * theta_outer.cos(), outer_r * theta_outer.sin()).unwrap());
        pts.push(Point2::new(inner_r * theta_inner.cos(), inner_r * theta_inner.sin()).unwrap());
    }
    Polygon2::new(pts)
}

fn check_delaunay(n: usize) {
    let pts = random_points(0x9E37_79B9_7F4A_7C15 ^ n as u64, n, 1000.0);
    let start = Instant::now();
    let t = delaunay2(&pts);
    let elapsed = start.elapsed();
    let errors = t.validate_topology();
    assert!(errors.is_empty(), "delaunay2 n={n}: {errors:?}");
    assert!(
        elapsed < TIME_CEILING,
        "delaunay2 n={n} took {elapsed:?}, expected well under {TIME_CEILING:?}"
    );
    eprintln!(
        "delaunay2               n={n:<5} {:>6} triangles  {elapsed:?}",
        t.len()
    );
}

fn check_constrained_delaunay(n: usize) {
    let pts = random_points(0xC2B2_AE3D_27D4_EB4F ^ n as u64, n, 1000.0);
    // A fan of non-crossing constraints sharing endpoint 0 -- two
    // segments sharing an endpoint can never properly cross each other,
    // so this needs no extra crossing check to stay valid, while still
    // forcing real flip-based recovery for constraints that aren't
    // already Delaunay edges.
    let constraints: Vec<(usize, usize)> = (1..pts.len()).step_by(7).map(|i| (0, i)).collect();
    let start = Instant::now();
    let cdt = constrained_delaunay2(&pts, &constraints).unwrap();
    let elapsed = start.elapsed();
    let errors = kika::validate_cdt_topology(&cdt);
    assert!(errors.is_empty(), "constrained_delaunay2 n={n}: {errors:?}");
    assert!(
        elapsed < TIME_CEILING,
        "constrained_delaunay2 n={n} took {elapsed:?}, expected well under {TIME_CEILING:?}"
    );
    eprintln!(
        "constrained_delaunay2  n={n:<5} {:>6} triangles  {} constraints  {elapsed:?}",
        cdt.triangulation().len(),
        constraints.len()
    );
}

fn check_voronoi(n: usize) {
    let pts = random_points(0xA5A5_1F2E_3C4D_5B6A ^ n as u64, n, 1000.0);
    let start = Instant::now();
    let voronoi = voronoi2(delaunay2(&pts));
    let elapsed = start.elapsed();
    let errors = voronoi.validate_voronoi_topology();
    assert!(errors.is_empty(), "voronoi2 n={n}: {errors:?}");
    assert!(
        elapsed < TIME_CEILING,
        "voronoi2 n={n} took {elapsed:?}, expected well under {TIME_CEILING:?}"
    );
    eprintln!(
        "voronoi2                n={n:<5} {:>6} cells  {:>6} edges  {elapsed:?}",
        voronoi.cells().count(),
        voronoi.edges().count()
    );
}

fn check_polygon_triangulation(n: usize) {
    let k = n / 2;
    let poly = star_polygon(k, 1000.0, 400.0);
    let start = Instant::now();
    let t = triangulate_polygon(&poly).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(
        t.len(),
        poly.len() - 2,
        "triangulate_polygon n={n}: wrong triangle count"
    );
    assert!(
        elapsed < POLYGON_TIME_CEILING,
        "triangulate_polygon n={n} took {elapsed:?}, expected well under {POLYGON_TIME_CEILING:?}"
    );
    eprintln!(
        "triangulate_polygon    n={n:<5} {:>6} triangles  {elapsed:?}",
        t.len()
    );
}

fn main() {
    println!(
        "Performance has not yet been optimized. Small deterministic sanity \
         benchmarks are provided to detect catastrophic regressions.\n"
    );
    for &n in &[100, 300, 1000] {
        check_delaunay(n);
        check_constrained_delaunay(n);
        check_voronoi(n);
        check_polygon_triangulation(n);
    }
    println!("\nAll sanity checks passed.");
}
