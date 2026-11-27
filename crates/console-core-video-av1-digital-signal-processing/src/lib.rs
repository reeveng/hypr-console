//! Shared AV1 digital signal processing interfaces and scalar kernels.
//!
//! Motion compensation works on borrowed sample views. Picture guards and
//! allocation belong to the decoder. All strides and positions are in samples.

#![no_std]
#![forbid(unsafe_code)]

mod motion;
mod taps;

pub use motion::{
  EightTap, FilterFootprint, MotionCompensation, MotionScratch, PlaneMut, PlaneRef, Prediction,
  RowFilters, SCRATCH_COLUMNS, VerticalInput, apply, copy_pixels, copy_predictors, read_extent,
  scalar_vertical_put,
};
pub use taps::{TapTable, taps};
