//! The various font tables

pub mod aat;
pub mod ankr;
pub mod avar;
pub mod base;
pub mod bitmap;
pub mod cbdt;
pub mod cblc;
pub mod cff;
pub mod cff2;
pub mod cmap;
pub mod colr;
pub mod cpal;
pub mod cvar;
pub mod dsig;
pub mod ebdt;
pub mod eblc;
pub mod feat;
pub mod fvar;
pub mod gasp;
pub mod gdef;
pub mod glyf;
pub mod gpos;
pub mod gsub;
pub mod gvar;
pub mod hdmx;
pub mod head;
pub mod hhea;
pub mod hmtx;
pub mod hvar;
pub mod kern;
pub mod kerx;
pub mod layout;
pub mod loca;
pub mod ltag;
pub mod maxp;
pub mod meta;
pub mod morx;
pub mod mvar;
pub mod name;
pub mod os2;
pub mod post;
pub mod sbix;
pub mod stat;
pub mod svg;
pub mod trak;
pub mod varc;
pub mod variations;
pub mod vhea;
pub mod vmtx;
pub mod vorg;
pub mod vvar;

#[cfg(feature = "ift")]
pub mod ift;

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the published `compute_checksum` function returns `u32` directly in the existing parser interface"
  )
)]
pub fn compute_checksum(table: &[u8]) -> u32 {
  let Ok(sum) = console_core_checksums::opentype::of(table);
  sum
}

#[cfg(feature = "hosted-compute")]
mod compute;
#[cfg(feature = "hosted-compute")]
pub use compute::{checksum_glsl, compute_checksum_with_compute};
