//! The program that holds every panel.
//!
//!     console-panels                     the host, which is how the unit runs it
//!     console-panels launcher            that panel, drawn here, and then gone
//!     console-panels settings-panel Sound
//!
//! With nothing after it, it stays: it opens the display, waits on the socket
//! and draws whatever is asked for. With a panel named, it is the same drawing
//! done the old way, in a process that ends when the card does -- which is what
//! a stand-in falls back to when this is not up, and what a check runs when it
//! wants an opening that owes nothing to a daemon.

fn main() {
    let asked: Vec<String> = std::env::args().skip(1).collect();

    let Ok(()) = console_panel_host::asked_for(&asked);
}
