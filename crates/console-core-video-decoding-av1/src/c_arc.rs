#![deny(unsafe_op_in_unsafe_fn)]

use crate::src::c_box::CBox;
use crate::src::error::{Rav1dError::EINVAL, Rav1dResult};
use console_core_number_conversion::fixed_index;
#[cfg(feature = "c-ffi")]
use console_core_number_conversion::checked_index;
use core::ops::Range;
#[cfg(feature = "c-ffi")]
use std::marker::PhantomData;
#[cfg(feature = "c-ffi")]
use std::mem::size_of_val;
use std::ops::Deref;
#[cfg(feature = "c-ffi")]
use std::pin::Pin;
#[cfg(feature = "c-ffi")]
use std::ptr::NonNull;
use std::sync::Arc;

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "Arc::into_raw always returns a non-null pointer to its allocation and transfers one strong reference"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "one strong reference is handed to the C owner until it returns the same handle"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the standard Arc raw transfer always yields a non-null allocation pointer"
  )
)]
pub fn arc_into_raw<T: ?Sized>(
  arc: Arc<T>,
) -> core::result::Result<NonNull<T>, console_core_never::Never> {
  Ok({
    let raw = Arc::into_raw(arc).cast_mut();

    // SAFETY: [`Arc::into_raw`] never returns null.
    unsafe { NonNull::new_unchecked(raw) }
  })
}

#[cfg(not(feature = "c-ffi"))]
#[derive(Debug)]
pub struct CArc<T: ?Sized> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "clones retain the same immutable packet backing instead of copying every decoded subrange"
    )
  )]
  owner: Arc<Box<T>>,

  view: Option<Range<u64>>,
}

#[cfg(feature = "c-ffi")]
#[derive(Debug)]
pub struct CArc<T: ?Sized> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "cloned C packet views share an immutable allocation and retain the C release callback until the last view closes"
    )
  )]
  owner: Arc<Pin<CBox<T>>>,

  stable_ref: StableRef<T>,
}

#[cfg(feature = "c-ffi")]
#[derive(Debug)]
struct StableRef<T: ?Sized>(NonNull<T>);

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> Clone for StableRef<T> {
  fn clone(&self) -> Self {
    *self
  }
}

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> Copy for StableRef<T> {}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "the pointer lends a shared reference to a stable allocation, and T: Sync permits that reference to be sent"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the stable reference can move when the pointee permits shared access across threads"
  )
)]
unsafe impl<T: Sync + ?Sized> Send for StableRef<T> {}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "the pointer gives only shared access to T, and T: Sync permits it to be shared"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "cloning the pointer lends only shared access and its pointee is Sync"
  )
)]
unsafe impl<T: Sync + ?Sized> Sync for StableRef<T> {}

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> AsRef<T> for CArc<T> {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the pointer is created from a live pinned owner or a checked subslice of that owner"
    )
  )]
  fn as_ref(&self) -> &T {
    let allocation = (*self.owner).as_ref().get_ref();
    let Ok(base) = fixed_index(std::ptr::from_ref(allocation).cast::<()>().addr());
    let Ok(address) = fixed_index(self.stable_ref.0.as_ptr().cast::<()>().addr());
    let Ok(length) = fixed_index(size_of_val(allocation));
    assert!(address >= base && address <= base.saturating_add(length));

    // SAFETY: Pin preserves the backing allocation and slice_in_place only stores a checked subslice.
    unsafe { self.stable_ref.0.as_ref() }
  }
}

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> Deref for CArc<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

#[cfg(not(feature = "c-ffi"))]
impl Deref for CArc<u8> {
  type Target = u8;

  fn deref(&self) -> &u8 {
    &self.owner
  }
}

#[cfg(not(feature = "c-ffi"))]
impl<T> Deref for CArc<[T]> {
  type Target = [T];

  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(
      explicit014_no_index_slice,
      reason = "slice_in_place establishes that every stored subrange is within the backing allocation"
    )
  )]
  fn deref(&self) -> &[T] {
    let full: &[T] = &self.owner;

    match &self.view {
      None => full,
      Some(window) => {
        let Ok(indices) = console_core_number_conversion::indices(window.clone());
        &full[indices]
      }
    }
  }
}

#[cfg(not(feature = "c-ffi"))]
impl<T: ?Sized> Clone for CArc<T> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the packet subrange shares the immutable backing allocation with the original view"
    )
  )]
  fn clone(&self) -> Self {
    Self {
      owner: self.owner.clone(),
      view: self.view.clone(),
    }
  }
}

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> Clone for CArc<T> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the borrowed pointer remains valid for this clone because the same backing allocation is retained"
    )
  )]
  fn clone(&self) -> Self {
    Self {
      owner: Arc::clone(&self.owner),
      stable_ref: self.stable_ref,
    }
  }
}

#[cfg(not(feature = "c-ffi"))]
impl<T: ?Sized> CArc<T> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "a packet can hold multiple independently released immutable views of this same allocation"
    )
  )]
  pub fn wrap(owner: CBox<T>) -> Rav1dResult<Self> {
    let CBox::Rust(boxed) = owner;
    Ok(Self {
      owner: Arc::new(boxed),
      view: None,
    })
  }
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "constructing a view consumes an existing strong reference and reuses the pinned shared allocation"
  )
)]
impl<T: ?Sized> From<Arc<Pin<CBox<T>>>> for CArc<T> {
  fn from(owner: Arc<Pin<CBox<T>>>) -> Self {
    let stable_ref = StableRef(NonNull::from((*owner).as_ref().get_ref()));
    Self {
      owner,
      stable_ref,
    }
  }
}

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> CArc<T> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "a C-owned packet keeps its release callback alive while independent decoded views are in use"
    )
  )]
  pub fn wrap(owner: CBox<T>) -> Rav1dResult<Self> {
    let Ok(pin) = owner.into_pin();
    let owner_2 = Arc::new(pin);
    Ok(Self::from(owner_2))
  }
}

#[cfg(feature = "c-ffi")]
#[repr(transparent)]
pub struct RawArc<T>(NonNull<PhantomData<T>>);

#[cfg(feature = "c-ffi")]
impl<T> Clone for RawArc<T> {
  fn clone(&self) -> Self {
    *self
  }
}

#[cfg(feature = "c-ffi")]
impl<T> Copy for RawArc<T> {}

#[cfg(feature = "c-ffi")]
impl<T> RawArc<T> {
  #[doc = "Retains a Rust reference while the C owner keeps its original handle."]
  #[doc = "\n# Safety\nThe handle must have an outstanding strong reference from `from_arc`. Its C owner must keep that reference alive until this call returns."]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "retaining a C handle requires the caller to keep its original raw reference alive"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "the retained strong reference is reconstructed immediately while the original C token stays live"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "C and Rust independently own references to the same picture buffer or metadata during the borrow"
    )
  )]
  pub(crate) unsafe fn borrow_clone(&self) -> core::result::Result<Arc<T>, console_core_never::Never> {
    let raw = self.0.cast::<T>().as_ptr();

    // SAFETY: The C owner holds the token, and from_raw consumes only the count added here.
    let retained = unsafe {
      Arc::increment_strong_count(raw);
      Arc::from_raw(raw)
    };

    Ok(retained)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "C handles carry one transferred strong reference returned by into_arc"
    )
  )]
  pub fn from_arc(arc: Arc<T>) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self(
        ({
          let Ok(explicit_value) = arc_into_raw(arc);
          explicit_value
        })
        .cast(),
      )
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C caller must hold a live, unconsumed Arc transfer token"
    )
  )]
  pub unsafe fn as_ref(&self) -> core::result::Result<&T, console_core_never::Never> {
    Ok({
      // SAFETY: Caller guarantees the RawArc is from from_arc and not yet consumed.
      // The NonNull was created from Arc::into_raw, so the pointer is valid.
      unsafe { self.0.cast().as_ref() }
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "the C handle carries exactly one ownership token from Arc::into_raw; the caller must pass it back once"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the raw C token represents one outstanding Arc strong reference"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the C handle returns its one transferred strong reference to Rust"
    )
  )]
  pub unsafe fn into_arc(self) -> core::result::Result<Arc<T>, console_core_never::Never> {
    Ok({
      let raw = self.0.cast().as_ptr();

      // SAFETY: Caller guarantees this RawArc originated from from_arc (Arc::into_raw),
      // and has not been consumed yet, so reconstructing the Arc is valid.
      unsafe { Arc::from_raw(raw) }
    })
  }
}

#[cfg(feature = "c-ffi")]
#[repr(transparent)]
pub struct RawCArc<T: ?Sized>(RawArc<Pin<CBox<T>>>);

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> CArc<T> {
  pub fn into_raw(self) -> core::result::Result<RawCArc<T>, console_core_never::Never> {
    Ok({
      let Ok(raw_arc) = RawArc::from_arc(self.owner);

      RawCArc(raw_arc)
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C caller transfers the token originally produced by into_raw back to Rust"
    )
  )]
  pub unsafe fn from_raw(
    raw: RawCArc<T>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      // SAFETY: Caller guarantees raw is from into_raw (which called RawArc::from_arc).
      let owner = unsafe {
        let Ok(arc) = raw.0.into_arc();
        arc
      };

      Self::from(owner)
    })
  }

  #[doc = "Retains a C allocation without consuming its original handle."]
  #[doc = "\n# Safety\nThe C owner must keep the token from `into_raw` alive until this call returns."]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the initialized C record retains its token while Rust borrows the allocation"
    )
  )]
  pub(crate) unsafe fn borrow_clone(
    raw: &RawCArc<T>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    // SAFETY: The caller keeps the original token from into_raw alive during the retain.
    let Ok(owner) = unsafe { raw.0.borrow_clone() };

    Ok(Self::from(owner))
  }
}

impl<T> CArc<[T]> {
  pub fn slice_in_place(&mut self, range: Range<u64>) -> Rav1dResult {
    #[cfg(not(feature = "c-ffi"))]
    {
      let backing = match &self.view {
        Some(view) => view.clone(),
        None => {
          let Ok(length) = fixed_index(self.owner.len());
          0..length
        }
      };
      let start = backing.start.checked_add(range.start).ok_or(EINVAL)?;
      let end = backing.start.checked_add(range.end).ok_or(EINVAL)?;

      match (start.cmp(&end), end.cmp(&backing.end)) {
        (core::cmp::Ordering::Greater, _) | (_, core::cmp::Ordering::Greater) => {
          return Err(EINVAL);
        }
        _ => self.view = Some(start..end),
      }
    }

    #[cfg(feature = "c-ffi")]
    {
      let start = checked_index(range.start).map_err(|_unrepresentable_start| EINVAL)?;
      let end = checked_index(range.end).map_err(|_unrepresentable_end| EINVAL)?;
      let sub = self.as_ref().get(start..end).ok_or(EINVAL)?;
      self.stable_ref = StableRef(NonNull::from(sub));
    }

    Ok(())
  }

  pub fn split_at(mut this: Self, mid: u64) -> Rav1dResult<(Self, Self)> {
    let Ok(length) = fixed_index(this.len());
    let mut first = this.clone();
    first.slice_in_place(0..mid)?;
    this.slice_in_place(mid..length)?;

    Ok((first, this))
  }
}

impl<T> CArc<[T]>
where
  T: Default + 'static,
{
  #[cfg(feature = "c-ffi")]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "native counts here describe the Vec allocator's address-space limit and allocation request"
    )
  )]
  pub fn zeroed_slice(size: u64) -> Rav1dResult<Self> {
    use crate::src::error::Rav1dError::ENOMEM;

    let length = checked_index(size).map_err(|_outside_address_space| ENOMEM)?;
    let elements = length.min(const {
      match isize::MAX.cast_unsigned().checked_div(core::mem::size_of::<T>()) {
        Some(allocation_limit) => allocation_limit,
        None => usize::MAX,
      }
    });

    match length.cmp(&elements) {
      core::cmp::Ordering::Greater => return Err(ENOMEM),
      core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
    }

    let mut data = Vec::new();
    data.try_reserve_exact(elements).map_err(|_allocation_failure| ENOMEM)?;
    data.resize_with(elements, T::default);
    let Ok(c_box) = CBox::from_box(data.into_boxed_slice());

    Self::wrap(c_box)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[cfg(feature = "c-ffi")]
  #[test]
  fn c_packet_storage_rejects_lengths_beyond_the_allocator_limit() -> Rav1dResult {
    use crate::src::error::Rav1dError::ENOMEM;

    assert!(matches!(CArc::<[u8]>::zeroed_slice(u64::MAX), Err(ENOMEM)));
    let data = CArc::<[u8]>::zeroed_slice(4)?;
    assert_eq!(&*data, &[0; 4]);

    Ok(())
  }

  #[test]
  fn invalid_subranges_preserve_the_current_view() -> Rav1dResult {
    let Ok(boxed) = CBox::from_box(Box::<[u16]>::from([1, 2, 3, 4]));
    let mut view = CArc::wrap(boxed)?;
    view.slice_in_place(1..3)?;

    for (start, end) in [(0, 3), (2, 1), (u64::MAX, u64::MAX)] {
      assert_eq!(view.slice_in_place(start..end), Err(EINVAL));
      assert_eq!(&*view, &[2, 3]);
    }

    view.slice_in_place(2..2)?;
    assert!(view.is_empty());

    Ok(())
  }

  #[test]
  fn zero_sized_elements_keep_their_logical_lengths() -> Rav1dResult {
    let Ok(boxed) = CBox::from_box(Box::<[()]>::from([(); 4]));
    let data = CArc::wrap(boxed)?;
    let (prefix, suffix) = CArc::split_at(data, 3)?;
    assert_eq!(prefix.len(), 3);
    assert_eq!(suffix.len(), 1);

    Ok(())
  }
}
