# ADR-006: Triangulation adjacency structure for Phase 6

Status: Decided and implemented for Phase 6A/6B (indexed triangle
adjacency, as recommended below). Phase 6B added `VertexId`/`EdgeId`/
`FaceId` and the 9 topology-query methods plus an internal
`validate_topology` — see `src/triangulation/delaunay2.rs` and
`src/triangulation/ids.rs`. Phase 6C (constrained Delaunay, using this
structure) and Phase 6b's overlay structure (still open, see below) are
not implemented yet.

## Context

`Triangulation2` (Phase 4) is deliberately a flat `Vec<Triangle2>` with no
adjacency exposed — see its doc comment: "deferred until a consumer
actually needs neighbor queries." AGENTS.md's own Phase 4 acceptance
criteria already listed triangle adjacency, boundary-edge retrieval, and
topology validation as requirements; Phase 4 shipped without them as a
documented, deliberate deferral (Yellow: implementable-later, not a
correctness gap), not an oversight. Phase 6 (constrained Delaunay + polygon
Boolean) is the consumer that needs them, so this ADR resolves the
deferral now, per AGENTS.md's Phase 5 section's own precedent for this
project: decide a structural question when the real need is concretely
known, not speculatively (the same reasoning ADR-004 used, and used again
in its own Phase 5 decision to defer re-opening past Phase 5 into "when
Phase 6 reveals its needs" — see that ADR's "Revisit when").

**Internal representation today** (`src/triangulation/delaunay2.rs`):
`tris: Vec<[usize; 3]>` — three vertex indices per triangle, into the
deduplicated, canonically-sorted point slice, plus the symbolic `GHOST`
sentinel index for the point-at-infinity. No neighbor links exist. Each
insertion (`insert_point`) does a full linear scan of every current
triangle to find the "bad" (circumcircle-violating) set, builds the cavity
boundary via directed-edge cancellation through a `HashSet`, then
`swap_remove`s bad triangles (which reorders the vector — **triangle
indices are not stable across insertions today**, and never have been;
this is purely internal, no public API exposes them). This confirms the
migration below has no existing public index-stability contract to
preserve.

## What Phase 6 actually needs

From AGENTS.md's Phase 4 and Phase 6 sections, translated into concrete
operations:

* Triangle → its (up to 3) neighboring triangles, one per edge.
* Edge → its 1 (boundary) or 2 (interior) incident triangle(s).
* Boundary-edge enumeration (edges with exactly 1 incident triangle).
* Edge flip — the core primitive for both routine Delaunay maintenance and
  constrained-edge recovery (inserting a required segment that Delaunay
  would not naturally produce, via repeated flipping along the
  segment's crossing path — the standard Anglin/Sloan-style approach).
* Constraint-edge marking: an edge that must never be flipped away once
  inserted, needed for CDT boundaries and polygon Boolean's ring edges.
* Vertex-star traversal (all triangles/edges around a vertex) — needed for
  insertion, deletion, point location, and local overlay rewiring.
* Safe handling of deleted/replaced triangles under heavy incremental
  mutation (CDT and Boolean mutate far more aggressively than Bowyer-Watson
  insertion alone).
* Some notion of stable identity for a triangle/edge across mutations, if
  any algorithm needs to hold a reference to one across multiple steps
  (point location caching, incremental flip queues).
* Topology validation as a reusable, testable check: Euler characteristic,
  manifold edge-incidence (every edge used by exactly 1 or 2 triangles).
  This session's `fuzz/fuzz_targets/triangulation_topology_validator.rs`
  already prototypes exactly this check ad hoc against the current flat
  list — see "Migration" below for promoting it to real internal API.
* Hole handling, ring boundaries, and n-gon overlay faces (polygon
  Boolean's union/intersection/difference/xor) — **not** part of CDT
  itself; see "Two different structural needs" below.

## Candidates compared

### A. Indexed triangle adjacency

Each triangle stores 3 vertex indices plus 3 neighbor-triangle references
(one opposite each vertex/edge, `None` at the boundary). This is the
representation used by Shewchuk's `Triangle` and most incremental/CDT
literature (de Berg et al.).

* **Fit with current code**: closest to what already exists — `insert_point`
  already computes, and throws away, exactly this adjacency information
  every insertion (the cavity-boundary edge-cancellation step *is* a
  neighbor computation, just not retained). Migrating is additive to the
  existing algorithm, not a rewrite.
  * **Concrete side benefit, not just a structural nicety**: today's
    "bad" scan is `O(triangle count)` per insertion (a full linear scan,
    no point location), an `O(n²)` cost for `n` insertions once summed.
    Persistent neighbor links enable the standard optimization — walk from
    a point-located seed triangle and flood-fill outward via neighbor
    links to find only the local bad region — turning that scan
    near-local instead of global. Not implemented by this ADR (design
    only), but worth recording as motivation beyond Phase 6: this is
    exactly the kind of finding the upcoming benchmarking pass (§13,
    `tasks/todo.md`) should quantify before deciding whether it's worth
    doing.
* **Edge flip**: swap two adjacent triangles' vertex triples, fix up the 4
  affected neighbor links. Simple, local, `O(1)`.
* **Constraint edges**: a 3-bit (one per local edge) flag per triangle, or
  a separate `HashSet` of constrained undirected vertex-index pairs
  checked before any flip. Either is straightforward.
* **Boundary/hole loops**: none — every face is a triangle, by
  construction. This is the limitation that matters for the overlay half
  of Phase 6 — see below.

### B. Half-edge (DCEL)

Each half-edge stores an origin vertex, its twin, the next half-edge
around its face (CCW), and its face; faces and vertices each store one
incident half-edge as an entry point.

* **Fit with current code**: a genuine rewrite of the working,
  fuzz-and-property-tested Bowyer-Watson core — not an additive change.
* **Edge flip / constraint marking**: comparable complexity to (A) — relink
  6 half-edges instead of 4 neighbor slots; a flag on the (twin-shared)
  edge record.
* **Boundary/hole loops**: this is half-edge's actual advantage. `.next`
  traversal walks a face boundary of *any* length, and a face can carry a
  separate inner-loop half-edge per hole — exactly the shape polygon
  Boolean's overlay step produces (merged, non-triangular result faces
  with holes), and the textbook approach for implementing overlay
  (Weiler-Atherton-style DCEL merge, per de Berg et al.) is built directly
  on this structure.
* **Cost**: more index/pointer fields per element (twin, next, vertex,
  face — vs. 3 neighbor slots), more ways for a mutation to leave the
  structure in a locally-inconsistent state, general-purpose machinery
  triangulation itself (CDT) never needs (every CDT face is a triangle;
  only the *overlay* step needs non-triangular faces).

### C. Quad-edge

Guibas-Stolfi's structure (the same paper this crate's own ghost-vertex
Delaunay design already draws from): each edge is 4 linked directed
records (`rot`/`sym`/`onext`), giving simultaneous access to a
subdivision and its dual.

* **Fit**: elegant specifically when an algorithm needs the Delaunay
  triangulation *and* its dual Voronoi diagram at once, or wants primal/dual
  symmetry for divide-and-conquer merge steps.
* **Rejected**: Kika has no stated or planned need for a Voronoi diagram —
  it is not on Phase 6's list (constrained Delaunay, polygon Boolean; see
  AGENTS.md) and not mentioned anywhere else as future scope. Building the
  general dual-supporting machinery to serve a feature the crate does not
  have would be exactly the kind of speculative generality this project's
  own design principle rejects (AGENTS.md §9 Phase 0: don't over-fix the
  design before it's needed — the same principle ADR-004 invoked to defer
  itself past Phase 1, and Phase 2's own scope note about not skipping
  ahead of it). It also has no native "face boundary loop with holes"
  concept, so it would not even solve the overlay half of Phase 6 better
  than half-edge does. Ruled out on both counts, not just one.

## Two different structural needs, not one

CDT's output is still a pure triangulation — every face is a triangle,
holes are typically handled by triangulating the full domain and then
tagging/discarding triangles that fall inside a hole region, constraint
edges are just triangulation edges marked "must survive." Indexed triangle
adjacency is sufficient and is the smaller, more additive change.

Polygon Boolean's overlay step (union/intersection/difference/xor,
multipolygon, ring orientation normalization) genuinely produces
non-triangular result faces with holes — the textbook structure for that
is half-edge/DCEL. But *how* overlay is actually implemented on top of a
CDT is still open: a classical DCEL-merge is one option, but
triangulation-based Boolean via point-location + inside/outside triangle
tagging + boundary re-extraction (avoiding ever materializing a full
half-edge structure) is a documented alternative in the literature and has
not been ruled out here.

## Decision

1. **For CDT (Phase 6a): extend `Triangulation2`'s internal representation
   to indexed triangle adjacency now** — the smaller, additive,
   backward-compatible change, sufficient for everything CDT itself needs
   (flip, constraint marking, boundary edges, topology validation), and
   the one that also closes Phase 4's originally-deferred acceptance
   criteria at the same time.
2. **For polygon Boolean/overlay (Phase 6b): do not decide the structure
   now.** Revisit in a follow-up ADR once CDT is actually implemented and
   its concrete output shape (and whether classical DCEL-merge or
   triangulation-tagging turns out simpler to build on top of it) is
   known — the same "decide when the need is concretely known" discipline
   this whole ADR opened with, applied one level deeper rather than
   abandoned at the first sub-decision.
3. **Quad-edge is rejected outright**, for both sub-phases, per the
   reasoning above — this is decided, not deferred, since no future
   Voronoi-diagram scope exists to revisit it against.

## ID stability

Current indices are not stable (`swap_remove` reorders on every deletion)
and nothing public depends on that today. CDT/Boolean's heavier mutation
needs stability for internal bookkeeping (flip queues, point-location
caches, neighbor back-references). Recommended: a small internal
generational slot arena — `Vec<Slot<TriangleRecord>>` where a slot is
either `Occupied(TriangleRecord, generation)` or `Tombstone(generation)`
with a free list for reuse; a triangle's "id" is `(slot_index,
generation)`, comparable to the well-known `slotmap`/`generational-arena`
crate pattern but implemented as a ~30-line internal module rather than a
new dependency (consistent with the zero-runtime-deps mandate — see
ADR-005; this is exactly the kind of small, self-contained utility that
does not need an external crate). Deletion tombstones a slot and bumps its
generation, so a stale `(index, generation)` handle from before the
deletion is detectably invalid rather than silently aliasing a
later-reused slot with unrelated data.

**Scope actually implemented for Phase 6B (below): this arena, not
needed.** `Triangulation2` gained the query API and adjacency as a
**static, post-construction snapshot** — `delaunay2`'s own
`insert_point`/cavity-construction internals were left untouched (still
the `swap_remove`-based `Vec<[usize; 3]>` from Phase 4). Since a completed
`Triangulation2` is immutable (no public mutation API exists), plain
indices into fixed-size parallel arrays are sufficient and correct for
`VertexId`/`EdgeId`/`FaceId` — the generational arena above remains a
recommendation for *if and when* Bowyer-Watson's own construction-time
loop is refactored to build adjacency incrementally (e.g. for the
point-location optimization noted under "indexed triangle adjacency"
above), not something Phase 6B needed or built.

## Migration plan

No public API break at any step (new public methods are additive; the
existing `triangles()`/`len()`/`is_empty()` keep their exact signatures
and behavior). **Implemented for Phase 6B** — this plan is now a record of
what happened, not a proposal:

1. ~~Replace `tris: Vec<[usize; 3]>` with the slot-based
   `Vec<Slot<TriangleRecord>>`~~ — not needed; see "ID stability" above.
   Instead, `build_topology` (in `delaunay2.rs`) derives the adjacency
   structure once, after Bowyer-Watson's insertion loop finishes, from the
   already-computed final `Vec<[usize; 3]>` — plain parallel arrays
   (`faces`, `face_neighbors`, `edges`, `edge_faces`), no slot arena.
2. `Triangulation2::triangles() -> &[Triangle2]` **stays a stored field**,
   not a computed view — kept deliberately redundant with the new
   index-based `faces` field (both describe the same triangles) rather
   than changing the return type, since `&[Triangle2]` cannot be
   lazily computed without breaking the signature. See the struct's own
   doc comment for why this redundancy is intentional.
3. Added `pub` + `#[doc(hidden)]` (not `pub(crate)` — Rust's crate-privacy
   rules make `pub(crate)` invisible to this repository's own `tests/`
   integration suite and `fuzz/` targets, which are separate crates
   despite living alongside `kika`; `#[doc(hidden)]` keeps it off
   downstream users' radar without blocking the crate's own test/fuzz
   coverage) `validate_topology(&self) -> Vec<TopologyError>` — broader
   than originally sketched: also checks adjacency reciprocity and
   per-edge local-Delaunay, not just edge-count and Euler's formula.
   `fuzz/fuzz_targets/triangulation_topology_validator.rs` and
   `tests/differential/delaunay2.rs` were both simplified to call it
   directly, collapsing what used to be independent ad hoc
   reimplementations of the same checks.
4. `vertices`, `edges`, `faces`, `edge_vertices`, `adjacent_faces`,
   `face_vertices`, `neighboring_faces`, `boundary_edges` added as real
   `pub` methods (not `pub(crate)`) — Phase 6C is expected to need them
   from outside this module (a future `constrained_delaunay2` function),
   and they're read-only queries over an already-immutable structure, not
   a speculative mutation API. `flip`/`mark_constrained`/`is_constrained`
   were **not** added — those need Phase 6C's actual constraint-recovery
   algorithm to shape their signatures correctly, exactly as originally
   planned; building them now would be the same speculative-generality
   mistake this ADR argues against elsewhere.

## Revisit when

Phase 6b (polygon Boolean/overlay) implementation begins and reveals
whether DCEL-merge or triangulation-tagging is the better fit on top of
whatever CDT actually produces — re-open this ADR (or open a follow-up)
at that point, not before.
