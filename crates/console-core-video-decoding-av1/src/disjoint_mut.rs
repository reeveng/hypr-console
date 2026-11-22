//! Wrapper that allows concurrent, disjoint mutation of a slice-like owned
//! structure.
//!
//! This module re-exports the core `DisjointMut` type from the `rav1d-disjoint-mut`
//! crate (a provably safe abstraction with always-on bounds checking by default).
//!
//! AlignedVec and Align* `ExternalAsMutPtr` impls live in the `rav1d-align` crate.

use core::cmp::Ordering;

use console_core_number_conversion::{fixed_index, index};

use crate::src::error::{Rav1dError::ENOMEM, Rav1dResult};

pub use rav1d_disjoint_mut::AsMutPtr;
pub use rav1d_disjoint_mut::DisjointImmutGuard;
pub use rav1d_disjoint_mut::DisjointMut;
pub use rav1d_disjoint_mut::DisjointMutArcSlice;
pub use rav1d_disjoint_mut::DisjointMutGuard;
pub use rav1d_disjoint_mut::DisjointMutSlice;
#[cfg(feature = "c-ffi")]
pub use rav1d_disjoint_mut::ExternalAsMutPtr;
pub use rav1d_disjoint_mut::SliceBounds;
pub use rav1d_disjoint_mut::TryResizable;
pub use rav1d_disjoint_mut::TryResizableWith;

#[cfg(not(feature = "unchecked"))]
pub fn dm_new<T: AsMutPtr>(
  val: T,
) -> core::result::Result<DisjointMut<T>, console_core_never::Never> {
  Ok(DisjointMut::new_eager(val))
}

#[cfg(feature = "unchecked")]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(explicit058_contain_unsafe, reason = "the unchecked feature selects the same tile-owned buffer layout without the default runtime disjointness checks")
)]
pub fn dm_new<T: AsMutPtr>(
  val: T,
) -> core::result::Result<DisjointMut<T>, console_core_never::Never> {
  Ok({
    // SAFETY: All borrow patterns in rav1d-safe are verified to be disjoint
    // by running the full 784-vector conformance suite in checked mode.
    unsafe { DisjointMut::dangerously_unchecked(val) }
  })
}

#[cfg_attr(
  all(feature = "unchecked", dylint_lib = "explicit058_contain_unsafe"),
  expect(explicit058_contain_unsafe, reason = "only the opt-in unchecked mode selects the untracked form of the frame-shared tile allocation")
)]
pub fn dm_arc_try_new<T: Copy>(count: u64, value: T) -> Rav1dResult<DisjointMutArcSlice<T>> {
  let Ok(native_count) = index(count);
  let Ok(round_trip) = fixed_index(native_count);

  match round_trip.cmp(&count) {
    Ordering::Equal => {}
    Ordering::Less | Ordering::Greater => return Err(ENOMEM),
  }

  #[cfg(not(feature = "unchecked"))]
  let allocation = DisjointMutArcSlice::try_new(native_count, value);
  #[cfg(feature = "unchecked")]
  let allocation = {
    // SAFETY: The shared slab uses the same tile ownership and access ranges
    // as the always-checked configuration. See the unchecked dm_new path.
    unsafe { DisjointMutArcSlice::try_new_unchecked(native_count, value) }
  };

  allocation.map_err(|_allocation| ENOMEM)
}
