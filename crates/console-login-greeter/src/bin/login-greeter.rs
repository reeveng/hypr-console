//! The half of the greeter that touches the machine: the palette, the
//! display, the pad, the touchscreen and the two pipes to the login window.
//!
//! It waits on the pad and on its stdin together, and draws again only when
//! what it decided changed -- once for everything that woke it, not once per
//! event, because a finger dragged across the dots arrives as a run of moves
//! and a whole frame drawn for each of them is a line that trails the finger. It stops when the window says welcome, and fails
//! when the window goes away, because a greeter nobody is listening to is a
//! screen that lies about being able to let anybody in.
//!
//! The wait ends when the minute turns as well, because the bar over the ring
//! has a clock on it and a battery that runs down whether anybody presses
//! anything or not. A finger put down on the bar is the bar's, and it is asked
//! of the bar that is on the screen, which is the one drawn last.

use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::AsFd;
use std::process::ExitCode;

use console_core_color::palette::{self, Wearing, WearingError};
use console_core_geometry::{Point, Size};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_draw_painting::Cannot;
use console_input_event_devices::devices::{Devices, Ready};
use console_input_event_devices::touches::ScreenTouch;
use console_login_greeter::bar;
use console_login_greeter::display::{Display, Mapping, Unshown};
use console_login_greeter::greeting::{GreeterEffect, GreeterEvent, GreeterState, Greeter, Status};
use console_login_greeter::picture::{RenderedFrame, Worn, in_the_room, on_the_bar, painted};
use console_login_greeter::turn::{changed, drawn_at, laid_into, on_the_picture};
use console_login_pattern::Touch;
use console_login_window::protocol::{ToGreeter, line_from_greeter, to_greeter};
use console_core_state_machine::{Machine, Transition};
use console_core_arguments::{Command, Operands, Presence, read};
use console_login_window::way_in::FELL;
use console_program_contract::{Effect, Event, Exit};
use console_status_bar::clock::{self, Standing};
use console_status_bar::lock_screen::LockScreenBarEvent;
use console_status_bar::showing::{self, Rendered};

struct Running<'a> {
    devices: Devices,
    display: Display,
    frames: Frames,
    state: GreeterState,
    standing: Standing,
    from_window: BufReader<io::StdinLock<'a>>,
}

struct Frames {
    shown: Vec<u8>,
    drawing: Vec<u8>,
    bar: Rendered,
}

enum Handled {
    Going(GreeterState),
    Stopped(Result<(), GreeterError>),
}

enum GreeterError {
    Palette(WearingError),
    Display(Unshown),
    Watching(io::Error),
    Waiting(io::Error),
    WindowGone,
    Receiving(io::Error),
    Sending(io::Error),
    Drawing(Cannot),
    Stopped(String),
}

impl std::fmt::Display for GreeterError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GreeterError::Palette(why) => write!(to, "{why}"),
            GreeterError::Display(why) => write!(to, "{why}"),
            GreeterError::Watching(why) => write!(to, "cannot watch the pad: {why}"),
            GreeterError::Waiting(why) => write!(to, "cannot wait for a press: {why}"),
            GreeterError::WindowGone => write!(to, "the login window went away"),
            GreeterError::Receiving(why) => write!(to, "cannot hear the login window: {why}"),
            GreeterError::Sending(why) => write!(to, "the login window did not hear: {why}"),
            GreeterError::Drawing(why) => write!(to, "{why}"),
            GreeterError::Stopped(why) => write!(to, "{why}"),
        }
    }
}

const COMMAND: Command = Command {
    name: "login-greeter",
    about: "the pattern screen the login window puts up before a desktop",
    flags: &[FELL],
    operands: Operands::None,
};

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let line = match read(&COMMAND, &words) {
        Ok(line) => line,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };
    let Ok(fell) = line.presence(FELL);

    let opening = match fell {
        Presence::Present => Status::Fell,
        Presence::Absent => Status::Waiting,
    };
    let Ok(ended) = run(&opening);

    match ended {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("login-greeter: {why}");

            ExitCode::FAILURE
        }
    }
}

fn worn() -> Result<(Wearing, showing::Wearing), WearingError> {
    let spent = palette::spent()?;
    let ring = Wearing::out_of(&spent)?;
    let bar = showing::Wearing::out_of(&spent)?;

    Ok((ring, bar))
}

fn run(opening: &Status) -> Result<Result<(), GreeterError>, Never> {
    let (ring, bar) = match worn().map_err(GreeterError::Palette) {
        Ok(wearing) => wearing,
        Err(why) => return Ok(Err(why)),
    };
    let worn = Worn { ring: &ring, bar: &bar };
    let mut display = match Display::open() {
        Ok(display) => display,
        Err(why) => return Ok(Err(GreeterError::Display(why))),
    };
    let devices = match Devices::open() {
        Ok(devices) => devices,
        Err(why) => return Ok(Err(GreeterError::Watching(why))),
    };
    let Ok(Transition { state, effects: _ }) = Greeter::initial_transition(opening, None);
    let Ok(standing) = clock::current();
    let Ok(read_out) = bar::read_all();
    let stdin = io::stdin();
    let from_window = BufReader::new(stdin.lock());

    let state = match handled(state, read_out.into_iter().map(Event::Custom).collect()) {
        Ok(Handled::Going(state)) => state,
        Ok(Handled::Stopped(stopped)) => return Ok(stopped),
        Err(why) => return Ok(Err(why)),
    };

    let nothing_drawn = Rendered { shapes: Vec::new(), room: Size { width: 0, height: 0 }, touching: Vec::new() };
    let frames = match draw(&mut display, Frames { shown: Vec::new(), drawing: Vec::new(), bar: nothing_drawn }, &state, worn) {
        Ok(frames) => frames,
        Err(why) => return Ok(Err(why)),
    };

    let running = Running { devices, display, frames, state, standing, from_window };

    let greeted = iterate(running, |Running { mut devices, mut display, mut frames, state, standing, mut from_window }| {
        let Ok(patience) = clock::until_the_minute_turns();
        let woke = match devices.wait(Some(stdin.as_fd()), Some(patience)) {
            Ok(woke) => woke,
            Err(why) => return Ok(Step::Halt(Err(GreeterError::Waiting(why)))),
        };
        let Ok(panel) = display.size();
        let mut queue: Vec<Event<GreeterEvent>> =
            woke.presses.into_iter().map(|press| Event::Custom(GreeterEvent::Pressed(press))).collect();

        queue.extend(woke.touches.into_iter().map(|touch| {
            let Ok(heard) = to_event(panel, &frames.bar, touch);

            Event::Custom(heard)
        }));

        match woke.also {
            Ready::Yes => match lines(&mut from_window) {
                Ok(lines) => queue.extend(lines.into_iter().map(|line| Event::Custom(GreeterEvent::Received(line)))),
                Err(why) => return Ok(Step::Halt(Err(why))),
            },
            Ready::No => {}
        }

        let Ok((standing, read_out)) = bar::again(standing);

        queue.extend(read_out.into_iter().map(Event::Custom));

        let shown = state.clone();

        let state = match handled(state, queue) {
            Ok(Handled::Going(state)) => state,
            Ok(Handled::Stopped(stopped)) => return Ok(Step::Halt(stopped)),
            Err(why) => return Ok(Step::Halt(Err(why))),
        };

        match state == shown {
            true => {}
            false => match draw(&mut display, frames, &state, worn) {
                Ok(drawn) => frames = drawn,
                Err(why) => return Ok(Step::Halt(Err(why))),
            },
        }

        Ok(Step::Again(Running { devices, display, frames, state, standing, from_window }))
    });

    Ok(match greeted {
        Ok(greeted) => greeted,
        Err(Endless) => Ok(()),
    })
}

fn handled(state: GreeterState, events: Vec<Event<GreeterEvent>>) -> Result<Handled, GreeterError> {
    let unanswered = state.clone();
    let ended = iterate((state, events), |(state, events)| {
        let mut state = state;
        let mut later = Vec::new();

        for event in events {
            let Ok(Transition { state: next, effects }) = Greeter::transition(state, event);

            state = next;

            for effect in effects {
                match effect {
                    Effect::Custom(GreeterEffect::Send(message)) => {
                        let Ok(line) = line_from_greeter(&message);
                        let mut out = io::stdout().lock();
                        let wrote = out.write_all(line.as_bytes());
                        let flushed = match wrote {
                            Ok(()) => out.flush(),
                            Err(why) => Err(why),
                        };

                        match flushed {
                            Ok(()) => {}
                            Err(why) => return Ok(Step::Halt(Err(GreeterError::Sending(why)))),
                        }
                    }
                    Effect::Custom(GreeterEffect::Bar(asked)) => {
                        let Ok(heard) = bar::carried(asked);

                        later.extend(heard.into_iter().map(Event::Custom));
                    }
                    Effect::Stop(Exit::Success) => return Ok(Step::Halt(Ok(Handled::Stopped(Ok(()))))),
                    Effect::Stop(Exit::Failure(why)) => return Ok(Step::Halt(Ok(Handled::Stopped(Err(GreeterError::Stopped(why)))))),
                    Effect::Run(_)
                    | Effect::Stream(_)
                    | Effect::Prompt(_)
                    | Effect::Spawn(_)
                    | Effect::Subscribe(_)
                    | Effect::Unsubscribe(_)
                    | Effect::Write(_)
                    | Effect::Notify(_)
                    | Effect::Print(_) => {}
                }
            }
        }

        Ok(match later.is_empty() {
            true => Step::Halt(Ok(Handled::Going(state))),
            false => Step::Again((state, later)),
        })
    });

    match ended {
        Ok(ended) => ended,
        Err(Endless) => Ok(Handled::Going(unanswered)),
    }
}

fn to_event(panel: Size<u32>, bar: &Rendered, touch: ScreenTouch) -> Result<GreeterEvent, Never> {
    let Ok(canvas) = drawn_at(panel);
    let room = |share: Point<f64>| {
        let Ok(on_the_picture) = on_the_picture(panel, share);
        let Ok(room) = in_the_room(canvas, on_the_picture);

        room
    };

    Ok(match touch {
        ScreenTouch::Down(share) => {
            let Ok(on_the_picture) = on_the_picture(panel, share);
            let Ok(point) = on_the_bar(canvas, on_the_picture);
            let Ok(tapped) = bar.on(point);

            match tapped {
                Some(action) => GreeterEvent::Bar(LockScreenBarEvent::Tapped(action)),
                None => GreeterEvent::Touched(Touch::Down(room(share))),
            }
        }
        ScreenTouch::Moved(share) => GreeterEvent::Touched(Touch::Moved(room(share))),
        ScreenTouch::Up => GreeterEvent::Touched(Touch::Up),
    })
}

fn lines(from_window: &mut BufReader<io::StdinLock<'_>>) -> Result<Vec<ToGreeter>, GreeterError> {
    let read = iterate((from_window, Vec::new()), |(from_window, mut received)| {
        let mut line = String::new();

        match from_window.read_line(&mut line) {
            Ok(0) => return Ok(Step::Halt(Err(GreeterError::WindowGone))),
            Ok(_) => {
                let Ok(line) = to_greeter(&line);

                received.extend(line);
            }
            Err(why) => return Ok(Step::Halt(Err(GreeterError::Receiving(why)))),
        }

        Ok(match from_window.buffer().is_empty() {
            true => Step::Halt(Ok(received)),
            false => Step::Again((from_window, received)),
        })
    });

    match read {
        Ok(read) => read,
        Err(Endless) => Err(GreeterError::WindowGone),
    }
}

fn draw(display: &mut Display, frames: Frames, state: &GreeterState, worn: Worn<'_>) -> Result<Frames, GreeterError> {
    let Ok(panel) = display.size();
    let Ok(canvas) = drawn_at(panel);
    let Frames { shown, drawing, bar: _ } = frames;
    let Ok(painted) = painted(drawing, state, worn, canvas);
    let RenderedFrame { pixels, bar } = painted.map_err(GreeterError::Drawing)?;
    let Ok(band) = changed(&shown, &pixels, canvas);

    match band {
        Some(rows) => {
            let Ok(Mapping { bytes, pitch }) = display.mapped();
            let Ok(()) = laid_into(&pixels, panel, rows, bytes, pitch);
        }
        None => {}
    }

    Ok(Frames { shown: pixels, drawing: shown, bar })
}
