use std::marker::PhantomData;
use std::ptr;

#[repr(C)]
pub struct FFISafe<'a, T> {
  phantom: PhantomData<&'a T>,
  non_zst: bool,
}

impl<'a, T> FFISafe<'a, T> {
  pub fn _new_mut(this: &'a mut T) -> core::result::Result<*mut Self, console_core_never::Never> {
    Ok(ptr::from_mut(this).cast())
  }

  #[cfg(feature = "c-ffi")]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C callback returns the opaque pointer previously created from the same exclusive Rust borrow"
    )
  )]
  pub unsafe fn _get_mut(
    this: *mut Self,
  ) -> core::result::Result<&'a mut T, console_core_never::Never> {
    Ok({
      // SAFETY: `this` originally was a `&'a mut T` in `Self::new_mut`.
      unsafe { &mut *this.cast() }
    })
  }
}
