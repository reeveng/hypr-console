//! PostScript fonts.

#[cfg(feature = "agl")]
pub mod agl;
pub mod cff;
pub mod charmap;
pub mod cs;
pub mod encoding;
pub mod error;
pub mod hinting;
mod num;
pub mod string;
pub mod transform;
pub mod type1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pitch {
  Fixed,
  #[default]
  Proportional,
}
