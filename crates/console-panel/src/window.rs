//! A program with a window of its own rather than a card.
//!
//! Books and notes draw on `console-draw-surface` themselves, and each wrote
//! the same `main` and the same gathering of a turn's presses into one
//! answer before the second of them was compared with the first. A card is
//! what `card` hands over; this is what such a program has left once it holds
//! its own loop.

use std::fmt::Display;
use std::process::ExitCode;

use console_core_arguments::Command;
use console_core_geometry::Size;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_draw_surface::standing::{KeyboardEvent, PointerEvent};
use console_draw_surface::{Keysym, Surface};

use crate::card::refusal;
use crate::picker::{self, Alone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Update {
    None,
    Redraw,
    Exit,
}

pub trait Responder {
    fn key_pressed(&mut self, key: Keysym, room: Size<u32>) -> Result<Update, Never>;

    fn pointer_event(&mut self, event: PointerEvent, room: Size<u32>) -> Result<Update, Never>;
}

pub fn open<E: Display>(
    asked: &[String],
    program: InternalProgram,
    command: &Command,
    run: fn(&[String]) -> Result<(), E>,
) -> Result<ExitCode, Never> {
    let Ok(refusing) = refusal(command, asked);

    match refusing {
        Some(code) => return Ok(code),
        None => {},
    }

    let Ok(named) = program.name();
    let Ok(alone) = picker::alone_as(named, asked);

    match alone {
        Alone::Yes => {},
        Alone::No => return Ok(ExitCode::SUCCESS),
    }

    Ok(match run(asked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{named}: {fault}");

            ExitCode::FAILURE
        },
    })
}

pub fn handle_input(responder: &mut impl Responder, surface: &mut Surface, room: Size<u32>) -> Result<Update, Never> {
    let Ok(keys) = surface.keyboard_events();
    let Ok(pointer) = surface.pointer_events();
    let mut outcome = Update::None;

    for KeyboardEvent::Down { key } in keys {
        let Ok(update) = responder.key_pressed(key, room);
        let Ok(merged) = merge(outcome, update);

        outcome = merged;
    }

    for event in pointer {
        let Ok(update) = responder.pointer_event(event, room);
        let Ok(merged) = merge(outcome, update);

        outcome = merged;
    }

    Ok(outcome)
}

pub fn merge(was: Update, now: Update) -> Result<Update, Never> {
    Ok(match (was, now) {
        (Update::Exit, _) | (_, Update::Exit) => Update::Exit,
        (Update::Redraw, _) | (_, Update::Redraw) => Update::Redraw,
        (Update::None, Update::None) => Update::None,
    })
}
