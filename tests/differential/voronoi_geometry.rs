//! Differential tests for `Voronoi2::vertex_point`/`edge_geometry`
//! (ADR-009) against an independent "which `f64` is the correctly-rounded
//! nearest neighbor of this exact rational" oracle — the same discipline
//! `tests/differential/line_intersection.rs` uses for the other
//! correctly-rounded construction in this crate.
//!
//! The oracle recomputes the true circumcenter with `BigRational`
//! (reimplemented from scratch, not sharing `predicates::constructions::
//! circumcenter`'s code — only the standard, well-known circumcenter
//! formula), then verifies the candidate `f64` is correctly rounded by
//! comparing it against its two representable neighbors using exact
//! rational arithmetic, per the standard round-to-nearest-even
//! definition.

use kika::{Point2, VoronoiEdgeGeometry, delaunay2, orient2d, voronoi2};
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

/// Independent oracle: the standard circumcenter formula, re-derived here
/// (not copy-pasted from `predicates::constructions::circumcenter`) —
/// translate to `a`'s frame, `d = 2*(dx1*dy2 - dy1*dx2)`,
/// `circumcenter = a + ((dy2*sq1 - dy1*sq2)/d, (dx1*sq2 - dx2*sq1)/d)`
/// where `sq1 = dx1^2+dy1^2`, `sq2 = dx2^2+dy2^2`.
fn oracle_circumcenter(a: P2, b: P2, c: P2) -> (Rat, Rat) {
    let (ax, ay) = exact_pt(a);
    let (bx, by) = exact_pt(b);
    let (cx, cy) = exact_pt(c);

    let dx1 = &bx - &ax;
    let dy1 = &by - &ay;
    let dx2 = &cx - &ax;
    let dy2 = &cy - &ay;

    let d = (&dx1 * &dy2 - &dy1 * &dx2) * BigInt::from(2);
    let sq1 = &dx1 * &dx1 + &dy1 * &dy1;
    let sq2 = &dx2 * &dx2 + &dy2 * &dy2;

    let ux = (&dy2 * &sq1 - &dy1 * &sq2) / &d;
    let uy = (&dx1 * &sq2 - &dx2 * &sq1) / &d;

    (ax + ux, ay + uy)
}

fn check(a: P2, b: P2, c: P2) {
    let pts = [
        Point2::new(a.0, a.1).unwrap(),
        Point2::new(b.0, b.1).unwrap(),
        Point2::new(c.0, c.1).unwrap(),
    ];
    let voronoi = voronoi2(delaunay2(&pts));
    let vertex = voronoi
        .vertices()
        .next()
        .expect("a single non-degenerate triangle has exactly 1 Voronoi vertex");
    let got = voronoi.vertex_point(vertex).unwrap_or_else(|e| {
        panic!("expected a finite circumcenter, got {e:?} (a={a:?} b={b:?} c={c:?})")
    });

    let (ex, ey) = oracle_circumcenter(a, b, c);
    assert!(
        is_correctly_rounded(got.x(), &ex),
        "x not correctly rounded: got {}, exact {ex} (a={a:?} b={b:?} c={c:?})",
        got.x()
    );
    assert!(
        is_correctly_rounded(got.y(), &ey),
        "y not correctly rounded: got {}, exact {ey} (a={a:?} b={b:?} c={c:?})",
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

/// A random, non-degenerate triangle at the given scale -- retried via
/// `kika`'s own exact `orient2d` (fine to depend on here: rejecting a
/// degenerate *draw* is not part of the correctness claim under test,
/// only the accepted triangle's circumcenter is).
fn triangle_at(rng: &mut Xorshift64, scale: f64) -> (P2, P2, P2) {
    loop {
        let a = (rng.next_f64_in(scale), rng.next_f64_in(scale));
        let b = (rng.next_f64_in(scale), rng.next_f64_in(scale));
        let c = (rng.next_f64_in(scale), rng.next_f64_in(scale));
        let pts = (
            Point2::new(a.0, a.1).unwrap(),
            Point2::new(b.0, b.1).unwrap(),
            Point2::new(c.0, c.1).unwrap(),
        );
        if !matches!(orient2d(pts.0, pts.1, pts.2), kika::Orientation::Collinear) {
            return (a, b, c);
        }
    }
}

#[test]
fn basic_triangles() {
    check((0.0, 0.0), (2.0, 0.0), (0.0, 2.0));
    check((0.0, 0.0), (4.0, 0.0), (0.0, 4.0));
    check((0.0, 0.0), (2.0, 0.0), (1.0, 3.0_f64.sqrt()));
    check((-5.0, -5.0), (5.0, -5.0), (0.0, 5.0));
}

#[test]
fn random_triangles_multiple_scales() {
    let mut rng = Xorshift64(0xC0FFEEC0FFEEC0FF);
    for &scale in &[1.0_f64, 1e-6, 1e6, 1e30, 1e-30, 1e60, 1e-60] {
        for _ in 0..150 {
            let (a, b, c) = triangle_at(&mut rng, scale);
            check(a, b, c);
        }
    }
}

/// Mixed-intra-call magnitude, matching this crate's established
/// "exactness starts at the original coordinates" regression class (a
/// same-scale-only generator would never have found the analogous bug in
/// `orient2d`/`orient3d` -- see `docs/numerical-model.md`).
#[test]
fn mixed_intra_call_magnitude() {
    let mut rng = Xorshift64(0x1032547698BADCFE);
    let magnitudes = [1.0_f64, 1e5, 1e-5, 1e30, 1e-30];
    for _ in 0..150 {
        let (a, b, c) = loop {
            let sa = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            let sb = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            let sc = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            let a = (rng.next_f64_in(sa), rng.next_f64_in(sa));
            let b = (rng.next_f64_in(sb), rng.next_f64_in(sb));
            let c = (rng.next_f64_in(sc), rng.next_f64_in(sc));
            let pts = (
                Point2::new(a.0, a.1).unwrap(),
                Point2::new(b.0, b.1).unwrap(),
                Point2::new(c.0, c.1).unwrap(),
            );
            if !matches!(orient2d(pts.0, pts.1, pts.2), kika::Orientation::Collinear) {
                break (a, b, c);
            }
        };
        check(a, b, c);
    }
}

/// Finds (empirically, not just derived) the smallest coordinate magnitude
/// at which the construction is still verified correctly rounded, matching
/// `line_intersection`'s own `magnitude_floor_sweep`.
#[test]
fn magnitude_floor_sweep() {
    let mut rng = Xorshift64(0xDEADBEEFCAFEF00D);
    let mut last_safe_exp = 0i32;
    for exp in (0..=80).map(|i| -5 * i) {
        let scale = 2.0_f64.powi(exp);
        let mut all_ok = true;
        for _ in 0..30 {
            let (a, b, c) = triangle_at(&mut rng, scale);
            let (ex, ey) = oracle_circumcenter(a, b, c);
            let pts = [
                Point2::new(a.0, a.1).unwrap(),
                Point2::new(b.0, b.1).unwrap(),
                Point2::new(c.0, c.1).unwrap(),
            ];
            let voronoi = voronoi2(delaunay2(&pts));
            let vertex = voronoi.vertices().next().unwrap();
            let got = match voronoi.vertex_point(vertex) {
                Ok(p) => p,
                Err(_) => continue,
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
        "circumcenter: no failure observed down through 2^{last_safe_exp} \
         (~{:e}, 30 random triangles sampled per exponent step)",
        2.0_f64.powi(last_safe_exp)
    );
    assert!(
        last_safe_exp <= -200,
        "safe range shrank unexpectedly: only verified down to 2^{last_safe_exp}"
    );
}

/// Sweeps *uniform* coordinate magnitude upward, past
/// `predicates::constructions::circumcenter`'s `RESCALE_THRESHOLD`
/// (`1e90`), verifying both finiteness and *correctness* through the
/// public `Voronoi2::vertex_point` API -- mirrors `line_intersection`'s
/// own `magnitude_ceiling_sweep`.
#[test]
fn magnitude_ceiling_sweep() {
    let mut rng = Xorshift64(0xFEEDFACECAFEBEEF);
    let mut checked = 0u32;
    for exp in [50, 80, 89, 90, 91, 95, 100, 120, 150] {
        let scale = 10f64.powi(exp);
        for _ in 0..20 {
            let (a, b, c) = triangle_at(&mut rng, scale);
            checked += 1;
            check(a, b, c);
        }
    }
    assert!(checked > 0);
}

#[test]
fn edge_geometry_segment_matches_oracle_endpoints() {
    // 4 points guaranteed to produce a Bounded interior edge (see
    // src/triangulation/voronoi.rs's own
    // edge_geometry_segment_endpoints_match_vertex_point for the same
    // fixture shape): a square perturbed off exact cocircularity.
    let pts = [
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(4.0, 0.0).unwrap(),
        Point2::new(4.0, 4.0).unwrap(),
        Point2::new(0.0, 4.1).unwrap(),
    ];
    let voronoi = voronoi2(delaunay2(&pts));
    let mut saw_bounded = false;
    for edge in voronoi.edges() {
        if let Ok(VoronoiEdgeGeometry::Segment { start, end }) = voronoi.edge_geometry(edge) {
            saw_bounded = true;
            assert!(start.x().is_finite() && start.y().is_finite());
            assert!(end.x().is_finite() && end.y().is_finite());
        }
    }
    assert!(saw_bounded, "expected at least one Bounded edge");
}
