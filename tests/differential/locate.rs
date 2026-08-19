//! Differential test for `Triangulation2::locate` against the same
//! independent BigRational oracle `tests/differential/point_in_triangle.rs`
//! already validates `Triangle2::relation_to` with (helpers duplicated
//! from there, matching this crate's own per-file convention rather than
//! sharing a common module).
//!
//! `locate`'s own correctness burden is the aggregation/dispatch logic
//! across faces (which vertex/edge/face wins, given each face's own
//! classification) -- the underlying point-in-triangle arithmetic is
//! already proven exact elsewhere. So this test checks `locate`'s
//! *postcondition* directly against the oracle, not against this
//! crate's own `Triangle2::relation_to`/`Segment2::relation_to` (that
//! would only re-test internal consistency, not independent
//! correctness): `Vertex(id)` -> query exactly equals that vertex;
//! `Edge(id)` -> query is exactly on that edge's segment (oracle-checked
//! collinear-and-between); `Face(id)` -> query is exactly inside that
//! triangle (oracle-checked); `Outside` -> no face's oracle
//! classification is `Inside` or `OnBoundary`.
//!
//! Small integer-grid point cloud and query points throughout, per this
//! crate's own established style -- no heavy magnitude/scale sweeps,
//! that's already `point_in_triangle.rs`'s job for the primitive this
//! builds on.

use kika::{Point2, PointLocation, PointTriangleRelation, Triangulation2, VertexId, delaunay2};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

fn exact(x: f64) -> BigRational {
    assert!(x.is_finite());
    if x == 0.0 {
        return BigRational::zero();
    }
    let bits = x.to_bits();
    let sign = if (bits >> 63) & 1 == 1 { -1 } else { 1 };
    let exponent_bits = ((bits >> 52) & 0x7ff) as i64;
    let mantissa_bits = bits & 0xf_ffff_ffff_ffff;
    let (mantissa, exponent) = if exponent_bits == 0 {
        (mantissa_bits, -1074i64)
    } else {
        (mantissa_bits | (1 << 52), exponent_bits - 1075)
    };
    let mantissa = BigInt::from(mantissa) * BigInt::from(sign);
    let mant_rat = BigRational::from_integer(mantissa);
    if exponent >= 0 {
        mant_rat * BigRational::from_integer(BigInt::from(2).pow(exponent as u32))
    } else {
        mant_rat / BigRational::from_integer(BigInt::from(2).pow((-exponent) as u32))
    }
}

type P2 = (f64, f64);
type Rat = BigRational;

fn cross(o: (Rat, Rat), p: (Rat, Rat), q: (Rat, Rat)) -> Rat {
    (p.0 - o.0.clone()) * (q.1 - o.1.clone()) - (p.1 - o.1) * (q.0 - o.0)
}

fn exact_pt(p: P2) -> (Rat, Rat) {
    (exact(p.0), exact(p.1))
}

/// Independent oracle for `Triangle2::relation_to`, reimplemented from
/// scratch (see `point_in_triangle.rs` for the original).
fn oracle_relation(a: P2, b: P2, c: P2, p: P2) -> PointTriangleRelation {
    let (ea, eb, ec, ep) = (exact_pt(a), exact_pt(b), exact_pt(c), exact_pt(p));
    let area2 = cross(ea.clone(), eb.clone(), ec.clone());

    if area2.is_zero() {
        return if oracle_on_segment(a, b, p)
            || oracle_on_segment(b, c, p)
            || oracle_on_segment(c, a, p)
        {
            PointTriangleRelation::OnBoundary
        } else {
            PointTriangleRelation::Outside
        };
    }

    let s1 = cross(ea.clone(), eb.clone(), ep.clone());
    let s2 = cross(eb, ec.clone(), ep.clone());
    let s3 = cross(ec, ea, ep);

    let has_pos = [&s1, &s2, &s3].iter().any(|s| s.is_positive());
    let has_neg = [&s1, &s2, &s3].iter().any(|s| s.is_negative());
    if has_pos && has_neg {
        PointTriangleRelation::Outside
    } else if s1.is_zero() || s2.is_zero() || s3.is_zero() {
        PointTriangleRelation::OnBoundary
    } else {
        PointTriangleRelation::Inside
    }
}

/// Independent oracle for "is `p` on the closed segment `u`-`v`" --
/// exact collinearity plus exact dot-product range membership, no
/// distance metric.
fn oracle_on_segment(u: P2, v: P2, p: P2) -> bool {
    let (eu, ev, ep) = (exact_pt(u), exact_pt(v), exact_pt(p));
    let cr = (ev.0.clone() - eu.0.clone()) * (ep.1.clone() - eu.1.clone())
        - (ev.1.clone() - eu.1.clone()) * (ep.0.clone() - eu.0.clone());
    if !cr.is_zero() {
        return false;
    }
    if eu == ev {
        return ep == eu;
    }
    let dx = ev.0.clone() - eu.0.clone();
    let dy = ev.1.clone() - eu.1.clone();
    let t_num = (ep.0.clone() - eu.0) * dx.clone() + (ep.1.clone() - eu.1) * dy.clone();
    let t_den = dx.clone() * dx + dy.clone() * dy;
    !(t_num.is_negative() || t_num > t_den)
}

fn coord(t: &Triangulation2, id: VertexId) -> P2 {
    let (_, p) = t.vertices().find(|&(vid, _)| vid == id).unwrap();
    (p.x(), p.y())
}

/// Independently verifies (via the oracle above, never any of
/// `locate`'s own primitives) that `result` is a valid `PointLocation`
/// for `query` against `t`.
fn assert_oracle_postcondition(t: &Triangulation2, query: P2, result: PointLocation) {
    match result {
        PointLocation::Vertex(id) => {
            assert_eq!(
                coord(t, id),
                query,
                "Vertex postcondition: query != vertex coord"
            );
        }
        PointLocation::Edge(id) => {
            let (u, v) = t.edge_vertices(id);
            assert!(
                oracle_on_segment(coord(t, u), coord(t, v), query),
                "Edge postcondition: query not on the edge's segment"
            );
        }
        PointLocation::Face(id) => {
            let [v0, v1, v2] = t.face_vertices(id);
            let (a, b, c) = (coord(t, v0), coord(t, v1), coord(t, v2));
            assert_eq!(
                oracle_relation(a, b, c, query),
                PointTriangleRelation::Inside,
                "Face postcondition: query not inside that triangle"
            );
        }
        PointLocation::Outside => {
            for face in t.faces() {
                let [v0, v1, v2] = t.face_vertices(face);
                let (a, b, c) = (coord(t, v0), coord(t, v1), coord(t, v2));
                let rel = oracle_relation(a, b, c, query);
                assert_eq!(
                    rel,
                    PointTriangleRelation::Outside,
                    "Outside postcondition violated: face {face:?} is {rel:?} for query"
                );
            }
        }
    }
}

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
}

#[test]
fn small_integer_grid_locate_matches_independent_oracle() {
    let pts: Vec<Point2> = [
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 4.0),
        (0.0, 4.0),
        (2.0, 2.0),
        (1.0, 3.0),
        (3.0, 1.0),
    ]
    .into_iter()
    .map(|(x, y)| Point2::new(x, y).unwrap())
    .collect();
    let t = delaunay2(&pts);

    // Query set: every input point itself (Vertex postcondition
    // exercised directly), plus small-integer-grid points covering
    // vertices/edges/faces/outside by chance across a fixed seed.
    let mut queries: Vec<P2> = pts.iter().map(|p| (p.x(), p.y())).collect();
    let mut rng = Xorshift64(0x0ac1_ef00_dba5_e155);
    for _ in 0..60 {
        let gx = (rng.next_u64() % 9) as f64 - 2.0; // -2..6
        let gy = (rng.next_u64() % 9) as f64 - 2.0;
        queries.push((gx, gy));
    }

    for q in queries {
        let query = Point2::new(q.0, q.1).unwrap();
        let result = t.locate(query);
        assert_oracle_postcondition(&t, q, result);
    }
}

#[test]
fn empty_and_degenerate_triangulations_satisfy_the_oracle_postcondition_too() {
    for pts in [
        vec![],
        vec![Point2::new(0.0, 0.0).unwrap()],
        vec![
            Point2::new(0.0, 0.0).unwrap(),
            Point2::new(1.0, 0.0).unwrap(),
        ],
        vec![
            Point2::new(0.0, 0.0).unwrap(),
            Point2::new(1.0, 0.0).unwrap(),
            Point2::new(2.0, 0.0).unwrap(),
        ],
    ] {
        let t = delaunay2(&pts);
        for q in [(0.0, 0.0), (5.0, 5.0), (-3.0, -3.0)] {
            let query = Point2::new(q.0, q.1).unwrap();
            let result = t.locate(query);
            assert_eq!(result, PointLocation::Outside);
            // Vacuously true (0 faces) but exercises the same postcondition path.
            assert_oracle_postcondition(&t, q, result);
        }
    }
}
