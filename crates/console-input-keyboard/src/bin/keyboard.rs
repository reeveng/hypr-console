//! The on-screen keyboard.
//!
//! Started once with the session and kept for it. Most of that time there is
//! nothing of it on the screen: `--hidden` starts it away, and the controller
//! shows and hides it with a word down a socket, because a keyboard that was started and
//! stopped would pay for a compositor connection, ten composed keymaps and a
//! font every time someone wanted to type a word.
//!
//!     console-keyboard --landscape-layers landscape,thai,landscapespecial
//!
//! The unit runs this, and `keyboard-toggle` sends the word. It reads the
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
//! kept the colors in `configuration::Scheme::default` and drew a keyboard in green
//! and red that no check could believe. `console_core_color::palette::beside` is
//! where that is decided, and on the device the two answers are one file.
//!
//! The loop is a `State` and its methods rather than one `while` body: what a
//! turn touches is a dozen things that change together, and written as locals
//! they nested five deep. `once_around` is the turn -- draw what changed, wait,
//! act on what woke it -- and the wait is the only place this program blocks.
//! An error out of that wait ends the run rather than being printed and gone
//! round again: what arrives there is the connection dying, and the next turn
//! would not block, so the keyboard would spin and say so forever.
//!
//! A turn also asks what alphabet this keyboard should be wearing, which is a
//! few bytes under the state directory that `language-switch` writes when the
//! board someone is typing on moves. KeyboardCommand here rather than waited for: the
//! keyboard already hears three words and a fourth would be a fourth program
//! to send it, and a turn happens when something happened -- a press, a frame,
//! a wake -- so this is one small read on a loop that is not spinning. What it
//! costs is that a switch made while the keyboard is up and untouched lands on
//! the next thing that wakes it rather than at once.


use console_core_arguments::{Flag, Operands, Presence, Takes};
use console_core_geometry::{Point, Size};
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixDatagram;
use std::process::ExitCode;

use console_core_color::palette::{beside, read};
use console_input_focus::{self as claim, CONTROLLER, Claim, Received, Spans};
use console_input_event_devices::{AbsoluteAxisCode, EventType};
use console_input_alphabets::{self as alphabets, Orientation as Holding};
use console_input_keyboard::configuration::{self, Configuration};
use console_input_keyboard::drawing::{Stride, Surface};
use console_input_keyboard::gamepad::{self, KeyboardCommand, PendingRepeat, RepeatMode};
use console_input_keyboard::layout::{Drops, Kind, LayoutKind, key, modifiers, find_layout, of, layout_keys, toward, under};
use console_input_keyboard::paint;
use console_input_keyboard::remote::{self, Command};
use console_input_keyboard::surface::{Closed, SurfaceError, PointerEvent, Screen, Showing};
use std::time::Instant;
use console_input_keyboard::typing::{After, Typist};
use console_response_times::{Wait, Waiting};

const NOWHERE: (i32, i32) = (0, 0);

const LIST_LAYERS: Flag = Flag { spelling: "--list-layers", takes: Takes::None, about: "the layers this keyboard can show, one to a line" };

const LANDSCAPE_LAYERS: Flag = Flag {
    spelling: "--landscape-layers",
    takes: Takes::Value("LAYERS"),
    about: "the layers to walk while the screen is wider than it is tall, separated by commas",
};

const ON_THE_SCREEN: console_core_arguments::Command = console_core_arguments::Command {
    name: "console-keyboard",
    about: "the keyboard on the screen, started away and shown by keyboard-toggle",
    flags: &[LIST_LAYERS, LANDSCAPE_LAYERS],
    operands: Operands::None,
};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Landscape,
    Portrait,
}

fn landscape(room: Size<u32>) -> Result<Shape, Never> {
    Ok(match room.width > room.height {
        true => Shape::Landscape,
        false => Shape::Portrait,
    })
}

fn main() -> ExitCode {
    let Ok(mut waiting) = Waiting::on(Wait { who: "keyboard", what: "starting" });
    let words: Vec<String> = std::env::args().skip(1).collect();
    let line = match console_core_arguments::read(&ON_THE_SCREEN, &words) {
        Ok(line) => line,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };
    let Ok(listing) = line.presence(LIST_LAYERS);

    match listing {
        Presence::Present => {
            for which in LayoutKind::ALL {
                let Ok(of) = of(which);

                println!("{}", of.name);
            }

            return ExitCode::SUCCESS;
        }
        Presence::Absent => {},
    }

    let Ok(landscape) = line.value(LANDSCAPE_LAYERS);
    let passed: Vec<String> = landscape.into_iter().flat_map(|layers| [LANDSCAPE_LAYERS.spelling.to_string(), layers.to_string()]).collect();
    let Ok(flags) = dressed(&passed);

    let Ok(()) = waiting.mark("palette");

    let configuration = match configuration::from_environment(&flags) {
        Ok(configuration) => configuration,
        Err(why) => {
            eprintln!("console-keyboard: {why:?}");
            return ExitCode::FAILURE;
        },
    };

    let Ok(()) = waiting.mark("asked");

    match run(&configuration, waiting) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("console-keyboard: {why}");
            ExitCode::FAILURE
        },
    }
}

fn dressed(passed: &[String]) -> Result<Vec<String>, Never> {
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
    let Ok(missing) = console_input_keyboard::palette::missing_colors(&palette);

    match missing.is_empty() {
        true => {},
        false => {
            eprintln!(
                "console-keyboard: the palette has no {}, so the keyboard keeps its own color \
                 for those",
                missing.join(", ")
            );
        }
    }

    console_input_keyboard::palette::arguments(&palette, passed)
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
    told: UnixDatagram,
    configuration: &'a Configuration,
    height: u32,
    typist: Typist,
    held: PendingRepeat,
    pressed: Option<u32>,
    shown: Option<KeyboardState>,
    showing: Option<Waiting>,
    reading: Option<Reading>,
    complained: Logged,
    worn: LayoutKind,
    selected: Option<u32>,
}

impl State<'_> {
    fn while_open(&mut self) -> Result<(), SurfaceError> {
        let ended = console_core_iteration::iterate(self, |state| {
            Ok(match state.screen.closed() == Ok(Closed::No) {
                true => match state.once_around() {
                    Ok(()) => Step::Again(state),
                    Err(fault) => Step::Halt(Err(fault)),
                },
                false => Step::Halt(Ok(())),
            })
        });

        match ended {
            Ok(ended) => ended,
            Err(_endless) => Ok(()),
        }
    }

    fn once_around(&mut self) -> Result<(), SurfaceError> {
        let Ok(showing_now) = self.screen.visibility();
        let Ok(()) = sync_reading(showing_now, &mut self.reading, &mut self.complained);

        match self.typist.last_alphabet == self.worn {
            true => {},
            false => {
                self.worn = self.typist.last_alphabet;

                let Ok(()) = keeps_its_own(self.worn);
            }
        }

        let Ok(()) = self.follows();

        self.drawing(showing_now)?;

        let events = self.waiting_on()?;
        let now = Instant::now();

        self.handle_commands(events)?;

        let Ok(()) = self.wants(now);
        let Ok(()) = self.pointer_events();

        Ok(())
    }

    fn follows(&mut self) -> Result<(), Never> {
        let Ok(shape) = landscape(Size { width: u32::MAX, height: self.configuration.height });
        let Ok(worn) = left_on(shape);

        let wanted = match worn {
            Some(wanted) => wanted,
            None => return Ok(()),
        };

        match wanted == self.worn {
            true => {},
            false => {
                let Ok(()) = self.typist.go(wanted);

                self.worn = wanted;
                self.selected = None;
            }
        }

        Ok(())
    }

    fn drawing(&mut self, showing_now: Showing) -> Result<(), SurfaceError> {
        let Ok(size) = self.screen.size();
        let Ok(scale) = self.screen.scale();
        let frame = KeyboardState {
            showing: self.typist.showing,
            held: self.typist.held,
            pressed: self.pressed,
            selected: self.selected,
            size,
            scale,
        };

        match showing_now == Showing::Yes && self.shown != Some(frame) {
            true => {
                let Ok(layout) = self.typist.layout();
                let modifiers = self.typist.held;
                let pressed = self.pressed;
                let selected = self.selected;
                let configuration = self.configuration;
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
                        let Ok(keys) = layout_keys(layout, Size { width: across, height: deep });
                        let Ok(()) = paint::keyboard(&onto, &paint::Look {
                            configuration,
                            layout,
                            keys: &keys,
                            pressed,
                            held: modifiers,
                            selected,
                            language,
                            width: across,
                            height: deep,
                        });
                    })?;

                self.shown = Some(frame);

                match self.showing.take() {
                    Some(mut waiting) => {
                        let Ok(()) = waiting.mark("drawn");
                        let Ok(()) = waiting.finish();
                    },
                    None => {},
                }
            }
            false => {},
        }

        Ok(())
    }

    fn waiting_on(&mut self) -> Result<u32, SurfaceError> {
        let now = Instant::now();
        let mut watching: Vec<BorrowedFd<'_>> = vec![self.told.as_fd()];

        match self.reading.as_ref() {
            Some(reading) => {
                let Ok(fds) = reading.claim.descriptors();

                watching.extend(fds);
            },
            None => {},
        }

        let Ok(until) = self.held.until(now);

        self.screen.wait_with(&watching, until)
    }

    fn handle_commands(&mut self, events: u32) -> Result<(), SurfaceError> {
        match events & 1 != 0 {
            true => {
                let Ok(heard) = receive_commands(&self.told);

                for asked in heard {
                    let Ok(showing_now) = self.screen.visibility();

                    match (asked, showing_now) {
                        (Command::Show, Showing::No) | (Command::Toggle, Showing::No) => self.onto_the_screen()?,
                        (Command::Show, Showing::Yes) | (Command::Hide, Showing::No) => {},
                        (Command::Hide, Showing::Yes) | (Command::Toggle, Showing::Yes) => {
                            let Ok(()) = self.away();
                        },
                    }
                }
            }
            false => {},
        }

        Ok(())
    }

    fn wants(&mut self, now: Instant) -> Result<(), Never> {
        let mut wants: Vec<KeyboardCommand> = Vec::new();

        match self.reading.as_mut() {
            Some(at_hand) => {
                let Ok(heard) = at_hand.claim.receive();

                let Ok(asked) = from_controller(&heard, &at_hand.spans, &mut self.held, now);

                wants.extend(asked);

                match heard.unplugged.is_empty() {
                    true => {},
                    false => {
                        eprintln!("console-keyboard: the controller is missing; taking it again");
                        self.reading = None;
                    },
                }
            },
            None => {},
        }

        let Ok(due) = self.held.due(now);

        wants.extend(due);

        for want in wants {
            let Ok(showing_now) = self.screen.visibility();

            match showing_now {
                Showing::No => continue,
                Showing::Yes => {},
            }

            let (wide, tall) = match self.screen.size() {
                Ok(Some((wide, tall))) => (wide, tall),
                Ok(None) | Err(_) => continue,
            };

            let Ok(layout) = self.typist.layout();
            let Ok(keys) = layout_keys(layout, Size { width: f64::from(wide), height: f64::from(tall) });

            match want {
                KeyboardCommand::Toggle => {
                    let Ok(()) = self.away();

                    self.selected = None;
                },
                KeyboardCommand::Up | KeyboardCommand::Down | KeyboardCommand::Left | KeyboardCommand::Right => {
                    let Ok(direction) = want.direction();

                    let (across, down) = match direction {
                        Some(both) => both,
                        None => NOWHERE,
                    };

                    let Ok(onto) = toward(&keys, self.selected, Point { x: across, y: down });

                    self.selected = onto;
                },
                KeyboardCommand::Select => {
                    let at = match self.selected {
                        Some(at) => at,
                        None => {
                            let Ok(onto) = toward(&keys, None, Point { x: 0, y: 0 });

                            self.selected = onto;
                            continue;
                        }
                    };

                    let Ok(at) = index(at);

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
                KeyboardCommand::Backspace => {
                    let Ok(()) = self.typist.tap(key::BACKSPACE);
                },
                KeyboardCommand::Enter => {
                    let Ok(()) = self.typist.tap(key::ENTER);
                },
                KeyboardCommand::Shift => {
                    let Ok(_) = self.typist.pressed(Kind::Mod(modifiers::SHIFT), modifiers::NONE, Drops::None);
                },
                KeyboardCommand::PreviousLanguage | KeyboardCommand::NextLanguage => {
                    let Ok(mut waiting) =
                        Waiting::here(Wait { who: "keyboard", what: "language" });
                    let Ok(_) = self.typist.pressed(Kind::Language, modifiers::NONE, Drops::None);
                    let Ok(()) = waiting.mark("keymap");
                    let Ok(()) = waiting.finish();

                    self.selected = None;
                },
            }
        }

        Ok(())
    }

    fn pointer_events(&mut self) -> Result<(), Never> {
        let (wide, tall) = match self.screen.size() {
            Ok(Some((wide, tall))) => (wide, tall),
            Ok(None) | Err(_) => {
                let Ok(_) = self.screen.pointer_events();

                return Ok(());
            }
        };

        let Ok(pointer_events) = self.screen.pointer_events();

        for event in pointer_events {
            let Ok(layout) = self.typist.layout();
            let Ok(keys) = layout_keys(layout, Size { width: f64::from(wide), height: f64::from(tall) });

            match event {
                PointerEvent::Down { x, y } => {
                    let hit = match under(&keys, Point { x, y }) {
                        Ok(Some(hit)) => hit,
                        Ok(None) | Err(_) => continue,
                    };

                    let Ok(at) = index(hit.at);

                    let key = match layout.keys.get(at) {
                        Some(key) => key,
                        None => continue,
                    };

                    self.pressed = Some(hit.at);
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
                            let Ok(()) = waiting.finish();
                        },
                        None => {},
                    }

                    match after {
                        After::Draw => {
                            self.pressed = match kind {
                                Kind::Code { .. } => self.pressed,
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
                PointerEvent::Moved { .. } => {},
                PointerEvent::Up => self.pressed = None,
            }
        }

        Ok(())
    }

    fn onto_the_screen(&mut self) -> Result<(), SurfaceError> {
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
        self.pressed = None;
        self.held = PendingRepeat::default();

        Ok(())
    }
}

#[derive(Debug)]
enum Unopened {
    NoKeymaps(console_input_keyboard::keymap::Error),
    NoLayer(Vec<String>),
    Sessionless,
    Deaf(std::path::PathBuf, std::io::Error),
    NoTyping,
    Screen(SurfaceError),
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
            Unopened::Sessionless => write!(
                to,
                "XDG_RUNTIME_DIR: nothing says where to listen for being shown or hidden"
            ),
            Unopened::Deaf(at, why) => write!(to, "{}: nothing can ask it to show or hide: {why}", at.display()),
            Unopened::NoTyping => write!(
                to,
                "this compositor has no zwp_virtual_keyboard_v1, so nothing here could type"
            ),
            Unopened::Screen(why) => write!(to, "{why}"),
        }
    }
}

impl std::error::Error for Unopened {}

impl std::convert::From<SurfaceError> for Unopened {
    fn from(why: SurfaceError) -> Self {
        Unopened::Screen(why)
    }
}

fn run(configuration: &Configuration, mut waiting: Waiting) -> Result<(), Unopened> {
    let Ok(root) = console_input_keyboard::keymap::default_symbols_root();
    let alphabets = console_input_keyboard::keymap::available("evdev", &root)
        .map_err(Unopened::NoKeymaps)?;

    let Ok(()) = waiting.mark("keymaps");

    let Ok(shape) = landscape(Size { width: u32::MAX, height: configuration.height });
    let Ok((asked, height)) = orientation(configuration, shape);
    let Ok(opening) = left_on(shape);
    let walk: Vec<LayoutKind> = asked
        .iter()
        .filter_map(|name| {
            let Ok(named) = find_layout(name.as_str());

            named
        })
        .collect();

    match walk.is_empty() {
        true => return Err(Unopened::NoLayer(asked)),
        false => {},
    }

    let told = bind_socket()?;
    let mut screen = Screen::connect()?;

    let Ok(()) = waiting.mark("compositor");

    match configuration.hidden {
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
    let Ok(()) = waiting.finish();

    let mut state = State {
        screen,
        told,
        configuration,
        height,
        typist,
        held: PendingRepeat::default(),
        pressed: None,
        shown: None,
        showing: None,
        reading: None,
        complained: Logged::No,
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
enum Logged {
    Yes,
    No,
}

fn sync_reading(showing: Showing, reading: &mut Option<Reading>, complained: &mut Logged) -> Result<(), Never> {
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

fn taking(complained: &mut Logged) -> Result<Option<Reading>, Never> {
    Ok(match Claim::of(&CONTROLLER) {
        Ok(claim) => {
            let Ok(holding) = claim.devices();

            for (which, path) in holding {
                let Ok(said) = which.said();

                eprintln!("console-keyboard: reading the {said} at {path}");
            }

            let spans = match claim.spans(claim::DeviceKind::Pad) {
                Ok(spans) => spans,

                Err(refused) => {
                    let Ok(said) = refused.message();

                    eprintln!("console-keyboard: {said}; the sticks will not walk");

                    Spans::new()
                },
            };
            *complained = Logged::No;

            Some(Reading { claim, spans })
        },

        Err(refused) => {
            match *complained {
                Logged::Yes => {},
                Logged::No => {
                    let Ok(said) = refused.message();

                    eprintln!("console-keyboard: {said}, so it is touch only");

                    *complained = Logged::Yes;
                },
            }

            None
        },
    })
}

fn from_controller(heard: &Received, spans: &Spans, held: &mut PendingRepeat, now: Instant) -> Result<Vec<KeyboardCommand>, Never> {
    let mut out = Vec::new();

    for (which, event) in &heard.events {
        let kind = event.kind;
        let Ok(named) = claim::translate(*which, kind, event.code, event.value);
        let axis = match kind {
            EventType::ABSOLUTE => Some((AbsoluteAxisCode(event.code), event.value)),
            _ => None,
        };
        let Ok(command) = gamepad::command_for(named, axis, spans);
        let moving = command.is_none_or(|command| {
            let Ok(mode) = command.repeat_mode();

            mode == RepeatMode::Repeating
        });

        match moving {
            true => {
                let Ok(pressed) = held.pressed(command, now);

                out.extend(pressed);
            },
            false => out.extend(command),
        }
    }

    Ok(out)
}

#[derive(Clone, Copy, PartialEq)]
struct KeyboardState {
    showing: LayoutKind,
    held: u8,
    pressed: Option<u32>,
    selected: Option<u32>,
    size: Option<(u32, u32)>,
    scale: i32,
}

fn left_on(shape: Shape) -> Result<Option<LayoutKind>, Never> {
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

    find_layout(arrangement)
}

fn keeps_its_own(showing: LayoutKind) -> Result<(), Never> {
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

fn orientation(configuration: &Configuration, shape: Shape) -> Result<(Vec<String>, u32), Never> {
    let (given, holding, height) = match shape {
        Shape::Landscape => {
            (&configuration.landscape_layers, Holding::Across, configuration.landscape_height)
        },
        Shape::Portrait => (&configuration.layers, Holding::Upright, configuration.height),
    };

    let asked = match given.is_empty() {
        true => {
            let Ok(chosen) = alphabets::current();
            let Ok(walk) = alphabets::walk(&chosen, holding);

            walk
        },
        false => given.clone(),
    };

    Ok((asked, height))
}

fn bind_socket() -> Result<UnixDatagram, Unopened> {
    let Ok(at) = remote::socket();
    let at = at.ok_or(Unopened::Sessionless)?;

    match at.parent() {
        Some(above) => std::fs::create_dir_all(above).map_err(|why| Unopened::Deaf(above.to_path_buf(), why))?,
        None => {},
    }

    match std::fs::remove_file(&at) {
        Ok(()) => {},
        Err(_no_keyboard_was_here_before) => {},
    }

    let told = UnixDatagram::bind(&at).map_err(|why| Unopened::Deaf(at.clone(), why))?;

    told.set_nonblocking(true).map_err(|why| Unopened::Deaf(at, why))?;

    Ok(told)
}

fn receive_commands(told: &UnixDatagram) -> Result<Vec<Command>, Never> {
    let said = std::iter::from_fn(|| {
        let mut said = [0_u8; 16];

        match told.recv(&mut said) {
            Ok(got) => Some(match said.get(..got).map(std::str::from_utf8) {
                Some(Ok(word)) => Some(word.to_string()),
                Some(Err(_not_a_word)) => None,
                None => None,
            }),
            Err(_nothing_more_was_said) => None,
        }
    });

    Ok(said
        .filter_map(|word| match word {
            Some(word) => {
                let Ok(asked) = Command::read(&word);

                asked
            },
            None => {
                eprintln!("console-keyboard: something was said that is not a word");

                None
            },
        })
        .collect())
}

