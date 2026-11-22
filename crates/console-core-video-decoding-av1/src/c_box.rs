#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "c-ffi")]
use crate::src::send_sync_non_null::Dav1dOpaquePointer;
#[cfg(feature = "c-ffi")]
use std::ffi::c_void;
#[cfg(feature = "c-ffi")]
use std::marker::PhantomData;
use std::ops::Deref;
#[cfg(feature = "c-ffi")]
use std::pin::Pin;
#[cfg(feature = "c-ffi")]
use std::ptr::NonNull;
#[cfg(feature = "c-ffi")]
use std::ptr::drop_in_place;

#[cfg(feature = "c-ffi")]
pub type FnFree = unsafe extern "C" fn(ptr: *const u8, cookie: Option<Dav1dOpaquePointer>);

#[cfg(feature = "c-ffi")]
#[derive(Debug)]
pub struct Free {
  pub free: FnFree,

  pub cookie: Option<Dav1dOpaquePointer>,
}

#[cfg(feature = "c-ffi")]
impl Free {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the allocation owner invokes its validated C deallocator once with the original pointer and cookie"
    )
  )]
  pub unsafe fn free(
    &self,
    ptr: *mut c_void,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      // SAFETY: `self` came from `CBox::from_c`,
      // which requires `self.free` to deallocate the `NonNull<T>` passed to it,
      // and `self.cookie` to be passed to it, which it is.
      unsafe { (self.free)(ptr.cast::<u8>().cast_const(), self.cookie) }
    };

    Ok(())
  }
}

#[cfg(feature = "c-ffi")]
#[derive(Debug)]
pub struct Unique<T: ?Sized> {
  pointer: NonNull<T>,

  _marker: PhantomData<T>,
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "the unique pointer moves the allocation and exclusive access to T, which is Send"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the unique pointer transfers or shares only the access permitted by the pointee bound"
  )
)]
unsafe impl<T: Send + ?Sized> Send for Unique<T> {}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "access through a shared borrow yields only a shared reference to T, which is Sync"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the unique pointer transfers or shares only the access permitted by the pointee bound"
  )
)]
unsafe impl<T: Sync + ?Sized> Sync for Unique<T> {}

#[derive(Debug)]
pub enum CBox<T: ?Sized> {
  Rust(Box<T>),
  #[cfg(feature = "c-ffi")]
  C {
    data: Unique<T>,
    free: Free,
  },
}

#[cfg(not(feature = "c-ffi"))]
impl<T: ?Sized> AsRef<T> for CBox<T> {
  fn as_ref(&self) -> &T {
    let Self::Rust(r#box) = self;
    r#box.as_ref()
  }
}

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> AsRef<T> for CBox<T> {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the CBox owns the initialized allocation for the duration of its shared borrow"
    )
  )]
  fn as_ref(&self) -> &T {
    match self {
      Self::Rust(r#box) => r#box.as_ref(),
      // SAFETY: `data` is a `Unique<T>`, which behaves as if it were a `T`,
      // so we can take `&` references of it.
      // Furthermore, `data` is never moved and is valid to dereference,
      // so this reference can live as long as `CBox` and still be valid the whole time.
      Self::C { data, .. } => unsafe { data.pointer.as_ref() },
    }
  }
}

impl<T: ?Sized> Deref for CBox<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "this branch owns an initialized Rust value in C storage, so it drops the value once before the C callback frees the allocation"
  )
)]
impl<T: ?Sized> Drop for CBox<T> {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "release of a CBox drops the initialized Rust value and calls the matching C deallocator exactly once"
    )
  )]
  fn drop(&mut self) {
    match self {
      Self::Rust(_) => {}
      Self::C { data, free, .. } => {
        let ptr = data.pointer.as_ptr();

        // SAFETY: See below.
        // The [`FnFree`] won't run Rust's `fn drop`,
        // so we have to do this ourselves first.
        unsafe { drop_in_place(ptr) };

        let ptr_2 = ptr.cast();

        // SAFETY: See safety docs on [`Self::data`] and [`Self::from_c`].
        unsafe {
          let Ok(free_free) = free.free(ptr_2);
          free_free
        }
      }
    }
  }
}

impl<T: ?Sized> CBox<T> {
  #[cfg(feature = "c-ffi")]
  pub unsafe fn from_c(
    data: NonNull<T>,
    free: Free,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok({
      Self::C {
        data: Unique {
          pointer: data,
          _marker: PhantomData,
        },
        free,
      }
    })
  }

  pub fn from_box(data: Box<T>) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self::Rust(data))
  }

  #[cfg(feature = "c-ffi")]
  #[cfg_attr(
    dylint_lib = "explicit096_no_unchecked_memory",
    expect(
      explicit096_no_unchecked_memory,
      reason = "both the boxed and C-owned pointees have stable addresses until their owner is dropped"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "moving the owning CBox leaves the boxed or C-allocated pointee in its original location"
    )
  )]
  pub fn into_pin(self) -> core::result::Result<Pin<Self>, console_core_never::Never> {
    Ok({
      // SAFETY: the Box or C allocation keeps its pointee at a stable address until drop.
      unsafe { Pin::new_unchecked(self) }
    })
  }
}

#[cfg(feature = "c-ffi")]
impl<T: ?Sized> From<CBox<T>> for Pin<CBox<T>> {
  fn from(value: CBox<T>) -> Self {
    let Ok(pin) = value.into_pin();

    pin
  }
}
