//! The pool, as a program.
//!
//! Everything that decides anything is in `console_event_broker::pool`, where
//! who hears what can be asked without opening a socket, and everything that
//! holds a socket is in `serving`, where a test can start it on one of its own.
//! What is here is the two real ones: the machine's runtime directory, and the
//! machine's compositor.
//!
//! It is deliberately not written to `console_program_contract` yet. The
//! contract's loop carries out doings on one thread and this is a program whose
//! whole job is holding sockets open, so it wants the thread-per-source shape
//! the document describes and the contract's loop does not have yet. Writing it
//! against a runtime that would have to grow for it is how a shared thing gets
//! bent to one caller.

use std::process::ExitCode;

use console_event_broker::{place, serving, sources};

fn main() -> ExitCode {
    let socket = match place::socket() {
        Ok(socket) => socket,
        Err(fault) => {
            eprintln!("console-events: {fault}, so there is nowhere to put the socket");

            return ExitCode::FAILURE;
        }
    };

    match serving::serve(&socket, sources::hold) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("console-events: {fault}");

            ExitCode::FAILURE
        }
    }
}
