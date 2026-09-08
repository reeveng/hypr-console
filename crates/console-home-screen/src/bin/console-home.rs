//! The home screen, drawn on the wallpaper.
//!
//! This desktop opened into nothing. A wallpaper and a bar, and every
//! application behind a button somebody had to know about first -- which is
//! the one thing a phone, a console and a laptop all decline to do. So the
//! applications are on the screen: panes of them over the wallpaper -- as
//! many as what is on them needs -- walked with the d-pad, opened with A, and
//! arranged with Y.
//!
//! ## It is the desktop, not a thing on top of it
//!
//! `console_onscreen::FURNITURE` names it, so the shoulders still
//! change workspace, the left Legion button still leaves for Steam, and the
//! paddles still do what they do everywhere. The d-pad is its own from the
//! moment it is drawn; A and Y become its own once the d-pad has woken it.
//! `Mode::Home` and `Mode::Standing` are where that is decided.
//!
//! ## It is told what the pad did, and holds no keyboard
//!
//! Every other surface here hears the pad as keys, and this one cannot. It is
//! drawn under everything and never in front, so the only way it could take
//! the keyboard was to ask for it exclusively -- which Hyprland answers by
//! handing it every pointer and every touch on the screen, wherever they land,
//! because that is what the thing it was written for needs. Held that way this
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
//! with nothing pointing at it is an offer nobody is making: by then the
//! pointer is over the bar, and A is the bar's.
//!
//! ## And it goes away when there is something to look at
//!
//! A window on the workspace is what somebody is doing, and the home screen is
//! what they do it from. So the surface is put away while a window is up --
//! which is also what keeps the wallpaper's own reading of "is anything in
//! front of me" true, and what keeps A a click while a game is on the screen.
//!
//! ## Swiping
//!
//! The panes are swiped with a finger on the surface itself, which is a GTK
//! gesture and wants nothing of the compositor. Hyprland's own workspace swipe
//! is the touchpad's, and a plugin -- hyprgrass -- is what a gesture *over
//! somebody else's window* would need. Neither is this: the finger is on the
//! home screen, and the home screen is the thing that reads it.
//!
//! Which means the same finger is on a square, and a swipe and a tap both end
//! with it coming up. `console_home_screen::touched` is what separates them, so the
//! flick that moves the panes is not also a press of whatever it started on.
//!
//! ## A bare square is not tapped into the chooser
//!
//! Most of the home screen is empty most of the time, and a flick that stops
//! short of `DRIFT` is a tap -- so the finger that meant to change panes and
//! did not travel far enough put the whole list of applications on the screen
//! instead, from a square nobody was aiming at. The same square answers the
//! d-pad, where it cannot happen: the highlight had to be walked there first,
//! and A on it is a sentence somebody finished.
//!
//! So who is asking decides it. `console_home_screen::on_a_bare_square` is
//! that question and `Reached` is the answer: a button chooses, a finger
//! waits. The finger still has the chooser -- holding a bare square is already
//! `lift`, and `lift` on a square with nothing to pick up is `place` -- so
//! nothing is taken away, it is moved onto the gesture that cannot be arrived
//! at by accident. A square that has something on it is unchanged either way:
//! a tap opens it, which is what a tap on a thing has always meant.
//!
//! ## And what it is carrying is asked for, never held
//!
//! The square in the hand is a `RefCell`, and a `match` on
//! `self.carrying.borrow().is_some()` keeps that borrow alive until the whole
//! match is over -- including the arm that puts the square down, which asks
//! the same cell for itself again. So the press that should have set a
//! carried square down killed the home screen instead, and Y offering to move
//! one led nowhere every time. `hand` answers `Hand` and lets the borrow go at
//! its own end, and it is the only thing here that reads that cell to decide
//! with.
//!
//! And it is said out loud, the way being awake is. Nothing outside this
//! process could see a square in the hand: the arrangement is only written
//! when the square is put back down, and what is held in between is not a
//! window, a layer or a file. So `console_onscreen::carrying` is written
//! wherever that cell changes, and the machine can be asked whether somebody
//! is holding an application instead of being photographed to find out.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::rc::Rc;

use console_applications::entry::Application;
use console_applications::found;
use console_home_screen::{
    Along, Bare, Home, Moved, On, Reached, Spot, Touch, Way, moved, nudged, on_a_bare_square,
    paned, touched,
};
use console_home_screen::shape::{self, Shape};
use console_compositor::stirred::Stirred;
use console_core_atomic_writes::Held;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_onscreen::{Awake, Hand, Over, Said, over_the_desktop};
use console_panel::icons::Icon;
use gtk4::{
    Align, Box as GtkBox, CssProvider, EventControllerMotion, GestureClick, GestureSwipe, Grid,
    Label, Orientation, Window, gdk, glib,
};
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

const NAMESPACE: &str = "console-home";


const CLEARED: i32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holds {
    AWindow,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Woke {
    Already,
    Just,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Put {
    Back,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pointer {
    OnASquare,
    Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shows {
    AHighlight,
    Nothing,
}

struct Screen {
    window: Window,
    grid: Grid,
    dots: GtkBox,
    here: Cell<Spot>,
    home: RefCell<Home>,
    apps: RefCell<BTreeMap<String, (Application, String)>>,
    carrying: RefCell<Option<Carrying>>,
    woken: Cell<bool>,
    pointer: Cell<Pointer>,
    seen: Cell<Option<(Spot, (f64, f64))>>,
    settled: Cell<Option<Showing>>,
    shape: Cell<Shape>,
    drawn: Cell<Option<console_home_screen::Square>>,
    sheet: CssProvider,
}

struct Carrying {
    name: String,
    from: Spot,
}

impl Screen {
    fn new() -> Result<Rc<Screen>, Never> {
        let window = Window::new();

        window.set_widget_name("home");

        laid_under_everything(&window)?;

        let grid = Grid::new();
        grid.set_widget_name("squares");
        grid.set_row_homogeneous(true);
        grid.set_column_homogeneous(true);
        grid.set_vexpand(true);
        grid.set_valign(Align::Center);

        grid.set_hexpand(true);
        grid.set_halign(Align::Fill);

        let dots = GtkBox::new(Orientation::Horizontal, 12);
        dots.set_widget_name("panes");
        dots.set_halign(Align::Center);

        let all = GtkBox::new(Orientation::Vertical, 0);
        all.set_widget_name("home");
        all.append(&grid);
        all.append(&dots);
        window.set_child(Some(&all));

        let asked = asked_shape()?;

        let screen = Rc::new(Screen {
            window,
            grid,
            dots,
            here: Cell::new(Spot::FIRST),
            home: RefCell::new(Home::default()),
            apps: RefCell::new(BTreeMap::new()),
            carrying: RefCell::new(None),
            woken: Cell::new(false),
            pointer: Cell::new(Pointer::Elsewhere),
            seen: Cell::new(None),
            settled: Cell::new(None),
            shape: Cell::new(asked),
            drawn: Cell::new(None),
            sheet: CssProvider::new(),
        });

        screen.dressed()?;

        screen.listens()?;

        Ok(screen)
    }

    fn told(self: &Rc<Screen>, said: Said) -> Result<(), Never> {
        let way = match said {
            Said::Up => Some(Way::Up),
            Said::Down => Some(Way::Down),
            Said::Left => Some(Way::Left),
            Said::Right => Some(Way::Right),
            Said::Pressed
            | Said::More
            | Said::Back
            | Said::Again
            | Said::Carry
            | Said::Off => None,
        };

        match way {
            Some(way) => {
                let woke = self.wakes()?;

                match (woke, self.pointer.get()) {
                    (Woke::Already, _) | (Woke::Just, Pointer::OnASquare) => {
                        let shown = self.shown()?;

                        let moved = moved(self.here.get(), way, shown, self.shape.get())?;

                        self.here.set(moved);
                    },
                    (Woke::Just, Pointer::Elsewhere) => {},
                }

                return self.draw();
            }
            None => {},
        }

        match said {
            Said::Pressed => self.press(Reached::ByButton),
            Said::More => self.ways(),
            Said::Back => {
                let put = self.put_back()?;

                match put {
                    Put::Back => Ok(()),
                    Put::Nothing => self.sleeps(),
                }
            }
            Said::Again => self.reshaped(),
            Said::Carry => self.lift(),
            Said::Off => self.take_off(),
            Said::Up | Said::Down | Said::Left | Said::Right => Ok(()),
        }
    }

    fn wakes(self: &Rc<Screen>) -> Result<Woke, Never> {
        match self.woken.replace(true) {
            true => return Ok(Woke::Already),
            false => {},
        }

        match console_onscreen::waking(Awake::Yes) {
            Ok(()) => {},
            Err(fault) => eprintln!("console-home: nobody was told it is awake: {fault}"),
        }

        Ok(Woke::Just)
    }

    fn sleeps(self: &Rc<Screen>) -> Result<(), Never> {
        match self.woken.replace(false) {
            true => {},
            false => return Ok(()),
        }

        match console_onscreen::waking(Awake::No) {
            Ok(()) => {},
            Err(fault) => eprintln!("console-home: nobody was told it is asleep: {fault}"),
        }

        self.draw()
    }

    fn shows(&self) -> Result<Shows, Never> {
        Ok(match (self.woken.get(), self.pointer.get()) {
            (true, _) | (_, Pointer::OnASquare) => Shows::AHighlight,
            (false, Pointer::Elsewhere) => Shows::Nothing,
        })
    }

    fn pointed(self: &Rc<Screen>, spot: Spot, at: (f64, f64)) -> Result<(), Never> {
        let (was, from) = match self.seen.replace(Some((spot, at))) {
            Some((was, from)) => (was, from),
            None => return Ok(()),
        };

        let nudged = nudged(from, at)?;

        match (was == spot, nudged) {
            (true, Moved::NotAtAll) => Ok(()),
            (true, Moved::Somewhere) | (false, _) => self.stands_on(spot),
        }
    }

    fn stands_on(self: &Rc<Screen>, spot: Spot) -> Result<(), Never> {
        let stood = self.here.replace(spot);

        match (self.pointer.replace(Pointer::OnASquare), stood == spot) {
            (Pointer::OnASquare, true) => Ok(()),
            (Pointer::OnASquare, false) | (Pointer::Elsewhere, _) => self.draw(),
        }
    }

    fn wandered(self: &Rc<Screen>, x: f64, y: f64) -> Result<(), Never> {
        let on = self.on_the_squares(x, y)?;

        match on {
            On::TheGrid => Ok(()),
            On::Nothing => self.nothing_pointed(),
        }
    }

    fn on_the_squares(&self, x: f64, y: f64) -> Result<On, Never> {
        let squares = match self.grid.compute_bounds(&self.window) {
            Some(squares) => squares,
            None => return Ok(On::Nothing),
        };

        let (left, top) = (f64::from(squares.x()), f64::from(squares.y()));
        let inside = x >= left
            && x < left + f64::from(squares.width())
            && y >= top
            && y < top + f64::from(squares.height());

        Ok(match inside {
            true => On::TheGrid,
            false => On::Nothing,
        })
    }

    fn nothing_pointed(self: &Rc<Screen>) -> Result<(), Never> {
        match self.pointer.replace(Pointer::Elsewhere) {
            Pointer::Elsewhere => Ok(()),
            Pointer::OnASquare => self.draw(),
        }
    }

    fn listens(self: &Rc<Screen>) -> Result<(), Never> {
        let swipe = GestureSwipe::new();
        swipe.set_touch_only(false);
        let screen = Rc::clone(self);
        swipe.connect_swipe(move |_, x, y| {
            const FLICK: f64 = 300.0;

            match y < -FLICK && y.abs() > x.abs() {
                true => {
                    let Ok(()) = screen.hands_over();

                    let Ok(()) =
                        console_panel::running::left_running(&["launcher".to_string()]);

                    return;
                }
                false => {},
            }

            let along = match x {
                _ if x < -FLICK && x.abs() > y.abs() => Along::After,
                _ if x > FLICK && x.abs() > y.abs() => Along::Before,
                _ => return,
            };

            let Ok(shown) = screen.shown();

            let Ok(paned) = paned(screen.here.get(), along, shown);

            screen.here.set(paned);

            let Ok(()) = screen.draw();
        });
        self.window.add_controller(swipe);

        let pointer = EventControllerMotion::new();
        let screen = Rc::clone(self);

        pointer.connect_motion(move |_, x, y| {
            let Ok(()) = screen.wandered(x, y);
        });

        self.window.add_controller(pointer);

        Ok(())
    }

    fn hand(&self) -> Result<Hand, Never> {
        Ok(match self.carrying.borrow().is_some() {
            true => Hand::Carries,
            false => Hand::Empty,
        })
    }

    fn told_hand(&self) -> Result<(), Never> {
        let hand = self.hand()?;

        match console_onscreen::carrying(hand) {
            Ok(()) => {},
            Err(fault) => {
                eprintln!("console-home: nobody was told what is in its hand: {fault}");
            },
        }

        Ok(())
    }

    fn shown(&self) -> Result<usize, Never> {
        let hand = self.hand()?;

        let carried = match hand {
            Hand::Carries => 1,
            Hand::Empty => 0,
        };

        let panes = self.home.borrow().panes()?;

        Ok(panes.saturating_add(carried))
    }

    fn draw(self: &Rc<Screen>) -> Result<(), Never> {
        self.dressed()?;

        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }

        let shown = self.shown()?;

        match self.here.get().pane >= shown {
            true => {
                self.here.set(Spot { pane: shown.saturating_sub(1), ..self.here.get() });
            }
            false => {},
        }

        let here = self.here.get();

        let shape = self.shape.get();

        for row in 0..shape.rows {
            for column in 0..shape.columns {
                let spot = Spot::new(here.pane, row, column)?;

                let square = self.square(spot)?;

                let Ok(column) = fitted(column);
                let Ok(row) = fitted(row);

                self.grid.attach(&square, column, row, 1, 1);
            }
        }

        while let Some(child) = self.dots.first_child() {
            self.dots.remove(&child);
        }

        match shown < 2 {
            true => return Ok(()),
            false => {},
        }

        for pane in 0..shown {
            let dot = Label::new(Some("\u{25cf}"));
            dot.set_widget_name("pane");

            match pane == here.pane {
                true => dot.add_css_class("here"),
                false => {},
            }

            self.dots.append(&dot);
        }

        Ok(())
    }

    fn square(self: &Rc<Screen>, spot: Spot) -> Result<GtkBox, Never> {
        let square = GtkBox::new(Orientation::Vertical, 6);
        square.set_widget_name("square");
        square.set_hexpand(true);
        square.set_halign(Align::Fill);
        square.set_valign(Align::Center);

        let shows = self.shows()?;

        match shows == Shows::AHighlight && spot == self.here.get() {
            true => square.add_css_class("here"),
            false => {},
        }

        let carrying = self.carrying.borrow();

        let held = match carrying.as_ref() {
            Some(carrying) if spot == self.here.get() => {
                square.add_css_class("carrying");
                Some(carrying.name.clone())
            },
            Some(carrying) if spot == carrying.from => None,
            Some(_) | None => {
                let home = self.home.borrow();

                let at = home.at(spot)?;

                at.map(str::to_string)
            }
        };

        drop(carrying);

        let measured = self.measured()?;

        match held {
            Some(name) => {
                let picture = self.apps.borrow().get(&name).map(|(_, at)| at.clone());

                let shown = drawn(picture.as_deref(), measured.icon)?;

                square.append(&shown);

                let said = Label::new(Some(&name));
                said.set_widget_name("named");
                said.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                said.set_max_width_chars(12);
                square.append(&said);
            },
            None => {
                square.add_css_class("empty");

                let icon = measured.icon;
                let room = gtk4::Image::new();
                room.set_widget_name("picture");
                room.set_pixel_size(icon);
                room.set_size_request(icon, icon);
                square.append(&room);

                let line = Label::new(Some(" "));
                line.set_widget_name("named");
                square.append(&line);
            },
        }

        let screen = Rc::clone(self);
        let touch = GestureClick::new();
        let down = Rc::new(Cell::new((0.0, 0.0)));
        let went = Rc::clone(&down);
        touch.connect_pressed(move |_, _, x, y| went.set((x, y)));
        touch.connect_released(move |_, _, x, y| {
            let Ok(touched) = touched(down.get(), (x, y));

            match touched {
                Touch::Pressed => {
                    screen.here.set(spot);

                    let Ok(()) = screen.press(Reached::ByTouch);
                },
                Touch::Travelled => {},
            }
        });

        square.add_controller(touch);

        let screen = Rc::clone(self);
        let held = gtk4::GestureLongPress::new();
        held.set_touch_only(false);
        held.connect_pressed(move |gesture, _, _| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            screen.here.set(spot);

            let Ok(()) = screen.lift();
        });

        square.add_controller(held);

        let screen = Rc::clone(self);
        let pointer = EventControllerMotion::new();

        pointer.connect_motion(move |_, x, y| {
            let Ok(()) = screen.pointed(spot, (x, y));
        });

        square.add_controller(pointer);

        Ok(square)
    }

    fn standing(&self, spot: Spot) -> Result<Option<String>, Never> {
        let home = self.home.borrow();

        let at = home.at(spot)?;

        Ok(at.map(str::to_string))
    }

    fn press(self: &Rc<Screen>, reached: Reached) -> Result<(), Never> {
        let hand = self.hand()?;

        match hand {
            Hand::Carries => return self.put_down(),
            Hand::Empty => {},
        }

        let here = self.here.get();

        let name = self.standing(here)?;

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

        let app = self.apps.borrow().get(&name).map(|(app, _)| app.clone());

        match app {
            Some(app) => {
                self.hands_over()?;

                found::run(&app)?;
            },
            None => eprintln!("console-home: {name} is not on this machine any more"),
        }

        Ok(())
    }

    fn lift(self: &Rc<Screen>) -> Result<(), Never> {
        let hand = self.hand()?;

        match hand {
            Hand::Carries => return self.put_down(),
            Hand::Empty => {},
        }

        let from = self.here.get();

        let name = self.standing(from)?;

        let name = match name {
            Some(name) => name,
            None => return self.place(),
        };

        self.wakes()?;

        *self.carrying.borrow_mut() = Some(Carrying { name, from });

        self.told_hand()?;

        self.draw()
    }

    fn put_down(self: &Rc<Screen>) -> Result<(), Never> {
        let (name, from) = match self.carrying.replace(None) {
            Some(Carrying { name, from }) => (name, from),
            None => return Ok(()),
        };

        self.told_hand()?;

        let here = self.here.get();
        let there = self.standing(here)?;

        {
            let mut home = self.home.borrow_mut();

            home.remove(from)?;

            home.place(here, &name)?;

            match there {
                Some(there) => home.place(from, &there)?,
                None => {},
            }
        }

        self.keep()?;

        self.draw()
    }

    fn put_back(self: &Rc<Screen>) -> Result<Put, Never> {
        match self.carrying.replace(None).is_none() {
            true => return Ok(Put::Nothing),
            false => {},
        }

        self.told_hand()?;

        self.draw()?;

        Ok(Put::Back)
    }

    fn ways(self: &Rc<Screen>) -> Result<(), Never> {
        let hand = self.hand()?;

        match hand {
            Hand::Carries => return Ok(()),
            Hand::Empty => {},
        }

        let name = self.standing(self.here.get())?;

        let name = match name {
            Some(name) => name,
            None => return self.place(),
        };

        self.hands_over()?;

        console_panel::running::left_running(&["home-square".to_string(), name])?;

        Ok(())
    }

    fn place(self: &Rc<Screen>) -> Result<(), Never> {
        self.hands_over()?;

        let said = self.here.get().said()?;

        console_panel::running::left_running(&[
            "launcher".to_string(),
            "--place".to_string(),
            said,
        ])?;

        Ok(())
    }

    fn take_off(self: &Rc<Screen>) -> Result<(), Never> {
        self.home.borrow_mut().remove(self.here.get())?;

        self.keep()?;

        self.draw()
    }

    fn hands_over(self: &Rc<Screen>) -> Result<(), Never> {
        self.sleeps()
    }

    fn room(&self) -> Result<(i32, i32), Never> {
        let granted = (self.window.width(), self.window.height());

        match granted.0 > 1 && granted.1 > 1 {
            true => return Ok(granted),
            false => {},
        }

        let display = match gdk::Display::default() {
            Some(display) => display,
            None => return Ok((0, 0)),
        };

        let first = match display.monitors().item(0).and_downcast::<gdk::Monitor>() {
            Some(first) => first,
            None => return Ok((0, 0)),
        };

        let screen = first.geometry();

        Ok((screen.width(), screen.height().saturating_sub(CLEARED)))
    }

    fn measured(&self) -> Result<console_home_screen::Square, Never> {
        let room = self.room()?;

        console_home_screen::square(room, self.shape.get())
    }

    fn dressed(self: &Rc<Screen>) -> Result<(), Never> {
        let square = self.measured()?;

        match self.drawn.replace(Some(square)) == Some(square) {
            true => return Ok(()),
            false => {},
        }

        let display = match gdk::Display::default() {
            Some(display) => display,
            None => return Ok(()),
        };

        let Ok(palette) = console_panel::style::palette();

        self.sheet.load_from_data(
            &include_str!("../home.css")
                .replace("{palette}", &palette)
                .replace("{padding}", &square.padding.to_string())
                .replace("{rounding}", &square.rounding.to_string())
                .replace("{margin}", &square.margin.to_string())
                .replace("{named}", &square.named.to_string())
                .replace("{inset}", &console_home_screen::shape::INSET.to_string())
                .replace("{sides}", &console_home_screen::shape::SIDES.to_string())
                .replace("{dots}", &console_home_screen::shape::DOTS.to_string())
                .replace("{border}", &console_home_screen::shape::BORDER.to_string())
                .replace("{dot}", &square.named.to_string()),
        );

        gtk4::style_context_add_provider_for_display(
            &display,
            &self.sheet,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        Ok(())
    }

    fn reshaped(self: &Rc<Screen>) -> Result<(), Never> {
        let shape = asked_shape()?;

        match self.shape.replace(shape) == shape {
            true => return Ok(()),
            false => {},
        }

        let fitted = self.home.borrow().fitted(shape)?;

        let same = *self.home.borrow() == fitted;

        match same {
            true => {},
            false => {
                *self.home.borrow_mut() = fitted;

                self.keep()?;
            }
        }

        let on = self.here.get().on_the_grid(shape)?;

        match on {
            On::Nothing => self.here.set(Spot::FIRST),
            On::TheGrid => {},
        }

        self.dressed()?;

        self.draw()
    }

    fn reread(self: &Rc<Screen>) -> Result<(), Never> {
        let hers = console_core_places::home()?;

        let at = match hers {
            Some(hers) => console_home_screen::file(&hers)?,
            None => return Ok(()),
        };

        let Ok(held) = console_core_atomic_writes::read(&at);

        let home = match held {
            Held::Said(said) => Home::read(&said)?,
            Held::Nothing => self.first()?,
            Held::Unreadable(fault) => {
                eprintln!("console-home: {}: {fault}", at.display());

                return Ok(());
            }
        };

        let home = home.fitted(self.shape.get())?;

        match *self.home.borrow() == home {
            true => return Ok(()),
            false => {},
        }

        *self.home.borrow_mut() = home;

        self.keep()?;

        self.draw()
    }

    fn first(self: &Rc<Screen>) -> Result<Home, Never> {
        let apps = self.apps.borrow();
        let names: Vec<String> = apps.keys().cloned().collect();
        let counted = found::counted()?;

        let order = console_applications::counts::order(&names, &counted)?;

        Home::first(&order, self.shape.get())
    }

    fn keep(self: &Rc<Screen>) -> Result<(), Never> {
        let hers = console_core_places::home()?;

        let at = match hers {
            Some(hers) => console_home_screen::file(&hers)?,
            None => {
                eprintln!("console-home: no home to keep the home screen in; leaving it as it is");

                return Ok(());
            }
        };

        let said = self.home.borrow().written()?;

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

        match std::fs::write(&at, said) {
            Ok(()) => {},
            Err(fault) => eprintln!("console-home: {}: {fault}", at.display()),
        }

        Ok(())
    }

    fn settle(self: &Rc<Screen>) -> Result<(), Never> {
        let holds = holds_a_window()?;

        let showing = match holds {
            Holds::AWindow => Showing::No,
            Holds::Nothing => Showing::Yes,
        };

        match self.settled.replace(Some(showing)) == Some(showing) {
            true => {},
            false => match showing {
                Showing::Yes => self.window.present(),
                Showing::No => self.window.set_visible(false),
            },
        }

        let over = anything_over_it()?;

        match (showing, over) == (Showing::Yes, Over::Nothing) {
            true => {},
            false => {
                self.nothing_pointed()?;

                self.sleeps()?;
            }
        }

        Ok(())
    }
}

fn drawn(at: Option<&str>, icon: i32) -> Result<gtk4::Image, Never> {
    let held = gtk4::Image::new();
    held.set_widget_name("picture");
    held.set_pixel_size(icon);
    held.set_size_request(icon, icon);

    match at {
        Some(at) if !at.is_empty() => held.set_from_file(Some(at)),
        Some(_) | None => {
            let Ok(named) = Icon::AnyApplication.name();

            held.set_icon_name(Some(named));
        }
    }

    Ok(held)
}

fn laid_under_everything(window: &Window) -> Result<(), Never> {
    window.init_layer_shell();
    window.set_namespace(Some(NAMESPACE));
    window.set_layer(Layer::Bottom);

    window.set_exclusive_zone(-1);
    window.set_margin(Edge::Top, CLEARED);

    window.set_keyboard_mode(KeyboardMode::None);

    for edge in [Edge::Bottom, Edge::Left, Edge::Right, Edge::Top] {
        window.set_anchor(edge, true);
    }

    Ok(())
}

fn holds_a_window() -> Result<Holds, Never> {
    let workspace = match console_compositor::asked(console_compositor::Asked::ActiveWorkspace) {
        Ok(workspace) => workspace,
        Err(_the_compositor_said_nothing) => return Ok(Holds::AWindow),
    };

    let Ok(held) = console_compositor::windows(&workspace);

    Ok(match held {
        Some(0) => Holds::Nothing,
        Some(_held) => Holds::AWindow,
        None => Holds::AWindow,
    })
}

fn anything_over_it() -> Result<Over, Never> {
    let screens = match console_panel::door::screens() {
        Ok(screens) => screens,
        Err(_fault) => return Ok(Over::Something),
    };

    over_the_desktop(&screens)
}

fn worth_asking_after(line: &str) -> Result<Over, Never> {
    let stirred = console_compositor::stirred::read(line)?;

    Ok(match stirred {
        Stirred::WindowOpened(_)
        | Stirred::WindowClosed(_)
        | Stirred::WindowMoved
        | Stirred::WindowFilled
        | Stirred::LayerOpened
        | Stirred::LayerClosed
        | Stirred::WorkspaceChanged
        | Stirred::ScreenFocused => Over::Something,
        Stirred::WindowRenamed(_)
        | Stirred::WindowFloated
        | Stirred::WindowPinned
        | Stirred::ConfigReloaded
        | Stirred::Nothing => Over::Nothing,
    })
}

fn listening(screen: &Rc<Screen>) -> Result<(), Never> {
    let at = match console_onscreen::homeward() {
        Ok(at) => at,
        Err(fault) => {
            eprintln!("console-home: nothing can be said to me: {fault}");

            return Ok(());
        },
    };

    match at.parent() {
        Some(above) => {
            let _ = std::fs::create_dir_all(above);
        }
        None => {},
    }

    let _ = std::fs::remove_file(&at);

    let socket = match UnixDatagram::bind(&at) {
        Ok(socket) => socket,
        Err(fault) => {
            eprintln!("console-home: {}: {fault}", at.display());

            return Ok(());
        },
    };

    let screen = Rc::clone(screen);
    glib::spawn_future_local(async move {
        let mut socket = socket;

        loop {
            let heard = gtk4::gio::spawn_blocking(move || {
                let mut said = [0u8; 64];
                let got = socket.recv(&mut said);

                (socket, said, got)
            })
            .await;

            let (held, said, got) = match heard {
                Ok((held, said, got)) => (held, said, got),
                Err(_fault) => return,
            };

            socket = held;

            let got = match got {
                Ok(got) => got,
                Err(fault) => {
                    eprintln!("console-home: nothing more can be said to me: {fault}");

                    return;
                },
            };

            let said = match said.get(..got).map(std::str::from_utf8) {
                Some(Ok(word)) => {
                    let Ok(read) = Said::read(word);

                    read
                },
                Some(Err(fault)) => {
                    eprintln!("console-home: something said {got} bytes that are not words: {fault}");

                    None
                },
                None => {
                    eprintln!("console-home: something said it wrote {got} bytes into 64");

                    None
                },
            };

            let said = match said {
                Some(said) => said,
                None => continue,
            };

            let Ok(()) = screen.told(said);
        }
    });

    Ok(())
}

fn following(screen: &Rc<Screen>) -> Result<(), Never> {
    let screen = Rc::clone(screen);
    glib::spawn_future_local(async move {
        loop {
            let socket = match console_panel::door::events() {
                Ok(socket) => socket,
                Err(_fault) => return,
            };

            let opened = gtk4::gio::spawn_blocking(move || UnixStream::connect(&socket)).await;

            let stream = match opened {
                Ok(Ok(stream)) => stream,
                Ok(Err(_)) | Err(_) => {
                    let Ok(()) = screen.settle();

                    #[cfg_attr(
                        dylint_lib = "explicit021_no_sleeping",
                        allow(
                            explicit021_no_sleeping,
                            reason = "the door is not open and nothing announces when it will be; this is the wait between two attempts at connecting, which is the same decision `console-core-reconnect` makes for a thread"
                        )
                    )]
                    glib::timeout_future(std::time::Duration::from_secs(2)).await;

                    continue;
                }
            };

            let mut lines = BufReader::new(stream);

            let Ok(()) = screen.settle();

            loop {
                let read = gtk4::gio::spawn_blocking(move || {
                    let mut said = String::new();

                    let got = match lines.read_line(&mut said) {
                        Ok(got) => got,
                        Err(_fault) => return (lines, said, 0),
                    };

                    (lines, said, got)
                })
                .await;

                let (held, said, got) = match read {
                    Ok((held, said, got)) => (held, said, got),
                    Err(_fault) => return,
                };

                match got {
                    0 => break,
                    _ => {},
                }

                lines = held;

                let Ok(worth) = worth_asking_after(&said);

                match worth {
                    Over::Something => {
                        let Ok(()) = screen.settle();

                        let Ok(()) = screen.reread();
                    }
                    Over::Nothing => {},
                }
            }
        }
    });

    Ok(())
}

fn asked_shape() -> Result<Shape, Never> {
    let hers = console_core_places::home()?;

    let at = match hers {
        Some(hers) => shape::at(&hers)?,
        None => return Ok(Shape::USUAL),
    };

    let Ok(held) = console_core_atomic_writes::read(&at);

    match held {
        Held::Said(said) => Shape::read(&said),
        Held::Nothing => Ok(Shape::USUAL),
        Held::Unreadable(fault) => {
            eprintln!("console-home: {}: {fault}", at.display());

            Ok(Shape::USUAL)
        },
    }
}

fn main() {
    match gtk4::init() {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-home: no screen to draw on: {fault}");
            return;
        }
    }

    let waiting = glib::MainLoop::new(None, false);

    let Ok(screen) = Screen::new();

    let Ok(()) = console_panel::asked::stops_when_asked({
        let waiting = waiting.clone();
        move || waiting.quit()
    });

    let Ok(found) = found::remembered();

    let Ok(apps) = named(found);

    *screen.apps.borrow_mut() = apps;

    let Ok(()) = screen.reread();

    let Ok(()) = screen.settle();

    let reading = Rc::clone(&screen);

    glib::spawn_future_local(async move {
        match gtk4::gio::spawn_blocking(found::quickly).await {
            Ok(found) => {
                let Ok(found) = found;

                let Ok(apps) = named(found);

                *reading.apps.borrow_mut() = apps;

                let Ok(()) = reading.reread();

                let Ok(()) = reading.draw();
            }
            Err(_the_search_went_away) => {},
        }

        let found = match gtk4::gio::spawn_blocking(found::machine).await {
            Ok(found) => found,
            Err(_fault) => return,
        };

        let Ok(found) = found;

        let Ok(apps) = named(found);

        *reading.apps.borrow_mut() = apps;

        let Ok(()) = reading.reread();

        let Ok(()) = reading.draw();
    });

    let Ok(()) = listening(&screen);

    let Ok(()) = following(&screen);

    match console_onscreen::waking(Awake::No) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-home: nobody was told it is asleep: {fault}"),
    }

    match console_onscreen::carrying(Hand::Empty) {
        Ok(()) => {},
        Err(fault) => eprintln!("console-home: nobody was told its hand is empty: {fault}"),
    }

    waiting.run();
}

type Named = BTreeMap<String, (Application, String)>;

fn named(found: found::Found) -> Result<Named, Never> {
    Ok(found
        .apps
        .into_iter()
        .map(|(name, app)| {
            let picture = found.icon.get(&name).cloned().unwrap_or_default();

            (name, (app, picture))
        })
        .collect())
}
