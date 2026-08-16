# Kika

**Kika — Robust Computational Geometry for Rust.** A pure-Rust,
memory-safe alternative to [CGAL](https://www.cgal.org/) for developers who
want robust geometric predicates without CMake, Boost, or a hard GMP/MPFR
dependency.

Kika ("幾何", Japanese for "geometry") is a Rust library building toward
robust 2D/3D computational geometry: exact predicates with adaptive/exact
fallback arithmetic, and — in later phases — triangulation, hull, and
polygon algorithms built on top of that foundation.

Status: **pre-alpha (Phase 1 in progress).** No stability guarantees. See
[Roadmap](#roadmap) for what does not exist yet — **Kika is not a CGAL
replacement yet**, it is the robust kernel a future one would be built on.

## Why not just use CGAL?

CGAL is the mature, comprehensive reference implementation for
computational geometry, and Kika's predicate layer is tested against it as
an external oracle during development (see
[`docs/compatibility.md`](docs/compatibility.md)). But pulling CGAL into a
Rust project means a C++ toolchain, CMake, Boost, and usually GMP/MPFR —
friction that's real for `cargo build`-only workflows, WASM targets, and
teams that want a pure-Rust dependency tree.

Kika's bet, in order:

| | CGAL | Kika |
|---|---|---|
| Language | C++ | Pure Rust |
| Build | CMake + Boost | `cargo build` |
| Big-number dependency | GMP/MPFR (typically required) | Not required at runtime (dev-only, for test oracles) |
| Memory safety | Manual | Enforced by the compiler (`unsafe` disallowed by default, see `docs/architecture.md`) |
| WASM target | Not practical | `wasm32-unknown-unknown`, CI-checked |
| License | GPL / commercial | MIT OR Apache-2.0 |
| Feature breadth today | Very large, decades mature | Deliberately small — see [Implemented today](#implemented-today) |

If you need mesh Booleans, NURBS, or a CAD kernel today, that's CGAL, not
Kika. If you want a small, robust, panic-free predicate layer to build on
in pure Rust, that's what Phase 1 of Kika is.

## Implemented today

* `Point2`, `Point3` — finite-coordinate points (`f64`). Construction
  validates and rejects NaN/infinity; once constructed, always finite.
* `Sign`, `Orientation` — meaningful enums returned by predicates (never a
  raw determinant, never an ambiguous `bool`).
* `orient2d`, `orient3d`, `incircle`, `insphere` — exact-sign geometric
  predicates. Each uses a fast floating-point filter with a *computed*
  error bound (never a fixed epsilon) and falls back to exact
  expansion-arithmetic evaluation when the filter is inconclusive. See
  [`docs/numerical-model.md`](docs/numerical-model.md).

## Exact predicates vs. exact constructions

Kika's predicates (`orient2d` etc.) guarantee a mathematically correct
**sign**. They do not, by themselves, guarantee that a *generated
coordinate* (e.g. a future segment-intersection point) is exact — that is a
separate problem ("construction"), not yet addressed. See
[`docs/architecture.md`](docs/architecture.md) §4.2 and ADR-004. Do not
assume constructions implemented in later phases carry the same exactness
guarantee as today's predicates until their own docs say so.

## Degenerate cases

Collinear/coplanar/cocircular/cospherical points, duplicate points, signed
zero, and subnormal coordinates are handled explicitly and tested, not
treated as "rare enough to ignore." See
[`docs/degeneracy-policy.md`](docs/degeneracy-policy.md).

## Minimal example

```rust
use kika::{Point2, orient2d, Orientation};

let a = Point2::new(0.0, 0.0).unwrap();
let b = Point2::new(1.0, 0.0).unwrap();
let c = Point2::new(0.0, 1.0).unwrap();

assert_eq!(orient2d(a, b, c), Orientation::CounterClockwise);
```

## WASM

The predicate core has no OS or platform-specific code and builds for
`wasm32-unknown-unknown`; this is checked in CI. No WASM-specific bindings
(`wasm-bindgen` etc.) exist yet.

## Difference from CGAL

Kika does not link CGAL and shares no source with it. CGAL is used only as
an external, separate differential-test oracle during development (§10 of
the project's development instructions) — never as a runtime or build
dependency of the `kika` crate.

## Stability

Pre-1.0, no semver guarantees. The public `Kernel` trait design described
in some computational-geometry libraries (CGAL included) is explicitly not
being finalized yet — see ADR-004.

## License

Licensed under either of

* MIT license ([LICENSE-MIT](LICENSE-MIT))
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

## Roadmap

Not yet implemented: 2D primitives beyond `Point2`/`Point3` (vectors,
segments, triangles, AABBs), segment intersection, polygon
validity/orientation, convex hull, Delaunay triangulation, exact
constructions, constrained Delaunay, polygon/mesh Boolean, mesh repair,
surface reconstruction, point-cloud processing. See
[`tasks/todo.md`](tasks/todo.md) for the phased backlog.
