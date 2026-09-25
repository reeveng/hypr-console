//! The half of the greeter that touches the machine: the palette, the
//! display, the pad, the touchscreen and the two pipes to the login window.
//!
//! It waits on the pad and on its stdin together, and draws again only when
//! what it decided changed. It stops when the window says welcome, and fails
//! when the window goes away, because a greeter nobody is listening to is a
//! screen that lies about being able to let anybody in.

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Write};
use std::os::fd::AsFd;
use std::process::ExitCode;

use console_core_color::palette::{Wearing, WearingError};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_draw_painting::{Cannot, Frame, onto};
use console_input_event_devices::devices::{Devices, Ready};
use console_input_event_devices::touches::ScreenTouch;
use console_login_greeter::display::{Display, Unshown};
use console_login_greeter::greeting::{Greeter, GreeterEffect, GreeterEvent, Greeting};
use console_login_greeter::picture::{in_the_room, picture};
use console_login_greeter::turn::{drawn_at, on_the_picture, turned};
use console_login_pattern::Touch;
use console_login_window::protocol::{ToGreeter, line_from_greeter, to_greeter};
use console_program_contract::{Arguments, Effect, Event, Exit, Initial, Program, Update};

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
    let Ok(ended) = greeted();

    match ended {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("login-greeter: {why}");

            ExitCode::FAILURE
        }
    }
}

fn greeted() -> Result<Result<(), GreeterError>, Never> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let borrowed: Vec<&str> = words.iter().map(String::as_str).collect();
    let Ok(arguments) = Arguments::of(&borrowed);
    let wearing = match Wearing::worn().map_err(GreeterError::Palette) {
        Ok(wearing) => wearing,
        Err(why) => return Ok(Err(why)),
    };
    let mut display = match Display::opened() {
        Ok(display) => display,
        Err(why) => return Ok(Err(GreeterError::Display(why))),
    };
    let mut devices = match Devices::watched() {
        Ok(devices) => devices,
        Err(why) => return Ok(Err(GreeterError::Watching(why))),
    };
    let Initial { state, subscriptions: _ } = Greeter::init(&arguments);
    let mut greeting = state;
    let stdin = io::stdin();
    let mut from_window = BufReader::new(stdin.lock());

    match drawn(&mut display, &greeting, &wearing) {
        Ok(()) => {}
        Err(why) => return Ok(Err(why)),
    }

    loop {
        let woke = match devices.waited(Some(stdin.as_fd())) {
            Ok(woke) => woke,
            Err(why) => return Ok(Err(GreeterError::Waiting(why))),
        };
        let Ok(panel) = display.size();
        let mut queue: VecDeque<Event<GreeterEvent>> =
            woke.presses.into_iter().map(|press| Event::Custom(GreeterEvent::Pressed(press))).collect();

        queue.extend(woke.touches.into_iter().map(|touch| {
            let Ok(touch) = touched(panel, touch);

            Event::Custom(GreeterEvent::Touched(touch))
        }));

        match woke.also {
            Ready::Yes => match lines(&mut from_window) {
                Ok(lines) => queue.extend(lines.into_iter().map(|line| Event::Custom(GreeterEvent::Received(line)))),
                Err(why) => return Ok(Err(why)),
            },
            Ready::No => {}
        }

        while let Some(event) = queue.pop_front() {
            let Update { state: next, effects } = Greeter::update(&greeting, &event);
            let changed = next != greeting;

            greeting = next;

            match changed {
                true => match drawn(&mut display, &greeting, &wearing) {
                    Ok(()) => {}
                    Err(why) => return Ok(Err(why)),
                },
                false => {}
            }

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
                            Err(why) => return Ok(Err(GreeterError::Sending(why))),
                        }
                    }
                    Effect::Stop(Exit::Success) => return Ok(Ok(())),
                    Effect::Stop(Exit::Failure(why)) => return Ok(Err(GreeterError::Stopped(why))),
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
    }
}

fn touched(panel: Size<u32>, touch: ScreenTouch) -> Result<Touch, Never> {
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
    let mut received = Vec::new();

    loop {
        let mut line = String::new();

        match from_window.read_line(&mut line) {
            Ok(0) => return Err(GreeterError::WindowGone),
            Ok(_) => {
                let Ok(line) = to_greeter(&line);

                received.extend(line);
            }
            Err(why) => return Err(GreeterError::Receiving(why)),
        }

        match from_window.buffer().is_empty() {
            true => return Ok(received),
            false => {}
        }
    }
}

fn drawn(display: &mut Display, greeting: &Greeting, wearing: &Wearing) -> Result<(), GreeterError> {
    let Ok(panel) = display.size();
    let Ok(canvas) = drawn_at(panel);
    let Ok(shapes) = picture(greeting, wearing, canvas);
    let Ok(long) = console_core_number_conversion::index(u64::from(canvas.width).saturating_mul(u64::from(canvas.height)).saturating_mul(4));
    let mut pixels = vec![0_u8; long];
    let frame = Frame { device: canvas, points: Size { width: canvas.width, height: canvas.height } };

    match onto(&mut pixels, frame, &shapes) {
        Ok(()) => {}
        Err(why) => return Err(GreeterError::Drawing(why)),
    }

    let Ok(laid) = turned(&pixels, panel);
    let Ok(()) = display.shown(&laid);

    Ok(())
}
