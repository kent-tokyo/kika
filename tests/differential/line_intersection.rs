//! Differential tests for `predicates::line_intersection` (exposed
//! indirectly via `segment_intersection`'s `Proper` case) against an
//! independent "which `f64` is the correctly-rounded nearest neighbor of
//! this exact rational" oracle — not just "close enough", the actual claim
//! the construction makes (ADR-004).
//!
//! The oracle recomputes the true intersection point with `BigRational`
//! (reimplemented from scratch, not sharing the production formula's
//! code — only its well-known math), then verifies the candidate `f64` is
//! correctly rounded by comparing it against its two representable
//! neighbors using exact rational arithmetic, per the standard
//! round-to-nearest-even definition. This checks the actual correctness
//! *claim*, not just "plausible", the same discipline used everywhere else
//! in this crate's differential tests.

use kika::{Point2, Segment2, SegmentIntersection2, segment_intersection};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

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

/// Next representable `f64` strictly greater than `x`, reimplemented
/// independently from the production `next_up` (same standard bit-pattern
/// technique, but a from-scratch copy so this oracle doesn't depend on the
/// code it's checking).
fn f64_next(x: f64) -> f64 {
    if x == 0.0 {
        return f64::from_bits(1);
    }
    let bits = x.to_bits();
    f64::from_bits(if x > 0.0 { bits + 1 } else { bits - 1 })
}

fn f64_prev(x: f64) -> f64 {
    -f64_next(-x)
}

/// True iff `candidate` is the round-to-nearest-even `f64` closest to the
/// exact value `target`.
fn is_correctly_rounded(candidate: f64, target: &BigRational) -> bool {
    let below = f64_prev(candidate);
    let above = f64_next(candidate);
    let c = exact(candidate);
    let mid_below = (exact(below) + c.clone()) / BigInt::from(2);
    let mid_above = (c + exact(above)) / BigInt::from(2);

    if *target < mid_below || *target > mid_above {
        return false;
    }
    let candidate_even = (candidate.to_bits() & 1) == 0;
    if *target == mid_below || *target == mid_above {
        return candidate_even;
    }
    true
}

type P2 = (f64, f64);
type Rat = BigRational;

fn exact_pt(p: P2) -> (Rat, Rat) {
    (exact(p.0), exact(p.1))
}

/// Independent oracle: `orient2d(p, q, r) = (p.x-r.x)(q.y-r.y) -
/// (p.y-r.y)(q.x-r.x)`, then the standard `d1*b - a*d2 / (d1-d2)`
/// parametric crossing formula (same math the production
/// `line_intersection` derives in its own doc comment, independently
/// re-derived here, not copy-pasted).
fn oracle_intersection(a: P2, b: P2, c: P2, d: P2) -> (Rat, Rat) {
    let (ax, ay) = exact_pt(a);
    let (bx, by) = exact_pt(b);
    let (cx, cy) = exact_pt(c);
    let (dx, dy) = exact_pt(d);

    let orient = |px: &Rat, py: &Rat, qx: &Rat, qy: &Rat, rx: &Rat, ry: &Rat| -> Rat {
        (px - rx) * (qy - ry) - (py - ry) * (qx - rx)
    };
    let d1 = orient(&cx, &cy, &dx, &dy, &ax, &ay);
    let d2 = orient(&cx, &cy, &dx, &dy, &bx, &by);

    let denom = d1.clone() - d2.clone();
    let num_x = d1.clone() * bx - ax * d2.clone();
    let num_y = d1 * by - ay * d2;
    (num_x / denom.clone(), num_y / denom)
}

fn check(a: P2, b: P2, c: P2, d: P2) {
    let s1 = Segment2::new(
        Point2::new(a.0, a.1).unwrap(),
        Point2::new(b.0, b.1).unwrap(),
    );
    let s2 = Segment2::new(
        Point2::new(c.0, c.1).unwrap(),
        Point2::new(d.0, d.1).unwrap(),
    );
    let got = match segment_intersection(s1, s2) {
        SegmentIntersection2::Point(p) => p,
        other => panic!("expected Proper Point intersection, got {other:?}"),
    };
    let (ex, ey) = oracle_intersection(a, b, c, d);
    assert!(
        is_correctly_rounded(got.x(), &ex),
        "x not correctly rounded: got {}, exact {ex} (a={a:?} b={b:?} c={c:?} d={d:?})",
        got.x()
    );
    assert!(
        is_correctly_rounded(got.y(), &ey),
        "y not correctly rounded: got {}, exact {ey} (a={a:?} b={b:?} c={c:?} d={d:?})",
        got.y()
    );
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

/// A "proper crossing" X shape at the given scale: two segments guaranteed
/// to cross strictly in their interiors, so `segment_intersection` reaches
/// the `Proper` construction path being tested.
fn crossing_at(rng: &mut Xorshift64, scale: f64) -> (P2, P2, P2, P2) {
    loop {
        let cx = rng.next_f64_in(scale);
        let cy = rng.next_f64_in(scale);
        let r = scale.max(f64::MIN_POSITIVE) * (0.1 + rng.next_f64_in(1.0).abs());
        let a = (cx - r, cy - r * (0.3 + rng.next_f64_in(1.0).abs()));
        let b = (cx + r, cy + r * (0.3 + rng.next_f64_in(1.0).abs()));
        let c = (cx - r * (0.3 + rng.next_f64_in(1.0).abs()), cy + r);
        let d = (cx + r * (0.3 + rng.next_f64_in(1.0).abs()), cy - r);
        // Guard against degenerate (parallel/zero-length) draws.
        let denom = (b.0 - a.0) * (d.1 - c.1) - (b.1 - a.1) * (d.0 - c.0);
        if a != b && c != d && denom.abs() > 0.0 && denom.is_finite() {
            return (a, b, c, d);
        }
    }
}

#[test]
fn basic_crossings() {
    check((0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0));
    check((0.0, 0.0), (3.0, 3.0), (0.0, 3.0), (3.0, 0.0));
    check((-5.0, -5.0), (5.0, 5.0), (-5.0, 5.0), (5.0, -5.0));
    check((0.0, 0.0), (1.0, 3.0), (0.0, 3.0), (3.0, 0.0));
}

#[test]
fn random_crossings_multiple_scales() {
    let mut rng = Xorshift64(0xC0FFEEC0FFEEC0FF);
    for &scale in &[1.0_f64, 1e-6, 1e6, 1e30, 1e-30, 1e100, 1e-80] {
        for _ in 0..150 {
            let (a, b, c, d) = crossing_at(&mut rng, scale);
            check(a, b, c, d);
        }
    }
}

/// Like `crossing_at`, but the "center" position and the "spread" (`r`)
/// are drawn from independently chosen magnitude buckets, so the 4
/// resulting points can carry wildly different coordinate magnitudes
/// *within the same call* — matching this crate's established
/// "exactness starts at the original coordinates" regression class (a
/// same-scale-only generator would never have found that bug). Still
/// crossing-by-construction (an X shape around the center), just retried
/// until the mixed magnitudes don't degenerate the crossing away.
fn crossing_at_mixed(rng: &mut Xorshift64, magnitudes: &[f64]) -> (P2, P2, P2, P2) {
    loop {
        let pick = |rng: &mut Xorshift64| magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
        let center_scale = pick(rng);
        let cx = rng.next_f64_in(center_scale);
        let cy = rng.next_f64_in(center_scale);
        let r = pick(rng).max(f64::MIN_POSITIVE) * (0.1 + rng.next_f64_in(1.0).abs());
        let a = (cx - r, cy - r * (0.3 + rng.next_f64_in(1.0).abs()));
        let b = (cx + r, cy + r * (0.3 + rng.next_f64_in(1.0).abs()));
        let c = (cx - r * (0.3 + rng.next_f64_in(1.0).abs()), cy + r);
        let d = (cx + r * (0.3 + rng.next_f64_in(1.0).abs()), cy - r);
        let denom = (b.0 - a.0) * (d.1 - c.1) - (b.1 - a.1) * (d.0 - c.0);
        if a != b && c != d && denom.abs() > 0.0 && denom.is_finite() {
            return (a, b, c, d);
        }
    }
}

#[test]
fn mixed_intra_call_magnitude() {
    let mut rng = Xorshift64(0x1032547698BADCFE);
    let magnitudes = [1.0_f64, 1e5, 1e-5, 1e40, 1e-40];
    for _ in 0..150 {
        let (a, b, c, d) = crossing_at_mixed(&mut rng, &magnitudes);
        check(a, b, c, d);
    }
}

/// Finds (empirically, not just derived) the smallest coordinate magnitude
/// at which the construction is still verified correctly rounded, matching
/// the "measure it" discipline used for `incircle`/`insphere`'s own
/// narrower safe ranges. Reports the boundary rather than asserting a
/// specific value, so this stays informative if the underlying expansion
/// machinery ever changes.
#[test]
fn magnitude_floor_sweep() {
    let mut rng = Xorshift64(0xDEADBEEFCAFEF00D);
    let mut last_safe_exp = 0i32;
    for exp in (0..=80).map(|i| -5 * i) {
        let scale = 2.0_f64.powi(exp);
        let mut all_ok = true;
        for _ in 0..50 {
            let (a, b, c, d) = crossing_at(&mut rng, scale);
            let (ex, ey) = oracle_intersection(a, b, c, d);
            let s1 = Segment2::new(
                Point2::new(a.0, a.1).unwrap(),
                Point2::new(b.0, b.1).unwrap(),
            );
            let s2 = Segment2::new(
                Point2::new(c.0, c.1).unwrap(),
                Point2::new(d.0, d.1).unwrap(),
            );
            let got = match segment_intersection(s1, s2) {
                SegmentIntersection2::Point(p) => p,
                _ => continue,
            };
            if !is_correctly_rounded(got.x(), &ex) || !is_correctly_rounded(got.y(), &ey) {
                all_ok = false;
                break;
            }
        }
        if all_ok {
            last_safe_exp = exp;
        } else {
            break;
        }
    }
    eprintln!(
        "line_intersection: no failure observed down through 2^{last_safe_exp} \
         (~{:e}, 50 random crossings sampled per exponent step)",
        2.0_f64.powi(last_safe_exp)
    );
    // Document a conservative floor: this crate's other narrow-range
    // predicates (incircle ~1e-70) stay well clear of their measured
    // limits, so require a comfortable margin here too, not the exact
    // measured edge.
    assert!(
        last_safe_exp <= -280,
        "safe range shrank unexpectedly: only verified down to 2^{last_safe_exp}"
    );
}
