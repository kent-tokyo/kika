# ADR-009: Voronoi vertex/edge geometry (circumcenters, rays) for 0.7.0

Status: Proposed, not yet implemented/reviewed. `ROADMAP.md` (internal,
gitignored) already scopes 0.7.0 as "Voronoi geometry" with an API sketch
(`Voronoi2::vertex_point`, `VoronoiEdgeGeometry::{Segment, Ray}`) and 4
explicit design questions this ADR answers. Per this project's own
"design and implementation are separate rounds, each with its own
go-ahead" discipline (ADR-007/ADR-008 precedent), this document is a
design artifact only — no `src/` code, `Cargo.toml` change, version
bump, `push`, or release.

## Context

0.5.0 shipped Voronoi *topology* (ADR-007) deliberately without
coordinates — "topology first, not coordinates first... publishing
Voronoi vertex coordinates immediately would open a brand-new
certified-construction problem," per ADR-007's own scoping. That new
problem is what this ADR takes on: given a Voronoi vertex (one or more
cocircular-merged Delaunay faces) or a Voronoi edge (a segment between
two vertices, or a ray from one), what is its actual geometry?

Two prior precedents this design has to reconcile:

- **ADR-004's Phase 5** (`line_intersection`): the established
  "correctly-rounded construction" discipline this project uses for any
  newly-computed coordinate. Unlike `orient2d`/`incircle`/`insphere`
  (filter-first, exact-fallback-second, because they're hot per-call
  predicates), `line_intersection_raw` has no float fast path at all —
  it always builds the exact expansion directly, because a construction
  needs the exact numerator/denominator either way to get a
  correctly-rounded result; a filter would only ever be a wasted first
  pass. This design follows that shape, not the predicate shape.
- **ADR-007's "canonical, not incidental" discipline**: a merged
  cocircular group's circumcenter must be the *same* value regardless of
  which member face happens to supply it.

## The central technical problem: circumcenter as a correctly-rounded construction

Standard formula, translated to vertex `a`'s frame to eliminate one term
(a Delaunay face's 3 vertices are always non-collinear — see "`d` can be
exactly zero" below for why this still needs a runtime check, not just
an assumption):

```text
dx1 = b.x - a.x,  dy1 = b.y - a.y
dx2 = c.x - a.x,  dy2 = c.y - a.y
d   = 2 * (dx1*dy2 - dy1*dx2)        -- note: d/2 is exactly orient2d(a,b,c)'s
                                      -- own determinant, same shape, no new formula

ux = (dy2*(dx1^2+dy1^2) - dy1*(dx2^2+dy2^2)) / d
uy = (dx1*(dx2^2+dy2^2) - dx2*(dx1^2+dy1^2)) / d
circumcenter = (a.x + ux, a.y + uy)
```

**Avoiding double rounding.** Computing `ux`/`uy` as a correctly-rounded
value and then adding it to `a.x()`/`a.y()` in plain `f64` would round
*twice*. `line_intersection`'s own derivation avoids the analogous trap
by folding `A`'s coordinate into one shared numerator before dividing
("the `A*d1` terms cancel" — its own doc comment). Same move here:

```text
circumcenter.x = [a.x*d + dy2*(dx1^2+dy1^2) - dy1*(dx2^2+dy2^2)] / d
circumcenter.y = [a.y*d + dx1*(dx2^2+dy2^2) - dx2*(dx1^2+dy1^2)] / d
```

One `correctly_rounded_divide` call per coordinate, not two roundings.
`d` is a degree-2 exact expansion (`orient2d`'s own determinant shape —
reuse the same `diff_expansion`/`product_of_expansions`/`expansion_sum`
construction `line_intersection.rs`'s own `orient2d_expansion` helper
already demonstrates, not a new technique). Each numerator is degree 3
(`a.x*d` is degree 1×2; `dy2*(dx1²+dy1²)` is degree 1×2, with the
squared terms built as `product_of_expansions(dx1, dx1)` — never a
squared rounded `f64`, the same "exactness starts at the original
coordinates" lesson `docs/numerical-model.md` already generalized from
`orient2d`/`orient3d` to `incircle`/`insphere`) — the same degree as
`line_intersection`'s own numerator, built the same way.

This is structurally `line_intersection`'s construction with a
different (but same-degree) formula: degree-2 denominator, degree-3
numerator, one `correctly_rounded_divide` per coordinate. **Working
assumption, to be measured, not trusted:** the magnitude range should
resemble `line_intersection`'s wide, favorable range rather than
`incircle`'s narrower `~1e-70..1e70` (degree 4). This project has an
explicit on-record lesson about assuming the wrong direction here
(`docs/numerical-model.md`'s Phase 5 section: "the first draft of this
document assumed narrower... and was wrong") — see "Assumptions to
prove" below.

### `correctly_rounded_divide`: reuse or duplicate?

Real design question, not a rehash. `orient2d_expansion` (a ~10-line
helper) was duplicated into `line_intersection.rs` rather than exposed
from `orient2d`'s internals, judged "a few lines, not worth changing
that module's return type for" (that file's own doc comment).
`correctly_rounded_divide` is not a few lines — it's the 8-iteration
refinement loop, round-to-even tie handling, and its own dedicated test
suite (`divide_loop_iteration_bound_is_generous`, the tie-rounding
tests). Duplicating that wholesale would duplicate real, nontrivial,
already-tested logic.

**Recommendation: extract `correctly_rounded_divide` (and its
`next_up`/`next_down` helpers) into a shared location** — see "Module
placement" below for exactly where. Both `line_intersection.rs` and the
new circumcenter construction call the shared version. Zero public API
change (the function stays `pub(crate)` either way); `line_intersection`'s
own existing tests continue to pin its behavior through the new call
site unchanged. One implementation wrinkle to carry into that round: the
function's iteration-count instrumentation (`record_iters`/
`MAX_DIVIDE_ITERS`, a `#[cfg(test)]` thread-local) currently lives
inside `line_intersection.rs` itself — extraction needs it reachable
from both call sites' test modules, not just one.

### The divide-loop iteration bound must be re-measured on this construction's own numerator shape

`correctly_rounded_divide`'s 8-iteration cap is a *measured* safety net,
not a proven bound — its own doc comment: the exhausted-loop path
"returns the last `q` without re-verifying it." `line_intersection`
measured 2 iterations worst case *for its own numerator shape*
(`divide_loop_iteration_bound_is_generous`); that measurement does not
transfer automatically to a differently-shaped numerator. Circumcenter's
numerator has a cancellation mode `line_intersection`'s own sweep does
not cover: **a circumcenter near the origin with vertices far from
it** (e.g. a triangle inscribed in a circle centered near the origin,
vertices at `~1e6`) — there, `a.x*d` and the offset terms are large and
nearly cancel, exposing the plain-`f64` initial-guess seed to
catastrophic cancellation the same way `line_intersection`'s own
near-parallel-lines sweep exposed *its* seed. This needs its own
measurement pass (reusing the existing `record_iters`/`MAX_DIVIDE_ITERS`
instrumentation once extracted, with a circumcenter-specific input
generator including the origin-centered/far-vertices family above) as
part of implementation, not assumed to inherit `line_intersection`'s
"2 iterations" result.

## Determining the ray direction for unbounded edges

Unlike the circumcenter (a rounded division, genuinely inexact relative
to the input), an unbounded edge's ray *direction* needs no division:
for boundary Delaunay edge `(u, v)` with its one incident face's third
vertex `w`, the outward direction is the perpendicular to `(v - u)`,
chosen to point away from `w`:

```text
edge_dx = v.x - u.x,  edge_dy = v.y - u.y   -- each a single correctly-rounded
                                              -- f64 subtraction (not exact --
                                              -- see the caveat below), same as
                                              -- any other Vector2 built from
                                              -- two Point2s in this crate
perp = (-edge_dy, edge_dx)                   -- one of the two perpendiculars
-- orient2d(u, v, w) gives which side w is on; orient2d(u, v, u + perp) gives
-- which side perp points to; flip perp's sign if they agree, so the result
-- points to the opposite side from w
direction = if same_side(perp, w) { -perp } else { perp }
```

**Precise framing, not overclaimed:** each of `edge_dx`/`edge_dy` is a
single *correctly-rounded* `f64` subtraction of two already-finite
coordinates — not an *exact* one. `docs/numerical-model.md` documents
exactly this trap for a naive coordinate difference (`fl(2^60 - 1.0) ==
2^60`, silently discarding the `-1.0`) — the reason `orient2d_exact`/
`orient3d_exact` build their *own* differences via `diff_expansion`
rather than reusing a once-rounded `f64` subtraction. This construction
is different: it is building a `Vector2` for public consumption (not an
internal predicate input feeding further exact arithmetic), and
`Vector2`'s own doc comment already establishes the convention this
follows — "arithmetic between already-finite values does not
re-validate," i.e. a plain, single, correctly-rounded subtraction is the
crate's existing, accepted precision level for a returned `Vector2`
value. The perpendicular rotation (negate-and-swap, exact for any finite
input) and the `orient2d`-based sign selection (already exact)
introduce no *further* error beyond that one subtraction. This is
strictly better-specified than most `Vector2` construction elsewhere in
the crate, not a new weaker guarantee.

**Not normalized to unit length** — deliberate. This crate has no
`sqrt`/normalize anywhere (`Vector2` exposes no `length`/`normalize`
method at all), matching the "no epsilon, no irrational rounding"
discipline the 0.8.0 nearest-site section already commits to ("compare
squared distances exactly, no `sqrt`"). A caller wanting a unit
direction normalizes it themselves; documented on `Ray` itself. This
also sidesteps a whole separate certified-construction problem
(correctly-rounded `sqrt`) that nothing in this ADR's scope needs.

## Canonical-per-group circumcenter (ADR-007's own flagged concern, answered)

`ROADMAP.md`'s design-question list names this directly: "every face in
a cocircular-merged `VoronoiVertexId` group shares one true circumcenter
... the real risk isn't mathematical ambiguity, it's a
construction/rounding choice that isn't canonical across *which* member
face happens to supply the 3 points."

**Claim: within this construction's verified-exact magnitude range, this
risk does not exist.** All member faces of one group are, by
`voronoi2`'s own construction (ADR-007), pairwise cocircular — three
non-collinear points determine a circle uniquely, so all member faces
share exactly one true circle and hence one true circumcenter.
"Correctly rounded" means "the `f64` nearest the one true
infinite-precision value" — and that true value is the same regardless
of which face's 3 points computed it. So, *while the construction stays
within its exact magnitude range*, any two member faces produce
identical output as a direct consequence of what "correctly rounded"
means, not a convention that needs separate enforcement.

**The caveat that qualifier carries:** `docs/numerical-model.md`
documents a representability floor below which this crate's exact
arithmetic silently degrades (not solved, matching `incircle`/`insphere`/
`line_intersection` precedent). Below that floor, two different member
faces' computations are no longer guaranteed byte-identical — so the
choice of *which* face supplies the 3 points does matter in that regime,
and needs to be canonical rather than incidental, exactly the standard
ADR-007 already set for `VoronoiVertexId`/`VoronoiEdgeId` numbering
itself. **Rejected**: picking by `group_faces`' existing `FaceId::raw()`
order (`Voronoi2`'s own field doc comment calls that ordering "this
instance's own internal determinism" — precisely the kind of
incidental, construction-order-dependent key ADR-007's canonical-id
work exists to avoid). **Chosen**: the member face whose 3 vertices'
`VertexId`s, sorted, are lexicographically smallest — the same
site-identity-keyed discipline ADR-007's own canonical `VoronoiVertexId`/
`VoronoiEdgeId` assignment already uses (`group_key`, `edge_key` in
`voronoi.rs`), applied one layer deeper. This makes the pick canonical
*unconditionally* — correct by the argument above inside the exact
range, and still deterministic/reproducible (same input, same Kika
version → same output) below it, rather than silently becoming
instance-dependent exactly where it would be hardest to notice.

**Still needs a property test, not just the argument**, matching this
project's repeated "measure it, don't just derive it" discipline: for a
multi-face cocircular group (5+ points, `assemble_triangulation`-built,
same technique ADR-007's own canonical-topology tests use), compute the
circumcenter directly from *every* member face (bypassing the
canonical-pick rule) and assert byte-identical `f64` output across all
of them — the test that would actually catch a violation of the claim
above, not just exercise the "pick canonical" code path.

## `d` can be exactly zero: never trust the CCW-face invariant at this boundary

A genuine Delaunay face has `orient2d(a,b,c) != Sign::Zero` (`d` is
literally `2 * orient2d(a,b,c)`'s determinant) — but that is an
invariant `Triangulation2`'s construction is *supposed* to maintain, not
one this public method should trust blindly. ADR-008 already established
the right posture for exactly this situation: `validate_topology()` is a
test-only diagnostic (every call site is `#[cfg(test)]`), never a
construction-time gate, so a public query method must not panic if that
invariant is ever violated by a future bug. Same rule here: before
calling `correctly_rounded_divide`, check `expansion_sign(&d)`
explicitly. If `Zero`, return `Err` rather than dividing by an exact
zero. An exactly-collinear "triangle" has no defined circumcircle at
all — mathematically the same failure shape as a circumradius that
diverges to infinity (see below), so it reuses the same error variant
rather than adding a second one for what is, at the limit, the same
case.

## `VoronoiGeometryError`: why it's needed here and wasn't needed by `line_intersection`

`line_intersection` solved its magnitude ceiling entirely via
power-of-two rescaling (`docs/numerical-model.md`'s Phase 5 "Ceiling"
section) — rescaling fixes overflow caused by large *input* coordinates,
because a line intersection's true coordinate is bounded in terms of the
input coordinates' own magnitude (roughly, a weighted average of 4 input
points).

**Circumcenter does not have that property.** A triangle's circumradius
— and therefore its circumcenter's distance from its own vertices —
grows without bound as the triangle approaches degenerate (collinear),
*independent of how small the input coordinates are*. Concretely, no
extreme input magnitude is required to demonstrate this: `a = (0,0)`,
`b = (1,0)`, `c = (0.5, ε)` for a tiny `ε > 0` gives `d ≈ 2ε` and a
numerator around `-0.25`, so `uy ≈ -0.25 / (2ε)` — for `ε` near the
smallest representable positive values, this already overflows `f64`
from perfectly ordinary, unit-scale input coordinates. Whether that
specific trigger regime (`d` below roughly `1e-308` for a unit-scale
numerator) falls above or below `docs/numerical-model.md`'s documented
`~1.7e-292` exact-product-representability floor needs checking at
implementation time — if `NonFiniteCircumcenter` turns out only
reachable in a regime where this crate's exact arithmetic already
disclaims correctness, that should be stated explicitly (an honest
"best-effort past this floor" note, matching `incircle`/`insphere`/
`line_intersection`'s own documented floors), not left implied as an
unconditional guarantee. **Rescaling does not fix this**: scaling all
three points by `s` scales the true circumcenter's offset by `s` too, so
scaling the (already-overflowed) result back by `s` again just
re-overflows — the finiteness check *after* scale-back, not the rescale
itself, is what actually catches this failure mode. (Rescaling remains
worth keeping regardless, for the separate, `line_intersection`-style
large-*input*-magnitude case it does solve.)

So: `vertex_point`/`edge_geometry` must be fallible.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VoronoiGeometryError {
    /// The true circumcenter is not representable as a finite `Point2` --
    /// either the defining face is exactly collinear (no circumcircle
    /// exists at all -- see "`d` can be exactly zero" above; not expected
    /// to be reachable from a valid `Triangulation2`, but checked rather
    /// than assumed, matching ADR-008's posture on `validate_topology`),
    /// or the face is thin enough that its true (finite, well-defined)
    /// circumradius overflows `f64`'s range. Never a NaN/Infinity leaking
    /// into a `Point2`.
    NonFiniteCircumcenter,
}
```

Single variant for 0.7.0 — `#[non_exhaustive]` from the start regardless,
matching every `Result`-style error enum in this crate since 0.3.0, not
because a second variant is anticipated yet. Construction path: the same
rescale-by-power-of-two guard `line_intersection` already uses (for
genuine large-input-coordinate overflow), *plus* the `d`-is-zero check
above, *plus* an explicit finiteness check on `correctly_rounded_divide`'s
result before ever wrapping it in `Point2::new_unchecked` — never call
`new_unchecked` with a non-finite value, which would silently violate
`Point2`'s own documented invariant. The last of these three is the one
genuinely new error path relative to `line_intersection`'s own
precedent, not a copy of it.

**Not proposed**: detecting near-degeneracy ahead of time via a
magnitude/eccentricity heuristic and erroring early instead of computing
then checking finiteness. Compute-then-check is simpler, is exactly
right by construction (finite output ⟺ representable, no threshold to
get wrong), and this crate has no precedent anywhere for a heuristic
pre-check — every existing finiteness boundary (`Point2::new`,
`Vector2::new`) is a direct check on the actual value.

## Public API

```rust
impl Voronoi2 {
    /// The correctly-rounded coordinate of a Voronoi vertex (the
    /// circumcenter of its merged Delaunay face group). `Err` only for a
    /// face group so thin/near-collinear that its true circumcenter
    /// overflows `f64`'s finite range -- see `VoronoiGeometryError`.
    /// Recomputed on demand each call, not cached -- matches
    /// `neighboring_cells`/`cell_is_unbounded`'s existing "derive on
    /// demand, don't pre-index" precedent (ADR-007).
    pub fn vertex_point(&self, vertex: VoronoiVertexId) -> Result<Point2, VoronoiGeometryError>;

    /// The actual geometry of a Voronoi edge -- a bounded segment between
    /// two vertex coordinates, or a ray from one vertex coordinate in an
    /// exact-direction (not normalized) outward ray. `Err` propagates
    /// from `vertex_point` on either endpoint.
    pub fn edge_geometry(&self, edge: VoronoiEdgeId) -> Result<VoronoiEdgeGeometry, VoronoiGeometryError>;
}

#[non_exhaustive]
pub enum VoronoiEdgeGeometry {
    Segment { start: Point2, end: Point2 },
    /// `direction` is an unnormalized outward vector, not a unit vector
    /// -- see "Determining the ray direction" above for why.
    Ray { origin: Point2, direction: Vector2 },
}
```

`VoronoiEdgeGeometry` is `#[non_exhaustive]` for the same reason
`VoronoiEdgeKind` is (ADR-007): closed under "≥3 non-collinear sites"
scope, not closed by mathematical necessity — a future 1-2-site `Line`
case would need a third variant.

## Degenerate cases

| Case | Behavior |
|---|---|
| An ordinary (non-merged) Voronoi vertex | `vertex_point` computes that one face's circumcenter directly. |
| A cocircular-merged group (2+ faces) | `vertex_point` uses the canonical (lexicographically-smallest-vertex-triple) member face; provably identical output regardless of which member supplies it within the exact range, canonical-by-construction below it -- see "Canonical-per-group" above, verified by the named property test. |
| An exactly collinear face (`d` exactly zero) | `Err(NonFiniteCircumcenter)` -- not expected to be reachable from a valid `Triangulation2`, checked rather than trusted (see "`d` can be exactly zero" above). |
| A face thin enough its true circumcenter overflows `f64` | `Err(NonFiniteCircumcenter)`, never a panic, never a silently wrong large-magnitude coordinate. |
| `Bounded` edge, both endpoints computable | `Ok(Segment { start, end })`. |
| `Bounded` edge, either endpoint's circumcenter fails | `Err`, propagated -- no partial `Segment` with one real and one garbage endpoint. |
| `Unbounded` edge | `Ok(Ray { origin, direction })` -- `origin` from the one finite vertex's `vertex_point` (same failure/propagation as above); `direction` always computable whenever `origin` is (no division in its construction). |
| Empty `Voronoi2` (inherited from `Triangulation2::empty()`) | No `VoronoiVertexId`/`VoronoiEdgeId` values exist to call either method with -- vacuously no new behavior needed. |

## Rejected alternatives

- **Eager, cached circumcenters** (computed once at `voronoi2()`
  construction, stored on `Voronoi2`). Rejected: breaks the "derive on
  demand" precedent every other Voronoi query method already follows
  (ADR-007); forces every `voronoi2()` call to pay the
  correctly-rounded-division cost even for a caller who never calls
  `vertex_point` (ADR-007's own primary use case is topology-only); and
  turns a currently-infallible constructor into one that must decide
  what to do about an overflowing group *at construction time* rather
  than only if/when a caller actually asks. On-demand keeps `voronoi2()`
  infallible and defers cost/failure to the one caller who needs it. See
  "Revisit when" for when to reconsider.
- **Normalized (unit-length) ray direction.** Rejected — see
  "Determining the ray direction" above: zero `sqrt`/normalize precedent
  in this crate, and normalizing would open a whole new
  correctly-rounded-construction problem this ADR's scope doesn't need.
- **Heuristic near-degeneracy pre-check before computing.** Rejected —
  compute-then-check-finiteness is exact and simpler than any
  threshold-based heuristic, matching `Point2::new`/`Vector2::new`'s own
  direct-check convention.
- **A fallible `Point2` field added directly to `VoronoiEdge`, populated
  eagerly.** Same objection as "eager, cached circumcenters," plus it
  would change `VoronoiEdge`'s existing (already-shipped, 0.5.0) public
  field shape — a breaking change for something deliverable additively.
- **Picking the cocircular-group representative by `FaceId::raw()`/scan
  order.** Rejected — see "Canonical-per-group" above: incidental,
  instance-dependent below the exact-arithmetic floor, the exact
  failure mode ADR-007's own canonical-id work exists to prevent.
- **Rounding `ux`/`uy` relative to vertex `a`, then adding to `a`'s
  coordinate in plain `f64`.** Rejected — double rounding, the same trap
  `line_intersection`'s own derivation avoided. See "Avoiding double
  rounding" above.

## Compatibility risk

Additive only: one new fallible method each on `Voronoi2` (`vertex_point`,
`edge_geometry`), one new `#[non_exhaustive]` error enum, one new
`#[non_exhaustive]` geometry enum. No change to any 0.5.0/0.6.0 public
item (`Voronoi2`, `VoronoiEdge`, `VoronoiEdgeKind`, the three Voronoi id
types, `voronoi2`, or any existing query method's signature). If
`correctly_rounded_divide` is extracted to a shared location (see
above), that is a private, `pub(crate)`-only move — zero public API
surface change.

## Assumptions to prove or test before implementation

1. **Magnitude range resembles `line_intersection`'s (wide), not
   `incircle`'s (narrow)** — the expected direction given the shared
   degree-3-numerator/degree-2-denominator shape, but this project has
   an explicit on-record lesson about assuming the wrong direction here
   — must be measured via an independent oracle sweep, not assumed.
2. **The divide-loop iteration bound holds for circumcenter's own
   numerator shape**, specifically the origin-centered-circumcenter/
   far-vertices cancellation family named above — measured via the
   existing (to-be-relocated) `record_iters`/`MAX_DIVIDE_ITERS`
   instrumentation, not inherited from `line_intersection`'s unrelated
   "2 iterations" result.
3. **Canonical-per-group circumcenter identity** — the byte-identical-
   output property test described above, not just the argument.
4. **`correctly_rounded_divide` extraction is behavior-preserving** —
   `line_intersection`'s full existing test suite must still pass
   unchanged after the function moves.
5. **Ray direction sign is correct** (points away from the triangle's
   third vertex) — a concrete hand-traced fixture (a simple 3-point
   triangle's 3 boundary edges) before trusting the general
   `orient2d`-based sign rule.
6. **The overflow path is actually reachable and returns `Err`, not a
   panic or a silently-huge finite value** — the concrete `a=(0,0)`,
   `b=(1,0)`, `c=(0.5, ε)` fixture above (or a variant confirmed to
   overflow), and a determination of whether that regime sits above or
   below the `~1.7e-292` representability floor (see "why it's needed"
   above) — stated explicitly either way, not left ambiguous.

## Acceptance tests (0.7.0, for the implementation round)

- A simple non-cocircular triangle → `vertex_point` matches a
  hand-computed circumcenter exactly (e.g. an isoceles right triangle
  with a "nice" circumcenter).
- Independent-oracle differential test (matching `line_intersection`/
  `locate`'s own precedent): a `BigRational` circumcenter oracle,
  comparing the candidate `f64` against both representable neighbors
  (round-to-nearest-even definition), across magnitude scales,
  mixed-magnitude inputs, and a floor/ceiling sweep — same shape as
  `tests/differential/line_intersection.rs`.
- Cocircular-merged group (4+ points, 5+ points) → `vertex_point`
  byte-identical regardless of which member face computed it (the
  property test named above).
- The exactly-collinear-face case and the near-degenerate-overflow case
  → both `Err(NonFiniteCircumcenter)`, never a panic (assumptions 6
  above).
- `Bounded` edge → `Segment` whose `start`/`end` match the two incident
  vertices' own `vertex_point` output exactly.
- `Unbounded` edge → `Ray` whose `direction`, tested against the vector
  toward the excluded third vertex via `orient2d`, is confirmed to point
  away — an exact predicate-based check, not a floating comparison.
- Regression guard: `voronoi2()` construction itself remains infallible
  and its existing 0.5.0/0.6.0 test suite is unaffected — this ADR adds
  new methods, it does not touch `voronoi2`'s own construction path.

### Documentation to update (implementation round, not this design round)

Matching this project's established per-feature documentation pattern
(see e.g. `docs/degeneracy-policy.md`'s point-location section, commit
`30ed670`): a Voronoi-geometry row set in `docs/degeneracy-policy.md`
(covering the degenerate-case table above), and a new phase section in
`docs/numerical-model.md` recording the *measured* (not assumed)
magnitude range and divide-loop iteration bound, matching its existing
one-section-per-construction-phase structure.

## Module placement

**Recommendation: `src/predicates/constructions/circumcenter.rs`**,
sibling to `line_intersection.rs`, exposing `pub(crate) fn
circumcenter(a: Point2, b: Point2, c: Point2) -> Result<Point2,
VoronoiGeometryError>`. A triangle circumcenter is a general
correctly-rounded coordinate builder — exactly what that directory
already holds — not something intrinsically Voronoi-specific: 0.8.0's
exact nearest-site query and 0.9.0's arrangement kernel are both
plausible future callers of the same primitive, and placing it in
`triangulation::voronoi` would make a later reuse either an awkward
cross-module dependency (`predicates` importing from `triangulation`,
backwards from every existing dependency direction in this crate) or a
duplicate implementation. `Voronoi2::vertex_point`/`edge_geometry`
(the public, Voronoi-specific parts — id resolution, group-representative
selection, ray direction/sign) stay in `src/triangulation/voronoi.rs`
(or a new sibling `voronoi_geometry.rs`, an implementation-round call)
and call the shared `circumcenter` construction. If
`correctly_rounded_divide` is extracted (see above), its natural new
home is this same `circumcenter.rs` file or `src/predicates/expansion.rs`
directly — either keeps it inside the `predicates` module tree, not
`triangulation`.

## Explicitly out of scope for 0.7.0

Everything `ROADMAP.md`'s own 0.7.0 section already excludes: AABB/
polygon clipping, weighted Voronoi, Lloyd relaxation, nearest-neighbor
query, new runtime dependencies, large-scale benchmarks. Also out of
scope for this specific design round: writing `src/` code, `Cargo.toml`
changes, a version bump, `push`, or a release — matching every prior
ADR's own same-round scope limit (ADR-007/ADR-008 precedent).

## Revisit when

- **The `correctly_rounded_divide` extraction turns out to need more
  than a mechanical move** (e.g. if `line_intersection`'s existing
  magnitude-rescaling wrapper and this construction's rescaling need
  genuinely different thresholds) — re-examine whether sharing is still
  right versus accepting the duplication after all.
- **A caller needs a normalized ray direction.** Add a separate,
  explicitly-named correctly-rounded-normalize construction then (its
  own certified-construction problem, ADR-004-style) — not retrofitted
  onto `Ray.direction`, which stays exact-difference/unnormalized.
- **Clipping (a later round) needs circumcenters at scale/eagerly.** If
  profiling ever shows on-demand `vertex_point` recomputation is a real
  bottleneck for a caller walking every vertex of a large diagram,
  revisit "eager, cached circumcenters" above — not assumed a problem
  now, per AGENTS.md §13's "measure before optimizing" discipline.
- **0.8.0's nearest-site query or 0.9.0's arrangement kernel need a
  triangle circumcenter.** Confirms (or refutes) the "shared `predicates::
  constructions::circumcenter`" placement call above — if it turns out
  nothing else ever calls it, that is fine (no harm from the placement
  either way), but worth noting whether the anticipated reuse actually
  materialized.
