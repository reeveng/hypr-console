//! The half of the greeter that touches the machine: the palette, the
//! display, the pad, the touchscreen and the two pipes to the login window.
//!
//! It waits on the pad and on its stdin together, and draws again only when
//! what it decided changed -- once for everything that woke it, not once per
//! event, because a finger dragged across the dots arrives as a run of moves
//! and a whole frame drawn for each of them is a line that trails the finger. It stops when the window says welcome, and fails
//! when the window goes away, because a greeter nobody is listening to is a
//! screen that lies about being able to let anybody in.

use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::AsFd;
use std::process::ExitCode;

use console_core_color::palette::{Wearing, WearingError};
use console_core_geometry::{Point, Size};
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_draw_painting::Cannot;
use console_input_event_devices::devices::{Devices, Ready};
use console_input_event_devices::touches::ScreenTouch;
use console_login_greeter::display::{Display, Mapping, Unshown};
use console_login_greeter::greeting::{Greeter, GreeterEffect, GreeterEvent, Greeting};
use console_login_greeter::picture::{in_the_room, painted};
use console_login_greeter::turn::{changed, drawn_at, laid_into, on_the_picture};
use console_login_pattern::Touch;
use console_login_window::protocol::{ToGreeter, line_from_greeter, to_greeter};
use console_program_contract::{Arguments, Effect, Event, Exit, Initial, Program, Update};

struct Running<'a> {
    devices: Devices,
    display: Display,
    frames: Frames,
    greeting: Greeting,
    from_window: BufReader<io::StdinLock<'a>>,
}

struct Frames {
    shown: Vec<u8>,
    drawing: Vec<u8>,
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

fn main() -> ExitCode {
    let Ok(ended) = run();

    match ended {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("login-greeter: {why}");

            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<Result<(), GreeterError>, Never> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let borrowed: Vec<&str> = words.iter().map(String::as_str).collect();
    let Ok(arguments) = Arguments::of(&borrowed);
    let wearing = match Wearing::worn().map_err(GreeterError::Palette) {
        Ok(wearing) => wearing,
        Err(why) => return Ok(Err(why)),
    };
    let mut display = match Display::open() {
        Ok(display) => display,
        Err(why) => return Ok(Err(GreeterError::Display(why))),
    };
    let devices = match Devices::open() {
        Ok(devices) => devices,
        Err(why) => return Ok(Err(GreeterError::Watching(why))),
    };
    let Initial { state, subscriptions: _ } = Greeter::init(&arguments);
    let greeting = state;
    let stdin = io::stdin();
    let from_window = BufReader::new(stdin.lock());

    let frames = match draw(&mut display, Frames { shown: Vec::new(), drawing: Vec::new() }, &greeting, &wearing) {
        Ok(frames) => frames,
        Err(why) => return Ok(Err(why)),
    };

    let state = Running { devices, display, frames, greeting, from_window };

    let greeted = iterate(state, |Running { mut devices, mut display, mut frames, mut greeting, mut from_window }| {
        let woke = match devices.wait(Some(stdin.as_fd())) {
            Ok(woke) => woke,
            Err(why) => return Ok(Step::Halt(Err(GreeterError::Waiting(why)))),
        };
        let Ok(panel) = display.size();
        let mut queue: Vec<Event<GreeterEvent>> =
            woke.presses.into_iter().map(|press| Event::Custom(GreeterEvent::Pressed(press))).collect();

        queue.extend(woke.touches.into_iter().map(|touch| {
            let Ok(touch) = to_touch(panel, touch);

            Event::Custom(GreeterEvent::Touched(touch))
        }));

        match woke.also {
            Ready::Yes => match lines(&mut from_window) {
                Ok(lines) => queue.extend(lines.into_iter().map(|line| Event::Custom(GreeterEvent::Received(line)))),
                Err(why) => return Ok(Step::Halt(Err(why))),
            },
            Ready::No => {}
        }

        let shown = greeting.clone();

        for event in queue {
            let Update { state: next, effects } = Greeter::update(&greeting, &event);

            greeting = next;

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
                    Effect::Stop(Exit::Success) => return Ok(Step::Halt(Ok(()))),
                    Effect::Stop(Exit::Failure(why)) => return Ok(Step::Halt(Err(GreeterError::Stopped(why)))),
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

        match greeting == shown {
            true => {}
            false => match draw(&mut display, frames, &greeting, &wearing) {
                Ok(drawn) => frames = drawn,
                Err(why) => return Ok(Step::Halt(Err(why))),
            },
        }

        Ok(Step::Again(Running { devices, display, frames, greeting, from_window }))
    });

    Ok(match greeted {
        Ok(greeted) => greeted,
        Err(Endless) => Ok(()),
    })
}

fn to_touch(panel: Size<u32>, touch: ScreenTouch) -> Result<Touch, Never> {
    let room = |share: Point<f64>| {
        let Ok(on_the_picture) = on_the_picture(panel, share);
        let Ok(canvas) = drawn_at(panel);
        let Ok(room) = in_the_room(canvas, on_the_picture);

        room
    };

    Ok(match touch {
        ScreenTouch::Down(share) => Touch::Down(room(share)),
        ScreenTouch::Moved(share) => Touch::Moved(room(share)),
        ScreenTouch::Up => Touch::Up,
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

fn draw(display: &mut Display, frames: Frames, greeting: &Greeting, wearing: &Wearing) -> Result<Frames, GreeterError> {
    let Ok(panel) = display.size();
    let Ok(canvas) = drawn_at(panel);
    let Frames { shown, drawing } = frames;
    let Ok(painted) = painted(drawing, greeting, wearing, canvas);
    let pixels = painted.map_err(GreeterError::Drawing)?;
    let Ok(band) = changed(&shown, &pixels, canvas);

    match band {
        Some(rows) => {
            let Ok(Mapping { bytes, pitch }) = display.mapped();
            let Ok(()) = laid_into(&pixels, panel, rows, bytes, pitch);
        }
        None => {}
    }

    Ok(Frames { shown: pixels, drawing: shown })
}
