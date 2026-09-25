//! The home screen, drawn on the wallpaper.
//!
//! This desktop opened into nothing. A wallpaper and a bar, and every
//! application behind a button someone had to know about first -- which is
//! the one thing a phone, a console and a laptop all decline to do. So the
//! applications are on the screen: panes of them over the wallpaper -- as
//! many as what is on them needs -- walked with the d-pad, opened with A, and
//! arranged with Y.
//!
//! ## It draws itself
//!
//! There was a toolkit under this: a window, a grid of boxes, a stylesheet
//! with the measurements written into it on every redraw, and four gesture
//! controllers per square. None of it decided anything. What fits was already
//! `console_home_screen::shape`, what a press means was already `moved` and
//! `touched`, and what colour a thing is was already the palette -- so the
//! toolkit's whole contribution was a second copy of the answers, in widgets,
//! and a dependency this desktop carries on every machine for one program.
//!
//! What is here instead is a layer surface of our own, a list of shapes, and a
//! loop that waits on everything at once. `console_home_screen::shape::laid`
//! says where a square goes, this puts a plate, a picture and a name there,
//! and `console_draw_painting` is the only thing in the tree that has heard of
//! cairo. The stylesheet is gone and what it said is [`Wears`], which is the
//! same sentences in a type.
//!
//! **A plate is a wash, and a wash is not a shape.** `alpha(@night, 0.34)` is
//! what the squares stood on, and a colour in this tree is an `Oklch` with no
//! alpha in it: a shape carrying one would be a field on `Panel` and every
//! panel in the tree written again. So the wash is mixed here instead --
//! `console_core_color::over`, the ink over the ground, at the share the
//! stylesheet asked for -- which is the same colour anywhere the wallpaper is
//! near the ground it was chosen against and a flatter one where it is not.
//! What would give it back is a shape that carries how much of what is behind
//! it comes through, and that is one decision for every surface here rather
//! than this one.
//!
//! ## It is the desktop, not a thing on top of it
//!
//! `console_onscreen::SYSTEM_SURFACES` names it, so the shoulders still
//! change workspace, the left Legion button still leaves for Steam, and the
//! paddles still do what they do everywhere. The d-pad is its own from the
//! moment it is drawn; A and Y become its own once the d-pad has woken it.
//! `Mode::Home` and `Mode::Standing` are where that is decided. `Under::AWindow`
//! is the same sentence to the compositor: the surface is on the bottom layer,
//! over the wallpaper and under everything a person opens.
//!
//! ## It is told what the pad did, and holds no keyboard
//!
//! Every other surface here hears the pad as keys, and this one cannot. It is
//! drawn under everything and never in front, so the only way it could take
//! the keyboard was to ask for it exclusively -- which Hyprland answers by
//! handing it every pointer and every touch on the screen, wherever they land,
//! because that is what the thing it was written for needs. Stored that way this
//! swallowed every tap on the bar: the launcher, the keyboard, the music and
//! the sound opened nothing, and the bar looked broken while it was never
//! being touched. So the daemon says what the pad did, over
//! `console_onscreen::homeward`, and this holds nothing.
//!
//! ## And it starts asleep
//!
//! A highlight is a claim on A. Drawn from the moment the surface was, it
//! claimed A for every minute the machine was on, and a thumb on the touchpad
//! had a pointer with nothing to press. So nothing is highlighted until the
//! d-pad says so, and the first press of it raises the highlight where it was
//! rather than moving it -- what is under a highlight has to be seen before it
//! can be meant.
//!
//! ## And the pointer moves the highlight, without waking anything
//!
//! There is a pointer on this machine -- the touchpad's, and the left
//! stick's -- and it walked over these squares without any of them answering,
//! which is the one arrangement nothing else on the desktop has. So a square
//! under the pointer stands up exactly as the d-pad leaves it.
//!
//! It costs nothing to say so. Asleep, A is the click, and the click lands on
//! whatever the pointer is over, which is the square that is standing up: the
//! highlight is already what A does, and pointing at one does not have to
//! claim A to be true. So the pointer moves the highlight and never wakes
//! anything, and the d-pad goes on owning what "awake" means here. What it
//! does change is the press above: a highlight the pointer has already raised
//! has already been seen, so the d-pad's first press moves it like any other
//! rather than raising what is up.
//!
//! The highlight goes when the pointer leaves the squares, because a highlight
//! with nothing pointing at it is an offer no one is making: by then the
//! pointer is over the bar, and A is the bar's. A pointer that has left the
//! surface altogether says so once, as `PointerEvent::Left`, because a surface
//! hears nothing at all about a pointer that is somewhere else.
//!
//! ## And it goes away when there is something to look at
//!
//! A window on the workspace is what someone is doing, and the home screen is
//! what they do it from. So the surface is put away while a window is up --
//! which is also what keeps the wallpaper's own reading of "is anything in
//! front of me" true, and what keeps A a click while a game is on the screen.
//!
//! ## Swiping
//!
//! The panes are swiped with a finger on the surface itself, which wants
//! nothing of the compositor. Hyprland's own workspace swipe is the touchpad's,
//! and a plugin -- hyprgrass -- is what a gesture *over someone else's window*
//! would need. Neither is this: the finger is on the home screen, and the home
//! screen is the thing that reads it.
//!
//! Which means the same finger is on a square, and a swipe and a tap both end
//! with it coming up. `console_home_screen::touched` is what separates them, so
//! the flick that moves the panes is not also a press of whatever it started
//! on, and `flicked` is which way it went. A toolkit measured that in pixels a
//! second and this measures it in pixels: a thumb dragged the width of two
//! squares meant the next pane whether it was quick about it or not, and the
//! speed of it was a threshold nobody could have named on purpose.
//!
//! ## A bare square is not tapped into the picker
//!
//! Most of the home screen is empty most of the time, and a flick that stops
//! short of `DRIFT` is a tap -- so the finger that meant to change panes and
//! did not travel far enough put the whole list of applications on the screen
//! instead, from a square no one was aiming at. The same square answers the
//! d-pad, where it cannot happen: the highlight had to be walked there first,
//! and A on it is a sentence someone finished.
//!
//! So who is asking decides it. `console_home_screen::on_a_bare_square` is
//! that question and `Reached` is the answer: a button chooses, a finger
//! waits. The finger still has the picker -- holding a bare square is already
//! `lift`, and `lift` on a square with nothing to pick up is `place` -- so
//! nothing is taken away, it is moved onto the gesture that cannot be arrived
//! at by accident. A square that has something on it is unchanged either way:
//! a tap opens it, which is what a tap on a thing has always meant.
//!
//! A hold is a hold here too, and nothing else on the loop is waiting on a
//! clock: a finger that has been down [`HELD`] long and has not travelled is
//! the lift, and the wait for it is the only thing that ever gives the loop a
//! deadline instead of letting it sleep until something happens.
//!
//! ## And what it is carrying is said out loud
//!
//! Nothing outside this process could see a square in the hand: the
//! arrangement is only written when the square is put back down, and what is
//! held in between is not a window, a layer or a file. So
//! `console_onscreen::carrying` is written wherever that changes, and the
//! machine can be asked whether someone is holding an application instead of
//! being photographed to find out.

use std::collections::BTreeMap;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_applications::entry::Application;
use console_applications::found;
use console_compositor::events::CompositorEvent;
use console_core_atomic_writes::Stored;
use console_core_color::palette::{self, PaletteError, WearingError};
use console_core_color::{Ground, HexColor, Oklch, Rgba, over};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32};
use console_events::again::Worth;
use console_program_contract::Topic;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use console_core_shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight};
use console_draw_painting::{Frame, Run};
use console_draw_surface::{
    Anchor, Closed, Keyboard, Margin, PointerEvent, Room, Surface, Under, Wanted,
};
use console_home_screen::shape::{self, Laid, Shape as Grid};
use console_home_screen::{
    Bare, Flick, LongPress, HomeScreen, Moved, On, Reached, Spot, Touch, Way, flicked, held, moved, nudged,
    on_a_bare_square, paned, touched,
};
use console_onscreen::{Woken, Hand, Over, PadInput, over_the_desktop};
use console_panel::pictures::Side;

const NAMESPACE: &str = "console-home";


const FONT: &str = "Noto Sans";

const A_DOT: &str = "\u{25cf}";

const AND_SO_ON: char = '\u{2026}';

const WIDE_ENOUGH: u32 = 4096;

const WHATEVER_THE_SCREEN_IS: Size<u32> = Size { width: 0, height: 0 };

const SOON: Duration = Duration::from_millis(250);

const A_MOMENT: Duration = Duration::from_millis(10);

const SAID_AT_ONCE: u32 = 64;

const A_MOUTHFUL: u32 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holds {
    AWindow,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Woke {
    Already,
    Just,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Put {
    Back,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pointer {
    OnASquare,
    Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shows {
    AHighlight,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carried {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waiting {
    ForAPicture,
    None,
}

struct Carrying {
    name: String,
    from: Spot,
}

struct TouchState {
    from: (f64, f64),
    at: (f64, f64),
    since: Instant,
    on: Option<Spot>,
    held: LongPress,
}

type Named = BTreeMap<String, (Application, String)>;

struct Screen {
    here: Spot,
    home: HomeScreen,
    apps: Named,
    carrying: Option<Carrying>,
    woken: Woken,
    pointer: Pointer,
    seen: Option<(Spot, (f64, f64))>,
    settled: Option<Showing>,
    grid: Grid,
    finger: Option<TouchState>,
    labels: BTreeMap<(String, u32), MeasuredText>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MeasuredText {
    said: String,
    width: u32,
}

struct Wears {
    plate: Oklch,
    standing: Oklch,
    bare: Oklch,
    edge: Oklch,
    carried: Oklch,
    held: Oklch,
    text: Oklch,
    dot: Oklch,
    here: Oklch,
}

struct Drawing {
    shapes: Vec<Shape>,
    touching: Vec<(Spot, Panel)>,
    waiting: Waiting,
}

impl Screen {
    fn new() -> Result<Screen, Never> {
        let grid = asked_shape()?;

        Ok(Screen {
            here: Spot::FIRST,
            home: HomeScreen::default(),
            apps: BTreeMap::new(),
            carrying: None,
            woken: Woken::No,
            pointer: Pointer::Elsewhere,
            seen: None,
            settled: None,
            grid,
            finger: None,
            labels: BTreeMap::new(),
        })
    }

    fn told(&mut self, said: PadInput) -> Result<(), Never> {
        let way = match said {
            PadInput::Up => Some(Way::Up),
            PadInput::Down => Some(Way::Down),
            PadInput::Left => Some(Way::Left),
            PadInput::Right => Some(Way::Right),
            PadInput::Pressed
            | PadInput::More
            | PadInput::Back
            | PadInput::Again
            | PadInput::Payload
            | PadInput::Off => None,
        };

        match way {
            Some(way) => {
                let woke = self.wakes()?;

                match (woke, self.pointer) {
                    (Woke::Already, _) | (Woke::Just, Pointer::OnASquare) => {
                        let panes = self.panes()?;

                        let moved = moved(self.here, way, panes, self.grid)?;

                        self.here = moved;
                    },
                    (Woke::Just, Pointer::Elsewhere) => {},
                }

                return Ok(());
            }
            None => {},
        }

        match said {
            PadInput::Pressed => self.press(Reached::ByButton),
            PadInput::More => self.ways(),
            PadInput::Back => {
                let put = self.put_back()?;

                match put {
                    Put::Back => Ok(()),
                    Put::None => self.sleeps(),
                }
            }
            PadInput::Again => self.reshaped(),
            PadInput::Payload => self.lift(),
            PadInput::Off => self.take_off(),
            PadInput::Up | PadInput::Down | PadInput::Left | PadInput::Right => Ok(()),
        }
    }

    fn wakes(&mut self) -> Result<Woke, Never> {
        match self.woken {
            Woken::Yes => return Ok(Woke::Already),
            Woken::No => {},
        }

        self.woken = Woken::Yes;

        match console_onscreen::waking(Woken::Yes) {
            Ok(()) => {},
            Err(fault) => eprintln!("console-home: no one was told it is awake: {fault}"),
        }

        Ok(Woke::Just)
    }

    fn sleeps(&mut self) -> Result<(), Never> {
        match self.woken {
            Woken::Yes => {},
            Woken::No => return Ok(()),
        }

        self.woken = Woken::No;

        match console_onscreen::waking(Woken::No) {
            Ok(()) => {},
            Err(fault) => eprintln!("console-home: no one was told it is asleep: {fault}"),
        }

        Ok(())
    }

    fn shows(&self) -> Result<Shows, Never> {
        Ok(match (self.woken, self.pointer) {
            (Woken::Yes, _) | (_, Pointer::OnASquare) => Shows::AHighlight,
            (Woken::No, Pointer::Elsewhere) => Shows::None,
        })
    }

    fn pointed(&mut self, spot: Spot, at: (f64, f64)) -> Result<(), Never> {
        let (was, from) = match self.seen.replace((spot, at)) {
            Some((was, from)) => (was, from),
            None => return Ok(()),
        };

        let nudged = nudged(from, at)?;

        match (was == spot, nudged) {
            (true, Moved::NotAtAll) => Ok(()),
            (true, Moved::Somewhere) | (false, _) => self.stands_on(spot),
        }
    }

    fn stands_on(&mut self, spot: Spot) -> Result<(), Never> {
        self.here = spot;
        self.pointer = Pointer::OnASquare;

        Ok(())
    }

    fn nothing_pointed(&mut self) -> Result<(), Never> {
        self.pointer = Pointer::Elsewhere;
        self.seen = None;

        Ok(())
    }

    fn hand(&self) -> Result<Hand, Never> {
        Ok(match self.carrying {
            Some(_) => Hand::Carries,
            None => Hand::Empty,
        })
    }

    fn told_hand(&self) -> Result<(), Never> {
        let hand = self.hand()?;

        match console_onscreen::carrying(hand) {
            Ok(()) => {},
            Err(fault) => {
                eprintln!("console-home: no one was told what is in its hand: {fault}");
            },
        }

        Ok(())
    }

    fn panes(&self) -> Result<u32, Never> {
        let hand = self.hand()?;

        let carried = match hand {
            Hand::Carries => 1,
            Hand::Empty => 0,
        };

        let panes = self.home.panes()?;

        Ok(panes.saturating_add(carried))
    }

    fn standing(&self, spot: Spot) -> Result<Option<String>, Never> {
        let at = self.home.at(spot)?;

        Ok(at.map(str::to_string))
    }

    fn on(&self, spot: Spot) -> Result<(Option<String>, Carried), Never> {
        let Ok(standing) = self.standing(spot);

        let carrying = match self.carrying.as_ref() {
            Some(carrying) => carrying,
            None => return Ok((standing, Carried::No)),
        };

        Ok(match (spot == self.here, spot == carrying.from) {
            (true, _) => (Some(carrying.name.clone()), Carried::Yes),
            (false, true) => (None, Carried::No),
            (false, false) => (standing, Carried::No),
        })
    }

    fn press(&mut self, reached: Reached) -> Result<(), Never> {
        let hand = self.hand()?;

        match hand {
            Hand::Carries => return self.put_down(),
            Hand::Empty => {},
        }

        let name = self.standing(self.here)?;

        let name = match name {
            Some(name) => name,
            None => {
                let Ok(bare) = on_a_bare_square(reached);

                return match bare {
                    Bare::Chooses => self.place(),
                    Bare::Waits => Ok(()),
                };
            }
        };

        let app = self.apps.get(&name).map(|(app, _)| app.clone());

        match app {
            Some(app) => {
                self.hands_over()?;

                let command = found::command(&app)?;

                match command {
                    Some(arguments) => {
                        let Ok(()) = console_panel::running::left_running(&arguments);
                    }
                    None => {}
                }
            },
            None => eprintln!("console-home: {name} is not on this machine any more"),
        }

        Ok(())
    }

    fn lift(&mut self) -> Result<(), Never> {
        let hand = self.hand()?;

        match hand {
            Hand::Carries => return self.put_down(),
            Hand::Empty => {},
        }

        let from = self.here;

        let name = self.standing(from)?;

        let name = match name {
            Some(name) => name,
            None => return self.place(),
        };

        self.wakes()?;

        self.carrying = Some(Carrying { name, from });

        self.told_hand()
    }

    fn put_down(&mut self) -> Result<(), Never> {
        let (name, from) = match self.carrying.take() {
            Some(Carrying { name, from }) => (name, from),
            None => return Ok(()),
        };

        self.told_hand()?;

        let here = self.here;
        let there = self.standing(here)?;

        self.home.remove(from)?;

        self.home.place(here, &name)?;

        match there {
            Some(there) => self.home.place(from, &there)?,
            None => {},
        }

        self.keep()
    }

    fn put_back(&mut self) -> Result<Put, Never> {
        match self.carrying.take() {
            Some(_) => {},
            None => return Ok(Put::None),
        }

        self.told_hand()?;

        Ok(Put::Back)
    }

    fn ways(&mut self) -> Result<(), Never> {
        let hand = self.hand()?;

        match hand {
            Hand::Carries => return Ok(()),
            Hand::Empty => {},
        }

        let name = self.standing(self.here)?;

        let name = match name {
            Some(name) => name,
            None => return self.place(),
        };

        self.hands_over()?;

        console_panel::running::left_running(&["home-square".to_string(), name])
    }

    fn place(&mut self) -> Result<(), Never> {
        self.hands_over()?;

        let said = self.here.said()?;

        console_panel::running::left_running(&[
            "launcher".to_string(),
            "--place".to_string(),
            said,
        ])
    }

    fn take_off(&mut self) -> Result<(), Never> {
        let here = self.here;

        self.home.remove(here)?;

        self.keep()
    }

    fn hands_over(&mut self) -> Result<(), Never> {
        self.sleeps()
    }

    fn reshaped(&mut self) -> Result<(), Never> {
        let grid = asked_shape()?;

        match self.grid == grid {
            true => return Ok(()),
            false => {},
        }

        self.grid = grid;
        self.labels.clear();

        let fitted = self.home.fitted(grid)?;

        match self.home == fitted {
            true => {},
            false => {
                self.home = fitted;

                self.keep()?;
            }
        }

        let on = self.here.on_the_grid(grid)?;

        match on {
            On::None => self.here = Spot::FIRST,
            On::TheGrid => {},
        }

        Ok(())
    }

    fn reread(&mut self) -> Result<(), Never> {
        let hers = console_core_places::home()?;

        let at = match hers {
            Some(hers) => console_home_screen::file(&hers)?,
            None => return Ok(()),
        };

        let Ok(held) = console_core_atomic_writes::read(&at);

        let home = match held {
            Stored::Text(said) => HomeScreen::read(&said)?,
            Stored::Absent => self.first()?,
            Stored::Failed(fault) => {
                eprintln!("console-home: {}: {fault}", at.display());

                return Ok(());
            }
        };

        let home = home.fitted(self.grid)?;

        match self.home == home {
            true => return Ok(()),
            false => {},
        }

        self.home = home;

        self.keep()
    }

    fn first(&self) -> Result<HomeScreen, Never> {
        let names: Vec<String> = self.apps.keys().cloned().collect();
        let counted = found::counted()?;

        let order = console_applications::counts::order(&names, &counted)?;

        HomeScreen::first(&order, self.grid)
    }

    fn keep(&self) -> Result<(), Never> {
        let hers = console_core_places::home()?;

        let at = match hers {
            Some(hers) => console_home_screen::file(&hers)?,
            None => {
                eprintln!("console-home: no home to keep the home screen in; leaving it as it is");

                return Ok(());
            }
        };

        let said = self.home.written()?;

        match said.is_empty() && !at.exists() {
            true => return Ok(()),
            false => {},
        }

        match std::fs::read_to_string(&at).is_ok_and(|before| before == said) {
            true => return Ok(()),
            false => {},
        }

        match at.parent() {
            Some(above) => {
                let _ = std::fs::create_dir_all(above);
            }
            None => {},
        }

        match console_core_atomic_writes::whole(&at, said.as_bytes()) {
            Ok(()) => {},
            Err(fault) => eprintln!("console-home: {}: {fault}", at.display()),
        }

        Ok(())
    }

    fn settle(&mut self, surface: &mut Surface) -> Result<(), Never> {
        let holds = holds_a_window()?;

        let showing = match holds {
            Holds::AWindow => Showing::No,
            Holds::None => Showing::Yes,
        };

        match self.settled.replace(showing) == Some(showing) {
            true => {},
            false => match showing {
                Showing::Yes => {
                    let Ok(wanted) = covering();

                    match surface.show(&wanted) {
                        Ok(()) => {},
                        Err(fault) => eprintln!("console-home: no surface to draw on: {fault}"),
                    }
                }
                Showing::No => {
                    let Ok(()) = surface.hide();
                },
            },
        }

        let over = anything_over_it()?;

        match (showing, over) == (Showing::Yes, Over::None) {
            true => {},
            false => {
                self.nothing_pointed()?;

                self.sleeps()?;
            }
        }

        Ok(())
    }
}

fn covering() -> Result<Wanted, Never> {
    let Ok(bar) = console_status_bar::showing::Fitting::of_em();
    let Ok(bar) = bar.height();
    let Ok(top) = out(bar);

    Ok(Wanted {
        namespace: NAMESPACE.to_string(),
        anchor: Anchor::Whole,
        size: WHATEVER_THE_SCREEN_IS,
        margin: Margin { top, right: 0, bottom: 0, left: 0 },
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Under::AWindow,
    })
}

fn up(many: i32) -> Result<u32, Never> {
    fitted(many.max(0))
}

fn out(many: u32) -> Result<i32, Never> {
    fitted(many)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Centred {
    room: u32,
    thing: u32,
}

fn middle(centred: Centred) -> Result<i32, Never> {
    let Centred { room, thing } = centred;
    let Ok(room) = out(room);
    let Ok(thing) = out(thing);

    Ok(room.saturating_sub(thing).saturating_div(2))
}

fn plate(laid: &Laid, rounding: i32) -> Result<Panel, Never> {
    let Ok(round) = up(rounding);

    Ok(Panel {
        at: laid.plate.at,
        size: laid.plate.size,
        round: Round(round),
        fill: Oklch { lightness: 0.0, chroma: 0.0, hue: 0.0 },
        edge: Edge::None,
    })
}

fn dressed(
    holding: Option<&str>,
    carried: Carried,
    standing: Standing,
    wears: &Wears,
) -> Result<Option<(Oklch, Edge)>, Never> {
    let Ok(wide) = up(shape::BORDER);

    Ok(match (holding.is_some(), carried, standing) {
        (_, Carried::Yes, _) => Some((wears.carried, Edge::Of { wide, color: wears.held })),
        (true, Carried::No, Standing::Yes) => {
            Some((wears.standing, Edge::Of { wide, color: wears.edge }))
        }
        (true, Carried::No, Standing::No) => Some((wears.plate, Edge::None)),
        (false, Carried::No, Standing::Yes) => {
            Some((wears.bare, Edge::Of { wide, color: wears.edge }))
        }
        (false, Carried::No, Standing::No) => None,
    })
}

fn icon(at: &str, side: Side, wanted: &mut Vec<String>) -> Result<Option<Pixels>, Never> {
    match at.is_empty() {
        true => return Ok(None),
        false => {},
    }

    let Ok(held) = console_panel::pictures::pixels_at(Path::new(at), side);

    Ok(match held {
        Some(held) => Some(held),
        None => {
            wanted.push(at.to_string());

            None
        }
    })
}

fn words(said: &str, wide: u32, font: &Font) -> Result<MeasuredText, Never> {
    let whole = console_draw_painting::measured(
        Run { said, weight: Weight::Plain, width: WIDE_ENOUGH },
        font,
    )?;

    match whole.width <= wide {
        true => return Ok(MeasuredText { said: said.to_string(), width: whole.width }),
        false => {},
    }

    let letters: Vec<char> = said.chars().collect();
    let mut kept: &[char] = &letters;

    while let Some((_, shorter)) = kept.split_last() {
        kept = shorter;

        let mut cut: String = kept.iter().collect();
        cut.push(AND_SO_ON);

        let size = console_draw_painting::measured(
            Run { said: &cut, weight: Weight::Plain, width: WIDE_ENOUGH },
            font,
        )?;

        match size.width <= wide {
            true => return Ok(MeasuredText { said: cut, width: size.width }),
            false => {},
        }
    }

    Ok(MeasuredText { said: String::new(), width: 0 })
}

fn label(screen: &mut Screen, said: &str, wide: u32, font: &Font) -> Result<MeasuredText, Never> {
    let asked = (said.to_string(), wide);

    match screen.labels.get(&asked) {
        Some(found) => return Ok(found.clone()),
        None => {},
    }

    let Ok(fitted) = words(said, wide, font);

    screen.labels.insert(asked, fitted.clone());

    Ok(fitted)
}

fn drawing(screen: &mut Screen, wears: &Wears, room: (i32, i32)) -> Result<Drawing, Never> {
    let grid = screen.grid;
    let square = shape::square(room, grid)?;
    let Ok(side) = up(square.icon);
    let Ok(tall) = up(square.named);
    let font = Font { family: FONT.to_string(), height: tall };
    let panes = screen.panes()?;

    match screen.here.pane >= panes {
        true => screen.here.pane = panes.saturating_sub(1),
        false => {},
    }

    let here = screen.here;
    let shows = screen.shows()?;
    let inset = square.padding.saturating_add(shape::BORDER).saturating_mul(2);
    let mut shapes: Vec<Shape> = Vec::new();
    let mut touching: Vec<(Spot, Panel)> = Vec::new();
    let mut wanted: Vec<String> = Vec::new();

    for row in 0..grid.rows {
        'over_columns: for column in 0..grid.columns {
            let spot = Spot { pane: here.pane, row, column };
            let laid = shape::laid(room, grid, spot)?;
            let plate = plate(&laid, square.rounding)?;

            touching.push((spot, plate));

            let standing = match (shows, spot == here) {
                (Shows::AHighlight, true) => Standing::Yes,
                (Shows::AHighlight, false) | (Shows::None, _) => Standing::No,
            };
            let (holding, carried) = screen.on(spot)?;
            let dressed = dressed(holding.as_deref(), carried, standing, wears)?;

            match dressed {
                Some((fill, edge)) => shapes.push(Shape::Panel(Panel { fill, edge, ..plate })),
                None => {},
            }

            let holding = match holding {
                Some(holding) => holding,
                None => continue 'over_columns,
            };

            let at = screen.apps.get(&holding).map(|(_, at)| at.clone());

            let pixels = match at {
                Some(at) => icon(&at, Side(side), &mut wanted)?,
                None => None,
            };

            match pixels {
                Some(pixels) => shapes.push(Shape::Picture(Picture {
                    at: laid.icon.at,
                    size: laid.icon.size,
                    pixels,
                })),
                None => {},
            }

            let Ok(room_for_words) = up(inset);
            let room_for_words = laid.plate.size.width.saturating_sub(room_for_words);
            let words = label(screen, &holding, room_for_words, &font)?;
            let Ok(inside) = middle(Centred { room: laid.plate.size.width, thing: words.width });
            let across = laid.plate.at.x.saturating_add(inside);

            shapes.push(Shape::Text(Text {
                at: Point { x: across, y: laid.named.y },
                width: WIDE_ENOUGH,
                said: words.said,
                weight: Weight::Plain,
                font: font.clone(),
                ink: wears.text,
            }));
        }
    }

    let dots = dots(screen, room, panes, &font, wears)?;

    shapes.extend(dots);

    let waiting = match wanted.is_empty() {
        true => Waiting::None,
        false => {
            let Ok(()) = console_panel::pictures::make_at(&wanted, Side(side));

            Waiting::ForAPicture
        }
    };

    Ok(Drawing { shapes, touching, waiting })
}

fn dots(
    screen: &mut Screen,
    room: (i32, i32),
    panes: u32,
    font: &Font,
    wears: &Wears,
) -> Result<Vec<Shape>, Never> {
    match panes < 2 {
        true => return Ok(Vec::new()),
        false => {},
    }

    let one = label(screen, A_DOT, WIDE_ENOUGH, font)?;
    let Ok(many) = fitted::<_, u32>(panes);
    let Ok(between) = up(shape::BETWEEN);
    let whole = one
        .width
        .saturating_mul(many)
        .saturating_add(between.saturating_mul(many.saturating_sub(1)));
    let at = shape::dotted(room)?;
    let Ok(half) = middle(Centred { room: whole, thing: 0 });
    let mut across = at.x.saturating_sub(half);
    let mut drawn: Vec<Shape> = Vec::new();

    for pane in 0..panes {
        let ink = match pane == screen.here.pane {
            true => wears.here,
            false => wears.dot,
        };

        drawn.push(Shape::Text(Text {
            at: Point { x: across, y: at.y },
            width: WIDE_ENOUGH,
            said: A_DOT.to_string(),
            weight: Weight::Plain,
            font: font.clone(),
            ink,
        }));

        let Ok(step) = out(one.width.saturating_add(between));

        across = across.saturating_add(step);
    }

    Ok(drawn)
}

fn dressing() -> Result<Wears, WearingError> {
    let spent = palette::spent()?;
    let wears = wears(&spent)?;

    Ok(wears)
}

fn wears(spent: &BTreeMap<String, String>) -> Result<Wears, PaletteError> {
    let ground = hex(spent, "ground")?;

    let plate = washed(spent, "night", Ground(ground), 0.34)?;
    let standing = palette::named(spent, "panel")?;
    let bare = washed(spent, "panel", Ground(ground), 0.55)?;
    let edge = palette::named(spent, "mint")?;
    let carried = washed(spent, "pink", Ground(ground), 0.22)?;
    let held = palette::named(spent, "pink")?;
    let text = palette::named(spent, "text")?;
    let dot = washed(spent, "text", Ground(ground), 0.3)?;
    let here = palette::named(spent, "pink")?;

    Ok(Wears { plate, standing, bare, edge, carried, held, text, dot, here })
}

fn hex<'a>(
    spent: &'a BTreeMap<String, String>,
    what: &'static str,
) -> Result<&'a str, PaletteError> {
    match spent.get(what) {
        Some(said) => Ok(said),
        None => Err(PaletteError::Absent(what)),
    }
}

fn washed(
    spent: &BTreeMap<String, String>,
    what: &'static str,
    ground: Ground<'_>,
    share: f64,
) -> Result<Oklch, PaletteError> {
    let ink = hex(spent, what)?;
    let Ok(mixed) = over(HexColor(ink), ground, share);
    let channels = Rgba::of(&mixed)
        .map_err(|_| PaletteError::Invalid { color: what, value: mixed.clone() })?;
    let Ok(washed) = Oklch::of(channels);

    Ok(washed)
}

fn listening() -> Result<Option<UnixDatagram>, Never> {
    let at = match console_onscreen::homeward() {
        Ok(at) => at,
        Err(fault) => {
            eprintln!("console-home: nothing can be said to me: {fault}");

            return Ok(None);
        }
    };

    match at.parent() {
        Some(above) => {
            let _ = std::fs::create_dir_all(above);
        }
        None => {},
    }

    let _ = std::fs::remove_file(&at);

    Ok(match UnixDatagram::bind(&at) {
        Ok(socket) => Some(socket),
        Err(fault) => {
            eprintln!("console-home: {}: {fault}", at.display());

            None
        }
    })
}

fn listened(socket: &UnixDatagram) -> Result<Option<PadInput>, Never> {
    let Ok(room) = index(SAID_AT_ONCE);
    let mut said = vec![0u8; room];

    let heard = match socket.recv(&mut said) {
        Ok(got) => said.get(..got),
        Err(fault) => {
            eprintln!("console-home: nothing more can be said to me: {fault}");

            return Ok(None);
        }
    };

    match heard {
        Some(bytes) => match std::str::from_utf8(bytes) {
            Ok(word) => PadInput::read(word),
            Err(fault) => {
                eprintln!("console-home: something said {} bytes that are not words: {fault}", bytes.len());

                Ok(None)
            }
        },
        None => {
            eprintln!("console-home: something said it wrote more than the {SAID_AT_ONCE} bytes it had room for");

            Ok(None)
        }
    }
}

fn stirring() -> Result<Option<OwnedFd>, Never> {
    let (hear, tell) = match rustix::pipe::pipe() {
        Ok(ends) => ends,
        Err(fault) => {
            eprintln!("console-home: nothing to hear the compositor on: {fault}");

            return Ok(None);
        }
    };

    let (say, heard) = std::sync::mpsc::channel();
    let Ok(()) = console_events::again::about(&Topic::Compositor, worth_asking_after, say);

    let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
        for () in heard.iter() {
            match rustix::io::write(&tell, &[1]) {
                Ok(_) => {},
                Err(_the_loop_has_gone) => return,
            }
        }
    }));

    Ok(Some(hear))
}

fn stirred(hear: &OwnedFd) -> Result<(), Never> {
    let Ok(room) = index(A_MOUTHFUL);
    let mut said = vec![0u8; room];
    let _as_many_as_arrived_are_one_look = rustix::io::read(hear, &mut said);

    Ok(())
}

type Filling = Arc<Mutex<Vec<found::Found>>>;

fn searching() -> Result<Option<(OwnedFd, Filling)>, Never> {
    let (hear, tell) = match rustix::pipe::pipe() {
        Ok(ends) => ends,
        Err(fault) => {
            eprintln!("console-home: nothing to hear the search on: {fault}");

            return Ok(None);
        }
    };

    let held: Filling = Arc::new(Mutex::new(Vec::new()));
    let filling = Arc::clone(&held);

    let Ok(()) = console_program_lifetime::threads::let_go(std::thread::spawn(move || {
        let telling = tell;
        let looking: [fn() -> Result<found::Found, Never>; 2] = [found::quickly, found::machine];

        for look in looking {
            let Ok(found) = look();

            match filling.lock() {
                Ok(mut filling) => filling.push(found),
                Err(_no_one_is_reading) => return,
            }

            let _ = rustix::io::write(&telling, &[1]);
        }
    }));

    Ok(Some((hear, held)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Search {
    Going,
    Finished,
}

fn heard(hear: &OwnedFd) -> Result<Search, Never> {
    let Ok(room) = index(A_MOUTHFUL);
    let mut said = vec![0u8; room];

    Ok(match rustix::io::read(hear, &mut said) {
        Ok(0) | Err(_) => Search::Finished,
        Ok(_) => Search::Going,
    })
}

fn found_more(
    screen: &mut Screen,
    hear: &OwnedFd,
    held: &Filling,
) -> Result<Search, Never> {
    let Ok(search) = heard(hear);

    let taken = match held.lock() {
        Ok(mut held) => std::mem::take(&mut *held),
        Err(_the_search_gave_up) => Vec::new(),
    };

    for found in taken {
        let apps = named(found)?;

        screen.apps = apps;

        screen.reread()?;
    }

    Ok(search)
}

fn ready(watching: &[BorrowedFd<'_>]) -> Result<Vec<u32>, Never> {
    let mut watch: Vec<PollFd<'_>> =
        watching.iter().map(|fd| PollFd::from_borrowed_fd(*fd, PollFlags::IN)).collect();
    let now = Timespec { tv_sec: 0, tv_nsec: 0 };

    match poll(&mut watch, Some(&now)) {
        Ok(_) => {},
        Err(_nothing_to_watch) => return Ok(Vec::new()),
    }

    Ok(watch
        .iter()
        .enumerate()
        .filter(|(_, fd)| fd.revents().intersects(PollFlags::IN | PollFlags::HUP))
        .map(|(which, _)| {
            let Ok(which) = fitted(which);

            which
        })
        .collect())
}

fn least(soonest: Option<Duration>, asked: Duration) -> Result<Option<Duration>, Never> {
    Ok(match soonest {
        Some(soonest) => Some(soonest.min(asked)),
        None => Some(asked),
    })
}

fn until(
    screen: &Screen,
    waiting: Waiting,
) -> Result<Option<Duration>, Never> {
    let mut soonest: Option<Duration> = None;

    match screen.finger.as_ref() {
        Some(finger) => match finger.held {
            LongPress::NotYet => {
                let left = console_home_screen::HELD.saturating_sub(finger.since.elapsed());

                let Ok(sooner) = least(soonest, left.max(A_MOMENT));

                soonest = sooner;
            }
            LongPress::LongEnough => {},
        },
        None => {},
    }

    match waiting {
        Waiting::ForAPicture => least(soonest, SOON),
        Waiting::None => Ok(soonest),
    }
}

fn under(touching: &[(Spot, Panel)], at: (f64, f64)) -> Result<Option<Spot>, Never> {
    let Ok(across) = toward_zero_i32(at.0);
    let Ok(down) = toward_zero_i32(at.1);
    let point = Point { x: across, y: down };

    for (spot, panel) in touching {
        let covers = panel.covers(point)?;

        match covers {
            console_core_shapes::Covers::Yes => return Ok(Some(*spot)),
            console_core_shapes::Covers::No => {},
        }
    }

    Ok(None)
}

fn wandered(
    screen: &mut Screen,
    touching: &[(Spot, Panel)],
    room: (i32, i32),
    at: (f64, f64),
) -> Result<(), Never> {
    match screen.finger.as_mut() {
        Some(finger) => finger.at = at,
        None => {},
    }

    let on = under(touching, at)?;

    match on {
        Some(spot) => return screen.pointed(spot, at),
        None => {},
    }

    let Ok(across) = toward_zero_i32(at.0);
    let Ok(down) = toward_zero_i32(at.1);
    let over = shape::over(room, Point { x: across, y: down })?;

    match over {
        On::TheGrid => Ok(()),
        On::None => screen.nothing_pointed(),
    }
}

fn lifted(screen: &mut Screen) -> Result<(), Never> {
    let finger = match screen.finger.take() {
        Some(finger) => finger,
        None => return Ok(()),
    };

    let Ok(flick) = flicked(finger.from, finger.at);

    match flick {
        Flick::Upward => {
            screen.hands_over()?;

            return console_panel::running::left_running(&["launcher".to_string()]);
        }
        Flick::Across(along) => {
            let panes = screen.panes()?;

            let paned = paned(screen.here, along, panes)?;

            screen.here = paned;

            return Ok(());
        }
        Flick::Nowhere => {},
    }

    match finger.held {
        LongPress::LongEnough => return Ok(()),
        LongPress::NotYet => {},
    }

    let touched = touched(finger.from, finger.at)?;

    match (touched, finger.on) {
        (Touch::Pressed, Some(spot)) => {
            screen.here = spot;

            screen.press(Reached::ByTouch)
        }
        (Touch::Pressed, None) | (Touch::Travelled, _) => Ok(()),
    }
}

fn holding(screen: &mut Screen, now: Instant) -> Result<(), Never> {
    let ripe = match screen.finger.as_mut() {
        Some(finger) => {
            let since = now.saturating_duration_since(finger.since);
            let Ok(held) = held(since, finger.from, finger.at);

            match (held, finger.held) {
                (LongPress::LongEnough, LongPress::NotYet) => {
                    finger.held = LongPress::LongEnough;

                    finger.on
                }
                (LongPress::LongEnough, LongPress::LongEnough) | (LongPress::NotYet, _) => None,
            }
        }
        None => None,
    };

    match ripe {
        Some(spot) => {
            screen.here = spot;

            screen.lift()
        }
        None => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Watching {
    Signal,
    Subscriber,
    Searching,
    Stirred,
}

fn worth_asking_after(line: &str) -> Result<Worth, Never> {
    let stirred = console_compositor::events::read(line)?;

    Ok(match stirred {
        CompositorEvent::WindowOpened(_)
        | CompositorEvent::WindowClosed(_)
        | CompositorEvent::WindowMoved
        | CompositorEvent::WindowFilled
        | CompositorEvent::LayerOpened
        | CompositorEvent::LayerClosed
        | CompositorEvent::WorkspaceChanged
        | CompositorEvent::ScreenFocused => Worth::Querying,
        CompositorEvent::WindowRenamed(_)
        | CompositorEvent::WindowFloated
        | CompositorEvent::WindowPinned
        | CompositorEvent::ConfigReloaded
        | CompositorEvent::Ignored => Worth::Ignoring,
    })
}

fn asked_shape() -> Result<Grid, Never> {
    let hers = console_core_places::home()?;

    let at = match hers {
        Some(hers) => shape::at(&hers)?,
        None => return Ok(Grid::USUAL),
    };

    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        Stored::Text(said) => Grid::read(&said),
        Stored::Absent => Ok(Grid::USUAL),
        Stored::Failed(fault) => {
            eprintln!("console-home: {}: {fault}", at.display());

            Ok(Grid::USUAL)
        }
    }
}

fn holds_a_window() -> Result<Holds, Never> {
    let workspace = match console_compositor::query(console_compositor::Query::ActiveWorkspace) {
        Ok(console_compositor::Answer::ActiveWorkspace(workspace)) => workspace,
        Ok(_not_what_was_asked) => return Ok(Holds::AWindow),
        Err(_the_compositor_said_nothing) => return Ok(Holds::AWindow),
    };

    Ok(match workspace {
        Some(front) => match front.windows {
            Some(0) => Holds::None,
            Some(_held) => Holds::AWindow,
            None => Holds::AWindow,
        },
        None => Holds::AWindow,
    })
}

fn anything_over_it() -> Result<Over, Never> {
    let screens = match console_onscreen::screens() {
        Ok(screens) => screens,
        Err(_fault) => return Ok(Over::Some),
    };

    over_the_desktop(&screens)
}

fn named(found: found::Found) -> Result<Named, Never> {
    Ok(found
        .apps
        .into_iter()
        .map(|(name, app)| {
            let picture = match found.icon.get(&name) {
                Some(picture) => picture.clone(),
                None => String::new(),
            };

            (name, (app, picture))
        })
        .collect())
}

fn main() {
    let Ok(stopping) = console_panel::asked::told();

    let wears = match dressing() {
        Ok(wears) => wears,
        Err(why) => {
            eprintln!("console-home: no palette: {why}");

            return;
        }
    };

    let mut surface = match Surface::connect() {
        Ok(surface) => surface,
        Err(fault) => {
            eprintln!("console-home: no surface: {fault}");

            return;
        }
    };

    let Ok(mut screen) = Screen::new();
    let Ok(remembered) = found::remembered();
    let Ok(apps) = named(remembered);

    screen.apps = apps;

    let Ok(()) = screen.reread();
    let Ok(()) = screen.settle(&mut surface);
    let Ok(listening) = listening();
    let Ok(stirring) = stirring();
    let Ok(mut searching) = searching();

    match console_onscreen::waking(Woken::No) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-home: no one was told it is asleep: {fault}"),
    }

    match console_onscreen::carrying(Hand::Empty) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-home: no one was told its hand is empty: {fault}"),
    }

    let mut drew: Option<Vec<Shape>> = None;
    let mut waiting = Waiting::None;
    let mut touching: Vec<(Spot, Panel)> = Vec::new();
    let mut room = (0, 0);

    loop {
        let now = Instant::now();

        let Ok(()) = holding(&mut screen, now);
        let Ok(events) = surface.pointer_events();

        for event in events {
            match event {
                PointerEvent::Moved { at } => {
                    let Ok(()) = wandered(&mut screen, &touching, room, at);
                }
                PointerEvent::Down { at } => {
                    let Ok(on) = under(&touching, at);

                    screen.finger =
                        Some(TouchState { from: at, at, since: now, on, held: LongPress::NotYet });
                }
                PointerEvent::Up => {
                    let Ok(()) = lifted(&mut screen);
                }
                PointerEvent::Left => {
                    screen.finger = None;

                    let Ok(()) = screen.nothing_pointed();
                }
                PointerEvent::Scrolled { by: _ } | PointerEvent::Pinched { by: _ } => {},
            }
        }

        let Ok(logical) = surface.logical();

        match logical {
            Some(logical) => {
                let Ok(across) = out(logical.width);
                let Ok(down) = out(logical.height);

                room = (across, down);

                let Ok(drawing) = drawing(&mut screen, &wears, room);

                waiting = drawing.waiting;
                touching = drawing.touching;

                match drew.as_ref() == Some(&drawing.shapes) {
                    true => {},
                    false => {
                        let painted = surface.draw(|pixels, device, _scale| {
                            let frame = Frame { device, points: logical };
                            let _ = console_draw_painting::onto(pixels, frame, &drawing.shapes);

                            Ok(())
                        });

                        match painted {
                            Ok(()) => {},
                            Err(fault) => eprintln!("console-home: {fault}"),
                        }

                        drew = Some(drawing.shapes);
                    }
                }
            }
            None => drew = None,
        }

        let Ok(until) = until(&screen, waiting);

        let woke = {
            let mut watching: Vec<(Watching, BorrowedFd<'_>)> = Vec::new();

            match stopping.as_ref() {
                Some(fd) => watching.push((Watching::Signal, fd.as_fd())),
                None => {},
            }

            match listening.as_ref() {
                Some(socket) => watching.push((Watching::Subscriber, socket.as_fd())),
                None => {},
            }

            match searching.as_ref() {
                Some((fd, _)) => watching.push((Watching::Searching, fd.as_fd())),
                None => {},
            }

            match stirring.as_ref() {
                Some(fd) => watching.push((Watching::Stirred, fd.as_fd())),
                None => {},
            }

            let also: Vec<BorrowedFd<'_>> = watching.iter().map(|(_, fd)| *fd).collect();

            match surface.wait(&also, until) {
                Ok(()) => {},
                Err(fault) => {
                    eprintln!("console-home: {fault}");

                    return;
                }
            }

            let Ok(ready) = ready(&also);

            ready
                .iter()
                .filter_map(|which| {
                    let Ok(at) = index(*which);

                    watching.get(at).map(|(what, _)| *what)
                })
                .collect::<Vec<Watching>>()
        };

        let Ok(closed) = surface.closed();

        match closed {
            Closed::Yes => return,
            Closed::No => {},
        }

        for what in woke {
            match what {
                Watching::Signal => return,
                Watching::Subscriber => {
                    let said = match listening.as_ref() {
                        Some(socket) => {
                            let Ok(said) = listened(socket);

                            said
                        }
                        None => None,
                    };

                    match said {
                        Some(said) => {
                            let Ok(()) = screen.told(said);
                        }
                        None => {},
                    }
                }
                Watching::Searching => {
                    let search = match searching.as_ref() {
                        Some((fd, held)) => {
                            let Ok(search) = found_more(&mut screen, fd, held);

                            search
                        }
                        None => Search::Finished,
                    };

                    match search {
                        Search::Finished => searching = None,
                        Search::Going => {},
                    }
                }
                Watching::Stirred => {
                    let stirred = match stirring.as_ref() {
                        Some(hear) => stirred(hear),
                        None => Ok(()),
                    };
                    let Ok(()) = stirred;
                    let Ok(()) = screen.settle(&mut surface);
                    let Ok(()) = screen.reread();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_that_has_told_everything_it_found_is_finished_and_not_ready_forever() {
        let (hear, tell) = rustix::pipe::pipe().expect("a pipe");
        let _ = rustix::io::write(&tell, &[1]);

        assert_eq!(heard(&hear), Ok(Search::Going), "something was found and more may follow");

        drop(tell);

        assert_eq!(
            heard(&hear),
            Ok(Search::Finished),
            "the search is over, and a pipe nobody writes to is ready on every poll: watched, it spins a core"
        );
    }
}
