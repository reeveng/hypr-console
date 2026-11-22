//! Audio sample values expressed as integers or normalized floating-point numbers.
//!
//! The conversion traits preserve sample scale and signedness, including the
//! 24-bit sample types, and never dither: a conversion into a lower precision
//! drops the low bits, which is what every caller here wants of a decoder's
//! output. No codec, container, audio device, or allocator is required.

#![no_std]
#![forbid(unsafe_code)]

pub mod conv;
pub mod sample;

pub use conv::{ConvertibleSample, FromSample, IntoSample};
pub use sample::{Sample, SampleFormat, i24, u24};
