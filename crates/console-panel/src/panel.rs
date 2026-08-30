//! The panel, drawn.
//!
//! Nothing here decides anything that could be decided somewhere quieter: how
//! many tabs the strip has room for is `strip`, how tall the card should be is
//! `fitting`, and what a button means is `keys`. This puts what they answer on
//! the screen.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
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
use crate::page::{Answer, Does, Page, Picture, Row, Showing, Taken};
use crate::strip::{EDGE, GAP, MARGIN, PICTURE};

/// Nearer than this and the pointer did not move: the list moved under it.
const A_HAIR: f64 = 0.5;

/// How wide each answer to a question is drawn, which is a thumb's worth
/// rather than a share of a card that could be any width.
const ANSWER: i32 = 150;

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
use crate::{asked, chooser, fitting, running, strip, style};

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
    asked: Option<(i32, i32)>,
    /// What each tab said last time, so coming back to one shows it at once and
    /// corrects itself a moment later rather than blinking empty.
    remembered: BTreeMap<usize, Vec<Row>>,
    /// The rows on the tab as it stands, which is where a chosen row is looked
    /// up.
    placed: Vec<Row>,
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
        self.asking(question, then, true);
    }

    fn ask_aloud(&self, question: &str, then: Answer) {
        self.asking(question, then, false);
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
        scroller.set_margin_top(10);
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
                from_tab: 0,
                wide: 0,
                cell: None,
                spent: 0,
                asking: None,
                sure: None,
                pointed: None,
                noted: 0,
                reading: 0,
                asked: None,
                remembered: BTreeMap::new(),
                placed: Vec::new(),
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
                panel.state.borrow_mut().at = row.index().max(0) as usize;
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
        match meaning(key, self.driving()) {
            Meaning::Abandon => {
                self.state.borrow_mut().asking = None;
                self.left_alone();
            }
            Meaning::Choose if self.state.borrow().sure.is_some() => self.answered_sure(),
            Meaning::Choose => {
                if let Some(row) = self.rows.selected_row() {
                    match self.typing_at(row.index()) {
                        // The way off the line and onto the first thing it has
                        // left standing, which is the whole of what a search
                        // box is for. What it lands on is read before it is
                        // taken, like every other row here.
                        true => self.walk(1),
                        false => self.chose(row.index()),
                    }
                }
            }
            Meaning::More => self.offered(),
            Meaning::Nothing => return glib::Propagation::Proceed,
            Meaning::Nudge(step) if self.state.borrow().sure.is_some() => self.lean(step),
            Meaning::Nudge(step) => self.nudge(step),
            Meaning::Shut => self.backed_out(),
            Meaning::Step(step) => self.walk(step),
            Meaning::Tab(step) => self.turn(step),
        }
        glib::Propagation::Stop
    }

    /// Whether a point is on the card rather than on the desktop showing round
    /// it.
    fn on_the_card(&self, x: f64, y: f64) -> bool {
        let card = self.card.allocation();
        let (left, top) = (f64::from(card.x()), f64::from(card.y()));
        x >= left
            && x < left + f64::from(card.width())
            && y >= top
            && y < top + f64::from(card.height())
    }

    /// A tap off the card is a tap on what the panel is covering, and there is
    /// only one thing it can mean.
    fn tapped(self: &Rc<Self>, x: f64, y: f64) {
        if !self.on_the_card(x, y) {
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

        let Some(row) = self.rows.row_at_y(y as i32) else { return };
        if self.rows.selected_row().as_ref() != Some(&row) {
            self.rows.select_row(Some(&row));
            self.seen(&row);
        }
    }

    fn chose(self: &Rc<Self>, index: i32) {
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

    fn turn(self: &Rc<Self>, step: i32) {
        let going = {
            let state = self.state.borrow();
            let last = state.pages.len().saturating_sub(1) as i32;
            (state.here as i32 + step).clamp(0, last) as usize
        };
        self.went_to(going);
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
            match index == state.here {
                true => button.add_css_class("here"),
                false => button.remove_css_class("here"),
            }
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
        let (stamp, here, rows) = {
            let mut state = self.state.borrow_mut();
            state.reading += 1;
            let rows = state.pages.get(state.here).map(|page| page.rows.clone());
            (state.reading, state.here, rows)
        };
        let Some(rows) = rows else { return };
        let panel = Rc::clone(self);
        glib::spawn_future_local(async move {
            let read = gtk4::gio::spawn_blocking(move || rows.read()).await.unwrap_or_default();
            panel.arrived(stamp, here, read);
        });
    }

    fn arrived(self: &Rc<Self>, stamp: u64, here: usize, rows: Vec<Row>) {
        {
            let mut state = self.state.borrow_mut();
            if stamp != state.reading || state.asking.is_some() || state.sure.is_some() {
                return;
            }
            state.remembered.insert(here, rows.clone());
        }
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
        let seeking = self.seeks_here();
        emptied(&self.rows, seeking.then_some(&self.seeker));
        if seeking {
            rows.insert(0, Row::line_to_type_in());
            if self.seeker.parent().is_none() {
                self.rows.prepend(&self.seeker);
            }
        }
        for row in rows.iter().skip(usize::from(seeking)) {
            let held = ListBoxRow::new();
            if row.now() {
                held.add_css_class("now");
            }
            // Nothing happens to it, so nothing about it is offered to a hand:
            // the d-pad walks past it and a tap slides off it. Said to GTK as
            // well as worked out here, because the pad is not the only thing
            // that picks a row and a finger would otherwise land where the
            // highlight cannot.
            if row.heading() {
                held.set_activatable(false);
                held.set_selectable(false);
            }
            if row.naming {
                held.add_css_class("naming");
            }
            if row.nothing {
                held.add_css_class("nothing");
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
        // Whichever of the two is being stood on gets the letters. Standing on
        // the line, they are the line's; standing anywhere else, they are
        // nobody's and the pad is the list's.
        match self.typing_at(at) {
            true => self.typed_into(),
            false => {
                self.rows.grab_focus();
            }
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
        match &row.picture {
            Picture::None => {}
            Picture::Written(markup) => line.append(&written(markup)),
            picture => line.append(&shown(picture)),
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
        let (less, more) = match &row.ends {
            Some((less, more)) => (less.as_str(), more.as_str()),
            None => (marks::LESS, marks::MORE),
        };
        // A level is its two ends with the reading held between them, so the
        // mark that makes it smaller is on the side it shrinks towards and the
        // one that makes it bigger is on the side it grows into. Laid from the
        // right inward: the plus, the reading, the minus.
        if let Some(level) = &row.level {
            line.append(&self.step(level.clone(), less, -1));
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
            line.append(&self.step(level.clone(), more, 1));
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
    /// keyboard up over this: the panel keeps the focus, and wvkbd types into
    /// whatever holds it.
    fn asking(self: &Rc<Self>, question: &str, then: Answer, secret: bool) {
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
        entry.set_visibility(!secret);
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
            let going = sure.at as i32 + step;
            sure.at = going.clamp(0, last as i32) as usize;
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
            true => Driving::Search,
            false => Driving::Panel,
        }
    }

    /// Whether the tab in front has a line to type into.
    fn seeks_here(&self) -> bool {
        let state = self.state.borrow();
        state.pages.get(state.here).is_some_and(|page| page.sought.is_some())
    }

    /// Whether the highlight is standing on that line.
    fn typing_here(&self) -> bool {
        self.rows.selected_row().is_some_and(|row| self.typing_at(row.index()))
    }

    /// Whether the row at that place in the list is the line to type into.
    fn typing_at(&self, at: impl TryInto<usize>) -> bool {
        let Ok(at) = at.try_into() else { return false };
        self.state.borrow().placed.get(at).is_some_and(|row| row.typing)
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
            true => {
                self.typed_into();
                self.keep_the_highlight_in_view();
            }
            false => {
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
        fitting::across(self.given().0, self.monitor().0)
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
        let first = i32::from(self.seeks_here());
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
        let ceiling = fitting::ceiling(self.given().1, self.monitor().1);
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
                    let got = lines.read_line(&mut said).unwrap_or(0);
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
            crate::door::saying(&page.title);
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
        crate::door::forget();
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
fn written(markup: &str) -> Label {
    let drawn = Label::new(None);
    drawn.set_widget_name(named::COVER);
    drawn.set_markup(markup);
    drawn.set_xalign(0.0);
    drawn.set_yalign(0.0);
    drawn.set_margin_end(2 * GAP);
    drawn
}

fn shown(picture: &Picture) -> gtk4::Image {
    let held = gtk4::Image::new();
    held.set_widget_name(named::ICON);
    held.set_pixel_size(PICTURE);
    held.set_size_request(PICTURE, PICTURE);
    held.set_margin_end(2 * GAP);
    match picture {
        Picture::Named(icon) => held.set_icon_name(Some(icon)),
        Picture::At(path) if path.exists() => held.set_from_file(Some(path)),
        _ => {}
    }
    held
}

/// Where the highlight stands, given where it was standing.
///
/// Forwards to the first row something happens to, and back where it started
/// if there is none: a tab that says one thing and offers nothing still has to
/// put the highlight somewhere.
fn standing(rows: &[Row], at: usize) -> i32 {
    let found = rows.iter().enumerate().skip(at).find(|(_, row)| !row.heading());
    found.map_or(at, |(index, _)| index) as i32
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
    let last = rows.len().saturating_sub(1) as i32;
    let mut going = at;
    loop {
        going += step;
        if going < 0 || going > last {
            return at;
        }
        if rows.get(going as usize).is_none_or(|row| !row.heading()) {
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
    if let Err(fault) = gtk4::init() {
        eprintln!("no screen to draw on: {fault}");
        return;
    }
    let waiting = glib::MainLoop::new(None, false);
    let panel = Panel::new(build, column, start, waiting.clone());

    let asked_of = Rc::clone(&panel);
    asked::stops_when_asked(move || asked_of.shut());

    running::controller("tabs");
    panel.window.present();
    let opened = Rc::clone(&panel);
    glib::idle_add_local_once(move || {
        opened.fit();
        opened.say_which_tab(opened.state.borrow().here);
        chooser::drawn();
    });
    waiting.run();
    running::controller("desktop");
}

#[cfg(test)]
mod tests {
    use super::{standing, walked};
    use crate::page::{Does, Row};

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
        assert!(Row::nothing("Nothing is waiting").heading());
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
