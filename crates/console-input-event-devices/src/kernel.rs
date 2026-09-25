//! The calls, the request numbers and the structs, as the kernel spells them.
//!
//! Declared against the C library std already links, so nothing here is
//! anybody else's crate. A request number is written out rather than put
//! together from `_IOC`'s pieces: it is a fact about this architecture's ABI,
//! each was checked against what the kernel's own headers expand to, and a
//! number that is read is one nobody has to trust a shift for.

use std::ffi::{c_char, c_int, c_short, c_ulong};
use std::io;

use crate::codes::BusType;

pub const IN: c_short = 0x0001;

pub const CLOSE_ON_EXEC: c_int = 0o2_000_000;

pub const NOT_BLOCKING: c_int = 0o4_000;

pub const GET_FLAGS: c_int = 3;

pub const SET_FLAGS: c_int = 4;

pub const CREATED: u32 = 0x0000_0100;

pub const CHANGED: u32 = 0x0000_0004;

pub const TEXT: u32 = 256;

pub const BITS: u32 = 96;

pub const GET_ID: c_ulong = 0x8008_4502;

pub const GET_NAME: c_ulong = 0x8100_4506;

pub const GET_PHYSICAL_PATH: c_ulong = 0x8100_4507;

pub const GET_PROPERTIES: c_ulong = 0x8060_4509;

pub const GET_KEYS: c_ulong = 0x8060_4521;

pub const GET_RELATIVE_AXES: c_ulong = 0x8060_4522;

pub const GET_ABSOLUTE_AXES: c_ulong = 0x8060_4523;

pub const GET_MISC: c_ulong = 0x8060_4524;

pub const GET_FORCE_FEEDBACK: c_ulong = 0x8060_4535;

pub const GET_ABSOLUTE: c_ulong = 0x8018_4540;

pub const GRAB: c_ulong = 0x4004_4590;

pub const CREATE: c_ulong = 0x5501;

pub const SETUP: c_ulong = 0x405c_5503;

pub const ABSOLUTE_SETUP: c_ulong = 0x401c_5504;

pub const SET_EVENT_TYPE: c_ulong = 0x4004_5564;

pub const SET_KEY: c_ulong = 0x4004_5565;

pub const SET_RELATIVE_AXIS: c_ulong = 0x4004_5566;

pub const SET_ABSOLUTE_AXIS: c_ulong = 0x4004_5567;

pub const SET_MISC: c_ulong = 0x4004_5568;

pub const SET_PHYSICAL_PATH: c_ulong = 0x4008_556c;

pub const SET_PROPERTY: c_ulong = 0x4004_556e;

pub const GET_SYSTEM_NAME: c_ulong = 0x8040_552c;

pub const SYSTEM_NAME: u32 = 64;

pub const NAME: u32 = 80;

#[repr(C)]
pub struct Waiting {
    pub descriptor: c_int,
    pub events: c_short,
    pub returned: c_short,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputId {
    pub bus: BusType,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[repr(C)]
pub struct UinputSetup {
    pub id: InputId,
    pub name: [c_char; 80],
    pub effects: u32,
}

#[repr(C)]
pub struct AbsoluteSetup {
    pub code: u16,
    pub information: AbsInfo,
}

unsafe extern "C" {
    pub fn poll(waiting: *mut Waiting, many: c_ulong, patience: c_int) -> c_int;

    pub fn inotify_init1(flags: c_int) -> c_int;

    pub fn inotify_add_watch(descriptor: c_int, path: *const c_char, mask: u32) -> c_int;

    pub fn ioctl(file: c_int, request: c_ulong, ...) -> c_int;

    pub fn fcntl(file: c_int, command: c_int, ...) -> c_int;
}

pub fn checked(code: c_int) -> io::Result<c_int> {
    match code {
        -1 => Err(io::Error::last_os_error()),
        code => Ok(code),
    }
}
