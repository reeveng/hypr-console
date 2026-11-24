#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::Cell;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use console_core_api_compatibility::{NativeOffset, NativeSize};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_pixel_buffers::{RowLayout, RowLayoutError};

#[doc = r###"true once any decoder has used tile threading (n_tc > 1).
When true, compact_read/compact_write_back and
`with_pixel_guard_*` use per-row guards to avoid stride-padding overlap.
When false, they use a single wide guard, which is only sound with no
concurrent tile workers."###]
#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "public standalone picture components without decoder settings retain the legacy monotonic policy; decoder-owned pictures use their own policy"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "public standalone picture components without decoder settings retain the legacy monotonic policy; decoder-owned pictures use their own policy"
  )
)]
static TILE_THREADING: AtomicBool = AtomicBool::new(false);

#[doc = r###"
MONOTONE ON PURPOSE — never stores `false`. This is process-global state
but decoders are per-instance, so a store of `false` from a
single-threaded `rav1d_open` used to clobber the flag for every
CONCURRENTLY LIVE multi-threaded decoder: their tile workers then took the
wide `narrow_guard` path, whose extent is `(h-1) * stride + w` — a 1x16
intra left-edge column claims 16,321 pixels to read 16 — and any
concurrent row write inside that span is a real conflict. In a checked
build that surfaced as a spurious `overlapping DisjointMut` panic in
`rav1d_prepare_intra_edges`; in an `unchecked` build it is an undetected
data race on picture memory.

Measured on this branch before the latch: 6 concurrent
`tile_threading_overlap --ignored` processes (whose
`single_threaded_no_panic` case opens an `n_tc == 1` decoder alongside
`multi_threaded_cdef_lpf_race`'s threaded ones) panicked in 8-9 of 24
runs. After: 0 of 24. See `benchmarks/p2_kernels_2026-08-07.meta`.

Decoder-owned pictures now carry their own policy, installed before
publication and retained across copies. This conservative fallback remains
for raw/legacy components whose scheduling context is unknown."###]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the exported legacy threading latch has a unit return and remains compatible with its existing callback callers"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit008_no_bool_param",
  expect(
    explicit008_no_bool_param,
    reason = "the exported legacy latch accepts the existing native boolean flag"
  )
)]
pub fn set_tile_threading(active: bool) {
  match ::core::primitive::bool::then_some(active, ()) {
    ::core::option::Option::Some(()) => {
      TILE_THREADING.store(true, Ordering::Relaxed);
    }
    ::core::option::Option::None => {}
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this exported compatibility query returns the historical threading flag directly"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit007_no_bool_return",
  expect(
    explicit007_no_bool_return,
    reason = "existing compatibility callers consume this native predicate as a boolean"
  )
)]
pub fn tile_threading_active() -> bool {
  TILE_THREADING.load(Ordering::Relaxed)
}

#[inline(always)]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this exported compatibility query returns the established ARM hull flag directly"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit007_no_bool_return",
  expect(
    explicit007_no_bool_return,
    reason = "existing compatibility callers consume this native predicate as a boolean"
  )
)]
pub fn rect_hull_arm() -> bool {
  false
}

#[inline(always)]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this exported compatibility query returns the established CDEF reservation flag directly"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit007_no_bool_return",
  expect(
    explicit007_no_bool_return,
    reason = "existing compatibility callers consume this native predicate as a boolean"
  )
)]
pub fn cdef_double_reads() -> bool {
  false
}

#[doc = r###"while tile threading is active. See [`Rav1dPictureDataComponent::note_pic_extent`].

**Keyed by file, not by `file:line`, on purpose.** The bounds map's own
reconciliation had to compare multisets rather than `file:line` keys because
inserting two no-op declarations shifted every line number below them. A
line-keyed table would go stale on the next unrelated edit and then either
fail spuriously or, worse, stop matching and silently pass.

Only files whose reservation is bounded by a **constant** (a tap count, a
block dimension, a scratch width) get a tight entry. Files whose reservation
scales with the frame — `owned_recon.rs` (2,688 B observed),
`looprestoration.rs` (~2,050 B), `picture.rs`'s `copy_pixels_to` (4,096 B) —
would need a frame-size-relative bound, which this table cannot express, so
they fall through to [`TILE_THREADED_PIC_EXTENT_MAX_BYTES`].

Provenance for every number: `benchmarks/bounds_verdicts_2026-08-10.meta`,
measured by this module's own counters over the committed crash vectors plus
`dav1d-test-data` `8-bit/data` + `10-bit/data` at t=1 and t=8. Each entry is
the observed maximum; there is no slack, so the gate fails on the first byte
of any widening."###]
pub const PIC_EXTENT_CEILINGS: &[(&str, ::console_core_api_compatibility::NativeSize)] = &[
  ("src/loopfilter.rs", 32),
  ("src/safe_simd/cdef_arm.rs", 32),
  ("src/lr_apply.rs", 32),
];

#[doc = r###"entry. Set from the same measurement; catches gross widenings (a strided
hull at 4K is 50-60 KB) without pretending to bound the frame-scaling sites
tightly."###]
pub const TILE_THREADED_PIC_EXTENT_MAX_BYTES: ::console_core_api_compatibility::NativeSize = 64;

#[doc = r###"
`None` means the file is held to one picture row instead, which
[`pic_extent_ceiling`] cannot express without knowing the plane. The gate
uses this to apply exactly the bound the decoder applies."###]
#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this exported const compatibility query returns the optional native reservation limit"
  )
)]
pub fn pic_extent_ceiling_const(
  file: &str,
) -> Option<::console_core_api_compatibility::NativeSize> {
  PIC_EXTENT_CEILINGS
    .iter()
    .find_map(|(path, limit)| file.ends_with(*path).then_some(*limit))
}

#[doc = r###"row is `row_bytes` long.

A file with a [`PIC_EXTENT_CEILINGS`] entry is held to that constant. Every
other file is held to **one picture row**, which is the coarsest extent that
cannot contain an inter-row gap — the thing #469's rectangle and #475's hull
reserved and that AV1's column-partitioned tiles make unsafe."###]
#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "this exported diagnostic API returns the native byte limit to the existing picture probes"
  )
)]
pub fn pic_extent_ceiling(
  file: &str,
  row_bytes: ::console_core_api_compatibility::NativeSize,
) -> ::console_core_api_compatibility::NativeSize {
  match pic_extent_ceiling_const(file) {
    Some(ceiling) => ceiling,
    None => row_bytes.max(TILE_THREADED_PIC_EXTENT_MAX_BYTES),
  }
}

thread_local! {
    #[doc = r###"(and the pristine copy kept by the loopfilter's diff write-back).
Pulled on read, returned via [`recycle_compact_scratch`] after the
compact buffer has been written back / consumed.

Under tile threading the loop filter / ipred / `with_pixel_guard_*`
paths materialize a per-edge / per-block compact buffer; allocating one
`Vec` per call produced ~3M alloc+free pairs decoding a single 8K frame
(issue #17) — cheap on glibc, pathological on the Windows allocator.
Reusing buffers per thread eliminates that churn. It is per-thread,
so each tile thread owns its own buffers with no cross-tile aliasing —
tile threading stays enabled. Two slots because the loopfilter's
tile-threading path holds a working copy and a pristine copy at once."###]
    static COMPACT_SCRATCH: Cell<[Option<Vec<u8>>; 2]> = const { Cell::new([None, None]) };
}

#[doc = r###"is dry). Pair with [`recycle_compact_scratch`]."###]
#[inline]
#[must_use]
pub(crate) fn take_compact_scratch() -> Result<Vec<u8>, Never> {
  let buffer = COMPACT_SCRATCH.with(|cache| {
    let mut slots = cache.take();
    let [first, second] = &mut slots;
    let reused = match first.take() {
      Some(buffer) => Some(buffer),
      None => second.take(),
    };
    cache.set(slots);

    match reused {
      Some(buffer) => buffer,
      None => Vec::new(),
    }
  });

  Ok(buffer)
}

#[doc = r###"[`WithOffset::compact_read_per_row`] on this thread can reuse the allocation.

Pairs with the `Vec` that `compact_read_per_row` returns. Calling this is an
optimization, not a requirement: simply dropping the `Vec` is always correct
(the next read just allocates a fresh buffer). If a panic unwinds past a
pending recycle the buffer is freed normally — no unsoundness, the pool is
merely empty for the next call (same contract as the MC mid-buffer pool).

Crate-internal: this is decoder plumbing, not part of the public API."###]
#[inline]
pub(crate) fn recycle_compact_scratch(buffer: Vec<u8>) -> Result<(), Never> {
  use core::cmp::Ordering::{Equal, Greater, Less};

  COMPACT_SCRATCH.with(|cache| {
    let mut slots = cache.take();
    let [first, second] = &mut slots;
    let destination = match (first.as_ref(), second.as_ref()) {
      (None, _) => first,
      (Some(_), None) => second,
      (Some(first_buffer), Some(second_buffer)) => {
        match first_buffer.capacity().cmp(&second_buffer.capacity()) {
          Less => first,
          Equal | Greater => second,
        }
      }
    };

    match destination
      .as_ref()
      .map(|cached| cached.capacity().cmp(&buffer.capacity()))
    {
      Some(Equal | Greater) => {}
      Some(Less) | None => *destination = Some(buffer),
    }

    cache.set(slots);
  });

  Ok(())
}

use crate::include::common::bitdepth::BitDepth;

#[doc = r###"
The closure receives `(bytes, offset, stride)` — a mutable byte slice, the
BYTE offset to the first pixel, and the byte stride between rows.

This is a closure-shaped front end for [`WithOffset::block_mut`], which owns
the single-threaded-vs-tile-threaded policy. The two exist because call sites
come in two shapes — a closure suits the x86-64 and generic dispatchers, an
RAII guard suits the aarch64/wasm ones that need the slice to outlive a
borrow — but there must be exactly ONE implementation of the policy behind
them. When there were two, the aarch64 half silently kept reserving inter-row
gaps long after the x86-64 half stopped, which is the bug this whole change
exists to fix."###]
#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the native picture visitor returns its callback's existing result type"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "this compatibility entry point retains the existing positional width and height arguments"
  )
)]
pub fn with_pixel_guard_mut<BD: BitDepth, R>(
  pic: &crate::src::with_offset::WithOffset<&Rav1dPictureDataComponent>,
  w: ::console_core_api_compatibility::NativeSize,
  h: ::console_core_api_compatibility::NativeSize,
  f: impl FnOnce(
    &mut [u8],
    ::console_core_api_compatibility::NativeSize,
    ::console_core_api_compatibility::NativeOffset,
  ) -> R,
) -> R {
  let mut block = pic.block_mut::<BD>(w, h);
  let Ok(pixel_bytes) =
    console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());
  let Ok(start) = console_core_number_conversion::index(block.base.wrapping_mul(pixel_bytes));
  let Ok(pitch) = console_core_number_conversion::wrapped_offset(block.byte_stride);
  f(block.as_mut_bytes(), start, pitch)
}

#[doc = r###"
In single-threaded mode: zero-copy via `narrow_guard`.
In multi-threaded mode: copies into a compact buffer with per-row guards
(one guard per row, each covering exactly `w` pixels) so the helper is
safe to use alongside concurrent tile-thread writes on adjacent blocks.

The closure receives `(bytes, offset, stride)` — an immutable byte slice,
the byte offset to the first pixel, and the byte stride between rows.

Counterpart to [`with_pixel_guard_mut`]; see [`feedback_mt_safe_batching`]
for the rationale on why wide guards (`strided_slice_mut` / `narrow_guard`)
are unsafe under tile threading even in the immutable case (they cover
`(h-1)*stride + w` bytes and overlap with adjacent tiles' mutable ranges)."###]
#[inline]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the native picture visitor returns its callback's existing result type"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "this compatibility entry point retains the existing positional width and height arguments"
  )
)]
pub fn with_pixel_guard_immut<BD: BitDepth, R>(
  pic: &crate::src::with_offset::WithOffset<&Rav1dPictureDataComponent>,
  w: ::console_core_api_compatibility::NativeSize,
  h: ::console_core_api_compatibility::NativeSize,
  f: impl FnOnce(
    &[u8],
    ::console_core_api_compatibility::NativeSize,
    ::console_core_api_compatibility::NativeOffset,
  ) -> R,
) -> R {
  use crate::src::strided::Strided as _;
  let ::core::result::Result::Ok(pixel_size) =
    ::console_core_number_conversion::fixed_index(core::mem::size_of::<BD::Pixel>());

  let Ok(mode) = pic.data.guard_mode();

  match mode {
    PictureGuardMode::SeparateRows => {
      #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        expect(
          explicit051_no_machine_width,
          reason = "the compatibility compact view returns its native stride beside the owned bytes"
        )
      )]
      let (buf, pitch) = pic.compact_read_per_row::<BD>(w, h);
      let Ok(byte_stride) = console_core_number_conversion::fixed_offset(pitch.cast_signed());
      let Ok(stride) = console_core_number_conversion::wrapped_offset(byte_stride);
      let result = f(&buf, 0, stride);
      let Ok(()) = recycle_compact_scratch(buf);
      result
    }
    PictureGuardMode::WholeHull => {
      #[cfg_attr(
        dylint_lib = "explicit051_no_machine_width",
        expect(
          explicit051_no_machine_width,
          reason = "the compatibility hull returns the native origin of row zero alongside the guard"
        )
      )]
      let (guard, base) = pic.narrow_guard::<BD>(w, h);
      let Ok(first) = console_core_number_conversion::fixed_index(base);
      let bytes = guard.as_bytes();
      let Ok(offset) = console_core_number_conversion::index(first.wrapping_mul(pixel_size));
      let Ok(byte_stride) = console_core_number_conversion::fixed_offset(pic.stride());
      let Ok(pitch) = console_core_number_conversion::wrapped_offset(byte_stride);
      f(bytes, offset, pitch)
    }
  }
}

#[cfg(feature = "c-ffi")]
use crate::include::common::validate::validate_input;
#[cfg(feature = "c-ffi")]
use crate::include::dav1d::common::Dav1dDataProps;
use crate::include::dav1d::common::Rav1dDataProps;
use crate::include::dav1d::headers::DRav1d;
use crate::include::dav1d::headers::Dav1dFrameHeader;
use crate::include::dav1d::headers::Dav1dITUTT35;
use crate::include::dav1d::headers::Dav1dPixelLayout;
use crate::include::dav1d::headers::Dav1dSequenceHeader;
use crate::include::dav1d::headers::Rav1dContentLightLevel;
use crate::include::dav1d::headers::Rav1dFrameHeader;
use crate::include::dav1d::headers::Rav1dITUTT35;
use crate::include::dav1d::headers::Rav1dMasteringDisplay;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::headers::Rav1dSequenceHeader;
#[cfg(feature = "c-ffi")]
use crate::src::c_arc::RawArc;
use crate::src::disjoint_mut::DisjointImmutGuard;
use crate::src::disjoint_mut::DisjointMut;
use crate::src::disjoint_mut::DisjointMutGuard;
#[cfg(feature = "c-ffi")]
use crate::src::disjoint_mut::ExternalAsMutPtr;
use crate::src::disjoint_mut::SliceBounds;
#[cfg(feature = "c-ffi")]
use crate::src::error::Dav1dResult;
use crate::src::error::Rav1dError;
#[cfg(feature = "c-ffi")]
use crate::src::error::Rav1dError::EINVAL;
use crate::src::error::Rav1dResult;
use crate::src::mem::MemPool;
#[cfg(feature = "c-ffi")]
use crate::src::send_sync_non_null::{Dav1dOpaquePointer, SendSyncNonNull};
use crate::src::strided::Strided;
use crate::src::with_offset::WithOffset;
#[cfg(not(feature = "c-ffi"))]
use rav1d_disjoint_mut::PicBuf;
use std::ffi::c_int;
#[cfg(feature = "c-ffi")]
use std::ffi::c_void;
use std::iter;
use std::mem;
#[cfg(feature = "c-ffi")]
use std::ptr::NonNull;
use std::sync::Arc;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

pub const DAV1D_PICTURE_ALIGNMENT: NativeSize = 64;
pub(crate) use DAV1D_PICTURE_ALIGNMENT as RAV1D_PICTURE_ALIGNMENT;

#[doc = r###"
Uses `SendSyncNonNull` from `rav1d-disjoint-mut` so that `Send + Sync` are
automatically derived without any `unsafe impl` in this crate.

Thread safety rationale:

1. **Owned buffers**: Points into a `Vec<u8>` stored in
   the same `Rav1dPictureData` struct, behind `Arc`. The Vec cannot be
   grown or reallocated through `&Rav1dPictureData`, so the pointer stays
   valid. Concurrent access is tracked by `DisjointMut`.

2. **Borrowed scratch buffers** (`wrap_buf`): Points into a `&mut [BD::Pixel]`
   that outlives the component. These are single-threaded temporaries in
   `recon.rs` — they are never shared across threads."###]
#[cfg(feature = "c-ffi")]
#[derive(Clone, Copy)]
#[repr(transparent)]
struct PicDataPtr(SendSyncNonNull<u8>);

#[cfg(feature = "c-ffi")]
#[allow(
  unsafe_code,
  reason = "only C allocator pointers and the aligned fallback sentinel are wrapped here"
)]
impl PicDataPtr {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C allocation or borrowed scratch storage remains live and pixel access is registered with the disjoint guard"
    )
  )]
  fn from_non_null(ptr: NonNull<u8>) -> Result<Self, Never> {
    // SAFETY: C construction verifies the allocator descriptor and the scratch constructor lends an aligned mutable slice.
    let Ok(pointer) = unsafe { SendSyncNonNull::new_unchecked(ptr) };

    Ok(Self(pointer))
  }

  #[must_use]
  fn as_ptr(self) -> Result<*mut u8, Never> {
    let Ok(pointer) = self.0.as_ptr();

    Ok(pointer.as_ptr())
  }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct Dav1dPictureParameters {
  pub w: c_int,
  pub h: c_int,
  pub layout: Dav1dPixelLayout,
  pub bpc: c_int,
}

#[derive(Clone, Default)]
#[repr(C)]
#[cfg_attr(
  all(feature = "c-ffi", dylint_lib = "explicit065_checked_constructors"),
  expect(
    explicit065_checked_constructors,
    reason = "the wire conversion validates the C layout tag and bit-depth width, and Rust picture parameters already store those types"
  )
)]
pub(crate) struct Rav1dPictureParameters {
  pub w: c_int,
  pub h: c_int,
  pub layout: Rav1dPixelLayout,
  pub bpc: u8,
}

#[cfg(feature = "c-ffi")]
impl TryFrom<Dav1dPictureParameters> for Rav1dPictureParameters {
  type Error = Rav1dError;

  fn try_from(value: Dav1dPictureParameters) -> Rav1dResult<Self> {
    let Dav1dPictureParameters { w, h, layout, bpc } = value;
    let layout_2 = Rav1dPixelLayout::try_from(layout).map_err(|_| EINVAL)?;
    let bpc_2 = u8::try_from(bpc).map_err(|_| EINVAL)?;
    Ok(Self {
      w,
      h,
      layout: layout_2,
      bpc: bpc_2,
    })
  }
}

impl From<Rav1dPictureParameters> for Dav1dPictureParameters {
  fn from(value: Rav1dPictureParameters) -> Self {
    let Rav1dPictureParameters { w, h, layout, bpc } = value;
    Self {
      w,
      h,
      layout: <::core::primitive::u32 as ::core::convert::From<
        crate::include::dav1d::headers::Rav1dPixelLayout,
      >>::from(layout),
      bpc: <::core::primitive::i32 as ::core::convert::From<::core::primitive::u8>>::from(bpc),
    }
  }
}

#[cfg(feature = "c-ffi")]
#[derive(Default)]
#[repr(C)]
pub struct Dav1dPicture {
  pub seq_hdr: Option<NonNull<Dav1dSequenceHeader>>,
  pub frame_hdr: Option<NonNull<Dav1dFrameHeader>>,
  pub data: [Option<NonNull<c_void>>; 3],
  pub stride: [NativeOffset; 2],
  pub p: Dav1dPictureParameters,
  pub m: Dav1dDataProps,
  pub content_light: Option<NonNull<Rav1dContentLightLevel>>,
  pub mastering_display: Option<NonNull<Rav1dMasteringDisplay>>,
  pub itut_t35: Option<NonNull<Dav1dITUTT35>>,
  pub n_itut_t35: ::console_core_api_compatibility::NativeSize,
  pub reserved: [NativeSize; 4],
  pub frame_hdr_ref: Option<RawArc<DRav1d<Rav1dFrameHeader, Dav1dFrameHeader>>>,
  pub seq_hdr_ref: Option<RawArc<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>>>,
  pub content_light_ref: Option<RawArc<Rav1dContentLightLevel>>,
  pub mastering_display_ref: Option<RawArc<Rav1dMasteringDisplay>>,
  pub itut_t35_ref: Option<RawArc<DRav1d<Box<[Rav1dITUTT35]>, Box<[Dav1dITUTT35]>>>>,
  pub reserved_ref: [NativeSize; 4],
  pub r#ref: Option<RawArc<Rav1dPictureData>>,
  pub allocator_data: Option<Dav1dOpaquePointer>,
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "these sizes and offsets describe the target's C pointer fields and native allocation alignment"
  )
)]
mod picture_abi {
  use super::{Dav1dDataProps, Dav1dPicAllocator, Dav1dPicture, Dav1dPictureParameters};
  use console_core_api_compatibility::NativeSize;
  use core::mem::{align_of, offset_of, size_of};

  const POINTER: NativeSize = size_of::<*const ()>();
  const DATA: NativeSize = POINTER.wrapping_mul(2);
  const STRIDES: NativeSize = POINTER.wrapping_mul(5);
  const PARAMETERS: NativeSize = POINTER.wrapping_mul(7);
  const ALIGNMENT: NativeSize = align_of::<Dav1dDataProps>();
  const PROPERTIES: NativeSize = PARAMETERS.wrapping_add(16).next_multiple_of(ALIGNMENT);
  const CONTENT: NativeSize = PROPERTIES.wrapping_add(size_of::<Dav1dDataProps>());
  const DISPLAY: NativeSize = CONTENT.wrapping_add(POINTER);
  const ITUT: NativeSize = DISPLAY.wrapping_add(POINTER);
  const ITUT_COUNT: NativeSize = ITUT.wrapping_add(POINTER);
  const RESERVED: NativeSize = ITUT_COUNT.wrapping_add(POINTER);
  const FRAME_REF: NativeSize = RESERVED.wrapping_add(POINTER.wrapping_mul(4));
  const SEQUENCE_REF: NativeSize = FRAME_REF.wrapping_add(POINTER);
  const CONTENT_REF: NativeSize = SEQUENCE_REF.wrapping_add(POINTER);
  const DISPLAY_REF: NativeSize = CONTENT_REF.wrapping_add(POINTER);
  const ITUT_REF: NativeSize = DISPLAY_REF.wrapping_add(POINTER);
  const RESERVED_REF: NativeSize = ITUT_REF.wrapping_add(POINTER);
  const PICTURE_REF: NativeSize = RESERVED_REF.wrapping_add(POINTER.wrapping_mul(4));
  const ALLOCATOR_COOKIE: NativeSize = PICTURE_REF.wrapping_add(POINTER);
  const PICTURE_BYTES: NativeSize = ALLOCATOR_COOKIE
    .wrapping_add(POINTER)
    .next_multiple_of(ALIGNMENT);
  const RELEASE: NativeSize = POINTER.wrapping_mul(2);
  const ALLOCATOR_BYTES: NativeSize = POINTER.wrapping_mul(3);

  const _: () = {
    assert!(ALIGNMENT >= align_of::<*const ()>());
    assert!(size_of::<Dav1dPicture>() == PICTURE_BYTES);
    assert!(align_of::<Dav1dPicture>() == ALIGNMENT);
    assert!(offset_of!(Dav1dPicture, seq_hdr) == 0);
    assert!(offset_of!(Dav1dPicture, frame_hdr) == POINTER);
    assert!(offset_of!(Dav1dPicture, data) == DATA);
    assert!(offset_of!(Dav1dPicture, stride) == STRIDES);
    assert!(offset_of!(Dav1dPicture, p) == PARAMETERS);
    assert!(offset_of!(Dav1dPicture, m) == PROPERTIES);
    assert!(offset_of!(Dav1dPicture, content_light) == CONTENT);
    assert!(offset_of!(Dav1dPicture, mastering_display) == DISPLAY);
    assert!(offset_of!(Dav1dPicture, itut_t35) == ITUT);
    assert!(offset_of!(Dav1dPicture, n_itut_t35) == ITUT_COUNT);
    assert!(offset_of!(Dav1dPicture, reserved) == RESERVED);
    assert!(offset_of!(Dav1dPicture, frame_hdr_ref) == FRAME_REF);
    assert!(offset_of!(Dav1dPicture, seq_hdr_ref) == SEQUENCE_REF);
    assert!(offset_of!(Dav1dPicture, content_light_ref) == CONTENT_REF);
    assert!(offset_of!(Dav1dPicture, mastering_display_ref) == DISPLAY_REF);
    assert!(offset_of!(Dav1dPicture, itut_t35_ref) == ITUT_REF);
    assert!(offset_of!(Dav1dPicture, reserved_ref) == RESERVED_REF);
    assert!(offset_of!(Dav1dPicture, r#ref) == PICTURE_REF);
    assert!(offset_of!(Dav1dPicture, allocator_data) == ALLOCATOR_COOKIE);

    assert!(size_of::<Dav1dPicAllocator>() == ALLOCATOR_BYTES);
    assert!(align_of::<Dav1dPicAllocator>() == align_of::<*const ()>());
    assert!(offset_of!(Dav1dPicAllocator, cookie) == 0);
    assert!(offset_of!(Dav1dPicAllocator, alloc_picture_callback) == POINTER);
    assert!(offset_of!(Dav1dPicAllocator, release_picture_callback) == RELEASE);

    assert!(size_of::<Dav1dPictureParameters>() == 16);
    assert!(align_of::<Dav1dPictureParameters>() == 4);
    assert!(offset_of!(Dav1dPictureParameters, w) == 0);
    assert!(offset_of!(Dav1dPictureParameters, h) == 4);
    assert!(offset_of!(Dav1dPictureParameters, layout) == 8);
    assert!(offset_of!(Dav1dPictureParameters, bpc) == 12);
  };
}

#[derive(Clone, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(64))]
pub struct AlignedPixelChunk([u8; RAV1D_PICTURE_ALIGNMENT]);

const _: () = assert!(mem::align_of::<AlignedPixelChunk>() == RAV1D_PICTURE_ALIGNMENT);
const _: () = assert!(mem::size_of::<AlignedPixelChunk>() == RAV1D_PICTURE_ALIGNMENT);

#[doc = r###"Constructors check this before exposing a picture component."###]
const RAV1D_PICTURE_GUARANTEED_MULTIPLE: u64 = 64;

#[doc = r###"as this is guaranteed by [`Rav1dPicAllocator::alloc_picture_callback`],
though wrapped buffers may only be [`RAV1D_PICTURE_GUARANTEED_MULTIPLE`]."###]
const RAV1D_PICTURE_MULTIPLE: u64 = 4096;

#[doc = r###"
For c-ffi: a struct with raw pointer, length, and stride (supports C allocator callbacks).
Without c-ffi: aliases [`StridedBuf`] from the disjoint-mut crate (all unsafe confined there)."###]
#[cfg(feature = "c-ffi")]
pub struct Rav1dPictureDataComponentInner {
  #[doc = r###"even if [`Self::stride`] is negative.

It is aligned to [`RAV1D_PICTURE_ALIGNMENT`]."###]
  ptr: PicDataPtr,

  #[doc = r###"
It is a multiple of [`RAV1D_PICTURE_GUARANTEED_MULTIPLE`]."###]
  len: u64,

  stride: i64,
}

#[cfg(feature = "c-ffi")]
#[derive(Clone, Copy)]
struct CPlaneLayout {
  length: u64,
  stride: i64,
}

#[doc = r###"All unsafe for `AsMutPtr` is confined to that crate. Stride is stored separately
in [`Rav1dPictureDataComponent`]."###]
#[cfg(not(feature = "c-ffi"))]
pub type Rav1dPictureDataComponentInner = PicBuf;

#[cfg(feature = "c-ffi")]
impl Rav1dPictureDataComponentInner {
  #[doc = r###"
# Safety

`ptr`, `len`, and `stride` must follow the requirements of [`Dav1dPicAllocator::alloc_picture_callback`]."###]
  unsafe fn new(ptr: Option<NonNull<u8>>, layout: CPlaneLayout) -> Rav1dResult<Self> {
    let CPlaneLayout { length, stride } = layout;
    let pointer = match ptr {
      None => {
        let Ok(dangling) =
          PicDataPtr::from_non_null(NonNull::<AlignedPixelChunk>::dangling().cast());

        return Ok(Self {
          ptr: dangling,
          len: 0,
          stride,
        });
      }
      Some(address) => address,
    };

    match (
      length,
      length.wrapping_rem(RAV1D_PICTURE_MULTIPLE),
      pointer.cast::<AlignedPixelChunk>().is_aligned(),
    ) {
      (1.., 0, true) => {}
      _ => return Err(EINVAL),
    }

    let count = console_core_number_conversion::checked_index(length).map_err(|_| EINVAL)?;
    std::alloc::Layout::array::<u8>(count).map_err(|_| EINVAL)?;

    let start = match stride {
      i64::MIN..=-1 => {
        let distance = length.checked_sub(stride.unsigned_abs()).ok_or(EINVAL)?;
        let Ok(offset) = console_core_number_conversion::index(distance);
        let address = NonNull::new(pointer.as_ptr().wrapping_sub(offset)).ok_or(EINVAL)?;

        match address
          .cast::<AlignedPixelChunk>()
          .is_aligned()
          .then_some(())
        {
          Some(()) => address,
          None => return Err(EINVAL),
        }
      }
      0.. => pointer,
    };
    let Ok(ptr_2) = PicDataPtr::from_non_null(start);

    Ok(Self {
      ptr: ptr_2,
      len: length,
      stride,
    })
  }

  #[doc = r###"
As opposed to [`Self::new`], this is safe because `buf` is a `&mut` and thus unique,
so it is sound to further subdivide it into disjoint `&mut`s.

# Panics

`buf` must satisfy the same two invariants [`Self::new`] enforces on the
C allocator's buffers, because this constructs the same type and the
same [`ExternalAsMutPtr`] impl exposes the same storage to the decoder:

1. it must START on a [`RAV1D_PICTURE_ALIGNMENT`]-byte boundary, and
2. its BYTE length must be a multiple of
   [`RAV1D_PICTURE_GUARANTEED_MULTIPLE`].

Neither can be relaxed into a copy here: this path is zero-copy on
purpose, and its callers (`recon.rs`'s OBMC `lap` and interintra `tmp`)
read their results back out of `buf` afterwards — which is why the
safe-mode twin's `copy_pixels_to` call sites are
`cfg(not(feature = "c-ffi"))`.

A plain `Vec<BD::Pixel>` satisfies (1) only by luck. Every production
caller satisfies it as a TYPE property: the scratch buffers in
`src/internal.rs` are `#[repr(C, align(64))]`, pinned by the
`scratch_alignment` assertions there."###]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported slice-to-picture adapter returns the existing picture component type directly"
    )
  )]
  pub fn wrap_buf<BD: BitDepth>(
    buf: &mut [BD::Pixel],
    stride: ::console_core_api_compatibility::NativeSize,
  ) -> Self {
    let bytes = IntoBytes::as_mut_bytes(buf);
    let pointer = NonNull::from(&mut *bytes).cast::<u8>();
    assert!(pointer.cast::<AlignedPixelChunk>().is_aligned());
    let Ok(length) = console_core_number_conversion::fixed_index(bytes.len());
    assert!(length.wrapping_rem(RAV1D_PICTURE_GUARANTEED_MULTIPLE) == 0);
    let Ok(samples) = console_core_number_conversion::fixed_offset(stride.cast_signed());
    let Ok(pixel_bytes) =
      console_core_number_conversion::fixed_index(mem::size_of::<BD::Pixel>());
    let signed_stride = samples.wrapping_mul(pixel_bytes.cast_signed());
    let Ok(ptr) = PicDataPtr::from_non_null(pointer);

    Self {
      ptr,
      len: length,
      stride: signed_stride,
    }
  }
}

#[cfg(feature = "c-ffi")]
#[allow(
  unsafe_code,
  reason = "the external buffer adapter lends the address while disjoint borrow guards reserve its bytes"
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the external decoder allocator hands off a raw pixel address through the disjoint guard's unsafe interoperability trait"
  )
)]
// SAFETY: The component stores the external address as a raw pointer. The adapter returns
// that address without creating a mutable reference to any of the reserved pixel bytes.
unsafe impl ExternalAsMutPtr for Rav1dPictureDataComponentInner {
  type Target = u8;

  #[inline]
  unsafe fn as_mut_ptr(ptr: *mut Self) -> *mut Self::Target {
    // SAFETY: Safe to dereference by unsafe preconditions.
    // Since we don't store any `&mut`s, just a raw ptr, we can have a `&Self`.
    let this = unsafe { &*ptr };

    let Ok(pointer) = this.ptr.as_ptr();
    pointer
  }

  unsafe fn as_mut_slice(ptr: *mut Self) -> *mut [Self::Target] {
    // SAFETY: Only creates &Self (SharedReadOnly). Data is behind a raw pointer,
    // not inline, so SharedReadOnly doesn't cover element data.
    let this = unsafe { &*ptr };

    let Ok(pointer) = this.ptr.as_ptr();
    let Ok(length) = console_core_number_conversion::index(this.len);
    core::ptr::slice_from_raw_parts_mut(pointer, length)
  }

  #[inline]
  fn len(&self) -> ::console_core_api_compatibility::NativeSize {
    let Ok(length) = console_core_number_conversion::index(self.len);
    length
  }
}

#[doc = "The worker and tile counts determine the reservation granularity and the overlap tracker's scheduling configuration."]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PictureThreading {
  OneWorkerOneTile,
  OneWorkerMultipleTiles,
  MultipleWorkersOneTile,
  MultipleWorkersMultipleTiles,
}

pub(crate) struct PictureThreadingCounts {
  pub workers: u64,
  pub tiles: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PictureGuardMode {
  WholeHull,
  SeparateRows,
}

impl PictureThreading {
  #[must_use]
  pub(crate) fn new(counts: PictureThreadingCounts) -> Result<Self, Never> {
    Ok(match (counts.workers, counts.tiles) {
      (0..=1, 0..=1) => Self::OneWorkerOneTile,
      (0..=1, 2..) => Self::OneWorkerMultipleTiles,
      (2.., 0..=1) => Self::MultipleWorkersOneTile,
      (2.., 2..) => Self::MultipleWorkersMultipleTiles,
    })
  }
}

#[doc = r###"
For c-ffi: stride is stored inside the inner type.
Without c-ffi: stride is stored alongside the [`DisjointMut<PicBuf>`]."###]
#[cfg(feature = "c-ffi")]
pub struct Rav1dPictureDataComponent {
  data: DisjointMut<Rav1dPictureDataComponentInner>,
  threading: Option<PictureThreading>,
}

#[cfg(not(feature = "c-ffi"))]
pub struct Rav1dPictureDataComponent {
  data: DisjointMut<Rav1dPictureDataComponentInner>,
  stride: i64,
  threading: Option<PictureThreading>,
}

impl Rav1dPictureDataComponent {
  #[doc = r###"allocations install a policy while still exclusively owned."###]
  #[inline(always)]
  #[must_use]
  pub(crate) fn guard_mode(&self) -> Result<PictureGuardMode, Never> {
    Ok(match self.threading {
      Some(PictureThreading::OneWorkerOneTile | PictureThreading::OneWorkerMultipleTiles) => {
        PictureGuardMode::WholeHull
      }
      Some(
        PictureThreading::MultipleWorkersOneTile | PictureThreading::MultipleWorkersMultipleTiles,
      ) => PictureGuardMode::SeparateRows,
      None => match tile_threading_active().then_some(()) {
        Some(()) => PictureGuardMode::SeparateRows,
        None => PictureGuardMode::WholeHull,
      },
    })
  }

  #[must_use]
  pub(crate) fn threading_policy(&self) -> Result<Option<PictureThreading>, Never> {
    Ok(self.threading)
  }

  pub(crate) fn set_threading_policy(&mut self, policy: PictureThreading) -> Result<(), Never> {
    match policy {
      PictureThreading::OneWorkerOneTile => self.data.configure_parallelism(1, 1),
      PictureThreading::OneWorkerMultipleTiles => self.data.configure_parallelism(1, 2),
      PictureThreading::MultipleWorkersOneTile => self.data.configure_parallelism(2, 1),
      PictureThreading::MultipleWorkersMultipleTiles => self.data.configure_parallelism(2, 2),
    }

    self.threading = Some(policy);
    Ok(())
  }

  #[inline(always)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the native disjoint-buffer adapter lends the existing external guard container directly"
    )
  )]
  pub(crate) fn dm(&self) -> &DisjointMut<Rav1dPictureDataComponentInner> {
    &self.data
  }

  #[doc = r###"For non-c-ffi, stride is stored separately."###]
  #[cfg(feature = "c-ffi")]
  #[must_use]
  fn from_parts(inner: Rav1dPictureDataComponentInner) -> Result<Self, Never> {
    let stride = inner.stride;
    let Ok(data) = crate::src::disjoint_mut::dm_new(inner);
    let mut component = Self {
      data,
      threading: None,
    };
    let Ok(()) = component.declare_stride(stride);

    Ok(component)
  }

  #[cfg(not(feature = "c-ffi"))]
  #[must_use]
  pub(crate) fn from_parts(
    inner: Rav1dPictureDataComponentInner,
    stride: i64,
  ) -> Result<Self, Never> {
    let Ok(data) = crate::src::disjoint_mut::dm_new(inner);
    let mut component = Self {
      data,
      threading: None,
      stride,
    };
    let Ok(()) = component.declare_stride(stride);

    Ok(component)
  }

  fn declare_stride(&mut self, stride: i64) -> Result<(), Never> {
    let Ok(row_bytes) = console_core_number_conversion::index(stride.unsigned_abs());
    let Ok(pitch) = console_core_number_conversion::wrapped_offset(stride);
    self.data.declare_row_stride(row_bytes);
    self.data.probe_declare_stride(pitch);
    Ok(())
  }

  #[doc = r###"
Returns `Some(vec)` for owned allocations (from `alloc_picture_data`),
`None` for borrowed scratch buffers (from `wrap_buf`).
Used by [`Rav1dPictureData::drop`] to return buffers to the memory pool."###]
  #[cfg(not(feature = "c-ffi"))]
  fn take_buf(&mut self) -> Result<Option<Vec<u8>>, Never> {
    Ok(self.data.get_mut().take_buf())
  }

  #[doc = r###"
In c-ffi mode: wraps a raw pointer into the caller's buffer (zero-copy).
In safe mode: copies the data into an owned `Vec<u8>` (no raw pointers,
auto `Send + Sync`). For dst-scratch usage, call [`copy_pixels_to`] after
writing to retrieve the results.

# Panics

`buf`'s BYTE length must be a multiple of
`RAV1D_PICTURE_GUARANTEED_MULTIPLE` (64) in every configuration, and
under `c-ffi` it must additionally START on a
`RAV1D_PICTURE_ALIGNMENT`-byte (64) boundary — the zero-copy path keeps
the caller's pointer, and that
alignment is a type invariant of `Rav1dPictureDataComponentInner`. See
that type's `wrap_buf` (c-ffi only) for the full rationale.

The default build copies, so it does NOT check the alignment rule. A
caller that only ever runs without `c-ffi` can therefore violate it
undetected; test harnesses should allocate through
`crate::src::safe_simd::aligned_plane`.

[`copy_pixels_to`]: Self::copy_pixels_to"###]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported slice-to-picture adapter returns the existing picture component type directly"
    )
  )]
  pub fn wrap_buf<BD: BitDepth>(
    buf: &mut [BD::Pixel],
    stride: ::console_core_api_compatibility::NativeSize,
  ) -> Self {
    #[cfg(feature = "c-ffi")]
    let Ok(mut this) =
      Self::from_parts(Rav1dPictureDataComponentInner::wrap_buf::<BD>(buf, stride));
    #[cfg(not(feature = "c-ffi"))]
    let Ok(mut this) = {
      let bytes = IntoBytes::as_bytes(buf);
      let Ok(length) = console_core_number_conversion::fixed_index(bytes.len());
      assert!(length.wrapping_rem(RAV1D_PICTURE_GUARANTEED_MULTIPLE) == 0);
      let Ok(samples) = console_core_number_conversion::fixed_offset(stride.cast_signed());
      let Ok(pixel_bytes) =
        console_core_number_conversion::fixed_index(mem::size_of::<BD::Pixel>());
      let pitch = samples.wrapping_mul(pixel_bytes.cast_signed());

      Self::from_parts(PicBuf::from_slice_copy(bytes), pitch)
    };

    this.data.configure_parallelism(1, 1);
    this
  }

  #[doc = r###"
Used after MC/ipred writes into a copy-backed scratch component
to retrieve the results for subsequent operations (e.g., blend)."###]
  #[cfg(not(feature = "c-ffi"))]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported transfer adapter uses the established native pixel slice and unit return"
    )
  )]
  pub fn copy_pixels_to<BD: BitDepth>(&self, dst: &mut [BD::Pixel]) {
    let guard = self.slice::<BD, _>(..);
    let Ok(output) =
      require_picture_layout(dst.get_mut(..guard.len()).ok_or(RowLayoutError::Bounds));
    let Ok(()) = require_picture_layout(copy_picture_samples(output, &guard));
  }
}

#[cfg(feature = "c-ffi")]
#[allow(
  unsafe_code,
  reason = "the C-backed stride is read from its descriptor alongside the tracked pixel allocation"
)]
impl Strided for Rav1dPictureDataComponent {
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the descriptor's immutable stride is read at the C disjoint-allocation boundary"
    )
  )]
  fn fixed_stride(&self) -> Result<i64, Never> {
    // SAFETY: The component owns the descriptor. The stride field is immutable while the view is live.
    let stride = unsafe { (*self.dm().inner()).stride };
    Ok(stride)
  }
}

#[cfg(not(feature = "c-ffi"))]
impl Strided for Rav1dPictureDataComponent {
  fn fixed_stride(&self) -> Result<i64, Never> {
    Ok(self.stride)
  }
}

impl Rav1dPictureDataComponent {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported picture buffer length accessor returns the native slice length"
    )
  )]
  pub fn byte_len(&self) -> ::console_core_api_compatibility::NativeSize {
    self.dm().len()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported pointer identity predicate returns the conventional boolean for picture references"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit007_no_bool_return",
    expect(
      explicit007_no_bool_return,
      reason = "existing compatibility callers consume this native predicate as a boolean"
    )
  )]
  pub fn ref_eq(&self, other: &Self) -> bool {
    self.dm().as_mut_ptr() == other.dm().as_mut_ptr()
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported picture buffer length accessor returns the native pixel count"
    )
  )]
  pub fn pixel_len<BD: BitDepth>(&self) -> ::console_core_api_compatibility::NativeSize {
    let Ok(length) = console_core_number_conversion::fixed_index(self.dm().len());
    let Ok(pixel_bytes) =
      console_core_number_conversion::fixed_index(mem::size_of::<BD::Pixel>());
    let count = length
      .checked_div(pixel_bytes)
      .ok_or(RowLayoutError::Stride);
    let Ok(pixels) = require_picture_layout(count);
    let Ok(samples) = console_core_number_conversion::index(pixels);
    samples
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported guard offset accessor returns the native slice index"
    )
  )]
  pub fn pixel_offset<BD: BitDepth>(&self) -> ::console_core_api_compatibility::NativeSize {
    let Ok(stride) = console_core_number_conversion::fixed_offset(self.stride());

    match stride {
      0.. => 0,
      i64::MIN..=-1 => {
        let Ok(length) = console_core_number_conversion::fixed_index(self.byte_len());
        let first = length.wrapping_sub(stride.unsigned_abs());
        let Ok(offset) = console_core_number_conversion::wrapped_index(first);
        BD::pxstride(offset)
      }
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported picture view constructor preserves the original borrowed view type"
    )
  )]
  pub fn with_offset<BD: BitDepth>(&self) -> Rav1dPictureDataComponentOffset<'_> {
    Rav1dPictureDataComponentOffset {
      data: self,
      offset: self.pixel_offset::<BD>(),
    }
  }

  #[doc = r###"
Only used by asm (mc emu_edge) and c-ffi (Dav1dPicture conversion)."###]
  #[cfg(feature = "c-ffi")]
  #[must_use]
  fn as_strided_byte_mut_ptr(&self) -> Result<*mut u8, Never> {
    let ptr = self.dm().as_mut_ptr();
    let Ok(stride) = console_core_number_conversion::fixed_offset(self.stride());

    Ok(match stride {
      i64::MIN..=-1 => {
        let Ok(pitch) = console_core_number_conversion::wrapped_offset(stride);
        ptr.wrapping_add(self.byte_len()).wrapping_offset(pitch)
      }
      0.. => ptr,
    })
  }

  #[cfg(feature = "c-ffi")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this C picture adapter returns the established borrowed plane pointer"
    )
  )]
  pub fn as_strided_mut_ptr<BD: BitDepth>(&self) -> *mut BD::Pixel {
    // SAFETY: Transmutation is safe because we verify this with `zerocopy` in `Self::slice`.
    let Ok(pointer) = self.as_strided_byte_mut_ptr();
    pointer.cast()
  }

  #[cfg(feature = "c-ffi")]
  #[must_use]
  fn as_dav1d(&self) -> Result<Option<NonNull<c_void>>, Never> {
    Ok(match self.byte_len() {
      0 => None,
      _ => {
        let Ok(pointer) = self.as_strided_byte_mut_ptr();
        NonNull::new(pointer.cast())
      }
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this exported copy adapter preserves the established picture transfer signature"
    )
  )]
  pub fn copy_from(&self, src: &Self) {
    let dst = &mut *self.dm().index_mut(..);
    let src_2 = &*src.dm().index(..);
    dst.clone_from_slice(src_2);
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint guard"
    )
  )]
  pub fn index<'a, BD: BitDepth>(
    &'a self,
    index: ::console_core_api_compatibility::NativeSize,
  ) -> DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, BD::Pixel> {
    self.dm().element_as(index)
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint guard"
    )
  )]
  pub fn index_mut<'a, BD: BitDepth>(
    &'a self,
    index: ::console_core_api_compatibility::NativeSize,
  ) -> DisjointMutGuard<'a, Rav1dPictureDataComponentInner, BD::Pixel> {
    self.dm().mut_element_as(index)
  }

  #[doc = r###"Check a tracked picture-plane reservation against the debug budget.

> While tile threading is active, a **partial** reservation against a
> picture plane may not exceed [`TILE_THREADED_PIC_EXTENT_MAX_BYTES`].

# Why this invariant and not "reserved <= footprint"

`docs/BOUNDS_MAP.md` measured both halves. At t=8 the ratio half is already
1.000 at every hot site — there is no over-reservation left to assert
against — while the *absolute* extent is what decides whether a reservation
collides with another worker's write. The three refuted attempts
(#469 strided rectangle, #475 hull, #485 read band) all widened the absolute
extent; two of them kept `reserved == footprint` while doing it, so a ratio
assertion would have passed all three. The measured headroom at the decoder's
largest site (`loopfilter.rs:710:14`) is **60 bytes** to a concurrent
`loopfilter.rs:887:14` write-back, so a ceiling in that neighbourhood is the
property that actually gates the failure.

# What is exempt, and why that is not a hole

* **Whole-component reservations** (`full_guard`, `full_guard_mut`,
  `copy_pixels_to`, `copy_from`). A reservation of the entire plane is
  unambiguous, greppable and deliberate; the map measured those sites
  (`mc.rs:121:61`, `mc.rs:1342:44`, `mc_arm.rs:5971:41`) as having **no
  concurrent foreign write at any distance**, because they read reference
  frames that are immutable for the whole decode. They are a tracker-COST
  question, not a conflict question.
* **Bounded reference-source reads** in `safe_simd/mc_reference.rs`. These
  narrow the formerly whole-component x86 MC reads to at most 135x135
  pixels including taps. Their complete contiguous hull remains tracked,
  including row gaps; see `audit/concurrency-fixes/README.md`. They use a
  separate private helper and cannot widen reconstruction writes.
* **Single-element access** (`index`, `index_mut`). One element is the
  smallest reservation expressible; there is nothing to widen.
* **Everything at t=1.** The hull paths deliberately over-reserve by
  153x-1680x there and it is correct: with no second worker there is nobody
  for the inter-row gaps to collide with, and the count reduction is worth
  2.6% (`docs/MUT_RECON_KERNELS.md` §11d).

# Cost

Compiled under `debug_assertions`. The release build does not inspect
the reservation or read the thread policy at this call site."###]
  #[inline]
  #[track_caller]
  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "an internal guard exceeding the measured debug instrumentation budget is a decoder invariant violation"
    )
  )]
  fn note_pic_extent<BD: BitDepth>(&self, index: &impl SliceBounds) -> Result<(), Never> {
    let check_diagnostic_budget = cfg!(debug_assertions);

    match ::core::primitive::bool::then_some(check_diagnostic_budget, ()) {
      ::core::option::Option::None => return Ok(()),
      ::core::option::Option::Some(()) => {}
    }

    let Ok(total) = console_core_number_conversion::fixed_index(self.pixel_len::<BD>());
    let Ok(slice_length) = console_core_number_conversion::index(total);
    #[cfg_attr(
      dylint_lib = "explicit051_no_machine_width",
      expect(
        explicit051_no_machine_width,
        reason = "SliceBounds describes its source slice using native Rust range endpoints"
      )
    )]
    let range = index.clone().to_range(slice_length);
    let Ok(start) = console_core_number_conversion::fixed_index(range.start);
    let Ok(end) = console_core_number_conversion::fixed_index(range.end);

    match (
      start == 0 && end == total,
      matches!(self.guard_mode(), Ok(PictureGuardMode::SeparateRows)),
    ) {
      (false, true) => {}
      (true, _) | (false, false) => return Ok(()),
    }

    let Ok(pixel_size) = console_core_number_conversion::fixed_index(mem::size_of::<BD::Pixel>());
    let Ok(stride) =
      ::console_core_number_conversion::fixed_offset(match self.pixel_stride::<BD>() {
        ::core::result::Result::Ok(stride) => stride,
      });
    let Ok(pixel_stride) = console_core_number_conversion::fixed_offset(
      match ::console_core_number_conversion::wrapped_offset(stride) {
        ::core::result::Result::Ok(native_position) => native_position,
      },
    );
    let row_bytes = pixel_stride.unsigned_abs().wrapping_mul(pixel_size);
    let bytes = end.saturating_sub(start).wrapping_mul(pixel_size);
    let loc = core::panic::Location::caller();
    let Ok(row_byte_count) = console_core_number_conversion::index(row_bytes);
    let Ok(ceiling) =
      console_core_number_conversion::fixed_index(pic_extent_ceiling(loc.file(), row_byte_count));

    match bytes.cmp(&ceiling) {
      core::cmp::Ordering::Greater => {
        let rows = match row_bytes {
          0 => 1,
          _ => bytes.div_ceil(row_bytes),
        };
        panic!(
          "{}:{}:{} took a {bytes} B picture-plane reservation while tile \
               threading is active; the measured ceiling for that file is \
               {ceiling} B.\n\
               This is the bounds map's standing extent invariant \
               (docs/BOUNDS_MAP.md, PIC_EXTENT_CEILINGS). A wider reservation \
               collides with a concurrent foreign WRITE at the rate the map's \
               widening-budget column predicts — the decoder's largest guard \
               site has 60 bytes of measured headroom, and the three refuted \
               attempts (#469 rectangle, #475 hull, #485 band) all failed \
               exactly here.\n\
               If the widening is deliberate: price it against the budget \
               table FIRST (`--features __probe_bounds`), then raise the \
               ceiling WITH the measurement in the same commit. \
               (one picture row here is {row_bytes} B; this reservation spans \
               {rows} rows)",
          loc.file(),
          loc.line(),
          loc.column()
        );
      }
      core::cmp::Ordering::Equal | core::cmp::Ordering::Less => {}
    }

    Ok(())
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint slice guard"
    )
  )]
  pub fn slice<'a, BD, I>(
    &'a self,
    index: I,
  ) -> DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>
  where
    BD: BitDepth,
    I: SliceBounds,
  {
    let Ok(()) = self.note_pic_extent::<BD>(&index);

    self.dm().slice_as(index)
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint slice guard"
    )
  )]
  pub fn slice_mut<'a, BD, I>(
    &'a self,
    index: I,
  ) -> DisjointMutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>
  where
    BD: BitDepth,
    I: SliceBounds,
  {
    let Ok(()) = self.note_pic_extent::<BD>(&index);

    self.dm().mut_slice_as(index)
  }
}

pub type Rav1dPictureDataComponentOffset<'a> = WithOffset<&'a Rav1dPictureDataComponent>;

struct CompactPictureRows {
  samples: Size<u64>,
  base: u64,
  stride: i64,
  pixel_bytes: u64,
  row_bytes: u64,
}

enum CompactPictureAccess {
  Read,
  Write,
}

struct CompactPictureRow {
  plane: Range<u64>,
  buffer: Range<u64>,
}

impl CompactPictureRows {
  #[inline]
  fn new<BD: BitDepth>(
    component: &Rav1dPictureDataComponentOffset<'_>,
    samples: Size<u64>,
  ) -> Result<Self, Never> {
    let Ok(pixel_bytes) =
      console_core_number_conversion::fixed_index(mem::size_of::<BD::Pixel>());
    let Ok(base) = console_core_number_conversion::fixed_index(component.offset);
    let Ok(stride) =
      console_core_number_conversion::fixed_offset(match component.data.pixel_stride::<BD>() {
        Ok(samples_per_row) => samples_per_row,
      });
    Ok(Self {
      samples,
      base,
      stride,
      pixel_bytes,
      row_bytes: samples.width.wrapping_mul(pixel_bytes),
    })
  }

  #[inline]
  #[track_caller]
  fn probe(
    &self,
    data: &Rav1dPictureDataComponent,
    access: CompactPictureAccess,
  ) -> Result<(), Never> {
    let Ok(offset) =
      console_core_number_conversion::wrapped_index(self.base.wrapping_mul(self.pixel_bytes));
    let Ok(row_bytes) = console_core_number_conversion::wrapped_index(self.row_bytes);
    let Ok(height) = console_core_number_conversion::wrapped_index(self.samples.height);
    let stride_bytes = self.stride.wrapping_mul(self.pixel_bytes.cast_signed());
    let Ok(stride) = console_core_number_conversion::wrapped_offset(stride_bytes);
    data.dm().probe_eval_rect(
      core::panic::Location::caller(),
      matches!(access, CompactPictureAccess::Write),
      offset,
      row_bytes,
      height,
      stride,
    );
    Ok(())
  }

  #[inline]
  fn row(&self, ordinal: u64) -> Result<CompactPictureRow, Never> {
    let plane_offset = self
      .base
      .wrapping_add_signed(ordinal.cast_signed().wrapping_mul(self.stride));
    let buffer_offset = ordinal.wrapping_mul(self.row_bytes);
    let buffer_end = buffer_offset.wrapping_add(self.row_bytes);
    Ok(CompactPictureRow {
      plane: plane_offset..plane_offset.wrapping_add(self.samples.width),
      buffer: buffer_offset..buffer_end,
    })
  }
}

struct PictureBlockHull {
  extent: Size<u64>,
  start: u64,
  end: u64,
  first: u64,
  pixel_stride: i64,
}

impl PictureBlockHull {
  fn probe<BD: BitDepth>(
    &self,
    mut declare: impl FnMut(u64, u64, u64, i64),
  ) -> Result<(), Never> {
    let Ok(pixel_bytes) =
      console_core_number_conversion::fixed_index(mem::size_of::<BD::Pixel>());
    declare(
      self.start.wrapping_mul(pixel_bytes),
      self.extent.width.wrapping_mul(pixel_bytes),
      self.extent.height,
      self.pixel_stride.wrapping_mul(pixel_bytes.cast_signed()),
    );
    Ok(())
  }
}

enum GuardAccess {
  Shared,
  Mutable,
}

struct GuardedRows {
  extent: Size<u64>,
  origin: u64,
  pitch: i64,
  mode: PictureGuardMode,
}

#[track_caller]
#[cfg_attr(
  dylint_lib = "explicit004_no_panic",
  expect(
    explicit004_no_panic,
    reason = "the existing synchronous picture view contract rejects invalid geometry before returning a guard"
  )
)]
fn require_picture_layout<T>(result: Result<T, RowLayoutError>) -> Result<T, Never> {
  Ok(result.expect("picture buffer must cover its declared geometry"))
}

fn copy_picture_samples<T: Copy>(
  destination: &mut [T],
  source: &[T],
) -> Result<(), RowLayoutError> {
  match destination.len().cmp(&source.len()) {
    core::cmp::Ordering::Equal => {}
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
      return Err(RowLayoutError::Bounds);
    }
  }

  for (output, &input) in destination.iter_mut().zip(source) {
    *output = input;
  }

  Ok(())
}

impl<'a> Rav1dPictureDataComponentOffset<'a> {
  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint guard"
    )
  )]
  pub fn index<BD: BitDepth>(
    &self,
  ) -> DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, BD::Pixel> {
    self.data.index::<BD>(self.offset)
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint guard"
    )
  )]
  pub fn index_mut<BD: BitDepth>(
    &self,
  ) -> DisjointMutGuard<'a, Rav1dPictureDataComponentInner, BD::Pixel> {
    self.data.index_mut::<BD>(self.offset)
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint slice guard"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn slice<BD: BitDepth>(
    &self,
    len: NativeSize,
  ) -> DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]> {
    self.data.slice::<BD, _>((self.offset.., ..len))
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this picture access adapter returns the existing native disjoint slice guard"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn slice_mut<BD: BitDepth>(
    &self,
    len: NativeSize,
  ) -> DisjointMutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]> {
    self.data.slice_mut::<BD, _>((self.offset.., ..len))
  }

  #[doc = r###"
Handles both positive and negative strides. The returned guard covers
all pixels that would be accessed by iterating h rows with the given
pixel stride, each row being w pixels wide.

Returns `(guard, base_offset_within_guard)` where `base_offset_within_guard`
is the index within the guard's slice that corresponds to `self.offset`."###]
  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported picture guard keeps the native guard and base-index contract"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn strided_slice_mut<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
  ) -> (
    DisjointMutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>,
    NativeSize,
  ) {
    self.narrow_guard_mut::<BD>(w, h)
  }

  #[doc = r###"write other blocks on the same rows.

# Why this exists

[`Self::strided_slice_mut`] reserves ONE contiguous range covering the
whole strided span, `(h - 1) * stride + w` pixels — everything from the
block's first pixel to its last, INCLUDING the inter-row gaps, which
belong to other blocks. That is correct single-threaded and wrong under
tile threading: AV1 tiles partition a frame by columns, so two tile
workers routinely write the SAME rows at different columns. The second
one lands in the first one's reserved gap and `DisjointMut` panics —
a false positive, since the two writes are genuinely disjoint.

Concretely, on a 3840-wide frame a 16×16 block reserves 57,616 bytes in
order to write 256, and a neighbouring 16-byte intra-prediction write
anywhere in those rows trips it. (zenavif#30; the same defect class the
loopfilter and `ipred` compact paths already fixed for themselves.)

# What it does

* Tile threading OFF — hands back exactly what `strided_slice_mut`
  does: one contiguous guard, the picture's own stride, no copying.
  Byte-identical behavior and identical cost to before.
* Tile threading ON — copies the block into a compact `w × h` scratch
  buffer through PER-ROW guards (each reserving only the `w` pixels
  actually touched), lets the caller work on that, and writes it back
  per row on drop. No gap is ever reserved, so disjoint neighbours
  cannot collide.

Callers must take the stride from [`BlockMut::byte_stride`] rather than
from the picture, because the compact buffer has its own stride."###]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported native picture accessor returns the existing RAII write guard"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn block_mut<BD: BitDepth>(&self, w: NativeSize, h: NativeSize) -> BlockMut<'a, BD> {
    let Ok(width) = console_core_number_conversion::fixed_index(w);
    let Ok(height) = console_core_number_conversion::fixed_index(h);
    let extent = Size { width, height };

    match ::core::primitive::bool::then_some(
      matches!(self.data.guard_mode(), Ok(PictureGuardMode::SeparateRows)),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        let (buf, byte_stride) = self.compact_read_per_row::<BD>(w, h);
        let Ok(pitch) = console_core_number_conversion::fixed_index(byte_stride);
        BlockMut {
          storage: BlockMutStorage::Compact { buf },
          dst: *self,
          extent,
          base: 0,
          byte_stride: pitch.cast_signed(),
        }
      }
      ::core::option::Option::None => {
        use crate::src::strided::Strided as _;
        let Ok(byte_stride) = console_core_number_conversion::fixed_offset(self.stride());
        let (guard, base) = self.narrow_guard_mut::<BD>(w, h);
        let Ok(first) = console_core_number_conversion::fixed_index(base);
        BlockMut {
          storage: BlockMutStorage::Direct { guard },
          dst: *self,
          extent,
          base: first,
          byte_stride,
        }
      }
    }
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported picture guard keeps the native guard and base-index contract"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn strided_slice<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
  ) -> (
    DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>,
    NativeSize,
  ) {
    self.narrow_guard::<BD>(w, h)
  }

  #[doc = "Finds the physical storage between the lowest row start and the end of the highest row, preserving the first logical row's offset for either stride direction."]
  #[inline]
  fn block_hull<BD: BitDepth>(
    &self,
    extent: Size<u64>,
  ) -> Result<PictureBlockHull, RowLayoutError> {
    let Ok(origin) = console_core_number_conversion::fixed_index(self.offset);
    let Ok(pixel_stride) =
      console_core_number_conversion::fixed_offset(match self.data.pixel_stride::<BD>() {
        Ok(samples) => samples,
      });

    match (extent.width, extent.height) {
      (0, _) | (_, 0) => {
        return Ok(PictureBlockHull {
          extent,
          start: origin,
          end: origin,
          first: 0,
          pixel_stride,
        });
      }
      _ => {}
    }

    let distance = extent
      .height
      .wrapping_sub(1)
      .checked_mul(pixel_stride.unsigned_abs())
      .ok_or(RowLayoutError::Bounds)?;
    let (start, first) = match pixel_stride {
      i64::MIN..=-1 => (
        {
          let checked_value_4 = origin.checked_sub(distance).ok_or(RowLayoutError::Bounds)?;
          checked_value_4
        },
        distance,
      ),
      0.. => (origin, 0),
    };
    let last_row = start.checked_add(distance).ok_or(RowLayoutError::Bounds)?;
    let end = last_row
      .checked_add(extent.width)
      .ok_or(RowLayoutError::Bounds)?;
    console_core_number_conversion::checked_index(end).map_err(|_| RowLayoutError::Bounds)?;

    Ok(PictureBlockHull {
      extent,
      start,
      end,
      first,
      pixel_stride,
    })
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  fn require_hull<BD: BitDepth>(&self, extent: Size<u64>) -> Result<PictureBlockHull, Never> {
    require_picture_layout(self.block_hull::<BD>(extent))
  }

  #[doc = r###"
Returns `(guard, base)`: `base` is 0 for a positive stride and
`(h-1)*|stride|` for a negative one — see [`Self::block_hull`]."###]
  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported picture guard keeps the native guard and base-index contract"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn narrow_guard<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
  ) -> (
    DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>,
    NativeSize,
  ) {
    let Ok(width) = console_core_number_conversion::fixed_index(w);
    let Ok(height) = console_core_number_conversion::fixed_index(h);
    let Ok(hull) = self.require_hull::<BD>(Size { width, height });
    let Ok(range) = console_core_number_conversion::indices(hull.start..hull.end);
    let guard = self.data.slice::<BD, _>(range);
    let Ok(()) = hull.probe::<BD>(|offset, width_2, rows, stride| {
      let Ok(first_byte) = console_core_number_conversion::index(offset);
      let Ok(row_bytes) = console_core_number_conversion::index(width_2);
      let Ok(height_2) = console_core_number_conversion::index(rows);
      let Ok(pitch) = console_core_number_conversion::wrapped_offset(stride);
      guard.probe_declare_rows(first_byte, row_bytes, height_2, pitch);
    });
    let Ok(base) = console_core_number_conversion::index(hull.first);
    (guard, base)
  }

  #[doc = r###"ONE borrow registration instead of `h` when no tile worker can be alive.

# The policy, and why it is the existing one

A `w×h` strided block is either `h` registrations of exactly `w` pixels,
or one registration of `(h-1)*stride + w` — the hull, which additionally
reserves the inter-row gaps belonging to other columns of the same rows.
[`Rav1dPictureDataComponentOffset::block_mut`] already documents the
trade in full: the hull is "correct single-threaded and wrong under tile
threading", because AV1 tiles partition a frame by COLUMNS, so two tile
workers routinely write the same rows at different columns and the gap
reservation turns a genuinely disjoint pair into a false positive.

Decoder-owned pictures carry their worker policy from allocation through
their last retained copy. Raw/legacy components use the monotone
[`tile_threading_active`] fallback. `block_mut`, [`with_pixel_guard_immut`]
and [`Self::compact_read`] use the same per-component decision.

Neither branch can MISS an overlap: the hull is a superset of the `h`
row ranges, and a superset registration conflicts with strictly more.
The choice prevents false conflicts with neighbouring tile workers.

# Why it exists

Per-row guards over small blocks ARE the decoder's borrow-count
distribution. Measured with `--features __probe_sites` on `v4k_8tile` 8bpc
at t=1: collapsing the per-row loops in the loopfilter, `ipred` and
`cdef` took registrations per frame from 15,646,727 to 7,924,706
(-49.4%), at a mean per-row extent of ~10 bytes."###]
  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported native slice visitor retains its existing callback and unit return"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn for_rows<BD: BitDepth, F: FnMut(NativeSize, &[BD::Pixel])>(
    &self,
    w: NativeSize,
    h: NativeSize,
    mut f: F,
  ) {
    use console_core_number_conversion::{fixed_index, index};

    let Ok(width) = fixed_index(w);
    let Ok(height) = fixed_index(h);
    let Ok(()) = self.for_rows_with_context::<BD, _, _>(
      console_core_geometry::Size { width, height },
      &mut f,
      |row, pixels, visit| {
        let Ok(position) = index(row);
        visit(position, pixels);
      },
    );
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  fn row_reservation<BD: BitDepth>(
    &self,
    extent: Size<u64>,
    access: GuardAccess,
  ) -> Result<Option<GuardedRows>, Never> {
    use crate::src::strided::Strided as _;
    use console_core_number_conversion::{fixed_index, fixed_offset, index, wrapped_offset};

    let Ok(width) = index(extent.width);
    let Ok(height) = index(extent.height);

    match (width, height) {
      (0, _) | (_, 0) => return Ok(None),
      _ => {}
    }

    let Ok(visible_width) = fixed_index(width);
    let Ok(visible_height) = fixed_index(height);
    let Ok(origin) = fixed_index(self.offset);
    let Ok(pitch) = fixed_offset(match self.data.pixel_stride::<BD>() {
      Ok(stride) => stride,
    });
    let Ok(mode) = self.data.guard_mode();

    match mode {
      PictureGuardMode::SeparateRows => {
        let Ok(pixel_bytes) = fixed_index(mem::size_of::<BD::Pixel>());
        let Ok(start) = index(origin.wrapping_mul(pixel_bytes));
        let Ok(bytes) = index(visible_width.wrapping_mul(pixel_bytes));
        let Ok(stride_bytes) = wrapped_offset(pitch.wrapping_mul(pixel_bytes.cast_signed()));
        let writable = matches!(access, GuardAccess::Mutable);
        self.data.dm().probe_eval_rect(
          core::panic::Location::caller(),
          writable,
          start,
          bytes,
          height,
          stride_bytes,
        );
      }
      PictureGuardMode::WholeHull => {}
    }

    Ok(Some(GuardedRows {
      extent: Size {
        width: visible_width,
        height: visible_height,
      },
      origin,
      pitch,
      mode,
    }))
  }

  #[doc = "Visits guarded rows with the caller's mutable context passed explicitly."]
  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn for_rows_with_context<
    BD: BitDepth,
    Context,
    F: FnMut(u64, &[BD::Pixel], &mut Context),
  >(
    &self,
    extent: console_core_geometry::Size<u64>,
    context: &mut Context,
    mut visit: F,
  ) -> Result<(), console_core_never::Never> {
    use console_core_number_conversion::index;

    let Ok(reservation) = self.row_reservation::<BD>(extent, GuardAccess::Shared);
    let GuardedRows {
      extent: size,
      origin,
      pitch,
      mode,
    } = match reservation {
      Some(rows) => rows,
      None => return Ok(()),
    };
    let Ok(base_index) = index(origin);
    let Ok(width) = index(size.width);
    let Ok(height) = index(size.height);
    let Ok(stride) = console_core_number_conversion::wrapped_offset(pitch);

    match mode {
      PictureGuardMode::SeparateRows => {
        let rectangle = self
          .data
          .dm()
          .index_rect_as::<BD::Pixel>(base_index, width, height, stride);

        match rectangle {
          Some(rect) => {
            for ordinal in 0..size.height {
              let Ok(row) = index(ordinal);
              visit(ordinal, rect.row(row), context);
            }

            return Ok(());
          }
          None => {}
        }

        for ordinal in 0..size.height {
          let delta = ordinal.cast_signed().wrapping_mul(pitch);
          let Ok(start) = index(origin.wrapping_add_signed(delta));
          let guard = self.data.slice::<BD, _>((start.., ..width));
          visit(ordinal, &guard, context);
        }
      }
      PictureGuardMode::WholeHull => {
        #[cfg_attr(
          dylint_lib = "explicit051_no_machine_width",
          expect(
            explicit051_no_machine_width,
            reason = "the public hull guard returns its native slice origin alongside the guard"
          )
        )]
        let (guard, base) = self.narrow_guard::<BD>(width, height);
        let Ok(first) = console_core_number_conversion::fixed_index(base);

        for ordinal in 0..size.height {
          let delta = ordinal.cast_signed().wrapping_mul(pitch);
          let Ok(start) = index(first.wrapping_add_signed(delta));
          let selected = guard.get(start..).and_then(|window| window.get(..width));
          let Ok(pixels) = require_picture_layout(selected.ok_or(RowLayoutError::Bounds));
          visit(ordinal, pixels, context);
        }
      }
    }

    Ok(())
  }

  #[doc = "Legacy instrumentation entry point for read reservations."]
  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the legacy native reservation instrumentation hook retains its unit return"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn dup_rows<BD: BitDepth>(
    &self,
    _width: ::console_core_api_compatibility::NativeSize,
    _height: ::console_core_api_compatibility::NativeSize,
  ) {
  }

  #[doc = "Legacy instrumentation entry point for write reservations."]
  #[inline(always)]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the legacy native reservation instrumentation hook retains its unit return"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn dup_rows_mut<BD: BitDepth>(
    &self,
    _width: ::console_core_api_compatibility::NativeSize,
    _height: ::console_core_api_compatibility::NativeSize,
  ) {
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported native slice visitor retains its existing callback and unit return"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn for_rows_mut<BD: BitDepth, F: FnMut(NativeSize, &mut [BD::Pixel])>(
    &self,
    w: NativeSize,
    h: NativeSize,
    mut f: F,
  ) {
    use console_core_number_conversion::{fixed_index, index};

    let Ok(width) = fixed_index(w);
    let Ok(height) = fixed_index(h);
    let Ok(()) = self.for_rows_mut_with_context::<BD, _, Never>(
      Size { width, height },
      &mut f,
      |row, pixels, visit| {
        let Ok(position) = index(row);
        visit(position, pixels);
        Ok(())
      },
    );
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn for_rows_mut_with_context<BD: BitDepth, Context, Error>(
    &self,
    extent: Size<u64>,
    context: &mut Context,
    mut visit: impl FnMut(u64, &mut [BD::Pixel], &mut Context) -> Result<(), Error>,
  ) -> Result<(), Error> {
    use console_core_number_conversion::index;

    let Ok(reservation) = self.row_reservation::<BD>(extent, GuardAccess::Mutable);
    let GuardedRows {
      extent: size,
      origin,
      pitch,
      mode,
    } = match reservation {
      Some(rows) => rows,
      None => return Ok(()),
    };
    let Ok(base_index) = index(origin);
    let Ok(width) = index(size.width);
    let Ok(height) = index(size.height);
    let Ok(stride) = console_core_number_conversion::wrapped_offset(pitch);

    match mode {
      PictureGuardMode::SeparateRows => {
        let rectangle = self
          .data
          .dm()
          .index_rect_mut_as::<BD::Pixel>(base_index, width, height, stride);

        match rectangle {
          Some(mut rect) => {
            for ordinal in 0..size.height {
              let Ok(row) = index(ordinal);
              visit(ordinal, rect.row_mut(row), context)?;
            }

            return Ok(());
          }
          None => {}
        }

        for ordinal in 0..size.height {
          let delta = ordinal.cast_signed().wrapping_mul(pitch);
          let Ok(start) = index(origin.wrapping_add_signed(delta));
          let mut guard = self.data.slice_mut::<BD, _>((start.., ..width));
          visit(ordinal, &mut guard, context)?;
        }
      }
      PictureGuardMode::WholeHull => {
        #[cfg_attr(
          dylint_lib = "explicit051_no_machine_width",
          expect(
            explicit051_no_machine_width,
            reason = "the public hull guard returns its native slice origin alongside the guard"
          )
        )]
        let (mut guard, base) = self.narrow_guard_mut::<BD>(width, height);
        let Ok(first) = console_core_number_conversion::fixed_index(base);

        for ordinal in 0..size.height {
          let delta = ordinal.cast_signed().wrapping_mul(pitch);
          let Ok(start) = index(first.wrapping_add_signed(delta));
          let selected = guard
            .get_mut(start..)
            .and_then(|window| window.get_mut(..width));
          let Ok(pixels) = require_picture_layout(selected.ok_or(RowLayoutError::Bounds));
          visit(ordinal, pixels, context)?;
        }
      }
    }

    Ok(())
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the checked adapter preserves the existing native row-index callback of for_rows_mut"
    )
  )]
  pub(crate) fn checked_for_rows_mut<BD: BitDepth>(
    &self,
    extent: Size<u64>,
    mut f: impl FnMut(NativeSize, &mut [BD::Pixel]),
  ) -> Result<(), RowLayoutError> {
    self.checked_for_rows_mut_with_context::<BD, _>(extent, &mut f, |row, pixels, context| {
      let Ok(position) = console_core_number_conversion::index(row);
      context(position, pixels);
      Ok(())
    })
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn checked_for_rows_mut_with_context<BD: BitDepth, Context>(
    &self,
    extent: Size<u64>,
    context: &mut Context,
    visit: impl FnMut(u64, &mut [BD::Pixel], &mut Context) -> Result<(), RowLayoutError>,
  ) -> Result<(), RowLayoutError> {
    let Ok(origin) = console_core_number_conversion::fixed_index(self.offset);
    let Ok(stride) = console_core_number_conversion::fixed_offset(self.stride());
    let layout = block_row_layout::<BD>(extent, origin, stride)?;
    layout.validate_len(self.data.pixel_len::<BD>())?;
    self.for_rows_mut_with_context::<BD, _, _>(extent, context, visit)
  }

  #[doc = r###"
When tile threading is active ([`set_tile_threading`]), each row guard covers
exactly `w` pixels, avoiding stride-padding overlap between concurrent tiles.
When single-threaded, uses one guard for the whole block (fast path).

Returns `(buffer, byte_stride)` where `byte_stride` is `w * pixel_size` when
threading (compact layout) or the original stride when single-threaded."###]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the native compact view returns the existing buffer and byte stride"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn compact_read<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
  ) -> (Vec<u8>, NativeSize) {
    use crate::src::strided::Strided as _;

    match ::core::primitive::bool::then_some(
      {
        matches!(self.data.guard_mode(), Ok(PictureGuardMode::SeparateRows))
          || ({
            match self.data.pixel_stride::<BD>() {
              ::core::result::Result::Ok(value_) => value_,
            }
          }) < 0
      },
      (),
    ) {
      ::core::option::Option::Some(()) => self.compact_read_per_row::<BD>(w, h),
      ::core::option::Option::None => {
        let Ok(width) = console_core_number_conversion::fixed_index(w);
        let Ok(height) = console_core_number_conversion::fixed_index(h);
        let Ok((bytes, pitch)) = self.compact_read_fast::<BD>(Size { width, height });
        let Ok(byte_stride) = console_core_number_conversion::index(pitch);
        (bytes, byte_stride)
      }
    }
  }

  #[doc = r###"order with the original (unsigned) stride — row 0 first only when the
stride is positive, which is the only case `compact_read` sends here."###]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  fn compact_read_fast<BD: BitDepth>(&self, extent: Size<u64>) -> Result<(Vec<u8>, u64), Never> {
    use zerocopy::IntoBytes;
    let Ok(w) = console_core_number_conversion::index(extent.width);
    let Ok(h) = console_core_number_conversion::index(extent.height);
    let (guard, _first) = self.narrow_guard::<BD>(w, h);
    let Ok(stride) = console_core_number_conversion::fixed_offset(self.data.stride());
    Ok((guard.as_bytes().to_vec(), stride.unsigned_abs()))
  }

  #[doc = r###"Always returns compact stride = w * pixel_size.
Used by the loopfilter (needs compact layout for 2D decomposition)
and by tile threading (needs per-row guards to avoid stride overlap)."###]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the native compact view returns the existing buffer and byte stride"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn compact_read_per_row<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
  ) -> (Vec<u8>, NativeSize) {
    use zerocopy::IntoBytes;
    let Ok(width) = console_core_number_conversion::fixed_index(w);
    let Ok(height) = console_core_number_conversion::fixed_index(h);
    let Ok(rows) = CompactPictureRows::new::<BD>(self, Size { width, height });
    let Ok(mut buf) = take_compact_scratch();
    buf.clear();
    let Ok(()) = rows.probe(self.data, CompactPictureAccess::Read);

    for ordinal in 0..rows.samples.height {
      let Ok(CompactPictureRow { plane, .. }) = rows.row(ordinal);
      let Ok(storage) = console_core_number_conversion::indices(plane);
      let guard = self.data.slice::<BD, _>(storage);
      buf.extend_from_slice(guard.as_bytes());
    }

    let Ok(byte_stride) = console_core_number_conversion::wrapped_index(rows.row_bytes);
    (buf, byte_stride)
  }

  #[doc = r###"
Matches the layout produced by [`compact_read`]."###]
  #[cfg_attr(debug_assertions, track_caller)]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the native compact buffer write retains the established unit-returning interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn compact_write_back<BD: BitDepth>(&self, w: NativeSize, h: NativeSize, buf: &[u8]) {
    use crate::src::strided::Strided as _;

    match ::core::primitive::bool::then_some(
      {
        matches!(self.data.guard_mode(), Ok(PictureGuardMode::SeparateRows))
          || ({
            match self.data.pixel_stride::<BD>() {
              ::core::result::Result::Ok(value_) => value_,
            }
          }) < 0
      },
      (),
    ) {
      ::core::option::Option::Some(()) => {
        self.compact_write_back_per_row::<BD>(w, h, buf);
      }
      ::core::option::Option::None => {
        use zerocopy::IntoBytes;
        let (mut guard, _first) = self.narrow_guard_mut::<BD>(w, h);

        for (output, &input) in guard.as_mut_bytes().iter_mut().zip(buf) {
          *output = input;
        }
      }
    }
  }

  #[cfg_attr(debug_assertions, track_caller)]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the native compact buffer write retains the established unit-returning interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn compact_write_back_per_row<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
    buf: &[u8],
  ) {
    let Ok(width) = console_core_number_conversion::fixed_index(w);
    let Ok(height) = console_core_number_conversion::fixed_index(h);
    let Ok(()) = self.write_compact_rows::<BD>(Size { width, height }, buf);
  }

  #[cfg_attr(debug_assertions, track_caller)]
  fn write_compact_rows<BD: BitDepth>(&self, extent: Size<u64>, buf: &[u8]) -> Result<(), Never> {
    use zerocopy::IntoBytes;
    let Ok(rows) = CompactPictureRows::new::<BD>(self, extent);
    let Ok(()) = rows.probe(self.data, CompactPictureAccess::Write);

    for ordinal in 0..rows.samples.height {
      let Ok(CompactPictureRow { plane, buffer }) = rows.row(ordinal);
      let Ok(storage) = console_core_number_conversion::indices(plane);
      let Ok(source) = console_core_number_conversion::indices(buffer);
      let selected = buf.get(source).ok_or(RowLayoutError::Bounds);
      let Ok(input) = require_picture_layout(selected);
      let mut guard = self.data.slice_mut::<BD, _>(storage);
      let Ok(()) = require_picture_layout(copy_picture_samples(guard.as_mut_bytes(), input));
    }

    Ok(())
  }

  #[doc = r###"modified, by diffing `work` against the `pristine` copy taken before
filtering. Rows (and row spans) that are byte-identical are neither
written nor mutably guarded.

This is the tile-threading-correct write-back for filters whose
*read* region is wider than their *write* region (the loop filter
reads 7 tap rows/cols beyond the ≤6 it can modify): a plain
[`Self::compact_write_back_per_row`] would rewrite — and take `&mut`
guards on — tap-input pixels it never changed, which (a) collides
with concurrent readers that dav1d's task schedule legitimately
allows (CDEF bottom-edge padding reads the 2 rows the deblock task
of the next sbrow only ever *reads*; dav1d's 8-row CDEF lag is
exactly 2 pad rows + 6 modified rows), and (b) risks clobbering a
concurrent writer's output with stale copied bytes. Diffing makes the
write-set equal dav1d's by construction. Found via zenavif#30: the
`overlapping DisjointMut` worker panic (`cdef.rs` padding read vs
this write-back's 7th tap row) wedged the decode wait forever.

`work` and `pristine` must both use the compact layout produced by
[`Self::compact_read_per_row`] (stride = `w * pixel_size`)."###]
  #[cfg_attr(debug_assertions, track_caller)]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the native compact buffer write retains the established unit-returning interface"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn compact_write_back_per_row_diff<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
    work: &[u8],
    pristine: &[u8],
  ) {
    use zerocopy::IntoBytes;
    let Ok(width) = console_core_number_conversion::fixed_index(w);
    let Ok(height) = console_core_number_conversion::fixed_index(h);
    let Ok(rows) = CompactPictureRows::new::<BD>(self, Size { width, height });

    for ordinal in 0..rows.samples.height {
      let Ok(CompactPictureRow { plane, buffer }) = rows.row(ordinal);
      let Ok(range) = console_core_number_conversion::indices(buffer);
      let Ok(work_row) =
        require_picture_layout(work.get(range.clone()).ok_or(RowLayoutError::Bounds));
      let Ok(pristine_row) =
        require_picture_layout(pristine.get(range).ok_or(RowLayoutError::Bounds));
      let mut differences = iter::zip(work_row, pristine_row);

      let ::core::result::Result::Ok(changed_start) =
        ::console_core_number_conversion::fixed_index(
          match differences
            .clone()
            .position(|(after, before)| after != before)
          {
            Some(changed) => changed,
            None => continue,
          },
        );
      let ::core::result::Result::Ok(changed_end) = ::console_core_number_conversion::fixed_index(
        match differences.rposition(|(after, before)| after != before) {
          Some(changed) => changed,
          None => continue,
        },
      );
      let Ok(first_pixel) = require_picture_layout(
        changed_start
          .checked_div(rows.pixel_bytes)
          .ok_or(RowLayoutError::Bounds),
      );
      let Ok(last_pixel) = require_picture_layout(
        changed_end
          .checked_div(rows.pixel_bytes)
          .ok_or(RowLayoutError::Bounds),
      );
      let end_pixel = last_pixel.wrapping_add(1);
      let Ok(start) =
        console_core_number_conversion::wrapped_index(plane.start.wrapping_add(first_pixel));
      let Ok(changed_width) =
        console_core_number_conversion::wrapped_index(end_pixel.wrapping_sub(first_pixel));
      let Ok(first_byte) =
        console_core_number_conversion::wrapped_index(first_pixel.wrapping_mul(rows.pixel_bytes));
      let Ok(end_byte) =
        console_core_number_conversion::wrapped_index(end_pixel.wrapping_mul(rows.pixel_bytes));
      let Ok(input) = require_picture_layout(
        work_row
          .get(first_byte..end_byte)
          .ok_or(RowLayoutError::Bounds),
      );
      let mut guard = self.data.slice_mut::<BD, _>((start.., ..changed_width));
      let Ok(()) = require_picture_layout(copy_picture_samples(guard.as_mut_bytes(), input));
    }
  }

  #[doc = r###"
Returns `(guard, offset_within_guard)` where the offset corresponds to
this PicOffset's logical position within the full guard.
Use this when the access pattern is complex (e.g., loopfilter accessing
negative offsets from the base pointer)."###]
  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported full-plane view returns its native disjoint guard and offset directly"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn full_guard_mut<BD: BitDepth>(
    &self,
  ) -> (
    DisjointMutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>,
    NativeSize,
  ) {
    let guard = self.data.slice_mut::<BD, _>(..);
    (guard, self.offset)
  }

  #[doc = r###"whose row 0 starts at this offset.

Returns `(guard, base)`: the guard is the block's hull,
`(h-1)*|stride| + w` pixels, and `base` is row 0's index inside it — 0
for a positive stride, `(h-1)*|stride|` for a negative one, where the
hull starts at the last row. See [`Self::block_hull`]."###]
  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported picture guard keeps the native guard and base-index contract"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn narrow_guard_mut<BD: BitDepth>(
    &self,
    w: NativeSize,
    h: NativeSize,
  ) -> (
    DisjointMutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>,
    NativeSize,
  ) {
    let Ok(width) = console_core_number_conversion::fixed_index(w);
    let Ok(height) = console_core_number_conversion::fixed_index(h);
    let Ok(hull) = self.require_hull::<BD>(Size { width, height });
    let Ok(range) = console_core_number_conversion::indices(hull.start..hull.end);
    let guard = self.data.slice_mut::<BD, _>(range);
    let Ok(()) = hull.probe::<BD>(|offset, width_2, rows, stride| {
      let Ok(first_byte) = console_core_number_conversion::index(offset);
      let Ok(row_bytes) = console_core_number_conversion::index(width_2);
      let Ok(height_2) = console_core_number_conversion::index(rows);
      let Ok(pitch) = console_core_number_conversion::wrapped_offset(stride);
      guard.probe_declare_rows(first_byte, row_bytes, height_2, pitch);
    });
    let Ok(base) = console_core_number_conversion::index(hull.first);
    (guard, base)
  }

  #[inline]
  #[cfg_attr(debug_assertions, track_caller)]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported full-plane view returns its native disjoint guard and offset directly"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "this compatibility view requires native slice indices, lengths, and callback positions"
    )
  )]
  pub fn full_guard<BD: BitDepth>(
    &self,
  ) -> (
    DisjointImmutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>,
    NativeSize,
  ) {
    let guard = self.data.slice::<BD, _>(..);
    (guard, self.offset)
  }
}

#[cfg(feature = "c-ffi")]
pub struct Rav1dPictureData {
  pub data: [Rav1dPictureDataComponent; 3],
  pub(crate) frame_progress: Option<[std::sync::atomic::AtomicU32; 2]>,
  pub(crate) allocator_data: Option<Dav1dOpaquePointer>,
  pub(crate) allocator: Rav1dPicAllocator,
}

#[cfg(not(feature = "c-ffi"))]
pub struct Rav1dPictureData {
  pub data: [Rav1dPictureDataComponent; 3],
  pub(crate) frame_progress: Option<[std::sync::atomic::AtomicU32; 2]>,
  pub(crate) allocator: Rav1dPicAllocator,
}

#[cfg(feature = "c-ffi")]
impl Drop for Rav1dPictureData {
  fn drop(&mut self) {
    let Self {
      data,
      allocator_data,
      allocator,
      ..
    } = self;
    let Ok(()) = allocator.dealloc_picture_data(data, *allocator_data);
  }
}

#[cfg(not(feature = "c-ffi"))]
impl Drop for Rav1dPictureData {
  fn drop(&mut self) {
    for component in &mut self.data {
      let Ok(storage) = component.take_buf();

      match storage {
        Some(buf) => match buf.first() {
          Some(_) => {
            let Ok(()) = self.allocator.pool.push(buf);
          }
          None => {}
        },
        None => {}
      }
    }
  }
}

#[derive(Clone, Default)]
#[repr(C)]
#[cfg_attr(
  all(feature = "c-ffi", dylint_lib = "explicit065_checked_constructors"),
  expect(
    explicit065_checked_constructors,
    reason = "C conversion validates the incoming parameters while the Rust decoder uses a default empty picture when it has no output"
  )
)]
pub(crate) struct Rav1dPicture {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "retained output pictures independently release their shared decoded sequence metadata"
    )
  )]
  pub seq_hdr: Option<Arc<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "queued and retained pictures keep the frame metadata live until their last release"
    )
  )]
  pub frame_hdr: Option<Arc<DRav1d<Rav1dFrameHeader, Dav1dFrameHeader>>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the scheduler and native output handles independently retain the same decoded plane allocation"
    )
  )]
  pub data: Option<Arc<Rav1dPictureData>>,
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the retained Rust picture has the same signed stride representation used by existing C and Rust output views"
    )
  )]
  pub stride: [NativeOffset; 2],
  pub p: Rav1dPictureParameters,
  pub m: Rav1dDataProps,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "each retained native output handle keeps the inherited content-light metadata live"
    )
  )]
  pub content_light: Option<Arc<Rav1dContentLightLevel>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "each retained native output handle keeps the inherited display metadata live"
    )
  )]
  pub mastering_display: Option<Arc<Rav1dMasteringDisplay>>,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "native picture references independently retain and release shared ITU metadata"
    )
  )]
  pub itut_t35: Arc<DRav1d<Box<[Rav1dITUTT35]>, Box<[Dav1dITUTT35]>>>,
}

#[cfg(feature = "c-ffi")]
impl Rav1dPicture {
  #[doc = "\n# Safety\nThe caller must retain the original picture's live handles until this call returns."]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the initialized C picture still owns every raw reference while Rust retains its own copies"
    )
  )]
  pub(crate) unsafe fn from_borrowed(picture: &Dav1dPicture) -> Rav1dResult<Self> {
    #[cfg_attr(
      dylint_lib = "explicit058_contain_unsafe",
      expect(
        explicit058_contain_unsafe,
        reason = "the raw handle belongs to the live C picture being borrowed"
      )
    )]
    #[cfg_attr(
      dylint_lib = "explicit081_no_shared_ownership",
      expect(
        explicit081_no_shared_ownership,
        reason = "the two picture references release their shared data independently"
      )
    )]
    #[must_use]
    unsafe fn retain<T>(handle: Option<RawArc<T>>) -> Result<Option<Arc<T>>, Never> {
      Ok(handle.map(|original| {
        // SAFETY: The original C picture continues to hold this handle.
        let Ok(retained) = unsafe { original.borrow_clone() };

        retained
      }))
    }

    let parameters = Rav1dPictureParameters::try_from(picture.p)?;
    // SAFETY: The caller guarantees these initialized handles stay live while the picture is borrowed.
    let retained = unsafe {
      let Ok(m) = Rav1dDataProps::from_borrowed(&picture.m);
      let Ok(seq_hdr) = retain(picture.seq_hdr_ref);
      let Ok(frame_hdr) = retain(picture.frame_hdr_ref);
      let Ok(data) = retain(picture.r#ref);
      let Ok(content_light) = retain(picture.content_light_ref);
      let Ok(mastering_display) = retain(picture.mastering_display_ref);
      let Ok(itu_records) = retain(picture.itut_t35_ref);
      let itut_t35 = match itu_records {
        Some(records) => records,
        None => Default::default(),
      };

      Self {
        seq_hdr,
        frame_hdr,
        data,
        stride: picture.stride,
        p: parameters,
        m,
        content_light,
        mastering_display,
        itut_t35,
      }
    };

    Ok(retained)
  }
}

#[cfg(feature = "c-ffi")]
impl TryFrom<Dav1dPicture> for Rav1dPicture {
  type Error = Rav1dError;

  fn try_from(value: Dav1dPicture) -> Rav1dResult<Self> {
    #[cfg_attr(
      dylint_lib = "explicit058_contain_unsafe",
      expect(
        explicit058_contain_unsafe,
        reason = "each initialized picture handle transfers its one raw Arc owner to this conversion"
      )
    )]
    #[cfg_attr(
      dylint_lib = "explicit081_no_shared_ownership",
      expect(
        explicit081_no_shared_ownership,
        reason = "an existing C picture handle returns to its original shared allocation"
      )
    )]
    fn reclaim<T>(raw_owner: Option<RawArc<T>>) -> Result<Option<Arc<T>>, Never> {
      Ok(raw_owner.map(|token| {
        // SAFETY: The consumed C record transfers the original owner of each handle.
        let Ok(owner) = unsafe { token.into_arc() };
        owner
      }))
    }

    let Dav1dPicture {
      p,
      m,
      frame_hdr_ref,
      seq_hdr_ref,
      content_light_ref,
      mastering_display_ref,
      itut_t35_ref,
      r#ref: data_ref,
      ..
    } = value;
    let Ok(sequence) = reclaim(seq_hdr_ref);
    let Ok(frame) = reclaim(frame_hdr_ref);
    let Ok(data) = reclaim(data_ref);
    let Ok(content_light) = reclaim(content_light_ref);
    let Ok(mastering_display) = reclaim(mastering_display_ref);
    let Ok(itut_t35) = reclaim(itut_t35_ref);
    let metadata = Rav1dDataProps::from(m);
    let parameters = Rav1dPictureParameters::try_from(p)?;

    Ok(Self {
      seq_hdr: sequence,
      frame_hdr: frame,
      data,
      stride: value.stride,
      p: parameters,
      m: metadata,
      content_light,
      mastering_display,
      itut_t35: match itut_t35 {
        Some(records) => records,
        None => Default::default(),
      },
    })
  }
}

#[cfg(feature = "c-ffi")]
impl From<Rav1dPicture> for Dav1dPicture {
  fn from(value: Rav1dPicture) -> Self {
    let Rav1dPicture {
      seq_hdr,
      frame_hdr,
      data,
      p,
      m,
      content_light,
      mastering_display,
      itut_t35,
      ..
    } = value;

    #[must_use]
    #[cfg_attr(
      dylint_lib = "explicit081_no_shared_ownership",
      expect(
        explicit081_no_shared_ownership,
        reason = "export hands the picture's strong reference to C until the corresponding unref"
      )
    )]
    fn export<T>(owner: Option<Arc<T>>) -> Result<Option<RawArc<T>>, Never> {
      Ok(owner.map(|retained| {
        let Ok(token) = RawArc::from_arc(retained);
        token
      }))
    }

    let mut output = Self {
      seq_hdr: seq_hdr.as_ref().map(|handle| NonNull::from(&handle.dav1d)),
      frame_hdr: frame_hdr
        .as_ref()
        .map(|handle| NonNull::from(&handle.dav1d)),
      data: match &data {
        Some(owner) => owner.data.each_ref().map(|component| {
          let Ok(pointer) = component.as_dav1d();
          pointer
        }),
        None => [None; 3],
      },
      stride: value.stride,
      p: Dav1dPictureParameters::from(p),
      m: Dav1dDataProps::from(m),
      content_light: content_light.as_deref().map(NonNull::from),
      mastering_display: mastering_display.as_deref().map(NonNull::from),
      itut_t35: Some(NonNull::from(itut_t35.dav1d.as_ref()).cast()),
      n_itut_t35: itut_t35.len(),
      reserved: Default::default(),
      frame_hdr_ref: None,
      seq_hdr_ref: None,
      content_light_ref: None,
      mastering_display_ref: None,
      itut_t35_ref: None,
      reserved_ref: Default::default(),
      allocator_data: data.as_ref().and_then(|owner| owner.allocator_data),
      r#ref: None,
    };

    let Ok(frame_owner) = export(frame_hdr);
    let Ok(sequence_owner) = export(seq_hdr);
    let Ok(content_owner) = export(content_light);
    let Ok(display_owner) = export(mastering_display);
    let Ok(itu_owner) = export(Some(itut_t35));
    let Ok(plane_owner) = export(data);
    output.frame_hdr_ref = frame_owner;
    output.seq_hdr_ref = sequence_owner;
    output.content_light_ref = content_owner;
    output.mastering_display_ref = display_owner;
    output.itut_t35_ref = itu_owner;
    output.r#ref = plane_owner;

    output
  }
}

impl Rav1dPicture {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the exported filtering adapter returns the established fixed set of three plane views"
    )
  )]
  pub fn lf_offsets<BD: BitDepth>(&self, y: c_int) -> [Rav1dPictureDataComponentOffset<'_>; 3] {
    let Ok(picture) = require_picture_layout(self.data.as_deref().ok_or(RowLayoutError::Bounds));
    let [luma, u, v] = &picture.data;
    let planes = match self.p.layout {
      Rav1dPixelLayout::I400 => [(luma, 0), (luma, 0), (luma, 0)],
      Rav1dPixelLayout::I420 => [(luma, 0), (u, 1), (v, 1)],
      Rav1dPixelLayout::I422 | Rav1dPixelLayout::I444 => [(luma, 0), (u, 0), (v, 0)],
    };

    planes.map(|(plane, shift)| {
      let Ok(stride) =
        console_core_number_conversion::fixed_offset(match plane.pixel_stride::<BD>() {
          Ok(samples) => samples,
        });
      let displacement = i64::from(y).wrapping_mul(stride).wrapping_shr(shift);
      let Ok(offset) = console_core_number_conversion::wrapped_offset(displacement);
      plane.with_offset::<BD>() + offset
    })
  }
}

#[cfg(feature = "c-ffi")]
#[derive(Clone)]
#[repr(C)]
pub struct Dav1dPicAllocator {
  #[doc = r###"
# Safety

All accesses to [`Self::cookie`] must be thread-safe
(i.e. [`Self::cookie`] must be [`Send`]` + `[`Sync`]).

If used from Rust, [`Self::cookie`] is a [`SendSyncNonNull`],
whose constructors ensure this [`Send`]` + `[`Sync`] safety."###]
  pub cookie: Option<Dav1dOpaquePointer>,

  #[doc = r###"
[`data`]`[0]`, [`data`]`[1]` and [`data`]`[2]`
must be [`DAV1D_PICTURE_ALIGNMENT`]-byte aligned
and with a pixel width/height multiple of 128 pixels.
Any allocated memory area should also be padded by [`DAV1D_PICTURE_ALIGNMENT`] bytes.
[`data`]`[1]` and [`data`]`[2]` must share the same [`stride`]`[1]`.

# Safety

See [`Self::cookie`]'s safety requirements.

### Additional `rav1d` requirement:

The allocated data must be initialized.
If newly (e.x. not reused) allocated data is zero initialized using OS APIs,
it is possible for this to not be slower than an uninitialized allocation.
For example, see `dav1d_default_picture_alloc` and `MemPool::pop_init`.

If the allocated data is not initialized,
it is possible there will be reads of uninitialized data.
`rav1d` should not read this data before writing to it first,
but it does not guarantee that it does so.
Instead, initializing the allocated data guarantees all uses of it will be sound.

# Args

* `pic`: The picture to allocate the buffer for.
    The callback needs to fill the picture
    [`data`]`[0]`, [`data`]`[1]`, [`data`]`[2]`,
    [`stride`]`[0]`, and [`stride`]`[1]`.
    The allocator can fill the pic [`allocator_data`] pointer
    with a custom pointer that will be passed to
    [`release_picture_callback`].

    The only fields of `pic` that will be already set are:
    * [`Dav1dPicture::p`]
    * [`Dav1dPicture::seq_hdr`]
    * [`Dav1dPicture::frame_hdr`]

    This is not a change from the original `DAV1D_API`,
    just a clarification of it.

* `cookie`: Custom pointer passed to all calls.

*Note*: No fields other than [`data`], [`stride`] and [`allocator_data`]
must be filled by this callback.

# Return

0 on success. A negative `DAV1D_ERR` value on error.
<!--- TODO(kkysen) Translate `DAV1D_ERR` -->

[`data`]: Dav1dPicture::data
[`stride`]: Dav1dPicture::data
[`allocator_data`]: Dav1dPicture::allocator_data
[`release_picture_callback`]: Self::release_picture_callback"###]
  pub alloc_picture_callback: Option<
    unsafe extern "C" fn(
      pic: *mut Dav1dPicture,
      cookie: Option<Dav1dOpaquePointer>,
    ) -> Dav1dResult,
  >,

  #[doc = r###"
# Safety

If frame threading is used, accesses to `cookie` must be thread-safe.

If frame threading is used, this function may be called by the main thread
(the thread which calls [`dav1d_get_picture`]),
or any of the frame threads and thus must be thread-safe.
If frame threading is not used, this function will only be called on the main thread.

# Args

* `pic`: The picture that was filled by [`alloc_picture_callback`].

    The only fields of `pic` that will be set are
    the ones allocated by [`Self::alloc_picture_callback`]:
    * [`Dav1dPicture::data`]
    * [`Dav1dPicture::allocator_data`]

    NOTE: This is a slight change from the original `DAV1D_API`, which was underspecified.
    However, all known uses of this API follow this already:
    * `libdav1d`: [`dav1d_default_picture_release`](https://code.videolan.org/videolan/dav1d/-/blob/16ed8e8b99f2fcfffe016e929d3626e15267ad3e/src/picture.c#L85-87)
    * `dav1d`: [`picture_release`](https://code.videolan.org/videolan/dav1d/-/blob/16ed8e8b99f2fcfffe016e929d3626e15267ad3e/tools/dav1d.c#L180-182)
    * `dav1dplay`: [`placebo_release_pic`](https://code.videolan.org/videolan/dav1d/-/blob/16ed8e8b99f2fcfffe016e929d3626e15267ad3e/examples/dp_renderer_placebo.c#L375-383)
    * `libplacebo`: [`pl_release_dav1dpicture`](https://github.com/haasn/libplacebo/blob/34e019bfedaa5a64f268d8f9263db352c0a8f67f/src/include/libplacebo/utils/dav1d_internal.h#L594-L607)
    * `ffmpeg`: [`libdav1d_picture_release`](https://github.com/FFmpeg/FFmpeg/blob/00b288da73f45acb78b74bcc40f73c7ba1fff7cb/libavcodec/libdav1d.c#L124-L129)

    Making this API safe without this slight tightening of the API
    [is very difficult](https://github.com/memorysafety/rav1d/pull/685#discussion_r1458171639).

* `cookie`: Custom pointer passed to all calls.

[`dav1d_get_picture`]: crate::src::lib::dav1d_get_picture
[`alloc_picture_callback`]: Self::alloc_picture_callback"###]
  pub release_picture_callback: Option<
    unsafe extern "C" fn(pic: *mut Dav1dPicture, cookie: Option<Dav1dOpaquePointer>) -> (),
  >,
}

#[cfg(feature = "c-ffi")]
#[derive(Clone)]
#[repr(C)]
#[cfg_attr(
  dylint_lib = "explicit065_checked_constructors",
  expect(
    explicit065_checked_constructors,
    reason = "imported C allocators require both callbacks and the Rust default installs its own matched allocation and release callbacks"
  )
)]
pub(crate) struct Rav1dPicAllocator {
  #[doc = r###"
# Safety

Custom allocator cookies retain the C API's caller-owned lifetime.
The default allocator instead passes an owned pool to its Rust helper,
so a retained picture can allocate a copy after its decoder is dropped."###]
  pub cookie: Option<Dav1dOpaquePointer>,

  #[doc = r###"initialization. This field is not part of the public Dav1dPicAllocator
ABI and is never exported as an owning C cookie."###]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "retained C pictures recycle and copy their planes after their decoder has been released"
    )
  )]
  pub(crate) default_pool: Option<Arc<MemPool<u8>>>,

  #[doc = r###"
# Safety

`pic` is passed as a `&mut`.

If frame threading is used, accesses to [`Self::cookie`] must be thread-safe,
i.e. [`Self::cookie`] must be [`Send`]` + `[`Sync`]."###]
  pub alloc_picture_callback: unsafe extern "C" fn(
    pic: *mut Dav1dPicture,
    cookie: Option<Dav1dOpaquePointer>,
  ) -> Dav1dResult,

  #[doc = r###"
# Safety

`pic` is passed as a `&mut`.

If frame threading is used, accesses to [`Self::cookie`] must be thread-safe,
i.e. [`Self::cookie`] must be [`Send`]` + `[`Sync`]."###]
  pub release_picture_callback:
    unsafe extern "C" fn(pic: *mut Dav1dPicture, cookie: Option<Dav1dOpaquePointer>) -> (),
}

#[cfg(not(feature = "c-ffi"))]
#[derive(Clone, Default)]
pub(crate) struct Rav1dPicAllocator {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "retained output pictures can recycle or copy their planes after decoder shutdown"
    )
  )]
  pub(crate) pool: Arc<MemPool<u8>>,
}

#[cfg(feature = "c-ffi")]
impl TryFrom<Dav1dPicAllocator> for Rav1dPicAllocator {
  type Error = Rav1dError;

  fn try_from(value: Dav1dPicAllocator) -> Result<Self, Self::Error> {
    let Dav1dPicAllocator {
      cookie,
      alloc_picture_callback,
      release_picture_callback,
    } = value;
    let allocate = validate_input!(alloc_picture_callback.ok_or(EINVAL))?;
    let release = validate_input!(release_picture_callback.ok_or(EINVAL))?;

    Ok(Self {
      cookie,
      default_pool: None,
      alloc_picture_callback: allocate,
      release_picture_callback: release,
    })
  }
}

#[cfg(feature = "c-ffi")]
impl From<Rav1dPicAllocator> for Dav1dPicAllocator {
  fn from(value: Rav1dPicAllocator) -> Self {
    let Rav1dPicAllocator {
      cookie,
      default_pool: _,
      alloc_picture_callback,
      release_picture_callback,
    } = value;
    Self {
      cookie,
      alloc_picture_callback: Some(alloc_picture_callback),
      release_picture_callback: Some(release_picture_callback),
    }
  }
}

#[cfg(feature = "c-ffi")]
pub(crate) enum BufferReturn {
  Free,
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the C picture release callback may return the buffer after its decoder has closed"
    )
  )]
  Recycle(Arc<MemPool<u8>>),
}

#[cfg(feature = "c-ffi")]
struct CPlane {
  address: Option<NonNull<u8>>,
  layout: CPlaneLayout,
}

#[cfg(feature = "c-ffi")]
fn describe_picture_planes(
  picture: &Rav1dPicture,
  data: [Option<NonNull<c_void>>; 3],
) -> Rav1dResult<[CPlane; 3]> {
  let [luma, chroma] = picture.stride.map(|stride| {
    let Ok(fixed) = console_core_number_conversion::fixed_offset(stride);
    fixed
  });
  let [luma_bytes, chroma_bytes] = picture.p.pic_len([luma, chroma])?;
  let y_layout = CPlaneLayout {
    length: luma_bytes,
    stride: luma,
  };
  let uv_layout = CPlaneLayout {
    length: chroma_bytes,
    stride: chroma,
  };
  let [y, u, v] = data.map(|pointer| pointer.map(NonNull::cast));

  let missing_chroma = picture.p.layout != Rav1dPixelLayout::I400 && (u.is_none() || v.is_none());

  match (y.is_none() || missing_chroma).then_some(()) {
    Some(()) => return Err(EINVAL),
    None => {}
  }

  Ok([
    CPlane { address: y, layout: y_layout },
    CPlane { address: u, layout: uv_layout },
    CPlane { address: v, layout: uv_layout },
  ])
}

#[cfg(feature = "c-ffi")]
impl Rav1dPicAllocator {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the output, decoder and caller may hold independent strong references to the picture and its headers"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the c-ffi allocator invokes caller-supplied allocation callbacks and validates the returned plane descriptors"
    )
  )]
  pub fn alloc_picture_data(
    &self,
    w: c_int,
    h: c_int,
    seq_hdr: Arc<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>>,
    frame_hdr: Option<Arc<DRav1d<Rav1dFrameHeader, Dav1dFrameHeader>>>,
  ) -> Rav1dResult<Rav1dPicture> {
    let pic = Rav1dPicture {
      p: Rav1dPictureParameters {
        w,
        h,
        layout: seq_hdr.layout,
        bpc: ::core::primitive::u8::wrapping_add(
          8,
          ::core::primitive::u8::wrapping_mul(2, seq_hdr.hbd),
        ),
      },
      seq_hdr: Some(seq_hdr),
      frame_hdr,
      ..Default::default()
    };
    let mut pic_c = Dav1dPicture::from(pic);
    let status = match &self.default_pool {
      Some(pool) => {
        let release = BufferReturn::Recycle(Arc::clone(pool));
        Self::allocate_default_buffer(&mut pic_c, release)
      }
      None => {
        // SAFETY: The initialized picture has empty allocation fields and a live callback cookie.
        let status = unsafe { (self.alloc_picture_callback)(&mut pic_c, self.cookie) };

        match Rav1dResult::try_from(status) {
          Ok(result) => result,
          Err(unrecognized) => {
            console_core_logging::warn!(
              "AV1 picture allocator returned status {}",
              unrecognized.0
            );
            Err(Rav1dError::EGeneric)
          }
        }
      }
    };
    let data = pic_c.data;
    let allocator_data = pic_c.allocator_data;

    let reclaimed = Rav1dPicture::try_from(pic_c);
    status?;
    let mut picture = match reclaimed {
      Ok(owned) => owned,
      Err(failure) => {
        let Ok(()) = self.release_buffers(data, allocator_data);
        return Err(failure);
      }
    };

    let mapped_planes = describe_picture_planes(&picture, data).and_then(|[y, u, v]| {
      // SAFETY: The successful allocator call lends the described planes until release.
      let planes = unsafe {
        let luma = Rav1dPictureDataComponentInner::new(y.address, y.layout)?;
        let chroma_u = Rav1dPictureDataComponentInner::new(u.address, u.layout)?;
        let chroma_v = Rav1dPictureDataComponentInner::new(v.address, v.layout)?;

        [luma, chroma_u, chroma_v]
      };

      Ok(planes.map(|inner| {
        let Ok(component) = Rav1dPictureDataComponent::from_parts(inner);
        component
      }))
    });

    let components = match mapped_planes {
      Ok(components) => components,
      Err(error) => {
        let Ok(()) = self.release_buffers(data, allocator_data);
        return Err(error);
      }
    };

    picture.data = Some(Arc::new(Rav1dPictureData {
      data: components,
      frame_progress: None,
      allocator_data,
      allocator: self.clone(),
    }));
    Ok(picture)
  }

  pub fn dealloc_picture_data(
    &self,
    data: &mut [Rav1dPictureDataComponent; 3],
    allocator_data: Option<Dav1dOpaquePointer>,
  ) -> Result<(), Never> {
    let pointers = data.each_ref().map(|component| {
      let Ok(pointer) = component.as_dav1d();
      pointer
    });
    self.release_buffers(pointers, allocator_data)
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C allocator release callback receives the same live allocation descriptor and cookie that produced the picture"
    )
  )]
  fn release_buffers(
    &self,
    data: [Option<NonNull<c_void>>; 3],
    allocator_data: Option<Dav1dOpaquePointer>,
  ) -> Result<(), Never> {
    let mut pic_c = Dav1dPicture {
      data,
      allocator_data,
      ..Default::default()
    };

    // SAFETY: `pic_c` contains the same `data` and `allocator_data`
    // that `Self::alloc_picture_data` set, which now get deallocated here.
    unsafe {
      (self.release_picture_callback)(&mut pic_c, self.cookie);
    }

    Ok(())
  }
}

#[cfg(not(feature = "c-ffi"))]
impl Rav1dPicAllocator {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "this compatibility entry point retains the existing positional width and height arguments"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the output, decoder and caller may hold independent strong references to the picture and its headers"
    )
  )]
  pub fn alloc_picture_data(
    &self,
    w: c_int,
    h: c_int,
    seq_hdr: Arc<DRav1d<Rav1dSequenceHeader, Dav1dSequenceHeader>>,
    frame_hdr: Option<Arc<DRav1d<Rav1dFrameHeader, Dav1dFrameHeader>>>,
  ) -> Rav1dResult<Rav1dPicture> {
    let p = Rav1dPictureParameters {
      w,
      h,
      layout: seq_hdr.layout,
      bpc: 8_u8.wrapping_add(2_u8.wrapping_mul(seq_hdr.hbd)),
    };

    let strides = p.default_strides()?;
    let [luma_stride, chroma_stride] = strides;
    let y_stride = console_core_number_conversion::checked_offset(luma_stride)
      .map_err(|_| Rav1dError::ENOMEM)?;
    let uv_stride = console_core_number_conversion::checked_offset(chroma_stride)
      .map_err(|_| Rav1dError::ENOMEM)?;
    let [luma_bytes, chroma_bytes] = p.pic_len(strides)?;
    let plane_multiple = RAV1D_PICTURE_MULTIPLE;
    let Ok(alignment) = console_core_number_conversion::fixed_index(RAV1D_PICTURE_ALIGNMENT);

    let alloc_plane = |bytes: u64| -> Rav1dResult<PicBuf> {
      let padded = bytes
        .checked_next_multiple_of(plane_multiple)
        .ok_or(Rav1dError::ENOMEM)?;
      let storage = match padded {
        0 => Vec::new(),
        _ => {
          let capacity = padded.checked_add(alignment).ok_or(Rav1dError::ENOMEM)?;
          self.pool.pop_init(capacity, 0)?
        }
      };
      let length =
        console_core_number_conversion::checked_index(padded).map_err(|_| Rav1dError::ENOMEM)?;
      Ok(PicBuf::from_vec_aligned(
        storage,
        RAV1D_PICTURE_ALIGNMENT,
        length,
      ))
    };
    let y = alloc_plane(luma_bytes)?;
    let u = alloc_plane(chroma_bytes)?;
    let v = alloc_plane(chroma_bytes)?;
    let Ok(luma) = Rav1dPictureDataComponent::from_parts(y, luma_stride);
    let Ok(chroma_u) = Rav1dPictureDataComponent::from_parts(u, chroma_stride);
    let Ok(chroma_v) = Rav1dPictureDataComponent::from_parts(v, chroma_stride);
    let data = [luma, chroma_u, chroma_v];

    let pic = Rav1dPicture {
      p,
      seq_hdr: Some(seq_hdr),
      frame_hdr,
      stride: [y_stride, uv_stride],
      data: Some(Arc::new(Rav1dPictureData {
        data,
        frame_progress: None,
        allocator: self.clone(),
      })),
      ..Default::default()
    };

    Ok(pic)
  }
}

pub type PicOffset<'a> = Rav1dPictureDataComponentOffset<'a>;

enum BlockMutStorage<'a, BD: BitDepth> {
  Direct {
    guard: DisjointMutGuard<'a, Rav1dPictureDataComponentInner, [BD::Pixel]>,
  },
  #[doc = r###"through fresh per-row guards."###]
  Compact { buf: Vec<u8> },
}

#[doc = r###"documents why the compact variant exists.

On drop, a compact block is written back to the picture through per-row
guards. A direct block has nothing to do — the caller wrote the picture in
place."###]
pub struct BlockMut<'a, BD: BitDepth> {
  storage: BlockMutStorage<'a, BD>,
  dst: WithOffset<&'a Rav1dPictureDataComponent>,
  extent: Size<u64>,
  base: u64,
  byte_stride: i64,
}

impl<'a, BD: BitDepth> BlockMut<'a, BD> {
  #[doc = r###"
NOT necessarily the picture's stride: a compact block is tightly packed
at `w * size_of::<BD::Pixel>()`. Always positive in the compact case;
may be negative in the direct case, exactly as the picture's is."###]
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this RAII picture guard exposes its native byte stride directly"
    )
  )]
  pub fn byte_stride(&self) -> ::console_core_api_compatibility::NativeOffset {
    let Ok(stride) = console_core_number_conversion::wrapped_offset(self.byte_stride);
    stride
  }

  #[doc = r###"
Zero for a compact block; for a direct block with negative stride it is
the last row's offset, matching `strided_slice_mut`'s second return value."###]
  #[inline]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this RAII picture guard exposes its native base offset directly"
    )
  )]
  pub fn base(&self) -> ::console_core_api_compatibility::NativeSize {
    let Ok(index) = console_core_number_conversion::index(self.base);
    index
  }

  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this RAII picture guard exposes the existing borrowed native byte slice"
    )
  )]
  pub fn as_mut_bytes(&mut self) -> &mut [u8] {
    match &mut self.storage {
      BlockMutStorage::Direct { guard } => guard.as_mut_bytes(),
      BlockMutStorage::Compact { buf } => buf.as_mut_slice(),
    }
  }
}

fn block_row_layout<BD: BitDepth>(
  extent: Size<u64>,
  origin: u64,
  stride: i64,
) -> Result<RowLayout, RowLayoutError> {
  let Ok(width) = console_core_number_conversion::fixed_index(mem::size_of::<BD::Pixel>());
  let pixel_bytes = i64::try_from(width).map_err(|_too_large| RowLayoutError::Stride)?;

  match stride.checked_rem(pixel_bytes) {
    Some(0) => {}
    Some(_) | None => return Err(RowLayoutError::Stride),
  }

  let stride_samples = stride
    .checked_div(pixel_bytes)
    .ok_or(RowLayoutError::Stride)?;
  RowLayout::new(extent, origin, stride_samples)
}

impl<BD: BitDepth> Drop for BlockMut<'_, BD> {
  fn drop(&mut self) {
    match &mut self.storage {
      BlockMutStorage::Direct { .. } => {}
      BlockMutStorage::Compact { buf } => {
        let Ok(()) = self.dst.write_compact_rows::<BD>(self.extent, buf);
        let Ok(()) = recycle_compact_scratch(core::mem::take(buf));
      }
    }
  }
}

#[cfg(test)]
mod tile_threading_latch_tests {
  #[doc = r###"
It is one process-global bool shared by every decoder, so a
single-threaded `rav1d_open` storing `false` used to push concurrently
live multi-threaded decoders back onto the wide-guard path — whose
borrow extent is `(h-1) * stride + w`, e.g. 16,321 pixels for a 1x16
intra left-edge column — while their tile workers were running. That
showed up as spurious `overlapping DisjointMut` panics under load (8-9
of 24 concurrent runs) and would be an undetected data race in an
`unchecked` build."###]
  #[test]
  fn set_tile_threading_is_monotone() {
    use super::{set_tile_threading, tile_threading_active};
    set_tile_threading(true);
    assert!(tile_threading_active());
    set_tile_threading(false);
    assert!(
      tile_threading_active(),
      "a single-threaded open must not clear tile threading for \
             concurrently live multi-threaded decoders"
    );
  }
}

#[doc = r###"[`Rav1dPictureDataComponentOffset::for_rows_mut`].

Those two pick between ONE hull registration and `h` per-row ones on
[`tile_threading_active`]. Which branch runs is invisible in the decoded
pixels, so nothing else in the tree can catch a regression in either
direction:

* per-row taken with threading OFF — the whole point of the helper is lost
  and 8.4 M registrations per frame come back, with no test going red.
* hull taken with threading ON — the hull reserves the inter-row gaps, which
  belong to other tile COLUMNS. A neighbouring tile's legitimate write then
  trips a spurious `overlapping DisjointMut` panic (measured 8-9 of 24
  concurrent runs before [`set_tile_threading`] was made monotone), or, in
  an `unchecked` build, races undetected.

So each test asserts which EXTENT got reserved, by holding a byte that only
the hull covers and checking whether the tracker rejects the call."###]
#[cfg(all(test, not(feature = "unchecked")))]
mod row_guard_policy_tests {
  use super::{
    Rav1dPictureDataComponent, Rav1dPictureDataComponentInner, RowLayoutError,
    set_tile_threading, tile_threading_active,
  };
  use crate::include::common::bitdepth::BitDepth8;
  use crate::src::managed::Error as DecoderError;
  use crate::src::with_offset::WithOffset;
  use console_core_geometry::{Point, Size};
  use console_core_never::Never;
  use console_core_number_conversion::{fixed_index, index, wrapped_offset};
  use std::panic::{self, AssertUnwindSafe};

  const STRIDE: u64 = 64;
  const ROWS: u64 = 4;
  const W: u64 = 8;
  const SPAN: u64 = ROWS.wrapping_sub(1).wrapping_mul(STRIDE);
  const HULL: u64 = SPAN.wrapping_add(W);
  const EXTENT: Size<u64> = Size {
    width: W,
    height: ROWS,
  };
  const BU_MEM_ROWS: u64 = ROWS.wrapping_add(2);
  const BU_TOP_ROW: u64 = BU_MEM_ROWS.wrapping_sub(2);
  const ROW0: u64 = BU_TOP_ROW.wrapping_mul(STRIDE);

  fn plane() -> Result<Rav1dPictureDataComponent, Never> {
    let mut buf = [0_u8; 256];
    let Ok(stride) = index(STRIDE);
    Ok(Rav1dPictureDataComponent::wrap_buf::<BitDepth8>(
      &mut buf, stride,
    ))
  }

  #[derive(Debug, PartialEq, Eq)]
  enum Reservation {
    Conflicts,
    Available,
  }

  fn reservation(outcome: std::thread::Result<()>) -> Result<Reservation, Never> {
    Ok(match outcome {
      Ok(()) => Reservation::Available,
      Err(_overlapping_guard) => Reservation::Conflicts,
    })
  }

  #[test]
  fn explicit_picture_policy_controls_gap_reservations_after_global_promotion() {
    set_tile_threading(true);

    for (workers, expected) in [(1, Reservation::Conflicts), (8, Reservation::Available)] {
      let Ok(mut pic) = plane();
      let counts = super::PictureThreadingCounts { workers, tiles: 4 };
      let Ok(policy) = super::PictureThreading::new(counts);
      let Ok(()) = pic.set_threading_policy(policy);
      let Ok(gap) = index(W);
      let held = pic.slice_mut::<BitDepth8, _>((gap.., ..1));
      let top = WithOffset {
        data: &pic,
        offset: 0,
      };
      let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        let Ok(width) = index(W);
        let Ok(height) = index(ROWS);
        top.for_rows_mut::<BitDepth8, _>(width, height, |_, row| row.fill(1));
      }));
      let Ok(actual) = reservation(outcome);
      assert_eq!(actual, expected, "workers={workers}");
      drop(held);
    }
  }

  fn conflicts_with(probe: u64) -> Result<Reservation, Never> {
    let Ok(pic) = plane();
    let Ok(position) = index(probe);
    let held = pic.slice_mut::<BitDepth8, _>((position.., ..1));
    let top = WithOffset {
      data: &pic,
      offset: 0,
    };
    let prev = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
      let Ok(width) = index(W);
      let Ok(height) = index(ROWS);
      top.for_rows_mut::<BitDepth8, _>(width, height, |_, row| row.fill(1));
    }));
    panic::set_hook(prev);
    drop(held);
    reservation(outcome)
  }

  #[derive(Debug)]
  enum FixtureError {
    Process(std::io::Error),
    Layout(RowLayoutError),
    Decode(DecoderError),
  }

  impl core::fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
      match self {
        Self::Process(error) => write!(formatter, "test process: {error}"),
        Self::Layout(error) => write!(formatter, "test picture: {error:?}"),
        Self::Decode(error) => write!(formatter, "test decoder: {error:?}"),
      }
    }
  }

  enum FixtureExecution {
    Parent,
    Child,
  }

  const CHILD_ENV: &str = "RAV1D_ROW_GUARD_CHILD";

  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    expect(
      explicit026_env_read_once,
      reason = "RAV1D_ROW_GUARD_CHILD selects the test fixture process in this module"
    )
  )]
  fn fixture_execution() -> Result<FixtureExecution, Never> {
    Ok(match std::env::var_os(CHILD_ENV) {
      Some(_) => FixtureExecution::Child,
      None => FixtureExecution::Parent,
    })
  }

  struct ChildFixture<'a> {
    name: &'a str,
    marker: &'static str,
  }

  fn in_child_process(
    fixture: ChildFixture<'_>,
    body: impl FnOnce() -> Result<(), FixtureError>,
  ) -> Result<(), FixtureError> {
    let ChildFixture { name, marker } = fixture;
    let Ok(execution) = fixture_execution();

    match execution {
      FixtureExecution::Child => {
        assert!(
          !tile_threading_active(),
          "the child must start before any decoder latches tile threading"
        );
        body()?;
        println!("{marker}");
      }
      FixtureExecution::Parent => {
        let exe = std::env::current_exe().map_err(FixtureError::Process)?;
        let out = std::process::Command::new(exe)
          .args(["--exact", name, "--nocapture"])
          .env(CHILD_ENV, "1")
          .output()
          .map_err(FixtureError::Process)?;
        let stdout = core::str::from_utf8(&out.stdout);
        let stderr = core::str::from_utf8(&out.stderr);
        assert!(
          matches!(stdout, Ok(output) if output.contains(marker)) && out.status.success(),
          "child did not finish fixture {name} successfully. status={:?}\nstdout={stdout:?}\nstderr={stderr:?}",
          out.status,
        );
      }
    }

    Ok(())
  }

  const TESTS: &str = "include::dav1d::picture::row_guard_policy_tests::";

  #[test]
  fn for_rows_mut_reserves_the_strided_hull_when_tile_threading_is_off()
  -> Result<(), FixtureError> {
    in_child_process(
      ChildFixture {
        name: &format!(
          "{TESTS}for_rows_mut_reserves_the_strided_hull_when_tile_threading_is_off"
        ),
        marker: "ROW_GUARD_HULL_CHILD_RAN",
      },
      || {
        let Ok(gap) = conflicts_with(W);
        let Ok(outside) = conflicts_with(HULL);
        assert_eq!(
          gap,
          Reservation::Conflicts,
          "the whole hull must reserve inter-row gaps"
        );
        assert_eq!(
          outside,
          Reservation::Available,
          "a byte outside the hull must stay available"
        );
        Ok(())
      },
    )
  }

  fn stamp(point: Point<u64>) -> Result<u8, Never> {
    let Point { x, y } = point;
    console_core_number_conversion::wrapped(y.wrapping_mul(16).wrapping_add(x))
  }

  fn bottom_up_plane() -> Result<Rav1dPictureDataComponent, Never> {
    let mut buf = [0_u8; 384];
    let (memory_rows, _remainder) = buf.as_chunks_mut::<64>();

    for (mem_row, bytes) in (0..BU_MEM_ROWS).zip(memory_rows) {
      for (x, pixel) in (0..16).zip(bytes.iter_mut()) {
        *pixel = match BU_TOP_ROW.checked_sub(mem_row) {
          Some(y) => {
            let Ok(value) = stamp(Point { x, y });
            value
          }
          None => 0xEE,
        };
      }
    }

    let inner = Rav1dPictureDataComponentInner::from_slice_copy(&buf);
    Rav1dPictureDataComponent::from_parts(inner, STRIDE.cast_signed().wrapping_neg())
  }

  #[test]
  fn checked_rows_write_directly_in_picture_order() -> Result<(), RowLayoutError> {
    for workers in [1, 8] {
      let Ok(mut pic) = bottom_up_plane();
      let counts = super::PictureThreadingCounts { workers, tiles: 4 };
      let Ok(policy) = super::PictureThreading::new(counts);
      let Ok(()) = pic.set_threading_policy(policy);
      let Ok(origin) = index(ROW0);
      let top = WithOffset {
        data: &pic,
        offset: origin,
      };
      let invalid = WithOffset {
        data: &pic,
        offset: 0,
      };
      assert!(matches!(
        invalid.checked_for_rows_mut::<BitDepth8>(EXTENT, |_, row| row.fill(0)),
        Err(RowLayoutError::Bounds)
      ));

      top.checked_for_rows_mut::<BitDepth8>(EXTENT, |position, row| {
        let Ok(y) = fixed_index(position);
        assert!(row.iter().copied().eq((0..W).map(|x| {
          let Ok(value) = stamp(Point { x, y });
          value
        })));
        row.fill(200);
      })?;

      let extent = Size {
        width: W.wrapping_add(1),
        height: ROWS,
      };
      let Ok(()) = top.for_rows_with_context::<BitDepth8, _, _>(extent, &mut (), |y, row, ()| {
        let Ok(width) = index(W);
        let Ok(neighbor) = stamp(Point { x: W, y });
        assert!(row.iter().take(width).all(|&sample| sample == 200));
        assert_eq!(row.last(), Some(&neighbor));
      });
    }

    Ok(())
  }

  fn check_bottom_up_hull(samples: &[u8], origin: u64) -> Result<(), FixtureError> {
    for y in 0..ROWS {
      for x in 0..W {
        let position = origin.wrapping_add(x).wrapping_sub(y.wrapping_mul(STRIDE));
        let Ok(offset) = index(position);
        let Ok(expected) = stamp(Point { x, y });
        assert_eq!(samples.get(offset), Some(&expected), "row {y} col {x}");
      }
    }

    Ok(())
  }

  #[test]
  fn narrow_guards_cover_the_block_on_a_negative_stride() -> Result<(), FixtureError> {
    in_child_process(
      ChildFixture {
        name: &format!("{TESTS}narrow_guards_cover_the_block_on_a_negative_stride"),
        marker: "NEG_STRIDE_NARROW_GUARDS_CHILD_RAN",
      },
      || {
        let Ok(pic) = bottom_up_plane();
        let Ok(origin) = index(ROW0);
        let top = WithOffset {
          data: &pic,
          offset: origin,
        };
        let Ok(width) = index(W);
        let Ok(height) = index(ROWS);
        let Ok(hull_length) = index(HULL);

        {
          #[cfg_attr(
            dylint_lib = "explicit051_no_machine_width",
            expect(
              explicit051_no_machine_width,
              reason = "the test asserts the native hull origin through the existing public guard API"
            )
          )]
          let (guard, base) = top.narrow_guard::<BitDepth8>(width, height);
          let Ok(first) = fixed_index(base);
          assert_eq!(guard.len(), hull_length);
          assert_eq!(first, SPAN);
          check_bottom_up_hull(&guard, first)?;
        }

        #[cfg_attr(
          dylint_lib = "explicit051_no_machine_width",
          expect(
            explicit051_no_machine_width,
            reason = "the test asserts the native mutable hull origin through the existing public guard API"
          )
        )]
        let (guard, base) = top.narrow_guard_mut::<BitDepth8>(width, height);
        let Ok(first) = fixed_index(base);
        assert_eq!(guard.len(), hull_length);
        assert_eq!(first, SPAN);
        check_bottom_up_hull(&guard, first)
      },
    )
  }

  #[test]
  fn for_rows_visits_picture_rows_top_down_on_a_negative_stride() -> Result<(), FixtureError> {
    fn check() -> Result<(), FixtureError> {
      let Ok(pic) = bottom_up_plane();
      let Ok(origin) = index(ROW0);
      let top = WithOffset {
        data: &pic,
        offset: origin,
      };
      let mut seen = 0_u64;
      let Ok(()) =
        top.for_rows_with_context::<BitDepth8, _, _>(EXTENT, &mut seen, |y, row, count| {
          let Ok(width) = index(W);
          assert_eq!(row.len(), width);

          for (x, &pixel) in (0_u64..W).zip(row) {
            let Ok(expected) = stamp(Point { x, y });
            assert_eq!(pixel, expected, "row {y} col {x}");
          }

          *count = count.wrapping_add(1);
        });
      assert_eq!(seen, ROWS);
      let Ok(()) =
        top.for_rows_mut_with_context::<BitDepth8, _, Never>(EXTENT, &mut (), |y, row, ()| {
          for (x, pixel) in (0_u64..W).zip(row) {
            let Ok(expected) = stamp(Point { x, y });
            assert_eq!(*pixel, expected, "row {y} col {x}");
            *pixel ^= 0x80;
          }

          Ok(())
        });
      let Ok(()) = top.for_rows_with_context::<BitDepth8, _, _>(EXTENT, &mut (), |y, row, ()| {
        for (x, &pixel) in (0_u64..W).zip(row) {
          let Ok(expected) = stamp(Point { x, y });
          assert_eq!(pixel, expected ^ 0x80, "row {y} col {x} after write");
        }
      });
      Ok(())
    }

    let Ok(execution) = fixture_execution();

    match execution {
      FixtureExecution::Parent => check()?,
      FixtureExecution::Child => {}
    }

    in_child_process(
      ChildFixture {
        name: &format!("{TESTS}for_rows_visits_picture_rows_top_down_on_a_negative_stride"),
        marker: "NEG_STRIDE_FOR_ROWS_CHILD_RAN",
      },
      check,
    )
  }

  #[test]
  fn compact_read_fast_hull_starts_at_the_last_row_on_a_negative_stride()
  -> Result<(), FixtureError> {
    in_child_process(
      ChildFixture {
        name: &format!(
          "{TESTS}compact_read_fast_hull_starts_at_the_last_row_on_a_negative_stride"
        ),
        marker: "NEG_STRIDE_COMPACT_READ_FAST_CHILD_RAN",
      },
      || {
        let Ok(pic) = bottom_up_plane();
        let Ok(origin) = index(ROW0);
        let top = WithOffset {
          data: &pic,
          offset: origin,
        };
        let Ok((buf, byte_stride)) = top.compact_read_fast::<BitDepth8>(EXTENT);
        let Ok(length) = index(HULL);
        assert_eq!(byte_stride, STRIDE);
        assert_eq!(buf.len(), length);
        check_bottom_up_hull(&buf, SPAN)
      },
    )
  }

  #[test]
  fn compact_read_returns_picture_rows_top_down_on_a_negative_stride() -> Result<(), FixtureError>
  {
    in_child_process(
      ChildFixture {
        name: &format!("{TESTS}compact_read_returns_picture_rows_top_down_on_a_negative_stride"),
        marker: "NEG_STRIDE_COMPACT_READ_CHILD_RAN",
      },
      || {
        let Ok(pic) = bottom_up_plane();
        let Ok(origin) = index(ROW0);
        let top = WithOffset {
          data: &pic,
          offset: origin,
        };
        let Ok(width) = index(W);
        let Ok(height) = index(ROWS);
        #[cfg_attr(
          dylint_lib = "explicit051_no_machine_width",
          expect(
            explicit051_no_machine_width,
            reason = "the compatibility view exposes its native stride for the test to verify"
          )
        )]
        let (buf, byte_stride) = top.compact_read::<BitDepth8>(width, height);
        let Ok(stride) = fixed_index(byte_stride);
        assert_eq!(stride, W, "compact layout starts at picture row zero");

        for y in 0..ROWS {
          for x in 0..W {
            let Ok(offset) = index(y.wrapping_mul(stride).wrapping_add(x));
            let Ok(expected) = stamp(Point { x, y });
            assert_eq!(buf.get(offset), Some(&expected), "row {y} col {x}");
          }
        }

        Ok(())
      },
    )
  }

  #[cfg(feature = "bitdepth_16")]
  #[test]
  fn compact_difference_touches_only_changed_pixels_on_negative_stride()
  -> Result<(), super::RowLayoutError> {
    use crate::include::common::bitdepth::BitDepth16;
    use zerocopy::IntoBytes;

    let pixels: [u16; 24] = core::array::from_fn(|sample| {
      let Ok(value) = console_core_number_conversion::wrapped::<_, u16>(sample);
      value
    });
    let allocation = Rav1dPictureDataComponentInner::from_slice_copy(pixels.as_bytes());
    let Ok(mut picture) = Rav1dPictureDataComponent::from_parts(allocation, -16);
    let counts = super::PictureThreadingCounts {
      workers: 8,
      tiles: 4,
    };
    let Ok(policy) = super::PictureThreading::new(counts);
    let Ok(()) = picture.set_threading_policy(policy);
    let top = WithOffset {
      data: &picture,
      offset: 16,
    };
    #[cfg_attr(
      dylint_lib = "explicit051_no_machine_width",
      expect(
        explicit051_no_machine_width,
        reason = "the test verifies the native byte stride returned by the public compact view"
      )
    )]
    let (pristine, stride) = top.compact_read_per_row::<BitDepth16>(5, 3);
    assert_eq!(stride, 10);
    let mut filtered = pristine.clone();
    let changed_byte = filtered.get_mut(5).ok_or(super::RowLayoutError::Bounds)?;
    *changed_byte ^= 0x40;

    let unchanged_prefix = picture.slice_mut::<BitDepth16, _>((16.., ..1));
    let unchanged_row = picture.slice_mut::<BitDepth16, _>((8.., ..5));
    top.compact_write_back_per_row_diff::<BitDepth16>(5, 3, &filtered, &pristine);
    drop(unchanged_row);
    drop(unchanged_prefix);

    let (rendered, _) = top.compact_read_per_row::<BitDepth16>(5, 3);
    assert_eq!(rendered, filtered);
    Ok(())
  }

  fn bottom_up_block_conflicts_with(probe: u64) -> Result<Reservation, Never> {
    let Ok(pic) = bottom_up_plane();
    let Ok(position) = index(probe);
    let held = pic.slice_mut::<BitDepth8, _>((position.., ..1));
    let Ok(origin) = index(ROW0);
    let top = WithOffset {
      data: &pic,
      offset: origin,
    };
    let prev = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
      let Ok(width) = index(W);
      let Ok(height) = index(ROWS);
      let (guard, _base) = top.narrow_guard_mut::<BitDepth8>(width, height);
      std::hint::black_box(guard.len());
    }));
    panic::set_hook(prev);
    drop(held);
    reservation(outcome)
  }

  #[test]
  fn narrow_guard_mut_reserves_exactly_the_block_hull_on_a_negative_stride()
  -> Result<(), FixtureError> {
    in_child_process(
      ChildFixture {
        name: &format!(
          "{TESTS}narrow_guard_mut_reserves_exactly_the_block_hull_on_a_negative_stride"
        ),
        marker: "NEG_STRIDE_RESERVATION_CHILD_RAN",
      },
      || {
        let last_column = ROW0.wrapping_add(W).wrapping_sub(1);
        let last_row = ROW0.wrapping_sub(SPAN);
        let past_row = ROW0.wrapping_add(W);
        let before_row = last_row.wrapping_sub(1);
        let Ok(column) = bottom_up_block_conflicts_with(last_column);
        let Ok(row) = bottom_up_block_conflicts_with(last_row);
        let Ok(after) = bottom_up_block_conflicts_with(past_row);
        let Ok(before) = bottom_up_block_conflicts_with(before_row);
        assert_eq!(
          column,
          Reservation::Conflicts,
          "the last column of row zero is held"
        );
        assert_eq!(
          row,
          Reservation::Conflicts,
          "the first column of the last row is held"
        );
        assert_eq!(
          after,
          Reservation::Available,
          "the adjacent column is available"
        );
        assert_eq!(
          before,
          Reservation::Available,
          "the preceding byte is available"
        );
        Ok(())
      },
    )
  }

  #[test]
  fn for_rows_mut_never_reserves_an_inter_row_gap_when_tile_threading_is_on() {
    set_tile_threading(true);
    assert!(tile_threading_active());
    let Ok(gap) = conflicts_with(W);
    let Ok(row) = conflicts_with(STRIDE);
    assert_eq!(
      gap,
      Reservation::Available,
      "tile workers may borrow inter-row gaps"
    );
    assert_eq!(row, Reservation::Conflicts, "the next picture row is held");
  }

  #[test]
  fn live_block_keeps_its_storage_when_another_decoder_changes_threading()
  -> Result<(), FixtureError> {
    in_child_process(
      ChildFixture {
        name: &format!(
          "{TESTS}live_block_keeps_its_storage_when_another_decoder_changes_threading"
        ),
        marker: "LIVE_BLOCK_THREADING_TRANSITION_RAN",
      },
      check_live_block,
    )
  }

  fn check_live_block() -> Result<(), FixtureError> {
    use crate::src::managed::{Decoder, Settings};

    let Ok(pic) = plane();
    let top = WithOffset {
      data: &pic,
      offset: 0,
    };
    let Ok(width) = index(W);
    let Ok(height) = index(ROWS);
    let Ok(full_stride) = wrapped_offset(STRIDE.cast_signed());
    let Ok(compact_stride) = wrapped_offset(W.cast_signed());
    let mut old = top.block_mut::<BitDepth8>(width, height);
    assert_eq!(old.byte_stride(), full_stride);
    assert!(matches!(old.storage, super::BlockMutStorage::Direct { .. }));

    let threaded = Decoder::with_workers(
      Settings {
        threads: 2,
        max_frame_delay: 1,
        ..Default::default()
      },
      &console_concurrency::HostWorkers,
    )
    .map_err(|fault| FixtureError::Decode(fault.decompose().0))?;
    let single_threaded = std::thread::spawn(|| {
      for _ in 0..4 {
        let decoder = Decoder::with_settings(Settings {
          threads: 1,
          ..Default::default()
        })
        .map_err(|fault| FixtureError::Decode(fault.decompose().0))?;
        assert!(tile_threading_active());
        drop(decoder);
      }

      Ok(())
    });
    #[cfg_attr(
      dylint_lib = "explicit057_no_unbounded_wait",
      expect(
        explicit057_no_unbounded_wait,
        reason = "the fixture joins its one worker after four fixed decoder constructions before checking latch preservation"
      )
    )]
    let outcome = single_threaded.join();

    match outcome {
      Ok(result) => result?,
      Err(panic) => std::panic::resume_unwind(panic),
    }

    assert_eq!(old.byte_stride(), full_stride);
    paint_first_column(old.as_mut_bytes(), STRIDE, 7)?;
    drop(old);

    let mut gap = pic.slice_mut::<BitDepth8, _>((width.., ..1));
    let sample = gap
      .first_mut()
      .ok_or(FixtureError::Layout(RowLayoutError::Bounds))?;
    *sample = 11;
    let mut new = top.block_mut::<BitDepth8>(width, height);
    assert_eq!(new.byte_stride(), compact_stride);
    paint_first_column(new.as_mut_bytes(), W, 9)?;
    drop(new);
    assert_eq!(gap.first(), Some(&11));
    drop(gap);

    for y in 0..ROWS {
      let Ok(offset) = index(y.wrapping_mul(STRIDE));
      let row = pic.slice::<BitDepth8, _>((offset.., ..1));
      assert_eq!(row.first(), Some(&9));
    }

    drop(threaded);
    assert!(tile_threading_active());
    Ok(())
  }

  fn paint_first_column(samples: &mut [u8], stride: u64, value: u8) -> Result<(), FixtureError> {
    for y in 0..ROWS {
      let Ok(offset) = index(y.wrapping_mul(stride));
      let pixel = samples
        .get_mut(offset)
        .ok_or(FixtureError::Layout(RowLayoutError::Bounds))?;
      *pixel = value;
    }

    Ok(())
  }
}
