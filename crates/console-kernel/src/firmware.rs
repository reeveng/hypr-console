//! What UEFI hands a program it starts, what is taken from it, and the leaving.
//!
//! The firmware is the only thing on the machine before the kernel is, and
//! for the first few steps the kernel lives inside it: the firmware's pool is
//! where memory comes from until the kernel has a map of its own to allocate
//! from. The map is that handover. It is read into a buffer taken from the
//! pool beforehand, because allocating after the map is read changes the map
//! and makes the key it came with stale, and then boot services are exited
//! with the key. Firmware events can still change the map. A stale key is
//! retried with a fresh map, and a growing map gets a larger buffer. After the
//! first exit attempt, only memory allocation services may be called.
//!
//! The console the firmware drew is not borrowed at all any more: it is a
//! protocol in boot services and goes with them, and `serial` says the lines
//! from the first one.
//!
//! What the firmware drew on is the exception. The graphics output protocol
//! goes with boot services like the rest, but the frame buffer it names is
//! the panel's own memory and stays one, so `graphics` asks where it is and
//! how it is laid out while there is still somebody to ask. Only a pixel of
//! four bytes with its colours in one of the two plain orders is taken: the
//! third format describes a pixel by masks and the fourth has no frame buffer
//! at all, and a kernel with no screen says so on the serial port.
//!
//! The tables are the specification's, in its order, and only as far as the
//! last field read. A field this file never reads is still declared, because
//! the layout is the position and a missing one moves every field after it.

use core::alloc::Layout;
use core::ffi::c_void;
use core::fmt;
use core::mem::size_of;
use core::ptr;
use core::slice;
use core::sync::atomic::{AtomicPtr, Ordering};

use alloc::vec::Vec;
use console_core_never::Never;
use console_kernel::memory::{MemoryError, MemoryMap, PAGE};
use console_kernel::screen::{Panel, Pixels};

use crate::acpi;

pub type Status = u64;

pub const SUCCESS: Status = 0;

const ERROR: Status = 1 << 63;
const INVALID_PARAMETER: Status = ERROR | 2;
const BUFFER_TOO_SMALL: Status = ERROR | 5;
const LOADER_DATA: u32 = 2;

const POOL_ALIGNMENT: u64 = 8;

const ATTEMPTS: u32 = 8;

const GRAPHICS_OUTPUT: Guid = Guid {
  first: 0x9042_a9de,
  second: 0x23dc,
  third: 0x4a38,
  rest: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

const ACPI_2: Guid = Guid {
  first: 0x8868_e871,
  second: 0xe4f1,
  third: 0x11d3,
  rest: [0xbc, 0x22, 0, 0x80, 0xc7, 0x3c, 0x88, 0x81],
};

const BY_PROTOCOL: u32 = 2;
const RED_GREEN_BLUE: u32 = 0;

const BLUE_GREEN_RED: u32 = 1;

const BYTES_IN_A_PIXEL: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareError {
  MemoryMap(Status),
  MapBufferAllocation,
  ExitBootServices(Status),
  UnstableMap,
  Memory(MemoryError),
}

#[repr(C)]
struct TableHeader {
  signature: u64,
  revision: u32,
  header_size: u32,
  crc32: u32,
  reserved: u32,
}

#[repr(C)]
pub struct SystemTable {
  header: TableHeader,
  firmware_vendor: *const u16,
  firmware_revision: u32,
  console_in_handle: *const c_void,
  console_in: *const c_void,
  console_out_handle: *const c_void,
  console_out: *const c_void,
  standard_error_handle: *const c_void,
  standard_error: *const c_void,
  runtime_services: *const c_void,
  boot_services: *mut BootServices,
  configuration_count: u64,
  configuration_table: *const ConfigurationTable,
}

#[repr(C)]
struct ConfigurationTable {
  id: Guid,
  address: *const c_void,
}

const _: () = {
  assert!(size_of::<SystemTable>() == 120);
  assert!(core::mem::align_of::<SystemTable>() == 8);
  assert!(core::mem::offset_of!(SystemTable, header) == 0);
  assert!(core::mem::offset_of!(SystemTable, firmware_vendor) == 24);
  assert!(core::mem::offset_of!(SystemTable, firmware_revision) == 32);
  assert!(core::mem::offset_of!(SystemTable, console_in_handle) == 40);
  assert!(core::mem::offset_of!(SystemTable, console_in) == 48);
  assert!(core::mem::offset_of!(SystemTable, console_out_handle) == 56);
  assert!(core::mem::offset_of!(SystemTable, console_out) == 64);
  assert!(core::mem::offset_of!(SystemTable, standard_error_handle) == 72);
  assert!(core::mem::offset_of!(SystemTable, standard_error) == 80);
  assert!(core::mem::offset_of!(SystemTable, runtime_services) == 88);
  assert!(core::mem::offset_of!(SystemTable, boot_services) == 96);
  assert!(core::mem::offset_of!(SystemTable, configuration_count) == 104);
  assert!(core::mem::offset_of!(SystemTable, configuration_table) == 112);
};

type GetMemoryMap = extern "efiapi" fn(*mut u64, *mut u8, *mut u64, *mut u64, *mut u32) -> Status;

#[cfg_attr(
  dylint_lib = "explicit072_no_dynamic_dispatch",
  allow(
    explicit072_no_dynamic_dispatch,
    reason = "these are the firmware's own function-pointer fields, laid out by the UEFI ABI; the call is the firmware's table and not a behaviour this tree chooses"
  )
)]
#[repr(C)]
struct BootServices {
  header: TableHeader,
  raise_tpl: *const c_void,
  restore_tpl: *const c_void,
  allocate_pages: *const c_void,
  free_pages: *const c_void,
  get_memory_map: GetMemoryMap,
  allocate_pool: extern "efiapi" fn(u32, u64, *mut *mut u8) -> Status,
  free_pool: extern "efiapi" fn(*mut u8) -> Status,
  create_event: *const c_void,
  set_timer: *const c_void,
  wait_for_event: *const c_void,
  signal_event: *const c_void,
  close_event: *const c_void,
  check_event: *const c_void,
  install_protocol_interface: *const c_void,
  reinstall_protocol_interface: *const c_void,
  uninstall_protocol_interface: *const c_void,
  handle_protocol:
    extern "efiapi" fn(*const c_void, *const Guid, *mut *const GraphicsOutput) -> Status,
  reserved: *const c_void,
  register_protocol_notify: *const c_void,
  locate_handle: *const c_void,
  locate_device_path: *const c_void,
  install_configuration_table: *const c_void,
  load_image: *const c_void,
  start_image: *const c_void,
  exit: *const c_void,
  unload_image: *const c_void,
  exit_boot_services: extern "efiapi" fn(*const c_void, u64) -> Status,
  get_next_monotonic_count: *const c_void,
  stall: *const c_void,
  set_watchdog_timer: *const c_void,
  connect_controller: *const c_void,
  disconnect_controller: *const c_void,
  open_protocol: *const c_void,
  close_protocol: *const c_void,
  open_protocol_information: *const c_void,
  protocols_per_handle: *const c_void,
  locate_handle_buffer: extern "efiapi" fn(
    u32,
    *const Guid,
    *const c_void,
    *mut u64,
    *mut *const *const c_void,
  ) -> Status,
}

#[derive(PartialEq, Eq)]
#[repr(C)]
struct Guid {
  first: u32,
  second: u16,
  third: u16,
  rest: [u8; 8],
}

const _: () = {
  assert!(size_of::<Guid>() == 16);
  assert!(core::mem::align_of::<Guid>() == 4);
  assert!(core::mem::offset_of!(Guid, first) == 0);
  assert!(core::mem::offset_of!(Guid, second) == 4);
  assert!(core::mem::offset_of!(Guid, third) == 6);
  assert!(core::mem::offset_of!(Guid, rest) == 8);
};
const _: [(); 24] = [(); size_of::<ConfigurationTable>()];
const _: [(); 16] = [(); core::mem::offset_of!(ConfigurationTable, address)];

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "UEFI's validated configuration table count becomes a Rust slice length"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "the UEFI configuration table describes this many initialized entries before handoff"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "this is the UEFI ABI boundary that borrows the firmware's live configuration entries"
  )
)]
pub(super) fn acpi_root(system: *const SystemTable) -> Result<Option<acpi::Root>, Never> {
  // SAFETY: efi_main received the live x86-64 UEFI system table.
  let table = unsafe { &*system };

  match (
    table.configuration_table.is_null(),
    table.configuration_count,
  ) {
    (false, 1..=1024) => {}
    (_, _) => return Ok(None),
  }

  let count = match usize::try_from(table.configuration_count) {
    Ok(count) => count,
    Err(_too_wide) => return Ok(None),
  };

  // SAFETY: UEFI owns configuration_count live ConfigurationTable entries during this call.
  let entries = unsafe { slice::from_raw_parts(table.configuration_table, count) };

  for entry in entries {
    match ::core::primitive::bool::then_some(entry.id == ACPI_2, ()) {
      ::core::option::Option::Some(()) => {
        let address = match u64::try_from(entry.address.expose_provenance()) {
          Ok(address) => address,
          Err(_too_wide) => return Ok(None),
        };

        return Ok(Some(acpi::Root(address)));
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(None)
}

#[repr(C)]
struct GraphicsOutput {
  query_mode: *const c_void,
  set_mode: *const c_void,
  block_transfer: *const c_void,
  mode: *const GraphicsMode,
}

const _: () = {
  assert!(size_of::<GraphicsOutput>() == 32);
  assert!(core::mem::align_of::<GraphicsOutput>() == 8);
  assert!(core::mem::offset_of!(GraphicsOutput, query_mode) == 0);
  assert!(core::mem::offset_of!(GraphicsOutput, set_mode) == 8);
  assert!(core::mem::offset_of!(GraphicsOutput, block_transfer) == 16);
  assert!(core::mem::offset_of!(GraphicsOutput, mode) == 24);
};

#[repr(C)]
struct GraphicsMode {
  max_mode: u32,
  mode: u32,
  information: *const ModeInformation,
  information_size: u64,
  frame_buffer_base: u64,
  frame_buffer_size: u64,
}

#[repr(C)]
struct ModeInformation {
  version: u32,
  horizontal_resolution: u32,
  vertical_resolution: u32,
  pixel_format: u32,
  pixel_information: [u32; 4],
  pixels_per_scan_line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Graphics {
  pub base: u64,
  pub pixels: u64,
  pub panel: Panel,
}

struct Reading {
  size: u64,
  key: u64,
  descriptor_size: u64,
}

enum MapReply {
  Ready(Reading),
  NeedsBuffer(u64),
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "the global allocator is the language's, one per program and called by `alloc` with a layout and nothing else, so the table it allocates from can only be where the whole program can reach it; it is filled once, before the first allocation, from the table the firmware handed `efi_main`, and emptied when boot services are exited"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the Rust global allocator has no firmware context parameter, and this table pointer is cleared exactly once at handover"
  )
)]
static BOOT_SERVICES: AtomicPtr<BootServices> = AtomicPtr::new(ptr::null_mut());

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the efiapi entry supplies the live UEFI system table for the global allocation adapter"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "this Release publishes the live table to the allocator and map readers that use Acquire"
  )
)]
pub fn borrow(system: *const SystemTable) -> Result<(), Never> {
  // SAFETY: the firmware hands `efi_main` a system table that stays valid until boot services are exited, and nothing reads it after that.
  let table = unsafe { &*system };

  BOOT_SERVICES.store(table.boot_services, Ordering::Release);

  Ok(())
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the GOP adapter borrows the firmware system table and its firmware-owned handle buffer"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "a firmware handle count becomes a Rust slice length at this ABI boundary"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit096_no_unchecked_memory",
  expect(
    explicit096_no_unchecked_memory,
    reason = "successful LocateHandleBuffer supplies count initialized handles in one live pool buffer"
  )
)]
pub fn graphics(system: *const SystemTable) -> Result<Option<Graphics>, Never> {
  // SAFETY: efi_main passes the live UEFI system table before boot services exit.
  let table = unsafe { &*system };
  // SAFETY: the live system table references the firmware boot-services table.
  let services = unsafe { &*table.boot_services };
  let Ok(console) = graphics_on(services, table.console_out_handle);

  match console {
    Some(graphics) => return Ok(Some(graphics)),
    None => {}
  }

  let mut count = 0_u64;
  let mut handles = ptr::null();
  let status = (services.locate_handle_buffer)(
    BY_PROTOCOL,
    &GRAPHICS_OUTPUT,
    ptr::null(),
    &mut count,
    &mut handles,
  );

  match ::core::primitive::bool::then_some(status == SUCCESS && !handles.is_null(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(None),
  }

  let represented = usize::try_from(count);
  let found = match represented {
    Ok(length) => {
      // SAFETY: success gives one firmware allocation with length initialized handles, retained through the search.
      let candidates = unsafe { slice::from_raw_parts(handles, length) };
      let Ok(found) = scan_graphics(services, candidates, table.console_out_handle);

      found
    }
    Err(_wide) => None,
  };
  let _released = (services.free_pool)(handles.cast_mut().cast());

  Ok(found)
}

fn scan_graphics(
  services: &BootServices,
  candidates: &[*const c_void],
  console: *const c_void,
) -> Result<Option<Graphics>, Never> {
  for &handle in candidates {
    match ::core::primitive::bool::then_some(handle == console, ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let Ok(candidate) = graphics_on(services, handle);

    match candidate {
      Some(graphics) => return Ok(Some(graphics)),
      None => {}
    }
  }

  Ok(None)
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "successful HandleProtocol temporarily lends GOP, the current mode pointer and its size-checked info"
  )
)]
fn graphics_on(
  services: &BootServices,
  handle: *const c_void,
) -> Result<Option<Graphics>, Never> {
  match ::core::primitive::bool::then_some(handle.is_null(), ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  let mut output: *const GraphicsOutput = ptr::null();
  let status = (services.handle_protocol)(handle, &GRAPHICS_OUTPUT, &mut output);

  match status {
    SUCCESS => {}
    _ => return Ok(None),
  }

  // SAFETY: a successful lookup supplies the firmware's GOP and its active mode until boot services exit.
  let available = unsafe { output.as_ref().and_then(|gop| gop.mode.as_ref()) };
  let mode = match available {
    Some(mode) => mode,
    None => return Ok(None),
  };

  match u64::try_from(size_of::<ModeInformation>()) {
    Ok(required) => match ::core::primitive::bool::then_some(mode.information_size >= required, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Ok(None),
    },
    Err(_wide) => return Ok(None),
  }

  // SAFETY: GOP reports the size of this mode's information structure, checked above.
  let parameters = unsafe { mode.information.as_ref() };
  let information = match parameters {
    Some(information) => information,
    None => return Ok(None),
  };
  let stride = u64::from(information.pixels_per_scan_line);
  let height = u64::from(information.vertical_resolution);
  let width = u64::from(information.horizontal_resolution);
  let pixels = mode.frame_buffer_size.div_euclid(BYTES_IN_A_PIXEL);
  let supported = matches!(information.pixel_format, RED_GREEN_BLUE | BLUE_GREEN_RED);
  let aligned = mode.frame_buffer_base.is_multiple_of(BYTES_IN_A_PIXEL);

  match ::core::primitive::bool::then_some(supported
    && aligned
    && mode.frame_buffer_base != 0
    && width != 0
    && height != 0
    && stride >= width
    && stride.saturating_mul(height) <= pixels, ())
  {
    ::core::option::Option::Some(()) => Ok(Some(Graphics {
      base: mode.frame_buffer_base,
      pixels,
      panel: Panel {
        width: Pixels(width),
        height: Pixels(height),
        scan_line: Pixels(stride),
      },
    })),
    ::core::option::Option::None => Ok(None),
  }
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "the buffer the firmware writes the map into is a slice, measured in `usize`, and `console-core-number-conversion`, where the width is otherwise met, links `std` and so cannot be linked into a kernel"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit059_no_unchecked_api",
  allow(
    explicit059_no_unchecked_api,
    reason = "the map is the kernel's for as long as the machine is up, and the pool its buffer came from is gone once boot services are, so there is no owner left whose end could be said"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "ExitBootServices must be called through the live firmware table with its latest map key"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "this Acquire reads the boot table published by borrow and Release clears the pointer for later allocator calls"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit066_no_heap_after_init",
  expect(
    explicit066_no_heap_after_init,
    reason = "this allocates the memory-map buffer before ExitBootServices, while the firmware allocator is active"
  )
)]
pub fn exit_boot_services(image: *const c_void) -> Result<MemoryMap<'static>, FirmwareError> {
  let services = BOOT_SERVICES.load(Ordering::Acquire);
  let mut buffer = Vec::new();

  for _attempt in 0..ATTEMPTS {
    let response = read(services, &mut buffer)?;
    let reading = match response {
      MapReply::Ready(reading) => reading,
      MapReply::NeedsBuffer(needed) => {
        grow_map_buffer(&mut buffer, needed)?;
        continue;
      }
    };

    let read_length = usize::try_from(reading.size)
      .map_err(|_too_wide| FirmwareError::Memory(MemoryError::Overflow))?;
    let _reported_bytes = buffer
      .get(..read_length)
      .ok_or(FirmwareError::Memory(MemoryError::Overflow))?;

    // SAFETY: `services` is the boot services table, the key came from the last successful read, and no firmware service has run since that read.
    let status = unsafe { ((*services).exit_boot_services)(image, reading.key) };

    match status {
      SUCCESS => {
        BOOT_SERVICES.store(ptr::null_mut(), Ordering::Release);

        buffer.truncate(read_length);
        let bytes_2 = buffer.leak();

        return MemoryMap::new(bytes_2, reading.descriptor_size).map_err(FirmwareError::Memory);
      }
      INVALID_PARAMETER => {}
      failure => return Err(FirmwareError::ExitBootServices(failure)),
    }
  }

  Err(FirmwareError::UnstableMap)
}

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "the checked firmware buffer requirement becomes a Rust vector capacity"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "a firmware memory-map requirement is checked for overflow and successfully reserved before the buffer is resized"
  )
)]
fn grow_map_buffer(buffer: &mut Vec<u8>, needed: u64) -> Result<(), FirmwareError> {
  let wanted = needed
    .checked_add(PAGE)
    .ok_or(FirmwareError::Memory(MemoryError::Overflow))?;
  let capacity =
    usize::try_from(wanted).map_err(|_wide| FirmwareError::Memory(MemoryError::Overflow))?;
  let additional = capacity
    .checked_sub(buffer.len())
    .filter(|additional| *additional > 0)
    .ok_or(FirmwareError::MemoryMap(BUFFER_TOO_SMALL))?;
  buffer
    .try_reserve_exact(additional)
    .map_err(|_error| FirmwareError::MapBufferAllocation)?;
  buffer.resize(capacity, 0);

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "GetMemoryMap writes the size and key through the firmware ABI into the buffer owned by this call"
  )
)]
fn read(services: *mut BootServices, buffer: &mut [u8]) -> Result<MapReply, FirmwareError> {
  let size = u64::try_from(buffer.len())
    .map_err(|_too_wide| FirmwareError::Memory(MemoryError::Overflow))?;
  let mut reading = Reading {
    size,
    key: 0,
    descriptor_size: 0,
  };
  let mut version = 0_u32;
  let output = match ::core::primitive::bool::then_some(buffer.is_empty(), ()) {
    ::core::option::Option::Some(()) => ptr::null_mut(),
    ::core::option::Option::None => buffer.as_mut_ptr(),
  };

  // SAFETY: the output has `size` writable bytes. A null pointer with size zero asks only for the required size.
  let status = unsafe {
    ((*services).get_memory_map)(
      &mut reading.size,
      output,
      &mut reading.key,
      &mut reading.descriptor_size,
      &mut version,
    )
  };

  match status {
    SUCCESS => Ok(MapReply::Ready(reading)),
    BUFFER_TOO_SMALL => Ok(MapReply::NeedsBuffer(reading.size)),
    failure => Err(FirmwareError::MemoryMap(failure)),
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the firmware global-allocation adapter calls AllocatePool through the table borrowed before handoff"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "Acquire observes the live table published by boot or the null pointer published at handoff"
  )
)]
pub fn pool_allocate(layout: Layout) -> Result<*mut u8, Never> {
  let services = BOOT_SERVICES.load(Ordering::Acquire);

  let alignment = u64::try_from(layout.align());
  let size = u64::try_from(layout.size());

  Ok(match (services.is_null(), alignment, size) {
    (false, Ok(alignment_2), Ok(size_2)) => match ::core::primitive::bool::then_some(alignment_2 <= POOL_ALIGNMENT, ()) {
      ::core::option::Option::Some(()) => {
        let mut at = ptr::null_mut();
        // SAFETY: `services` is the boot services table the firmware handed over, stored before the first allocation and emptied when they are exited.
        let status: Status = unsafe { ((*services).allocate_pool)(LOADER_DATA, size_2, &mut at) };

        match status {
          SUCCESS => at,
          _ => ptr::null_mut(),
        }
      }
      ::core::option::Option::None => ptr::null_mut(),
    },
    (true, _, _) => ptr::null_mut(),
    (false, Err(_too_large), _) | (false, _, Err(_too_large)) => ptr::null_mut(),
  })
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "only this firmware allocator adapter returns live firmware allocations to FreePool"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit069_lock_discipline",
  expect(
    explicit069_lock_discipline,
    reason = "Acquire observes the table publication or its removal once firmware services have exited"
  )
)]
pub fn pool_free(at: *mut u8) -> Result<(), Never> {
  let services = BOOT_SERVICES.load(Ordering::Acquire);

  match ::core::primitive::bool::then_some(services.is_null(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      // SAFETY: `at` came from `allocate_pool` on this same table, which is still there because it has not been emptied.
      let _status = unsafe { ((*services).free_pool)(at) };
    }
  }

  Ok(())
}

impl fmt::Display for FirmwareError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      FirmwareError::MemoryMap(status) => write!(
        to,
        "the firmware would not give its memory map (status {status:#x})"
      ),
      FirmwareError::MapBufferAllocation => {
        to.write_str("could not allocate the memory map buffer")
      }
      FirmwareError::ExitBootServices(status) => {
        write!(to, "ExitBootServices returned status {status:#x}")
      }
      FirmwareError::UnstableMap => {
        write!(to, "the memory map kept changing during {ATTEMPTS} reads")
      }
      FirmwareError::Memory(error) => write!(to, "{error}"),
    }
  }
}
