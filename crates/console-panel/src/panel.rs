//! The panel, drawn.
//!
//! Nothing here decides anything that could be decided somewhere quieter: how
//! many tabs the strip has room for is `strip`, how tall the card should be is
//! `fitting`, and what a button means is `keys`. This puts what they answer on
//! the screen.


use console_number::{fitted, toward_zero_i32};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CssProvider, Entry, EventControllerKey,
    EventControllerMotion, GestureClick, Label, ListBox, ListBoxRow, Orientation, Overlay,
    PolicyType, PropagationPhase, ScrolledWindow, Window,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::keys::{Driving, Meaning, meaning};
use crate::marks::{self, named};
use crate::page::{
    Answer, Does, Heading, InEffect, Page, Picture, Row, Same, Showing, Stirred, Taken,
};
use crate::strip::{ANSWER, EDGE, GAP, MARGIN, PICTURE, PRESSED, SLEEVE};

/// Nearer than this and the pointer did not move: the list moved under it.
const A_HAIR: f64 = 0.5;

/// What stands between a question and its answers.
const BREATH: i32 = 14;

/// How long a word in the corner stays there.
///
/// Long enough to be read by somebody who was looking at the row they pressed
/// rather than at the corner, and short enough to be gone before anybody
/// wonders whether the panel is waiting to be answered. It says what has been
/// set going, so it has nothing to wait for and nothing to be dismissed.
const A_MOMENT: std::time::Duration = std::time::Duration::from_secs(6);

/// How wide a word in the corner is let run before it wraps, in characters.
///
/// A sentence and not a paragraph. A note says one thing that has been set
/// going, and anything needing more room than this is something that belongs
/// on the panel rather than beside it.
const NOTE_WIDE: i32 = 34;

/// The gap between the strip and the first row, in points.
///
/// Smaller than the margins round the sides, because the strip above it
/// already stands off the rows on padding of its own.
const OVER_ROWS: i32 = 10;

/// Room kept at either end of a seek bar, in characters.
///
/// Enough for `1:03:20`, which is a mix or a set and is the longest anything
/// in a music folder gets. Held rather than sized to what it says, or the bar
/// between the two would shift a pixel every time a digit changed width.
const TIME_WIDE: i32 = 7;
use crate::{asked, chooser, fitting, opening, running, strip, style};

/// Where the pages come from.
///
/// Handed a function rather than a list, a panel redraws itself when something
/// changes rather than going stale.
pub type Build = Arc<dyn Fn() -> Vec<Page> + Send + Sync>;

/// What is known between one draw and the next.
struct State {
    pages: Vec<Page>,
    /// The tab in front, and the row being stood on.
    here: usize,
    at: usize,
    /// Whether the thumb is on the way out rather than on the strip.
    ///
    /// The × is the last place along the top of the panel, one press of a
    /// shoulder past the last tab, and A there closes the card. Which tab is in
    /// front does not change while somebody stands on it: the way out is a
    /// place to be, not a tab, and stepping back off it leaves the panel
    /// exactly where it was.
    out: bool,
    /// Whether the card has been opened out to fill the screen.
    ///
    /// One picture is what a viewer is for, and the card it is drawn on is a
    /// share of the desktop with a strip of tabs over it -- which is right for
    /// a list of settings and is a frame round a photograph somebody wanted to
    /// look at. Opened out, the card is the screen: no strip, no desktop down
    /// the sides, and every point the card gained goes to the picture.
    ///
    /// The rows under it stay. They are what the film is started and walked
    /// with, and a full screen with no way to press play on it is a picture of
    /// a film rather than one being watched. What takes them away is the tab
    /// itself, once nobody has pressed anything for a while, and the same press
    /// that brings them back is spent doing only that.
    opened: Opened,
    /// The leftmost tab the strip is showing, how wide the card came out this
    /// time, how wide the widest tab is, and what the row spends on things that
    /// are not tabs. All of them are worked out when there is something to
    /// measure.
    from_tab: usize,
    wide: i32,
    cell: Option<i32>,
    spent: i32,
    /// What is waiting on a line of text, while anything is.
    asking: Option<Answer>,
    /// The yes-or-no question that is up, while one is.
    sure: Option<Sure>,
    /// Where the pointer was on the screen the last time it said, so that a
    /// list moving under a pointer lying still is not read as a thumb.
    pointed: Option<f64>,
    /// Every word said in the corner is stamped, so the one that takes a note
    /// down is the one that put it there. Two presses inside the six seconds
    /// would otherwise leave the first one's timer taking the second one's
    /// word off the screen a moment after it arrived.
    noted: u64,
    /// Every reading is stamped. Pressing along the strip faster than the
    /// machine answers leaves earlier readings arriving after later ones, and
    /// an answer about a tab you have already left is a wrong answer however
    /// true it was when it was asked.
    reading: u64,
    /// The stamp of the last reading that made it onto the card.
    ///
    /// What the stamp is compared against. A reading used to land only if it
    /// was the latest one asked for, which under a held d-pad is nearly never:
    /// every repeat asked again, every answer arrived already outrun, and the
    /// card stood still while the folder under it walked fifty files. A
    /// reading is read from the machine as it is at that moment, so one that
    /// has been outrun is still newer than whatever is drawn -- the only
    /// reading with nothing to say is one that arrives behind a reading
    /// already placed.
    landed: u64,
    asked: Option<(i32, i32)>,
    /// What each tab said last time, so coming back to one shows it at once and
    /// corrects itself a moment later rather than blinking empty.
    remembered: BTreeMap<usize, Vec<Row>>,
    /// The rows on the tab as it stands, which is where a chosen row is looked
    /// up.
    placed: Vec<Row>,
    /// How many rows the tab in front has written under the one picture it is
    /// about.
    ///
    /// Counted from the rows rather than written down, because it is a number
    /// that changes while somebody is looking: a film gains a bar and both
    /// kinds put everything away when the card is left alone. What the picture
    /// may be drawn at is whatever those rows have not taken.
    under: i32,
    tabs: Vec<Button>,
    /// Every watcher started, so every one of them can be stopped. A panel is
    /// opened and closed dozens of times a day and each one used to leave its
    /// `pactl subscribe` behind, reparented to init and reading a pipe nobody
    /// holds. Twenty-five of them were found alive on the device, the oldest
    /// four hours old, which on a handheld is battery.
    watchers: Vec<Child>,
    reshaping: bool,
    due: bool,
}

/// Whether the card is filling the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    Out,
    No,
}

/// A question and its answers, while they are on the screen.
struct Sure {
    then: Taken,
    /// Which answer is standing. No is nought and is where it opens; the ones
    /// that do something follow, in the order they were given.
    at: usize,
    answers: Vec<Button>,
}

pub struct Panel {
    build: Build,
    /// How wide the first words are held, for a page that reads as two columns
    /// rather than as a list of things to pick.
    column: i32,
    /// As tall as the panel may get before it scrolls instead.
    /// What is running while the panel is up.
    over: glib::MainLoop,

    window: Window,
    card: GtkBox,
    /// A word in the corner, while there is one.
    note: Label,
    /// The line to type into, and the row it lives in. Shown only on a tab
    /// that asked for one, and never rebuilt: the list is rebuilt on every
    /// letter, and a widget that is unparented loses the focus and hands the
    /// next letter to nothing.
    search: Entry,
    seeker: ListBoxRow,
    top: GtkBox,
    less: Button,
    more: Button,
    shut: Button,
    scroller: ScrolledWindow,
    rows: ListBox,

    state: RefCell<State>,
}

impl Showing for Rc<Panel> {
    fn refresh(&self) {
        self.redraw();
    }

    fn replace(&self, standing_on: usize) {
        {
            let mut state = self.state.borrow_mut();
            let here = state.here;
            state.remembered.remove(&here);
            state.at = standing_on;
        }

        self.redraw();
    }

    /// The line is emptied rather than told to be empty: what that leaves
    /// behind goes to the tab the same way a rubbed-out letter does, so a tab
    /// hears one thing whether the word went because she took it back or
    /// because the search was over.
    fn forget_typing(&self) {
        self.search.set_text("");
    }

    fn ask(&self, question: &str, then: Answer) {
        self.asking(question, then, Secret::Yes);
    }

    fn ask_aloud(&self, question: &str, then: Answer) {
        self.asking(question, then, Secret::No);
    }

    fn sure(&self, question: &str, about: &str, does: &[&str], then: Taken) {
        self.wondering(question, about, does, then);
    }

    fn later(&self, argv: Vec<String>) {
        Panel::later(self, argv);
    }

    fn leave_running(&self, argv: Vec<String>) {
        Panel::leave_running(self, argv);
    }

    fn note(&self, said: &str) {
        Panel::note(self, said);
    }

    fn open_out(&self) {
        Panel::open_out(self);
    }

    fn turn_to(&self, tab: usize) {
        let last = self.state.borrow().pages.len().saturating_sub(1);
        self.state.borrow_mut().out = false;
        self.went_to(tab.min(last));
    }
}

impl Panel {
    /// A panel, built and drawn but not yet shown.
    pub fn new(
        build: Build,
        column: i32,
        start: Option<&str>,
        over: glib::MainLoop,
    ) -> Rc<Self> {
        let pages = build();
        // Named, that tab. Not named, the tab it was left on, so opening the
        // settings for the Wi-Fi twice running does not go by way of the
        // battery twice running. Where the highlight was is not kept with it:
        // the row you want is rarely the row you left, and the tab is the part
        // that is tedious to walk back to.
        let left_on = crate::tab::last(&namespace());
        let here = crate::page::find(&pages, start.or(left_on.as_deref()));

        if let Some(page) = pages.get(here) {
            crate::tab::keep(&namespace(), &page.title);
        }

        let window = Window::new();
        laid_over_everything(&window);

        let card = GtkBox::new(Orientation::Vertical, 0);
        card.set_widget_name(named::CARD);
        card.set_halign(Align::Center);
        card.set_valign(Align::Center);

        // A word in the corner of the screen, over the card rather than in it.
        //
        // What it says is about something that has been set going off the
        // panel: a wallpaper being put up, a picture being pressed. Those take
        // seconds to minutes, and done where the panel is drawn they stop it
        // answering the buttons for that whole time, which reads as a machine
        // that has crashed. So they are done off to one side and this is what
        // says so, and the panel goes on being a panel while it happens.
        //
        // Here rather than through the notification daemon: this surface is a
        // layer over everything, so a notification would be drawn behind the
        // very panel the press was made on.
        let note = Label::new(None);
        note.set_widget_name(named::NOTE);
        note.set_halign(Align::End);
        note.set_valign(Align::End);
        note.set_margin_end(fitting::BREATH);
        note.set_margin_bottom(fitting::BREATH);
        note.set_wrap(true);
        note.set_max_width_chars(NOTE_WIDE);
        note.set_visible(false);

        let over_the_card = Overlay::new();
        over_the_card.set_child(Some(&card));
        over_the_card.add_overlay(&note);
        window.set_child(Some(&over_the_card));

        // The strip, and beside it the way out. They are held in one row so
        // that the panel is as tall as the taller of the two, whichever that
        // turns out to be once the font is known.
        let top = GtkBox::new(Orientation::Horizontal, 0);
        top.set_widget_name(named::TOP);
        card.append(&top);

        let less = arrow(marks::BEFORE);
        less.set_margin_end(GAP);
        top.append(&less);

        // The tabs share the width between them. There is room for it, and a
        // tab the width of its word is a small thing to hit with a thumb.
        //
        // Between the two arrows, because the panel is one width whatever is
        // written on its tabs. Given the whole row, a strip of five long words
        // asked for more than the card was ever going to be and got it: the
        // guide was wider than the settings it sits beside, and how much wider
        // depended on the longest heading anybody had written that week.
        let strip = GtkBox::new(Orientation::Horizontal, GAP);
        strip.set_widget_name(named::STRIP);
        strip.set_homogeneous(true);
        strip.set_hexpand(true);
        top.append(&strip);

        let more = arrow(marks::AFTER);
        more.set_margin_start(GAP);
        top.append(&more);

        // B closes a panel and a finger has no B. This is the same door said in
        // the other language, and it is the only one the bar's icons can reach:
        // they open a panel over a screen where nothing else answers.
        let shut = Button::with_label(marks::SHUT);
        shut.set_widget_name(named::SHUT);
        shut.set_margin_start(2 * GAP);
        top.append(&shut);

        // The first row of the list rather than a line above it. Above it,
        // the only way onto it was a pointer, and this machine's hands are on
        // a controller: the list could not be narrowed at all without one.
        //
        // Made once and kept. It is put into the list on a tab that asks for
        // one and taken out of it on a tab that does not, and in between the
        // list is emptied and filled again on every letter, which is why the
        // one row that must not be rebuilt is this one.
        let search = Entry::new();
        search.set_widget_name(named::SOUGHT);
        search.set_hexpand(true);

        let seeker = ListBoxRow::new();
        seeker.add_css_class("typing");
        seeker.set_child(Some(&search));

        let rows = ListBox::new();
        rows.set_widget_name(named::PANEL);
        rows.set_activate_on_single_click(true);

        // The rows are held off the border by the scroller's own margins rather
        // than by padding inside it. Padding scrolls away with the content, so
        // a long list ran its last row over the panel's edge and rubbed out the
        // line it was drawn in.
        let scroller = ScrolledWindow::new();
        scroller.set_policy(PolicyType::Never, PolicyType::Automatic);
        scroller.set_propagate_natural_height(false);
        scroller.set_margin_start(MARGIN);
        scroller.set_margin_end(MARGIN);
        scroller.set_margin_bottom(MARGIN);
        scroller.set_margin_top(OVER_ROWS);
        scroller.set_vexpand(true);
        scroller.set_child(Some(&rows));
        card.append(&scroller);

        let tabs: Vec<Button> = pages
            .iter()
            .map(|page| {
                let button = Button::with_label(&page.title);
                button.set_widget_name(named::TAB);
                button.set_hexpand(true);
                strip.append(&button);
                button
            })
            .collect();

        dressed();

        let panel = Rc::new(Panel {
            build,
            column,
            over,
            window,
            card,
            note,
            search,
            seeker,
            top,
            less,
            more,
            shut,
            scroller,
            rows,
            state: RefCell::new(State {
                pages,
                here,
                at: 0,
                out: false,
                opened: Opened::No,
                from_tab: 0,
                wide: 0,
                cell: None,
                spent: 0,
                asking: None,
                sure: None,
                pointed: None,
                noted: 0,
                reading: 0,
                landed: 0,
                asked: None,
                remembered: BTreeMap::new(),
                placed: Vec::new(),
                // The two a card about a picture began with, until a reading
                // has said otherwise. The card is drawn before its first
                // reading lands, and a picture given the whole card and then
                // taken back down is a jump somebody sees.
                under: 2,
                tabs,
                watchers: Vec::new(),
                reshaping: false,
                due: false,
            }),
        });
        panel.answers();
        panel.seeks();
        panel.watch_everything();

        // The room, before anything is drawn in it: the strip is measured
        // against the width the moment it is first marked.
        let wide = panel.across();
        panel.state.borrow_mut().wide = wide;
        // The card exists and has nothing on it. What follows is the rows, and
        // on a menu that is one picture opened per application installed, which
        // is the stretch worth telling from this one.
        opening::mark("built");
        panel.draw();
        panel.entered();

        // And the card is asked for its size here, before the window is put on
        // screen, rather than only on the first idle after it. Asked after, the
        // frame everybody sees first is the card at whatever size its own
        // contents wanted, and the size it is meant to be arrives visibly late:
        // the panel opens small and jumps. There is nothing granted to measure
        // against this early, which is what the remembered room is for.
        panel.fit();
        panel
    }

    /// The window, for whoever is holding this open.
    pub fn window(&self) -> &Window {
        &self.window
    }

    // -------------------------------------------------------------- answering

    fn answers(self: &Rc<Self>) {
        let keys = EventControllerKey::new();
        let panel = Rc::clone(self);
        keys.connect_key_pressed(move |_, key, _, _| panel.pressed(key));
        // Before the row that is standing there, not after it.
        //
        // A controller left where GTK puts it answers only what the focused
        // widget did not want, and a list wants the page keys: it moves its own
        // cursor by a screenful and says the key is spent. So the shoulders
        // turned the tabs while the panel had only just opened, and stopped the
        // moment the d-pad put the focus on a row, which is every time anybody
        // uses one. Reading the keys first costs nothing, because everything
        // this panel has no meaning for is passed straight on.
        keys.set_propagation_phase(PropagationPhase::Capture);
        self.window.add_controller(keys);

        // The pointer moves the highlight, so there is one answer to where you
        // are rather than two. A is a keypress and a keypress acts on what is
        // highlighted; without this, moving the pointer over a row and pressing
        // A chose whatever the highlight had been left on somewhere else.
        let pointer = EventControllerMotion::new();
        let panel = Rc::clone(self);
        pointer.connect_motion(move |_, _, y| panel.hovered(y));
        self.rows.add_controller(pointer);

        // A tap anywhere off the card puts the panel away.
        //
        // The surface is anchored to all four edges and sits over everything,
        // so it covers the bar, and a tap meant for the icon that opened the
        // panel never reaches the bar at all: the panel is in the way. Nothing
        // a finger could do put it away except the one small cross in the
        // corner. Now the tap the bar never got is the tap that closes it,
        // which is what pressing that icon a second time was for.
        let taps = GestureClick::new();
        let panel = Rc::clone(self);
        taps.connect_pressed(move |_, _, x, y| panel.tapped(x, y));
        self.window.add_controller(taps);

        let panel = Rc::clone(self);
        self.rows.connect_row_activated(move |_, row| panel.chose(row.index()));
        let panel = Rc::clone(self);
        self.rows.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                panel.state.borrow_mut().at = fitted(row.index().max(0));
            }
        });

        let panel = Rc::clone(self);
        self.shut.connect_clicked(move |_| panel.shut());
        let panel = Rc::clone(self);
        self.less.connect_clicked(move |_| panel.turn(-1));
        let panel = Rc::clone(self);
        self.more.connect_clicked(move |_| panel.turn(1));

        let buttons: Vec<Button> = self.state.borrow().tabs.clone();

        for (index, button) in buttons.into_iter().enumerate() {
            let panel = Rc::clone(self);
            button.connect_clicked(move |_| panel.went_to(index));
        }

        // The room changed shape: the keyboard went up, or came down. Nothing
        // is measured in the answer, because asking for a size from inside a
        // layout is asking GTK to lay out while it is laying out; it says the
        // answer is stale and the next idle moment works out what it should be.
        let panel = Rc::clone(self);
        self.window.connect_realize(move |window| {
            let Some(surface) = window.surface() else { return };

            let panel = Rc::clone(&panel);
            surface.connect_layout(move |_, _, _| panel.reshaped());
        });
    }

    fn pressed(self: &Rc<Self>, key: Key) -> glib::Propagation {
        let meaning = meaning(key, self.driving());

        // The tab hears it first, and may say the press was spent waking it.
        //
        // A card whose rows go away when it is left alone has the picture where
        // the transport used to be, so a press taken at face value acts on a
        // row nobody could see: the thumb reaching for pause fills the screen
        // with the film instead of stopping it. The press that brings the rows
        // back is the press that brings the rows back, and the next one is the
        // one it looked like.
        if meaning != Meaning::Nothing && self.stirred() == Stirred::Woke {
            self.refresh();

            return glib::Propagation::Stop;
        }

        match meaning {
            Meaning::Abandon => {
                self.state.borrow_mut().asking = None;
                self.left_alone();
            }
            Meaning::Choose if self.state.borrow().sure.is_some() => self.answered_sure(),
            // The way out is stood on, which the shoulders can now do. A on it
            // is the press that mark has always meant, said by a thumb instead
            // of a finger.
            Meaning::Choose if self.leaving() == Leaving::Yes => self.shut(),
            Meaning::Choose => {
                if let Some(row) = self.rows.selected_row() {
                    match self.typing_at(row.index()) {
                        // The way off the line and onto the first thing it has
                        // left standing, which is the whole of what a search
                        // box is for. What it lands on is read before it is
                        // taken, like every other row here.
                        Typing::Yes => self.walk(1),
                        Typing::No => self.chose(row.index()),
                    }
                }
            }
            Meaning::More => {
                self.came_back();
                self.offered();
            }
            Meaning::Nothing => return glib::Propagation::Proceed,
            Meaning::Nudge(step) if self.state.borrow().sure.is_some() => self.lean(step),
            Meaning::Nudge(step) => {
                self.came_back();
                self.nudge(step);
            }
            // Opened out, B is the way back to the card and not the way out of
            // the panel. A press that closed the whole thing would be a press
            // that punished looking at a photograph closely.
            Meaning::Shut if self.state.borrow().opened == Opened::Out => self.close_out(),
            Meaning::Shut => self.backed_out(),
            Meaning::Step(step) => {
                self.came_back();
                self.walk(step);
            }
            Meaning::Tab(step) => self.turn(step),
        }

        glib::Propagation::Stop
    }

    /// Ask the tab in front what this press was for, where it wants asking.
    ///
    /// Not while a question is up. A question replaces the rows with its own
    /// and is answered by pressing one of them, so a tab that swallowed the
    /// first press there would be a question that cannot be answered until it
    /// has been asked twice.
    fn stirred(self: &Rc<Self>) -> Stirred {
        let stirs = {
            let state = self.state.borrow();

            match state.asking.is_some() || state.sure.is_some() {
                true => None,
                false => state.pages.get(state.here).and_then(|page| page.stirs.clone()),
            }
        };

        match stirs {
            Some(stirs) => stirs(),
            None => Stirred::Awake,
        }
    }

    /// Whether a point is on the card rather than on the desktop showing round
    /// it.
    fn on_the_card(&self, x: f64, y: f64) -> On {
        let card = self.card.allocation();
        let (left, top) = (f64::from(card.x()), f64::from(card.y()));
        let inside = x >= left
            && x < left + f64::from(card.width())
            && y >= top
            && y < top + f64::from(card.height());

        match inside {
            true => On::TheCard,
            false => On::TheDesktop,
        }
    }

    /// A tap off the card is a tap on what the panel is covering, and there is
    /// only one thing it can mean.
    fn tapped(self: &Rc<Self>, x: f64, y: f64) {
        if self.on_the_card(x, y) == On::TheDesktop {
            self.shut();
        }
    }

    /// The pointer moves the highlight, and the cursor with it.
    ///
    /// Selecting a row and standing on it are two different things to GTK: the
    /// highlight is the selection and the keys act on the cursor. Moved apart,
    /// the pointer highlights one row and the d-pad carries on from another.
    ///
    /// Only when the pointer has actually moved. The d-pad scrolls the row it
    /// is going to into view, and a list scrolling under a pointer that is
    /// lying still reports motion exactly as a thumb does: the row under it
    /// changed. Acted on, that put the highlight back where the pointer was,
    /// and it did it hardest going up, because scrolling up carries the list
    /// down and leaves the pointer over a row below the one just chosen. The
    /// d-pad's up looked broken and its down looked fine, which is one bug
    /// wearing two faces.
    fn hovered(&self, y: f64) {
        // Where the pointer is on the screen rather than where it is in the
        // list, because the list is what moves. Asked of the window rather
        // than worked out from the scrollbar: the list slides under a still
        // pointer when it scrolls, and the whole card moves and changes height
        // when the on-screen keyboard takes the bottom of the screen. Only the
        // second of those is missing from the scrollbar, and it is the one that
        // moved the highlight a row down every time the keyboard came up and a
        // row back every time it went away.
        let still = self
            .rows
            .translate_coordinates(&self.window, 0.0, y)
            .map_or(y, |(_, on_screen)| on_screen);
        // The first word from the pointer is where it is, not that it moved. A
        // panel opens under a pointer that has been lying still since whatever
        // opened it was tapped, and a surface appearing under a still pointer
        // reports motion: the panel came up with the highlight on whatever row
        // happened to arrive under it rather than on its first row.
        let before = self.state.borrow().pointed;
        self.state.borrow_mut().pointed = Some(still);

        let Some(before) = before else { return };

        if (still - before).abs() < A_HAIR {
            return;
        }

        let Some(row) = self.rows.row_at_y(toward_zero_i32(y)) else { return };

        if self.rows.selected_row().as_ref() != Some(&row) {
            self.rows.select_row(Some(&row));
            self.seen(&row);
        }
    }

    fn chose(self: &Rc<Self>, index: i32) {
        // A row was tapped, which a finger can do while the thumb is standing
        // on the way out. Where the thumb is is stale the moment the panel is
        // acted on by hand, and left lit it would make the next A close the
        // card rather than take the row.
        self.came_back();

        let Ok(index) = usize::try_from(index) else { return };

        let does = self.state.borrow().placed.get(index).and_then(|row| row.does.clone());

        match does {
            // A press that lands on a row with nothing on it, said out loud.
            // Silence here is the worst answer a panel gives: the row is
            // highlighted, A is pressed, and what comes back is indis-
            // tinguishable from a panel that has crashed, from a program that
            // failed to start, and from a button that is not wired up. Which of
            // those it is has cost this desktop an evening more than once.
            None => eprintln!(
                "nothing happens on row {index} of {}: {:?}",
                self.state.borrow().placed.len(),
                self.state.borrow().placed.get(index).map(|row| row.says.clone())
            ),
            Some(Does::Call(act)) => {
                if act(self) {
                    self.shut();
                }
            }
            Some(Does::Run(argv)) => {
                running::left_running(&argv);
                self.shut();
            }
        }
    }

    /// What else can be done with the row being stood on.
    ///
    /// The row is asked rather than the tab, so the options are about the thing
    /// under the highlight and never about whatever was last selected. A row
    /// with nothing more to offer says nothing, which is why Y can mean this
    /// everywhere and still be silent on most of what it is pressed over.
    fn offered(self: &Rc<Self>) {
        let Some(row) = self.rows.selected_row() else { return };

        let Ok(index) = usize::try_from(row.index()) else { return };

        let more = self.state.borrow().placed.get(index).and_then(|row| row.more.clone());

        if let Some(more) = more {
            if more(self) {
                self.shut();
            }
        }
    }

    /// B, which is the way out of where you are before it is the way out of the
    /// panel.
    ///
    /// A tab that is somewhere gets the press first. Only when it says there
    /// was nowhere left to go does back mean the panel, so walking three
    /// folders deep and pressing B three times leaves you where you started
    /// rather than looking at the desktop.
    /// Draw this card on the whole screen.
    ///
    /// The rows are asked for again rather than kept, because how big a picture
    /// is drawn is a question about the card it is on and the answer has just
    /// changed. Nothing else about the page moves: the same rows, in the same
    /// order, with the thumb on the one it was on.
    fn open_out(self: &Rc<Self>) {
        if self.state.borrow().opened == Opened::Out {
            return;
        }

        self.state.borrow_mut().opened = Opened::Out;
        self.opened_out();
    }

    /// Put the card back on the desktop.
    fn close_out(self: &Rc<Self>) {
        if self.state.borrow().opened == Opened::No {
            return;
        }

        self.state.borrow_mut().opened = Opened::No;
        self.opened_out();
    }

    /// What either of those two has to do afterwards.
    ///
    /// The strip goes with the card: opened out there is one thing on the
    /// screen and it is not a page of a panel, so a row of tabs over it is a
    /// frame this was opened out to be rid of. The shoulders still work and
    /// still turn the page, which is the same rule as the × -- a part of the
    /// panel that cannot be seen is still a part of it.
    fn opened_out(self: &Rc<Self>) {
        let out = self.state.borrow().opened == Opened::Out;
        self.top.set_visible(!out);

        // The whole screen means the whole screen. The surface is anchored to
        // every edge but keeps out of the room other surfaces have reserved,
        // which is right for a card lying on the desktop and wrong here: it
        // left the bar standing down its edge, and a photograph opened out
        // beside a bar is a photograph in a frame again. Opened out, the one
        // reservation this surface honours is its own.
        self.window.set_exclusive_zone(match out {
            true => -1,
            false => 0,
        });

        // And nothing that says card: no border, no rounded corners, no
        // desktop showing round the edges, no margins holding the picture off
        // them. The stylesheet reads the same word off both, because the
        // window's ground and the card's dressing are its to paint.
        for dressed in [self.window.upcast_ref::<gtk4::Widget>(), self.card.upcast_ref()] {
            match out {
                true => dressed.add_css_class("out"),
                false => dressed.remove_css_class("out"),
            }
        }

        let margin = match out {
            true => 0,
            false => MARGIN,
        };

        self.scroller.set_margin_start(margin);
        self.scroller.set_margin_end(margin);
        self.scroller.set_margin_bottom(margin);
        self.scroller.set_margin_top(match out {
            true => 0,
            false => OVER_ROWS,
        });

        // A list is read from the top, so a list is laid from the top -- and a
        // screen holding one picture is not being read. Left where a list
        // lies, a photograph shorter than the screen sat against its top edge
        // with the whole of the spare room underneath it, which reads as a
        // picture that slipped rather than one being shown.
        self.rows.set_valign(match out {
            true => gtk4::Align::Center,
            false => gtk4::Align::Fill,
        });

        {
            let mut state = self.state.borrow_mut();
            // The size has to be asked for again, and `fit` will not ask twice
            // for what it thinks it already has.
            state.asked = None;
            // And the rows have to be built again rather than kept. They say
            // exactly what they said a moment ago -- the same file, the same
            // words -- so the panel would keep the widgets it had, which is
            // right on every other redraw and wrong on this one: how big the
            // picture is drawn is not something a row says, and the answer has
            // just changed.
            state.placed.clear();
            state.remembered.clear();
        }

        self.fit();
        self.redraw();
    }

    fn backed_out(self: &Rc<Self>) {
        let back = {
            let state = self.state.borrow();
            state.pages.get(state.here).and_then(|page| page.back.clone())
        };

        match back {
            Some(back) if !back(self) => (),
            _ => self.shut(),
        }
    }

    /// Left and right, on a row that carries a level.
    ///
    /// The reading is asked for again rather than worked out here. What a level
    /// is is the machine's answer, and a panel that adds a step to the number it
    /// drew last time is a panel that drifts away from the thing it claims to
    /// be showing.
    /// Move the highlight, and stand on what it moved to.
    ///
    /// The list would do this itself if a row had the focus, and on a panel
    /// that has just opened none has: the focus is the list's own, and the
    /// d-pad did nothing until a finger touched the screen and gave a row the
    /// focus by hovering it. Doing it here also keeps the highlight and the
    /// cursor together, which is the same reason the pointer moves both.
    fn walk(self: &Rc<Self>, step: i32) {
        let now = self.rows.selected_row().map_or(0, |row| row.index());
        // Worked out and the state given back before anything is selected:
        // selecting a row calls back in to remember where the highlight is, and
        // a borrow still alive at that point is a panel that dies as it walks.
        let at = walked(&self.state.borrow().placed, now, step);

        let Some(going) = self.rows.row_at_index(at) else { return };

        self.rows.select_row(Some(&going));
        self.seen(&going);
    }

    fn nudge(self: &Rc<Self>, step: i32) {
        let Some(row) = self.rows.selected_row() else { return };

        let Ok(index) = usize::try_from(row.index()) else { return };

        let level = self.state.borrow().placed.get(index).and_then(|row| row.level.clone());

        if let Some(level) = level {
            level(step);
            self.redraw();
        }
    }

    /// One press of a shoulder, along the top of the panel.
    ///
    /// The tabs, and then the way out, which is a place like they are. Nothing
    /// is read and nothing is drawn to arrive on it: the tab that was in front
    /// stays in front and stays on the screen, and the only thing that has
    /// moved is where the thumb is.
    fn turn(self: &Rc<Self>, step: i32) {
        let going = {
            let state = self.state.borrow();
            let from = match state.out {
                true => strip::Stop::Out,
                false => strip::Stop::Tab(state.here),
            };
            strip::along(state.pages.len(), from, step)
        };

        match going {
            strip::Stop::Out => {
                self.state.borrow_mut().out = true;
                self.mark();
            }
            // Back onto the tab that was already in front, which is what one
            // press left off the way out is. There is nothing to draw and the
            // strip is marked again all the same, because where the thumb is
            // has moved even though what is on the screen has not.
            strip::Stop::Tab(index) => {
                let front = {
                    let mut state = self.state.borrow_mut();
                    state.out = false;
                    index == state.here
                };

                match front {
                    true => self.mark(),
                    false => self.went_to(index),
                }
            }
        }
    }

    /// Whether the thumb is on the way out.
    fn leaving(&self) -> Leaving {
        match self.state.borrow().out {
            true => Leaving::Yes,
            false => Leaving::No,
        }
    }

    /// Off the way out and back into the list.
    ///
    /// Everything but the shoulders and A belongs to the list, so the first
    /// press of anything else hands the panel back to it. Without this the ×
    /// stays lit while the d-pad walks the rows, and then A closes the panel
    /// under a thumb that was choosing one, which is the worst press on here to
    /// meet by accident: what was being read goes, and nothing says why.
    fn came_back(&self) {
        let was = std::mem::replace(&mut self.state.borrow_mut().out, false);

        if was {
            self.mark();
        }
    }

    fn went_to(self: &Rc<Self>, index: usize) {
        if index == self.state.borrow().here {
            return;
        }

        self.say_which_tab(index);

        {
            let mut state = self.state.borrow_mut();
            state.here = index;
            state.at = 0;
        }

        // At its head, which is where the highlight has just been put. A tab is
        // arrived on rather than redrawn, and the place a drawing keeps is the
        // place on the tab you were already standing on: kept across a shoulder
        // it would open the next tab as far down as the last one was left,
        // with the highlight on a first row nobody can see. Stood back here
        // rather than after the drawing, so the drawing keeps this instead.
        self.scroller.vadjustment().set_value(0.0);

        let title = self.state.borrow().pages.get(index).map(|page| page.title.clone());

        if let Some(title) = title {
            crate::tab::keep(&namespace(), &title);
        }

        self.draw();
        self.entered();
    }

    // ---------------------------------------------------------------- drawing

    /// Move to the tab, then go and find out what is on it.
    ///
    /// The tab moves on the press and nothing waits for anything. Reading a tab
    /// means asking the machine, and asking the machine is quick rather than
    /// instant: held behind it, the strip answers late enough that a second
    /// press feels like it was not noticed, and the way to press a button that
    /// ignores you is to press it again.
    fn draw(self: &Rc<Self>) {
        if self.state.borrow().sure.is_some() {
            return;
        }

        self.mark();
        self.seeking();
        // What the tab said last time, or, on a tab that has not been up yet,
        // whatever it can say without asking the machine anything.
        let (said_before, meanwhile) = {
            let state = self.state.borrow();
            let before = state.remembered.get(&state.here).cloned();
            let meanwhile = state.pages.get(state.here).and_then(|page| page.meanwhile.clone());
            (before, meanwhile)
        };
        let showing = said_before.or_else(|| meanwhile.map(|at_once| at_once())).unwrap_or_default();
        self.place(showing);
        self.fill();
    }

    /// Ask for the rows again, in case they say something else now.
    fn redraw(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            state.pages = (self.build)();
            state.here = state.here.min(state.pages.len().saturating_sub(1));
        }

        self.draw();
    }

    /// What a tab wants done on arriving, if anything.
    fn entered(self: &Rc<Self>) {
        let arriving = {
            let state = self.state.borrow();
            state.pages.get(state.here).and_then(|page| page.entered.clone())
        };

        if let Some(arriving) = arriving {
            arriving(self);
        }
    }

    /// Which tab is in front, and which tabs the strip has room for.
    ///
    /// Nothing here asks the machine anything.
    fn mark(&self) {
        let showing = {
            let mut state = self.state.borrow_mut();
            let cell = self.measure(&mut state);
            let room = strip::room(state.wide, state.spent);
            let fits = strip::fits(room, cell);
            let showing = strip::showing(state.pages.len(), state.here, state.from_tab, fits);
            state.from_tab = showing.start;
            showing
        };
        let state = self.state.borrow();

        for (index, button) in state.tabs.iter().enumerate() {
            button.set_visible(showing.contains(&index));

            // Which tab is in front, and whether the thumb is on it. They are
            // the same thing until somebody walks one press past the last tab,
            // and then the tab in front is still open and is no longer where
            // you are standing: it says so quietly, and the highlight goes
            // with the thumb onto the ×. Two pinks at the top of a card is the
            // panel asking which of them A is about.
            match (index == state.here, state.out) {
                (true, false) => {
                    button.add_css_class("here");
                    button.remove_css_class("open");
                }
                (true, true) => {
                    button.add_css_class("open");
                    button.remove_css_class("here");
                }
                _ => {
                    button.remove_css_class("here");
                    button.remove_css_class("open");
                }
            }
        }

        match state.out {
            true => self.shut.add_css_class("here"),
            false => self.shut.remove_css_class("here"),
        }

        // The way to what is not on the strip. There is no arrow towards
        // nothing: at the first tab the left one is a button that would do what
        // pressing it does anyway, which is nothing at all.
        self.less.set_visible(showing.start > 0);
        self.more.set_visible(showing.end < state.pages.len());
    }

    /// How wide the longest tab is, and how much of the row is not tabs.
    ///
    /// Asked once, while every one of them is still on the screen. A widget
    /// that has been hidden says it is nothing wide, and both arrows are hidden
    /// as often as they are drawn, so asking again later would answer with
    /// whatever the strip happens to be showing and change the count each time.
    fn measure(&self, state: &mut State) -> i32 {
        if let Some(cell) = state.cell {
            return cell;
        }

        state.spent =
            [&self.less, &self.more, &self.shut].iter().map(|button| wide_as(*button)).sum();
        let cell = state.tabs.iter().map(wide_as).max().unwrap_or(0);
        state.cell = Some(cell);
        cell
    }

    /// Read the tab somewhere else, and take the answer if it is still wanted.
    fn fill(self: &Rc<Self>) {
        let (stamp, here, rows, tab) = {
            let mut state = self.state.borrow_mut();
            state.reading += 1;
            let rows = state.pages.get(state.here).map(|page| page.rows.clone());
            let tab = state.pages.get(state.here).map(|page| page.title.clone());
            (state.reading, state.here, rows, tab.unwrap_or_default())
        };

        let Some(rows) = rows else { return };

        // How long the tab stood as it was last time before the machine
        // answered. It is the other half of an opening: the card is up in the
        // time the first line says, and this is how long what is on it is a
        // reading from before the press.
        //
        // Written only where somebody could have seen it. Every letter typed
        // into the menu reads the list again, and most of those are back inside
        // a frame.
        let mut waiting = console_timings::Waiting::here(&namespace(), "list");
        waiting.named("tab", &tab);
        let panel = Rc::clone(self);
        glib::spawn_future_local(async move {
            let read = match gtk4::gio::spawn_blocking(move || rows.read()).await {
                Ok(read) => read,

                Err(_) => {
                    eprintln!("console: the rows of this tab were not read; it is drawn empty");
                    Default::default()
                }
            };

            waiting.mark("read");
            let many = fitted::<usize, u64>(read.len());
            panel.arrived(stamp, here, read);
            waiting.mark("placed");
            waiting.counted("rows", many);
            waiting.done_if_felt();
        });
    }

    fn arrived(self: &Rc<Self>, stamp: u64, here: usize, rows: Vec<Row>) {
        {
            let mut state = self.state.borrow_mut();

            // Behind a reading already placed, which is the one arrival with
            // nothing to add. Being outrun is not that: a reading asked for
            // before the latest press still read the machine as it was when it
            // ran, so it is fresher than the card and goes on. Dropping those
            // too was a card that never moved while a d-pad was held on it,
            // because every repeat outran the answer to the one before.
            if stamp <= state.landed || state.asking.is_some() || state.sure.is_some() {
                return;
            }

            state.landed = stamp;
            state.remembered.insert(here, rows.clone());

            // Read for a tab that is no longer in front. Remembered, so coming
            // back to it opens on this rather than on something older, and not
            // placed: what is on the card is the tab that is.
            if here != state.here {
                return;
            }
        }

        // What this tab actually asked for, made for the next opening. Here
        // rather than off the remembered rows, because these are the machine as
        // it is: an application installed since the last opening is a picture
        // nothing has made yet, and it is on this list and not on that one.
        let wanted: Vec<String> = rows
            .iter()
            .filter_map(|row| match &row.picture {
                Picture::At(path) => path.to_str().map(str::to_string),
                _ => None,
            })
            .collect();
        crate::pictures::make(&crate::pictures::missing(&wanted));
        self.place(rows);
    }

    /// Put rows on the tab, keeping where you were standing.
    ///
    /// The line to type into is the first of them on a tab that asks for one.
    /// It is kept across the emptying rather than made again, so that the
    /// letter being typed goes on going where it was going: every letter
    /// narrows the list, narrowing the list draws it again, and a line drawn
    /// again is a line that has lost the focus and the word in it.
    fn place(self: &Rc<Self>, mut rows: Vec<Row>) {
        // The same list as the one already on the card, which is what nearly
        // every second drawing of a tab is: the applications have not changed
        // since the menu was last opened, and the reading that lands a moment
        // after the card does arrives at the rows that are already there. Every
        // one of them was being taken off and built again to get back to that.
        //
        // The rows themselves are still kept, because they are not the same
        // objects: what a row does is a closure made by the reading, and A on a
        // row has to run this reading's and not the one from before the press.
        let seeking = self.seeks_here() == Seeks::Yes;

        // How much of the card the picture may have, worked out before any of
        // it is drawn. A row is measured against this the moment it is built,
        // so a tab that has just put its controls away has to have said so
        // before the picture asks how tall it is.
        self.state.borrow_mut().under = under(&rows);

        // A film is kept going here rather than where it is drawn, and this is
        // the whole reason it is a separate call.
        //
        // Everything else on a card is read and drawn: the panel asks the tab
        // what its rows say, and where they say what they said last time it
        // leaves the widgets alone. A film is not read, it runs -- and asking
        // the decoder where it has got to was happening in the drawing, which
        // only happens when a row has changed. So the clock could not move
        // until the clock had moved. The card came up on the first frame and
        // stayed there, looking exactly like a film that would not open.
        keep_films_going(&rows);

        if self.same_as_drawn(&rows) == Same::Yes {
            // The line to type into is one of the rows the panel keeps, because
            // what is standing on the list is looked up by the same number the
            // list box answers with. Left out of the kept rows and left in the
            // widgets, every row would answer for the one above it.
            if seeking {
                rows.insert(0, Row::line_to_type_in());
            }

            let mut state = self.state.borrow_mut();
            state.placed = rows;
            return;
        }

        // Where the list stood before it is taken down. Emptying the scroller
        // stands it back at the top, so a press of left on a row half way down
        // a tab was answered by the whole tab jumping to its head -- the row
        // changed its reading, the reading changed the rows, and the drawing
        // forgot where anybody was. Most redrawings are the same list with a
        // number changed on one row, so the place itself is what is kept, and
        // it is put back off an idle rather than here: set in the middle of the
        // teardown it would be clamped against a list that measures empty.
        let stood = self.scroller.vadjustment().value();

        emptied(&self.rows, seeking.then_some(&self.seeker));

        if seeking {
            rows.insert(0, Row::line_to_type_in());

            if self.seeker.parent().is_none() {
                self.rows.prepend(&self.seeker);
            }
        }

        for row in rows.iter().skip(usize::from(seeking)) {
            let held = ListBoxRow::new();

            if row.now() == InEffect::Yes {
                held.add_css_class("now");
            }

            // Nothing happens to it, so nothing about it is offered to a hand:
            // the d-pad walks past it and a tap slides off it. Said to GTK as
            // well as worked out here, because the pad is not the only thing
            // that picks a row and a finger would otherwise land where the
            // highlight cannot.
            if row.heading() == Heading::Yes {
                held.set_activatable(false);
                held.set_selectable(false);
            }

            if row.naming {
                held.add_css_class("naming");
            }

            if row.middle {
                held.add_css_class("middle");
            }

            // A row that is a seek bar is not a card in a list either: it is a
            // strip of the card it sits on, the way the transport is.
            if matches!(row.picture, Picture::Bar(_)) {
                held.add_css_class("scrub");
            }

            if row.nothing {
                held.add_css_class("nothing");
            }

            if row.across.is_some() {
                held.add_css_class("transport");
            }

            // The row the one picture is on, named so that a card opened out
            // can take the card's ink off it and leave only the picture.
            if matches!(row.picture, Picture::Showing(_) | Picture::Playing(_)) {
                held.add_css_class("showing");
            }

            held.set_child(Some(&self.line(row)));
            self.rows.append(&held);
        }

        // Worked out while the state is already borrowed, and nothing held
        // afterwards. Selecting a row calls back into the panel, which borrows
        // the state again, so a borrow still alive at that point is a panel
        // that dies as it draws.
        let at = {
            let mut state = self.state.borrow_mut();
            state.placed = rows;
            let at = state.at.min(state.placed.len().saturating_sub(1));
            standing(&state.placed, at)
        };
        self.fit();

        // Stay where you were. A panel that redraws itself after every change
        // and drops you back at the top is a panel you cannot turn a volume up
        // in without counting rows again.
        //
        // Past a row nothing happens to, because a tab whose first row is a
        // heading opens with the highlight on a word, and the first press of A
        // on a panel that has just come up does nothing at all.
        if let Some(staying) = self.rows.row_at_index(at) {
            self.rows.select_row(Some(&staying));
        }

        // Where the list stood, put back before the layout this drawing has
        // asked for: the adjustment the layout reads is the one that decides
        // where the rows are drawn, and it is still the one the place was read
        // off, so the number goes back unclamped. A list that came back shorter
        // is clamped by the layout itself and stands as low as it still can.
        //
        // And nothing else. Holding the highlight in view was here too, and it
        // undid the line above it: a row that has not been laid out yet
        // allocates at nothing, and a row sitting at nothing is a row above the
        // fold of any list standing anywhere but its top, so a list at the
        // bottom was put back and then scrolled to the head again to show a row
        // that was already on the screen. That was the jump that survived
        // keeping the place, and why it was only ever met at the bottom. It is
        // gone rather than deferred to where the rows have places: a press of
        // left or right changes a reading, and a reading changing is no reason
        // for the list under the thumb to move at all. Up and down move the
        // highlight and take the list with them, a shoulder opens the next tab
        // at its head, and those are the two things that may move it.
        let panel = Rc::clone(self);
        glib::idle_add_local_once(move || {
            panel.scroller.vadjustment().set_value(stood);
        });

        // Whichever of the two is being stood on gets the letters. Standing on
        // the line, they are the line's; standing anywhere else, they are
        // nobody's and the pad is the list's.
        match self.typing_at(at) {
            Typing::Yes => self.typed_into(),
            Typing::No => {
                self.rows.grab_focus();
            }
        }

        // The rows are on the card. Every drawing after the first one says this
        // and nothing is written down for it: an opening is the first time
        // anybody saw the panel.
        opening::mark("placed");
    }

    /// Whether these rows are drawn on the card already.
    ///
    /// The line to type into is the panel's own and is never in the list this
    /// is asked about, so a tab that seeks is compared from the row after it.
    fn same_as_drawn(&self, rows: &[Row]) -> Same {
        let state = self.state.borrow();
        let drawn = &state.placed;
        let seeking = self.seeks_here() == Seeks::Yes;
        let from = usize::from(seeking && drawn.first().is_some_and(|row| row.typing));
        let alike = drawn.len() - from == rows.len()
            && drawn
                .iter()
                .skip(from)
                .zip(rows)
                .all(|(drawn, row)| drawn.looks_like(row) == Same::Yes);

        match alike {
            true => Same::Yes,
            false => Same::No,
        }
    }

    /// One row, laid out for reading or for picking.
    fn line(self: &Rc<Self>, row: &Row) -> GtkBox {
        let line = GtkBox::new(Orientation::Horizontal, 0);

        // The panel saying the list is empty. One line and nothing else: no
        // room kept at the front for a picture there will never be one of, and
        // none of the two-column measuring a tab of readings is given, because
        // it is not a reading of anything. Across the width and wrapped, since
        // what it says is a sentence and one of them names a folder.
        if row.nothing {
            let said = Label::new(Some(&row.says));
            said.set_hexpand(true);
            said.set_wrap(true);
            said.set_justify(gtk4::Justification::Center);
            line.append(&said);
            return line;
        }

        // A strip of presses across the row: the transport. Nothing else on
        // the row is drawn, because the row is the buttons -- there is no name
        // to write beside them and nothing to line up with the list above.
        if let Some(across) = &row.across {
            line.append(&self.strip(across));
            return line;
        }

        // A bar is the one picture with words either side of it rather than
        // words after it: how far in on the left and how long altogether on
        // the right, which is where a hand has read them on every player it
        // has held. Everything else keeps its picture at the front.
        if let Picture::Bar(bar) = &row.picture {
            let held = scrub(self, *bar, row.seek.clone());
            held.prepend(&edge(&row.says, 1.0));
            held.append(&edge(&row.aside, 0.0));
            line.append(&held);
            return line;
        }

        match &row.picture {
            Picture::None => {}
            Picture::Written(markup) => line.append(&written(markup)),
            Picture::Sleeve(art) => line.append(&sleeve(art.as_deref())),
            Picture::Showing(at) => {
                line.append(&showing(at.as_deref(), self.picture_room(), self.down()))
            }
            Picture::Playing(at) => {
                line.append(&playing(at.as_deref(), self.picture_room(), self.down()))
            }
            Picture::Bar(_) => {}
            picture => line.append(&shown(picture)),
        }

        // Up the middle rather than down the left edge. A list is read down
        // its left edge and a card about one song is not a list: the sleeve,
        // the title and the artist stack up the middle of it.
        if row.middle {
            // A gap between what is on it. Down the left edge the words are one
            // label after another with their own margins; centred they are two
            // labels butting, and a name run into the note about it -- the song
            // into the artist, the file into how big it is -- reads as one word
            // nobody wrote.
            let words = GtkBox::new(Orientation::Horizontal, 2 * GAP);
            words.set_halign(gtk4::Align::Center);
            words.set_hexpand(true);

            let said = Label::new(Some(&row.says));
            said.set_justify(gtk4::Justification::Center);
            said.set_wrap(true);

            if !row.says.is_empty() {
                words.append(&said);
            }

            if !row.aside.is_empty() {
                let note = Label::new(Some(&row.aside));
                note.set_widget_name(named::ASIDE);
                note.set_justify(gtk4::Justification::Center);
                words.append(&note);
            }

            // A middle row carrying a level is walked, and its two ends sit
            // where a finger looks for them: at either edge of the row, with
            // the name held between. On the viewer that is the one before this
            // file and the one after it, flanking which file this is.
            match &row.level {
                Some(level) => {
                    let (less, more) = ends_of(row);
                    line.append(&self.step(level.clone(), less, -1));
                    line.append(&words);
                    line.append(&self.step(level.clone(), more, 1));
                }
                None => {
                    line.set_halign(gtk4::Align::Center);
                    line.append(&words);
                }
            }

            return line;
        }

        let label = Label::new(Some(&row.says));
        label.set_xalign(0.0);
        // Cut short rather than allowed to push the panel wider. A network is
        // named by whoever set it up and one of them is always long enough to
        // make the settings a different size from everything else here.
        label.set_ellipsize(EllipsizeMode::End);

        // Two columns for a row that only tells you something. A row you can act
        // on is drawn like every other row you can act on, wherever it is, so
        // that what is clickable looks clickable.
        if self.column > 0 && row.does.is_none() {
            label.set_size_request(self.column, -1);
            line.append(&label);
            let said = Label::new(Some(&row.aside));
            said.set_widget_name(named::SAID);
            said.set_xalign(0.0);
            said.set_wrap(true);
            said.set_hexpand(true);
            line.append(&said);
            return line;
        }

        label.set_hexpand(true);
        line.append(&label);
        let (less, more) = ends_of(row);

        // A level is its two ends with the reading held between them, so the
        // mark that makes it smaller is on the side it shrinks towards and the
        // one that makes it bigger is on the side it grows into. Laid from the
        // right inward: the plus, the reading, the minus. An end with no mark
        // on it is not drawn at all -- the level is still the d-pad's, but the
        // row has put its presses somewhere better, the way the viewer's count
        // hands them to the name above it.
        if let Some(level) = &row.level {
            if !less.is_empty() {
                line.append(&self.step(level.clone(), less, -1));
            }
        }

        if !row.aside.is_empty() {
            let note = Label::new(Some(&row.aside));
            note.set_widget_name(named::ASIDE);

            if row.level.is_some() {
                // Room kept for the longest the reading gets, counted in
                // characters rather than in pixels so that it holds whatever
                // the font turns out to be. Left to size itself to what it says,
                // the two marks either side would move under the thumb every
                // time the number changed width.
                note.set_width_chars(16);
            }

            line.append(&note);
        }

        if let Some(level) = &row.level {
            if !more.is_empty() {
                line.append(&self.step(level.clone(), more, 1));
            }
        }

        // A picture at the other end. Most rows have their picture of themselves
        // on the left and nothing else; a few have a picture of the thing they
        // are about on the right, because the row is about what is beside it.
        if let Some(tail) = &row.tail {
            match tail {
                Picture::Written(markup) => line.append(&written(markup)),
                Picture::None => {}
                _ => line.append(&shown(tail)),
            }
        }

        // Last, at the edge the list goes deeper through. A label rather than
        // something to press: the whole row is already the way in, and a mark
        // that could be tapped on its own would be a second, smaller target for
        // the thing the row does anyway.
        if row.opens {
            let into = Label::new(Some(marks::INTO));
            into.set_widget_name(named::INTO);
            line.append(&into);
        }

        line
    }

    /// One end of a level, as something to press.
    /// A strip of presses, side by side and centred on their row.
    ///
    /// Each one is a button of its own, so a finger has every one of them
    /// rather than only the one the highlight happens to be on -- the same
    /// promise the two ends of a level make. The highlight is drawn here
    /// rather than by the list box, because the list box highlights a row and
    /// what is standing is one press within it.
    fn strip(self: &Rc<Self>, across: &crate::page::Across) -> GtkBox {
        let held = GtkBox::new(Orientation::Horizontal, 0);
        held.set_hexpand(true);
        held.set_halign(gtk4::Align::Center);

        for (at, press) in across.presses.iter().enumerate() {
            let button = Button::new();
            button.set_widget_name(named::PRESS);
            let icon = gtk4::Image::from_icon_name(press.icon);
            icon.set_pixel_size(PRESSED);
            button.set_child(Some(&icon));

            // Lit for what is on now, and marked for where the d-pad is
            // standing. Two different questions: shuffle can be on while the
            // thumb is somewhere else along the strip.
            if press.now {
                button.add_css_class("now");
            }

            if at == across.at {
                button.add_css_class("standing");
            }

            // The one the strip is for, drawn as a filled circle. On a music
            // player that is play, and it is the press a hand makes without
            // looking while the four around it are the ones it aims at.
            if press.chief {
                button.add_css_class("chief");
            }

            let does = Arc::clone(&press.does);
            let panel = Rc::clone(self);
            button.connect_clicked(move |_| {
                does(&panel);
                panel.redraw();
            });
            held.append(&button);
        }

        held
    }

    fn step(self: &Rc<Self>, level: crate::page::Level, mark: &str, amount: i32) -> Button {
        let end = Button::with_label(mark);
        end.set_widget_name(named::STEP);
        // A row is as tall as what is in it, and on a row holding a picture the
        // two ends would be drawn as two columns the height of the picture.
        end.set_valign(gtk4::Align::Center);
        let panel = Rc::clone(self);
        end.connect_clicked(move |_| {
            level(amount);
            panel.redraw();
        });
        end
    }

    /// Take a line of text, and hand it on.
    ///
    /// The on-screen keyboard is how it gets typed, so X still brings the
    /// keyboard up over this: the panel keeps the focus, and the keyboard
    /// types into whatever holds it.
    fn asking(self: &Rc<Self>, question: &str, then: Answer, secret: Secret) {
        self.state.borrow_mut().asking = Some(then);
        emptied(&self.rows, None);
        self.state.borrow_mut().placed = Vec::new();

        let row = ListBoxRow::new();
        row.set_activatable(false);
        let box_ = GtkBox::new(Orientation::Vertical, 0);
        let label = Label::new(Some(question));
        label.set_widget_name(named::ASKED);
        label.set_xalign(0.0);
        box_.append(&label);
        let entry = Entry::new();
        entry.set_visibility(secret == Secret::No);
        let panel = Rc::clone(self);
        entry.connect_activate(move |entry| panel.answered(&entry.text()));
        box_.append(&entry);
        row.set_child(Some(&box_));
        self.rows.append(&row);
        entry.grab_focus();
    }

    /// Put a question up, in place of the rows.
    ///
    /// Two lines: what is being asked, with the thing it is about beside it,
    /// and the answers under that. It is not a page, so nothing is pushed and
    /// nothing has to be walked back out of.
    fn wondering(self: &Rc<Self>, question: &str, about: &str, does: &[&str], then: Taken) {
        emptied(&self.rows, None);
        self.state.borrow_mut().placed = Vec::new();

        let asked = Label::new(Some(question));
        asked.set_widget_name(named::SURE);
        asked.set_xalign(0.0);
        let thing = Label::new(Some(about));
        thing.set_widget_name(named::ABOUT);
        thing.set_xalign(0.0);
        thing.set_ellipsize(EllipsizeMode::Middle);
        let line = GtkBox::new(Orientation::Horizontal, GAP);
        line.append(&asked);
        line.append(&thing);

        let foot = GtkBox::new(Orientation::Horizontal, GAP);
        foot.set_halign(Align::Fill);
        let mut answers = Vec::new();

        for (at, says) in std::iter::once(&marks::NO).chain(does.iter()).enumerate() {
            let answer = Button::with_label(says);
            answer.set_widget_name(named::ANSWER);
            answer.set_size_request(ANSWER, -1);
            answer.set_hexpand(true);

            if at > 0 {
                answer.add_css_class("does");
            }

            let panel = Rc::clone(self);
            answer.connect_clicked(move |_| panel.took(at));
            foot.append(&answer);
            answers.push(answer);
        }

        let held = GtkBox::new(Orientation::Vertical, BREATH);
        held.append(&line);
        held.append(&foot);
        // Not a row: a list highlights the row it is standing on, and the
        // question would be a sentence on the pink a chosen thing wears.
        let row = ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.set_child(Some(&held));
        self.rows.append(&row);
        self.rows.select_row(None::<&ListBoxRow>);

        self.state.borrow_mut().sure = Some(Sure { then, at: 0, answers });
        self.leaning();
    }

    /// Show which answer is standing.
    fn leaning(&self) {
        let state = self.state.borrow();

        let Some(sure) = &state.sure else { return };

        for (at, answer) in sure.answers.iter().enumerate() {
            match at == sure.at {
                true => answer.add_css_class("here"),
                false => answer.remove_css_class("here"),
            }
        }
    }

    /// Left and right, along the answers, which stop at the ends.
    fn lean(self: &Rc<Self>, step: i32) {
        {
            let mut state = self.state.borrow_mut();

            let Some(sure) = &mut state.sure else { return };

            let last = sure.answers.len().saturating_sub(1);
            let going = fitted::<usize, i32>(sure.at) + step;
            sure.at = fitted(going.clamp(0, fitted::<usize, i32>(last)));
        }

        self.leaning();
    }

    /// A takes whichever answer is standing.
    fn answered_sure(self: &Rc<Self>) {
        let at = self.state.borrow().sure.as_ref().map(|sure| sure.at);

        if let Some(at) = at {
            self.took(at);
        }
    }

    /// One of the answers, taken.
    ///
    /// No is nought and does nothing but leave, which is also what B does, so
    /// the question has one way out however it is answered.
    fn took(self: &Rc<Self>, at: usize) {
        let Some(sure) = self.state.borrow_mut().sure.take() else { return };

        match at.checked_sub(1) {
            None => self.draw(),
            Some(which) => {
                (sure.then)(self, which);
                self.redraw();
            }
        }
    }

    /// No, and B, which is the same answer said with the button that means
    /// back.
    fn left_alone(self: &Rc<Self>) {
        self.state.borrow_mut().sure = None;
        self.draw();
    }

    /// What the front of the machine is driving, which decides what its
    /// buttons mean.
    ///
    /// A question outranks a search line: both take letters, and only one of
    /// them is on the screen at a time.
    ///
    /// A line to type into on the tab is not enough. The line is a row, and the
    /// letters are only its while it is the row being stood on: walk down to an
    /// application and the pad is the list's again, which is what makes B mean
    /// back out of the menu there and rub out a letter on the line.
    fn driving(&self) -> Driving {
        if self.state.borrow().sure.is_some() {
            return Driving::Sure;
        }

        if self.state.borrow().asking.is_some() {
            return Driving::Question;
        }

        match self.typing_here() {
            Typing::Yes => Driving::Search,
            Typing::No => Driving::Panel,
        }
    }

    /// Whether the tab in front has a line to type into.
    fn seeks_here(&self) -> Seeks {
        let state = self.state.borrow();

        match state.pages.get(state.here).is_some_and(|page| page.sought.is_some()) {
            true => Seeks::Yes,
            false => Seeks::No,
        }
    }

    /// Whether the highlight is standing on that line.
    fn typing_here(&self) -> Typing {
        let on = self
            .rows
            .selected_row()
            .is_some_and(|row| self.typing_at(row.index()) == Typing::Yes);

        match on {
            true => Typing::Yes,
            false => Typing::No,
        }
    }

    /// Whether the row at that place in the list is the line to type into.
    fn typing_at(&self, at: impl TryInto<usize>) -> Typing {
        let Ok(at) = at.try_into() else { return Typing::No };

        match self.state.borrow().placed.get(at).is_some_and(|row| row.typing) {
            true => Typing::Yes,
            false => Typing::No,
        }
    }

    /// Put the caret in the line, and at the end of what is already there.
    ///
    /// Taking the focus selects the whole of an entry, and a letter typed over
    /// a selection replaces it. So a line that lost the focus and was handed it
    /// back between one letter and the next rubbed out everything typed before
    /// it, and the list could never be narrowed by more than a single letter.
    /// The end of the word is also where a thumb coming back to the line would
    /// look for the caret.
    fn typed_into(&self) {
        if self.search.has_focus() {
            return;
        }

        self.search.grab_focus();
        self.search.set_position(-1);
    }

    /// Put the highlighted row where it can be seen.
    ///
    /// A row is brought into view by being given the focus, which costs
    /// nothing on a panel with nothing to type into. On one that has a line to
    /// type into it costs the whole of it: the d-pad took the focus off the
    /// line at the first press, every letter after that went to a row instead
    /// of to the search, and there was no way back to it without a pointer. A
    /// hand holding only the controller could not narrow the list at all.
    ///
    /// The line to type into is the one row where that is the wrong thing: the
    /// focus is what makes the letters go into it, and it is scrolled into view
    /// instead. Walking off it hands the focus to the row walked onto, which is
    /// how the pad gets the letters back.
    fn seen(&self, row: &ListBoxRow) {
        match self.typing_at(row.index()) {
            Typing::Yes => {
                self.typed_into();
                self.keep_the_highlight_in_view();
            }
            Typing::No => {
                row.grab_focus();
            }
        }
    }

    /// Say what the line to type into is for, on a tab that asked for one.
    ///
    /// Whether it is on the screen at all is `place`, which puts the row it
    /// lives in at the top of the list or takes it out. Only the word standing
    /// in the empty line is decided here.
    fn seeking(&self) {
        let about = {
            let state = self.state.borrow();
            state
                .pages
                .get(state.here)
                .and_then(|page| page.sought.as_ref().map(|sought| sought.about.clone()))
        };

        if let Some(about) = about {
            self.search.set_placeholder_text(Some(&about));
        }
    }

    /// Hand the tab what has been typed, every time it changes.
    fn seeks(self: &Rc<Self>) {
        let panel = Rc::clone(self);
        self.search.connect_changed(move |entry| panel.narrowed(&entry.text()));
    }

    /// The line changed. Whatever built the rows decides what it means.
    fn narrowed(self: &Rc<Self>, word: &str) {
        let then = {
            let state = self.state.borrow();
            state.pages.get(state.here).and_then(|page| page.sought.clone()).map(|sought| sought.then)
        };

        if let Some(then) = then {
            then(self, word);
        }
    }

    fn answered(self: &Rc<Self>, word: &str) {
        let then = self.state.borrow_mut().asking.take();

        if let Some(then) = then {
            then(self, word);
        }

        self.redraw();
    }

    // --------------------------------------------------------------- the room

    fn reshaped(self: &Rc<Self>) {
        if self.state.borrow().reshaping {
            return;
        }

        self.state.borrow_mut().reshaping = true;
        let panel = Rc::clone(self);
        glib::idle_add_local_once(move || {
            panel.state.borrow_mut().reshaping = false;
            panel.fit();
        });
    }

    /// How much room the panel has sideways, and its share of it.
    fn across(&self) -> i32 {
        match self.state.borrow().opened {
            // The whole of it, with none kept back. What the share leaves down
            // each side is what says a card is lying on the desktop, and a card
            // that has been opened out is saying the opposite.
            Opened::Out => self.given().0,
            Opened::No => fitting::across(self.given().0, self.monitor().0),
        }
    }

    /// How wide the one picture may be drawn.
    ///
    /// The card less the margins holding the rows off its edges -- and opened
    /// out there are no edges to be held off, so the picture gets the width
    /// whole. Anything less leaves a stripe of not-picture down each side of
    /// a screen that is supposed to be nothing else.
    fn picture_room(&self) -> i32 {
        match self.state.borrow().opened {
            Opened::Out => self.across(),
            Opened::No => self.across() - 2 * MARGIN,
        }
    }

    /// How tall the one picture this card is about may be drawn.
    ///
    /// Worked out from the card's own height rather than written down, because
    /// the card has two heights: its share of the desktop, and the whole screen
    /// once it has been opened out. What the card gains the picture gains, and
    /// so is every row the tab has stopped writing under it.
    fn down(&self) -> i32 {
        let (strip, under) = {
            let state = self.state.borrow();
            let strip = match state.opened {
                Opened::Out => fitting::Strip::Hidden,
                Opened::No => fitting::Strip::Shown,
            };

            (strip, state.under)
        };

        fitting::showing(self.tall(), strip, under)
    }

    /// How tall the card is drawn, before anything on it is measured.
    fn tall(&self) -> i32 {
        match self.state.borrow().opened {
            Opened::Out => self.given().1,
            Opened::No => fitting::ceiling(self.given().1, self.monitor().1),
        }
    }

    /// The room to measure against: what the compositor has granted, or what
    /// it granted the last time this panel was up.
    ///
    /// The remembered size is a guess and the granted one is the answer, so
    /// the granted one wins the moment there is one. What it buys is the first
    /// draw: a panel that opens against the monitor opens far too wide and
    /// corrects itself where somebody can see it.
    fn given(&self) -> (i32, i32) {
        let granted = (self.window.width(), self.window.height());

        if granted.0 > 1 && granted.1 > 1 {
            return granted;
        }

        // Never larger than the screen, whatever is written down. A remembered
        // room outlives the screen it was measured on.
        let (remembered, screen) = (crate::room::last(&namespace()), self.monitor());

        match screen.0 > 1 && screen.1 > 1 {
            true => (remembered.0.min(screen.0), remembered.1.min(screen.1)),
            false => remembered,
        }
    }

    /// The screen, for the times there is nothing granted to measure against.
    fn monitor(&self) -> (i32, i32) {
        let Some(display) = gtk4::gdk::Display::default() else { return (0, 0) };

        let monitors = display.monitors();

        let Some(first) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() else {
            return (0, 0);
        };

        let screen = first.geometry();
        (screen.width(), screen.height())
    }

    /// Ask the card for the room it needs, once that is knowable.
    ///
    /// Before the window is on screen a row does not know how tall it is, and
    /// the answer given then is close enough to look right and wrong enough to
    /// cut the last row in half. So this is asked again once there is something
    /// to measure.
    ///
    /// The row height comes from GTK rather than from a number written here, so
    /// it stays right whatever the font ends up being, and the room comes from
    /// the compositor rather than from the screen's size, so a keyboard or a
    /// bar taking part of the screen takes it from the panel too.
    pub fn fit(self: &Rc<Self>) {
        // What the compositor granted, kept for the next time this panel is
        // opened. Written here rather than when the window is mapped because
        // this is the one place that runs whenever the room changes.
        crate::room::keep(&namespace(), (self.window.width(), self.window.height()));

        // Sideways first, and before the rows are asked about, because how many
        // tabs the strip has room for is a question about the width, and a
        // panel with nothing on it yet still has a strip.
        let wide = self.across();

        if wide != self.state.borrow().wide {
            self.state.borrow_mut().wide = wide;
            self.mark();
        }

        // A page with nothing on it yet still gets a size. The height is the
        // ceiling whatever the rows are, so a row height of nothing is the
        // right answer rather than a reason to give up: giving up left the card
        // with no size request at all, and it drew at whatever its own contents
        // wanted, which is small.
        // The first row that is a row. On a tab that seeks, the first is the
        // line to type into, which is taller than anything in the list under it
        // and would answer a question about how much room one row wants with a
        // number no row wants.
        let first = i32::from(self.seeks_here() == Seeks::Yes);
        let tall = self
            .rows
            .row_at_index(first)
            .or_else(|| self.rows.row_at_index(0))
            .map_or(0, |first| tall_as(&first));
        // Everything the card spends on something that is not a row: the tab
        // strip, the margins holding the list off the card's edges, and the line
        // drawn round the whole of it. Asked of the widgets rather than written
        // down here, because a number written twice is a number that goes out of
        // step, and out of step here means a cut row.
        let frame = tall_as(&self.top)
            + 2 * EDGE
            + self.scroller.margin_top()
            + self.scroller.margin_bottom();
        let ceiling = self.tall();
        let tall_enough = fitting::tall_enough(frame, tall, ceiling);

        // Asking for a size is asking the compositor for it, and every tab is
        // the same size, so asking again on every press is a round trip bought
        // for nothing.
        let asking = (wide, tall_enough);

        if self.state.borrow().asked != Some(asking) {
            self.state.borrow_mut().asked = Some(asking);
            self.card.set_size_request(wide, tall_enough);
            // Once the card is the size it has just been asked to be, rather
            // than now, when the rows are still where they were.
            let panel = Rc::clone(self);
            glib::idle_add_local_once(move || panel.keep_the_highlight_in_view());
        }
    }

    /// Bring the highlight back where it can be seen.
    ///
    /// The room changes under a panel that is already open: the on-screen
    /// keyboard takes the bottom of the screen and the card shrinks into what
    /// is left. The list keeps its place while that happens, so the row that
    /// was highlighted ends up under the keys, and the highlight is the one
    /// thing on a panel that has to stay in sight: it is where the next press
    /// of A will land.
    ///
    /// Moved by the scrollbar rather than by taking the focus. With the
    /// keyboard up the focus is in the line being typed into, and a panel that
    /// took it back would swallow the next letter.
    fn keep_the_highlight_in_view(&self) {
        let Some(row) = self.rows.selected_row() else { return };

        let at = row.allocation();
        let (top, tall) = (f64::from(at.y()), f64::from(at.height()));
        let scroll = self.scroller.vadjustment();
        let (seen, page) = (scroll.value(), scroll.page_size());

        if top < seen {
            scroll.set_value(top);
        } else if top + tall > seen + page {
            scroll.set_value(top + tall - page);
        }
    }

    // -------------------------------------------------------------- listening

    fn watch_everything(self: &Rc<Self>) {
        let watching: Vec<(usize, crate::page::Watch)> = self
            .state
            .borrow()
            .pages
            .iter()
            .enumerate()
            .filter_map(|(index, page)| page.watch.clone().map(|watch| (index, watch)))
            .collect();

        for (index, watch) in watching {
            self.watch(index, &watch);
        }
    }

    /// Redraw a tab when something outside the panel changes it.
    ///
    /// Even the lines that count are answered on a delay, so a rocker held down
    /// redraws a few times rather than a hundred. Both ends of the pipe have to
    /// hand a line over as it is written: a program whose output is not a
    /// terminal holds it back by default, and the news arrives in a batch long
    /// after it was news.
    fn watch(self: &Rc<Self>, index: usize, watch: &crate::page::Watch) {
        let Some((program, rest)) = watch.argv.split_first() else { return };

        let started = Command::new(program)
            .args(rest)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let Ok(mut running) = started else { return };

        let Some(reading) = running.stdout.take() else { return };

        self.state.borrow_mut().watchers.push(running);

        let panel = Rc::clone(self);
        let about = watch.about.clone();
        glib::spawn_future_local(async move {
            let mut lines = BufReader::new(reading);

            loop {
                let read = gtk4::gio::spawn_blocking(move || {
                    let mut said = String::new();

                    // A pipe that will not read is a program that has stopped
                    // talking, which the nought below is already read as.
                    let Ok(got) = lines.read_line(&mut said) else {
                        return (lines, said, 0);
                    };

                    (lines, said, got)
                })
                .await;

                let Ok((back, said, got)) = read else { break };

                if got == 0 {
                    break;
                }

                lines = back;

                if said.contains(&about) {
                    panel.heard(index);
                }
            }
        });
    }

    fn heard(self: &Rc<Self>, index: usize) {
        if self.state.borrow().due {
            return;
        }

        self.state.borrow_mut().due = true;
        let panel = Rc::clone(self);
        glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            panel.state.borrow_mut().due = false;
            let state = panel.state.borrow();
            let redraw = state.here == index && state.asking.is_none();
            drop(state);

            if redraw {
                panel.redraw();
            }
        });
    }

    /// Say which tab is in front, for anything outside that draws what is on
    /// the screen. The bar lights the icon that opened this one.
    ///
    /// Written down on every move between tabs and not only on the way in.
    /// The compositor has no event for a tab changing -- no layer opened or
    /// closed -- so this file is the only thing that says so, and the bar
    /// reads it rather than being told.
    fn say_which_tab(&self, index: usize) {
        let state = self.state.borrow();

        if let Some(page) = state.pages.get(index) {
            // Nothing here can put a failed write right, and it is not worth
            // a card: the cost is a bar that goes on naming the tab before
            // this one until the next change. Said where the journal will
            // hold it, and the panel carries on.
            if let Err(fault) = crate::door::saying(&page.title) {
                eprintln!("the tab in front could not be written down: {fault}");
            }
        }
    }

    /// The panel is over.
    ///
    /// GTK4 hands a caller nothing to hang a tidy-up on: taking the window
    /// down ends the surface, and the widget itself lives as long as the
    /// program holds a name for it, so the signal the Python waited on never
    /// arrives. The way out is written here instead of watched for, and
    /// everything that ends a panel comes through it.
    fn shut(&self) {
        // On the way out, so there is nothing left to do about it but say so.
        // What it leaves behind is a file naming a tab of a panel that has
        // gone, which the next panel to open writes over.
        if let Err(fault) = crate::door::forget() {
            eprintln!("the tab in front could not be forgotten: {fault}");
        }

        self.stop_watching();
        self.window.destroy();
        crate::chooser::gone();
        self.over.quit();
    }

    /// The panel is going, and what it started goes with it.
    ///
    /// Nothing owns these once this exits: they would sit on init holding a
    /// pipe with no reader for as long as the machine is up.
    fn stop_watching(&self) {
        for running in &mut self.state.borrow_mut().watchers {
            let _ = running.kill();
            let _ = running.wait();
        }
    }

    /// Say something in the corner, and take it down again in a moment.
    ///
    /// Stamped, because the note that is up is the last one said and the timer
    /// that takes it down has to be that one's: two presses inside the six
    /// seconds would otherwise leave the first one's timer rubbing out the
    /// second one's word.
    pub fn note(self: &Rc<Self>, said: &str) {
        let stamp = {
            let mut state = self.state.borrow_mut();
            state.noted += 1;
            state.noted
        };
        self.note.set_text(said);
        self.note.set_visible(true);
        let panel = Rc::clone(self);
        glib::timeout_add_local_once(A_MOMENT, move || {
            if panel.state.borrow().noted == stamp {
                panel.note.set_visible(false);
            }
        });
    }

    /// Run something slow without the panel going deaf while it happens.
    ///
    /// Connecting to a network takes seconds. Waiting for it on the drawing
    /// thread stops the panel answering the buttons, which reads as a machine
    /// that has crashed rather than one that is working.
    pub fn later(self: &Rc<Self>, argv: Vec<String>) {
        let panel = Rc::clone(self);
        glib::spawn_future_local(async move {
            let _ = gtk4::gio::spawn_blocking(move || {
                let Some((program, rest)) = argv.split_first() else { return };

                let _ = Command::new(program)
                    .args(rest)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            })
            .await;
            panel.redraw();
        });
    }

    /// Start something and leave it running.
    ///
    /// In a session of its own, the way the menu starts an application: what a
    /// panel starts and leaves on goes on running after the panel has gone.
    /// `later` is the other kind, for a command that ends, and a player handed
    /// to it was a player waited on for the length of the song and killed with
    /// the panel that started it. Choosing a song did nothing that outlived
    /// looking at it.
    ///
    /// Drawn again a moment later rather than at once, because what the tab
    /// says is what the player says about itself and the player has not been
    /// asked yet.
    pub fn leave_running(self: &Rc<Self>, argv: Vec<String>) {
        crate::running::left_running(&argv);
        let panel = Rc::clone(self);
        glib::timeout_add_local_once(crate::running::SETTLING, move || panel.redraw());
    }
}

/// The signals that mean go away, held back from killing this outright.
///
/// A chooser being replaced is asked to stop by the one replacing it, and it
/// has to put the controller back before it goes. Answered where the default
/// action would have been taken, the buttons would stay the panel's over a
/// desktop with no panel on it.
///
/// Held back on the main thread before anything else is started, because a
/// thread inherits the mask of whatever made it.
/// Anchored to all four edges, and claiming no exclusive zone of its own, so
/// the compositor hands over exactly the room nothing else has taken: the
/// screen less the bar, and less the on-screen keyboard while that is up.
///
/// Before this the surface was unanchored, which centres it and leaves it
/// whatever height it asked for. With the keyboard up that was too tall for the
/// gap between the two: it hung over the bar and its last rows were behind the
/// keys.
fn laid_over_everything(window: &Window) {
    window.init_layer_shell();
    window.set_namespace(Some(&namespace()));
    window.set_layer(Layer::Overlay);
    // Asked for rather than taken. A surface holding the keyboard exclusively
    // is one the compositor will not focus away from: Hyprland refuses to
    // refocus at all while one is up, and a touch is given to whatever was
    // focused before it. So every tap on the bar was handed to the panel with
    // coordinates outside the panel, which is a tap that does nothing at all.
    // The bar went dead the moment anything opened, and the icon that opened
    // it could not close it because the tap never arrived anywhere.
    //
    // On demand, the panel is focused when it comes up and the compositor is
    // free to hand the bar the taps that land on the bar.
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    for edge in [Edge::Bottom, Edge::Left, Edge::Right, Edge::Top] {
        window.set_anchor(edge, true);
    }
}

/// What the compositor lists this surface as.
///
/// The program's own name, so the launcher is "launcher" and the settings are
/// "settings-panel". Left unset every panel on this machine is
/// "gtk4-layer-shell", which is one name for all of them and no way for
/// anything looking at the compositor's list to say which is up. `bar-door`
/// is what asks, and it is how the bar lights the icon that opened a panel.
///
/// Taken from the program rather than passed in, so that a panel written later
/// is named without anybody choosing a name for it.
fn namespace() -> String {
    std::env::args()
        .next()
        .and_then(|argv0| {
            std::path::Path::new(&argv0).file_name().and_then(|name| name.to_str()).map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "console-panel".to_string())
}

/// One end of a strip with more tabs than it can show.
fn arrow(mark: &str) -> Button {
    let end = Button::with_label(mark);
    end.set_widget_name(named::MORE);
    end
}

/// Everything off a list, which GTK4 will only do one child at a time before
/// 4.12 and this device is not promised.
/// The room a row keeps at its front, with whatever is in it.
///
/// A picture that will not load leaves the room empty rather than drawing the
/// mark GTK has for a broken one. One is being made while the other is being
/// shown, so a row waiting for its own would otherwise say the picture is wrong
/// when what is true is that it is not there yet.
/// A picture made of characters.
/// A word at one end of a seek bar: how far in, or how long altogether.
///
/// Room kept for the longest either of them gets, counted in characters, so
/// the bar between them does not move as the seconds tick over.
/// A clock at one end of a bar, `toward` the bar so the two of them read as
/// one thing: the elapsed time pushed right, up against where the bar starts,
/// and the whole length pushed left, up against where it ends. Both are the
/// same number of characters wide whatever the numbers are, so the bar stays
/// where it is while the minutes count up rather than shuffling along with
/// them.
fn edge(said: &str, toward: f32) -> Label {
    let drawn = Label::new(Some(said));
    drawn.set_widget_name(named::ASIDE);
    drawn.set_width_chars(TIME_WIDE);
    drawn.set_xalign(toward);
    drawn.set_margin_start(GAP);
    drawn.set_margin_end(GAP);
    drawn
}

/// A picture written in characters, on its own plaque.
///
/// No margin of its own. It had one, from when the only one of these was a
/// thumbnail at the end of a row with words to its left; the one on the screen
/// now is the sleeve, alone on a row that centres what is on it, and a margin
/// on one side of a centred thing is that thing sitting off to the other.
fn written(markup: &str) -> Label {
    let drawn = Label::new(None);
    drawn.set_widget_name(named::COVER);
    drawn.set_markup(markup);
    drawn.set_xalign(0.0);
    drawn.set_yalign(0.0);
    drawn
}

/// A seek bar, drawn as one row of one colour with a dot at the position.
///
/// The bar is a `Label` wrapped in a `GestureClick`: a tap anywhere on it
/// lands at that fraction of the song. The d-pad on the row moves it a step
/// at a time, which is what `level` was always for; the bar only has to know
/// about the tap, which is what goes through the gesture.
fn scrub(panel: &Rc<Panel>, bar: crate::page::Bar, seek: Option<crate::page::Seek>) -> GtkBox {
    let wide = bar.wide.max(2);
    let at = bar.at.min(wide.saturating_sub(1));

    // Three labels side by side: what has been played, the dot, and what has
    // not. Three, so they can be two colours -- the played part warm and the
    // rest quiet, which is how every player draws it and is the only part of a
    // bar that says anything at a glance.
    //
    // Coloured by the stylesheet rather than by markup. It used to be one
    // label holding `<span foreground="@pink">`, and `@pink` is a name the
    // stylesheet knows and Pango does not: the markup would not parse, the
    // label kept the text it had, which was none, and what was on the screen
    // was a black row with nothing in it. Nothing said so, which is why it
    // stood there for as long as it did.
    //
    // Counted in characters throughout. `─` is three bytes, so splitting the
    // bar at the dot's position as if it were a byte offset lands inside a
    // character and takes the panel down -- which nothing ever saw, because
    // the dot was at nought for as long as the bar was empty.
    let body = "─".repeat(wide);
    let split = body.char_indices().nth(at).map_or(body.len(), |(where_, _)| where_);
    let (left, right) = body.split_at(split);
    let part = |said: &str, which: &str| {
        let drawn = Label::new(Some(said));
        drawn.set_widget_name(named::BAR);
        drawn.add_css_class(which);
        drawn
    };

    let drawn = GtkBox::new(Orientation::Horizontal, 0);
    drawn.set_margin_start(2 * GAP);
    drawn.set_margin_end(2 * GAP);
    drawn.append(&part(left, "done"));
    drawn.append(&part("●", "done"));
    drawn.append(&part(right, "over"));

    // The bar and the two times travel together, centred on the row. Left to
    // spread out, the bar took the middle and the times went to the far edges
    // of the card, where a time is a number floating in a corner rather than
    // one end of a bar.
    let outer = GtkBox::new(Orientation::Horizontal, 0);
    outer.set_hexpand(true);
    outer.set_halign(gtk4::Align::Center);
    outer.append(&drawn);

    if let Some(seek) = seek {
        // A tap is a fraction of where the finger landed. The label is `wide`
        // characters but the tap is a pixel anywhere on the widget, so the
        // fraction is taken off the click's x against the widget's own width
        // rather than off a count of characters: the row is wider than its
        // monospace cell, and using characters would never reach the end.
        //
        // The seek callback is what reads the tap, computes the position, and
        // asks the player to jump. The panel comes in alongside so the bar
        // can ask to be redrawn with the dot where the song now is.
        // On the bar itself, not on the box holding it: the box grew two clocks
        // either side, and a fraction taken across those would put every tap
        // a little to the left of where the finger was.
        let touch = GestureClick::new();
        let label = drawn.clone();
        let panel = Rc::clone(panel);
        touch.connect_pressed(move |_, _, x, _| {
            let width = f64::from(label.allocated_width().max(1));
            let frac = (x / width).clamp(0.0, 1.0);
            seek(&panel, frac);
            panel.redraw();
        });
        drawn.add_controller(touch);
    }

    outer
}

/// A sleeve, drawn large and square in the middle of its row.
///
/// Read from the file rather than out of the picture store: the store keeps
/// what a row in a list wants, which is a thumbnail, and this is the one
/// picture on the desktop that is the point of the card it is on.
///
/// The square is held whether there is a picture for it or not, so a cover
/// that arrives a moment after the song does fills a box that is already
/// there instead of pushing everything under it down the card.
fn sleeve(art: Option<&Path>) -> gtk4::Image {
    // A square, whatever shape the file is. What a player writes out for a
    // cover is not always a cover: kew hands over a wide picture with the
    // record in the middle of it, and drawn as it came that is a letterbox
    // with two green margins where a sleeve should be. Filled and cropped
    // takes the middle of it, which is the record.
    let held = gtk4::Image::new();
    held.set_widget_name(named::SLEEVE);
    held.set_pixel_size(SLEEVE);
    held.set_size_request(SLEEVE, SLEEVE);
    held.set_hexpand(false);
    held.set_halign(gtk4::Align::Center);
    held.set_valign(gtk4::Align::Center);

    // A cover that will not open leaves the card without one, which is what a
    // card whose song has no cover at all shows. There is nobody to tell:
    // the file is the player's, written by the player, and a panel saying so
    // out loud would be saying it about every song of a record.
    if let Some(Ok(square)) = art.map(middle) {
        held.set_paintable(Some(&square));
    }

    held
}

/// How many pixels are decoded for every point a picture is drawn at.
///
/// The screen this desktop is for has two and a half, and a photograph decoded
/// at the point size is a soft photograph on the one card whose whole purpose
/// is the photograph. Rounded up rather than down, and rounded here rather
/// than read off the widget: a texture is asked for before there is a widget
/// to ask the scale of, and asking for a few pixels more than the screen wants
/// costs a fraction of a decode that was going to happen anyway.
const SHARP: i32 = 3;

/// What the two ends of a level are drawn as: its own marks where it has
/// them, minus and plus where it has not.
fn ends_of(row: &Row) -> (&str, &str) {
    match &row.ends {
        Some((less, more)) => (less.as_str(), more.as_str()),
        None => (marks::LESS, marks::MORE),
    }
}

/// A picture drawn as large as the card will let it be, in the shape it came
/// in.
///
/// # Why the picture is in a scroller that does not scroll
///
/// The size is worked out here, from the file and from how much card there is,
/// and the widget has to be held to it. GTK has no maximum size on a widget:
/// a request is a floor, and every container asks its child how big it would
/// like to be and gives it that.
///
/// Both of the toolkit's picture widgets answer that question badly for this.
/// An image draws into a square of its pixel size, so a photograph taken the
/// usual way is drawn at the height of a square as wide as the card is tall --
/// a third of the picture there was room for. A picture widget answers with
/// the size of what it was handed, and what it was handed is two and a half
/// times the drawn size, because that is what the screen has pixels for; on a
/// card that is a column of rows a portrait photograph then comes out three
/// cards tall and everything under it goes off the bottom.
///
/// A scrolled window is the one container in GTK that does not pass its
/// child's wishes on: it asks for what it was told to ask for and hands the
/// child that. So the size is the scroller's, the drawing is the picture
/// widget's, and `can_shrink` is what lets the second fit inside the first.
/// Nothing scrolls -- the child is given exactly the room it draws in -- and
/// the policy says so, because a scroller left to decide would put a bar down
/// the side of every photograph.
fn showing(at: Option<&Path>, room: i32, down: i32) -> gtk4::ScrolledWindow {
    let held = gtk4::Picture::new();
    held.set_widget_name(named::SHOWING);
    held.set_can_shrink(true);
    held.set_hexpand(true);
    held.set_vexpand(true);

    // A file that will not open leaves the card without a picture, and the row
    // under it says so. Said there rather than here because this function has
    // no words: it is handed a path and draws what is behind it. The room is
    // held either way, so a card that cannot open its file is the same shape
    // as one that can with the reason written under the hole.
    let (across, down) = match at {
        Some(at) => {
            let (across, down, want) = box_for(at, room, down);
            fetch(&held, at.to_path_buf(), want);
            (across, down)
        }
        None => (down, down),
    };

    let frame = gtk4::ScrolledWindow::new();
    frame.set_policy(gtk4::PolicyType::External, gtk4::PolicyType::External);
    frame.set_size_request(across, down);
    frame.set_hexpand(false);
    frame.set_halign(gtk4::Align::Center);
    frame.set_valign(gtk4::Align::Center);
    frame.set_child(Some(&held));
    frame
}

/// What a panel says to do about a film it wants drawn.
///
/// Handed the path and answering with the surface to draw, or nothing where it
/// will not open. Called on the thread that draws, every time the card is drawn
/// -- which is a timer -- so the panel answering is expected to hand back the
/// same surface for the same film rather than starting it again.
pub type Films = Rc<dyn Fn(&Path) -> Option<gtk4::gdk::Paintable>>;

thread_local! {
    /// The one panel on this process that knows how to read a film.
    ///
    /// A thread local and not a lock, because the only thread that may ask is
    /// the one that draws, and a surface GTK draws with may not leave it. Set
    /// before the panel is shown; a card that asks before anything has been set
    /// gets a held space and the row under it says why.
    static FILMS: RefCell<Option<Films>> = const { RefCell::new(None) };
}

/// Give every film on a list a turn, whether or not the card is redrawn.
///
/// What the panel that shows films does with the turn is its own: open the
/// file, tell the decoder what the card has been asked for, and say where the
/// film has got to. What matters here is only that it is asked, and asked on
/// the thread that draws, on every reading rather than on every redraw.
/// How many rows a tab has written under the one picture it is about.
///
/// Everything but the picture itself, and nothing at all on a tab that has no
/// picture on it -- a list of settings never asks how tall a picture may be, so
/// the answer there is one nothing reads.
fn under(rows: &[Row]) -> i32 {
    let shown = rows
        .iter()
        .any(|row| matches!(row.picture, Picture::Showing(_) | Picture::Playing(_)));

    match shown {
        true => fitted(rows.len().saturating_sub(1)),
        false => 0,
    }
}

fn keep_films_going(rows: &[Row]) {
    let Some(films) = FILMS.with_borrow(|films| films.clone()) else { return };

    for row in rows {
        if let Picture::Playing(Some(at)) = &row.picture {
            films(at);
        }
    }
}

/// Say what to do about [`Picture::Playing`].
///
/// Called once, before [`show`], by the panel that draws films. Nothing here
/// reads one: what a film is decoded by is a package on the machine and a
/// decision about what this desktop carries, and both belong to the panel that
/// made them rather than to the framework every panel draws through.
///
/// # Why the framework does not simply do it
///
/// It was going to. GTK's own `GtkMediaFile` is a paintable and this would have
/// been four lines. It plays nothing here -- GTK is packaged with no media
/// backend, so every media file is the do-nothing stream -- and worse, its
/// paintable does not draw an empty square: handed to a widget it takes the
/// whole list down, title strip up and every row gone. What replaces it is a
/// GStreamer pipeline, which is a dependency, and a dependency every panel pays
/// for to draw a film that only one of them shows.
pub fn films(making: impl Fn(&Path) -> Option<gtk4::gdk::Paintable> + 'static) {
    FILMS.with_borrow_mut(|held| *held = Some(Rc::new(making)));
}

/// A film, in the room a still picture would have had.
///
/// The box is the whole of it, always, and the film is fitted inside. That is
/// the one place this and [`showing`] part company, and it is deliberate: a
/// still picture is measured before it is drawn, so the box can be its shape
/// and the caption can sit against its edge. A film has no shape until the
/// decoder has read some of it, which is after the card went up -- a box that
/// followed it would open one size, jump to another, and move the caption out
/// from under the thumb that was reading it. A film is going to be watched, so
/// what it wants is a place that stays where it was put.
fn playing(at: Option<&Path>, room: i32, down: i32) -> gtk4::ScrolledWindow {
    let drawn = at.and_then(|at| FILMS.with_borrow(|films| films.clone()).and_then(|films| films(at)));

    let (across, down) = film_size(drawn.as_ref(), room, down);

    let held = gtk4::Picture::new();
    held.set_widget_name(named::PLAYING);
    held.set_can_shrink(true);
    held.set_hexpand(true);
    held.set_vexpand(true);
    held.set_paintable(drawn.as_ref());

    // The same clamp a still picture is drawn in, and for a sharper reason. A
    // film's own idea of how big it is is its pixels -- a thousand points and
    // more -- and a row that wants that is a row the list grows to hold: the
    // card came out taller than the screen, with the transport below the edge
    // of it. This is the one container that will not pass a child's wishes on.
    let frame = gtk4::ScrolledWindow::new();
    frame.set_policy(gtk4::PolicyType::External, gtk4::PolicyType::External);
    frame.set_size_request(across, down);
    frame.set_hexpand(false);
    frame.set_halign(gtk4::Align::Center);
    frame.set_valign(gtk4::Align::Center);
    frame.set_child(Some(&held));
    frame
}

/// How big a film is drawn: across, then down, in points.
///
/// The same two bounds a still picture is fitted by -- never wider than the
/// card, never taller than what is left under it -- and one of that one's three
/// rules dropped. A still is never drawn larger than the file holds, because a
/// decoder asked for more than it has is a decoder asked to invent. A film is:
/// standard definition on a screen this size is meant to be filled out, and
/// nobody has ever wanted one drawn in a postage stamp in the middle of a card
/// because that is the number of pixels it was recorded at.
///
/// A film that has not said yet -- which is every film for the first moment,
/// because the decoder has to read some of the file before it knows -- is given
/// the whole box. It is where a film of the ordinary shape ends up anyway, so
/// the card settles rather than jumps.
fn film_size(drawn: Option<&gtk4::gdk::Paintable>, room: i32, down: i32) -> (i32, i32) {
    let (wide, tall) = match drawn {
        Some(held) => (held.intrinsic_width(), held.intrinsic_height()),
        None => (0, 0),
    };

    match wide > 0 && tall > 0 {
        true => (room.min(down * wide / tall), down.min(room * tall / wide)),
        false => (room, down),
    }
}

/// How big a picture is drawn -- across, then down, in points -- and how many
/// pixels to ask the decoder for.
///
/// Three rules, and the whole of why this is not left to the decoder.
///
/// Never larger than the file holds. `from_file_at_scale` will happily blow a
/// thirty-two pixel icon up to fill the card, and what that draws is a grid of
/// coloured squares rather than a bigger picture; a decoder asked for more
/// than it has is a decoder asked to invent. So a small picture is drawn small
/// and the card has room around it, which is what looking at a small picture
/// looks like.
///
/// Never taller than the card leaves, and never wider than the card is. Two
/// bounds and not one, because a square bound is only right for a square
/// picture: a panorama fitted into a square as tall as the card allows is
/// drawn a third of the width it could have had, and a portrait photograph
/// fitted into a square as wide as the card is runs off the bottom.
///
/// All of it read off `Pixbuf::file_info`, which reads the header and stops,
/// so the card takes its shape at once and nothing here waits on a decode.
fn box_for(path: &Path, room: i32, tall_room: i32) -> (i32, i32, i32) {
    // A picture that will not say how big it is has not been opened yet; the
    // decode is the one that says so, and it is asked for the card's size
    // because nothing is known to be smaller.
    let (wide, tall) = match gtk4::gdk_pixbuf::Pixbuf::file_info(path) {
        Some((_, wide, tall)) => (wide.max(1), tall.max(1)),
        None => (room, tall_room),
    };

    // Fitted inside the card and inside itself, on both sides at once. Each
    // line is that side's three bounds: what the picture has, what the card
    // gives this way, and what the card gives the other way once the shape is
    // followed across.
    let across = wide.min(room).min(tall_room * wide / tall);
    let down = tall.min(tall_room).min(room * tall / wide);

    let longer = across.max(down);

    // What to ask the decoder for: the longer side in pixels rather than in
    // points, and no more than the file has. `from_file_at_scale` fits a
    // picture into a square of this and keeps its shape, so one number is the
    // whole of the request whichever way up the picture is.
    (across, down, (longer * SHARP).min(wide.max(tall)))
}

/// Which picture, at how many pixels: what one decode is asked for, and what
/// one kept texture answers to.
type Asked = (PathBuf, i32);

/// One picture out of a file, in pieces that may cross a thread: the pixels,
/// their shape, how many bytes one row of them takes, and whether they carry
/// transparency.
///
/// The decode does not run where the card is, and in these bindings neither of
/// the toolkit's picture types may leave the thread it was made on. Bytes may,
/// so the decoder hands back bytes and the drawing thread wraps them in a
/// texture, which costs a header and copies nothing.
type Pixels = (glib::Bytes, i32, i32, usize, bool);

/// The pictures decoded lately, the one the card wants now, and whether a
/// decode is out being done.
struct Decoding {
    /// The last few, each at the size it was asked at. Walking back a file
    /// shows it at once rather than decoding it again.
    kept: VecDeque<(Asked, gtk4::gdk::Texture)>,
    /// The picture the card wants, and the widget waiting for it. One at
    /// most, because a card shows one thing: a d-pad held down wants where it
    /// is now, not everywhere it has been.
    wanted: Option<(Asked, glib::WeakRef<gtk4::Picture>)>,
    /// Whether a decode is away. One at a time, so a walk queues nothing:
    /// whatever is wanted when the one away lands is the next one done, and
    /// everything walked past in between is never decoded at all.
    busy: bool,
}

thread_local! {
    static DECODING: RefCell<Decoding> =
        const { RefCell::new(Decoding { kept: VecDeque::new(), wanted: None, busy: false }) };
}

/// How many decoded pictures are kept.
///
/// A texture at card size is megabytes on a machine whose graphics share
/// memory with the screen, so this is a number of steps back a thumb takes
/// without thinking, not a folder.
const KEPT: usize = 4;

/// Put what is behind a path into a picture widget, without making the card
/// wait for the decode.
///
/// The whole reason the decode is not done where the card is built: it was,
/// and a walk through a folder moved at the speed a photograph decodes rather
/// than the speed a thumb presses -- the words saying which file this is and
/// which of how many were held up by pixels they never needed. Built this
/// way the words land at once; the pixels arrive when they arrive, into this
/// widget if it is still the one on the card, and into the kept pile if the
/// walk has moved on.
fn fetch(into: &gtk4::Picture, at: PathBuf, want: i32) {
    let asked = (at, want);

    let kept = DECODING.with_borrow(|decoding| {
        decoding.kept.iter().find(|(key, _)| key == &asked).map(|(_, texture)| texture.clone())
    });

    if let Some(texture) = kept {
        into.set_paintable(Some(&texture));
        // And nothing older is owed anything: this widget is the card's
        // picture now, and a decode still away is for somewhere the walk left.
        DECODING.with_borrow_mut(|decoding| decoding.wanted = None);

        return;
    }

    let begin = DECODING.with_borrow_mut(|decoding| {
        decoding.wanted = Some((asked.clone(), into.downgrade()));

        match decoding.busy {
            true => None,
            false => {
                decoding.busy = true;
                Some(asked)
            }
        }
    });

    if let Some(asked) = begin {
        decode(asked);
    }
}

/// Send one decode out, and hand whatever comes back to [`decoded`].
fn decode(asked: Asked) {
    glib::spawn_future_local(async move {
        let (at, want) = asked.clone();

        // The one failure `pixels` cannot say for itself: the decoder
        // panicked. Its payload is a box with no words in it, so the file is
        // named here instead, and the card goes on the way it does for any
        // picture that would not open -- the room held, the row under it
        // saying which file this was.
        let read = match gtk4::gio::spawn_blocking(move || pixels(&at, want)).await {
            Ok(read) => read,
            Err(_panicked) => {
                eprintln!("console: {}: the decode did not come back", asked.0.display());

                None
            }
        };

        decoded(asked, read);
    });
}

/// One decode, off the thread that draws.
///
/// A photograph off this machine's camera is four thousand pixels across and
/// the card is a few hundred points, so decoding the file whole would hold the
/// better part of a hundred megabytes to draw a tenth of it. The decode is
/// asked for at the size the card drew its box for, and only ever asked to
/// shrink: a decoder asked for more than the file holds is a decoder asked to
/// invent.
fn pixels(at: &Path, want: i32) -> Option<Pixels> {
    let (wide, tall) = match gtk4::gdk_pixbuf::Pixbuf::file_info(at) {
        Some((_, wide, tall)) => (wide, tall),
        None => (0, 0),
    };

    let read = match wide > want || tall > want {
        true => gtk4::gdk_pixbuf::Pixbuf::from_file_at_scale(at, want, want, true),
        false => gtk4::gdk_pixbuf::Pixbuf::from_file(at),
    };

    let held = match read {
        Ok(held) => held,
        Err(fault) => {
            // Said aloud, because a file that will not decode is otherwise a
            // silent hole in a card. The card itself still reads right: the
            // room is held and the row under it names the file.
            eprintln!("console: {}: {fault}", at.display());

            return None;
        }
    };

    Some((held.read_pixel_bytes(), held.width(), held.height(), fitted(held.rowstride()), held.has_alpha()))
}

/// A decode came back. Keep it, hand it to whoever still wants it, and send
/// the next one out if the card has moved on meanwhile.
fn decoded(asked: Asked, read: Option<Pixels>) {
    let next = DECODING.with_borrow_mut(|decoding| {
        decoding.busy = false;

        if let Some((bytes, wide, tall, stride, alpha)) = read {
            let format = match alpha {
                true => gtk4::gdk::MemoryFormat::R8g8b8a8,
                false => gtk4::gdk::MemoryFormat::R8g8b8,
            };

            let texture = gtk4::gdk::MemoryTexture::new(wide, tall, format, &bytes, stride);
            decoding.kept.push_back((asked.clone(), texture.upcast()));

            while decoding.kept.len() > KEPT {
                decoding.kept.pop_front();
            }
        }

        let (wants, into) = decoding.wanted.take()?;

        // The walk moved on while this one was being read. What is wanted now
        // is the one worth doing; this one is in the kept pile and may yet be
        // walked back to.
        if wants != asked {
            decoding.wanted = Some((wants.clone(), into));
            decoding.busy = true;

            return Some(wants);
        }

        if let Some(into) = into.upgrade() {
            if let Some((_, texture)) = decoding.kept.iter().find(|(key, _)| key == &asked) {
                into.set_paintable(Some(texture));
            }
        }

        None
    });

    if let Some(wants) = next {
        decode(wants);
    }
}

/// The middle square of a picture, at the size a sleeve is drawn.
///
/// Cut here rather than left to the picture widget, which will letterbox
/// anything that is not square and has no way to be told otherwise on the
/// version of the toolkit this is built against.
fn middle(path: &Path) -> Result<gtk4::gdk::Texture, gtk4::glib::Error> {
    let whole = gtk4::gdk_pixbuf::Pixbuf::from_file(path)?;
    let side = whole.width().min(whole.height());
    let square = whole.new_subpixbuf((whole.width() - side) / 2, (whole.height() - side) / 2, side, side);
    // Twice the size it is drawn at, and no smaller than it came. The screen
    // this is for has two pixels to the point, so a picture cut down to the
    // number of points a sleeve is wide is drawn back up again and arrives
    // soft.
    let want = SLEEVE * 2;
    let drawn = match side > want {
        true => square.scale_simple(want, want, gtk4::gdk_pixbuf::InterpType::Bilinear).unwrap_or(square),
        false => square,
    };

    Ok(gtk4::gdk::Texture::for_pixbuf(&drawn))
}

fn shown(picture: &Picture) -> gtk4::Image {
    let held = gtk4::Image::new();
    held.set_widget_name(named::ICON);
    held.set_pixel_size(PICTURE);
    held.set_size_request(PICTURE, PICTURE);
    held.set_margin_end(2 * GAP);

    match picture {
        Picture::Named(icon) => held.set_icon_name(Some(icon)),
        // Out of the store if it is in there, which is the whole of why the
        // store exists: opening the file here is a format worked out and an
        // image scaled, on the loop that draws, once per row.
        Picture::At(path) => match crate::pictures::ready(path) {
            Some(ready) => held.set_paintable(Some(&ready)),
            None if path.exists() => held.set_from_file(Some(path)),
            None => {}
        },
        _ => {}
    }

    held
}

/// Where the highlight stands, given where it was standing.
///
/// Whether what is typed into a question is shown as it is typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Secret {
    /// A password: the letters are hidden as they arrive.
    Yes,
    /// Anything else, which is read back as it is typed.
    No,
}

/// Whether a point is on the card or on the desktop showing round it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum On {
    /// On the panel itself.
    TheCard,
    /// On what the panel is covering, which is one thing only: shut it.
    TheDesktop,
}

/// Whether the thumb is on the way out of the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leaving {
    /// It is, so the next press closes.
    Yes,
    /// It is not.
    No,
}

/// Whether the tab in front has a line to type into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seeks {
    /// It has.
    Yes,
    /// It has not.
    No,
}

/// Whether the highlight is standing on the line to type into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typing {
    /// It is, so a letter goes into the line.
    Yes,
    /// It is not.
    No,
}

/// Forwards to the first row something happens to, and back where it started
/// if there is none: a tab that says one thing and offers nothing still has to
/// put the highlight somewhere.
///
/// A card that names the row it opens on is stood on there instead, and only
/// while the highlight has not been put anywhere yet. Standing on a row
/// nothing happens to is what that looks like: `at` is nought on a tab nobody
/// has walked yet, and every drawing after the first is asked about the row
/// the thumb is actually on. So the now-playing card opens under play and
/// stays where it is walked to, rather than jumping back on every tick of the
/// second that redraws it.
fn standing(rows: &[Row], at: usize) -> i32 {
    let fresh = rows.get(at).is_none_or(|row| row.heading() == Heading::Yes);
    let asked = rows.iter().position(|row| row.chief && row.heading() == Heading::No);

    if let (true, Some(index)) = (fresh, asked) {
        return fitted(index);
    }

    let found =
        rows.iter().enumerate().skip(at).find(|(_, row)| row.heading() == Heading::No);
    fitted(found.map_or(at, |(index, _)| index))
}

/// Where one press of the d-pad takes the highlight.
///
/// Past anything nothing happens to, rather than onto it. A title is a row on
/// the list and is not one of the answers on it, and a thumb walking down six
/// things that can be done should meet six of them.
///
/// It stops where the list does, and a step that finds nothing to stand on
/// stays where it was: at the bottom of a list ending in a word to be read,
/// pressing down again should leave the highlight where it is rather than take
/// it off the last row anybody can choose.
fn walked(rows: &[Row], at: i32, step: i32) -> i32 {
    let last = fitted::<usize, i32>(rows.len().saturating_sub(1));
    let mut going = at;

    loop {
        going += step;

        if going < 0 || going > last {
            return at;
        }

        if rows.get(fitted::<i32, usize>(going)).is_none_or(|row| row.heading() == Heading::No) {
            return going;
        }
    }
}

/// Take everything off the list, except the one row that is asked to stay.
///
/// The line to type into stays. Removing it would unparent the entry inside it,
/// and an unparented entry has lost the focus and the word in it, on a list
/// that is emptied and filled again every time a letter narrows it.
fn emptied(list: &ListBox, keep: Option<&ListBoxRow>) {
    let mut child = list.first_child();

    while let Some(here) = child {
        let next = here.next_sibling();

        if !keep.is_some_and(|row| &here == row.upcast_ref::<gtk4::Widget>()) {
            list.remove(&here);
        }

        child = next;
    }
}

fn wide_as(widget: &impl IsA<gtk4::Widget>) -> i32 {
    widget.measure(Orientation::Horizontal, -1).1
}

fn tall_as(widget: &impl IsA<gtk4::Widget>) -> i32 {
    widget.measure(Orientation::Vertical, -1).1
}

/// The one stylesheet, given to every surface this process draws.
fn dressed() {
    let Some(display) = gtk4::gdk::Display::default() else { return };

    let sheet = CssProvider::new();
    sheet.load_from_data(&style::sheet());
    gtk4::style_context_add_provider_for_display(
        &display,
        &sheet,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Put a panel on screen and wait for it to be dismissed.
pub fn show(build: Build, column: i32, start: Option<&str>) {
    // From the press, where the daemon stamped one, and from this program's own
    // exec where it did not. Everything up to here is already spent by the time
    // there is anything of ours to spend it: the fork, the loader, and the wait
    // for whatever chooser was on the screen to get off it.
    opening::started(&namespace());
    opening::taking("screen", chooser::waited_for_screen());

    if let Err(fault) = gtk4::init() {
        eprintln!("no screen to draw on: {fault}");
        return;
    }

    opening::mark("gtk");
    let waiting = glib::MainLoop::new(None, false);
    let panel = Panel::new(build, column, start, waiting.clone());

    let asked_of = Rc::clone(&panel);
    asked::stops_when_asked(move || asked_of.shut());

    panel.window.present();
    opening::mark("shown");

    // The first frame is the answer to the press, and it is the frame clock
    // that knows when it happened -- not the idle below, which runs whenever
    // the loop next has nothing to do and may be either side of it. Left
    // connected afterwards because an opening is written once and every frame
    // after it finds nothing left to write.
    if let Some(clock) = panel.window.frame_clock() {
        let drawn = Rc::clone(&panel);
        clock.connect_after_paint(move |_| {
            if opening::running() == opening::Running::No {
                return;
            }

            // Asked rather than taken: a frame drawn while the panel is in the
            // middle of changing its own state is a frame that must not be the
            // thing that kills it, and what is lost by stepping over it is one
            // number about the rows on a card that is already on the screen.
            if let Ok(state) = drawn.state.try_borrow() {
                opening::counted("rows", fitted(state.placed.len()));

                if let Some(page) = state.pages.get(state.here) {
                    opening::named("tab", &page.title);
                }
            }

            opening::mark("frame");
            opening::done();
        });
    }

    let opened = Rc::clone(&panel);
    glib::idle_add_local_once(move || {
        opened.fit();
        opened.say_which_tab(opened.state.borrow().here);
        chooser::drawn();
    });
    waiting.run();
}

#[cfg(test)]
mod tests {
    use super::{standing, walked};
    use crate::page::{Does, Heading, Row};

    fn heading(says: &str) -> Row {
        Row::said(says, "")
    }

    fn chooseable(says: &str) -> Row {
        Row::new(says, "", Does::run(&["true"]))
    }

    /// A tab whose first row is a heading opened with the highlight on a word,
    /// and the first press of A did nothing at all.
    #[test]
    fn the_highlight_opens_on_something_it_can_act_on() {
        let rows = [heading("Search with"), chooseable("DuckDuckGo")];
        assert_eq!(standing(&rows, 0), 1);
    }

    /// The screen and the speakers are held at a level and chosen for nothing.
    /// Read as headings, the two settings anybody actually touches would be the
    /// two the highlight walked past.
    #[test]
    fn a_row_held_at_a_level_is_something_to_act_on() {
        let rows = [heading("Screen").levelled(std::sync::Arc::new(|_| ())), chooseable("Balanced")];
        assert_eq!(standing(&rows, 0), 0);
    }

    /// The now-playing card: the sleeve and the two lines of words are read
    /// rather than chosen, so walking down it the first row anything happens
    /// to is the bar -- and the press a hand opened the card to make is play,
    /// one row below it. Every opening of the tab began with a press of down.
    #[test]
    fn a_card_that_names_the_row_it_opens_on_opens_there() {
        let rows = [
            heading("Blue Monday"),
            chooseable("0:31").levelled(std::sync::Arc::new(|_| ())),
            chooseable("the transport").chief(),
        ];
        assert_eq!(standing(&rows, 0), 2);
    }

    /// And is asked once. The playing tab is drawn again every second, so a
    /// card that went back to its own press on every reading would take the
    /// highlight off whatever the thumb had walked to a second after it got
    /// there.
    #[test]
    fn a_thumb_that_walked_off_it_is_left_where_it_walked_to() {
        let rows = [
            heading("Blue Monday"),
            chooseable("0:31").levelled(std::sync::Arc::new(|_| ())),
            chooseable("the transport").chief(),
        ];
        assert_eq!(standing(&rows, 1), 1);
    }

    /// A row nothing happens to cannot be the row a card opens on, whatever it
    /// says about itself: the highlight would be standing where a press does
    /// nothing, which is the fault this whole rule is about.
    #[test]
    fn a_card_cannot_open_on_a_row_nothing_happens_to() {
        let rows = [heading("Blue Monday").chief(), chooseable("0:31")];
        assert_eq!(standing(&rows, 0), 1);
    }

    #[test]
    fn where_you_were_standing_is_where_you_stay() {
        let rows = [chooseable("one"), chooseable("two"), chooseable("three")];
        assert_eq!(standing(&rows, 2), 2);
    }

    /// A tab that says one thing and offers nothing still has to put the
    /// highlight somewhere.
    #[test]
    fn a_tab_with_nothing_to_act_on_stays_where_it_was_put() {
        assert_eq!(standing(&[heading("Nothing else is playing")], 0), 0);
        assert_eq!(standing(&[], 0), 0);
    }

    /// A title is walked past rather than onto, from either direction.
    #[test]
    fn the_dpad_steps_over_the_name_of_what_a_list_is_about() {
        let rows = [chooseable("\u{2039} Pictures"), Row::naming("holiday.jpg", "2.4 MB"),
                    chooseable("Open"), chooseable("Delete")];
        assert_eq!(walked(&rows, 0, 1), 2);
        assert_eq!(walked(&rows, 2, -1), 0);
    }

    /// A step off the end is a step that stays, so the last row anybody can
    /// choose is one the highlight can rest on.
    #[test]
    fn a_step_past_the_end_of_a_list_stays_where_it_was() {
        let rows = [chooseable("Open"), chooseable("Delete"), heading("Nothing else")];
        assert_eq!(walked(&rows, 1, 1), 1);
        assert_eq!(walked(&rows, 0, -1), 0);
    }

    /// A list with nothing in it says so, and what it says is not one of the
    /// things in it. The highlight walks past it from either direction, so a
    /// tab holding nothing has nothing under the thumb to press.
    #[test]
    fn the_dpad_steps_over_the_panel_saying_a_list_is_empty() {
        let rows = [chooseable("\u{2039} Music"), Row::nothing("Nothing in /home/music"),
                    chooseable("Open the folder")];
        assert_eq!(walked(&rows, 0, 1), 2);
        assert_eq!(walked(&rows, 2, -1), 0);
    }

    /// Drawn as a class rather than as a part with a name of its own, so the
    /// check that every part is dressed does not see it. A row that says a list
    /// is empty and is dressed like one of its options is the whole fault this
    /// was written for.
    #[test]
    fn the_panel_saying_a_list_is_empty_is_not_dressed_as_an_option() {
        assert_eq!(Row::nothing("Nothing is waiting").heading(), Heading::Yes);
        assert!(crate::style::sheet().contains("row.nothing {"));
    }

    /// Nothing happens to it and nothing is written beside it, which is a
    /// heading by every other measure. It is the row the menu opens on: the
    /// first thing a hand does with a list of two hundred applications is say
    /// which one it is after.
    #[test]
    fn the_line_to_type_into_is_where_a_menu_opens() {
        let rows = [Row::line_to_type_in(), chooseable("Files")];
        assert_eq!(standing(&rows, 0), 0);
    }
}
