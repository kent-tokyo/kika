//! Differential tests for `Polygon2::relation_to` against an independent
//! exact-rational **winding-number** oracle -- a different algorithm class
//! from production's even-odd **crossing-number** test. For a simple
//! polygon the two rules agree everywhere (a well-known equivalence), so
//! this checks the decision tree with real algorithmic diversity, not just
//! a restatement of the same logic under a different name. See
//! `tests/differential/point_in_triangle.rs`'s module doc for why checking
//! the decision tree (not just the underlying exact arithmetic) matters.

use kika::{Point2, PointPolygonRelation, Polygon2};
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

fn cross(o: (Rat, Rat), a: (Rat, Rat), b: (Rat, Rat)) -> Rat {
    (a.0 - o.0.clone()) * (b.1 - o.1.clone()) - (a.1 - o.1) * (b.0 - o.0)
}

fn exact_pt(p: P2) -> (Rat, Rat) {
    (exact(p.0), exact(p.1))
}

fn on_segment(u: (Rat, Rat), v: (Rat, Rat), p: (Rat, Rat)) -> bool {
    let cr = (v.0.clone() - u.0.clone()) * (p.1.clone() - u.1.clone())
        - (v.1.clone() - u.1.clone()) * (p.0.clone() - u.0.clone());
    if !cr.is_zero() {
        return false;
    }
    if u == v {
        return p == u;
    }
    let dx = v.0.clone() - u.0.clone();
    let dy = v.1.clone() - u.1.clone();
    let t_num = (p.0.clone() - u.0) * dx.clone() + (p.1.clone() - u.1) * dy.clone();
    let t_den = dx.clone() * dx + dy.clone() * dy;
    !(t_num.is_negative() || t_num > t_den)
}

/// Independent oracle: Dan Sunday's winding-number algorithm, done in
/// exact rationals. Mathematically distinct from production's even-odd
/// crossing-number test (asymmetric `<=`/`>` thresholds with a signed
/// +1/-1 per crossing, vs. production's symmetric `>`/`>` even-odd
/// toggle) -- agrees with it everywhere only because both are being
/// evaluated on a simple polygon.
fn oracle_relation(vertices: &[P2], p: P2) -> PointPolygonRelation {
    let n = vertices.len();
    let ep = exact_pt(p);
    let ev: Vec<(Rat, Rat)> = vertices.iter().map(|&v| exact_pt(v)).collect();

    for i in 0..n {
        let j = (i + 1) % n;
        if on_segment(ev[i].clone(), ev[j].clone(), ep.clone()) {
            return PointPolygonRelation::OnBoundary;
        }
    }

    let mut wn: i64 = 0;
    for i in 0..n {
        let j = (i + 1) % n;
        let (a, b) = (ev[i].clone(), ev[j].clone());
        if a.1 <= ep.1 {
            if b.1 > ep.1 && cross(a, b, ep.clone()).is_positive() {
                wn += 1;
            }
        } else if b.1 <= ep.1 && cross(a, b, ep.clone()).is_negative() {
            wn -= 1;
        }
    }
    if wn != 0 {
        PointPolygonRelation::Inside
    } else {
        PointPolygonRelation::Outside
    }
}

fn check(vertices: &[P2], p: P2) {
    let poly = Polygon2::new(
        vertices
            .iter()
            .map(|&(x, y)| Point2::new(x, y).unwrap())
            .collect(),
    );
    let got = poly.relation_to(Point2::new(p.0, p.1).unwrap());
    let want = oracle_relation(vertices, p);
    assert_eq!(got, want, "relation_to(vertices={vertices:?}, p={p:?})");
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
    fn next_f64_in(&mut self, scale: f64) -> f64 {
        let bits = self.next_u64();
        let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        (unit * 2.0 - 1.0) * scale
    }
}

/// Sorts `pts` by angle around their (plain-`f64`) centroid -- star-shaped
/// around the centroid, hence simple, *provided* the centroid actually
/// lands inside the point set's hull. Same technique already used in
/// `src/triangulation/cdt.rs`'s multi-constraint flip-count test, which
/// only ever used it at one consistent magnitude scale.
fn sort_by_angle_around_centroid(pts: &mut [P2]) {
    let cx = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let cy = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;
    pts.sort_by(|a, b| {
        let angle = |p: &P2| (p.1 - cy).atan2(p.0 - cx);
        angle(a).partial_cmp(&angle(b)).unwrap()
    });
}

/// The plain-centroid guarantee above is a real guarantee only when the
/// centroid isn't poisoned by extreme magnitude mixing (found empirically:
/// mixing ~1e29 and ~1e-31 points in one ring drags the arithmetic-mean
/// centroid far outside the hull the small points actually span, and the
/// angle sort silently stops being star-shaped -- see the regression note
/// on `mixed_intra_call_magnitude` below). Verify with the crate's own
/// (already differential-tested) `find_self_intersection` rather than
/// trusting the sort unconditionally.
fn is_simple(pts: &[P2]) -> bool {
    let poly = Polygon2::new(
        pts.iter()
            .map(|&(x, y)| Point2::new(x, y).unwrap())
            .collect(),
    );
    poly.find_self_intersection().is_none()
}

/// A simple (non-self-intersecting) polygon: random points (each drawn
/// from a scale independently chosen out of `magnitudes`) sorted by angle
/// around their centroid, retried until [`is_simple`] confirms it. At a
/// single consistent scale this succeeds essentially immediately; mixing
/// wildly different magnitudes within one ring lowers the success rate
/// (per [`is_simple`]'s doc comment) but the retry loop, not a hand-tuned
/// attempt cap, is what makes this reliable rather than just usually
/// working.
fn random_simple_polygon(rng: &mut Xorshift64, count: usize, magnitudes: &[f64]) -> Vec<P2> {
    for _ in 0..500 {
        let mut pts: Vec<P2> = (0..count)
            .map(|_| {
                let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
                (rng.next_f64_in(m), rng.next_f64_in(m))
            })
            .collect();
        sort_by_angle_around_centroid(&mut pts);
        if is_simple(&pts) {
            return pts;
        }
    }
    panic!("failed to generate a simple polygon after 500 attempts, magnitudes={magnitudes:?}");
}

#[test]
fn basic_convex_and_boundary_cases() {
    let square = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
    check(&square, (2.0, 2.0));
    check(&square, (0.0, 0.0));
    check(&square, (2.0, 0.0));
    check(&square, (5.0, 5.0));
    check(&square, (-1.0, 2.0));
}

#[test]
fn non_convex_l_shape() {
    let l = [
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 2.0),
        (2.0, 2.0),
        (2.0, 4.0),
        (0.0, 4.0),
    ];
    check(&l, (1.0, 1.0));
    check(&l, (3.0, 3.0));
    check(&l, (2.0, 2.0));
    check(&l, (3.0, 1.0));
}

#[test]
fn degenerate_vertex_counts() {
    check(&[], (0.0, 0.0));
    check(&[(1.0, 1.0)], (1.0, 1.0));
    check(&[(1.0, 1.0)], (0.0, 0.0));
    check(&[(0.0, 0.0), (2.0, 0.0)], (1.0, 0.0));
    check(&[(0.0, 0.0), (2.0, 0.0)], (1.0, 1.0));
    check(&[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0)], (2.0, 0.0));
    check(&[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0)], (2.0, 1.0));
}

#[test]
fn random_simple_polygons_random_points() {
    let mut rng = Xorshift64(0xC0FFEEC0FFEEC0FF);
    for &scale in &[1.0_f64, 1e-6, 1e6, 1e-30, 1e30] {
        for _ in 0..40 {
            let count = 4 + (rng.next_u64() % 10) as usize;
            let poly = random_simple_polygon(&mut rng, count, &[scale]);
            for _ in 0..15 {
                let p = (rng.next_f64_in(scale * 1.5), rng.next_f64_in(scale * 1.5));
                check(&poly, p);
            }
            // Also stress boundary membership directly: exact vertices and
            // exact edge midpoints (rational, so still exactly on the edge
            // after round-tripping through f64 -- midpoint of two f64s at
            // the same scale bucket stays exactly representable here since
            // these are small integers/simple fractions in practice; any
            // rounding just becomes a near-boundary interior/exterior case,
            // which is still a valid (if less targeted) check).
            for i in 0..poly.len() {
                check(&poly, poly[i]);
                let j = (i + 1) % poly.len();
                let mid = ((poly[i].0 + poly[j].0) / 2.0, (poly[i].1 + poly[j].1) / 2.0);
                check(&poly, mid);
            }
        }
    }
}

/// Regression note: an earlier version of this test built its polygon by
/// sorting mixed-magnitude points (~1e-31 alongside ~1e29 in the same
/// ring) by angle around a plain-`f64` centroid, unchecked. The centroid
/// ended up dominated by the huge-magnitude points and landed far outside
/// the hull the tiny points actually span, so the sort silently produced a
/// **self-intersecting** ring -- and the test then failed because the
/// winding-number/even-odd equivalence this oracle relies on only holds
/// for a genuinely simple polygon, not because `relation_to` was wrong.
/// Caught by checking `find_self_intersection()` on the generated ring
/// (see [`is_simple`]) rather than trusting the angle-sort unconditionally
/// -- now shared via [`random_simple_polygon`], which retries until the
/// ring is confirmed simple instead of assuming it.
#[test]
fn mixed_intra_call_magnitude() {
    let mut rng = Xorshift64(0x1032547698BADCFE);
    let magnitudes = [1.0_f64, 1e5, 1e-5, 1e30, 1e-30];
    for _ in 0..20 {
        let count = 4 + (rng.next_u64() % 8) as usize;
        let poly = random_simple_polygon(&mut rng, count, &magnitudes);
        for _ in 0..10 {
            let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            check(&poly, (rng.next_f64_in(m), rng.next_f64_in(m)));
        }
    }
}
