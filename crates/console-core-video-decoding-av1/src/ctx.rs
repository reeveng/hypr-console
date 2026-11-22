#![forbid(unsafe_code)]
//! The [`CaseSet`] API below is a safe and simplified version of the `case_set*` macros in `ctx.h`.
//!
//! The `case_set*` macros themselves replaced `memset`s in order to further optimize them
//! (in e3b5d4d044506f9e0e95e79b3de42fd94386cc61,
//! which performed the following optimizations:
//! 1. larger, inlinable writes for small power of 2 lengths
//! 2. aligned writes for the above
//! 3. 8-byte aligned fields [`BlockContext`]
//!   * allows 8-byte aligned writes
//!   * better cache boundary alignment
//!
//! (1) is easy to preserve in Rust, but (2) is difficult to do so without overhead,
//! as unaligned writes are UB, and so we'd need to check at runtime if they're aligned
//! (a runtime-determined `off`set is used, so we can't reasonably ensure this at compile-time).
//!
//! To more thoroughly check this, I ran the same benchmarks done in
//! e3b5d4d044506f9e0e95e79b3de42fd94386cc61, which introduced the `case_set*` macros:
//!
//! ```sh
//! cargo build --release && hyperfine './target/release/dav1d -i ./tests/large/chimera_8b_1080p.ivf -l 1000 -o /dev/null'
//! ```
//!
//! for 3 implementations:
//! 1. the original `case_set*` macros translated directly to `unsafe` Rust `fn`s
//! 2. the safe [`CaseSet`] implementation below using [`small_memset`] with its small powers of 2 optimization
//! 3. a safe [`CaseSet`] implementation using [`slice::fill`]/`memset` only
//!
//! The [`small_memset`] version was ~1.27% faster than the `case_set*` one,
//! and ~3.26% faster than the `memset` one.
//! The `case_set*` macros were also faster than `memset` in C by a similar margin,
//! meaning the `memset` option is the slowest in both C and Rust,
//! and since it was replaced with `case_set*` in C, we shouldn't use it in Rust.
//! Thus, the [`small_memset`] implementation seems optimal, as it:
//! * is the fastest of the Rust implementations
//! * is completely safe
//! * employs the same small powers of 2 optimization the `case_set*` implementation did
//! * is far simpler than the `case_set*` implementation, consisting of a `match` and array writes
//!
//! [`BlockContext`]: crate::src::env::BlockContext
use crate::src::disjoint_mut::AsMutPtr;
use crate::src::disjoint_mut::DisjointMut;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ops::Range;

#[inline]
pub fn small_memset<T: Copy, const UP_TO: usize, const WITH_DEFAULT: bool>(
  buf: &mut [T],
  val: T,
) -> core::result::Result<(), console_core_never::Never> {
  fn write_array<T: Copy, const N: usize>(
    buf: &mut [T],
    value: T,
  ) -> core::result::Result<(), console_core_never::Never> {
    match buf.first_chunk_mut::<N>() {
      Some(array) => *array = [value; N],
      None => buf.fill(value),
    }

    Ok(())
  }

  match (buf.len(), UP_TO) {
    (1, 1..) => write_array::<T, 1>(buf, val),
    (2, 2..) => write_array::<T, 2>(buf, val),
    (4, 4..) => write_array::<T, 4>(buf, val),
    (8, 8..) => write_array::<T, 8>(buf, val),
    (16, 16..) => write_array::<T, 16>(buf, val),
    (32, 32..) => write_array::<T, 32>(buf, val),
    _ => {
      match WITH_DEFAULT.then_some(()) {
        Some(()) => buf.fill(val),
        None => {}
      }

      Ok(())
    }
  }
}

pub struct ContextRun {
  pub offset: u64,
  pub length: u64,
}

pub struct CaseSetter<const UP_TO: usize, const WITH_DEFAULT: bool> {
  window: Range<u64>,
}

impl<const UP_TO: usize, const WITH_DEFAULT: bool> CaseSetter<UP_TO, WITH_DEFAULT> {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(explicit014_no_index_slice, reason = "the caller passes either a reserved segment-map row or a width-checked transform map row")
  )]
  pub fn set<T: Copy>(
    &self,
    buf: &mut [T],
    val: T,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(window) = console_core_number_conversion::indices(self.window.clone());
    small_memset::<T, UP_TO, WITH_DEFAULT>(&mut buf[window], val)
  }

  #[inline]
  pub fn set_disjoint<T, V>(
    &self,
    buf: &DisjointMut<T>,
    val: V,
  ) -> core::result::Result<(), console_core_never::Never>
  where
    T: AsMutPtr<Target = V>,
    V: Copy,
  {
    let Ok(window) = console_core_number_conversion::indices(self.window.clone());
    let mut guard = buf.index_mut(window);
    small_memset::<V, UP_TO, WITH_DEFAULT>(&mut guard, val)
  }

  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit014_no_index_slice",
    expect(explicit014_no_index_slice, reason = "the block geometry limits the left context update to the fixed-width context buffer")
  )]
  pub fn set_exclusive<T, V>(
    &self,
    buf: &mut DisjointMut<T>,
    val: V,
  ) -> core::result::Result<(), console_core_never::Never>
  where
    T: AsMutPtr<Target = V> + DerefMut,
    <T as Deref>::Target: AsMut<[V]>,
    V: Copy,
  {
    let Ok(window) = console_core_number_conversion::indices(self.window.clone());
    small_memset::<V, UP_TO, WITH_DEFAULT>(&mut buf.get_mut().as_mut()[window], val)
  }
}

pub struct CaseSet<const UP_TO: usize, const WITH_DEFAULT: bool>;

impl<const UP_TO: usize, const WITH_DEFAULT: bool> CaseSet<UP_TO, WITH_DEFAULT> {
  #[inline]
  pub fn one<T, F>(
    ctx: T,
    run: ContextRun,
    set_ctx: F,
  ) -> core::result::Result<(), console_core_never::Never>
  where
    F: FnOnce(&CaseSetter<UP_TO, WITH_DEFAULT>, T),
  {
    let window = run.offset..run.offset.wrapping_add(run.length);
    set_ctx(&CaseSetter { window }, ctx);
    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "a shared field list updates the corresponding left-exclusive and above-disjoint AV1 block contexts without repeating their ownership rules"
  )
)]
macro_rules! case_set_al {
    (
        <$UP:literal, $WD:literal>
        l: ($l:expr, $llen:expr, $loff:expr) $(also |$lc:ident| $lextra:block)?,
        a: ($a:expr, $alen:expr, $aoff:expr) $(also |$ac:ident| $aextra:block)?,
        $( $f:ident $([$fi:expr])? = ($lv:expr, $av:expr) ),* $(,)?
    ) => {{

        let Ok(left_length) = ::console_core_number_conversion::fixed_index($llen);
        let Ok(left_offset) = ::console_core_number_conversion::fixed_index($loff);
        let left_run = $crate::src::ctx::ContextRun { offset: left_offset, length: left_length };
        let Ok(()) = $crate::src::ctx::CaseSet::<$UP, $WD>::one((), left_run, |case, ()| {
            let l = $l;
            $( let Ok(()) = case.set_exclusive(&mut l.$f $([$fi])?, $lv); )*
            $( { let $lc = case; $lextra } )?
        });

        let Ok(above_length) = ::console_core_number_conversion::fixed_index($alen);
        let Ok(above_offset) = ::console_core_number_conversion::fixed_index($aoff);
        let above_run = $crate::src::ctx::ContextRun { offset: above_offset, length: above_length };
        let Ok(()) = $crate::src::ctx::CaseSet::<$UP, $WD>::one((), above_run, |case, ()| {
            let a = $a;
            $( let Ok(()) = case.set_disjoint(&a.$f $([$fi])?, $av); )*
            $( { let $ac = case; $aextra } )?
        });
    }};
}
pub(crate) use case_set_al;
