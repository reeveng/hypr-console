//! The ten functions and three structs of libpam this tree calls, as the
//! headers under `/usr/include/security` declare them, and nothing else.

use std::ffi::{c_char, c_int, c_void};

pub const SUCCESS: c_int = 0;
pub const BUF_ERR: c_int = 5;
pub const PERM_DENIED: c_int = 6;
pub const AUTH_ERR: c_int = 7;
pub const USER_UNKNOWN: c_int = 10;
pub const MAXTRIES: c_int = 11;
pub const NEW_AUTHTOK_REQD: c_int = 12;
pub const ACCT_EXPIRED: c_int = 13;
pub const CONV_ERR: c_int = 19;

pub const PROMPT_ECHO_OFF: c_int = 1;
pub const PROMPT_ECHO_ON: c_int = 2;
pub const ERROR_MSG: c_int = 3;
pub const TEXT_INFO: c_int = 4;

pub const TTY: c_int = 3;

pub const ESTABLISH_CRED: c_int = 0x0002;
pub const DELETE_CRED: c_int = 0x0004;

#[repr(C)]
pub struct Handle {
    private: [u8; 0],
}

#[repr(C)]
pub struct Message {
    pub style: c_int,
    pub text: *const c_char,
}

#[repr(C)]
pub struct Response {
    pub answer: *mut c_char,
    pub code: c_int,
}

pub type Answer =
    unsafe extern "C" fn(c_int, *mut *const Message, *mut *mut Response, *mut c_void) -> c_int;

#[repr(C)]
pub struct Conversation {
    pub answer: Option<Answer>,
    pub data: *mut c_void,
}

#[link(name = "pam")]
unsafe extern "C" {
    pub fn pam_start_confdir(
        service: *const c_char,
        user: *const c_char,
        conversation: *const Conversation,
        rules: *const c_char,
        handle: *mut *mut Handle,
    ) -> c_int;

    pub fn pam_end(handle: *mut Handle, status: c_int) -> c_int;

    pub fn pam_authenticate(handle: *mut Handle, flags: c_int) -> c_int;

    pub fn pam_acct_mgmt(handle: *mut Handle, flags: c_int) -> c_int;

    pub fn pam_setcred(handle: *mut Handle, flags: c_int) -> c_int;

    pub fn pam_open_session(handle: *mut Handle, flags: c_int) -> c_int;

    pub fn pam_close_session(handle: *mut Handle, flags: c_int) -> c_int;

    pub fn pam_putenv(handle: *mut Handle, pair: *const c_char) -> c_int;

    pub fn pam_getenvlist(handle: *mut Handle) -> *mut *mut c_char;

    pub fn pam_set_item(handle: *mut Handle, kind: c_int, item: *const c_void) -> c_int;
}

unsafe extern "C" {
    #[cfg_attr(dylint_lib = "explicit051_no_machine_width", allow(explicit051_no_machine_width, reason = "calloc's two parameters are C's `size_t`, which is the machine's width by the C ABI and not by choice here"))]
    pub fn calloc(count: usize, size: usize) -> *mut c_void;

    pub fn strdup(text: *const c_char) -> *mut c_char;

    pub fn free(pointer: *mut c_void);
}
