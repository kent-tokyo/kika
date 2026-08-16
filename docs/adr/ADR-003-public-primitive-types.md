# ADR-003: Public primitive types

Status: Accepted (Phase 1 subset landed; Phase 2 remainder — see
"Phase 2: point equality policy" below — now also accepted)

## Context

AGENTS.md §5 lists the required v0.1 primitive types. AGENTS.md §7.1
requires choosing one of: (a) a validated finite-coordinate type, or (b)
public functions that return `Result`. §9 Phase 1 scopes implementation to
predicates only; the rest of §5's primitives belong to Phase 2.

## Decision

**Validated finite-coordinate type**, not `Result`-returning predicate
functions.

* `Point2` / `Point3` store `f64` coordinates and can only be constructed
  through `Point2::new(x, y) -> Result<Point2, KikaError>` /
  `Point3::new(x, y, z) -> Result<Point3, KikaError>`, which reject NaN and
  infinite coordinates. Once constructed, a `Point2`/`Point3` is guaranteed
  finite for its lifetime (fields are private; there is no path to mutate
  them into a non-finite state).
* Predicates (`orient2d`, `orient3d`, `incircle`, `insphere`) take
  `Point2`/`Point3` by value (they are `Copy`, two `f64`s or three `f64`s)
  and therefore never need to validate their inputs or return `Result`.
  They return `Orientation` / `Sign` directly and cannot panic.

This pushes the finiteness boundary check to one place (construction)
instead of repeating it in every function that consumes points, matching
AGENTS.md §7.1's "内部実装では、有限値であることを保証した後に高速な
unchecked pathを使用できます".

## Phase 1 scope

Only `Point2` and `Point3` are implemented in Phase 1, because only they are
needed by the four predicates. `Vector2`, `Vector3`, `Segment2`, `Triangle2`,
`Triangle3`, `Aabb2`, `Aabb3` are deferred to Phase 2 (§9 explicitly scopes
Phase 1 to "Robust Predicates" and Phase 2 to "2D Primitives and
Intersections") and are not stubbed out ahead of need.

## Consequences

* `Point2::new`/`Point3::new` are the only fallible entry points in the
  Phase 1 API surface; everything downstream is panic-free and
  `Result`-free.

## Phase 2: point equality policy

**Exact coordinate equality, via derived `PartialEq`** — `a == b` iff
every coordinate compares equal under IEEE-754 `f64` equality
(`a.x() == b.x() && a.y() == b.y()`, etc.).

No tolerance/epsilon-based "coincident point" comparison is provided at
this layer, for the same reason AGENTS.md §4.1 bans fixed epsilons in
predicates: there is no scale-invariant threshold that is correct for
every caller, and baking one into `Point2`/`Point3` would silently
reintroduce exactly the problem the predicate layer exists to avoid.
Algorithms that need a tolerant "are these effectively the same point"
notion (e.g. Delaunay's duplicate-input-point handling, Phase 4) define
their own policy on top of this exact equality, explicitly, at the
algorithm level — not here.

Consequences of "exact" specifically:

* `-0.0 == 0.0` (IEEE-754's own rule, inherited via `f64`'s `PartialEq`)
  — consistent with how the predicates already treat signed zero (Phase
  1: `Sign::of` maps both to `Sign::Zero`).
* `Eq` is not derived — `f64` only implements `PartialEq` (NaN breaks
  reflexivity at the type-system level), even though `Point2`/`Point3`'s
  finite-only invariant makes this particular `PartialEq` total in
  practice. Not worth an unsafe/manual `Eq` impl for that technicality
  before something actually needs `Point2` as e.g. a `HashMap` key.
* `PartialEq` was in fact already derived on `Point2`/`Point3` during
  Phase 1 (for test ergonomics) before this ADR section existed; this
  formalizes that as the deliberate Phase 2 decision rather than an
  accidental one, and is the reason this ADR's status line now marks the
  Phase 2 item accepted too.
