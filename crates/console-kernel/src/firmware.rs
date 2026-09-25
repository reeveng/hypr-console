//! What UEFI hands a program it starts, what is taken from it, and the leaving.
//!
//! The firmware is the only thing on the machine before the kernel is, and
//! for the first few steps the kernel lives inside it: the firmware's pool is
//! where memory comes from until the kernel has a map of its own to allocate
//! from. The map is that handover. It is read into a buffer taken from the
//! pool beforehand, because allocating after the map is read changes the map
//! and makes the key it came with stale, and then boot services are exited
//! with the key. A key that went stale anyway -- the firmware's own timers
//! still run until the exit -- is answered by reading the map again into the
//! same buffer, which the specification allows and nothing else is.
//!
//! The console the firmware drew is not borrowed at all any more: it is a
//! protocol in boot services and goes with them, and `serial` says the lines
//! from the first one.
//!
//! The tables are the specification's, in its order, and only as far as the
//! last field read. A field this file never reads is still declared, because
//! the layout is the position and a missing one moves every field after it.

use core::alloc::Layout;
use core::ffi::c_void;
use core::fmt;
use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

use alloc::vec;
use console_core_never::Never;
use console_kernel::memory::{MemoryError, MemoryMap};

pub type Status = u64;

pub const SUCCESS: Status = 0;

const LOADER_DATA: u32 = 2;

const POOL_ALIGNMENT: u64 = 8;

const SPARE_DESCRIPTORS: u64 = 16;

const ATTEMPTS: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareError {
    MemoryMap(Status),
    StaleKey,
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
}

type GetMemoryMap = extern "efiapi" fn(*mut u64, *mut u8, *mut u64, *mut u64, *mut u32) -> Status;

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
    handle_protocol: *const c_void,
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
}

struct Reading {
    size: u64,
    key: u64,
    descriptor_size: u64,
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the global allocator is the language's, one per program and called by `alloc` with a layout and nothing else, so the table it allocates from can only be where the whole program can reach it; it is filled once, before the first allocation, from the table the firmware handed `efi_main`, and emptied when boot services are exited"
    )
)]
static BOOT_SERVICES: AtomicPtr<BootServices> = AtomicPtr::new(ptr::null_mut());

pub fn borrow(system: *const SystemTable) -> Result<(), Never> {
    // SAFETY: the firmware hands `efi_main` a system table that stays valid until boot services are exited, and nothing reads it after that.
    let table = unsafe { &*system };

    BOOT_SERVICES.store(table.boot_services, Ordering::Release);

    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    allow(explicit051_no_machine_width, reason = "the buffer the firmware writes the map into is a slice, measured in `usize`, and `console-core-number-conversion`, where the width is otherwise met, links `std` and so cannot be linked into a kernel")
)]
pub fn exit_boot_services(image: *const c_void) -> Result<MemoryMap<'static>, FirmwareError> {
    let services = BOOT_SERVICES.load(Ordering::Acquire);
    let capacity = measure(services)?;
    let length = usize::try_from(capacity).map_err(|_too_wide| FirmwareError::Memory(MemoryError::Overflow))?;
    let buffer = vec![0_u8; length].leak();

    for _attempt in 0..ATTEMPTS {
        let reading = read(services, buffer.as_mut_ptr(), capacity)?;

        // SAFETY: `services` is the boot services table, the key is the one the map just read came with, and nothing has allocated since.
        let status = unsafe { ((*services).exit_boot_services)(image, reading.key) };

        match status {
            SUCCESS => {
                BOOT_SERVICES.store(ptr::null_mut(), Ordering::Release);

                let read_length = usize::try_from(reading.size).map_err(|_too_wide| FirmwareError::Memory(MemoryError::Overflow))?;
                let handed_over: &'static [u8] = buffer;
                let bytes = handed_over.get(..read_length).ok_or(FirmwareError::Memory(MemoryError::Overflow))?;

                return MemoryMap::new(bytes, reading.descriptor_size).map_err(FirmwareError::Memory);
            },
            _stale => {},
        }
    }

    Err(FirmwareError::StaleKey)
}

fn measure(services: *mut BootServices) -> Result<u64, FirmwareError> {
    let mut reading = Reading { size: 0, key: 0, descriptor_size: 0 };
    let mut version = 0_u32;

    // SAFETY: an empty buffer is how the specification asks the map's size; the call writes the four numbers and nothing else.
    let _too_small = unsafe { ((*services).get_memory_map)(&mut reading.size, ptr::null_mut(), &mut reading.key, &mut reading.descriptor_size, &mut version) };

    let spare = reading.descriptor_size.checked_mul(SPARE_DESCRIPTORS).ok_or(FirmwareError::Memory(MemoryError::Overflow))?;

    reading.size.checked_add(spare).ok_or(FirmwareError::Memory(MemoryError::Overflow))
}

fn read(services: *mut BootServices, buffer: *mut u8, capacity: u64) -> Result<Reading, FirmwareError> {
    let mut reading = Reading { size: capacity, key: 0, descriptor_size: 0 };
    let mut version = 0_u32;

    // SAFETY: `buffer` holds `capacity` bytes, leaked from the pool so that nothing frees it, and the firmware writes no more than the size it is handed.
    let status = unsafe { ((*services).get_memory_map)(&mut reading.size, buffer, &mut reading.key, &mut reading.descriptor_size, &mut version) };

    match status {
        SUCCESS => Ok(reading),
        failure => Err(FirmwareError::MemoryMap(failure)),
    }
}

#[must_use]
pub fn pool_allocate(layout: Layout) -> Result<*mut u8, Never> {
    let services = BOOT_SERVICES.load(Ordering::Acquire);

    let alignment = u64::try_from(layout.align());
    let size = u64::try_from(layout.size());

    Ok(match (services.is_null(), alignment, size) {
        (false, Ok(alignment), Ok(size)) => match alignment <= POOL_ALIGNMENT {
            true => {
                let mut at = ptr::null_mut();
                // SAFETY: `services` is the boot services table the firmware handed over, stored before the first allocation and emptied when they are exited.
                let status: Status = unsafe { ((*services).allocate_pool)(LOADER_DATA, size, &mut at) };

                match status {
                    SUCCESS => at,
                    _ => ptr::null_mut(),
                }
            },
            false => ptr::null_mut(),
        },
        (true, _, _) => ptr::null_mut(),
        (false, Err(_too_large), _) | (false, _, Err(_too_large)) => ptr::null_mut(),
    })
}

pub fn pool_free(at: *mut u8) -> Result<(), Never> {
    let services = BOOT_SERVICES.load(Ordering::Acquire);

    match services.is_null() {
        true => {},
        false => {
            // SAFETY: `at` came from `allocate_pool` on this same table, which is still there because it has not been emptied.
            let _status = unsafe { ((*services).free_pool)(at) };
        },
    }

    Ok(())
}

impl fmt::Display for FirmwareError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FirmwareError::MemoryMap(status) => write!(to, "the firmware would not give its memory map (status {status:#x})"),
            FirmwareError::StaleKey => write!(to, "the memory map changed under every one of {ATTEMPTS} attempts to leave"),
            FirmwareError::Memory(error) => write!(to, "{error}"),
        }
    }
}
