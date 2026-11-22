use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicU8, Ordering};

static ASKED_TO_STOP: AtomicU8 = AtomicU8::new(0);

type Handler = extern "C" fn(i32);

unsafe fn install_handler(_signals: &[i32], _by: Handler) -> Result<(), u8> {
    Ok(())
}

fn tidy() -> u8 {
    1
}

extern "C" fn recorded(_number: i32) {
    ASKED_TO_STOP.store(1, Ordering::SeqCst);
}

extern "C" fn talkative(number: i32) {
    //~v WARN EXPLICIT095_NO_UNSAFE_IN_A_HANDLER
    let said = number.to_string();
    //~v WARN EXPLICIT095_NO_UNSAFE_IN_A_HANDLER
    let _ = tidy();
    let _ = said;
}

fn listening() -> Result<(), u8> {
    // SAFETY: the case is about what the handler calls.
    unsafe { install_handler(&[15], recorded) }?;

    // SAFETY: the case is about what the handler calls.
    unsafe { install_handler(&[15], talkative) }
}

fn started(mut command: Command) -> Command {
    // SAFETY: the case is about what the closure calls.
    unsafe {
        let _ = command.pre_exec(|| {
            ASKED_TO_STOP.store(0, Ordering::SeqCst);

            Ok(())
        });
    }

    // SAFETY: the case is about what the closure calls.
    unsafe {
        let _ = command.pre_exec(|| {
            //~v WARN EXPLICIT095_NO_UNSAFE_IN_A_HANDLER
            let home = std::env::var("HOME");
            let _ = home;

            Ok(())
        });
    }

    command
}

fn main() {
    let _ = listening();
    let _ = started(Command::new("true"));
}
