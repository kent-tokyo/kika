//! Kika — robust computational geometry for Rust.
//!
//! Pre-alpha; see the repository README and `docs/` for scope and status.

mod error;
mod predicates;
mod primitives;

pub use error::KikaError;
pub use predicates::{Orientation, Sign, incircle, insphere, orient2d, orient3d};
pub use primitives::{
    Aabb2, Aabb3, Point2, Point3, Segment2, Triangle2, Triangle3, Vector2, Vector3,
};
