//! Constrained Delaunay triangulation (Phase 6C), narrow scope per
//! AGENTS.md's phased plan and this session's explicit direction: only
//! non-crossing constraint edges between *existing* input vertices.
//! Deliberately **not** supported (typed errors instead, or simply out of
//! scope — see each error variant and [`constrained_delaunay2`]'s doc
//! comment): constraint segments that properly cross each other,
//! automatic intersection/Steiner-point generation, refinement, quality
//! meshing, automatic constraint splitting.
//!
//! ADR-004's Phase 6 re-evaluation found CDT needs **no new construction**
//! at all — segment recovery here is done entirely by flipping existing
//! Delaunay edges (never dividing, never building a new coordinate), so
//! this module reuses the crate's own [`Segment2`]/[`segment_intersection_kind`]
//! and [`orient2d`]/[`incircle`] predicates throughout, exactly like
//! [`super::delaunay2`] does, and touches ADR-004's construction model not
//! at all.

use std::collections::{HashSet, VecDeque};

use super::delaunay2::{TopologyError, assemble_triangulation};
use super::ids::{EdgeId, FaceId, VertexId};
use super::{Triangulation2, delaunay2};
use crate::hull::dedup_sorted;
use crate::intersections::{SegmentIntersectionKind, segment_intersection_kind};
use crate::predicates::{Orientation, Sign, incircle, orient2d};
use crate::primitives::{Point2, Segment2};

/// Why [`constrained_delaunay2`] rejected an input or failed to build a
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdtError {
    /// A constraint referenced an index `>= points.len()`, or `points`
    /// itself contains a duplicate coordinate — the latter makes
    /// constraint indices ambiguous (which copy does index `i` mean?), so
    /// it is rejected the same way as an out-of-range index rather than
    /// silently deduplicated.
    InvalidVertexIndex,
    /// The same unordered vertex pair appears more than once in
    /// `constraints`.
    DuplicateConstraint,
    /// A constraint's two indices are equal.
    ZeroLengthConstraint,
    /// Two distinct constraint segments properly cross each other (share
    /// a single interior point, neither an endpoint of both). Automatic
    /// intersection generation is out of scope — see the module doc
    /// comment.
    ProperlyCrossingConstraints,
    /// Two distinct constraint segments are collinear and overlap along a
    /// sub-segment (more than a shared endpoint).
    CollinearOverlappingConstraints,
    /// `points` has fewer than 3 elements, or every point in it is exactly
    /// collinear — [`delaunay2`]'s own documented degenerate-input policy
    /// (see its doc comment and `docs/degeneracy-policy.md`): no
    /// triangulation face exists at all, so a non-empty `constraints` list
    /// can never be realized as an edge (there is nothing to flip). An
    /// *empty* `constraints` list is not an error for this same input —
    /// see [`constrained_delaunay2`]'s doc comment.
    DegeneratePointSet,
    /// A constraint could not be realized by edge flipping within the
    /// bounded number of attempts. Covers both a genuine algorithm
    /// exhaustion (see [`constrained_delaunay2`]'s doc comment on the
    /// flip bound) and constraints this narrow scope does not claim to
    /// support (e.g. a constraint segment passing exactly through a third,
    /// unrelated input vertex — no single triangulation edge can realize
    /// that, and this scope does not auto-split it into sub-constraints).
    ConstraintInsertionFailed,
}

/// A 2D constrained Delaunay triangulation: a [`Triangulation2`] plus a
/// marked set of edges that are guaranteed present and were never flipped
/// away, even where doing so would otherwise be locally Delaunay —
/// see [`constrained_delaunay2`].
#[derive(Debug, Clone, PartialEq)]
pub struct ConstrainedTriangulation2 {
    triangulation: Triangulation2,
    constrained_edges: HashSet<EdgeId>,
}

impl ConstrainedTriangulation2 {
    /// The underlying triangulation — every [`Triangulation2`] query
    /// method (`vertices`, `edges`, `faces`, `triangles`, adjacency, …)
    /// is available through it.
    pub fn triangulation(&self) -> &Triangulation2 {
        &self.triangulation
    }

    /// `true` iff `edge` is one of the constraint edges: guaranteed
    /// present in the triangulation and never flipped, even if that means
    /// it is not locally Delaunay.
    pub fn is_constrained(&self, edge: EdgeId) -> bool {
        self.constrained_edges.contains(&edge)
    }
}

fn validate_constraints(points: &[Point2], constraints: &[(usize, usize)]) -> Result<(), CdtError> {
    for &(a, b) in constraints {
        if a >= points.len() || b >= points.len() {
            return Err(CdtError::InvalidVertexIndex);
        }
        if a == b {
            return Err(CdtError::ZeroLengthConstraint);
        }
    }
    if dedup_sorted(points).len() != points.len() {
        return Err(CdtError::InvalidVertexIndex);
    }

    let mut seen_pairs: HashSet<(usize, usize)> = HashSet::new();
    for &(a, b) in constraints {
        let key = if a <= b { (a, b) } else { (b, a) };
        if !seen_pairs.insert(key) {
            return Err(CdtError::DuplicateConstraint);
        }
    }

    for i in 0..constraints.len() {
        for j in (i + 1)..constraints.len() {
            let (a, b) = constraints[i];
            let (c, d) = constraints[j];
            let s1 = Segment2::new(points[a], points[b]);
            let s2 = Segment2::new(points[c], points[d]);
            match segment_intersection_kind(s1, s2) {
                SegmentIntersectionKind::Proper => {
                    return Err(CdtError::ProperlyCrossingConstraints);
                }
                SegmentIntersectionKind::CollinearOverlap => {
                    return Err(CdtError::CollinearOverlappingConstraints);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Builds the constrained Delaunay triangulation of `points` with the
/// given `constraints` (pairs of indices into `points`), or a typed error
/// explaining why it couldn't.
///
/// # Scope (deliberately narrow — see the module doc comment)
///
/// Only constraints between existing, distinct input vertices, and only
/// when no two constraint segments properly cross or collinearly overlap
/// each other (checked exhaustively up front, `O(constraints²)` — fine
/// for this scope's expected input sizes, not optimized for thousands of
/// constraints). `points` must contain no duplicate coordinates (checked
/// via the same `dedup_sorted` the unconstrained `delaunay2` already
/// uses) — a duplicate would make a constraint index ambiguous. `points`
/// may itself be degenerate (fewer than 3 elements, or exactly collinear)
/// — see "Degenerate point sets" below.
///
/// # Degenerate point sets
///
/// If `points` has fewer than 3 elements, or every point in it is exactly
/// collinear, [`delaunay2`] itself returns an empty [`Triangulation2`]
/// (0 faces, 0 vertices) rather than an error — see its own doc comment
/// and `docs/degeneracy-policy.md`. `constrained_delaunay2` matches that
/// policy: with an empty `constraints` list, it returns `Ok` wrapping
/// that same empty triangulation (0 triangles, 0 vertices, no
/// constrained edges). With a non-empty `constraints` list, no
/// triangulation face exists for any constraint to become an edge of, so
/// it returns [`CdtError::DegeneratePointSet`] instead.
///
/// # Algorithm
///
/// 1. Validate every precondition above before touching any triangulation
///    (fail fast, never partially build then discover a problem).
/// 2. Build the ordinary (unconstrained) Delaunay triangulation of
///    `points` via [`delaunay2`]. If it is empty (see "Degenerate point
///    sets" above), return immediately — `Ok` for no constraints,
///    [`CdtError::DegeneratePointSet`] otherwise. Otherwise, map each
///    input index to its [`VertexId`] by coordinate — `delaunay2` returns
///    vertices in its own canonical sorted order, not input order.
/// 3. For each constraint, if it is not already a triangulation edge,
///    realize it via `insert_constraint_edge`'s persistent FIFO queue of
///    crossing edges (via [`segment_intersection_kind`], reusing the same
///    exact primitive `segment_intersection` itself uses) — the standard
///    diagonal-swap / Sloan-style segment recovery, never constructing a
///    new coordinate, only relabeling existing triangles' vertex triples.
///    See that function's own doc comment for why a full rescan each
///    iteration (an earlier, simpler-looking version of this function)
///    is not just less efficient but not even guaranteed to terminate.
/// 4. Once every constraint edge exists, restore local Delaunay-ness on
///    every **unconstrained** edge by the same bounded flipping — a
///    constraint edge is never a flip candidate, by construction.
///
/// Both flip passes are bounded (`4 * face_count + 16` loop passes per
/// constraint, or per full restoration pass) rather than looping until
/// convergence with no ceiling — the same "measured, not assumed"
/// discipline as Phase 5's `correctly_rounded_divide` loop bound. Hitting
/// the bound returns [`CdtError::ConstraintInsertionFailed`] rather than
/// hanging or returning a silently-wrong result — as does a constraint
/// whose segment passes exactly through a third, unrelated input vertex
/// (no single edge can realize that, and this scope does not auto-split
/// it into sub-constraints).
///
/// # Examples
///
/// ```
/// use kika::{Point2, constrained_delaunay2};
///
/// let pts = [
///     Point2::new(0.0, 0.0).unwrap(),
///     Point2::new(4.0, 0.0).unwrap(),
///     Point2::new(4.0, 4.0).unwrap(),
///     Point2::new(0.0, 4.0).unwrap(),
/// ];
/// let constraints = [(0, 2)]; // one diagonal of the square
/// let cdt = constrained_delaunay2(&pts, &constraints).unwrap();
///
/// // Every constraint edge must survive into the result.
/// let constrained_edge_count = cdt
///     .triangulation()
///     .edges()
///     .filter(|&e| cdt.is_constrained(e))
///     .count();
/// assert_eq!(constrained_edge_count, constraints.len());
/// ```
///
/// See `examples/constrained_delaunay.rs` for a runnable version
/// (`cargo run --example constrained_delaunay`).
pub fn constrained_delaunay2(
    points: &[Point2],
    constraints: &[(usize, usize)],
) -> Result<ConstrainedTriangulation2, CdtError> {
    validate_constraints(points, constraints)?;

    let triangulation = delaunay2(points);

    // `delaunay2` returns an empty triangulation (0 faces, 0 vertices) when
    // `points` has fewer than 3 elements or is exactly collinear -- its own
    // documented degenerate-input policy. No face exists in that case, so
    // `vertex_of_coord` below has nothing to map any point onto (it would
    // panic for every point in `points`), and no constraint edge could ever
    // be realized either -- both handled explicitly here, before
    // `vertex_of_coord` is ever called.
    if triangulation.is_empty() {
        return if constraints.is_empty() {
            // No constraints to realize either: matches `delaunay2`'s own
            // "degenerate is a valid, representable value" policy -- an
            // empty result, not an error.
            Ok(ConstrainedTriangulation2 {
                triangulation,
                constrained_edges: HashSet::new(),
            })
        } else {
            Err(CdtError::DegeneratePointSet)
        };
    }

    let vertex_of_coord = |p: Point2| -> VertexId {
        triangulation
            .vertices()
            .find(|&(_, q)| q == p)
            .expect("every input point has a VertexId: duplicates were already rejected")
            .0
    };
    let vertex_id: Vec<VertexId> = points.iter().map(|&p| vertex_of_coord(p)).collect();

    let mut faces: Vec<[VertexId; 3]> = triangulation
        .faces()
        .map(|f| triangulation.face_vertices(f))
        .collect();
    let mut face_neighbors: Vec<[Option<FaceId>; 3]> = triangulation
        .faces()
        .map(|f| triangulation.neighboring_faces(f))
        .collect();
    let vertex_pos: Vec<Point2> = triangulation.vertices().map(|(_, p)| p).collect();

    let mut constrained_pairs: HashSet<(VertexId, VertexId)> = HashSet::new();
    for &(a, b) in constraints {
        let (u, v) = (vertex_id[a], vertex_id[b]);
        insert_constraint_edge(
            &mut faces,
            &mut face_neighbors,
            &vertex_pos,
            &constrained_pairs,
            u,
            v,
        )?;
        constrained_pairs.insert(canon(u, v));
    }

    restore_unconstrained_delaunay(
        &mut faces,
        &mut face_neighbors,
        &vertex_pos,
        &constrained_pairs,
    )?;

    let final_triangulation = assemble_triangulation(vertex_pos, faces);
    let constrained_edges: HashSet<EdgeId> = final_triangulation
        .edges()
        .filter(|&e| {
            let (u, v) = final_triangulation.edge_vertices(e);
            constrained_pairs.contains(&canon(u, v))
        })
        .collect();

    Ok(ConstrainedTriangulation2 {
        triangulation: final_triangulation,
        constrained_edges,
    })
}

fn canon(u: VertexId, v: VertexId) -> (VertexId, VertexId) {
    if u.raw() <= v.raw() { (u, v) } else { (v, u) }
}

fn edge_exists(faces: &[[VertexId; 3]], u: VertexId, v: VertexId) -> bool {
    faces.iter().any(|f| f.contains(&u) && f.contains(&v))
}

/// Every unique undirected *interior* edge (both incident faces
/// present), excluding anything in `constrained_pairs`, as `(u, v, fa,
/// fb)` — `u`/`v` the canonical vertex pair, `fa`/`fb` its two incident
/// faces. Shared scan behind [`crossing_edges`] and
/// [`find_first_bad_unconstrained_edge`], which differ only in what they
/// do with each edge once found (checking whether it crosses a given
/// segment, vs. whether it's locally Delaunay).
///
/// `constrained_pairs` (edges already realized for an earlier constraint
/// in this same call) are never yielded, even though the upfront
/// pairwise non-crossing validation in [`constrained_delaunay2`] should
/// already make excluding them from crossing-edge candidates
/// geometrically unreachable: two constraint segments that don't
/// properly cross each other can't have one's realized edge properly
/// crossed by the other's insertion path either. This filter is defense
/// in depth against that argument being wrong (or violated by a future
/// edit), not a case this scope expects to hit — and is simply correct
/// for the local-Delaunay-restoration use, which must never touch a
/// constrained edge regardless.
fn unconstrained_interior_edges(
    faces: &[[VertexId; 3]],
    face_neighbors: &[[Option<FaceId>; 3]],
    constrained_pairs: &HashSet<(VertexId, VertexId)>,
) -> Vec<(VertexId, VertexId, FaceId, FaceId)> {
    let mut found = Vec::new();
    let mut seen: HashSet<(VertexId, VertexId)> = HashSet::new();
    for (i, face) in faces.iter().enumerate() {
        let fa = FaceId::new(i as u32);
        for k in 0..3 {
            let Some(fb) = face_neighbors[i][k] else {
                continue;
            };
            let (a, b) = (face[(k + 1) % 3], face[(k + 2) % 3]);
            let key = canon(a, b);
            if !seen.insert(key) {
                continue;
            }
            if constrained_pairs.contains(&key) {
                continue;
            }
            found.push((key.0, key.1, fa, fb));
        }
    }
    found
}

/// Every currently-existing triangulation edge that properly crosses
/// segment `(u, v)`, as canonical vertex pairs (not face pairs — a
/// vertex pair stays valid to re-look-up after other flips change
/// adjacency, where a captured `FaceId` pair could go stale).
fn crossing_edges(
    faces: &[[VertexId; 3]],
    face_neighbors: &[[Option<FaceId>; 3]],
    vertex_pos: &[Point2],
    constrained_pairs: &HashSet<(VertexId, VertexId)>,
    u: VertexId,
    v: VertexId,
) -> Vec<(VertexId, VertexId)> {
    let seg_uv = Segment2::new(vertex_pos[u.raw() as usize], vertex_pos[v.raw() as usize]);
    unconstrained_interior_edges(faces, face_neighbors, constrained_pairs)
        .into_iter()
        .filter_map(|(a, b, _fa, _fb)| {
            let seg_ab = Segment2::new(vertex_pos[a.raw() as usize], vertex_pos[b.raw() as usize]);
            (segment_intersection_kind(seg_uv, seg_ab) == SegmentIntersectionKind::Proper)
                .then_some((a, b))
        })
        .collect()
}

/// The two faces currently incident to the (still-existing) edge `(a,
/// b)`, or `None` if `(a, b)` is no longer a triangulation edge (e.g. it
/// was itself flipped away since being queued).
fn adjacent_faces_of_edge(
    faces: &[[VertexId; 3]],
    face_neighbors: &[[Option<FaceId>; 3]],
    a: VertexId,
    b: VertexId,
) -> Option<(FaceId, FaceId)> {
    for (i, face) in faces.iter().enumerate() {
        for k in 0..3 {
            let Some(fb) = face_neighbors[i][k] else {
                continue;
            };
            let (x, y) = (face[(k + 1) % 3], face[(k + 2) % 3]);
            if (x, y) == (a, b) || (x, y) == (b, a) {
                return Some((FaceId::new(i as u32), fb));
            }
        }
    }
    None
}

/// The two vertices shared by `fa`/`fb`'s triangles, and each triangle's
/// own (unshared) apex vertex: `(shared_a, shared_b, apex_of_fa,
/// apex_of_fb)`. Panics (via `expect`) only if `fa`/`fb` are not actually
/// adjacent — an internal-invariant violation, never reachable from
/// `constrained_delaunay2`'s public input validation.
fn shared_and_apex(
    faces: &[[VertexId; 3]],
    fa: FaceId,
    fb: FaceId,
) -> (VertexId, VertexId, VertexId, VertexId) {
    let tri_a = faces[fa.raw() as usize];
    let tri_b = faces[fb.raw() as usize];
    let p = *tri_a
        .iter()
        .find(|x| !tri_b.contains(x))
        .expect("fa, fb must share exactly 2 vertices");
    let q = *tri_b
        .iter()
        .find(|x| !tri_a.contains(x))
        .expect("fa, fb must share exactly 2 vertices");
    let shared: Vec<VertexId> = tri_a.into_iter().filter(|x| *x != p).collect();
    (shared[0], shared[1], p, q)
}

/// `true` iff the quadrilateral formed by `fa`/`fb` (sharing an edge) is
/// strictly convex — the precondition for flipping their shared edge to
/// the other diagonal.
fn can_flip(faces: &[[VertexId; 3]], vertex_pos: &[Point2], fa: FaceId, fb: FaceId) -> bool {
    let (a, b, p, q) = shared_and_apex(faces, fa, fb);
    let (pp, pq, pa, pb) = (
        vertex_pos[p.raw() as usize],
        vertex_pos[q.raw() as usize],
        vertex_pos[a.raw() as usize],
        vertex_pos[b.raw() as usize],
    );
    let oa = orient2d(pp, pq, pa);
    let ob = orient2d(pp, pq, pb);
    matches!(
        (oa, ob),
        (Orientation::Clockwise, Orientation::CounterClockwise)
            | (Orientation::CounterClockwise, Orientation::Clockwise)
    )
}

/// Flips the edge shared by `fa`/`fb` to the other diagonal, reusing both
/// faces' existing slots (their `FaceId`s do not change) and fixing up
/// every affected neighbor pointer, including the reciprocal update on
/// the two outer neighbors that move from one face to the other.
fn flip_edge(
    faces: &mut [[VertexId; 3]],
    face_neighbors: &mut [[Option<FaceId>; 3]],
    fa: FaceId,
    fb: FaceId,
) {
    let ia = fa.raw() as usize;
    let ib = fb.raw() as usize;
    let old_fa = faces[ia];
    let old_fb = faces[ib];
    let old_na = face_neighbors[ia];
    let old_nb = face_neighbors[ib];

    let p = *old_fa
        .iter()
        .find(|x| !old_fb.contains(x))
        .expect("fa, fb must share exactly 2 vertices");
    let q = *old_fb
        .iter()
        .find(|x| !old_fa.contains(x))
        .expect("fa, fb must share exactly 2 vertices");

    let pa = old_fa.iter().position(|&x| x == p).unwrap();
    let fa_rot = [old_fa[pa], old_fa[(pa + 1) % 3], old_fa[(pa + 2) % 3]];
    let na_rot = [old_na[pa], old_na[(pa + 1) % 3], old_na[(pa + 2) % 3]];
    let (u, v) = (fa_rot[1], fa_rot[2]);
    let n_opp_u = na_rot[1]; // fa's old neighbor across edge (p, v), opposite u
    let n_opp_v = na_rot[2]; // fa's old neighbor across edge (p, u), opposite v

    let pb = old_fb.iter().position(|&x| x == q).unwrap();
    let fb_rot = [old_fb[pb], old_fb[(pb + 1) % 3], old_fb[(pb + 2) % 3]];
    let nb_rot = [old_nb[pb], old_nb[(pb + 1) % 3], old_nb[(pb + 2) % 3]];
    debug_assert_eq!(fb_rot[1], v, "fb's CCW order must trace v then u");
    debug_assert_eq!(fb_rot[2], u);
    let n_opp_v_in_fb = nb_rot[1]; // fb's old neighbor across edge (q, u), opposite v
    let n_opp_u_in_fb = nb_rot[2]; // fb's old neighbor across edge (q, v), opposite u

    faces[ia] = [p, u, q];
    faces[ib] = [p, q, v];
    face_neighbors[ia] = [n_opp_v_in_fb, Some(fb), n_opp_v];
    face_neighbors[ib] = [n_opp_u_in_fb, n_opp_u, Some(fa)];

    if let Some(outer) = n_opp_v_in_fb {
        let slot = face_neighbors[outer.raw() as usize]
            .iter()
            .position(|&n| n == Some(fb))
            .expect("reciprocal neighbor must exist");
        face_neighbors[outer.raw() as usize][slot] = Some(fa);
    }
    if let Some(outer) = n_opp_u {
        let slot = face_neighbors[outer.raw() as usize]
            .iter()
            .position(|&n| n == Some(fa))
            .expect("reciprocal neighbor must exist");
        face_neighbors[outer.raw() as usize][slot] = Some(fb);
    }
}

fn flip_bound(face_count: usize) -> usize {
    4 * face_count + 16
}

/// Realizes `(u, v)` as a triangulation edge by flipping every existing
/// edge that properly crosses it — the standard Sloan-style segment
/// recovery, via a **persistent FIFO queue** of crossing edges (not a
/// full rescan-and-pick-first each iteration, which this function
/// originally did and which an earlier version of this doc comment
/// called just a performance simplification of the "same" algorithm).
///
/// That rescan-and-pick-first approach is not just slower — it isn't
/// even guaranteed to terminate: always re-selecting "whichever crossing
/// edge appears first in array-index order" can settle into a 2-cycle,
/// repeatedly flipping the same pair of diagonals back and forth without
/// ever making progress, found via sanity benchmarking on a 300-point
/// random cloud (a single, otherwise-unremarkable constraint). See
/// `tasks/lessons.md`.
///
/// The queue-based fix relies on one fact that only holds when popping
/// (not rescanning): flipping edge `(a, b)` to its only other diagonal
/// `(p, q)` changes the *existence* of exactly those two edges — every
/// other edge's endpoints, and therefore its crossing status against
/// `(u, v)`, is untouched. So after the initial scan, no edge other than
/// the fresh `(p, q)` can newly start (or stop) crossing `(u, v)`; the
/// queue only ever needs `(p, q)` appended, never a full rescan.
fn insert_constraint_edge(
    faces: &mut [[VertexId; 3]],
    face_neighbors: &mut [[Option<FaceId>; 3]],
    vertex_pos: &[Point2],
    constrained_pairs: &HashSet<(VertexId, VertexId)>,
    u: VertexId,
    v: VertexId,
) -> Result<(), CdtError> {
    if edge_exists(faces, u, v) {
        return Ok(());
    }

    let mut queue: VecDeque<(VertexId, VertexId)> =
        crossing_edges(faces, face_neighbors, vertex_pos, constrained_pairs, u, v)
            .into_iter()
            .collect();
    let seg_uv = Segment2::new(vertex_pos[u.raw() as usize], vertex_pos[v.raw() as usize]);

    // `bound` limits total loop passes (flips plus not-yet-flippable
    // retries), not just successful flips -- a requeue-without-flipping
    // pass still consumes one. `flips` counts actual `flip_edge` calls;
    // `passes` counts total loop iterations; both are measured (not just
    // the one `bound` is named after) by
    // `flip_count_stays_well_below_the_bound`.
    let bound = flip_bound(faces.len());
    let mut flips = 0u32;
    for passes in 1..=bound as u32 {
        let Some((a, b)) = queue.pop_front() else {
            // An empty queue means every *found* crossing is resolved,
            // not that (u, v) itself now exists -- e.g. if (u, v) passes
            // exactly through a third input vertex w, edges incident to w
            // are classified EndpointTouch/CollinearTouch (never Proper),
            // so they never entered the crossing set at all, and nothing
            // here would ever realize (u, v). Confirm before declaring
            // success.
            if !edge_exists(faces, u, v) {
                return Err(CdtError::ConstraintInsertionFailed);
            }
            record_cdt_flips(flips);
            record_cdt_passes(passes);
            return Ok(());
        };
        let Some((fa, fb)) = adjacent_faces_of_edge(faces, face_neighbors, a, b) else {
            // Already resolved (flipped away) as some other edge's side
            // effect -- not expected given the argument above (no flip
            // touches an edge other than the one popped and its fresh
            // replacement), kept as a defensive skip rather than a panic.
            continue;
        };
        if !can_flip(faces, vertex_pos, fa, fb) {
            queue.push_back((a, b));
            continue;
        }
        let (_, _, p, q) = shared_and_apex(faces, fa, fb);
        flip_edge(faces, face_neighbors, fa, fb);
        flips += 1;
        if (p == u && q == v) || (p == v && q == u) {
            record_cdt_flips(flips);
            record_cdt_passes(passes);
            return Ok(());
        }
        let seg_pq = Segment2::new(vertex_pos[p.raw() as usize], vertex_pos[q.raw() as usize]);
        if segment_intersection_kind(seg_uv, seg_pq) == SegmentIntersectionKind::Proper {
            queue.push_back(canon(p, q));
        }
    }
    Err(CdtError::ConstraintInsertionFailed)
}

/// `true` iff the edge shared by `fa`/`fb` (apexes `p`, `q`) is locally
/// Delaunay: neither apex lies strictly inside the other triangle's
/// circumcircle.
fn is_locally_delaunay(
    faces: &[[VertexId; 3]],
    vertex_pos: &[Point2],
    fa: FaceId,
    fb: FaceId,
) -> bool {
    let (_, _, p, q) = shared_and_apex(faces, fa, fb);
    let (pp, pq) = (vertex_pos[p.raw() as usize], vertex_pos[q.raw() as usize]);
    // faces[fa] is CCW as (p, a, b) up to rotation -- reconstruct the
    // correct CCW order via face_vertices' own stored rotation instead of
    // assuming one, so incircle's orientation precondition holds.
    let tri_a = faces[fa.raw() as usize];
    let tri_b = faces[fb.raw() as usize];
    let ccw_incircle = |tri: [VertexId; 3], vertex_pos: &[Point2], query: Point2| -> Sign {
        let pts = [
            vertex_pos[tri[0].raw() as usize],
            vertex_pos[tri[1].raw() as usize],
            vertex_pos[tri[2].raw() as usize],
        ];
        incircle(pts[0], pts[1], pts[2], query)
    };
    ccw_incircle(tri_a, vertex_pos, pq) != Sign::Positive
        && ccw_incircle(tri_b, vertex_pos, pp) != Sign::Positive
}

/// Restores local Delaunay-ness on every unconstrained edge by
/// repeatedly finding one locally-illegal edge (any one —
/// `find_first_bad_unconstrained_edge` always returns whichever
/// candidate appears first in array order) and flipping it, until none
/// remain or `bound` is hit.
///
/// **Why rescan-and-pick-first terminates here, unlike
/// `insert_constraint_edge`'s original version.** This loop has the same
/// surface shape as `insert_constraint_edge`'s original crossing-edge-recovery
/// loop, which could 2-cycle forever (see that function's doc comment,
/// `tasks/lessons.md`) — but the two loops are governed by different
/// mathematics. Lifting every vertex `(x, y)` to `(x, y, x²+y²)` on the
/// paraboloid, a triangulation's edges are locally Delaunay exactly
/// where the corresponding lifted piecewise-linear surface is convex at
/// that edge (the standard 2D-Delaunay / 3D-lower-convex-hull
/// equivalence — `is_locally_delaunay`'s `incircle` check and
/// `can_flip`'s `orient2d`-based convexity check are exactly this
/// equivalence's two halves). Flipping a locally-illegal edge replaces
/// the two lifted triangles spanning it with the strictly lower pair, so
/// each *executed* flip strictly lowers the total volume under this
/// piecewise-linear surface. That volume is bounded below (by the volume
/// under the point set's true lower convex hull) and there are only
/// finitely many triangulations of a fixed point set, so a sequence of
/// flips that each strictly lowers this quantity can never revisit a
/// prior triangulation and must terminate — regardless of which illegal
/// edge is picked at each step. `insert_constraint_edge`'s crossing-edge
/// selection has no analogous monotonic quantity, which is why it could
/// 2-cycle and this can't.
///
/// Excluding constrained edges from candidacy restricts *which* flips can
/// occur, but every flip this function actually executes is still a
/// legalizing flip of a currently-illegal unconstrained edge, so it still
/// strictly lowers the same global potential — this "frozen edge"
/// extension is the standard treatment for constrained-Delaunay
/// legalization (e.g. Sloan 1993), not independently re-derived from
/// scratch here. If it's wrong, `bound` and the typed
/// `CdtError::ConstraintInsertionFailed` return on exhaustion are the
/// actual safety net, same as `insert_constraint_edge`.
///
/// Stress-tested (not proven) up to 3000-point / 3000-constraint random
/// inputs with zero bound-exhaustion failures; see
/// `flip_count_stays_well_below_the_bound_many_constraints` for the
/// smaller, always-run regression version of that stress test, and its
/// coverage of the multi-constraint-in-one-call mode specifically (the
/// single-constraint `flip_count_stays_well_below_the_bound` never
/// exercises more than one constraint before this function runs).
///
/// `can_flip` failing for an edge already reported illegal is expected to
/// be unreachable in general position (an illegal edge's quadrilateral is
/// convex by a classical lemma), so this errors immediately rather than
/// requeuing like `insert_constraint_edge` does — kept as a defensive
/// typed error, not a panic, in case `incircle`/`orient2d` disagree at
/// the margins; not part of the termination argument above.
fn restore_unconstrained_delaunay(
    faces: &mut [[VertexId; 3]],
    face_neighbors: &mut [[Option<FaceId>; 3]],
    vertex_pos: &[Point2],
    constrained_pairs: &HashSet<(VertexId, VertexId)>,
) -> Result<(), CdtError> {
    let bound = flip_bound(faces.len());
    for iter in 0..bound {
        let bad =
            find_first_bad_unconstrained_edge(faces, face_neighbors, vertex_pos, constrained_pairs);
        match bad {
            None => {
                record_restore_flips(iter as u32);
                return Ok(());
            }
            Some((fa, fb)) => {
                if !can_flip(faces, vertex_pos, fa, fb) {
                    return Err(CdtError::ConstraintInsertionFailed);
                }
                flip_edge(faces, face_neighbors, fa, fb);
            }
        }
    }
    Err(CdtError::ConstraintInsertionFailed)
}

fn find_first_bad_unconstrained_edge(
    faces: &[[VertexId; 3]],
    face_neighbors: &[[Option<FaceId>; 3]],
    vertex_pos: &[Point2],
    constrained_pairs: &HashSet<(VertexId, VertexId)>,
) -> Option<(FaceId, FaceId)> {
    unconstrained_interior_edges(faces, face_neighbors, constrained_pairs)
        .into_iter()
        .find_map(|(_a, _b, fa, fb)| {
            (!is_locally_delaunay(faces, vertex_pos, fa, fb)).then_some((fa, fb))
        })
}

/// Checks `t`'s topology the same way [`super::delaunay2::Triangulation2::validate_topology`]
/// does, but narrowing the local-Delaunay check to unconstrained edges
/// only — a constrained edge is allowed (expected) to violate it.
#[doc(hidden)]
pub fn validate_cdt_topology(cdt: &ConstrainedTriangulation2) -> Vec<TopologyError> {
    cdt.triangulation
        .validate_topology_excluding(&|e| cdt.is_constrained(e))
}

#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
thread_local! {
    static MAX_CDT_FLIPS: Cell<u32> = const { Cell::new(0) };
    // `insert_constraint_edge`'s `bound` limits total loop passes (flips
    // plus not-yet-flippable requeues), not flips alone -- tracked
    // separately since a requeue-heavy run could approach `bound` well
    // before `MAX_CDT_FLIPS` does.
    static MAX_CDT_PASSES: Cell<u32> = const { Cell::new(0) };
    static MAX_RESTORE_FLIPS: Cell<u32> = const { Cell::new(0) };
}
#[cfg(test)]
fn record_cdt_flips(n: u32) {
    MAX_CDT_FLIPS.with(|c| c.set(c.get().max(n)));
}
#[cfg(not(test))]
#[inline(always)]
fn record_cdt_flips(_n: u32) {}
#[cfg(test)]
fn record_cdt_passes(n: u32) {
    MAX_CDT_PASSES.with(|c| c.set(c.get().max(n)));
}
#[cfg(not(test))]
#[inline(always)]
fn record_cdt_passes(_n: u32) {}
#[cfg(test)]
fn record_restore_flips(n: u32) {
    MAX_RESTORE_FLIPS.with(|c| c.set(c.get().max(n)));
}
#[cfg(not(test))]
#[inline(always)]
fn record_restore_flips(_n: u32) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn reset_flip_counters() {
        MAX_CDT_FLIPS.with(|c| c.set(0));
        MAX_CDT_PASSES.with(|c| c.set(0));
        MAX_RESTORE_FLIPS.with(|c| c.set(0));
    }

    /// The unordered vertex-coordinate pair for `edge`, for assertions
    /// that don't want to depend on `VertexId`'s arbitrary numbering.
    fn edge_coords(t: &Triangulation2, edge: EdgeId) -> ((f64, f64), (f64, f64)) {
        let (u, v) = t.edge_vertices(edge);
        let pu = t.vertices().find(|&(id, _)| id == u).unwrap().1;
        let pv = t.vertices().find(|&(id, _)| id == v).unwrap().1;
        let ku = (pu.x(), pu.y());
        let kv = (pv.x(), pv.y());
        if ku <= kv { (ku, kv) } else { (kv, ku) }
    }

    fn find_edge_by_coords(t: &Triangulation2, a: Point2, b: Point2) -> Option<EdgeId> {
        let want = {
            let ka = (a.x(), a.y());
            let kb = (b.x(), b.y());
            if ka <= kb { (ka, kb) } else { (kb, ka) }
        };
        t.edges().find(|&e| edge_coords(t, e) == want)
    }

    #[test]
    fn empty_input_and_no_constraints() {
        let cdt = constrained_delaunay2(&[], &[]).unwrap();
        assert!(cdt.triangulation().is_empty());
        assert!(validate_cdt_topology(&cdt).is_empty());
    }

    #[test]
    fn no_constraints_matches_plain_delaunay() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        let cdt = constrained_delaunay2(&pts, &[]).unwrap();
        let plain = delaunay2(&pts);
        assert_eq!(cdt.triangulation().len(), plain.len());
        assert!(validate_cdt_topology(&cdt).is_empty());
        assert!(cdt.triangulation().validate_topology().is_empty());
    }

    #[test]
    fn invalid_vertex_index() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0)];
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 5)]),
            Err(CdtError::InvalidVertexIndex)
        );
    }

    #[test]
    fn duplicate_point_is_invalid_vertex_index() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0)];
        assert_eq!(
            constrained_delaunay2(&pts, &[]),
            Err(CdtError::InvalidVertexIndex)
        );
    }

    #[test]
    fn zero_length_constraint() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0)];
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 0)]),
            Err(CdtError::ZeroLengthConstraint)
        );
    }

    #[test]
    fn duplicate_constraint() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)];
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 1), (1, 0)]),
            Err(CdtError::DuplicateConstraint)
        );
    }

    #[test]
    fn properly_crossing_constraints() {
        // Two diagonals of a square, both as constraints -- they cross
        // strictly in the interior.
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 2), (1, 3)]),
            Err(CdtError::ProperlyCrossingConstraints)
        );
    }

    #[test]
    fn collinear_overlapping_constraints() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)];
        // (0,2) and (1,3) are collinear and overlap along [1,2].
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 2), (1, 3)]),
            Err(CdtError::CollinearOverlappingConstraints)
        );
    }

    #[test]
    fn single_point_no_constraints_is_empty_ok() {
        let pts = [p(0.0, 0.0)];
        let cdt = constrained_delaunay2(&pts, &[]).unwrap();
        assert!(cdt.triangulation().is_empty());
        assert_eq!(cdt.triangulation().vertices().count(), 0);
        assert!(validate_cdt_topology(&cdt).is_empty());
    }

    #[test]
    fn two_collinear_points_no_constraints_is_empty_ok() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0)];
        let cdt = constrained_delaunay2(&pts, &[]).unwrap();
        assert!(cdt.triangulation().is_empty());
        assert!(validate_cdt_topology(&cdt).is_empty());
    }

    #[test]
    fn four_collinear_points_no_constraints_is_empty_ok() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)];
        let cdt = constrained_delaunay2(&pts, &[]).unwrap();
        assert!(cdt.triangulation().is_empty());
        assert!(validate_cdt_topology(&cdt).is_empty());
    }

    #[test]
    fn two_collinear_points_with_constraint_is_degenerate_point_set() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0)];
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 1)]),
            Err(CdtError::DegeneratePointSet)
        );
    }

    #[test]
    fn four_collinear_points_with_constraint_is_degenerate_point_set() {
        let pts = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)];
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 3)]),
            Err(CdtError::DegeneratePointSet)
        );
    }

    #[test]
    fn shared_endpoint_constraints_are_allowed() {
        // A simple non-crossing PSLG: a triangle's 3 edges, all sharing
        // endpoints pairwise -- must not be rejected as crossing/overlapping.
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)];
        let cdt = constrained_delaunay2(&pts, &[(0, 1), (1, 2), (2, 0)]).unwrap();
        assert!(validate_cdt_topology(&cdt).is_empty());
    }

    /// The key acceptance test: force a constraint onto the edge the
    /// *unconstrained* Delaunay triangulation would NOT choose (the
    /// non-Delaunay diagonal of a convex quad), and confirm it survives
    /// -- both that it's present as an edge and that the restore-Delaunay
    /// pass didn't flip it away, which it would if the "never flip a
    /// constrained edge" rule were ignored. Determined empirically (via
    /// the crate's own `delaunay2`, not hand-derived), matching this
    /// project's "measure it" discipline instead of assuming which
    /// diagonal a hand-picked quad prefers.
    #[test]
    fn constrained_edge_survives_even_when_not_locally_delaunay() {
        let a = p(0.0, 0.0);
        let b = p(5.0, 1.0);
        let c = p(4.0, 4.0);
        let d = p(-1.0, 3.0);
        let pts = [a, b, c, d];

        let natural = delaunay2(&pts);
        assert_eq!(natural.len(), 2, "expected a convex quad -> 2 triangles");
        // The two candidate diagonals are (a,c) and (b,d) (indices 0,2 and
        // 1,3) -- whichever one is NOT already a natural Delaunay edge is
        // the non-Delaunay one; constrain that one.
        let ac_exists = find_edge_by_coords(&natural, a, c).is_some();
        let (constrained_pair, constrained_a, constrained_b) = if ac_exists {
            ((1usize, 3usize), b, d)
        } else {
            ((0usize, 2usize), a, c)
        };

        let cdt = constrained_delaunay2(&pts, &[constrained_pair]).unwrap();
        let edge = find_edge_by_coords(cdt.triangulation(), constrained_a, constrained_b)
            .expect("constrained diagonal must be present as an edge");
        assert!(
            cdt.is_constrained(edge),
            "the inserted diagonal must be marked constrained"
        );
        assert!(
            validate_cdt_topology(&cdt).is_empty(),
            "constrained-aware validator must not flag the constrained edge"
        );

        // Confirm this genuinely exercised the "would have been flipped"
        // path: the unconstrained validator (no exclusions) SHOULD flag
        // this same edge as not locally Delaunay, proving the constraint
        // exclusion is load-bearing, not vacuous.
        let unconstrained_errors = cdt.triangulation().validate_topology();
        assert!(
            unconstrained_errors
                .iter()
                .any(|e| matches!(e, TopologyError::NotLocallyDelaunay { .. })),
            "test setup didn't actually pick a non-Delaunay diagonal: {unconstrained_errors:?}"
        );
    }

    /// Two independent non-Delaunay diagonals, each needing its own flip,
    /// inserted in the same call. Guards against `crossing_edges`
    /// offering an *already-realized* constraint edge as a flip candidate
    /// while recovering a later constraint -- see `crossing_edges`' doc
    /// comment. The two quads are far apart so their local Delaunay
    /// structure can't interact, isolating this from
    /// `constrained_edge_survives_even_when_not_locally_delaunay`'s
    /// single-constraint case.
    #[test]
    fn multiple_constraints_each_needing_a_flip_all_survive() {
        let quad = |ox: f64, oy: f64| {
            [
                p(0.0 + ox, 0.0 + oy),
                p(5.0 + ox, 1.0 + oy),
                p(4.0 + ox, 4.0 + oy),
                p(-1.0 + ox, 3.0 + oy),
            ]
        };
        let qa = quad(0.0, 0.0);
        let qb = quad(100.0, 0.0);
        let pts = [qa[0], qa[1], qa[2], qa[3], qb[0], qb[1], qb[2], qb[3]];

        let natural = delaunay2(&pts);
        // For each quad, whichever diagonal is NOT already a natural
        // Delaunay edge is the one to constrain (same reasoning as
        // `constrained_edge_survives_even_when_not_locally_delaunay`).
        let pick_diagonal = |q: [Point2; 4], idx: [usize; 4]| -> ((usize, usize), Point2, Point2) {
            let ac_exists = find_edge_by_coords(&natural, q[0], q[2]).is_some();
            if ac_exists {
                ((idx[1], idx[3]), q[1], q[3])
            } else {
                ((idx[0], idx[2]), q[0], q[2])
            }
        };
        let (constraint_a, ca0, ca1) = pick_diagonal(qa, [0, 1, 2, 3]);
        let (constraint_b, cb0, cb1) = pick_diagonal(qb, [4, 5, 6, 7]);

        let cdt = constrained_delaunay2(&pts, &[constraint_a, constraint_b]).unwrap();
        let edge_a = find_edge_by_coords(cdt.triangulation(), ca0, ca1)
            .expect("quad A's constrained diagonal must be present");
        let edge_b = find_edge_by_coords(cdt.triangulation(), cb0, cb1)
            .expect("quad B's constrained diagonal must be present");
        assert!(cdt.is_constrained(edge_a), "quad A's diagonal must survive");
        assert!(cdt.is_constrained(edge_b), "quad B's diagonal must survive");
        assert!(
            validate_cdt_topology(&cdt).is_empty(),
            "constrained-aware validator must not flag either constrained edge"
        );

        // Confirm both actually needed the exclusion -- the unconstrained
        // validator must flag both, proving neither flip was a no-op and
        // neither was silently dropped as a flip candidate while
        // recovering the other constraint.
        let unconstrained_errors = cdt.triangulation().validate_topology();
        let flags_edge = |x: Point2, y: Point2| {
            let coord = |id: VertexId| {
                cdt.triangulation()
                    .vertices()
                    .find(|&(vid, _)| vid == id)
                    .unwrap()
                    .1
            };
            unconstrained_errors.iter().any(|e| match e {
                TopologyError::NotLocallyDelaunay { u, v } => {
                    let (pu, pv) = (coord(*u), coord(*v));
                    (pu == x && pv == y) || (pu == y && pv == x)
                }
                _ => false,
            })
        };
        assert!(
            flags_edge(ca0, ca1),
            "test setup didn't pick a non-Delaunay diagonal for quad A: {unconstrained_errors:?}"
        );
        assert!(
            flags_edge(cb0, cb1),
            "test setup didn't pick a non-Delaunay diagonal for quad B: {unconstrained_errors:?}"
        );
    }

    /// A constraint spanning `(p0, p2)` whose segment passes exactly
    /// through a third input vertex `p1` -- no single triangulation edge
    /// can realize it (this narrow scope doesn't auto-split it into two
    /// sub-constraints, per the module doc comment), and edges incident
    /// to `p1` never even enter the crossing set (`p1` lying exactly on
    /// the segment makes them `EndpointTouch`/`CollinearTouch`, not
    /// `Proper`) -- so the crossing-edge queue can drain to empty
    /// entirely, without `(p0, p2)` ever becoming an edge. Guards against
    /// treating "queue empty" as "constraint realized" without checking.
    #[test]
    fn constraint_through_a_collinear_third_vertex_is_rejected() {
        let p0 = p(0.0, 0.0);
        let p1 = p(2.0, 0.0);
        let p2 = p(4.0, 0.0);
        let p3 = p(2.0, 2.0);
        let p4 = p(2.0, -2.0);
        let pts = [p0, p1, p2, p3, p4];
        assert_eq!(
            constrained_delaunay2(&pts, &[(0, 2)]),
            Err(CdtError::ConstraintInsertionFailed)
        );
    }

    #[test]
    fn constraint_already_a_delaunay_edge_is_a_noop_flip() {
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)];
        let cdt = constrained_delaunay2(&pts, &[(0, 1)]).unwrap();
        let edge = find_edge_by_coords(cdt.triangulation(), pts[0], pts[1]).unwrap();
        assert!(cdt.is_constrained(edge));
    }

    #[test]
    fn deterministic_regardless_of_constraint_order() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        // Opposite hull edges: clearly non-crossing and non-collinear,
        // regardless of which order they're inserted in.
        let a = constrained_delaunay2(&pts, &[(0, 1), (2, 3)]).unwrap();
        let b = constrained_delaunay2(&pts, &[(2, 3), (0, 1)]).unwrap();
        assert_eq!(a.triangulation().triangles(), b.triangulation().triangles());
    }

    /// Measures the worst-case flip count across a spread of convex-quad
    /// and grid configurations, the same "measure the loop bound, don't
    /// just assert it's fine" discipline as Phase 5's
    /// `divide_loop_iteration_bound_is_generous`.
    #[test]
    fn flip_count_stays_well_below_the_bound() {
        reset_flip_counters();
        let mut rng = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng % 13) as f64
        };

        for _ in 0..40 {
            // A small random point grid plus every non-crossing pairwise
            // constraint we can find without violating the crossing checks.
            let mut pts = Vec::new();
            for _ in 0..8 {
                pts.push(p(next(), next()));
            }
            pts.dedup_by(|a, b| a == b);
            if pts.len() < 3 {
                continue;
            }
            // Try a handful of single-constraint insertions (skip any that
            // fail validation -- this test measures flip counts on
            // whatever succeeds, not validation logic itself).
            for i in 0..pts.len() {
                for j in (i + 1)..pts.len() {
                    if pts[i] == pts[j] {
                        continue;
                    }
                    let _ = constrained_delaunay2(&pts, &[(i, j)]);
                }
            }
        }

        let max_cdt = MAX_CDT_FLIPS.with(|c| c.get());
        let max_cdt_passes = MAX_CDT_PASSES.with(|c| c.get());
        let max_restore = MAX_RESTORE_FLIPS.with(|c| c.get());
        eprintln!(
            "cdt: measured max insertion flips = {max_cdt} (passes = {max_cdt_passes}), max restore flips = {max_restore}"
        );
        // Bound is `4 * face_count + 16`; these grids have at most ~14
        // faces, so the bound is ~72 -- require a comfortable margin below
        // it, not just "didn't hit the ceiling". `bound` limits total loop
        // *passes* (flips plus not-yet-flippable requeues), so that's the
        // quantity actually checked against it; flip count (a subset of
        // passes) is checked with the same margin for the same reason.
        assert!(
            max_cdt < 20,
            "insertion flip count {max_cdt} closer to the bound than expected"
        );
        assert!(
            max_cdt_passes < 20,
            "insertion pass count {max_cdt_passes} closer to the bound than expected"
        );
        assert!(
            max_restore < 20,
            "restore flip count {max_restore} closer to the bound than expected"
        );
    }

    /// Same measurement as `flip_count_stays_well_below_the_bound`, but
    /// for the mode that test never exercises: many constraints in a
    /// single call, forming a closed cycle around the point set --
    /// exactly `triangulate_polygon`'s own construction (`(i, (i+1) % n)`
    /// for every boundary vertex). `restore_unconstrained_delaunay` runs
    /// exactly once per call, after *all* constraints are inserted, so
    /// this is the input shape that actually stresses its flip-count
    /// margin, not the single-constraint case.
    #[test]
    fn flip_count_stays_well_below_the_bound_many_constraints() {
        reset_flip_counters();
        let mut rng = 0x1BD1_1BDA_A9FC_1A22_u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng % 13) as f64
        };

        let mut trials_run = 0u32;
        for n in [6usize, 8, 10, 12, 14] {
            for _ in 0..20 {
                let mut pts: Vec<Point2> = (0..n).map(|_| p(next(), next())).collect();
                pts.dedup_by(|a, b| a == b);
                if pts.len() < 3 {
                    continue;
                }
                // Sort by angle around the centroid so the boundary cycle
                // (i, i+1) is simple (non-self-intersecting) -- an
                // arbitrary random order would almost always self-intersect
                // and every trial would fail validation before any flip
                // ever ran, making the measurement below vacuous.
                let cx = pts.iter().map(Point2::x).sum::<f64>() / pts.len() as f64;
                let cy = pts.iter().map(Point2::y).sum::<f64>() / pts.len() as f64;
                pts.sort_by(|a, b| {
                    let angle = |p: &Point2| (p.y() - cy).atan2(p.x() - cx);
                    angle(a).total_cmp(&angle(b))
                });

                let m = pts.len();
                let constraints: Vec<(usize, usize)> = (0..m).map(|i| (i, (i + 1) % m)).collect();
                if constrained_delaunay2(&pts, &constraints).is_ok() {
                    trials_run += 1;
                }
            }
        }

        let max_cdt = MAX_CDT_FLIPS.with(|c| c.get());
        let max_cdt_passes = MAX_CDT_PASSES.with(|c| c.get());
        let max_restore = MAX_RESTORE_FLIPS.with(|c| c.get());
        eprintln!(
            "cdt (many constraints): {trials_run} trials succeeded, measured max insertion \
             flips = {max_cdt} (passes = {max_cdt_passes}), max restore flips = {max_restore}"
        );
        assert!(
            trials_run > 0,
            "every trial failed validation -- measurement below is vacuous"
        );
        // Anti-vacuousness: confirm this mode actually exercised the
        // restoration pass at least once (a fan of already-Delaunay hull
        // edges would need zero flips and this assertion would catch
        // that silently-trivial outcome).
        assert!(
            max_restore > 0,
            "restore flip count was never above zero -- this mode never exercised a real flip"
        );
        // Bound is `4 * face_count + 16`; these point sets have at most
        // ~26 faces (n=14), so the bound is ~120 -- require a comfortable
        // margin below it, matching the single-constraint test's
        // discipline. Measured (not copied from the single-constraint
        // test's `< 20`), since this multi-constraint mode plausibly needs
        // more flips.
        assert!(
            max_cdt < 40,
            "insertion flip count {max_cdt} closer to the bound than expected"
        );
        assert!(
            max_cdt_passes < 40,
            "insertion pass count {max_cdt_passes} closer to the bound than expected"
        );
        assert!(
            max_restore < 40,
            "restore flip count {max_restore} closer to the bound than expected"
        );
    }
}
