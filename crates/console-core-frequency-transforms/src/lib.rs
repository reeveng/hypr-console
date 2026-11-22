//! The inverse modified discrete cosine transform for blocks of decoded audio.
//!
//! AAC and Vorbis share it while each decoder owns its block sequencing,
//! scaling, and overlap state. A transform is made once for a size, holds the
//! buffers it works in, and is asked again for every block, so asking
//! allocates nothing. It is a complex FFT of half the size with a twiddle
//! before and after, and the FFT is radix-2 over unrolled kernels for the
//! smallest sizes, which is why it stays private: nothing asks for one on its
//! own.
//!
//! The scalar transform is the only one. The SIMD one this began with was
//! behind features nothing here turned on, and `rustfft` with it.

#![forbid(unsafe_code)]

mod complex;
mod fft;
pub mod mdct;

use core::fmt;

pub use mdct::Imdct;

enum TransformSize {
  Supported,
  Unsupported,
}

impl From<bool> for TransformSize {
  fn from(supported: bool) -> Self {
    match bool::then_some(supported, ()) {
      Some(()) => Self::Supported,
      None => Self::Unsupported,
    }
  }
}

enum BufferLength {
  Exact,
  Mismatched,
}

impl From<bool> for BufferLength {
  fn from(exact: bool) -> Self {
    match bool::then_some(exact, ()) {
      Some(()) => Self::Exact,
      None => Self::Mismatched,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
  Size,
  Spectrum,
  Output,
}

impl fmt::Display for Fault {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Fault::Size => write!(f, "there is no transform of that size"),
      Fault::Spectrum => write!(f, "the spectrum is not as long as the transform"),
      Fault::Output => write!(f, "the output is not twice as long as the spectrum"),
    }
  }
}

impl core::error::Error for Fault {}
