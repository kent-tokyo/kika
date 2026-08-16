//! Property-based checks for `convex_hull2`, not a from-scratch exact
//! reimplementation like the other `tests/differential/*` files.
//!
//! A `BigRational` reimplementation of monotone chain would mostly
//! re-exercise `orient2d`, which is already proven exact against its own
//! oracle (`tests/differential/orient2d.rs`) — it wouldn't catch a bug in
//! the hull's *decision structure* any better than checking the structural
//! invariants a convex hull must satisfy directly:
//!
//! - every returned vertex is one of the input points
//! - every input point is inside-or-on the returned hull
//! - the hull winds counterclockwise, strictly (`ExtremesOnly`) or
//!   allowing collinear boundary points (`KeepAllOnBoundary`)
//! - the result does not depend on input order
//! - hulling the hull again returns the same thing (idempotence)
//!
//! These are checked directly against `orient2d`/`Segment2::relation_to`,
//! which is the same style of independent check used to design the
//! algorithm itself (see `docs/architecture.md` and the design discussion
//! in `tasks/lessons.md`).

use kika::{
    HullBoundaryPoints, Orientation, Point2, PointSegmentRelation, Polygon2, Segment2,
    convex_hull2, orient2d,
};

struct Xorshift64(u64);
impl Xorshift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn next_f64_in(&mut self, scale: f64) -> f64 {
        let bits = self.next_u64();
        let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        (unit * 2.0 - 1.0) * scale
    }
    fn next_index(&mut self, len: usize) -> usize {
        (self.next_u64() as usize) % len
    }
}

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

/// Checks every structural invariant a convex hull must satisfy, for a
/// single `points`/`boundary` combination.
fn check_hull_properties(points: &[Point2], boundary: HullBoundaryPoints) {
    let hull = convex_hull2(points, boundary);
    let verts = hull.vertices().to_vec();

    for v in &verts {
        assert!(
            points.contains(v),
            "hull vertex {v:?} is not one of the input points"
        );
    }

    let degenerate =
        verts.len() < 3 || Polygon2::new(verts.clone()).orientation() == Orientation::Collinear;

    if degenerate {
        match verts.len() {
            0 => assert!(points.is_empty(), "empty hull but non-empty input"),
            1 => {
                for &p in points {
                    assert_eq!(p, verts[0], "non-matching point with a single-vertex hull");
                }
            }
            _ => {
                let seg = Segment2::new(verts[0], verts[verts.len() - 1]);
                for &p in points {
                    assert_ne!(
                        seg.relation_to(p),
                        PointSegmentRelation::NotOnSegment,
                        "input point {p:?} not within the collinear hull's span {seg:?}"
                    );
                }
            }
        }
    } else {
        let n = verts.len();
        for &p in points {
            for i in 0..n {
                let (a, b) = (verts[i], verts[(i + 1) % n]);
                assert_ne!(
                    orient2d(a, b, p),
                    Orientation::Clockwise,
                    "input point {p:?} lies outside hull edge {a:?} -> {b:?}"
                );
            }
        }
        for i in 0..n {
            let (a, b, c) = (verts[i], verts[(i + 1) % n], verts[(i + 2) % n]);
            let turn = orient2d(a, b, c);
            match boundary {
                HullBoundaryPoints::ExtremesOnly => assert_eq!(
                    turn,
                    Orientation::CounterClockwise,
                    "ExtremesOnly hull has a non-strict corner at {b:?}"
                ),
                HullBoundaryPoints::KeepAllOnBoundary => assert_ne!(
                    turn,
                    Orientation::Clockwise,
                    "KeepAllOnBoundary hull is not convex at {b:?}"
                ),
            }
        }
    }

    let rehulled = convex_hull2(&verts, boundary);
    assert_eq!(
        rehulled.vertices(),
        verts.as_slice(),
        "hull of the hull changed (not idempotent)"
    );
}

fn shuffled(points: &[Point2], rng: &mut Xorshift64) -> Vec<Point2> {
    let mut v = points.to_vec();
    for i in (1..v.len()).rev() {
        let j = rng.next_index(i + 1);
        v.swap(i, j);
    }
    v
}

fn check_all(points: &[Point2], rng: &mut Xorshift64) {
    for boundary in [
        HullBoundaryPoints::ExtremesOnly,
        HullBoundaryPoints::KeepAllOnBoundary,
    ] {
        check_hull_properties(points, boundary);
        let base = convex_hull2(points, boundary);
        for _ in 0..3 {
            let perm = shuffled(points, rng);
            let other = convex_hull2(&perm, boundary);
            assert_eq!(
                base.vertices(),
                other.vertices(),
                "hull depends on input order for {points:?}"
            );
        }
    }
}

#[test]
fn basic_shapes() {
    let mut rng = Xorshift64(0x9E3779B97F4A7C15);
    check_all(&[], &mut rng);
    check_all(&[pt(1.0, 1.0)], &mut rng);
    check_all(&[pt(1.0, 1.0), pt(2.0, 2.0)], &mut rng);
    check_all(
        &[pt(0.0, 0.0), pt(4.0, 0.0), pt(4.0, 4.0), pt(0.0, 4.0)],
        &mut rng,
    );
    check_all(
        &[
            pt(0.0, 0.0),
            pt(4.0, 0.0),
            pt(4.0, 4.0),
            pt(0.0, 4.0),
            pt(2.0, 2.0),
        ],
        &mut rng,
    );
}

#[test]
fn fully_collinear_sets() {
    let mut rng = Xorshift64(0xC0FFEEC0FFEEC0FF);
    for &scale in &[1.0_f64, 1e-30, 1e30] {
        for _ in 0..30 {
            let origin = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let dir = (rng.next_f64_in(1.0), rng.next_f64_in(1.0));
            let points: Vec<Point2> = (0..8)
                .map(|_| {
                    let t = rng.next_f64_in(scale);
                    pt(origin.0 + t * dir.0, origin.1 + t * dir.1)
                })
                .collect();
            check_all(&points, &mut rng);
        }
    }
}

#[test]
fn random_point_clouds() {
    let mut rng = Xorshift64(0xABCDEF0123456789);
    for &scale in &[1.0_f64, 1e-6, 1e6, 1e-40, 1e40] {
        for _ in 0..30 {
            let n = 5 + rng.next_index(15);
            let points: Vec<Point2> = (0..n)
                .map(|_| pt(rng.next_f64_in(scale), rng.next_f64_in(scale)))
                .collect();
            check_all(&points, &mut rng);
        }
    }
}

#[test]
fn random_points_on_a_circle() {
    // Points on a circle stress the "boundary is degenerate everywhere"
    // case differently from a random cloud: many near-collinear triples
    // for a fine sampling, and (for a coarse sampling) genuinely distinct
    // corners.
    let mut rng = Xorshift64(0x1032547698BADCFE);
    for &n in &[6usize, 40] {
        for _ in 0..15 {
            let r = 1.0 + rng.next_f64_in(1.0).abs();
            let points: Vec<Point2> = (0..n)
                .map(|i| {
                    let theta = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
                    pt(r * theta.cos(), r * theta.sin())
                })
                .collect();
            check_all(&points, &mut rng);
        }
    }
}

#[test]
fn duplicates_and_mixed_magnitude() {
    let mut rng = Xorshift64(0xDEADBEEFCAFEF00D);
    let magnitudes = [1.0_f64, 1e5, 1e-5, 1e35, 1e-35];
    for _ in 0..30 {
        let n = 6 + rng.next_index(10);
        let mut points: Vec<Point2> = (0..n)
            .map(|_| {
                let m = magnitudes[rng.next_index(magnitudes.len())];
                pt(rng.next_f64_in(m), rng.next_f64_in(m))
            })
            .collect();
        // Inject exact duplicates, including a signed-zero-only variant.
        if !points.is_empty() {
            let dup = points[0];
            points.push(dup);
        }
        points.push(pt(-0.0, 0.0));
        points.push(pt(0.0, -0.0));
        check_all(&points, &mut rng);
    }
}
