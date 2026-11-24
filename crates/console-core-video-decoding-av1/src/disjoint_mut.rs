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
use crate::src::internal::ScratchParallelism;

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
  expect(
    explicit058_contain_unsafe,
    reason = "the unchecked feature selects the same tile-owned buffer layout without the default runtime disjointness checks"
  )
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

pub(crate) fn dm_arc_try_new<T: Copy>(
  count: u64,
  value: T,
  parallelism: &ScratchParallelism,
) -> Rav1dResult<DisjointMutArcSlice<T>> {
  let Ok(native_count) = index(count);
  let Ok(round_trip) = fixed_index(native_count);

  match round_trip.cmp(&count) {
    Ordering::Equal => {}
    Ordering::Less | Ordering::Greater => return Err(ENOMEM),
  }

  let mut values = Vec::new();
  values
    .try_resize(native_count, value)
    .map_err(|_allocate| ENOMEM)?;
  let Ok(mut allocation) = dm_new(values.into_boxed_slice());
  let Ok(threads) = index(parallelism.workers);
  let Ok(tiles) = index(parallelism.tiles);
  allocation.configure_parallelism(threads, tiles);
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the external DisjointMutArcSlice ABI retains a frame buffer across its decoded reference slots after its unique initialization"
    )
  )]
  let shared = std::sync::Arc::new(allocation);

  Ok(DisjointMutArcSlice { inner: shared })
}
