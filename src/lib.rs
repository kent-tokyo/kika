//! Kika — robust computational geometry for Rust.
//!
//! Pre-alpha; see the repository README and `docs/` for scope and status.

mod error;
mod predicates;
mod primitives;

pub use error::KikaError;
pub use predicates::{Orientation, Sign, incircle, orient2d, orient3d};
pub use primitives::{Point2, Point3};
