//! Hand the session's environment to systemd, then start the desktop.
//!
//! Run by the compositor, once, as it comes up.

use console_session::{run_each, starting};

fn main() {
    run_each(&starting());
}
