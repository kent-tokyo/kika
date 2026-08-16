//! Entry point for `tests/differential/*`, per AGENTS.md §6's required
//! directory layout. Cargo only auto-discovers `tests/*.rs` files, not
//! nested ones, so this just declares the submodules.

#[path = "differential/incircle.rs"]
mod incircle;
#[path = "differential/insphere.rs"]
mod insphere;
#[path = "differential/orient2d.rs"]
mod orient2d;
#[path = "differential/orient3d.rs"]
mod orient3d;
#[path = "differential/point_in_triangle.rs"]
mod point_in_triangle;
#[path = "differential/point_on_segment.rs"]
mod point_on_segment;
#[path = "differential/segment_intersection.rs"]
mod segment_intersection;
