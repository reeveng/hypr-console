//! The bar over the ring, at the edge: what it reads off the machine, and what
//! a press on it runs.
//!
//! **Sleep and Shut Down are the greeter's own to ask for.** logind lets the
//! session that is in front put the machine to sleep or stop it, and the
//! greeter's session is the one in front before anybody logs in. That is how
//! GDM's and LightDM's login screens do it, and it keeps the login window --
//! the one that runs as root -- to the one question of who may come in. Over a
//! lock the same calls are the person's, for the same reason.
//!
//! **The sound is whoever's sound server answers.** Over a lock it is the
//! person's. Before a login it is the greeter account's, which a machine may
//! never have started, and a sound nobody answered for is left off the bar
//! rather than drawn silent.
//!
//! What is read again waits for the minute to turn, because the clock is the
//! one thing on the bar that moves on its own and the battery is read with it.
//! A step of the volume reads the sound again as soon as it is taken, because a
//! person is looking at the figure to see whether it moved.

use std::process::{Command, Stdio};

use console_core_never::Never;
use console_status_bar::clock::{self, Standing};
use console_status_bar::lock_screen::{LockScreenBarEffect, LockScreenBarEvent, READ};
use console_status_bar::reading::{Reading, StatusItem, read_level};

use crate::greeting::GreeterEvent;

fn read(item: StatusItem) -> Result<Option<Reading>, Never> {
    match item {
        StatusItem::Sound => read_level(),
        StatusItem::Battery | StatusItem::Bluetooth | StatusItem::Network => item.reading().map(Some),
    }
}

fn told(asked: LockScreenBarEvent) -> Result<GreeterEvent, Never> {
    Ok(GreeterEvent::Bar(asked))
}

pub fn read_all() -> Result<Vec<GreeterEvent>, Never> {
    let Ok(now) = clock::now();
    let Ok(ticked) = told(LockScreenBarEvent::Ticked(now));
    let mut read_out = vec![ticked];

    for item in READ {
        let Ok(reading) = read(item);
        let Ok(said) = told(LockScreenBarEvent::Read(item, reading));

        read_out.push(said);
    }

    Ok(read_out)
}

pub fn again(was: Standing) -> Result<(Standing, Vec<GreeterEvent>), Never> {
    let Ok(now) = clock::current();

    match now == was {
        true => Ok((now, Vec::new())),
        false => {
            let Ok(read_out) = read_all();

            Ok((now, read_out))
        }
    }
}

fn ran(mut command: Command) -> Result<(), Never> {
    command.stdin(Stdio::null()).stdout(Stdio::null());

    match command.status() {
        Ok(status) => match status.success() {
            true => {}
            false => eprintln!("{command:?} ended {status}"),
        },
        Err(why) => eprintln!("{command:?} would not start: {why}"),
    }

    Ok(())
}

pub fn carried(effect: LockScreenBarEffect) -> Result<Vec<GreeterEvent>, Never> {
    match effect {
        LockScreenBarEffect::Volume(volume) => {
            let Ok(command) = volume.command();
            let Ok(()) = ran(command);

            Ok(Vec::new())
        }
        LockScreenBarEffect::Power(power) => {
            let Ok(command) = power.command();
            let Ok(()) = ran(command);

            Ok(Vec::new())
        }
        LockScreenBarEffect::Read(item) => {
            let Ok(reading) = read(item);
            let Ok(said) = told(LockScreenBarEvent::Read(item, reading));

            Ok(vec![said])
        }
    }
}
