//! Fixed-point scale and pixels per em for an interpreter instance.

use read_fonts::types::Fixed;

#[derive(Clone, Copy, Debug)]
pub struct Scale {
  pub factor: Fixed,
  pub ppem: i32,
}
