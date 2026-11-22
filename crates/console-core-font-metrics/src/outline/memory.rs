//! Support for temporary memory allocation, making use of the stack for
//! small sizes.

/// Invokes the callback with a memory buffer of the requested size.
pub(super) fn with_temporary_memory<R>(size: usize, mut f: impl FnMut(&mut [u8]) -> R) -> R {
  #[inline(never)]
  fn stack_mem<const STACK_SIZE: usize, R>(size: usize, mut f: impl FnMut(&mut [u8]) -> R) -> R {
    f(&mut [0u8; STACK_SIZE][..size])
  }

  match size <= 512 {
    true => stack_mem::<512, _>(size, f),
    false => match size <= 1024 {
      true => stack_mem::<1024, _>(size, f),
      false => match size <= 2048 {
        true => stack_mem::<2048, _>(size, f),
        false => match size <= 4096 {
          true => stack_mem::<4096, _>(size, f),
          false => match size <= 8192 {
            true => stack_mem::<8192, _>(size, f),
            false => match size <= 16384 {
              true => stack_mem::<16384, _>(size, f),
              false => f(&mut vec![0u8; size]),
            },
          },
        },
      },
    },
  }
}
