use core::ffi::c_void;
use std::ptr::NonNull;

#[derive(Debug)]
#[repr(transparent)]
pub struct SendSyncNonNull<T: ?Sized>(NonNull<T>);

#[doc = "The thin opaque pointer specialization used in the dav1d C ABI."]
pub type Dav1dOpaquePointer = SendSyncNonNull<c_void>;

const _: () = {
  use core::mem::{align_of, offset_of, size_of};

  assert!(size_of::<Dav1dOpaquePointer>() == size_of::<*mut c_void>());
  assert!(align_of::<Dav1dOpaquePointer>() == align_of::<*mut c_void>());
  assert!(offset_of!(Dav1dOpaquePointer, 0) == 0);
  assert!(size_of::<Option<Dav1dOpaquePointer>>() == size_of::<*mut c_void>());
  assert!(align_of::<Option<Dav1dOpaquePointer>>() == align_of::<*mut c_void>());
};

impl<T: ?Sized> Clone for SendSyncNonNull<T> {
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: ?Sized> Copy for SendSyncNonNull<T> {}

#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "safe construction admits shared references to Send + Sync pointees and raw pointer access still requires the caller to prove its lifetime and access rules"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "safe construction requires a Send and Sync pointee and external raw pointers must uphold the same access rules"
  )
)]
// SAFETY: Sending the wrapper only copies the pointer. Dereferencing it requires the caller
// to establish validity and the pointee's access rules on the receiving thread.
unsafe impl<T: ?Sized> Send for SendSyncNonNull<T> {}

#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "cloning and sharing only copy a pointer; typed access is unsafe and its caller must uphold the pointee access rules"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "safe construction requires a Send and Sync pointee and external raw pointers must uphold the same access rules"
  )
)]
// SAFETY: Sharing the wrapper only exposes the pointer. Turning it into a reference or
// recovering ownership requires the caller to satisfy the corresponding safety contract.
unsafe impl<T: ?Sized> Sync for SendSyncNonNull<T> {}

impl<T: ?Sized> SendSyncNonNull<T> {
  pub const fn cast<U>(
    self,
  ) -> core::result::Result<SendSyncNonNull<U>, console_core_never::Never> {
    Ok(SendSyncNonNull(self.0.cast()))
  }
}

impl<T: ?Sized + Send + Sync> SendSyncNonNull<T> {
  #[doc = "# Safety\nThe caller must preserve the allocation and its access rules for every use of the returned pointer."]
  pub const unsafe fn new_unchecked(
    ptr: NonNull<T>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(ptr))
  }

  pub fn from_ref(reference: &T) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(NonNull::from(reference)))
  }

  #[cfg_attr(
    dylint_lib = "explicit059_no_unchecked_api",
    expect(
      explicit059_no_unchecked_api,
      reason = "the owning allocation is transferred into the C callback cookie and reclaimed from the same pointer by into_box"
    )
  )]
  pub fn from_box(owner: Box<T>) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(NonNull::from(Box::leak(owner))))
  }

  pub const fn as_ptr(&self) -> core::result::Result<NonNull<T>, console_core_never::Never> {
    Ok(self.0)
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the caller supplies the lifetime and guarantees shared access to the pointee"
    )
  )]
  #[doc = "# Safety\nThe pointer must be valid for shared access for the lifetime of the returned reference."]
  pub const unsafe fn as_ref(&self) -> core::result::Result<&T, console_core_never::Never> {
    Ok({
      // SAFETY: `self` originally came from a `&T` in `Self::from_ref`.
      unsafe { self.0.as_ref() }
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "the caller must transfer the unique allocation originally admitted by from_box; no second reconstruction is permitted"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the caller transfers the unique allocation whose Box ownership is recovered here"
    )
  )]
  #[doc = "# Safety\nThe pointer must identify an allocation transferred by `from_box`. The caller must own that allocation exclusively and may recover it only once."]
  pub unsafe fn into_box(self) -> core::result::Result<Box<T>, console_core_never::Never> {
    Ok({
      let Ok(own_ptr) = self.as_ptr();
      let ptr = own_ptr.as_ptr();

      // SAFETY: `self` originally came from a `Box<T>` in `Self::from_box`.
      // And since `Self::into_box` hasn't been called yet, it's unique.
      unsafe { Box::from_raw(ptr) }
    })
  }
}
