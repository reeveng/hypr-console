//! The on-screen keyboard.
//!
//! Started once with the session and kept for it. Most of that time there is
//! nothing of it on the screen: `--hidden` starts it away, and the controller
//! shows and hides it with a signal, because a keyboard that was started and
//! stopped would pay for a compositor connection, ten composed keymaps and a
//! font every time somebody wanted to type a word.
//!
//!     console-keyboard --landscape-layers landscape,thai,landscapespecial
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
//!
//! The loop is a `State` and its methods rather than one `while` body: what a
//! turn touches is a dozen things that change together, and written as locals
//! they nested five deep. `once_around` is the turn -- draw what changed, wait,
//! act on what woke it -- and the wait is the only place this program blocks.
//! An error out of that wait ends the run rather than being printed and gone
//! round again: what arrives there is the connection dying, and the next turn
//! would not block, so the keyboard would spin and say so forever.


use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::process::ExitCode;

use console_core_colour::spent::{beside, read};
use console_input_focus::{self as claim, CONTROLLER, Claim, Heard, Spans};
use evdev::{AbsoluteAxisCode, EventType};
use console_input_alphabets::{self as alphabets, Held as Holding};
use console_input_keyboard::config::{self, Config};
use console_input_keyboard::drawing::{Stride, Surface};
use console_input_keyboard::gamepad::{self, Asked, Held, Repeats};
use console_input_keyboard::layout::{Drops, Kind, Which, key, mods, named, of, placed, toward, under};
use console_input_keyboard::paint;
use console_input_keyboard::surface::{Gone, Missing, Poke, Screen, Showing};
use std::time::Instant;
use console_input_keyboard::typing::{After, Typist};
use console_response_times::{Wait, Waiting};

const NOWHERE: (i32, i32) = (0, 0);


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Landscape,
    Portrait,
}

fn landscape(room: Size<u32>) -> Result<Shape, Never> {
    Ok(match room.wide > room.tall {
        true => Shape::Landscape,
        false => Shape::Portrait,
    })
}

fn main() -> ExitCode {
    let Ok(mut waiting) = Waiting::on(Wait { who: "keyboard", what: "starting" });
    let argv: Vec<String> = std::env::args().collect();

    match argv.iter().any(|word| word == "--help" || word == "-h") {
        true => {
            println!("usage: console-keyboard [--hidden] [-H height] [-l layers] [--fn font]");
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
            eprintln!("console-keyboard: {why:?}");
            return ExitCode::FAILURE;
        },
    };

    let Ok(()) = waiting.mark("asked");

    match run(&config, waiting) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("console-keyboard: {why}");
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
            eprintln!("console-keyboard: no palette at {}: {why}", at.display());
            String::new()
        },
    };
    let Ok(palette) = read(&held);
    let Ok(missing) = console_input_keyboard::palette::missing(&palette);

    match missing.is_empty() {
        true => {},
        false => {
            eprintln!(
                "console-keyboard: the palette has no {}, so the keyboard keeps its own colour \
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
            eprintln!("console-keyboard: where this program is: {why}");

            std::path::PathBuf::from(console_input_keyboard::palette::VIRTUAL_KEYBOARD)
        },
    })
}

struct State<'a> {
    screen: Screen,
    signals: std::os::fd::RawFd,
    config: &'a Config,
    height: u32,
    typist: Typist,
    held: Held,
    down: Option<usize>,
    shown: Option<Drawn>,
    showing: Option<Waiting>,
    reading: Option<Reading>,
    complained: Quiet,
    worn: Which,
    selected: Option<usize>,
}

impl State<'_> {
    fn while_open(&mut self) -> Result<(), Missing> {
        while self.screen.closed() == Ok(Gone::No) {
            self.once_around()?;
        }

        Ok(())
    }

    fn once_around(&mut self) -> Result<(), Missing> {
        let Ok(showing_now) = self.screen.showing();
        let Ok(()) = keeping(showing_now, &mut self.reading, &mut self.complained);

        match self.typist.last_alphabet == self.worn {
            true => {},
            false => {
                self.worn = self.typist.last_alphabet;

                let Ok(()) = keeps_its_own(self.worn);
            }
        }

        self.drawing(showing_now)?;

        let spoke = self.waiting_on()?;
        let now = Instant::now();

        self.told(spoke)?;

        let Ok(()) = self.wants(now);
        let Ok(()) = self.pokes();

        Ok(())
    }

    fn drawing(&mut self, showing_now: Showing) -> Result<(), Missing> {
        let Ok(size) = self.screen.size();
        let Ok(scale) = self.screen.scale();
        let frame = Drawn {
            showing: self.typist.showing,
            held: self.typist.held,
            pressed: self.down,
            selected: self.selected,
            size,
            scale,
        };

        match showing_now == Showing::Yes && self.shown != Some(frame) {
            true => {
                let Ok(layout) = self.typist.layout();
                let modifiers = self.typist.held;
                let pressed = self.down;
                let selected = self.selected;
                let config = self.config;
                let Ok(next) = self.typist.next_language();
                let language = next.map(|which| {
                    let Ok(of) = of(which);
                    let Ok(written) = of.alphabet.written();

                    written
                });

                self.screen
                    .draw(|pixels, wide, tall, scale| {
                        let Ok(along) = fitted(wide.saturating_mul(4));
                        let Ok(down) = fitted(tall);

                        let onto = match Surface::new(
                            pixels,
                            Stride(along),
                            down,
                            f64::from(scale),
                        ) {
                            Ok(Some(onto)) => onto,
                            Ok(None) | Err(_) => return,
                        };

                        let across = f64::from(wide) / f64::from(scale);
                        let deep = f64::from(tall) / f64::from(scale);
                        let Ok(keys) = placed(layout, Size { wide: across, tall: deep });
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
                    })?;

                self.shown = Some(frame);

                match self.showing.take() {
                    Some(mut waiting) => {
                        let Ok(()) = waiting.mark("drawn");
                        let Ok(()) = waiting.done();
                    },
                    None => {},
                }
            }
            false => {},
        }

        Ok(())
    }

    fn waiting_on(&mut self) -> Result<u32, Missing> {
        let now = Instant::now();
        let mut watching: Vec<std::os::fd::RawFd> = vec![self.signals];

        match self.reading.as_ref() {
            Some(reading) => {
                let Ok(fds) = reading.claim.watching();

                watching.extend(fds);
            },
            None => {},
        }

        let Ok(until) = self.held.until(now);

        self.screen.wait_with(&watching, until)
    }

    fn told(&mut self, spoke: u32) -> Result<(), Missing> {
        match spoke & 1 != 0 {
            true => {
                let Ok(woken) = woken(self.signals);
                let Ok(showing_now) = self.screen.showing();

                match woken {
                    Some(Told::Show) => match showing_now {
                        Showing::No => self.onto_the_screen()?,
                        Showing::Yes => {},
                    },
                    Some(Told::Hide) => {
                        let Ok(()) = self.away();
                    },
                    Some(Told::Either) => match showing_now {
                        Showing::Yes => {
                            let Ok(()) = self.away();
                        },
                        Showing::No => self.onto_the_screen()?,
                    },
                    None => {},
                }
            }
            false => {},
        }

        Ok(())
    }

    fn wants(&mut self, now: Instant) -> Result<(), Never> {
        let mut wants: Vec<Asked> = Vec::new();

        match self.reading.as_mut() {
            Some(at_hand) => {
                let Ok(heard) = at_hand.claim.arrived();

                let Ok(asked) = from_controller(&heard, &at_hand.spans, &mut self.held, now);

                wants.extend(asked);

                match heard.gone.is_empty() {
                    true => {},
                    false => {
                        eprintln!("console-keyboard: the controller has gone; taking it again");
                        self.reading = None;
                    },
                }
            },
            None => {},
        }

        let Ok(due) = self.held.due(now);

        wants.extend(due);

        for want in wants {
            let Ok(showing_now) = self.screen.showing();

            match showing_now {
                Showing::No => continue,
                Showing::Yes => {},
            }

            let (wide, tall) = match self.screen.size() {
                Ok(Some((wide, tall))) => (wide, tall),
                Ok(None) | Err(_) => continue,
            };

            let Ok(layout) = self.typist.layout();
            let Ok(keys) = placed(layout, Size { wide: f64::from(wide), tall: f64::from(tall) });

            match want {
                Asked::Toggle => {
                    let Ok(()) = self.away();

                    self.selected = None;
                },
                _ if matches!(want.direction(), Ok(Some(_))) => {
                    let Ok(direction) = want.direction();

                    let (dx, dy) = match direction {
                        Some(both) => both,
                        None => NOWHERE,
                    };

                    let Ok(onto) = toward(&keys, self.selected, Point { across: dx, down: dy });

                    self.selected = onto;
                },
                Asked::Press => {
                    let at = match self.selected {
                        Some(at) => at,
                        None => {
                            let Ok(onto) = toward(&keys, None, Point { across: 0, down: 0 });

                            self.selected = onto;
                            continue;
                        }
                    };

                    let key = match layout.keys.get(at) {
                        Some(key) => key,
                        None => continue,
                    };

                    let Ok(after) = self.typist.pressed(key.kind, key.force, key.reset);

                    match after == After::Draw && !matches!(key.kind, Kind::Code { .. }) {
                        true => self.selected = None,
                        false => {},
                    }
                },
                Asked::Backspace => {
                    let Ok(()) = self.typist.tap(key::BACKSPACE);
                },
                Asked::Enter => {
                    let Ok(()) = self.typist.tap(key::ENTER);
                },
                Asked::Shift => {
                    let Ok(_) = self.typist.pressed(Kind::Mod(mods::SHIFT), mods::NONE, Drops::Nothing);
                },
                Asked::PreviousLanguage | Asked::NextLanguage => {
                    let Ok(mut waiting) =
                        Waiting::here(Wait { who: "keyboard", what: "language" });
                    let Ok(_) = self.typist.pressed(Kind::Language, mods::NONE, Drops::Nothing);
                    let Ok(()) = waiting.mark("keymap");
                    let Ok(()) = waiting.done();

                    self.selected = None;
                },
                Asked::Up | Asked::Down | Asked::Left | Asked::Right => {},
            }
        }

        Ok(())
    }

    fn pokes(&mut self) -> Result<(), Never> {
        let (wide, tall) = match self.screen.size() {
            Ok(Some((wide, tall))) => (wide, tall),
            Ok(None) | Err(_) => {
                let Ok(_) = self.screen.pokes();

                return Ok(());
            }
        };

        let Ok(pokes) = self.screen.pokes();

        for poke in pokes {
            let Ok(layout) = self.typist.layout();
            let Ok(keys) = placed(layout, Size { wide: f64::from(wide), tall: f64::from(tall) });

            match poke {
                Poke::Down { x, y } => {
                    let hit = match under(&keys, Point { across: x, down: y }) {
                        Ok(Some(hit)) => hit,
                        Ok(None) | Err(_) => continue,
                    };

                    let key = match layout.keys.get(hit.at) {
                        Some(key) => key,
                        None => continue,
                    };

                    self.down = Some(hit.at);
                    let kind = key.kind;
                    let force = key.force;
                    let reset = key.reset;
                    let changing = match matches!(kind, Kind::Language) {
                        true => {
                            let Ok(waiting) =
                                Waiting::here(Wait { who: "keyboard", what: "language" });

                            Some(waiting)
                        }
                        false => None,
                    };
                    let Ok(after) = self.typist.pressed(kind, force, reset);

                    match changing {
                        Some(mut waiting) => {
                            let Ok(()) = waiting.mark("keymap");
                            let Ok(()) = waiting.done();
                        },
                        None => {},
                    }

                    match after {
                        After::Draw => {
                            self.down = match kind {
                                Kind::Code { .. } => self.down,
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
                Poke::Up => self.down = None,
            }
        }

        Ok(())
    }

    fn onto_the_screen(&mut self) -> Result<(), Missing> {
        let Ok(mut waiting) = Waiting::here(Wait { who: "keyboard", what: "showing" });

        self.screen.show(self.height)?;

        let Ok(()) = waiting.mark("surface");

        self.showing = Some(waiting);

        Ok(())
    }

    fn away(&mut self) -> Result<(), Never> {
        let Ok(()) = self.screen.hide();

        self.shown = None;
        self.showing = None;
        self.down = None;
        self.held = Held::default();

        Ok(())
    }
}

#[derive(Debug)]
enum Unopened {
    NoKeymaps(console_input_keyboard::keymap::Error),
    NoLayer(Vec<String>),
    NoSignals(std::io::Error),
    NoTyping,
    Screen(Missing),
}

impl std::fmt::Display for Unopened {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unopened::NoKeymaps(why) => write!(
                to,
                "no keymaps: {why:?}. Is xkeyboard-config installed?"
            ),
            Unopened::NoLayer(asked) => write!(
                to,
                "no layer called any of {asked:?}. --list-layers says what there is"
            ),
            Unopened::NoSignals(why) => write!(to, "no signals: {why}"),
            Unopened::NoTyping => write!(
                to,
                "this compositor has no zwp_virtual_keyboard_v1, so nothing here could type"
            ),
            Unopened::Screen(why) => write!(to, "{why}"),
        }
    }
}

impl std::error::Error for Unopened {}

impl std::convert::From<Missing> for Unopened {
    fn from(why: Missing) -> Self {
        Unopened::Screen(why)
    }
}

fn run(config: &Config, mut waiting: Waiting) -> Result<(), Unopened> {
    let Ok(root) = console_input_keyboard::keymap::default_symbols_root();
    let alphabets = console_input_keyboard::keymap::available("evdev", &root)
        .map_err(Unopened::NoKeymaps)?;

    let Ok(()) = waiting.mark("keymaps");

    let Ok(shape) = landscape(Size { wide: u32::MAX, tall: config.height });
    let Ok((asked, height)) = orientation(config, shape);
    let Ok(opening) = left_on(shape);
    let walk: Vec<Which> = asked
        .iter()
        .filter_map(|name| {
            let Ok(named) = named(name.as_str());

            named
        })
        .collect();

    match walk.is_empty() {
        true => return Err(Unopened::NoLayer(asked)),
        false => {},
    }

    let signals = listening().map_err(Unopened::NoSignals)?;
    let mut screen = Screen::connect()?;

    let Ok(()) = waiting.mark("compositor");

    match config.hidden {
        true => {},
        false => screen.show(height)?,
    }

    let Ok(typing) = screen.typing();
    let typing = typing.ok_or(Unopened::NoTyping)?;
    let manager = typing.clone();
    let Ok(hand) = screen.hand();
    let Ok(seat) = screen.seat();
    let Ok(typist) = Typist::new(&manager, seat, &hand, alphabets, walk, opening, Instant::now());
    let worn = typist.last_alphabet;

    let Ok(()) = waiting.mark("typist");
    let Ok(()) = waiting.mark("pad");
    let Ok(()) = waiting.done();

    let mut state = State {
        screen,
        signals,
        config,
        height,
        typist,
        held: Held::default(),
        down: None,
        shown: None,
        showing: None,
        reading: None,
        complained: Quiet::No,
        worn,
        selected: None,
    };

    state.while_open().map_err(Unopened::Screen)
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

                eprintln!("console-keyboard: reading the {said} at {path}");
            }

            let spans = match claim.spans(claim::Which::Pad) {
                Ok(spans) => spans,

                Err(refused) => {
                    let Ok(said) = refused.said();

                    eprintln!("console-keyboard: {said}; the sticks will not walk");

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

                    eprintln!("console-keyboard: {said}, so it is touch only");

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

fn left_on(shape: Shape) -> Result<Option<Which>, Never> {
    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => return Ok(None),
    };

    let Ok(alphabet) = alphabets::wearing::read(&home, alphabets::wearing::SCREEN);

    let arrangement = match shape {
        Shape::Landscape => alphabet.across,
        Shape::Portrait => alphabet.upright,
    };

    named(arrangement)
}

fn keeps_its_own(showing: Which) -> Result<(), Never> {
    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => return Ok(()),
    };

    let Ok(of) = of(showing);
    let named = alphabets::EVERY.iter().find(|alphabet| {
        alphabet.across == of.name || alphabet.upright == of.name
    });

    let alphabet = match named {
        Some(alphabet) => alphabet,
        None => return Ok(()),
    };

    match alphabets::wearing::remember(&home, alphabets::wearing::SCREEN, alphabet) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-keyboard: {fault}"),
    }

    Ok(())
}

fn orientation(config: &Config, shape: Shape) -> Result<(Vec<String>, u32), Never> {
    let (given, holding, height) = match shape {
        Shape::Landscape => {
            (&config.landscape_layers, Holding::Across, config.landscape_height)
        },
        Shape::Portrait => (&config.layers, Holding::Upright, config.height),
    };

    let asked = match given.is_empty() {
        true => {
            let Ok(chosen) = alphabets::chosen();
            let Ok(walk) = alphabets::walk(&chosen, holding);

            walk
        },
        false => given.clone(),
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

