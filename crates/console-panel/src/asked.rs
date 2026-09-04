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

use std::rc::Rc;

use gtk4::glib;

/// What a chooser is asked to stop with.
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

/// Do this on the main loop, the first time any of them arrives.
pub fn stops_when_asked(then: impl Fn() + 'static) {
    let shared: Rc<dyn Fn()> = Rc::new(then);

    for number in STOPPING {
        let held = Box::into_raw(Box::new(Rc::clone(&shared))).cast::<std::ffi::c_void>();

        // SAFETY: the box is handed over with the notify that frees it, and
        // the source is the main loop's from here on.
        unsafe {
            g_unix_signal_add_full(
                glib::ffi::G_PRIORITY_DEFAULT,
                number,
                Some(answer),
                held,
                Some(forget),
            );
        }
    }
}

/// The signal, arrived somewhere it is safe to do something about.
unsafe extern "C" fn answer(data: glib::ffi::gpointer) -> glib::ffi::gboolean {
    // SAFETY: `data` is the box `stops_when_asked` leaked, and glib hands back
    // the same pointer it was given. It stays alive until `forget` runs, which
    // glib does after the last call to this, so the borrow cannot outlive it.
    let then = unsafe { &*data.cast::<Rc<dyn Fn()>>() };
    then();
    glib::ffi::GFALSE
}

unsafe extern "C" fn forget(data: glib::ffi::gpointer) {
    // SAFETY: the same pointer again, and this is the notify glib calls once
    // when the source is gone. Taking the box back is what frees it, and
    // nothing can reach it afterwards.
    drop(unsafe { Box::from_raw(data.cast::<Rc<dyn Fn()>>()) });
}
