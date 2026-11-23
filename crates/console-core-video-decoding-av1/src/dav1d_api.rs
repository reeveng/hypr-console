//! C FFI wrappers for the dav1d/rav1d ABI.
//!
//! All 19 `dav1d_*` `extern "C"` entry points live here, gated behind
//! `#[cfg(feature = "c-ffi")]` at the module level (see crate root `lib.rs`).
//! The Rust core API (`rav1d_*`) stays in `src/lib.rs`.
//!
//! ## Symbol naming
//!
//! When `dav1d-compat` feature is enabled (default with `c-ffi`), symbols are
//! exported as `dav1d_*` for drop-in replacement of libdav1d.
//!
//! When `dav1d-compat` is disabled, symbols are exported as `rav1d_*`, allowing
//! side-by-side loading with real libdav1d for benchmarking.
#![allow(
  unsafe_code,
  reason = "this module implements the public C pointer and callback ABI"
)]
#![deny(unsafe_op_in_unsafe_fn)]

use crate::include::common::validate::validate_input;
use crate::include::dav1d::common::Dav1dDataProps;
use crate::include::dav1d::common::Rav1dDataProps;
use crate::include::dav1d::data::Dav1dData;
use crate::include::dav1d::dav1d::Dav1dContext;
use crate::include::dav1d::dav1d::Dav1dEventFlags;
use crate::include::dav1d::dav1d::Dav1dSettings;
use crate::include::dav1d::headers::Dav1dSequenceHeader;
use crate::include::dav1d::picture::Dav1dPicture;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::src::c_arc::RawArc;
use crate::src::c_box::FnFree;
use crate::src::error::Dav1dResult;
use crate::src::error::Rav1dError::{EAGAIN, EINVAL};
use crate::src::error::Rav1dResult;
use crate::src::lib::DAV1D_API_VERSION_MAJOR;
use crate::src::lib::DAV1D_API_VERSION_MINOR;
use crate::src::lib::DAV1D_API_VERSION_PATCH;
use crate::src::lib::DAV1D_VERSION;
use crate::src::lib::rav1d_apply_grain;
use crate::src::lib::rav1d_close;
use crate::src::lib::rav1d_flush;
use crate::src::lib::rav1d_get_frame_delay;
use crate::src::lib::rav1d_get_picture;
use crate::src::lib::rav1d_open;
use crate::src::lib::rav1d_send_data;
use crate::src::obu::rav1d_parse_sequence_header;
use crate::src::send_sync_non_null::Dav1dOpaquePointer;
use console_concurrency::HostWorkers;
use console_core_api_compatibility::NativeSize;
use std::ffi::c_char;
use std::ffi::c_uint;
use std::mem;
use std::ptr;
use std::ptr::NonNull;
use std::slice;

use crate::include::dav1d::data::Rav1dData;
use crate::include::dav1d::dav1d::Rav1dSettings;

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "a slice borrowed from a C pointer cannot span more than the address space's signed offset limit"
  )
)]
const MAX_C_BUFFER_BYTES: NativeSize = isize::MAX.cast_unsigned();

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_version"))]
#[cfg_attr(not(feature = "dav1d-compat"), unsafe(export_name = "rav1d_version"))]
#[cold]
pub extern "C" fn dav1d_version() -> *const c_char {
  DAV1D_VERSION.as_ptr()
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_version_api"))]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_version_api")
)]
#[cold]
pub extern "C" fn dav1d_version_api() -> c_uint {
  u32::from_be_bytes([
    0,
    DAV1D_API_VERSION_MAJOR,
    DAV1D_API_VERSION_MINOR,
    DAV1D_API_VERSION_PATCH,
  ])
}

#[cfg_attr(
  feature = "dav1d-compat",
  unsafe(export_name = "dav1d_default_settings")
)]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_default_settings")
)]
#[cold]
#[doc = "\n# Safety\n`output` must be writable storage for a C settings record."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller supplies writable output storage"
  )
)]
pub unsafe extern "C" fn dav1d_default_settings(output: NonNull<Dav1dSettings>) {
  let settings = Dav1dSettings::from(Rav1dSettings::default());

  // SAFETY: The caller supplies writable storage, which may be uninitialized.
  unsafe { output.as_ptr().write(settings) };
}

#[cfg_attr(
  feature = "dav1d-compat",
  unsafe(export_name = "dav1d_get_frame_delay")
)]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_get_frame_delay")
)]
#[cold]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the C caller supplies a live initialized settings record, which is copied and never claimed as owned storage"
  )
)]
#[doc = "\n# Safety\nA non-null `settings` must point to an initialized C settings record for the duration of the call."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the settings record is supplied by the C caller"
  )
)]
pub unsafe extern "C" fn dav1d_get_frame_delay(
  settings: Option<NonNull<Dav1dSettings>>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let settings_ptr = validate_input!(settings.ok_or(EINVAL))?;
    // SAFETY: The C settings record is initialized and owns no resources.
    let c_settings = unsafe { settings_ptr.as_ptr().read() };
    let rust_settings = c_settings.into_rust()?;

    rav1d_get_frame_delay(&rust_settings, &HostWorkers)
  })())
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_open"))]
#[cfg_attr(not(feature = "dav1d-compat"), unsafe(export_name = "rav1d_open"))]
#[cold]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the C caller supplies an initialized readable settings struct for the duration of the call"
  )
)]
#[doc = "\n# Safety\nNon-null `output` must be writable decoder handle storage. Non-null `settings` must point to an initialized C settings record that may be read for the duration of the call."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller supplies the readable settings and writable output locations"
  )
)]
pub unsafe extern "C" fn dav1d_open(
  output: Option<NonNull<Option<Dav1dContext>>>,
  settings: Option<NonNull<Dav1dSettings>>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    let settings_ptr = validate_input!(settings.ok_or(EINVAL))?;
    // SAFETY: The C settings record is initialized and owns no resources.
    let c_settings = unsafe { settings_ptr.as_ptr().read() };
    let rust_settings = c_settings.into_rust()?;
    let (decoder, workers) = match rav1d_open(&rust_settings, &HostWorkers) {
      Ok(opened) => opened,
      Err(open_failure) => {
        // SAFETY: The caller supplies writable handle storage, which may be uninitialized.
        unsafe { output_slot.as_ptr().write(None) };

        return Err(open_failure);
      }
    };

    *decoder.c_workers.lock() = workers;
    let Ok(raw_context) = RawArc::from_arc(decoder);

    // SAFETY: The caller supplies writable handle storage, which may be uninitialized.
    unsafe { output_slot.as_ptr().write(Some(raw_context)) };

    Ok(())
  })())
}

#[cfg_attr(
  feature = "dav1d-compat",
  unsafe(export_name = "dav1d_parse_sequence_header")
)]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_parse_sequence_header")
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the byte count is checked against isize::MAX and the C caller supplies a readable buffer of that length"
  )
)]
#[doc = "\n# Safety\nNon-null `output` must point to writable storage. Non-null `input` must point to `size` initialized readable bytes for the duration of the call."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller supplies the readable packet and writable sequence header storage"
  )
)]
pub unsafe extern "C" fn dav1d_parse_sequence_header(
  output: Option<NonNull<Dav1dSequenceHeader>>,
  input: Option<NonNull<u8>>,
  size: NativeSize,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    let input_buffer = validate_input!(input.ok_or(EINVAL))?;
    validate_input!((size > 0 && size <= MAX_C_BUFFER_BYTES, EINVAL))?;
    // SAFETY: The caller lends a readable packet of the checked size.
    let bytes = unsafe { slice::from_raw_parts(input_buffer.as_ptr(), size) };
    let header = rav1d_parse_sequence_header(bytes)?;

    // SAFETY: The caller supplies writable storage for the sequence header.
    unsafe { output_slot.as_ptr().write(header.dav1d) };

    Ok(())
  })())
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_send_data"))]
#[cfg_attr(not(feature = "dav1d-compat"), unsafe(export_name = "rav1d_send_data"))]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the C caller supplies an initialized input and transfers its data reference during the call; the remaining ownership is written back to that slot"
  )
)]
#[doc = "\n# Safety\nNon-null `context` must be a live decoder. Non-null `input` must point to an initialized C packet and remain exclusively accessible during this call."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller lends a live decoder and transfers the initialized packet during this call"
  )
)]
pub unsafe extern "C" fn dav1d_send_data(
  context: Option<Dav1dContext>,
  input: Option<NonNull<Dav1dData>>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let context_handle = validate_input!(context.ok_or(EINVAL))?;
    let input_slot = validate_input!(input.ok_or(EINVAL))?;
    // SAFETY: The caller keeps the decoder alive throughout the call.
    let Ok(decoder) = unsafe { context_handle.as_ref() };
    // SAFETY: The caller transfers exclusive access to the initialized packet.
    let c_packet = unsafe { input_slot.as_ptr().read() };
    let mut packet = Rav1dData::from(c_packet);
    let result = rav1d_send_data(decoder, &mut packet);

    // SAFETY: The caller has lent the writable packet slot until its contents are restored.
    unsafe { input_slot.as_ptr().write(Dav1dData::from(packet)) };

    result
  })())
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_get_picture"))]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_get_picture")
)]
#[doc = "\n# Safety\nNon-null `context` must be a live decoder and non-null `output` must be writable picture storage."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller lends the decoder and supplies writable output storage"
  )
)]
pub unsafe extern "C" fn dav1d_get_picture(
  context: Option<Dav1dContext>,
  output: Option<NonNull<Dav1dPicture>>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let context_handle = validate_input!(context.ok_or(EINVAL))?;
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    // SAFETY: The caller keeps the decoder alive during this call.
    let Ok(decoder) = unsafe { context_handle.as_ref() };
    let mut picture = Rav1dPicture::default();
    let result = rav1d_get_picture(decoder, &mut picture);

    // SAFETY: The caller supplies writable picture storage.
    unsafe { output_slot.as_ptr().write(Dav1dPicture::from(picture)) };

    result
  })())
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_apply_grain"))]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_apply_grain")
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "for in-place application, the successful output replaces one initialized C picture and releases exactly its owned reference"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller lends the input and provides writable output storage, which may be the input slot itself"
  )
)]
#[doc = "\n# Safety\nNon-null `c` must be a live decoder and non-null `input` an initialized picture. `output` must be writable and either be the input slot or be disjoint from it. The caller must keep the decoder and picture alive and serialize access to the slots during the call."]
pub unsafe extern "C" fn dav1d_apply_grain(
  c: Option<Dav1dContext>,
  output: Option<NonNull<Dav1dPicture>>,
  input: Option<NonNull<Dav1dPicture>>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let context_handle = validate_input!(c.ok_or(EINVAL))?;
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    let input_slot = validate_input!(input.ok_or(EINVAL))?;

    // SAFETY: The caller keeps the decoder alive throughout the call.
    let Ok(context) = unsafe { context_handle.as_ref() };
    // SAFETY: The caller keeps the initialized input and its references alive during the borrow.
    let source = unsafe { Rav1dPicture::from_borrowed(input_slot.as_ref()) }?;

    let mut destination = Rav1dPicture::default();
    let result = rav1d_apply_grain(context, &mut destination, &source);

    match ptr::eq(output_slot.as_ptr(), input_slot.as_ptr()).then_some(()) {
      Some(()) => {
        result?;

        // SAFETY: Both pointers address the same initialized record, which is being replaced.
        let previous = unsafe { input_slot.as_ptr().read() };
        drop(Rav1dPicture::try_from(previous));
      }
      None => {}
    }

    // SAFETY: The caller provides writable output, and any former in-place input was reclaimed.
    unsafe { output_slot.as_ptr().write(Dav1dPicture::from(destination)) };

    result
  })())
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_flush"))]
#[cfg_attr(not(feature = "dav1d-compat"), unsafe(export_name = "rav1d_flush"))]
#[doc = "\n# Safety\n`context` must refer to a live decoder throughout the call. The caller must serialize flush with other operations on that decoder."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller owns the live decoder context"
  )
)]
pub unsafe extern "C" fn dav1d_flush(context: Dav1dContext) {
  // SAFETY: The caller keeps the decoder alive during this call.
  let Ok(decoder) = unsafe { context.as_ref() };

  let Ok(()) = rav1d_flush(decoder);
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_close"))]
#[cfg_attr(not(feature = "dav1d-compat"), unsafe(export_name = "rav1d_close"))]
#[cold]
#[doc = "\n# Safety\nA non-null `context` must point to an initialized, exclusively accessible handle slot. A present handle must own the reference returned by `dav1d_open`."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller transfers its decoder token and grants writable access to the handle slot"
  )
)]
pub unsafe extern "C" fn dav1d_close(context: Option<NonNull<Option<Dav1dContext>>>) {
  let mut context_slot = match validate_input!(context.ok_or(())) {
    Ok(slot) => slot,
    Err(_null_context_slot) => return,
  };

  // SAFETY: The caller grants exclusive access to the initialized slot.
  let handle = mem::take(unsafe { context_slot.as_mut() });

  match handle {
    Some(owned) => {
      // SAFETY: The slot transferred its one reference from dav1d_open.
      let Ok(decoder) = unsafe { owned.into_arc() };
      let Ok(()) = rav1d_close(decoder);
    }
    None => {}
  }
}

#[cfg_attr(
  feature = "dav1d-compat",
  unsafe(export_name = "dav1d_get_event_flags")
)]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_get_event_flags")
)]
#[doc = "\n# Safety\nNon-null `context` must be a live decoder. Non-null `output` must point to writable storage for the event flags."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller supplies a live decoder and writable flags storage"
  )
)]
pub unsafe extern "C" fn dav1d_get_event_flags(
  context: Option<Dav1dContext>,
  output: Option<NonNull<Dav1dEventFlags>>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let context_handle = validate_input!(context.ok_or(EINVAL))?;
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    // SAFETY: The caller keeps the decoder alive through this call.
    let Ok(decoder) = unsafe { context_handle.as_ref() };
    let flags = {
      let mut state = decoder.state.try_lock().ok_or(EAGAIN)?;
      mem::take(&mut state.event_flags)
    };
    let c_flags = Dav1dEventFlags::from(flags);

    // SAFETY: The caller supplies writable flags storage.
    unsafe { output_slot.as_ptr().write(c_flags) };

    Ok(())
  })())
}

#[cfg_attr(
  feature = "dav1d-compat",
  unsafe(export_name = "dav1d_get_decode_error_data_props")
)]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_get_decode_error_data_props")
)]
#[doc = "\n# Safety\nNon-null `context` must point to a live decoder. Non-null `output` must point to writable storage for the data properties."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller supplies a live decoder and writable data properties storage"
  )
)]
pub unsafe extern "C" fn dav1d_get_decode_error_data_props(
  context: Option<Dav1dContext>,
  output: Option<NonNull<Dav1dDataProps>>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let context_handle = validate_input!(context.ok_or(EINVAL))?;
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    // SAFETY: The caller keeps the decoder alive through this call.
    let Ok(decoder) = unsafe { context_handle.as_ref() };
    let properties = {
      let mut state = decoder.state.try_lock().ok_or(EAGAIN)?;
      mem::take(&mut state.cached_error_props)
    };
    let c_properties = Dav1dDataProps::from(properties);

    // SAFETY: The caller supplies writable data properties storage.
    unsafe { output_slot.as_ptr().write(c_properties) };

    Ok(())
  })())
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_picture_unref"))]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_picture_unref")
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the C caller supplies one initialized picture and transfers its reference before the slot is written back empty"
  )
)]
#[doc = "\n# Safety\nA non-null `picture` must point to an initialized, exclusively accessible C picture that owns any of its present references."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller transfers the picture tokens and grants writable access to its slot"
  )
)]
pub unsafe extern "C" fn dav1d_picture_unref(picture: Option<NonNull<Dav1dPicture>>) {
  let slot = match validate_input!(picture.ok_or(())) {
    Ok(picture_ptr) => picture_ptr,
    Err(_null_picture_slot) => return,
  };

  // SAFETY: The caller transfers the live references for this exclusive picture slot.
  let c_picture = unsafe { slot.as_ptr().read() };
  let owned = Rav1dPicture::try_from(c_picture);
  let cleared = Dav1dPicture {
    m: Dav1dDataProps::from(Rav1dDataProps::default()),
    ..Dav1dPicture::default()
  };

  // SAFETY: The caller grants write access and the cleared picture owns no references.
  unsafe { slot.as_ptr().write(cleared) };

  drop(owned);
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_data_create"))]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_data_create")
)]
#[doc = "\n# Safety\nA non-null `output` must point to writable packet storage."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller supplies the writable packet storage"
  )
)]
pub unsafe extern "C" fn dav1d_data_create(
  output: Option<NonNull<Dav1dData>>,
  size: NativeSize,
) -> *mut u8 {
  let allocated = (|| -> Rav1dResult<*mut u8> {
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    validate_input!((size <= MAX_C_BUFFER_BYTES, EINVAL))?;
    let Ok(length) = console_core_number_conversion::fixed_index(size);
    let packet = Rav1dData::create(length)?;
    let c_packet = Dav1dData::from(packet);
    let buffer = match c_packet.data {
      Some(data) => data.as_ptr(),
      None => ptr::null_mut(),
    };

    // SAFETY: The caller supplies writable packet storage, which may be uninitialized.
    unsafe { output_slot.as_ptr().write(c_packet) };

    Ok(buffer)
  })();

  match allocated {
    Ok(buffer) => buffer,
    Err(_reported_by_null_buffer) => ptr::null_mut(),
  }
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_data_wrap"))]
#[cfg_attr(not(feature = "dav1d-compat"), unsafe(export_name = "rav1d_data_wrap"))]
#[doc = "\n# Safety\nNon-null `output` must be writable packet storage. A non-null `input` must refer to an initialized buffer of `size` bytes that remains valid until `free_callback` releases it. The callback and `user_data` must be valid to use from decoder worker threads."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller transfers the original allocation with its matching release callback"
  )
)]
pub unsafe extern "C" fn dav1d_data_wrap(
  output: Option<NonNull<Dav1dData>>,
  input: Option<NonNull<u8>>,
  size: NativeSize,
  free_callback: Option<FnFree>,
  user_data: Option<Dav1dOpaquePointer>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let output_slot = validate_input!(output.ok_or(EINVAL))?;
    let input_buffer = validate_input!(input.ok_or(EINVAL))?;
    validate_input!((size <= MAX_C_BUFFER_BYTES, EINVAL))?;
    let bytes = NonNull::slice_from_raw_parts(input_buffer, size);
    // SAFETY: The caller transfers this initialized buffer with the callback that releases it.
    let packet = unsafe { Rav1dData::wrap(bytes, free_callback, user_data) }?;

    // SAFETY: The caller supplies writable packet storage.
    unsafe { output_slot.as_ptr().write(Dav1dData::from(packet)) };

    Ok(())
  })())
}

#[cfg_attr(
  feature = "dav1d-compat",
  unsafe(export_name = "dav1d_data_wrap_user_data")
)]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_data_wrap_user_data")
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the C caller supplies a valid initialized buffer; it is replaced with the adjusted handle before returning"
  )
)]
#[doc = "\n# Safety\nA non-null `packet` must be an initialized, exclusively accessible C packet. Non-null `user_data` must remain valid until its callback releases it. The callback and `cookie` may be used from decoder worker threads."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller transfers the user-data pointer with its matching callback and lends its packet slot"
  )
)]
pub unsafe extern "C" fn dav1d_data_wrap_user_data(
  packet: Option<NonNull<Dav1dData>>,
  user_data: Option<NonNull<u8>>,
  free_callback: Option<FnFree>,
  cookie: Option<Dav1dOpaquePointer>,
) -> Dav1dResult {
  Dav1dResult::from((|| {
    let packet_slot = validate_input!(packet.ok_or(EINVAL))?;
    let metadata = validate_input!(user_data.ok_or(EINVAL))?;
    // SAFETY: The caller grants exclusive access to the initialized C packet.
    let c_packet = unsafe { packet_slot.as_ptr().read() };
    let mut owned = Rav1dData::from(c_packet);
    // SAFETY: The caller transfers the initialized metadata and its releasing callback.
    let outcome = unsafe { owned.wrap_user_data(metadata, free_callback, cookie) };

    // SAFETY: Return the packet's ownership to the same writable slot on any outcome.
    unsafe { packet_slot.as_ptr().write(Dav1dData::from(owned)) };

    outcome
  })())
}

#[cfg_attr(feature = "dav1d-compat", unsafe(export_name = "dav1d_data_unref"))]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_data_unref")
)]
#[doc = "\n# Safety\nA non-null `packet` must be initialized and exclusively accessible. The record transfers its live owned references to this call."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller grants exclusive access to the initialized packet and transfers its references"
  )
)]
pub unsafe extern "C" fn dav1d_data_unref(packet: Option<NonNull<Dav1dData>>) {
  let mut slot = match validate_input!(packet.ok_or(())) {
    Ok(packet_slot) => packet_slot,
    Err(_null_packet_slot) => return,
  };
  // SAFETY: The caller grants exclusive access to the initialized packet.
  let owned = mem::take(unsafe { slot.as_mut() });
  drop(Rav1dData::from(owned));
}

#[cfg_attr(
  feature = "dav1d-compat",
  unsafe(export_name = "dav1d_data_props_unref")
)]
#[cfg_attr(
  not(feature = "dav1d-compat"),
  unsafe(export_name = "rav1d_data_props_unref")
)]
#[doc = "\n# Safety\nNon-null `properties` must be initialized and exclusively accessible, and the record transfers any live user-data reference to this call."]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the C caller grants exclusive access to the initialized data properties and transfers any user reference"
  )
)]
pub unsafe extern "C" fn dav1d_data_props_unref(properties: Option<NonNull<Dav1dDataProps>>) {
  let mut slot = match validate_input!(properties.ok_or(())) {
    Ok(properties_slot) => properties_slot,
    Err(_null_properties_slot) => return,
  };
  // SAFETY: The caller grants exclusive access to the initialized properties.
  let owned = mem::take(unsafe { slot.as_mut() });
  drop(Rav1dDataProps::from(owned));
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::include::dav1d::picture::Rav1dPicAllocator;
  use crate::src::c_arc::CArc;
  use crate::src::c_box::CBox;
  use crate::src::error::Rav1dError::ENOMEM;
  use crate::src::send_sync_non_null::SendSyncNonNull;
  use std::sync::Arc;
  use std::sync::atomic::{AtomicU32, Ordering};

  static USER_DATA: u8 = 23;

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this release callback only receives a live AtomicU32 from the local picture fixture"
    )
  )]
  unsafe extern "C" fn count_release(_data: *const u8, cookie: Option<Dav1dOpaquePointer>) {
    match cookie {
      Some(opaque) => {
        let Ok(counter) = opaque.cast::<AtomicU32>();
        // SAFETY: The fixture keeps this AtomicU32 alive through the final picture release.
        let Ok(released) = unsafe { counter.as_ref() };
        let _previous_count = released.fetch_add(1, Ordering::Relaxed);
      }
      None => {}
    }
  }

  extern "C" fn reject_picture(
    _picture: *mut Dav1dPicture,
    _cookie: Option<Dav1dOpaquePointer>,
  ) -> Dav1dResult {
    let rejected: Rav1dResult = Err(ENOMEM);
    Dav1dResult::from(rejected)
  }

  extern "C" fn accept_picture_without_planes(
    _picture: *mut Dav1dPicture,
    _cookie: Option<Dav1dOpaquePointer>,
  ) -> Dav1dResult {
    Dav1dResult(0)
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the allocator callback receives a live release counter from the fixture"
    )
  )]
  unsafe extern "C" fn count_picture_release(
    _picture: *mut Dav1dPicture,
    cookie: Option<Dav1dOpaquePointer>,
  ) {
    // SAFETY: The application supplied the same fixture counter to allocation and release.
    unsafe { count_release(std::ptr::null(), cookie) };
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "the fixture observes the extra header reference transferred through the C allocation callback and reclaimed once"
    )
  )]
  fn picture_allocation_failure_releases_each_transferred_owner_once() -> Rav1dResult {
    let stream = include_bytes!("../tests/crash_vectors/tile_threading_cdef_lpf_race.obu");
    let parsed = rav1d_parse_sequence_header(stream)?;
    let header = Arc::new(parsed);
    let releases = AtomicU32::new(0);
    let Ok(counter) = SendSyncNonNull::from_ref(&releases);
    let Ok(cookie) = counter.cast();
    let rejects = Rav1dPicAllocator {
      cookie: Some(cookie),
      default_pool: None,
      alloc_picture_callback: reject_picture,
      release_picture_callback: count_picture_release,
    };
    let rejected = rejects.alloc_picture_data(16, 16, Arc::clone(&header), None);
    assert!(matches!(rejected, Err(ENOMEM)));
    assert_eq!(Arc::strong_count(&header), 1);
    assert_eq!(releases.load(Ordering::Relaxed), 0);

    let returns_missing_planes = Rav1dPicAllocator {
      alloc_picture_callback: accept_picture_without_planes,
      ..rejects
    };
    let incomplete = returns_missing_planes.alloc_picture_data(16, 16, Arc::clone(&header), None);
    assert!(matches!(incomplete, Err(EINVAL)));
    assert_eq!(Arc::strong_count(&header), 1);
    assert_eq!(releases.load(Ordering::Relaxed), 1);

    let invalid_height = -256;
    let default_allocator = Rav1dPicAllocator::default();
    let default_rejected =
      default_allocator.alloc_picture_data(16, invalid_height, Arc::clone(&header), None);
    assert!(matches!(default_rejected, Err(ENOMEM)));
    assert_eq!(Arc::strong_count(&header), 1);

    let c_callback = Rav1dPicAllocator {
      default_pool: None,
      ..default_allocator
    };
    let callback_rejected =
      c_callback.alloc_picture_data(16, invalid_height, Arc::clone(&header), None);
    assert!(matches!(callback_rejected, Err(ENOMEM)));
    assert_eq!(Arc::strong_count(&header), 1);
    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the fixture lends an initialized byte and its live release counter to the C picture"
    )
  )]
  fn counted_picture(picture: &Rav1dPicture, released: &AtomicU32) -> Rav1dResult<Dav1dPicture> {
    let Ok(counter) = SendSyncNonNull::from_ref(released);
    let Ok(cookie) = counter.cast();
    let mut metadata = Rav1dData::default();
    let byte = NonNull::from(&USER_DATA);
    // SAFETY: The byte is static and count_release obtains the counter lent by the test.
    unsafe { metadata.wrap_user_data(byte, Some(count_release), Some(cookie)) }?;
    let mut input = picture.clone();
    input.m.user_data = metadata.m.user_data;

    Ok(Dav1dPicture::from(input))
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the test owns initialized C pictures and retains each live callback counter until the corresponding release"
    )
  )]
  fn malformed_picture_parameters_preserve_borrows_and_release_consumed_owners() -> Rav1dResult {
    use crate::include::dav1d::picture::Dav1dPictureParameters;

    for parameters in [
      Dav1dPictureParameters {
        layout: u32::MAX,
        ..Dav1dPictureParameters::default()
      },
      Dav1dPictureParameters {
        bpc: -1,
        ..Dav1dPictureParameters::default()
      },
    ] {
      let release_count = AtomicU32::new(0);
      let mut picture = counted_picture(&Rav1dPicture::default(), &release_count)?;
      picture.p = parameters;
      // SAFETY: The test retains the live picture and callback count throughout the borrow.
      let borrowed = unsafe { Rav1dPicture::from_borrowed(&picture) };
      assert!(matches!(borrowed, Err(EINVAL)));
      assert_eq!(release_count.load(Ordering::Relaxed), 0);

      let slot = NonNull::from(&mut picture);

      // SAFETY: The fixture transfers its initialized picture with exclusive access to the slot.
      unsafe { dav1d_picture_unref(Some(slot)) };

      assert_eq!(release_count.load(Ordering::Relaxed), 1);
      assert_eq!(picture.m.user_data.data, None);
    }

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the C ABI fixture provides live initialized pictures, writable output slots, and a local decoder handle"
    )
  )]
  fn apply_grain_keeps_input_and_output_references_independent() -> Rav1dResult {
    let settings = Rav1dSettings {
      n_threads: 1,
      max_frame_delay: 1,
      ..Rav1dSettings::default()
    };
    let (decoder, workers) = rav1d_open(&settings, &HostWorkers)?;
    assert!(workers.is_empty());
    let stream = include_bytes!("../tests/crash_vectors/tile_threading_cdef_lpf_race.obu");
    let Ok(storage) = CBox::from_box(Box::<[u8]>::from(stream.as_slice()));
    let packet = CArc::wrap(storage)?;
    let mut input = Rav1dData::from(packet);
    let mut picture = Rav1dPicture::default();
    rav1d_send_data(&decoder, &mut input)?;
    rav1d_get_picture(&decoder, &mut picture)?;
    let Ok(context) = RawArc::from_arc(decoder);

    let released = AtomicU32::new(0);
    let mut original = counted_picture(&picture, &released)?;
    let mut destination = std::mem::MaybeUninit::<Dav1dPicture>::uninit();
    let original_fields = (original.frame_hdr, original.data, original.m.user_data.data);
    let input_slot = NonNull::from(&mut original);
    let output_slot = NonNull::from(&mut destination).cast();
    // SAFETY: The initialized input and writable output are independent, and context remains live.
    let status = unsafe { dav1d_apply_grain(Some(context), Some(output_slot), Some(input_slot)) };
    assert_eq!(status, Dav1dResult(0));
    #[cfg_attr(
      dylint_lib = "explicit096_no_unchecked_memory",
      expect(
        explicit096_no_unchecked_memory,
        reason = "the successful C result established initialization of the output slot"
      )
    )]
    // SAFETY: Success initializes the output slot.
    let output = unsafe { destination.assume_init_ref() };
    assert_eq!(
      (output.frame_hdr, output.data, output.m.user_data.data),
      original_fields
    );
    assert_eq!(
      (original.frame_hdr, original.data, original.m.user_data.data),
      original_fields
    );
    assert_eq!(released.load(Ordering::Relaxed), 0);

    // SAFETY: Each slot owns its own reference and is released exactly once.
    unsafe { dav1d_picture_unref(Some(output_slot)) };

    assert_eq!(released.load(Ordering::Relaxed), 0);

    // SAFETY: The distinct source record keeps its own live reference until this release.
    unsafe { dav1d_picture_unref(Some(NonNull::from(&mut original))) };

    assert_eq!(released.load(Ordering::Relaxed), 1);
    assert!(original.r#ref.is_none());
    assert!(original.itut_t35_ref.is_none());

    let mut in_place = counted_picture(&picture, &released)?;
    let replaced_slot = NonNull::from(&mut in_place);
    // SAFETY: The output replaces the same initialized input slot.
    let in_place_status =
      unsafe { dav1d_apply_grain(Some(context), Some(replaced_slot), Some(replaced_slot)) };
    assert_eq!(in_place_status, Dav1dResult(0));
    assert_eq!(in_place.data, original_fields.1);
    assert_eq!(released.load(Ordering::Relaxed), 1);

    // SAFETY: This is the only reference in the replaced slot.
    unsafe { dav1d_picture_unref(Some(NonNull::from(&mut in_place))) };

    assert_eq!(released.load(Ordering::Relaxed), 2);

    let mut missing_header = Dav1dPicture::default();
    missing_header.m.timestamp = 42;
    let invalid_slot = NonNull::from(&mut missing_header);
    // SAFETY: Both pointers name the same exclusively accessible initialized record.
    let invalid =
      unsafe { dav1d_apply_grain(Some(context), Some(invalid_slot), Some(invalid_slot)) };
    let expected: Rav1dResult = Err(EINVAL);
    assert_eq!(invalid, Dav1dResult::from(expected));
    assert_eq!(missing_header.m.timestamp, 42);

    let mut context_slot = Some(context);

    // SAFETY: The local slot holds the decoder's sole C reference.
    unsafe { dav1d_close(Some(NonNull::from(&mut context_slot))) };

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the local C packet fixture lends a static byte and keeps its release counter alive"
    )
  )]
  fn rejected_user_data_keeps_the_c_packet_owned() -> Rav1dResult {
    let released = AtomicU32::new(0);
    let Ok(counter) = SendSyncNonNull::from_ref(&released);
    let Ok(cookie) = counter.cast();
    let bytes = NonNull::from(std::slice::from_ref(&USER_DATA));
    // SAFETY: The byte is static and the local counter stays live until the packet is released.
    let wrapped = unsafe { Rav1dData::wrap(bytes, Some(count_release), Some(cookie)) }?;
    let mut packet = Dav1dData::from(wrapped);
    let original_buffer = packet.data;
    let metadata = NonNull::from(&USER_DATA);
    let packet_slot = NonNull::from(&mut packet);
    // SAFETY: The packet is initialized and exclusive, and the static metadata is readable.
    let status =
      unsafe { dav1d_data_wrap_user_data(Some(packet_slot), Some(metadata), None, None) };
    let rejected: Rav1dResult = Err(EINVAL);
    assert_eq!(status, Dav1dResult::from(rejected));
    assert_eq!(packet.data, original_buffer);
    assert_eq!(packet.sz, 1);
    assert_eq!(released.load(Ordering::Relaxed), 0);

    // SAFETY: The error path preserved this packet's reference for the caller to release.
    unsafe { dav1d_data_unref(Some(NonNull::from(&mut packet))) };

    assert_eq!(released.load(Ordering::Relaxed), 1);

    Ok(())
  }
}
