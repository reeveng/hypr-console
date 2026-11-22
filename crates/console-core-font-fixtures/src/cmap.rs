//! cmap test data for scenarios not readily produced with ttx

use crate::{be_buffer, bebuffer::BeBuffer};

pub fn repetitive_cmap4() -> Result<BeBuffer, crate::bebuffer::BufferError> {
  Ok(be_buffer! {
    4_u16,
    0_u16,
    0_u16,
    4_u16,
    0_u16,
    0_u16,
    0_u16,

    64_u16,
    64_u16,

    0_u16,


    6_u16,
    6_u16,


    0_u16,
    0_u16,


    0_u16,
    0_u16


  })
}
