//! Being asked to stop.
//!
//! A chooser hands the screen over by sending whoever holds it a SIGTERM and
//! then waiting for the lock. Answering that signal is not tidiness, it is how
//! two of these take turns, and a chooser that sleeps through it holds the
//! screen shut against the next one.
//!
//! glib stopped binding `g_unix_signal_add`, which is what puts a signal on
//! the main loop rather than in the middle of whatever the process was doing.
//! The function is still in the library this links against, so it is asked for
//! by name. Blocking the signals and waiting for them on a thread of our own
//! was the other way, and it is the wrong one: a blocked mask is inherited by
//! every child the panel starts.
//!
//! Above everything else on the loop, which is not a preference. A panel whose
//! compositor has gone spins: the display's own source is handed a socket that
//! is hung up, says it is ready, is dispatched, finds nothing, and says it is
//! ready again, forever. At the same priority the signal waits its turn behind
//! that and never gets one -- a stray viewer sat at ninety per cent of a core
//! for forty minutes, ignoring every SIGTERM, holding the screen shut against
//! each panel that asked for it after. Being asked to stop is the one thing
//! that has to keep working when the loop is otherwise wedged, so it goes
//! first.

use std::rc::Rc;

use console_core_never::Never;
use gtk4::glib;

pub const STOPPING: [i32; 3] = [libc::SIGHUP, libc::SIGINT, libc::SIGTERM];

unsafe extern "C" {
    fn g_unix_signal_add_full(
        priority: i32,
        signum: i32,
        function: glib::ffi::GSourceFunc,
        data: glib::ffi::gpointer,
        notify: glib::ffi::GDestroyNotify,
    ) -> u32;
}

pub fn stops_when_asked(then: impl Fn() + 'static) -> Result<(), Never> {
    let shared: Rc<dyn Fn()> = Rc::new(then);

    for number in STOPPING {
        let held = Box::into_raw(Box::new(Rc::clone(&shared))).cast::<std::ffi::c_void>();

        // SAFETY: the box is handed over with the notify that frees it, and
        unsafe {
            g_unix_signal_add_full(
                glib::ffi::G_PRIORITY_HIGH,
                number,
                Some(answer),
                held,
                Some(forget),
            );
        }
    }

    Ok(())
}

unsafe extern "C" fn answer(data: glib::ffi::gpointer) -> glib::ffi::gboolean {
    // SAFETY: `data` is the box `stops_when_asked` leaked, and glib hands back
    let then = unsafe { &*data.cast::<Rc<dyn Fn()>>() };
    then();
    glib::ffi::GFALSE
}

unsafe extern "C" fn forget(data: glib::ffi::gpointer) {
    // SAFETY: the same pointer again, and this is the notify glib calls once
    drop(unsafe { Box::from_raw(data.cast::<Rc<dyn Fn()>>()) });
}
