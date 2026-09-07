//! A socket with something on it, answered on the loop that draws.
//!
//! The host waits on two things at once: a thumb, and a panel's own program
//! asking for a surface. Only one of them can be waited on by GTK, so the other
//! has to become a source on the same loop -- otherwise it is a thread, and a
//! thread would mean the request arrives somewhere that cannot touch a widget.
//!
//! glib stopped binding `g_unix_fd_add`, the way it stopped binding
//! `g_unix_signal_add`, and `console_panel::asked` is the same workaround for
//! the same reason: the function is still in the library this links against, so
//! it is asked for by name. What that buys over a thread and a channel is that
//! there is no second thread to be wrong about -- the request is read, the card
//! is built and the surface goes up, all in the order they were asked for, on
//! the one thread that is allowed to draw.
//!
//! A hang-up is watched for as well as a readable byte. A client killed outright
//! leaves a socket that is readable and answers nothing, which is the same
//! moment as far as this is concerned and is how a panel whose program was
//! killed takes its surface down with it.

use std::os::fd::RawFd;

use console_core_never::Never;
use gtk4::glib;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Again {
    Yes,
    No,
}

const SOMETHING: glib::ffi::GIOCondition =
    glib::ffi::G_IO_IN | glib::ffi::G_IO_HUP | glib::ffi::G_IO_ERR;

unsafe extern "C" {
    fn g_unix_fd_add_full(
        priority: i32,
        fd: i32,
        condition: glib::ffi::GIOCondition,
        function: Answering,
        data: glib::ffi::gpointer,
        notify: glib::ffi::GDestroyNotify,
    ) -> u32;
}

type Answering = unsafe extern "C" fn(i32, glib::ffi::GIOCondition, glib::ffi::gpointer) -> i32;

type Told = Box<dyn Fn() -> Again>;

pub fn when_there_is_something(
    fd: RawFd,
    then: impl Fn() -> Again + 'static,
) -> Result<Watching, Never> {
    let held: Told = Box::new(then);
    let held = Box::into_raw(Box::new(held)).cast::<std::ffi::c_void>();

    // SAFETY: the box is handed over with the notify that frees it, and the
    let id = unsafe {
        g_unix_fd_add_full(
            glib::ffi::G_PRIORITY_DEFAULT,
            fd,
            SOMETHING,
            answer,
            held,
            Some(forget),
        )
    };

    Ok(Watching(id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watching(u32);

impl Watching {
    pub fn stop(self) -> Result<(), Never> {
        // SAFETY: the id came from the call above and is removed once, because
        unsafe { glib::ffi::g_source_remove(self.0) };

        Ok(())
    }
}

unsafe extern "C" fn answer(
    _fd: i32,
    _condition: glib::ffi::GIOCondition,
    data: glib::ffi::gpointer,
) -> i32 {
    // SAFETY: `data` is the box `when_there_is_something` leaked, and glib
    let then = unsafe { &*data.cast::<Told>() };

    match then() {
        Again::Yes => glib::ffi::GTRUE,
        Again::No => glib::ffi::GFALSE,
    }
}

unsafe extern "C" fn forget(data: glib::ffi::gpointer) {
    // SAFETY: the same pointer again, and this is the notify glib calls once
    drop(unsafe { Box::from_raw(data.cast::<Told>()) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hang_up_is_something_arriving() {
        assert_ne!(SOMETHING & glib::ffi::G_IO_HUP, 0, "a killed panel would hold its surface up");
        assert_ne!(SOMETHING & glib::ffi::G_IO_IN, 0);
    }
}
