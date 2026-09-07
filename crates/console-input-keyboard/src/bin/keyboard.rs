//! The on-screen keyboard.
//!
//! Started once with the session and kept for it. Most of that time there is
//! nothing of it on the screen: `--hidden` starts it away, and the controller
//! shows and hides it with a signal, because a keyboard that was started and
//! stopped would pay for a compositor connection, ten composed keymaps and a
//! font every time somebody wanted to type a word.
//!
//!     virtual-keyboard --landscape-layers landscape,thai,landscapespecial
//!
//! The unit runs this, and `keyboard-toggle` is the signal. It reads the
//! palette on the way in rather than being handed it: there was a second
//! program that did the reading and exec'd this one, from when this one was C
//! and could not be given a Rust crate to ask. Both ends are Rust in one crate
//! now, and the indirection cost more than it carried -- a unit that names a
//! program that starts the program that matters is a unit `named_by` cannot see
//! through, so a new keyboard could be installed and the old one go on running.
//!
//! The palette it reads is the one its own tree spends, not the one at `/`.
//! There is a second tree: the nested desktop stages the whole of this desktop
//! under a directory of its own and runs it there, and a keyboard that joined
//! the palette onto `/` found the machine's own `/usr/local`, read nothing,
//! kept the colours in `config::Scheme::default` and drew a keyboard in green
//! and red that no check could believe. `console_core_colour::spent::beside` is
//! where that is decided, and on the device the two answers are one file.


use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::process::ExitCode;

use console_core_colour::spent::{beside, read};
use console_input_focus::{self as claim, CONTROLLER, Claim, Heard, Spans};
use evdev::{AbsoluteAxisCode, EventType};
use console_input_keyboard::config::{self, Config};
use console_input_keyboard::drawing::Surface;
use console_input_keyboard::gamepad::{self, Asked, Held, Repeats};
use console_input_keyboard::layout::{Drops, Kind, Which, key, mods, named, of, placed, toward, under};
use console_input_keyboard::paint;
use console_input_keyboard::surface::{Gone, Missing, Poke, Screen, Showing};
use std::time::Instant;
use console_input_keyboard::typing::{After, Typist};
use console_response_times::Waiting;

const WALK: [&str; 3] = ["full", "thai", "special"];
const LANDSCAPE_WALK: [&str; 3] = ["landscape", "thai", "landscapespecial"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Landscape,
    Portrait,
}

fn landscape(wide: u32, tall: u32) -> Result<Shape, Never> {
    Ok(match wide > tall {
        true => Shape::Landscape,
        false => Shape::Portrait,
    })
}

fn main() -> ExitCode {
    let Ok(mut waiting) = Waiting::on("keyboard", "starting");
    let argv: Vec<String> = std::env::args().collect();

    match argv.iter().any(|word| word == "--help" || word == "-h") {
        true => {
            println!("usage: virtual-keyboard [--hidden] [-H height] [-l layers] [--fn font]");
            return ExitCode::SUCCESS;
        }
        false => {},
    }

    match argv.iter().any(|word| word == "--list-layers") {
        true => {
            for which in Which::ALL {
                let Ok(of) = of(which);

                println!("{}", of.name);
            }

            return ExitCode::SUCCESS;
        }
        false => {},
    }

    let Ok(flags) = dressed(&argv);

    let Ok(()) = waiting.mark("palette");

    let config = match config::from_env(&flags) {
        Ok(config) => config,
        Err(why) => {
            eprintln!("virtual-keyboard: {why:?}");
            return ExitCode::FAILURE;
        },
    };

    let Ok(()) = waiting.mark("asked");

    match run(&config, waiting) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("virtual-keyboard: {why}");
            ExitCode::FAILURE
        },
    }
}

fn dressed(argv: &[String]) -> Result<Vec<String>, Never> {
    let Ok(me) = me();
    let Ok(at) = beside(&me);

    let held = match std::fs::read_to_string(&at) {
        Ok(held) => held,
        Err(why) => {
            eprintln!("virtual-keyboard: no palette at {}: {why}", at.display());
            String::new()
        },
    };
    let Ok(palette) = read(&held);
    let Ok(missing) = console_input_keyboard::palette::missing(&palette);

    match missing.is_empty() {
        true => {},
        false => {
            eprintln!(
                "virtual-keyboard: the palette has no {}, so the keyboard keeps its own colour \
                 for those",
                missing.join(", ")
            );
        }
    }

    let flags = match argv.split_first() {
        Some((_, flags)) => flags,
        None => &[],
    };

    console_input_keyboard::palette::argv(&palette, flags)
}

fn me() -> Result<std::path::PathBuf, Never> {
    Ok(match std::env::current_exe() {
        Ok(at) => at,
        Err(why) => {
            eprintln!("virtual-keyboard: where this program is: {why}");

            std::path::PathBuf::from(console_input_keyboard::palette::VIRTUAL_KEYBOARD)
        },
    })
}

fn run(config: &Config, mut waiting: Waiting) -> Result<(), String> {
    let told = |why| {
        let Ok(said) = said(why);

        said
    };
    let Ok(root) = console_input_keyboard::keymap::default_symbols_root();
    let alphabets = console_input_keyboard::keymap::available("evdev", &root)
        .map_err(|why| format!("no keymaps: {why:?}. Is xkeyboard-config installed?"))?;

    let Ok(()) = waiting.mark("keymaps");

    let Ok(shape) = landscape(u32::MAX, config.height);
    let Ok((asked, height)) = orientation(config, shape);
    let walk: Vec<Which> = asked
        .iter()
        .filter_map(|name| {
            let Ok(named) = named(name);

            named
        })
        .collect();

    match walk.is_empty() {
        true => {
            return Err(format!(
                "no layer called any of {asked:?}. --list-layers says what there is"
            ));
        }
        false => {},
    }

    let signals = listening().map_err(|why| format!("no signals: {why}"))?;
    let mut screen = Screen::connect().map_err(told)?;

    let Ok(()) = waiting.mark("compositor");

    match config.hidden {
        true => {},
        false => screen.show(height).map_err(told)?,
    }

    let Ok(typing) = screen.typing();
    let typing = typing
        .ok_or("this compositor has no zwp_virtual_keyboard_v1, so nothing here could type")?;
    let manager = typing.clone();
    let Ok(hand) = screen.hand();
    let Ok(seat) = screen.seat();
    let Ok(mut typist) = Typist::new(&manager, seat, &hand, alphabets, walk);

    let Ok(()) = waiting.mark("typist");

    let mut down: Option<usize> = None;
    let mut shown: Option<Drawn> = None;
    let mut showing: Option<Waiting> = None;

    let mut reading: Option<Reading> = None;
    let mut complained = Quiet::No;
    let mut held = Held::default();
    let mut selected: Option<usize> = None;

    let Ok(()) = waiting.mark("pad");
    let Ok(()) = waiting.done();

    while screen.closed() == Ok(Gone::No) {
        let Ok(showing_now) = screen.showing();
        let Ok(()) = keeping(showing_now, &mut reading, &mut complained);

        let Ok(size) = screen.size();
        let Ok(scale) = screen.scale();
        let frame = Drawn {
            showing: typist.showing,
            held: typist.held,
            pressed: down,
            selected,
            size,
            scale,
        };

        match showing_now == Showing::Yes && shown != Some(frame) {
            true => {
                let Ok(layout) = typist.layout();
                let modifiers = typist.held;
                let pressed = down;
                let Ok(next) = typist.next_language();
                let language = next.map(|which| {
                    let Ok(of) = of(which);
                    let Ok(written) = of.alphabet.written();

                    written
                });
                screen
                    .draw(|pixels, wide, tall, scale| {
                        let Ok(stride) = fitted(wide.saturating_mul(4));
                        let Ok(down) = fitted(tall);

                        let Ok(Some(onto)) = Surface::new(
                            pixels,
                            stride,
                            down,
                            f64::from(scale),
                        ) else {
                            return;
                        };

                        let across = f64::from(wide) / f64::from(scale);
                        let deep = f64::from(tall) / f64::from(scale);
                        let Ok(keys) = placed(layout, across, deep);
                        let Ok(()) = paint::keyboard(&onto, &paint::Look {
                            config,
                            layout,
                            keys: &keys,
                            pressed,
                            held: modifiers,
                            selected,
                            language,
                            wide: across,
                            tall: deep,
                        });
                    })
                    .map_err(told)?;
                shown = Some(frame);

                match showing.take() {
                    Some(mut waiting) => {
                        let Ok(()) = waiting.mark("drawn");
                        let Ok(()) = waiting.done();
                    },
                    None => {},
                }
            }
            false => {},
        }

        let now = Instant::now();
        let mut watching: Vec<std::os::fd::RawFd> = vec![signals];

        match reading.as_ref() {
            Some(reading) => {
                let Ok(fds) = reading.claim.watching();

                watching.extend(fds);
            },
            None => {},
        }

        let Ok(until) = held.until(now);
        let spoke = screen.wait_with(&watching, until).map_err(told)?;
        let now = Instant::now();

        match spoke & 1 != 0 {
            true => {
                let Ok(woken) = woken(signals);
                let Ok(showing_now) = screen.showing();

                match woken {
                    Some(Told::Show) => match showing_now {
                        Showing::No => onto_the_screen(&mut screen, height, &mut showing)?,
                        Showing::Yes => {},
                    },
                    Some(Told::Hide) => {
                        let Ok(()) = away(&mut screen, &mut shown, &mut showing, &mut down, &mut held);
                    },
                    Some(Told::Either) => match showing_now {
                        Showing::Yes => {
                            let Ok(()) = away(&mut screen, &mut shown, &mut showing, &mut down, &mut held);
                        },
                        Showing::No => onto_the_screen(&mut screen, height, &mut showing)?,
                    },
                    None => {},
                }
            }
            false => {},
        }

        let mut wants: Vec<Asked> = Vec::new();

        match reading.as_mut() {
            Some(at_hand) => {
                let Ok(heard) = at_hand.claim.arrived();

                let Ok(asked) = from_controller(&heard, &at_hand.spans, &mut held, now);

                wants.extend(asked);

                match heard.gone.is_empty() {
                    true => {},
                    false => {
                        eprintln!("virtual-keyboard: the controller has gone; taking it again");
                        reading = None;
                    },
                }
            },
            None => {},
        }

        let Ok(due) = held.due(now);

        wants.extend(due);

        for want in wants {
            let Ok(showing_now) = screen.showing();

            match showing_now {
                Showing::No => continue,
                Showing::Yes => {},
            }

            let Ok(Some((wide, tall))) = screen.size() else { continue };

            let Ok(layout) = typist.layout();
            let Ok(keys) = placed(layout, f64::from(wide), f64::from(tall));

            match want {
                Asked::Toggle => {
                    let Ok(()) = away(&mut screen, &mut shown, &mut showing, &mut down, &mut held);
                    selected = None;
                },
                _ if matches!(want.direction(), Ok(Some(_))) => {
                    let Ok(direction) = want.direction();
                    let (dx, dy) = direction.unwrap_or((0, 0));
                    let Ok(onto) = toward(&keys, selected, dx, dy);

                    selected = onto;
                },
                Asked::Press => {
                    let Some(at) = selected else {
                        let Ok(onto) = toward(&keys, None, 0, 0);

                        selected = onto;
                        continue;
                    };

                    let Some(key) = layout.keys.get(at) else { continue };

                    let Ok(after) = typist.pressed(key.kind, key.force, key.reset);

                    match after == After::Draw && !matches!(key.kind, Kind::Code { .. }) {
                        true => selected = None,
                        false => {},
                    }
                },
                Asked::Backspace => {
                    let Ok(()) = typist.tap(key::BACKSPACE);
                },
                Asked::Enter => {
                    let Ok(()) = typist.tap(key::ENTER);
                },
                Asked::Shift => {
                    let Ok(_) = typist.pressed(Kind::Mod(mods::SHIFT), mods::NONE, Drops::Nothing);
                },
                Asked::PreviousLanguage | Asked::NextLanguage => {
                    let Ok(mut waiting) = Waiting::here("keyboard", "language");
                    let Ok(_) = typist.pressed(Kind::Language, mods::NONE, Drops::Nothing);
                    let Ok(()) = waiting.mark("keymap");
                    let Ok(()) = waiting.done();
                    selected = None;
                },
                Asked::Up | Asked::Down | Asked::Left | Asked::Right => {},
            }
        }

        let Ok(Some((wide, tall))) = screen.size() else {
            let _ = screen.pokes();
            continue;
        };

        let Ok(pokes) = screen.pokes();

        for poke in pokes {
            let Ok(layout) = typist.layout();
            let Ok(keys) = placed(layout, f64::from(wide), f64::from(tall));

            match poke {
                Poke::Down { x, y } => {
                    let Ok(Some(hit)) = under(&keys, x, y) else { continue };

                    let Some(key) = layout.keys.get(hit.at) else { continue };

                    down = Some(hit.at);
                    let kind = key.kind;
                    let force = key.force;
                    let reset = key.reset;
                    let changing = match matches!(kind, Kind::Language) {
                        true => {
                            let Ok(waiting) = Waiting::here("keyboard", "language");

                            Some(waiting)
                        }
                        false => None,
                    };
                    let Ok(after) = typist.pressed(kind, force, reset);

                    match changing {
                        Some(mut waiting) => {
                            let Ok(()) = waiting.mark("keymap");
                            let Ok(()) = waiting.done();
                        },
                        None => {},
                    }

                    match after {
                        After::Draw => {
                            down = match kind {
                                Kind::Code { .. } => down,
                                Kind::Pad
                                | Kind::Mod(_)
                                | Kind::Copy { .. }
                                | Kind::Layout(_)
                                | Kind::Back
                                | Kind::Next
                                | Kind::Language
                                | Kind::Symbols
                                | Kind::Compose
                                | Kind::EndRow => None,
                            };
                        }
                        After::Still => {},
                    }
                },
                Poke::Moved { .. } => {},
                Poke::Up => down = None,
            }
        }
    }

    Ok(())
}

fn onto_the_screen(
    screen: &mut Screen,
    height: u32,
    showing: &mut Option<Waiting>,
) -> Result<(), String> {
    let told = |why| {
        let Ok(said) = said(why);

        said
    };
    let Ok(mut waiting) = Waiting::here("keyboard", "showing");

    screen.show(height).map_err(told)?;

    let Ok(()) = waiting.mark("surface");
    *showing = Some(waiting);

    Ok(())
}

fn away(
    screen: &mut Screen,
    shown: &mut Option<Drawn>,
    showing: &mut Option<Waiting>,
    down: &mut Option<usize>,
    held: &mut Held,
) -> Result<(), Never> {
    let Ok(()) = screen.hide();
    *shown = None;
    *showing = None;
    *down = None;
    *held = Held::default();

    Ok(())
}

struct Reading {
    claim: Claim,
    spans: Spans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quiet {
    Yes,
    No,
}

fn keeping(showing: Showing, reading: &mut Option<Reading>, complained: &mut Quiet) -> Result<(), Never> {
    match showing {
        Showing::No => *reading = None,
        Showing::Yes => match reading {
            Some(_) => {},
            None => {
                let Ok(taking) = taking(complained);

                *reading = taking;
            },
        },
    }

    Ok(())
}

fn taking(complained: &mut Quiet) -> Result<Option<Reading>, Never> {
    Ok(match Claim::of(&CONTROLLER) {
        Ok(claim) => {
            let Ok(holding) = claim.holding();

            for (which, path) in holding {
                let Ok(said) = which.said();

                eprintln!("virtual-keyboard: reading the {said} at {path}");
            }

            let spans = match claim.spans(claim::Which::Pad) {
                Ok(spans) => spans,

                Err(refused) => {
                    let Ok(said) = refused.said();

                    eprintln!("virtual-keyboard: {said}; the sticks will not walk");

                    Spans::new()
                },
            };
            *complained = Quiet::No;

            Some(Reading { claim, spans })
        },

        Err(refused) => {
            match *complained {
                Quiet::Yes => {},
                Quiet::No => {
                    let Ok(said) = refused.said();

                    eprintln!("virtual-keyboard: {said}, so it is touch only");

                    *complained = Quiet::Yes;
                },
            }

            None
        },
    })
}

fn from_controller(heard: &Heard, spans: &Spans, held: &mut Held, now: Instant) -> Result<Vec<Asked>, Never> {
    let mut out = Vec::new();

    for (which, event) in &heard.events {
        let kind = event.event_type();
        let Ok(said) = claim::said(*which, kind, event.code(), event.value());
        let axis = match kind {
            EventType::ABSOLUTE => Some((AbsoluteAxisCode(event.code()), event.value())),
            _ => None,
        };
        let Ok(wanted) = gamepad::wanted(said, axis, spans);
        let moving = wanted.is_none_or(|asked| {
            let Ok(repeats) = asked.repeats();

            repeats == Repeats::Held
        });

        match moving {
            true => {
                let Ok(went) = held.went(wanted, now);

                out.extend(went);
            },
            false => out.extend(wanted),
        }
    }

    Ok(out)
}

#[derive(Clone, Copy, PartialEq)]
struct Drawn {
    showing: Which,
    held: u8,
    pressed: Option<usize>,
    selected: Option<usize>,
    size: Option<(u32, u32)>,
    scale: i32,
}

fn orientation(config: &Config, shape: Shape) -> Result<(Vec<&str>, u32), Never> {
    let (given, fallback, height) = match shape {
        Shape::Landscape => {
            (&config.landscape_layers, LANDSCAPE_WALK, config.landscape_height)
        },
        Shape::Portrait => (&config.layers, WALK, config.height),
    };
    let asked: Vec<&str> = match given.is_empty() {
        true => fallback.to_vec(),
        false => given.iter().map(String::as_str).collect(),
    };
    Ok((asked, height))
}

enum Told {
    Show,
    Hide,
    Either,
}

fn listening() -> Result<std::os::fd::RawFd, std::io::Error> {
    // SAFETY: a mask on the stack, filled and applied by the calls that own it.
    unsafe {
        let mut mask: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, libc::SIGUSR1);
        libc::sigaddset(&mut mask, libc::SIGUSR2);
        libc::sigaddset(&mut mask, libc::SIGRTMIN());

        match libc::pthread_sigmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut()) != 0 {
            true => return Err(std::io::Error::last_os_error()),
            false => {},
        }

        let fd = libc::signalfd(-1, &mask, libc::SFD_CLOEXEC);

        match fd < 0 {
            true => Err(std::io::Error::last_os_error()),
            false => Ok(fd),
        }
    }
}

fn woken(from: std::os::fd::RawFd) -> Result<Option<Told>, Never> {
    // SAFETY: a struct the kernel fills, read whole or not at all.
    let said = unsafe {
        let mut said: libc::signalfd_siginfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::signalfd_siginfo>();
        let got = libc::read(from, std::ptr::from_mut(&mut said).cast(), size);

        let Ok(wanted) = fitted::<usize, isize>(size);

        match got != wanted {
            true => return Ok(None),
            false => {},
        }

        said
    };

    let Ok(signal) = fitted::<u32, i32>(said.ssi_signo);

    Ok(match signal {
        libc::SIGUSR1 => Some(Told::Hide),
        libc::SIGUSR2 => Some(Told::Show),
        _ => Some(Told::Either),
    })
}

fn said(why: Missing) -> Result<String, Never> {
    Ok(match why {
        Missing::Compositor(_) => "no compositor answered on WAYLAND_DISPLAY".to_string(),
        Missing::Global(what) => format!("the compositor has no {what}"),
        Missing::Gone(why) => format!("the compositor went away: {why}"),
        Missing::Hung => "the compositor closed the connection".to_string(),
        Missing::Memory(why) => format!("no memory for a frame: {why}"),
    })
}
