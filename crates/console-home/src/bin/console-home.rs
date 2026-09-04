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
//! `console_controller::mode::FURNITURE` names it, so the shoulders still
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
//! `console_door::homeward`, and this holds nothing.
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
//! with it coming up. `console_home::touched` is what separates them, so the
//! flick that moves the panes is not also a press of whatever it started on.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::rc::Rc;

use console_door::{Awake, Said};
use console_home::shape::{self, Shape};
use console_home::{Along, Home, Spot, Touch, Way, moved, paned, touched};
use console_menu::entry::Application;
use console_menu::found;
use console_number::fitted;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, CssProvider, GestureClick, GestureSwipe, Grid, Label, Orientation,
    Window, gdk, glib,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

/// What this calls itself to the compositor.
///
/// `console_controller::mode::HOME` is the same word from the other side and
/// `the_namespace` holds them together. Everything that follows from the home
/// screen being on the screen -- what A means, what Y means -- is downstream
/// of this string.
const NAMESPACE: &str = "console-home";


/// The room at the top that is the bar's, in logical pixels.
///
/// The surface stands out of the way of every exclusive zone on its own,
/// because the one zone it must not answer is the keyboard's: a home screen
/// that gave the keyboard its room was a grid that hopped upward behind
/// whatever was being typed into. Opting out is all or nothing, so the bar's
/// rows are cleared here by the same number they reserve, and
/// `the_room_left_for_the_bar_is_what_the_bar_reserves` holds the two
/// together.
const CLEARED: i32 = 40;

/// How long A has to be in before it is a hold rather than a press.
///
/// Long enough that opening something is never mistaken for picking it up, and
/// short enough that somebody who meant to pick it up is not left wondering.
/// The same length GTK gives a long press by default, so the finger and the
/// button agree without either being told about the other.
const HOLD: std::time::Duration = std::time::Duration::from_millis(500);

/// Whether the surface is on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    Yes,
    No,
}

/// Whether the workspace has a window on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holds {
    AWindow,
    Nothing,
}

/// Whether the highlight was already up when the d-pad asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Woke {
    /// It was, so this press is a move.
    Already,
    /// It was not, so this press is the highlight arriving and nothing else.
    Just,
}

/// Whether there was anything in your hand to put back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Put {
    Back,
    Nothing,
}

/// The home screen and everything it is holding.
struct Screen {
    window: Window,
    /// The squares, redrawn in place. One grid for the pane being looked at.
    grid: Grid,
    /// The pane marks under it.
    dots: GtkBox,
    /// Where the d-pad is standing.
    here: Cell<Spot>,
    /// What is on the home screen.
    home: RefCell<Home>,
    /// What each application is called, what it runs, and its picture.
    apps: RefCell<BTreeMap<String, (Application, String)>>,
    /// What has been picked up, and the square it was picked up from.
    ///
    /// Held here rather than written down: a home screen that fell over with
    /// something in its hand should come back with that thing where it was,
    /// and the file already says where that is.
    carrying: RefCell<Option<Carrying>>,
    /// When A went in, so that holding it can mean something other than
    /// pressing it.
    since: Cell<Option<std::time::Instant>>,
    /// Whether the highlight is up.
    ///
    /// Not where it is -- that is `here`, and it is remembered whether the
    /// highlight is drawn or not, so waking puts it back where it was left.
    /// This is only whether it is being drawn, and whether A is the square's
    /// or the pointer's.
    woken: Cell<bool>,
    /// Whether the surface was on the screen the last time this was asked.
    ///
    /// So that the answer is acted on when it changes and not every time the
    /// compositor says anything. Nothing at all until the first reading, which
    /// is the one that has to be acted on whatever it says.
    settled: Cell<Option<Showing>>,
    /// How many squares a pane has and how big they are drawn, as her own
    /// file says.
    ///
    /// Read again whenever the settings tab says it has changed, which is what
    /// [`Said::Again`] is for. Held rather than asked for at every drawing,
    /// because it is asked for once per square and a file read fifteen times a
    /// redraw is a disk answering a question it already answered.
    shape: Cell<Shape>,
    /// The square as it was last measured, so the stylesheet is written again
    /// only when it would say something different.
    drawn: Cell<Option<console_home::Square>>,
    /// The stylesheet, kept so the numbers in it can be written again when the
    /// shape or the room changes.
    sheet: CssProvider,
}

/// Something picked up, and where it came from.
struct Carrying {
    name: String,
    from: Spot,
}

impl Screen {
    fn new() -> Rc<Screen> {
        let window = Window::new();

        // Named, because the stylesheet asks for `window#home` and a window
        // with no name is not it. Without this the rule that makes the home
        // screen transparent matched only the box inside it, and the window
        // behind that went on painting what the toolkit paints a window it has
        // been told nothing about -- which the palette maps to `panel`. So a
        // screen written to be the wallpaper with applications on it was a
        // flat sheet of the card colour, with the wallpaper drawn and never
        // seen behind it.
        window.set_widget_name("home");

        laid_under_everything(&window);

        let grid = Grid::new();
        grid.set_widget_name("squares");
        grid.set_row_homogeneous(true);
        grid.set_column_homogeneous(true);
        grid.set_vexpand(true);
        grid.set_valign(Align::Center);

        // The width of the screen, rather than the width of the longest name
        // on it. Centred at its natural size the grid asked for no more than
        // its squares wanted, and a square wants what its picture and its
        // ellipsised name want -- which has nothing to do with how wide the
        // machine's screen is. The screen was drawn with the applications in a
        // column down the middle and a strip of untouched wallpaper down
        // either side, the same wherever the layout is asked to put more or
        // fewer of them. Filling is what makes the arrangement a function of
        // the screen and the number of columns, which is the thing that is
        // meant to be adjustable.
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

        let screen = Rc::new(Screen {
            window,
            grid,
            dots,
            here: Cell::new(Spot::FIRST),
            home: RefCell::new(Home::default()),
            apps: RefCell::new(BTreeMap::new()),
            carrying: RefCell::new(None),
            since: Cell::new(None),
            woken: Cell::new(false),
            settled: Cell::new(None),
            shape: Cell::new(asked_shape()),
            drawn: Cell::new(None),
            sheet: CssProvider::new(),
        });

        screen.dressed();

        screen.listens();
        screen
    }

    /// What the pad did, acted on.
    ///
    /// The same jobs the keys used to be, reached the way this surface can
    /// actually be reached. Nothing here decides whether a button belongs to
    /// the home screen -- the daemon decided that before it said anything, out
    /// of `Mode::Home` and `Mode::Standing` -- so a word that arrives is a
    /// word that was meant for here.
    fn told(self: &Rc<Screen>, said: Said) {
        let way = match said {
            Said::Up => Some(Way::Up),
            Said::Down => Some(Way::Down),
            Said::Left => Some(Way::Left),
            Said::Right => Some(Way::Right),
            _ => None,
        };

        if let Some(way) = way {
            // The first press wakes it and moves nothing. A highlight that
            // appeared already one square along would be a highlight nobody
            // saw arrive, and the square it left is the square somebody was
            // about to press.
            if self.wakes() == Woke::Already {
                self.here.set(moved(self.here.get(), way, self.shown(), self.shape.get()));
            }

            self.draw();

            return;
        }

        match said {
            // A going in. What it turns out to have meant is decided when it
            // comes back out, because a press and a hold are the same press
            // until one of them has gone on long enough.
            Said::Pressing => {
                if self.since.get().is_none() {
                    self.since.set(Some(std::time::Instant::now()));
                }
            },
            Said::Released => {
                let held =
                    self.since.replace(None).is_some_and(|since| since.elapsed() >= HOLD);

                match held {
                    true => self.lift(),
                    false => self.press(),
                }
            },
            // Y, which everywhere on this desktop is what else can be done
            // with what you are looking at. Here that is the card that says
            // which applications are on the home screen at all.
            Said::More => self.manage(),
            // B, which everywhere is out of what is up. With something in your
            // hand it puts it back where it was; with an empty hand there is
            // nothing up but the highlight, and that is what it puts away.
            Said::Back => match self.put_back() {
                Put::Back => {},
                Put::Nothing => self.sleeps(),
            },
            // The settings tab has changed the grid. Not a press: nothing is
            // woken and nothing moves, the screen is drawn to the shape it is
            // now.
            Said::Again => self.reshaped(),
            Said::Up | Said::Down | Said::Left | Said::Right => {},
        }
    }

    /// Raise the highlight, and say so.
    ///
    /// Whether it was already up is the answer, because the press that wakes
    /// it is a press that does nothing else.
    fn wakes(self: &Rc<Screen>) -> Woke {
        if self.woken.replace(true) {
            return Woke::Already;
        }

        // Said out loud, because the daemon is what decides whose A is whose
        // and it cannot see a highlight. Written before this returns, so the
        // button pressed after this one is read against the answer this press
        // arrived at.
        if let Err(fault) = console_door::waking(Awake::Yes) {
            eprintln!("console-home: nobody was told it is awake: {fault}");
        }

        Woke::Just
    }

    /// Put the highlight away, and say so.
    fn sleeps(self: &Rc<Screen>) {
        if !self.woken.replace(false) {
            return;
        }

        if let Err(fault) = console_door::waking(Awake::No) {
            eprintln!("console-home: nobody was told it is asleep: {fault}");
        }

        self.draw();
    }

    /// A finger on the surface: what the surface itself reads.
    fn listens(self: &Rc<Screen>) {
        // A finger, taking the panes one at a time. The gesture is on the
        // surface because the finger is: nothing is asked of the compositor,
        // which has its own swipe for the touchpad and knows nothing about
        // these panes.
        let swipe = GestureSwipe::new();
        swipe.set_touch_only(false);
        let screen = Rc::clone(self);
        swipe.connect_swipe(move |_, x, y| {
            // A flick, not a drag. Anything slower than this is a thumb
            // resting on an icon, and a pane that moved under one of those is
            // a home screen that will not stay still.
            const FLICK: f64 = 300.0;

            // Up is the menu, which is the gesture a hand already has from
            // every phone it has held: the home screen is the few things you
            // put there, and everything else is one flick up. It is only the
            // home screen's, because the home screen is the only surface the
            // finger is on when there is nothing else on the screen.
            if y < -FLICK && y.abs() > x.abs() {
                screen.hands_over();
                console_panel::running::left_running(&["launcher".to_string()]);

                return;
            }

            let along = match x {
                _ if x < -FLICK && x.abs() > y.abs() => Along::After,
                _ if x > FLICK && x.abs() > y.abs() => Along::Before,
                _ => return,
            };

            screen.here.set(paned(screen.here.get(), along, screen.shown()));
            screen.draw();
        });
        self.window.add_controller(swipe);
    }

    /// How many panes there are to walk: as many as what is placed needs, and
    /// one more while something is in your hand.
    ///
    /// The extra one is the offer of a fresh pane, which is the only way a
    /// pane comes into being -- and it is only there while there is something
    /// to put on it, because an empty pane reachable at any time is a place
    /// the screen would have to explain.
    fn shown(&self) -> usize {
        self.home.borrow().panes() + usize::from(self.carrying.borrow().is_some())
    }

    /// Draw the pane the d-pad is on.
    fn draw(self: &Rc<Screen>) {
        // Before the squares, because the numbers they are drawn with are in
        // the stylesheet. Asked on every drawing and answered on almost none:
        // the room only changes when the desktop is laid out at another
        // density or the compositor grants something different, and until the
        // window is up the monitor is what was measured against.
        self.dressed();

        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }

        // The pane being stood on can stop existing under the highlight: the
        // last thing on it taken off through the card, or something carried
        // off it and put down panes away. Stood past the end, the highlight
        // steps back to the last pane there is rather than drawing a pane
        // the home screen has not got.
        let shown = self.shown();

        if self.here.get().pane >= shown {
            self.here.set(Spot { pane: shown - 1, ..self.here.get() });
        }

        let here = self.here.get();

        let shape = self.shape.get();

        for row in 0..shape.rows {
            for column in 0..shape.columns {
                let spot = Spot::new(here.pane, row, column);
                let square = self.square(spot);
                self.grid.attach(&square, fitted(column), fitted(row), 1, 1);
            }
        }

        while let Some(child) = self.dots.first_child() {
            self.dots.remove(&child);
        }

        // One pane wears no dots: a lone dot under the grid points at nothing
        // anybody could go to. The row appears with the second pane, which is
        // also the moment there is somewhere else to be.
        if shown < 2 {
            return;
        }

        for pane in 0..shown {
            let dot = Label::new(Some("\u{25cf}"));
            dot.set_widget_name("pane");

            if pane == here.pane {
                dot.add_css_class("here");
            }

            self.dots.append(&dot);
        }
    }

    /// One square: what is on it, or the room for something to be.
    fn square(self: &Rc<Screen>, spot: Spot) -> GtkBox {
        let square = GtkBox::new(Orientation::Vertical, 6);
        square.set_widget_name("square");
        // The whole of its cell across, so the plates tile evenly and what
        // separates them is their margin and nothing else. Left to its natural
        // width a plate is as wide as its name, and neighbours with short names
        // sit further apart than neighbours with long ones.
        //
        // Both of these, and the grid's `hexpand` above is not enough on its
        // own: filling is what a widget does with the room it is given, and
        // expanding is what makes a column ask for more than its contents
        // want. A grid whose columns hold nothing that expands stays the width
        // of its widest name however much screen it has been handed.
        square.set_hexpand(true);
        square.set_halign(Align::Fill);
        square.set_valign(Align::Center);

        // Only while it is awake. Asleep there is no highlight at all, which
        // is what leaves A to the pointer and lets the bar be pressed.
        if self.woken.get() && spot == self.here.get() {
            square.add_css_class("here");
        }

        // What is in your hand travels with the highlight, and the square it
        // came from is drawn empty while it is up: a home screen that showed
        // it in both places at once would be one nobody could tell had picked
        // anything up.
        let carrying = self.carrying.borrow();
        let held = match carrying.as_ref() {
            Some(carrying) if spot == self.here.get() => {
                square.add_css_class("carrying");
                Some(carrying.name.clone())
            },
            Some(carrying) if spot == carrying.from => None,
            _ => self.home.borrow().at(spot).map(str::to_string),
        };
        drop(carrying);

        match held {
            Some(name) => {
                let picture = self.apps.borrow().get(&name).map(|(_, at)| at.clone());
                square.append(&drawn(picture.as_deref(), self.measured().icon));

                let said = Label::new(Some(&name));
                said.set_widget_name("named");
                said.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                said.set_max_width_chars(12);
                square.append(&said);
            },
            // An empty square is room and not a hole. It is drawn as the room
            // it is only while the d-pad is standing on it, because fifteen
            // dotted outlines on a wallpaper is a form, and one is an offer.
            //
            // The room is the size of what would stand in it: a blank the
            // picture's size and a line the name's height. It is the same
            // measurement its neighbours are drawn at rather than a number of
            // its own, so it is exactly their size at every shape and every
            // density -- a pinned box was shorter than the plates around it
            // and stayed the one size whatever the rest of the screen did.
            None => {
                square.add_css_class("empty");

                let icon = self.measured().icon;
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

        // A tap, which is the press A is. Where the finger went down is kept,
        // because a swipe comes up on a square as well: the surface is a screen
        // of applications, so a flick across it starts on one of them and ends
        // with the same release a tap ends with. `touched` is what tells them
        // apart, and without it one flick moved the panes and opened whatever
        // the thumb happened to be over.
        let screen = Rc::clone(self);
        let touch = GestureClick::new();
        let down = Rc::new(Cell::new((0.0, 0.0)));
        let went = Rc::clone(&down);
        touch.connect_pressed(move |_, _, x, y| went.set((x, y)));
        touch.connect_released(move |_, _, x, y| match touched(down.get(), (x, y)) {
            Touch::Pressed => {
                screen.here.set(spot);
                screen.press();
            },
            // The swipe on the window is reading this same finger and will do
            // what it asked for. Nothing is owed here.
            Touch::Travelled => {},
        });
        square.add_controller(touch);

        // Held rather than tapped, which is what a finger does on a phone: the
        // application is picked up, and the next press puts it down. It is the
        // same thing holding A does, so the screen alone is enough to arrange
        // the home screen and so is the pad alone.
        //
        // The press is claimed, so the tap underneath does not also fire and
        // open what has just been picked up.
        let screen = Rc::clone(self);
        let held = gtk4::GestureLongPress::new();
        held.set_touch_only(false);
        held.connect_pressed(move |gesture, _, _| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            screen.here.set(spot);
            screen.lift();
        });
        square.add_controller(held);

        square
    }

    /// A press, once it is known to have been a press and not a hold.
    ///
    /// With something in your hand it is put down here. With an empty hand it
    /// opens what is under it, and an empty square offers the card instead --
    /// because a press on nothing that says nothing is a button somebody
    /// presses twice before deciding it is broken.
    fn press(self: &Rc<Screen>) {
        if self.carrying.borrow().is_some() {
            self.put_down();
            return;
        }

        let here = self.here.get();

        let Some(name) = self.home.borrow().at(here).map(str::to_string) else {
            self.manage();
            return;
        };

        let app = self.apps.borrow().get(&name).map(|(app, _)| app.clone());

        match app {
            Some(app) => {
                self.hands_over();
                found::run(&app);
            },
            // Placed, and then uninstalled. It stays on the home screen, so it
            // can be seen and taken off, and says why nothing happened.
            None => eprintln!("console-home: {name} is not on this machine any more"),
        }
    }

    /// Pick up what is under the highlight.
    ///
    /// A finger held on it, or A held down: the same thing said two ways, so
    /// the home screen can be arranged with the screen alone and with the pad
    /// alone. Nothing under it is nothing to pick up, and the card that says
    /// what goes on the home screen is what that press is for instead.
    fn lift(self: &Rc<Screen>) {
        if self.carrying.borrow().is_some() {
            self.put_down();
            return;
        }

        let from = self.here.get();

        let Some(name) = self.home.borrow().at(from).map(str::to_string) else {
            self.manage();
            return;
        };

        // Something in your hand is something that has to be seen going
        // somewhere, so this raises the highlight if the finger got here
        // before the d-pad did.
        self.wakes();

        *self.carrying.borrow_mut() = Some(Carrying { name, from });
        self.draw();
    }

    /// Put down what is in your hand, where the highlight is.
    ///
    /// Onto an empty square it moves. Onto a taken one the two change places,
    /// which is what a hand expects of a grid that is full: dropping one onto
    /// another and losing the second is a home screen that eats things.
    fn put_down(self: &Rc<Screen>) {
        let Some(Carrying { name, from }) = self.carrying.replace(None) else { return };

        let here = self.here.get();

        {
            let mut home = self.home.borrow_mut();
            let there = home.at(here).map(str::to_string);
            home.remove(from);
            home.place(here, &name);

            if let Some(there) = there {
                home.place(from, &there);
            }
        }

        self.keep();
        self.draw();
    }

    /// Put back what is in your hand, where it came from.
    fn put_back(self: &Rc<Screen>) -> Put {
        if self.carrying.replace(None).is_none() {
            return Put::Nothing;
        }

        self.draw();

        Put::Back
    }

    /// The card that says which applications are on the home screen.
    ///
    /// Y, wherever the highlight is. Which square a thing goes on is not asked
    /// there -- it goes in the first free one, and it is moved by picking it
    /// up here, which is a grid you can see rather than a list of coordinates
    /// you cannot.
    fn manage(self: &Rc<Screen>) {
        self.hands_over();
        console_panel::running::left_running(&["home-place".to_string()]);
    }

    /// Put the highlight away, before starting something that will be in
    /// front of it.
    ///
    /// The layer socket says a panel opened and `settle` puts the highlight
    /// away when it hears that, but the panel is on the screen before anybody
    /// has been told it exists. Where the home screen is the one starting it,
    /// there is no need to wait to be told: a highlight still drawn under a
    /// menu is A still belonging to a square nobody is looking at.
    fn hands_over(self: &Rc<Screen>) {
        self.sleeps();
    }

    /// The room the squares are divided into: the surface, or the screen it is
    /// about to be given while there is nothing to measure.
    ///
    /// The surface is anchored to all four edges and stands out of the way of
    /// the bar by [`CLEARED`] rather than by answering its zone, so what the
    /// compositor grants is the screen less that. Before the window is up
    /// there is nothing granted and the monitor less the same number is the
    /// nearest thing to an answer; the first drawing after it lands corrects
    /// it.
    fn room(&self) -> (i32, i32) {
        let granted = (self.window.width(), self.window.height());

        if granted.0 > 1 && granted.1 > 1 {
            return granted;
        }

        let Some(display) = gdk::Display::default() else { return (0, 0) };
        let Some(first) = display.monitors().item(0).and_downcast::<gdk::Monitor>() else {
            return (0, 0);
        };

        let screen = first.geometry();
        (screen.width(), screen.height() - CLEARED)
    }

    /// How big a square is on this screen, in this shape.
    fn measured(&self) -> console_home::Square {
        console_home::square(self.room(), self.shape.get())
    }

    /// The stylesheet, in this machine's own colours and at this square's own
    /// size.
    ///
    /// Written again rather than once, because everything about a square but
    /// the picture is in here and every one of those numbers moves when the
    /// shape does or when the desktop is laid out at another density. Nothing
    /// happens where the numbers would come out the same, which is every
    /// drawing but the few that follow a change: loading a stylesheet makes
    /// every widget on the display work out its style again.
    fn dressed(self: &Rc<Screen>) {
        let square = self.measured();

        if self.drawn.replace(Some(square)) == Some(square) {
            return;
        }

        let Some(display) = gdk::Display::default() else { return };

        self.sheet.load_from_data(
            &include_str!("../home.css")
                .replace("{palette}", &console_panel::style::palette())
                .replace("{padding}", &square.padding.to_string())
                .replace("{rounding}", &square.rounding.to_string())
                .replace("{margin}", &square.margin.to_string())
                .replace("{named}", &square.named.to_string())
                // The dots under the panes are not part of a square and do not
                // want a square's proportions. They are a row of full stops
                // saying how many panes there are, and they read at the size
                // the names read at.
                .replace("{dot}", &square.named.to_string()),
        );

        gtk4::style_context_add_provider_for_display(
            &display,
            &self.sheet,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    /// Read the shape again, and draw to it if it has changed.
    ///
    /// What the settings tab's word means. Everything placed off the grid the
    /// new shape has is moved onto it rather than dropped, and written down
    /// where it landed, so taking a column off is applications folding round
    /// onto the end of the pane and never applications going away.
    fn reshaped(self: &Rc<Screen>) {
        let shape = asked_shape();

        if self.shape.replace(shape) == shape {
            return;
        }

        let fitted = self.home.borrow().fitted(shape);

        if *self.home.borrow() != fitted {
            *self.home.borrow_mut() = fitted;
            self.keep();
        }

        // Where the highlight was standing may not be a square any more.
        if self.here.get().on_the_grid(shape) == console_home::On::Nothing {
            self.here.set(Spot::FIRST);
        }

        self.dressed();
        self.draw();
    }

    /// Read what is on the home screen, from the file the card writes.
    ///
    /// A machine that has never had one gets the applications it opens most,
    /// which is the first pane full and nothing past it. Written down at
    /// once, so the first thing anybody does to it is a change to a home
    /// screen that already existed rather than a change to a guess.
    fn reread(self: &Rc<Screen>) {
        let at = console_home::file(&found::home());
        let home = match std::fs::read_to_string(&at) {
            Ok(said) => Home::read(&said),
            // No file at all is a machine that has never drawn one. A file
            // that is there and says nothing is a home screen somebody has
            // taken everything off, and filling that back in would be a
            // desktop that puts back what it was asked to clear.
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => self.first(),
            Err(fault) => {
                eprintln!("console-home: {}: {fault}", at.display());

                return;
            }
        };

        // Everything the file names is kept, whatever shape the grid was in
        // when it was written, and this is where it is put back on the grid.
        let home = home.fitted(self.shape.get());

        // A file that says what is already on the screen is nothing to do.
        // This is asked on every layer that opens or closes -- the on-screen
        // keyboard among them -- and rebuilding fifteen squares from fifteen
        // pictures on the disk each time is a home screen that hitches
        // whenever anything else appears in front of it.
        if *self.home.borrow() == home {
            return;
        }

        *self.home.borrow_mut() = home;
        self.keep();
        self.draw();
    }

    /// The home screen a machine that has never had one gets: what it opens
    /// most, in that order, filling the first pane.
    ///
    /// Nothing, while the applications are still being read. The reading says
    /// so when it lands and this is asked again, so the first pane fills a
    /// moment after the wallpaper rather than not at all -- and nothing is
    /// written down in the meantime, because a file written now is the file
    /// that says this machine has a home screen with nothing on it.
    fn first(self: &Rc<Screen>) -> Home {
        let apps = self.apps.borrow();
        let names: Vec<String> = apps.keys().cloned().collect();

        Home::first(&console_menu::counts::order(&names, &found::counted()), self.shape.get())
    }

    /// Write down what is on it.
    fn keep(self: &Rc<Screen>) {
        let at = console_home::file(&found::home());
        let said = self.home.borrow().written();

        // Nothing to write is either a machine whose applications have not
        // been read yet or one somebody has cleared, and only the second of
        // those is worth a file. Told apart by whether there is one already.
        if said.is_empty() && !at.exists() {
            return;
        }

        if std::fs::read_to_string(&at).is_ok_and(|before| before == said) {
            return;
        }

        if let Some(above) = at.parent() {
            let _ = std::fs::create_dir_all(above);
        }

        if let Err(fault) = std::fs::write(&at, said) {
            eprintln!("console-home: {}: {fault}", at.display());
        }
    }

    /// On the screen or put away, and holding the keyboard or not.
    ///
    /// Two answers out of one reading. A window on the workspace is somebody
    /// doing something, so the home screen goes away entirely -- it is what
    /// keeps A a click inside a game and what keeps the wallpaper's own
    /// reading of whether anything is in front of it true.
    ///
    /// The keys are a second question. The home screen is drawn under every
    /// panel on this machine, and a surface that held the keyboard while one
    /// was up would be a panel nothing could type into or press: Hyprland will
    /// not focus away from an exclusive layer. So the keys are taken only
    /// while there is nothing over it at all, and handed back the moment
    /// anything opens.
    fn settle(self: &Rc<Screen>) {
        let showing = match holds_a_window() {
            Holds::AWindow => Showing::No,
            Holds::Nothing => Showing::Yes,
        };

        // Said only when it changes. `settle` is asked on every window and
        // every layer, and presenting a surface that is already up is a commit
        // the compositor answers for -- which the keyboard coming up and going
        // down again is enough of to be felt.
        if self.settled.replace(Some(showing)) != Some(showing) {
            match showing {
                Showing::Yes => self.window.present(),
                Showing::No => self.window.set_visible(false),
            }
        }

        // The highlight belongs to a home screen that is the thing in front of
        // you. Behind a window or under a panel it is not, so it goes away --
        // and A goes back to the pointer with it.
        if (showing, anything_over_it()) != (Showing::Yes, Over::Nothing) {
            self.sleeps();
        }
    }
}

/// A picture for an application, or the room one would have taken.
fn drawn(at: Option<&str>, icon: i32) -> gtk4::Image {
    let held = gtk4::Image::new();
    held.set_widget_name("picture");
    held.set_pixel_size(icon);
    held.set_size_request(icon, icon);

    match at {
        Some(at) if !at.is_empty() => held.set_from_file(Some(at)),
        // The icon theme had nothing for it, which is a square with its name
        // on it and the room where the picture would be.
        _ => held.set_icon_name(Some("application-x-executable")),
    }

    held
}

/// Under everything, and anchored to the whole of the screen.
///
/// The bottom layer, which is above the wallpaper and below every window and
/// every panel. Anchored to all four edges and deaf to exclusive zones: the
/// two surfaces that claim one are the bar, whose rows are cleared by
/// [`CLEARED`] instead, and the keyboard -- and a surface that answered the
/// keyboard's was a home screen that shifted up whenever typing started in
/// front of it, and a grid that hopped when the squares themselves cannot be
/// typed into.
fn laid_under_everything(window: &Window) {
    window.init_layer_shell();
    window.set_namespace(Some(NAMESPACE));
    window.set_layer(Layer::Bottom);

    // Minus one is the layer shell's "do not move me for anybody": without it
    // the compositor subtracts every exclusive zone on the screen from this
    // surface, and the keyboard's is the whole bottom third.
    window.set_exclusive_zone(-1);
    window.set_margin(Edge::Top, CLEARED);

    // None, always, and this is the only place it is set. The keyboard is not
    // this surface's to take: the only interactivity that would reach a layer
    // drawn under everything is the exclusive one, and Hyprland gives an
    // exclusive layer every pointer and every touch on the screen -- which is
    // what a lock screen wants and what took every tap on the bar away for as
    // long as the home screen was drawn. What the pad did arrives over
    // `console_door::homeward` instead.
    window.set_keyboard_mode(KeyboardMode::None);

    for edge in [Edge::Bottom, Edge::Left, Edge::Right, Edge::Top] {
        window.set_anchor(edge, true);
    }
}

/// Whether the workspace being looked at has a window on it.
fn holds_a_window() -> Holds {
    let Ok(said) = std::process::Command::new("hyprctl").args(["activeworkspace", "-j"]).output()
    else {
        // No answer is not an empty workspace. Read as one, the home screen
        // would come up over whatever is running the moment the compositor
        // hiccupped.
        return Holds::AWindow;
    };

    let Ok(workspace) = serde_json::from_slice::<serde_json::Value>(&said.stdout) else {
        return Holds::AWindow;
    };

    match workspace.get("windows").and_then(serde_json::Value::as_i64).unwrap_or(1) {
        0 => Holds::Nothing,
        _ => Holds::AWindow,
    }
}

/// Whether anything that is not furniture is on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Over {
    Something,
    Nothing,
}

/// The surfaces that are not something in front of the home screen.
///
/// The same list the controller keeps, said from this side. The home screen is
/// in it: it is what the question is about.
const BEHIND: [&str; 7] = [
    "awww-daemon",
    "waybar",
    "updating",
    "virtual-keyboard",
    "notifications",
    "mako",
    NAMESPACE,
];

fn anything_over_it() -> Over {

    let Ok(screens) = console_panel::door::screens() else { return Over::Something };

    let over = screens
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| screen.get("levels")?.as_object())
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten()
        .filter_map(|surface| surface.get("namespace")?.as_str())
        .any(|named| !BEHIND.iter().any(|behind| named.starts_with(behind)));

    match over {
        true => Over::Something,
        false => Over::Nothing,
    }
}

/// Whether what the compositor said means anything here.
///
/// Windows and layers both. A window opening is what puts the home screen
/// away, and a layer opening is what makes it let go of the keyboard, so this
/// is a wider net than the one `console_door` casts for the bar.
fn worth_asking_after(line: &str) -> Over {
    const ASK: [&str; 8] = [
        "openwindow>>",
        "closewindow>>",
        "movewindow>>",
        "workspace>>",
        "focusedmon>>",
        "openlayer>>",
        "closelayer>>",
        "fullscreen>>",
    ];

    match ASK.iter().any(|word| line.starts_with(word)) {
        true => Over::Something,
        false => Over::Nothing,
    }
}

/// Listen for what the pad did, for as long as this is running.
///
/// A datagram socket, so there is no connection to lose and nothing to reopen:
/// the daemon writes a word or it does not, and a word said while this was
/// starting is a word dropped rather than a daemon left waiting. Which is the
/// right trade for a button -- a press that arrived late is worse than a press
/// that did not arrive.
fn listening(screen: &Rc<Screen>) {
    let at = match console_door::homeward() {
        Ok(at) => at,
        Err(fault) => {
            eprintln!("console-home: nothing can be said to me: {fault}");

            return;
        },
    };

    if let Some(above) = at.parent() {
        let _ = std::fs::create_dir_all(above);
    }

    // A name left behind by a home screen that is no longer running is a name
    // nothing is listening on, and binding will not take its place. Whoever is
    // running is the one that answers.
    let _ = std::fs::remove_file(&at);

    let socket = match UnixDatagram::bind(&at) {
        Ok(socket) => socket,
        Err(fault) => {
            eprintln!("console-home: {}: {fault}", at.display());

            return;
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

            let Ok((held, said, got)) = heard else { return };

            socket = held;

            let got = match got {
                Ok(got) => got,
                // The socket itself has gone wrong, which is not something
                // reading it again would fix. Reading it again in a loop is
                // how a daemon spins at a hundred percent saying nothing.
                Err(fault) => {
                    eprintln!("console-home: nothing more can be said to me: {fault}");

                    return;
                },
            };

            // Not a word we said, and so not a word meant for us. The
            // datagram is a public address in the runtime directory and
            // anything at all may write to it; a home screen that stopped
            // listening because something wrote rubbish to it would be one
            // anything could switch off.
            let said = match std::str::from_utf8(&said[..got]) {
                Ok(word) => Said::read(word),
                Err(fault) => {
                    eprintln!("console-home: something said {got} bytes that are not words: {fault}");

                    None
                },
            };

            let Some(said) = said else { continue };

            screen.told(said);
        }
    });
}

/// Follow the compositor for as long as this is running.
///
/// The connection is made again whenever it ends, for the reason
/// `console_door::watching_layers` gives: the socket is not there yet when the
/// desktop is coming up, and it goes away under a resume. A home screen that
/// gave up on it would be one that stopped hiding for windows and would look
/// like a home screen drawn over a game.
fn following(screen: &Rc<Screen>) {
    let screen = Rc::clone(screen);
    glib::spawn_future_local(async move {
        loop {
            let Ok(socket) = console_panel::door::events() else { return };

            let opened = gtk4::gio::spawn_blocking(move || UnixStream::connect(&socket)).await;

            let Ok(Ok(stream)) = opened else {
                // Not there, or not there yet. Asked again after a moment,
                // and the answer is looked at again meanwhile: what happened
                // while nothing was listening was said to nobody.
                screen.settle();
                glib::timeout_future(std::time::Duration::from_secs(2)).await;
                continue;
            };

            let mut lines = BufReader::new(stream);
            screen.settle();

            loop {
                let read = gtk4::gio::spawn_blocking(move || {
                    let mut said = String::new();

                    let Ok(got) = lines.read_line(&mut said) else { return (lines, said, 0) };

                    (lines, said, got)
                })
                .await;

                let Ok((held, said, got)) = read else { return };

                if got == 0 {
                    break;
                }

                lines = held;

                if worth_asking_after(&said) == Over::Something {
                    screen.settle();
                    // A card that put something on the home screen has just
                    // closed, which is a layer closing and nothing else.
                    screen.reread();
                }
            }
        }
    });
}

/// How the shape is asked for, which is a file under her own home.
///
/// A machine that has never been asked, or a home nothing will name, gets the
/// grid the home screen was written as. Said out loud only when the file is
/// there and will not be read, because a missing file is the ordinary case and
/// a home screen that complained about it on every boot would be a line in the
/// log that means nothing.
fn asked_shape() -> Shape {
    let at = shape::at(&found::home());

    match std::fs::read_to_string(&at) {
        Ok(said) => Shape::read(&said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Shape::USUAL,
        Err(fault) => {
            eprintln!("console-home: {}: {fault}", at.display());

            Shape::USUAL
        },
    }
}

fn main() {
    if let Err(fault) = gtk4::init() {
        eprintln!("console-home: no screen to draw on: {fault}");
        return;
    }

    let waiting = glib::MainLoop::new(None, false);
    let screen = Screen::new();

    console_panel::asked::stops_when_asked({
        let waiting = waiting.clone();
        move || waiting.quit()
    });

    // What was written down last time, which is on the screen in the moment
    // the surface is. The machine itself is read behind that: a home screen
    // that waited for every desktop file on the machine to be opened would be
    // a wallpaper for the first second of every boot.
    let found = found::remembered();
    *screen.apps.borrow_mut() = named(found);
    screen.reread();
    screen.settle();

    // Then the machine, in two goes. What applications there are is a few
    // hundred small files and lands in a moment; a picture for each of them is
    // a walk of every icon directory there is, and on a first boot that is
    // seconds. Done as one, the home screen is a wallpaper for those seconds
    // on the one boot where somebody is certainly looking at it.
    let reading = Rc::clone(&screen);
    glib::spawn_future_local(async move {
        if let Ok(found) = gtk4::gio::spawn_blocking(found::quickly).await {
            *reading.apps.borrow_mut() = named(found);
            reading.reread();
            // The placements have not moved, and the pictures are new: what
            // `reread` found nothing to do about is drawn here.
            reading.draw();
        }

        let Ok(found) = gtk4::gio::spawn_blocking(found::machine).await else { return };

        *reading.apps.borrow_mut() = named(found);
        reading.reread();
        reading.draw();
    });

    listening(&screen);
    following(&screen);

    // Asleep, however it was left. A home screen that fell over holding a
    // highlight and came back saying it still had one would be a home screen
    // that had taken A from a pointer nobody had moved.
    if let Err(fault) = console_door::waking(Awake::No) {
        eprintln!("console-home: nobody was told it is asleep: {fault}");
    }

    waiting.run();
}

/// The applications by name, each with its picture.
fn named(found: found::Found) -> BTreeMap<String, (Application, String)> {
    found
        .apps
        .into_iter()
        .map(|(name, app)| {
            let picture = found.icon.get(&name).cloned().unwrap_or_default();

            (name, (app, picture))
        })
        .collect()
}
