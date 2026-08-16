//! Differential tests for `segment_intersection_kind`/`segment_intersection`
//! against an independent exact-rational reimplementation of the whole
//! classification (not just orient2d — this is the most involved decision
//! tree in the crate so far, composing many orient2d-equivalent checks
//! plus range comparisons; see `tests/differential/point_in_triangle.rs`'s
//! module doc for why re-deriving the logic independently matters more
//! than re-checking arithmetic here).

use kika::{
    Point2, Segment2, SegmentIntersection2, SegmentIntersectionKind, segment_intersection,
    segment_intersection_kind,
};
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

/// Independent oracle, deliberately not sharing structure with the
/// production `classify`: computes exact areas for every configuration
/// (degenerate segments handled via direct point-on-segment / point
/// equality) rather than an AABB-reject + branching decision tree.
fn oracle_kind(a: P2, b: P2, c: P2, d: P2) -> SegmentIntersectionKind {
    let (ea, eb, ec, ed) = (exact_pt(a), exact_pt(b), exact_pt(c), exact_pt(d));

    let zero1 = ea == eb;
    let zero2 = ec == ed;

    if zero1 && zero2 {
        return if ea == ec {
            SegmentIntersectionKind::EndpointTouch
        } else {
            SegmentIntersectionKind::None
        };
    }
    if zero1 {
        return if on_segment(ec, ed, ea) {
            SegmentIntersectionKind::EndpointTouch
        } else {
            SegmentIntersectionKind::None
        };
    }
    if zero2 {
        return if on_segment(ea, eb, ec) {
            SegmentIntersectionKind::EndpointTouch
        } else {
            SegmentIntersectionKind::None
        };
    }

    let d1 = cross(ec.clone(), ed.clone(), ea.clone());
    let d2 = cross(ec.clone(), ed.clone(), eb.clone());
    let d3 = cross(ea.clone(), eb.clone(), ec.clone());
    let d4 = cross(ea.clone(), eb.clone(), ed.clone());

    let opposite = |x: &Rat, y: &Rat| {
        (x.is_positive() && y.is_negative()) || (x.is_negative() && y.is_positive())
    };

    if d1.is_zero() && d2.is_zero() {
        // Fully collinear: exact range overlap via dot-product
        // parametrization along a-b.
        let dx = eb.0.clone() - ea.0.clone();
        let dy = eb.1.clone() - ea.1.clone();
        let denom = dx.clone() * dx.clone() + dy.clone() * dy.clone();
        let t_of = |p: &(Rat, Rat)| -> Rat {
            ((p.0.clone() - ea.0.clone()) * dx.clone() + (p.1.clone() - ea.1.clone()) * dy.clone())
                / denom.clone()
        };
        let (t_a, t_b) = (
            Rat::from_integer(BigInt::from(0)),
            Rat::from_integer(BigInt::from(1)),
        );
        let (t_c, t_d) = (t_of(&ec), t_of(&ed));
        let (lo2, hi2) = if t_c <= t_d { (t_c, t_d) } else { (t_d, t_c) };
        let lo = if t_a >= lo2 { t_a } else { lo2 };
        let hi = if t_b <= hi2 { t_b } else { hi2 };
        return if lo > hi {
            SegmentIntersectionKind::None
        } else if lo == hi {
            SegmentIntersectionKind::CollinearTouch
        } else {
            SegmentIntersectionKind::CollinearOverlap
        };
    }

    if opposite(&d1, &d2) && opposite(&d3, &d4) {
        return SegmentIntersectionKind::Proper;
    }

    if d1.is_zero() && on_segment(ec.clone(), ed.clone(), ea.clone()) {
        return SegmentIntersectionKind::EndpointTouch;
    }
    if d2.is_zero() && on_segment(ec.clone(), ed.clone(), eb.clone()) {
        return SegmentIntersectionKind::EndpointTouch;
    }
    if d3.is_zero() && on_segment(ea.clone(), eb.clone(), ec) {
        return SegmentIntersectionKind::EndpointTouch;
    }
    if d4.is_zero() && on_segment(ea, eb, ed) {
        return SegmentIntersectionKind::EndpointTouch;
    }

    SegmentIntersectionKind::None
}

fn seg(a: P2, b: P2) -> Segment2 {
    Segment2::new(
        Point2::new(a.0, a.1).unwrap(),
        Point2::new(b.0, b.1).unwrap(),
    )
}

fn check(a: P2, b: P2, c: P2, d: P2) {
    let s1 = seg(a, b);
    let s2 = seg(c, d);
    let got = segment_intersection_kind(s1, s2);
    let want = oracle_kind(a, b, c, d);
    assert_eq!(got, want, "kind(a={a:?},b={b:?},c={c:?},d={d:?})");

    // Construction consistency: the kind implied by the construction
    // result must agree with the classification (a basic sanity check
    // on the split between the two, not a re-derivation of the oracle).
    match (got, segment_intersection(s1, s2)) {
        (SegmentIntersectionKind::None, SegmentIntersection2::None) => {}
        (
            SegmentIntersectionKind::Proper
            | SegmentIntersectionKind::EndpointTouch
            | SegmentIntersectionKind::CollinearTouch,
            SegmentIntersection2::Point(_),
        ) => {}
        (SegmentIntersectionKind::CollinearOverlap, SegmentIntersection2::Overlap(_)) => {}
        (k, c) => {
            panic!("kind {k:?} and construction {c:?} disagree for a={a:?},b={b:?},c={c:?},d={d:?}")
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
    fn next_f64_in(&mut self, scale: f64) -> f64 {
        let bits = self.next_u64();
        let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        (unit * 2.0 - 1.0) * scale
    }
}

#[test]
fn basic_cases() {
    // Proper crossing.
    check((0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0));
    // Disjoint.
    check((0.0, 0.0), (1.0, 0.0), (5.0, 5.0), (6.0, 6.0));
    // Shared endpoint.
    check((0.0, 0.0), (1.0, 1.0), (0.0, 0.0), (1.0, -1.0));
    // Endpoint touches other's interior (T-junction).
    check((0.0, 0.0), (4.0, 0.0), (2.0, 0.0), (2.0, 3.0));
    // Collinear touch (end to end).
    check((0.0, 0.0), (2.0, 0.0), (2.0, 0.0), (4.0, 0.0));
    // Collinear overlap.
    check((0.0, 0.0), (4.0, 0.0), (2.0, 0.0), (6.0, 0.0));
    // Collinear, no overlap.
    check((0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0));
    // Collinear, fully containing.
    check((0.0, 0.0), (10.0, 0.0), (2.0, 0.0), (4.0, 0.0));
    // Zero-length vs zero-length, same point.
    check((1.0, 1.0), (1.0, 1.0), (1.0, 1.0), (1.0, 1.0));
    // Zero-length vs zero-length, different points.
    check((1.0, 1.0), (1.0, 1.0), (2.0, 2.0), (2.0, 2.0));
    // Zero-length touching a real segment's interior.
    check((2.0, 0.0), (2.0, 0.0), (0.0, 0.0), (4.0, 0.0));
    // Zero-length off a real segment.
    check((2.0, 1.0), (2.0, 1.0), (0.0, 0.0), (4.0, 0.0));
    // Parallel, non-collinear (never intersect).
    check((0.0, 0.0), (4.0, 0.0), (0.0, 1.0), (4.0, 1.0));
}

#[test]
fn random_segments_multiple_scales() {
    let mut rng = Xorshift64(0x9E3779B97F4A7C15);
    for &scale in &[1.0_f64, 1e-8, 1e8, 1e-60, 1e60] {
        for _ in 0..150 {
            let pt = |rng: &mut Xorshift64| (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let (a, b, c, d) = (pt(&mut rng), pt(&mut rng), pt(&mut rng), pt(&mut rng));
            check(a, b, c, d);
        }
    }
}

#[test]
fn random_near_crossing_stresses_filter_boundary() {
    // Construct segments that cross near a specific point, perturbed
    // slightly, to walk across the endpoint/collinear/proper boundaries.
    let mut rng = Xorshift64(0xD1B54A32D192ED03);
    for &scale in &[1.0_f64, 1e-30, 1e30] {
        for _ in 0..150 {
            let center = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let dir1 = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let dir2 = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let a = (center.0 - dir1.0, center.1 - dir1.1);
            let b = (center.0 + dir1.0, center.1 + dir1.1);
            let perturb = rng.next_f64_in(scale * 1e-10);
            let c = (center.0 - dir2.0 + perturb, center.1 - dir2.1);
            let d = (center.0 + dir2.0 + perturb, center.1 + dir2.1);
            check(a, b, c, d);
        }
    }
}

#[test]
fn random_collinear_segments() {
    let mut rng = Xorshift64(0x2545F4914F6CDD1D);
    for &scale in &[1.0_f64, 1e-30, 1e30] {
        for _ in 0..150 {
            let origin = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let dir = (rng.next_f64_in(1.0), rng.next_f64_in(1.0));
            let along = |t: f64| (origin.0 + t * dir.0, origin.1 + t * dir.1);
            let a = along(rng.next_f64_in(scale));
            let b = along(rng.next_f64_in(scale));
            let c = along(rng.next_f64_in(scale));
            let d = along(rng.next_f64_in(scale));
            check(a, b, c, d);
        }
    }
}

#[test]
fn mixed_intra_call_magnitude() {
    let mut rng = Xorshift64(0x0FEDCBA987654321);
    let magnitudes = [1.0_f64, 1e3, 1e-3, 1e40, 1e-40];
    for _ in 0..300 {
        let coord = |rng: &mut Xorshift64| {
            let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            rng.next_f64_in(m)
        };
        let pt = |rng: &mut Xorshift64| (coord(rng), coord(rng));
        let (a, b, c, d) = (pt(&mut rng), pt(&mut rng), pt(&mut rng), pt(&mut rng));
        check(a, b, c, d);
    }
}
