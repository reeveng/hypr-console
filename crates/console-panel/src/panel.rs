//! The panel, drawn.
//!
//! Nothing here decides anything that could be decided somewhere quieter: how
//! many tabs the strip has room for is `strip`, how tall the card should be is
//! `fitting`, and what a button means is `keys`. This puts what they answer on
//! the screen.

use console_child_processes::{Alongside, alongside};
use console_never::Never;
use console_number_conversion::{Float, fitted, toward_zero_i32};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CssProvider, Entry, EventControllerKey,
    EventControllerMotion, GestureClick, GestureSwipe, Label, ListBox, ListBoxRow, Orientation,
    Overlay, PolicyType, ProgressBar, PropagationPhase, ScrolledWindow, Window,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::keys::{Driving, Meaning, meaning, swept};
use crate::marks::{self, named};
use crate::page::{
    Act, Answer, Does, Heading, InEffect, Page, Picture, Row, Same, Set, Showing, Stirred, Taken,
};
use crate::strip::{ANSWER, EDGE, GAP, MARGIN, PICTURE, PRESSED, SLEEVE};

const A_HAIR: f64 = 0.5;

const BREATH: i32 = 14;

const A_MOMENT: std::time::Duration = std::time::Duration::from_secs(6);

const NOTE_WIDE: i32 = 34;

const OVER_ROWS: i32 = 10;

const TIME_WIDE: i32 = 7;
use crate::{asked, chooser, fitting, opening, running, strip, style, telling};

pub type Build = Arc<dyn Fn() -> Vec<Page> + Send + Sync>;

struct State {
    pages: Vec<Page>,
    here: usize,
    at: usize,
    out: bool,
    opened: Opened,
    from_tab: usize,
    wide: i32,
    cell: Option<i32>,
    spent: i32,
    asking: Option<Answer>,
    sure: Option<Sure>,
    pointed: Option<f64>,
    noted: u64,
    reading: u64,
    landed: u64,
    asked: Option<(i32, i32)>,
    remembered: BTreeMap<usize, Vec<Row>>,
    placed: Vec<Row>,
    under: i32,
    tabs: Vec<Button>,
    watchers: Vec<Alongside>,
    reshaping: bool,
    due: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    Out,
    No,
}

pub enum Over {
    Loop(glib::MainLoop),
    Told(Rc<dyn Fn()>),
}

struct Sure {
    then: Taken,
    at: usize,
    answers: Vec<Button>,
}

pub struct Panel {
    build: Build,
    column: i32,
    over: Over,

    window: Window,
    card: GtkBox,
    note: Label,
    search: Entry,
    seeker: ListBoxRow,
    top: GtkBox,
    less: Button,
    more: Button,
    shut: Button,
    away: Button,
    scroller: ScrolledWindow,
    rows: ListBox,

    state: RefCell<State>,
}

impl Showing for Rc<Panel> {
    fn refresh(&self) {
        let Ok(()) = self.redraw();
    }

    fn replace(&self, standing_on: usize) {
        {
            let mut state = self.state.borrow_mut();
            let here = state.here;
            state.remembered.remove(&here);
            state.at = standing_on;
        }

        let Ok(()) = self.redraw();
    }

    fn forget_typing(&self) {
        self.search.set_text("");
    }

    fn ask(&self, question: &str, then: Answer) {
        let Ok(()) = self.asking(question, then, Secret::Yes);
    }

    fn ask_aloud(&self, question: &str, then: Answer) {
        let Ok(()) = self.asking(question, then, Secret::No);
    }

    fn sure(&self, question: &str, about: &str, does: &[&str], then: Taken) {
        let Ok(()) = self.wondering(question, about, does, then);
    }

    fn later(&self, argv: Vec<String>) {
        let Ok(()) = Panel::later(self, argv);
    }

    fn leave_running(&self, argv: Vec<String>) {
        let Ok(()) = Panel::leave_running(self, argv);
    }

    fn note(&self, said: &str) {
        let Ok(()) = Panel::note(self, said);
    }

    fn open_out(&self) {
        let Ok(()) = Panel::open_out(self);
    }

    fn turn_to(&self, tab: usize) {
        let last = self.state.borrow().pages.len().saturating_sub(1);
        self.state.borrow_mut().out = false;

        let Ok(()) = self.went_to(tab.min(last));
    }
}

impl Panel {
    pub fn new(
        build: Build,
        column: i32,
        start: Option<&str>,
        over: Over,
    ) -> Result<Rc<Self>, Never> {
        let pages = build();
        let Ok(whose) = namespace();
        let Ok(left_on) = crate::tab::last(&whose);
        let Ok(here) = crate::page::find(&pages, start.or(left_on.as_deref()));

        match pages.get(here) {
            Some(page) => {
                let Ok(()) = crate::tab::keep(&whose, &page.title);
            }
            None => {},
        }

        let window = Window::new();

        let Ok(()) = laid_over_everything(&window);

        let card = GtkBox::new(Orientation::Vertical, 0);
        card.set_widget_name(named::CARD);
        card.set_halign(Align::Center);
        card.set_valign(Align::Center);

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

        let away = Button::with_label(marks::SHUT);
        away.set_widget_name(named::SHUT);
        away.set_halign(Align::End);
        away.set_valign(Align::Start);
        away.set_margin_end(BREATH);
        away.set_margin_top(BREATH);
        away.set_visible(false);
        over_the_card.add_overlay(&away);

        window.set_child(Some(&over_the_card));

        let top = GtkBox::new(Orientation::Horizontal, 0);
        top.set_widget_name(named::TOP);
        card.append(&top);

        let Ok(less) = arrow(marks::BEFORE);
        less.set_margin_end(GAP);
        top.append(&less);

        let strip = GtkBox::new(Orientation::Horizontal, GAP);
        strip.set_widget_name(named::STRIP);
        strip.set_homogeneous(true);
        strip.set_hexpand(true);
        top.append(&strip);

        let Ok(more) = arrow(marks::AFTER);
        more.set_margin_start(GAP);
        top.append(&more);

        let shut = Button::with_label(marks::SHUT);
        shut.set_widget_name(named::SHUT);
        shut.set_margin_start(2i32.saturating_mul(GAP));
        top.append(&shut);

        let search = Entry::new();
        search.set_widget_name(named::SOUGHT);
        search.set_hexpand(true);

        let seeker = ListBoxRow::new();
        seeker.add_css_class("typing");
        seeker.set_child(Some(&search));

        let rows = ListBox::new();
        rows.set_widget_name(named::PANEL);
        rows.set_activate_on_single_click(true);

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

        let Ok(()) = dressed();

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
            away,
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
                under: 2,
                tabs,
                watchers: Vec::new(),
                reshaping: false,
                due: false,
            }),
        });
        let Ok(()) = panel.answers();
        let Ok(()) = panel.seeks();
        let Ok(()) = panel.watch_everything();
        let Ok(wide) = panel.across();

        panel.state.borrow_mut().wide = wide;

        let Ok(()) = opening::mark("built");
        let Ok(()) = panel.draw();
        let Ok(()) = panel.entered();
        let Ok(()) = panel.fit();

        Ok(panel)
    }

    pub fn window(&self) -> Result<&Window, Never> {
        Ok(&self.window)
    }

    fn answers(self: &Rc<Self>) -> Result<(), Never> {
        let keys = EventControllerKey::new();
        let panel = Rc::clone(self);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Ok(propagation) = panel.pressed(key);

            propagation
        });
        keys.set_propagation_phase(PropagationPhase::Capture);
        self.window.add_controller(keys);

        let pointer = EventControllerMotion::new();
        let panel = Rc::clone(self);
        pointer.connect_motion(move |_, _, y| {
            let Ok(()) = panel.hovered(y);
        });
        self.rows.add_controller(pointer);

        let taps = GestureClick::new();
        let panel = Rc::clone(self);
        taps.connect_pressed(move |_, _, x, y| {
            let Ok(()) = panel.tapped(x, y);
        });
        self.window.add_controller(taps);

        let panel = Rc::clone(self);
        self.rows.connect_row_activated(move |_, row| {
            let Ok(()) = panel.touched(row.index());
        });
        let panel = Rc::clone(self);
        self.rows.connect_row_selected(move |_, row| {
            match row {
                Some(row) => {
                    let Ok(whole) = fitted(row.index().max(0));
                    panel.state.borrow_mut().at = whole;
                }
                None => {},
            }
        });

        let panel = Rc::clone(self);
        self.shut.connect_clicked(move |_| {
            let Ok(()) = panel.the_way_back();
        });
        let panel = Rc::clone(self);
        self.away.connect_clicked(move |_| {
            let Ok(()) = panel.close_out();
        });
        let panel = Rc::clone(self);
        self.less.connect_clicked(move |_| {
            let Ok(()) = panel.turn(-1);
        });
        let panel = Rc::clone(self);
        self.more.connect_clicked(move |_| {
            let Ok(()) = panel.turn(1);
        });

        let buttons: Vec<Button> = self.state.borrow().tabs.clone();

        for (index, button) in buttons.into_iter().enumerate() {
            let panel = Rc::clone(self);
            button.connect_clicked(move |_| {
                let Ok(()) = panel.went_to(index);
            });
        }

        let panel = Rc::clone(self);
        self.window.connect_realize(move |window| {
            let Some(surface) = window.surface() else { return };

            let panel = Rc::clone(&panel);
            surface.connect_layout(move |_, _, _| {
                let Ok(()) = panel.reshaped();
            });
        });

        Ok(())
    }

    fn pressed(self: &Rc<Self>, key: Key) -> Result<glib::Propagation, Never> {
        let Ok(driving) = self.driving();
        let Ok(meaning) = meaning(key, driving);
        let Ok(stirred) = self.stirred();

        match meaning != Meaning::Nothing && stirred == Stirred::Woke {
            true => {
                self.refresh();

                return Ok(glib::Propagation::Stop);
            }
            false => {},
        }

        match meaning {
            Meaning::Abandon => {
                self.state.borrow_mut().asking = None;

                let Ok(()) = self.left_alone();
            }
            Meaning::Choose if self.state.borrow().sure.is_some() => {
                let Ok(()) = self.answered_sure();
            }
            Meaning::Choose if self.leaving() == Ok(Leaving::Yes) => {
                let Ok(()) = self.shut();
            }
            Meaning::Choose => {
                match self.rows.selected_row() {
                    Some(row) => {
                        let Ok(typing) = self.typing_at(row.index());

                        match typing {
                            Typing::Yes => {
                                let Ok(()) = self.walk(1);
                            }
                            Typing::No => {
                                let Ok(()) = self.chose(row.index());
                            }
                        }
                    }
                    None => {},
                }
            }
            Meaning::More => {
                let Ok(()) = self.came_back();
                let Ok(()) = self.offered();
            }
            Meaning::Nothing => return Ok(glib::Propagation::Proceed),
            Meaning::Nudge(step) if self.state.borrow().sure.is_some() => {
                let Ok(()) = self.lean(step);
            }
            Meaning::Nudge(step) => {
                let Ok(()) = self.came_back();
                let Ok(()) = self.nudge(step);
            }
            Meaning::Shut if self.state.borrow().opened == Opened::Out => {
                let Ok(()) = self.close_out();
            }
            Meaning::Shut => {
                let Ok(()) = self.backed_out();
            }
            Meaning::Step(step) => {
                let Ok(()) = self.came_back();
                let Ok(()) = self.walk(step);
            }
            Meaning::Tab(step) => {
                let Ok(()) = self.turn(step);
            }
        }

        Ok(glib::Propagation::Stop)
    }

    fn stirred(self: &Rc<Self>) -> Result<Stirred, Never> {
        let stirs = {
            let state = self.state.borrow();

            match state.asking.is_some() || state.sure.is_some() {
                true => None,
                false => state.pages.get(state.here).and_then(|page| page.stirs.clone()),
            }
        };

        Ok(match stirs {
            Some(stirs) => stirs(),
            None => Stirred::Awake,
        })
    }

    fn on_the_card(&self, x: f64, y: f64) -> Result<On, Never> {
        let card = self.card.allocation();
        let (left, top) = (f64::from(card.x()), f64::from(card.y()));
        let inside = x >= left
            && x < left + f64::from(card.width())
            && y >= top
            && y < top + f64::from(card.height());

        Ok(match inside {
            true => On::TheCard,
            false => On::TheDesktop,
        })
    }

    fn tapped(self: &Rc<Self>, x: f64, y: f64) -> Result<(), Never> {
        let Ok(on) = self.on_the_card(x, y);

        match on == On::TheDesktop {
            true => {
                let Ok(()) = self.shut();
            }
            false => {},
        }

        Ok(())
    }

    fn hovered(&self, y: f64) -> Result<(), Never> {
        let still = self
            .rows
            .translate_coordinates(&self.window, 0.0, y)
            .map_or(y, |(_, on_screen)| on_screen);
        let before = self.state.borrow().pointed;
        self.state.borrow_mut().pointed = Some(still);

        let Some(before) = before else { return Ok(()) };

        match (still - before).abs() < A_HAIR {
            true => return Ok(()),
            false => {},
        }

        let Ok(down) = toward_zero_i32(y);

        let Some(row) = self.rows.row_at_y(down) else { return Ok(()) };

        match self.rows.selected_row().as_ref() != Some(&row) {
            true => {
                self.rows.select_row(Some(&row));

                let Ok(()) = self.seen(&row);
            }
            false => {},
        }

        Ok(())
    }

    fn touched(self: &Rc<Self>, index: i32) -> Result<(), Never> {
        let Ok(stirred) = self.stirred();

        match stirred {
            Stirred::Woke => self.refresh(),
            Stirred::Awake => {
                let Ok(()) = self.chose(index);
            }
        }

        Ok(())
    }

    fn chose(self: &Rc<Self>, index: i32) -> Result<(), Never> {
        let Ok(()) = self.came_back();

        let Ok(index) = usize::try_from(index) else { return Ok(()) };

        let does = self.state.borrow().placed.get(index).and_then(|row| row.does.clone());

        match does {
            None => eprintln!(
                "nothing happens on row {index} of {}: {:?}",
                self.state.borrow().placed.len(),
                self.state.borrow().placed.get(index).map(|row| row.says.clone())
            ),
            Some(Does::Call(act)) => {
                match act(self) {
                    true => {
                        let Ok(()) = self.shut();
                    }
                    false => {},
                }
            }
            Some(Does::Run(argv)) => {
                let Ok(()) = running::left_running(&argv);
                let Ok(()) = self.shut();
            }
        }

        Ok(())
    }

    fn offered(self: &Rc<Self>) -> Result<(), Never> {
        let Some(row) = self.rows.selected_row() else { return Ok(()) };

        let Ok(index) = usize::try_from(row.index()) else { return Ok(()) };

        let more = self.state.borrow().placed.get(index).and_then(|row| row.more.clone());

        match more {
            Some(more) => {
                match more(self) {
                    true => {
                        let Ok(()) = self.shut();
                    }
                    false => {},
                }
            }
            None => {},
        }

        Ok(())
    }

    fn open_out(self: &Rc<Self>) -> Result<(), Never> {
        match self.state.borrow().opened == Opened::Out {
            true => return Ok(()),
            false => {},
        }

        self.state.borrow_mut().opened = Opened::Out;

        self.opened_out()
    }

    fn close_out(self: &Rc<Self>) -> Result<(), Never> {
        match self.state.borrow().opened == Opened::No {
            true => return Ok(()),
            false => {},
        }

        self.state.borrow_mut().opened = Opened::No;

        self.opened_out()
    }

    fn tells(self: &Rc<Self>) -> Result<(), Never> {
        let Ok(Some(where_to)) = telling::where_to() else { return Ok(()) };

        let mut card: Vec<telling::Spot> = Vec::new();
        let Ok(()) = self.spots_on(self.top.upcast_ref(), telling::Scrolls::No, &mut card);
        let Ok(()) = self.spots_on(self.away.upcast_ref(), telling::Scrolls::No, &mut card);
        let Ok(whose) = namespace();
        let Ok(lines) = self.lines_told();

        let told = telling::Told {
            panel: whose,
            tab: {
                let state = self.state.borrow();
                state.pages.get(state.here).map(|page| page.title.clone()).unwrap_or_default()
            },
            out: match self.state.borrow().opened {
                Opened::Out => telling::Out::Yes,
                Opened::No => telling::Out::No,
            },
            room: (self.window.width(), self.window.height()),
            spots: card,
            lines,
        };

        let Ok(told) = telling::said(&told);
        let said = format!("{told}\n");

        match std::fs::OpenOptions::new().create(true).append(true).open(&where_to) {
            Ok(mut file) => {
                match std::io::Write::write_all(&mut file, said.as_bytes()) {
                    Ok(_) => {},
                    Err(fault) => {
                        eprintln!("console-panel: {where_to}: {fault}");
                    }
                }
            }
            Err(fault) => eprintln!("console-panel: {where_to}: {fault}"),
        }

        Ok(())
    }

    fn lines_told(&self) -> Result<Vec<telling::Line>, Never> {
        let placed = self.state.borrow().placed.clone();

        Ok(placed
            .iter()
            .enumerate()
            .map(|(at, row)| {
                let mut spots = Vec::new();

                let Ok(which) = fitted(at);

                match self.rows.row_at_index(which) {
                    Some(held) => {
                        let Ok(()) =
                            self.spots_on(held.upcast_ref(), telling::Scrolls::Yes, &mut spots);
                    }
                    None => {},
                }

                let Ok(bare) = bare(row);

                telling::Line {
                    at,
                    says: row.says.clone(),
                    aside: row.aside.clone(),
                    offers: match row.more.is_some() {
                        true => telling::Offers::Yes,
                        false => telling::Offers::No,
                    },
                    bare: match bare {
                        Bare::Yes => telling::Bare::Yes,
                        Bare::No => telling::Bare::No,
                    },
                    spots,
                }
            })
            .collect())
    }

    fn spots_on(
        &self,
        held: &gtk4::Widget,
        scrolls: telling::Scrolls,
        into: &mut Vec<telling::Spot>,
    ) -> Result<(), Never> {
        let name = held.widget_name();

        match telling::OFFERED.contains(&name.as_str()) && held.is_visible() {
            true => {
                let found = held.compute_bounds(&self.window).map(|bounds| {
                    let whole = |said: f32| {
                        let Ok(whole) = toward_zero_i32(f64::from(said));

                        whole
                    };

                    (
                        (whole(bounds.x()), whole(bounds.y())),
                        (whole(bounds.width()), whole(bounds.height())),
                    )
                });

                let allocation = held.allocation();
                let (at, big) = found.unwrap_or((
                    (allocation.x(), allocation.y()),
                    (allocation.width(), allocation.height()),
                ));

                into.push(telling::Spot { name: name.to_string(), at, big, scrolls });
            }
            false => {},
        }

        let mut child = held.first_child();

        while let Some(held) = child {
            let Ok(()) = self.spots_on(&held, scrolls, into);

            child = held.next_sibling();
        }

        Ok(())
    }

    fn the_way_out(&self) -> Result<(), Never> {
        let state = self.state.borrow();

        let Ok(drawn) = drawn(&state.placed);

        let shown = match state.opened {
            Opened::No => false,
            Opened::Out => drawn == Drawn::TheCardWithIt,
        };

        self.away.set_visible(shown);

        Ok(())
    }

    fn opened_out(self: &Rc<Self>) -> Result<(), Never> {
        let out = self.state.borrow().opened == Opened::Out;
        self.top.set_visible(!out);

        let Ok(()) = self.the_way_out();

        self.window.set_exclusive_zone(match out {
            true => -1,
            false => 0,
        });

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

        let Ok(()) = self.set_the_rows();

        {
            let mut state = self.state.borrow_mut();
            state.asked = None;
            state.placed.clear();
            state.remembered.clear();
        }

        let Ok(()) = self.fit();

        self.redraw()
    }

    fn the_way_back(self: &Rc<Self>) -> Result<(), Never> {
        let Ok(driving) = self.driving();

        match driving {
            Driving::Question | Driving::Sure => {
                self.state.borrow_mut().asking = None;

                let Ok(()) = self.left_alone();
            }
            Driving::Panel | Driving::Search => {
                let Ok(()) = self.shut();
            }
        }

        Ok(())
    }

    fn backed_out(self: &Rc<Self>) -> Result<(), Never> {
        let back = {
            let state = self.state.borrow();
            state.pages.get(state.here).and_then(|page| page.back.clone())
        };

        match back {
            Some(back) if !back(self) => (),
            Some(_) | None => {
                let Ok(()) = self.shut();
            }
        }

        Ok(())
    }

    fn walk(self: &Rc<Self>, step: i32) -> Result<(), Never> {
        let now = self.rows.selected_row().map_or(0, |row| row.index());
        let Ok(at) = walked(&self.state.borrow().placed, now, step);

        let Some(going) = self.rows.row_at_index(at) else { return Ok(()) };

        self.rows.select_row(Some(&going));

        self.seen(&going)
    }

    fn nudge(self: &Rc<Self>, step: i32) -> Result<(), Never> {
        let Some(row) = self.rows.selected_row() else { return Ok(()) };

        let Ok(index) = usize::try_from(row.index()) else { return Ok(()) };

        let level = self.state.borrow().placed.get(index).and_then(|row| row.level.clone());

        match level {
            Some(level) => {
                level(step);

                let Ok(()) = self.redraw();
            }
            None => {},
        }

        Ok(())
    }

    fn turn(self: &Rc<Self>, step: i32) -> Result<(), Never> {
        let going = {
            let state = self.state.borrow();
            let from = match state.out {
                true => strip::Stop::Out,
                false => strip::Stop::Tab(state.here),
            };
            let Ok(going) = strip::along(state.pages.len(), from, step);

            going
        };

        match going {
            strip::Stop::Out => {
                self.state.borrow_mut().out = true;

                let Ok(()) = self.mark();
            }
            strip::Stop::Tab(index) => {
                let front = {
                    let mut state = self.state.borrow_mut();
                    state.out = false;
                    index == state.here
                };

                match front {
                    true => {
                        let Ok(()) = self.mark();
                    }
                    false => {
                        let Ok(()) = self.went_to(index);
                    }
                }
            }
        }

        Ok(())
    }

    fn leaving(&self) -> Result<Leaving, Never> {
        Ok(match self.state.borrow().out {
            true => Leaving::Yes,
            false => Leaving::No,
        })
    }

    fn came_back(&self) -> Result<(), Never> {
        let was = std::mem::replace(&mut self.state.borrow_mut().out, false);

        match was {
            true => {
                let Ok(()) = self.mark();
            }
            false => {},
        }

        Ok(())
    }

    fn went_to(self: &Rc<Self>, index: usize) -> Result<(), Never> {
        match index == self.state.borrow().here {
            true => return Ok(()),
            false => {},
        }

        let Ok(()) = self.say_which_tab(index);

        {
            let mut state = self.state.borrow_mut();
            state.here = index;
            state.at = 0;
        }

        self.scroller.vadjustment().set_value(0.0);

        let title = self.state.borrow().pages.get(index).map(|page| page.title.clone());

        match title {
            Some(title) => {
                let Ok(whose) = namespace();
                let Ok(()) = crate::tab::keep(&whose, &title);
            }
            None => {},
        }

        let Ok(()) = self.draw();

        self.entered()
    }

    fn draw(self: &Rc<Self>) -> Result<(), Never> {
        match self.state.borrow().sure.is_some() {
            true => return Ok(()),
            false => {},
        }

        let Ok(()) = self.mark();
        let Ok(()) = self.seeking();

        let (said_before, meanwhile) = {
            let state = self.state.borrow();
            let before = state.remembered.get(&state.here).cloned();
            let meanwhile = state.pages.get(state.here).and_then(|page| page.meanwhile.clone());
            (before, meanwhile)
        };
        let showing = said_before.or_else(|| meanwhile.map(|at_once| at_once())).unwrap_or_default();

        let Ok(()) = self.place(showing);

        self.fill()
    }

    fn redraw(self: &Rc<Self>) -> Result<(), Never> {
        {
            let mut state = self.state.borrow_mut();
            state.pages = (self.build)();
            state.here = state.here.min(state.pages.len().saturating_sub(1));
        }

        self.draw()
    }

    fn entered(self: &Rc<Self>) -> Result<(), Never> {
        let arriving = {
            let state = self.state.borrow();
            state.pages.get(state.here).and_then(|page| page.entered.clone())
        };

        match arriving {
            Some(arriving) => {
                arriving(self);
            }
            None => {},
        }

        Ok(())
    }

    fn mark(&self) -> Result<(), Never> {
        let showing = {
            let mut state = self.state.borrow_mut();
            let Ok(cell) = self.measure(&mut state);
            let Ok(room) = strip::room(state.wide, state.spent);
            let Ok(fits) = strip::fits(room, cell);
            let Ok(showing) = strip::showing(state.pages.len(), state.here, state.from_tab, fits);

            state.from_tab = showing.start;

            showing
        };
        let state = self.state.borrow();

        for (index, button) in state.tabs.iter().enumerate() {
            button.set_visible(showing.contains(&index));

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

        self.less.set_visible(showing.start > 0);
        self.more.set_visible(showing.end < state.pages.len());

        Ok(())
    }

    fn measure(&self, state: &mut State) -> Result<i32, Never> {
        match state.cell {
            Some(cell) => return Ok(cell),
            None => {},
        }

        state.spent = [&self.less, &self.more, &self.shut]
            .iter()
            .map(|button| {
                let Ok(wide) = wide_as(*button);

                wide
            })
            .sum();

        let cell = state
            .tabs
            .iter()
            .map(|button| {
                let Ok(wide) = wide_as(button);

                wide
            })
            .max()
            .unwrap_or(0);

        state.cell = Some(cell);

        Ok(cell)
    }

    fn fill(self: &Rc<Self>) -> Result<(), Never> {
        let (stamp, here, rows, tab) = {
            let mut state = self.state.borrow_mut();
            state.reading = state.reading.saturating_add(1);
            let rows = state.pages.get(state.here).map(|page| page.rows.clone());
            let tab = state.pages.get(state.here).map(|page| page.title.clone());
            (state.reading, state.here, rows, tab.unwrap_or_default())
        };

        let Some(rows) = rows else { return Ok(()) };

        let Ok(whose) = namespace();
        let Ok(mut waiting) = console_wait_times::Waiting::here(&whose, "list");
        let Ok(()) = waiting.named("tab", &tab);

        let panel = Rc::clone(self);
        glib::spawn_future_local(async move {
            let read = match gtk4::gio::spawn_blocking(move || {
                let Ok(read) = rows.read();

                read
            })
            .await
            {
                Ok(read) => read,

                Err(_) => {
                    eprintln!("console: the rows of this tab were not read; it is drawn empty");
                    Default::default()
                }
            };

            let Ok(()) = waiting.mark("read");

            let Ok(whole_3) = fitted::<usize, u64>(read.len());
            let many = whole_3;

            let Ok(()) = panel.arrived(stamp, here, read);

            let Ok(()) = waiting.mark("placed");
            let Ok(()) = waiting.counted("rows", many);
            let Ok(()) = waiting.done_if_felt();
        });

        Ok(())
    }

    fn arrived(self: &Rc<Self>, stamp: u64, here: usize, rows: Vec<Row>) -> Result<(), Never> {
        {
            let mut state = self.state.borrow_mut();

            match stamp <= state.landed || state.asking.is_some() || state.sure.is_some() {
                true => return Ok(()),
                false => {},
            }

            state.landed = stamp;
            state.remembered.insert(here, rows.clone());

            match here != state.here {
                true => return Ok(()),
                false => {},
            }
        }

        let wanted: Vec<String> = rows
            .iter()
            .filter_map(|row| match &row.picture {
                Picture::At(path) => path.to_str().map(str::to_string),
                Picture::None
                | Picture::Space
                | Picture::Named(_)
                | Picture::Sleeve(_)
                | Picture::Showing(_)
                | Picture::Playing(_)
                | Picture::Written(_)
                | Picture::Bar(_) => None,
            })
            .collect();
        let Ok(missing) = crate::pictures::missing(&wanted);
        let Ok(()) = crate::pictures::make(&missing);

        self.place(rows)
    }

    fn place(self: &Rc<Self>, mut rows: Vec<Row>) -> Result<(), Never> {
        let Ok(seeks) = self.seeks_here();
        let seeking = seeks == Seeks::Yes;

        let Ok(()) = self.set_the_rows();
        let Ok(under) = under(&rows);

        self.state.borrow_mut().under = under;

        let Ok(()) = keep_films_going(&rows);
        let Ok(same) = self.same_as_drawn(&rows);

        match same == Same::Yes {
            true => {
                match seeking {
                    true => {
                        let Ok(line) = Row::line_to_type_in();

                        rows.insert(0, line);
                    }
                    false => {},
                }

                let mut state = self.state.borrow_mut();
                state.placed = rows;

                return Ok(());
            }
            false => {},
        }

        let stood = self.scroller.vadjustment().value();

        let Ok(()) = emptied(&self.rows, seeking.then_some(&self.seeker));

        match seeking {
            true => {
                let Ok(line) = Row::line_to_type_in();

                rows.insert(0, line);

                match self.seeker.parent().is_none() {
                    true => {
                        self.rows.prepend(&self.seeker);
                    }
                    false => {},
                }
            }
            false => {},
        }

        for row in rows.iter().skip(usize::from(seeking)) {
            let held = ListBoxRow::new();

            let Ok(now) = row.now();
            let Ok(heading) = row.heading();

            match now == InEffect::Yes {
                true => {
                    held.add_css_class("now");
                }
                false => {},
            }

            match heading == Heading::Yes {
                true => {
                    held.set_activatable(false);
                    held.set_selectable(false);
                }
                false => {},
            }

            match row.naming {
                true => {
                    held.add_css_class("naming");
                }
                false => {},
            }

            match row.middle {
                true => {
                    held.add_css_class("middle");
                }
                false => {},
            }

            match row.stacked {
                true => {
                    held.add_css_class("stacked");
                }
                false => {},
            }

            match matches!(row.picture, Picture::Bar(_)) {
                true => {
                    held.add_css_class("scrub");
                }
                false => {},
            }

            match row.nothing {
                true => {
                    held.add_css_class("nothing");
                }
                false => {},
            }

            match row.across.is_some() {
                true => {
                    held.add_css_class("transport");
                }
                false => {},
            }

            match matches!(row.picture, Picture::Showing(_) | Picture::Playing(_)) {
                true => {
                    held.add_css_class("showing");
                }
                false => {},
            }

            let Ok(line) = self.line(row);

            held.set_child(Some(&line));

            match &row.level {
                Some(level) => {
                    let Ok(sweeping) = self.sweeping(level);

                    held.add_controller(sweeping);
                }
                None => {}
            }

            self.rows.append(&held);
        }

        let at = {
            let mut state = self.state.borrow_mut();
            state.placed = rows;
            let at = state.at.min(state.placed.len().saturating_sub(1));
            let Ok(standing) = standing(&state.placed, at);

            standing
        };

        let Ok(()) = self.fit();
        let Ok(()) = self.the_way_out();

        match self.rows.row_at_index(at) {
            Some(staying) => {
                self.rows.select_row(Some(&staying));
            }
            None => {},
        }

        let panel = Rc::clone(self);
        glib::idle_add_local_once(move || {
            panel.scroller.vadjustment().set_value(stood);

            let Ok(()) = panel.tells();
        });

        let Ok(typing) = self.typing_at(at);

        match typing {
            Typing::Yes => {
                let Ok(()) = self.typed_into();
            }
            Typing::No => {
                self.rows.grab_focus();
            }
        }

        opening::mark("placed")
    }

    fn same_as_drawn(&self, rows: &[Row]) -> Result<Same, Never> {
        let state = self.state.borrow();
        let drawn = &state.placed;
        let Ok(seeks) = self.seeks_here();
        let seeking = seeks == Seeks::Yes;
        let from = usize::from(seeking && drawn.first().is_some_and(|row| row.typing));
        let alike = drawn.len().saturating_sub(from) == rows.len()
            && drawn
                .iter()
                .skip(from)
                .zip(rows)
                .all(|(drawn, row)| {
                    let Ok(looks) = drawn.looks_like(row);

                    looks == Same::Yes
                });

        Ok(match alike {
            true => Same::Yes,
            false => Same::No,
        })
    }

    fn line(self: &Rc<Self>, row: &Row) -> Result<GtkBox, Never> {
        let Ok(line) = self.spelled(row);
        let Ok(wears) = wears(row);

        match (wears, &row.more) {
            (Wears::Nothing, _) | (Wears::WhatElse, None) => {}
            (Wears::WhatElse, Some(more)) => {
                let Ok(mark) = self.what_else(more.clone());

                match row.stacked {
                    true => mark.set_valign(Align::Start),
                    false => {},
                }

                match line.last_child().filter(|last| last.widget_name() == named::INTO) {
                    Some(into) => line.insert_child_after(&mark, into.prev_sibling().as_ref()),
                    None => line.append(&mark),
                }
            }
        }

        Ok(line)
    }

    fn what_else(self: &Rc<Self>, more: Act) -> Result<Button, Never> {
        let end = Button::with_label(marks::ELSE);
        end.set_widget_name(named::ELSE);
        end.set_valign(Align::Center);

        let panel = Rc::clone(self);
        end.connect_clicked(move |_| {
            let Ok(()) = panel.came_back();

            match more(&panel) {
                true => {
                    let Ok(()) = panel.shut();
                }
                false => {}
            }
        });

        Ok(end)
    }

    fn spelled(self: &Rc<Self>, row: &Row) -> Result<GtkBox, Never> {
        let line = GtkBox::new(Orientation::Horizontal, 0);

        match row.nothing {
            true => {
                let said = Label::new(Some(&row.says));
                said.set_hexpand(true);
                said.set_wrap(true);
                said.set_justify(gtk4::Justification::Center);
                line.append(&said);

                return Ok(line);
            }
            false => {},
        }

        match &row.across {
            Some(across) => {
                let Ok(strip) = self.strip(across);

                line.append(&strip);

                return Ok(line);
            }
            None => {},
        }

        match &row.picture {
            Picture::Bar(bar) => {
                let Ok(from) = edge(&row.says, 0.0);
                let Ok(to) = edge(&row.aside, 1.0);

                let under = GtkBox::new(Orientation::Horizontal, 0);
                under.append(&from);
                under.append(&to);

                let Ok(scrub) = scrub(self, *bar, row.seek.clone());

                let held = GtkBox::new(Orientation::Vertical, 0);
                held.set_hexpand(true);
                held.append(&scrub);
                held.append(&under);
                line.append(&held);

                return Ok(line);
            }
            Picture::None
            | Picture::Space
            | Picture::Named(_)
            | Picture::At(_)
            | Picture::Sleeve(_)
            | Picture::Showing(_)
            | Picture::Playing(_)
            | Picture::Written(_) => {},
        }

        match &row.picture {
            Picture::None => {}
            Picture::Written(markup) => {
                let Ok(written) = written(markup);

                line.append(&written);
            }
            Picture::Sleeve(art) => {
                let Ok(sleeve) = sleeve(art.as_deref());

                line.append(&sleeve);
            }
            Picture::Showing(at) => {
                let Ok(room) = self.picture_room();
                let Ok(down) = self.down();
                let Ok(showing) = showing(at.as_deref(), room, down);

                line.append(&showing);
            }
            Picture::Playing(at) => {
                let Ok(room) = self.picture_room();
                let Ok(down) = self.down();
                let Ok(playing) = playing(at.as_deref(), room, down);

                line.append(&playing);
            }
            Picture::Bar(_) => {}
            picture @ (Picture::Space | Picture::Named(_) | Picture::At(_)) => {
                let Ok(shown) = shown(picture);

                line.append(&shown);
            }
        }

        match row.stacked {
            true => {
                let words = GtkBox::new(Orientation::Vertical, 0);
                words.set_halign(Align::Start);
                words.set_valign(Align::Center);
                words.set_hexpand(true);
                words.set_margin_start(2i32.saturating_mul(GAP));

                let said = Label::new(Some(&row.says));
                said.set_xalign(0.0);
                said.set_wrap(true);
                words.append(&said);

                match row.aside.is_empty() {
                    true => {},
                    false => {
                        let note = Label::new(Some(&row.aside));
                        note.set_widget_name(named::ASIDE);
                        note.set_xalign(0.0);
                        note.set_wrap(true);
                        words.append(&note);
                    }
                }

                line.append(&words);

                return Ok(line);
            }
            false => {},
        }

        match row.middle {
            true => {
                let words = GtkBox::new(Orientation::Horizontal, 2i32.saturating_mul(GAP));
                words.set_halign(gtk4::Align::Center);
                words.set_hexpand(true);

                let said = Label::new(Some(&row.says));
                said.set_justify(gtk4::Justification::Center);
                said.set_wrap(true);

                match !row.says.is_empty() {
                    true => {
                        words.append(&said);
                    }
                    false => {},
                }

                match !row.aside.is_empty() {
                    true => {
                        let note = Label::new(Some(&row.aside));
                        note.set_widget_name(named::ASIDE);
                        note.set_justify(gtk4::Justification::Center);
                        words.append(&note);
                    }
                    false => {},
                }

                let Ok((less, more)) = ends_of(row);

                match (&row.level, less.is_empty() && more.is_empty()) {
                    (Some(level), false) => {
                        let Ok(back) = self.step(level.clone(), less, -1);
                        let Ok(on) = self.step(level.clone(), more, 1);

                        line.append(&back);
                        line.append(&words);
                        line.append(&on);
                    }
                    (Some(_), true) | (None, _) => {
                        line.set_halign(gtk4::Align::Center);
                        line.append(&words);
                    }
                }

                return Ok(line);
            }
            false => {},
        }

        let label = Label::new(Some(&row.says));
        label.set_xalign(0.0);
        label.set_ellipsize(EllipsizeMode::End);

        match self.column > 0 && row.does.is_none() {
            true => {
                label.set_size_request(self.column, -1);
                line.append(&label);
                let said = Label::new(Some(&row.aside));
                said.set_widget_name(named::SAID);
                said.set_xalign(0.0);
                said.set_wrap(true);
                said.set_hexpand(true);
                line.append(&said);

                return Ok(line);
            }
            false => {},
        }

        label.set_hexpand(true);
        line.append(&label);

        let Ok((less, more)) = ends_of(row);

        match &row.level {
            Some(level) => {
                match !less.is_empty() {
                    true => {
                        let Ok(back) = self.step(level.clone(), less, -1);

                        line.append(&back);
                    }
                    false => {},
                }
            }
            None => {},
        }

        match !row.aside.is_empty() {
            true => {
                let note = Label::new(Some(&row.aside));
                note.set_widget_name(named::ASIDE);

                match row.level.is_some() {
                    true => {
                        note.set_width_chars(16);
                    }
                    false => {},
                }

                line.append(&note);
            }
            false => {},
        }

        match &row.level {
            Some(level) => {
                match !more.is_empty() {
                    true => {
                        let Ok(on) = self.step(level.clone(), more, 1);

                        line.append(&on);
                    }
                    false => {},
                }
            }
            None => {},
        }

        match &row.tail {
            Some(tail) => {
                match tail {
                    Picture::Written(markup) => {
                        let Ok(written) = written(markup);

                        line.append(&written);
                    }
                    Picture::None => {}
                    Picture::Space
                    | Picture::Named(_)
                    | Picture::At(_)
                    | Picture::Sleeve(_)
                    | Picture::Showing(_)
                    | Picture::Playing(_)
                    | Picture::Bar(_) => {
                        let Ok(shown) = shown(tail);

                        line.append(&shown);
                    }
                }
            }
            None => {},
        }

        match row.opens {
            true => {
                let into = Label::new(Some(marks::INTO));
                into.set_widget_name(named::INTO);
                line.append(&into);
            }
            false => {},
        }

        Ok(line)
    }

    fn strip(self: &Rc<Self>, across: &crate::page::Across) -> Result<GtkBox, Never> {
        let held = GtkBox::new(Orientation::Horizontal, 0);
        held.set_hexpand(true);
        held.set_halign(gtk4::Align::Center);

        for (at, press) in across.presses.iter().enumerate() {
            let button = Button::new();
            button.set_widget_name(named::PRESS);
            let Ok(named) = press.icon.name();
            let icon = gtk4::Image::from_icon_name(named);
            icon.set_pixel_size(PRESSED);
            button.set_child(Some(&icon));

            match press.now {
                true => {
                    button.add_css_class("now");
                }
                false => {},
            }

            match at == across.at {
                true => {
                    button.add_css_class("standing");
                }
                false => {},
            }

            match press.chief {
                true => {
                    button.add_css_class("chief");
                }
                false => {},
            }

            let does = Arc::clone(&press.does);
            let panel = Rc::clone(self);
            button.connect_clicked(move |_| {
                does(&panel);

                let Ok(()) = panel.redraw();
            });
            held.append(&button);
        }

        Ok(held)
    }

    fn sweeping(self: &Rc<Self>, level: &crate::page::Level) -> Result<GestureSwipe, Never> {
        let swipe = GestureSwipe::new();
        swipe.set_touch_only(false);

        let level = level.clone();
        let panel = Rc::clone(self);
        swipe.connect_swipe(move |_, across, down| {
            let Ok(swept) = swept(across, down);

            let Ok(Some(step)) = swept.step() else { return };

            let Ok(()) = panel.came_back();

            level(step);

            let Ok(()) = panel.redraw();
        });

        Ok(swipe)
    }

    fn step(
        self: &Rc<Self>,
        level: crate::page::Level,
        mark: &str,
        amount: i32,
    ) -> Result<Button, Never> {
        let end = Button::with_label(mark);
        end.set_widget_name(named::STEP);
        end.set_valign(gtk4::Align::Center);
        let panel = Rc::clone(self);
        end.connect_clicked(move |_| {
            level(amount);

            let Ok(()) = panel.redraw();
        });

        Ok(end)
    }

    fn asking(
        self: &Rc<Self>,
        question: &str,
        then: Answer,
        secret: Secret,
    ) -> Result<(), Never> {
        self.state.borrow_mut().asking = Some(then);
        let Ok(()) = emptied(&self.rows, None);
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
        entry.connect_activate(move |entry| {
            let Ok(()) = panel.answered(&entry.text());
        });
        box_.append(&entry);
        row.set_child(Some(&box_));
        self.rows.append(&row);
        entry.grab_focus();

        Ok(())
    }

    fn wondering(
        self: &Rc<Self>,
        question: &str,
        about: &str,
        does: &[&str],
        then: Taken,
    ) -> Result<(), Never> {
        let Ok(()) = emptied(&self.rows, None);
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

            match at > 0 {
                true => {
                    answer.add_css_class("does");
                }
                false => {},
            }

            let panel = Rc::clone(self);
            answer.connect_clicked(move |_| {
                let Ok(()) = panel.took(at);
            });
            foot.append(&answer);
            answers.push(answer);
        }

        let held = GtkBox::new(Orientation::Vertical, BREATH);
        held.append(&line);
        held.append(&foot);
        let row = ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.set_child(Some(&held));
        self.rows.append(&row);
        self.rows.select_row(None::<&ListBoxRow>);

        self.state.borrow_mut().sure = Some(Sure { then, at: 0, answers });

        self.leaning()
    }

    fn leaning(&self) -> Result<(), Never> {
        let state = self.state.borrow();

        let Some(sure) = &state.sure else { return Ok(()) };

        for (at, answer) in sure.answers.iter().enumerate() {
            match at == sure.at {
                true => answer.add_css_class("here"),
                false => answer.remove_css_class("here"),
            }
        }

        Ok(())
    }

    fn lean(self: &Rc<Self>, step: i32) -> Result<(), Never> {
        {
            let mut state = self.state.borrow_mut();

            let Some(sure) = &mut state.sure else { return Ok(()) };

            let last = sure.answers.len().saturating_sub(1);
            let Ok(at) = fitted::<usize, i32>(sure.at);
            let Ok(last) = fitted::<usize, i32>(last);
            let going = at.saturating_add(step);
            let Ok(landed) = fitted(going.clamp(0, last));

            sure.at = landed;
        }

        self.leaning()
    }

    fn answered_sure(self: &Rc<Self>) -> Result<(), Never> {
        let at = self.state.borrow().sure.as_ref().map(|sure| sure.at);

        match at {
            Some(at) => {
                let Ok(()) = self.took(at);
            }
            None => {},
        }

        Ok(())
    }

    fn took(self: &Rc<Self>, at: usize) -> Result<(), Never> {
        let Some(sure) = self.state.borrow_mut().sure.take() else { return Ok(()) };

        match at.checked_sub(1) {
            None => {
                let Ok(()) = self.draw();
            }
            Some(which) => {
                (sure.then)(self, which);

                let Ok(()) = self.redraw();
            }
        }

        Ok(())
    }

    fn left_alone(self: &Rc<Self>) -> Result<(), Never> {
        self.state.borrow_mut().sure = None;

        self.draw()
    }

    fn driving(&self) -> Result<Driving, Never> {
        match self.state.borrow().sure.is_some() {
            true => return Ok(Driving::Sure),
            false => {},
        }

        match self.state.borrow().asking.is_some() {
            true => return Ok(Driving::Question),
            false => {},
        }

        let Ok(typing) = self.typing_here();

        Ok(match typing {
            Typing::Yes => Driving::Search,
            Typing::No => Driving::Panel,
        })
    }

    fn set_the_rows(&self) -> Result<(), Never> {
        let opened = self.state.borrow().opened;
        let Ok(set) = self.set_here();

        self.rows.set_valign(match (opened, set) {
            (Opened::Out, Set::FromTheTop)
            | (Opened::Out, Set::InTheMiddle)
            | (Opened::No, Set::InTheMiddle) => Align::Center,
            (Opened::No, Set::FromTheTop) => Align::Fill,
        });

        Ok(())
    }

    fn set_here(&self) -> Result<Set, Never> {
        let state = self.state.borrow();

        Ok(state.pages.get(state.here).map_or(Set::FromTheTop, |page| page.set))
    }

    fn seeks_here(&self) -> Result<Seeks, Never> {
        let state = self.state.borrow();

        Ok(match state.pages.get(state.here).is_some_and(|page| page.sought.is_some()) {
            true => Seeks::Yes,
            false => Seeks::No,
        })
    }

    fn typing_here(&self) -> Result<Typing, Never> {
        let on = self.rows.selected_row().is_some_and(|row| {
            let Ok(typing) = self.typing_at(row.index());

            typing == Typing::Yes
        });

        Ok(match on {
            true => Typing::Yes,
            false => Typing::No,
        })
    }

    fn typing_at(&self, at: impl TryInto<usize>) -> Result<Typing, Never> {
        let Ok(at) = at.try_into() else { return Ok(Typing::No) };

        Ok(match self.state.borrow().placed.get(at).is_some_and(|row| row.typing) {
            true => Typing::Yes,
            false => Typing::No,
        })
    }

    fn typed_into(&self) -> Result<(), Never> {
        match self.search.has_focus() {
            true => return Ok(()),
            false => {},
        }

        self.search.grab_focus();
        self.search.set_position(-1);

        Ok(())
    }

    fn seen(&self, row: &ListBoxRow) -> Result<(), Never> {
        let Ok(typing) = self.typing_at(row.index());

        match typing {
            Typing::Yes => {
                let Ok(()) = self.typed_into();
                let Ok(()) = self.keep_the_highlight_in_view();
            }
            Typing::No => {
                row.grab_focus();
            }
        }

        Ok(())
    }

    fn seeking(&self) -> Result<(), Never> {
        let about = {
            let state = self.state.borrow();
            state
                .pages
                .get(state.here)
                .and_then(|page| page.sought.as_ref().map(|sought| sought.about.clone()))
        };

        match about {
            Some(about) => {
                self.search.set_placeholder_text(Some(&about));
            }
            None => {},
        }

        Ok(())
    }

    fn seeks(self: &Rc<Self>) -> Result<(), Never> {
        let panel = Rc::clone(self);
        self.search.connect_changed(move |entry| {
            let Ok(()) = panel.narrowed(&entry.text());
        });

        Ok(())
    }

    fn narrowed(self: &Rc<Self>, word: &str) -> Result<(), Never> {
        let then = {
            let state = self.state.borrow();
            state.pages.get(state.here).and_then(|page| page.sought.clone()).map(|sought| sought.then)
        };

        match then {
            Some(then) => {
                then(self, word);
            }
            None => {},
        }

        Ok(())
    }

    fn answered(self: &Rc<Self>, word: &str) -> Result<(), Never> {
        let then = self.state.borrow_mut().asking.take();

        match then {
            Some(then) => {
                then(self, word);
            }
            None => {},
        }

        self.redraw()
    }

    fn reshaped(self: &Rc<Self>) -> Result<(), Never> {
        match self.state.borrow().reshaping {
            true => return Ok(()),
            false => {},
        }

        self.state.borrow_mut().reshaping = true;
        let panel = Rc::clone(self);
        glib::idle_add_local_once(move || {
            panel.state.borrow_mut().reshaping = false;

            let Ok(()) = panel.fit();
        });

        Ok(())
    }

    fn across(&self) -> Result<i32, Never> {
        let Ok(given) = self.given();

        match self.state.borrow().opened {
            Opened::Out => Ok(given.0),
            Opened::No => {
                let Ok(monitor) = self.monitor();

                fitting::across(given.0, monitor.0)
            }
        }
    }

    fn picture_room(&self) -> Result<i32, Never> {
        let Ok(across) = self.across();

        Ok(match self.state.borrow().opened {
            Opened::Out => across,
            Opened::No => across.saturating_sub(2i32.saturating_mul(MARGIN)),
        })
    }

    fn down(&self) -> Result<i32, Never> {
        let (strip, under) = {
            let state = self.state.borrow();
            let strip = match state.opened {
                Opened::Out => fitting::Strip::Hidden,
                Opened::No => fitting::Strip::Shown,
            };

            (strip, state.under)
        };
        let Ok(tall) = self.tall();

        fitting::showing(tall, strip, under)
    }

    fn tall(&self) -> Result<i32, Never> {
        let Ok(given) = self.given();

        match self.state.borrow().opened {
            Opened::Out => Ok(given.1),
            Opened::No => {
                let Ok(monitor) = self.monitor();

                fitting::ceiling(given.1, monitor.1)
            }
        }
    }

    fn given(&self) -> Result<(i32, i32), Never> {
        let granted = (self.window.width(), self.window.height());

        match granted.0 > 1 && granted.1 > 1 {
            true => return Ok(granted),
            false => {},
        }

        let Ok(whose) = namespace();
        let Ok(remembered) = crate::room::last(&whose);
        let Ok(screen) = self.monitor();

        Ok(match screen.0 > 1 && screen.1 > 1 {
            true => (remembered.0.min(screen.0), remembered.1.min(screen.1)),
            false => remembered,
        })
    }

    fn monitor(&self) -> Result<(i32, i32), Never> {
        let Some(display) = gtk4::gdk::Display::default() else { return Ok((0, 0)) };

        let monitors = display.monitors();

        let Some(first) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() else {
            return Ok((0, 0));
        };

        let screen = first.geometry();

        Ok((screen.width(), screen.height()))
    }

    pub fn fit(self: &Rc<Self>) -> Result<(), Never> {
        let Ok(whose) = namespace();
        let Ok(()) = crate::room::keep(&whose, (self.window.width(), self.window.height()));
        let Ok(wide) = self.across();

        let was = self.state.borrow().wide;

        match wide != was {
            true => {
                self.state.borrow_mut().wide = wide;

                let Ok(()) = self.mark();
            }
            false => {},
        }

        let Ok(seeks) = self.seeks_here();
        let first = i32::from(seeks == Seeks::Yes);
        let tall = self
            .rows
            .row_at_index(first)
            .or_else(|| self.rows.row_at_index(0))
            .map_or(0, |first| {
                let Ok(tall) = tall_as(&first);

                tall
            });
        let Ok(top) = tall_as(&self.top);
        let frame = top
            .saturating_add(2i32.saturating_mul(EDGE))
            .saturating_add(self.scroller.margin_top())
            .saturating_add(self.scroller.margin_bottom());
        let Ok(ceiling) = self.tall();
        let Ok(tall_enough) = fitting::tall_enough(frame, tall, ceiling);

        let asking = (wide, tall_enough);

        let asked_before = self.state.borrow().asked;

        match asked_before != Some(asking) {
            true => {
                self.state.borrow_mut().asked = Some(asking);
                self.card.set_size_request(wide, tall_enough);

                let panel = Rc::clone(self);
                glib::idle_add_local_once(move || {
                    let Ok(()) = panel.keep_the_highlight_in_view();
                });
            }
            false => {},
        }

        Ok(())
    }

    fn keep_the_highlight_in_view(&self) -> Result<(), Never> {
        let Some(row) = self.rows.selected_row() else { return Ok(()) };

        let at = row.allocation();
        let (top, tall) = (f64::from(at.y()), f64::from(at.height()));
        let scroll = self.scroller.vadjustment();
        let (seen, page) = (scroll.value(), scroll.page_size());

        match (top < seen, top + tall > seen + page) {
            (true, _) => scroll.set_value(top),
            (false, true) => scroll.set_value(top + tall - page),
            (false, false) => {},
        }

        Ok(())
    }

    fn watch_everything(self: &Rc<Self>) -> Result<(), Never> {
        let watching: Vec<(usize, crate::page::Watch)> = self
            .state
            .borrow()
            .pages
            .iter()
            .enumerate()
            .filter_map(|(index, page)| page.watch.clone().map(|watch| (index, watch)))
            .collect();

        for (index, watch) in watching {
            let Ok(()) = self.watch(index, &watch);
        }

        Ok(())
    }

    fn watch(self: &Rc<Self>, index: usize, watch: &crate::page::Watch) -> Result<(), Never> {
        let Some((program, rest)) = watch.argv.split_first() else { return Ok(()) };

        let mut watching = Command::new(program);
        watching.args(rest).stdout(Stdio::piped()).stderr(Stdio::null());
        let Ok(()) = console_wait_times::not_a_press(&mut watching);

        let Ok(mut running) = alongside(&mut watching) else { return Ok(()) };

        let Ok(Some(reading)) = running.reading() else { return Ok(()) };

        self.state.borrow_mut().watchers.push(running);

        let panel = Rc::clone(self);
        let about = watch.about.clone();
        glib::spawn_future_local(async move {
            let mut lines = BufReader::new(reading);

            loop {
                let read = gtk4::gio::spawn_blocking(move || {
                    let mut said = String::new();

                    let Ok(got) = lines.read_line(&mut said) else {
                        return (lines, said, 0);
                    };

                    (lines, said, got)
                })
                .await;

                let Ok((back, said, got)) = read else { break };

                match got == 0 {
                    true => break,
                    false => {},
                }

                lines = back;

                match said.contains(&about) {
                    true => {
                        let Ok(()) = panel.heard(index);
                    }
                    false => {},
                }
            }
        });

        Ok(())
    }

    fn heard(self: &Rc<Self>, index: usize) -> Result<(), Never> {
        match self.state.borrow().due {
            true => return Ok(()),
            false => {},
        }

        self.state.borrow_mut().due = true;
        let panel = Rc::clone(self);
        glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
            panel.state.borrow_mut().due = false;
            let state = panel.state.borrow();
            let redraw = state.here == index && state.asking.is_none();
            drop(state);

            match redraw {
                true => {
                    let Ok(()) = panel.redraw();
                }
                false => {},
            }
        });

        Ok(())
    }

    fn say_which_tab(&self, index: usize) -> Result<(), Never> {
        let state = self.state.borrow();

        match state.pages.get(index) {
            Some(page) => {
                match crate::door::saying(&page.title) {
                    Ok(_) => {},
                    Err(fault) => {
                        eprintln!("the tab in front could not be written down: {fault}");
                    }
                }
            }
            None => {},
        }

        Ok(())
    }

    pub fn shut(&self) -> Result<(), Never> {
        match crate::door::forget() {
            Ok(_) => {},
            Err(fault) => {
                eprintln!("the tab in front could not be forgotten: {fault}");
            }
        }

        let Ok(()) = self.stop_watching();

        self.window.destroy();

        let Ok(()) = crate::chooser::gone();

        match &self.over {
            Over::Loop(waiting) => waiting.quit(),
            Over::Told(then) => then(),
        }

        Ok(())
    }

    fn stop_watching(&self) -> Result<(), Never> {
        self.state.borrow_mut().watchers.clear();

        Ok(())
    }

    pub fn note(self: &Rc<Self>, said: &str) -> Result<(), Never> {
        let stamp = {
            let mut state = self.state.borrow_mut();
            state.noted = state.noted.saturating_add(1);
            state.noted
        };
        self.note.set_text(said);
        self.note.set_visible(true);
        let panel = Rc::clone(self);
        glib::timeout_add_local_once(A_MOMENT, move || {
            match panel.state.borrow().noted == stamp {
                true => {
                    panel.note.set_visible(false);
                }
                false => {},
            }
        });

        Ok(())
    }

    pub fn later(self: &Rc<Self>, argv: Vec<String>) -> Result<(), Never> {
        let panel = Rc::clone(self);
        glib::spawn_future_local(async move {
            let _ = gtk4::gio::spawn_blocking(move || {
                let Some((program, rest)) = argv.split_first() else { return };

                let mut doing = Command::new(program);
                doing.args(rest).stdout(Stdio::null()).stderr(Stdio::null());
                let Ok(()) = console_wait_times::not_a_press(&mut doing);
                let _ = doing.status();
            })
            .await;

            let Ok(()) = panel.redraw();
        });

        Ok(())
    }

    pub fn leave_running(self: &Rc<Self>, argv: Vec<String>) -> Result<(), Never> {
        let Ok(()) = crate::running::left_running(&argv);

        let panel = Rc::clone(self);
        glib::timeout_add_local_once(crate::running::SETTLING, move || {
            let Ok(()) = panel.redraw();
        });

        Ok(())
    }
}

fn laid_over_everything(window: &Window) -> Result<(), Never> {
    let Ok(whose) = namespace();

    window.init_layer_shell();
    window.set_namespace(Some(&whose));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::OnDemand);

    for edge in [Edge::Bottom, Edge::Left, Edge::Right, Edge::Top] {
        window.set_anchor(edge, true);
    }

    window.connect_realize(|_| {
        let Ok(()) = opening::mark("surface");
    });
    window.connect_map(|_| {
        let Ok(()) = opening::mark("mapped");
    });

    Ok(())
}

fn namespace() -> Result<String, Never> {
    crate::whose::name()
}

fn arrow(mark: &str) -> Result<Button, Never> {
    let end = Button::with_label(mark);
    end.set_widget_name(named::MORE);

    Ok(end)
}

fn edge(said: &str, toward: f32) -> Result<Label, Never> {
    let drawn = Label::new(Some(said));
    drawn.set_widget_name(named::ASIDE);
    drawn.set_width_chars(TIME_WIDE);
    drawn.set_xalign(toward);
    drawn.set_hexpand(true);
    drawn.set_margin_start(2i32.saturating_mul(GAP));
    drawn.set_margin_end(2i32.saturating_mul(GAP));

    Ok(drawn)
}

fn written(markup: &str) -> Result<Label, Never> {
    let drawn = Label::new(None);
    drawn.set_widget_name(named::COVER);
    drawn.set_markup(markup);
    drawn.set_xalign(0.0);
    drawn.set_yalign(0.0);

    Ok(drawn)
}

fn along(bar: crate::page::Bar) -> Result<f64, Never> {
    let Ok(at) = bar.at.min(bar.of).float();
    let Ok(of) = bar.of.float();

    Ok(match bar.of > 0 {
        true => (at / of).clamp(0.0, 1.0),
        false => 0.0,
    })
}

fn scrub(
    panel: &Rc<Panel>,
    bar: crate::page::Bar,
    seek: Option<crate::page::Seek>,
) -> Result<ProgressBar, Never> {
    let Ok(along) = along(bar);

    let drawn = ProgressBar::new();
    drawn.set_widget_name(named::BAR);
    drawn.set_fraction(along);
    drawn.set_hexpand(true);
    drawn.set_valign(Align::Center);
    drawn.set_margin_start(2i32.saturating_mul(GAP));
    drawn.set_margin_end(2i32.saturating_mul(GAP));

    match seek {
        Some(seek) => {
            let touch = GestureClick::new();
            let held = drawn.clone();
            let panel = Rc::clone(panel);
            touch.connect_pressed(move |_, _, x, _| {
                let width = f64::from(held.allocated_width().max(1));
                let frac = (x / width).clamp(0.0, 1.0);
                seek(&panel, frac);

                let Ok(()) = panel.redraw();
            });
            drawn.add_controller(touch);
        }
        None => {},
    }

    Ok(drawn)
}

fn sleeve(art: Option<&Path>) -> Result<gtk4::Image, Never> {
    let held = gtk4::Image::new();
    held.set_widget_name(named::SLEEVE);
    held.set_pixel_size(SLEEVE);
    held.set_size_request(SLEEVE, SLEEVE);
    held.set_hexpand(false);
    held.set_halign(gtk4::Align::Center);
    held.set_valign(gtk4::Align::Center);

    match art.map(middle) {
        Some(Ok(square)) => {
            held.set_paintable(Some(&square));
        }
        Some(Err(_)) | None => {},
    }

    Ok(held)
}

const SHARP: i32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Bare {
    Yes,
    No,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Wears {
    WhatElse,
    Nothing,
}

fn wears(row: &Row) -> Result<Wears, Never> {
    let Ok(bare) = bare(row);

    Ok(match (row.more.is_some(), bare) {
        (true, Bare::No) => Wears::WhatElse,
        (true, Bare::Yes) | (false, _) => Wears::Nothing,
    })
}

fn bare(row: &Row) -> Result<Bare, Never> {
    Ok(match &row.ends {
        Some((less, more)) if less.is_empty() && more.is_empty() => Bare::Yes,
        Some(_) | None => Bare::No,
    })
}

fn ends_of(row: &Row) -> Result<(&str, &str), Never> {
    Ok(match &row.ends {
        Some((less, more)) => (less.as_str(), more.as_str()),
        None => (marks::LESS, marks::MORE),
    })
}

fn showing(at: Option<&Path>, room: i32, down: i32) -> Result<gtk4::ScrolledWindow, Never> {
    let held = gtk4::Picture::new();
    held.set_widget_name(named::SHOWING);
    held.set_can_shrink(true);
    held.set_hexpand(true);
    held.set_vexpand(true);

    let (across, down) = match at {
        Some(at) => {
            let Ok((across, down, want)) = box_for(at, room, down);
            let Ok(()) = fetch(&held, at.to_path_buf(), want);

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

    Ok(frame)
}

pub type Films = Rc<dyn Fn(&Path) -> Option<gtk4::gdk::Paintable>>;

thread_local! {
    static FILMS: RefCell<Option<Films>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Drawn {
    ThePictureAlone,
    TheCardWithIt,
}

fn drawn(rows: &[Row]) -> Result<Drawn, Never> {
    let alone = match rows {
        [one] => matches!(one.picture, Picture::Showing(_) | Picture::Playing(_)),
        [] | [_, _, ..] => false,
    };

    Ok(match alone {
        true => Drawn::ThePictureAlone,
        false => Drawn::TheCardWithIt,
    })
}

fn under(rows: &[Row]) -> Result<i32, Never> {
    let shown = rows
        .iter()
        .any(|row| matches!(row.picture, Picture::Showing(_) | Picture::Playing(_)));

    let Ok(last) = fitted(rows.len().saturating_sub(1));

    Ok(match shown {
        true => last,
        false => 0,
    })
}

fn keep_films_going(rows: &[Row]) -> Result<(), Never> {
    let Some(films) = FILMS.with_borrow(|films| films.clone()) else { return Ok(()) };

    for row in rows {
        match &row.picture {
            Picture::Playing(Some(at)) => {
                films(at);
            }
            Picture::Playing(None)
            | Picture::None
            | Picture::Space
            | Picture::Named(_)
            | Picture::At(_)
            | Picture::Sleeve(_)
            | Picture::Showing(_)
            | Picture::Written(_)
            | Picture::Bar(_) => {},
        }
    }

    Ok(())
}

pub fn films(
    making: impl Fn(&Path) -> Option<gtk4::gdk::Paintable> + 'static,
) -> Result<(), Never> {
    FILMS.with_borrow_mut(|held| *held = Some(Rc::new(making)));

    Ok(())
}

fn playing(at: Option<&Path>, room: i32, down: i32) -> Result<gtk4::ScrolledWindow, Never> {
    let drawn = at.and_then(|at| FILMS.with_borrow(|films| films.clone()).and_then(|films| films(at)));

    let Ok((across, down)) = film_size(drawn.as_ref(), room, down);

    let held = gtk4::Picture::new();
    held.set_widget_name(named::PLAYING);
    held.set_can_shrink(true);
    held.set_hexpand(true);
    held.set_vexpand(true);
    held.set_paintable(drawn.as_ref());

    let frame = gtk4::ScrolledWindow::new();
    frame.set_policy(gtk4::PolicyType::External, gtk4::PolicyType::External);
    frame.set_size_request(across, down);
    frame.set_hexpand(false);
    frame.set_halign(gtk4::Align::Center);
    frame.set_valign(gtk4::Align::Center);
    frame.set_child(Some(&held));

    Ok(frame)
}

fn film_size(
    drawn: Option<&gtk4::gdk::Paintable>,
    room: i32,
    down: i32,
) -> Result<(i32, i32), Never> {
    let (wide, tall) = match drawn {
        Some(held) => (held.intrinsic_width(), held.intrinsic_height()),
        None => (0, 0),
    };

    Ok(match wide > 0 && tall > 0 {
        true => (
            room.min(down.saturating_mul(wide).saturating_div(tall)),
            down.min(room.saturating_mul(tall).saturating_div(wide)),
        ),
        false => (room, down),
    })
}

fn box_for(path: &Path, room: i32, tall_room: i32) -> Result<(i32, i32, i32), Never> {
    let (wide, tall) = match gtk4::gdk_pixbuf::Pixbuf::file_info(path) {
        Some((_, wide, tall)) => (wide.max(1), tall.max(1)),
        None => (room, tall_room),
    };

    let across = wide.min(room).min(tall_room.saturating_mul(wide).saturating_div(tall));
    let down = tall.min(tall_room).min(room.saturating_mul(tall).saturating_div(wide));

    let longer = across.max(down);

    Ok((across, down, longer.saturating_mul(SHARP).min(wide.max(tall))))
}

type Asked = (PathBuf, i32);

type Pixels = (glib::Bytes, i32, i32, usize, bool);

struct Decoding {
    kept: VecDeque<(Asked, gtk4::gdk::Texture)>,
    wanted: Option<(Asked, glib::WeakRef<gtk4::Picture>)>,
    busy: bool,
}

thread_local! {
    static DECODING: RefCell<Decoding> =
        const { RefCell::new(Decoding { kept: VecDeque::new(), wanted: None, busy: false }) };
}

const KEPT: usize = 4;

fn fetch(into: &gtk4::Picture, at: PathBuf, want: i32) -> Result<(), Never> {
    let asked = (at, want);

    let kept = DECODING.with_borrow(|decoding| {
        decoding.kept.iter().find(|(key, _)| key == &asked).map(|(_, texture)| texture.clone())
    });

    match kept {
        Some(texture) => {
            into.set_paintable(Some(&texture));
            DECODING.with_borrow_mut(|decoding| decoding.wanted = None);

            return Ok(());
        }
        None => {},
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

    match begin {
        Some(asked) => {
            let Ok(()) = decode(asked);
        }
        None => {},
    }

    Ok(())
}

fn decode(asked: Asked) -> Result<(), Never> {
    glib::spawn_future_local(async move {
        let (at, want) = asked.clone();

        let read = match gtk4::gio::spawn_blocking(move || {
            let Ok(read) = pixels(&at, want);

            read
        })
        .await
        {
            Ok(read) => read,
            Err(_panicked) => {
                eprintln!("console: {}: the decode did not come back", asked.0.display());

                None
            }
        };

        let Ok(()) = decoded(asked, read);
    });

    Ok(())
}

fn pixels(at: &Path, want: i32) -> Result<Option<Pixels>, Never> {
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
            eprintln!("console: {}: {fault}", at.display());

            return Ok(None);
        }
    };

    let Ok(rowstride) = fitted(held.rowstride());

    Ok(Some((
        held.read_pixel_bytes(),
        held.width(),
        held.height(),
        rowstride,
        held.has_alpha(),
    )))
}

fn decoded(asked: Asked, read: Option<Pixels>) -> Result<(), Never> {
    let next = DECODING.with_borrow_mut(|decoding| {
        decoding.busy = false;

        match read {
            Some((bytes, wide, tall, stride, alpha)) => {
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
            None => {},
        }

        let (wants, into) = decoding.wanted.take()?;

        match wants != asked {
            true => {
                decoding.wanted = Some((wants.clone(), into));
                decoding.busy = true;

                return Some(wants);
            }
            false => {},
        }

        match into.upgrade() {
            Some(into) => {
                match decoding.kept.iter().find(|(key, _)| key == &asked) {
                    Some((_, texture)) => {
                        into.set_paintable(Some(texture));
                    }
                    None => {},
                }
            }
            None => {},
        }

        None
    });

    match next {
        Some(wants) => {
            let Ok(()) = decode(wants);
        }
        None => {},
    }

    Ok(())
}

fn middle(path: &Path) -> Result<gtk4::gdk::Texture, gtk4::glib::Error> {
    let whole = gtk4::gdk_pixbuf::Pixbuf::from_file(path)?;
    let side = whole.width().min(whole.height());
    let square = whole.new_subpixbuf(
        whole.width().saturating_sub(side).saturating_div(2),
        whole.height().saturating_sub(side).saturating_div(2),
        side,
        side,
    );
    let want = SLEEVE.saturating_mul(2);
    let drawn = match side > want {
        true => square.scale_simple(want, want, gtk4::gdk_pixbuf::InterpType::Bilinear).unwrap_or(square),
        false => square,
    };

    Ok(gtk4::gdk::Texture::for_pixbuf(&drawn))
}

fn shown(picture: &Picture) -> Result<gtk4::Image, Never> {
    let held = gtk4::Image::new();
    held.set_widget_name(named::ICON);
    held.set_pixel_size(PICTURE);
    held.set_size_request(PICTURE, PICTURE);
    held.set_margin_end(2i32.saturating_mul(GAP));

    match picture {
        Picture::Named(icon) => {
            let Ok(named) = icon.name();

            held.set_icon_name(Some(named));
        }
        Picture::At(path) => {
            let Ok(ready) = crate::pictures::ready(path);

            match ready {
                Some(ready) => held.set_paintable(Some(&ready)),
                None if path.exists() => held.set_from_file(Some(path)),
                None => {}
            }
        }
        Picture::None
        | Picture::Space
        | Picture::Sleeve(_)
        | Picture::Showing(_)
        | Picture::Playing(_)
        | Picture::Written(_)
        | Picture::Bar(_) => {}
    }

    Ok(held)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Secret {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum On {
    TheCard,
    TheDesktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leaving {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seeks {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typing {
    Yes,
    No,
}

fn standing(rows: &[Row], at: usize) -> Result<i32, Never> {
    let fresh = rows.get(at).is_none_or(|row| {
        let Ok(heading) = row.heading();

        heading == Heading::Yes
    });
    let asked = rows.iter().position(|row| {
        let Ok(heading) = row.heading();

        row.chief && heading == Heading::No
    });

    match (fresh, asked) {
        (true, Some(index)) => {
            let Ok(index) = fitted(index);

            return Ok(index);
        }
        (true, None) | (false, _) => {},
    }

    let found = rows.iter().enumerate().skip(at).find(|(_, row)| {
        let Ok(heading) = row.heading();

        heading == Heading::No
    });

    let Ok(whole_9) = fitted(found.map_or(at, |(index, _)| index));
    Ok(whole_9)
}

fn walked(rows: &[Row], at: i32, step: i32) -> Result<i32, Never> {
    let Ok(whole_10) = fitted::<usize, i32>(rows.len().saturating_sub(1));
    let last = whole_10;
    let mut going = at;

    loop {
        going = going.saturating_add(step);

        match going < 0 || going > last {
            true => return Ok(at),
            false => {},
        }

        let Ok(whole_11) = fitted::<i32, usize>(going);
        let clear = rows.get(whole_11).is_none_or(|row| {
            let Ok(heading) = row.heading();

            heading == Heading::No
        });

        match clear {
            true => return Ok(going),
            false => {},
        }
    }
}

fn emptied(list: &ListBox, keep: Option<&ListBoxRow>) -> Result<(), Never> {
    let mut child = list.first_child();

    while let Some(here) = child {
        let next = here.next_sibling();

        match !keep.is_some_and(|row| &here == row.upcast_ref::<gtk4::Widget>()) {
            true => {
                list.remove(&here);
            }
            false => {},
        }

        child = next;
    }

    Ok(())
}

fn wide_as(widget: &impl IsA<gtk4::Widget>) -> Result<i32, Never> {
    Ok(widget.measure(Orientation::Horizontal, -1).1)
}

fn tall_as(widget: &impl IsA<gtk4::Widget>) -> Result<i32, Never> {
    Ok(widget.measure(Orientation::Vertical, -1).1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dressed {
    Already,
    Not,
}

thread_local! {
    static SHEET: RefCell<Option<CssProvider>> = const { RefCell::new(None) };
}

fn dressed() -> Result<(), Never> {
    let worn = SHEET.with(|held| match held.borrow().is_some() {
        true => Dressed::Already,
        false => Dressed::Not,
    });

    match worn {
        Dressed::Already => return Ok(()),
        Dressed::Not => {},
    }

    let Some(display) = gtk4::gdk::Display::default() else { return Ok(()) };

    let Ok(written) = style::sheet();

    let sheet = CssProvider::new();
    sheet.load_from_data(&written);
    gtk4::style_context_add_provider_for_display(
        &display,
        &sheet,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    SHEET.with(|held| *held.borrow_mut() = Some(sheet));

    Ok(())
}

pub fn show(build: Build, column: i32, start: Option<&str>) -> Result<(), Never> {
    let Ok(whose) = namespace();
    let Ok(()) = opening::started(&whose);
    let Ok(waited) = chooser::waited_for_screen();
    let Ok(()) = opening::taking("screen", waited);

    let Ok(toolkit) = toolkit();

    match toolkit {
        Toolkit::Up => {},
        Toolkit::None => return Ok(()),
    }

    let Ok(()) = opening::mark("gtk");
    let waiting = glib::MainLoop::new(None, false);

    match gtk4::gdk::Display::default() {
        Some(screen) => {
            let over = waiting.clone();
            screen.connect_closed(move |_, _| over.quit());
        }
        None => {},
    }

    let Ok(panel) = raised(&whose, build, column, start, Over::Loop(waiting.clone()));

    let asked_of = Rc::clone(&panel);
    let Ok(()) = asked::stops_when_asked(move || {
        let Ok(()) = asked_of.shut();
    });

    waiting.run();

    Ok(())
}

pub fn drawn_here(who: &str, card: crate::card::Card) -> Result<(), Never> {
    let crate::card::Card { build, column, start, done } = card;

    let Ok(()) = crate::whose::named(who);
    let Ok(()) = show(build, column, start.as_deref());
    let Ok(()) = done();

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toolkit {
    Up,
    None,
}

pub fn toolkit() -> Result<Toolkit, Never> {
    Ok(match gtk4::init() {
        Ok(_) => Toolkit::Up,
        Err(fault) => {
            eprintln!("no screen to draw on: {fault}");

            Toolkit::None
        }
    })
}

pub fn raised(
    who: &str,
    build: Build,
    column: i32,
    start: Option<&str>,
    over: Over,
) -> Result<Rc<Panel>, Never> {
    let Ok(()) = crate::whose::named(who);
    let Ok(panel) = Panel::new(build, column, start, over);

    panel.window.present();

    let Ok(()) = opening::mark("shown");

    match panel.window.frame_clock() {
        Some(clock) => {
            let drawn = Rc::clone(&panel);
            clock.connect_after_paint(move |_| {
                let Ok(running) = opening::running();

                match running == opening::Running::No {
                    true => return,
                    false => {},
                }

                match drawn.state.try_borrow() {
                    Ok(state) => {
                        let Ok(whole_12) = fitted(state.placed.len());
                        let Ok(()) = opening::counted("rows", whole_12);

                        match state.pages.get(state.here) {
                            Some(page) => {
                                let Ok(()) = opening::named("tab", &page.title);
                            }
                            None => {},
                        }
                    }
                    Err(_) => {},
                }

                let Ok(()) = opening::mark("frame");
                let Ok(()) = opening::done();
            });
        }
        None => {},
    }

    let opened = Rc::clone(&panel);
    glib::idle_add_local_once(move || {
        let Ok(()) = opened.fit();
        let here = opened.state.borrow().here;
        let Ok(()) = opened.say_which_tab(here);
        let Ok(()) = chooser::drawn();
    });

    Ok(panel)
}

#[cfg(test)]
mod tests {
    use console_external_programs::Program;

    use super::{Drawn, Wears, along, drawn, standing, walked, wears};
    use crate::page::{Does, Heading, Picture, Row};

    #[test]
    fn a_row_that_offers_something_behind_y_wears_the_mark_for_it() {
        let Ok(offering) = heading("Beach").offering(|_| false);

        assert_eq!(wears(&offering), Ok(Wears::WhatElse));
    }

    #[test]
    fn a_row_that_offers_nothing_wears_nothing() {
        assert_eq!(wears(&heading("Beach")), Ok(Wears::Nothing));
    }

    #[test]
    fn a_row_that_asked_for_no_marks_wears_none_even_when_it_offers() {
        let Ok(offering) = heading("Beach").offering(|_| false);
        let Ok(bare) = offering.ended("", "");

        assert_eq!(wears(&bare), Ok(Wears::Nothing));
    }

    fn heading(says: &str) -> Row {
        let Ok(row) = Row::said(says, "");

        row
    }

    fn chooseable(says: &str) -> Row {
        let Ok(yes) = Program::True.name();
        let Ok(runs) = Does::run(&[yes]);
        let Ok(row) = Row::new(says, "", runs);

        row
    }

    fn showing(picture: Picture) -> Row {
        let Ok(row) = Row::showing(picture);

        row
    }

    fn levelled(row: Row) -> Row {
        let Ok(row) = row.levelled(std::sync::Arc::new(|_| ()));

        row
    }

    fn chief(row: Row) -> Row {
        let Ok(row) = row.chief();

        row
    }

    fn naming(says: &str, aside: &str) -> Row {
        let Ok(row) = Row::naming(says, aside);

        row
    }

    fn nothing(says: &str) -> Row {
        let Ok(row) = Row::nothing(says);

        row
    }

    #[test]
    fn a_card_showing_nothing_but_the_picture_is_a_page_that_wants_the_screen() {
        let alone = [showing(Picture::Showing(None))];
        assert_eq!(drawn(&alone), Ok(Drawn::ThePictureAlone));

        let film = [showing(Picture::Playing(None))];
        assert_eq!(drawn(&film), Ok(Drawn::ThePictureAlone));
    }

    #[test]
    fn a_picture_with_the_cards_own_rows_under_it_is_the_card_still_being_used() {
        let with_it = [showing(Picture::Showing(None)), heading("beach.jpg")];
        assert_eq!(drawn(&with_it), Ok(Drawn::TheCardWithIt));
    }

    #[test]
    fn a_card_that_is_only_rows_is_not_a_picture_that_has_gone_quiet() {
        assert_eq!(drawn(&[]), Ok(Drawn::TheCardWithIt));
        assert_eq!(drawn(&[heading("Screen")]), Ok(Drawn::TheCardWithIt));
    }

    #[test]
    fn the_highlight_opens_on_something_it_can_act_on() {
        let rows = [heading("Search with"), chooseable("DuckDuckGo")];
        assert_eq!(standing(&rows, 0), Ok(1));
    }

    #[test]
    fn a_row_held_at_a_level_is_something_to_act_on() {
        let rows = [levelled(heading("Screen")), chooseable("Balanced")];
        assert_eq!(standing(&rows, 0), Ok(0));
    }

    #[test]
    fn a_card_that_names_the_row_it_opens_on_opens_there() {
        let rows = [
            heading("Blue Monday"),
            levelled(chooseable("0:31")),
            chief(chooseable("the transport")),
        ];
        assert_eq!(standing(&rows, 0), Ok(2));
    }

    #[test]
    fn a_thumb_that_walked_off_it_is_left_where_it_walked_to() {
        let rows = [
            heading("Blue Monday"),
            levelled(chooseable("0:31")),
            chief(chooseable("the transport")),
        ];
        assert_eq!(standing(&rows, 1), Ok(1));
    }

    #[test]
    fn a_card_cannot_open_on_a_row_nothing_happens_to() {
        let rows = [chief(heading("Blue Monday")), chooseable("0:31")];
        assert_eq!(standing(&rows, 0), Ok(1));
    }

    #[test]
    fn where_you_were_standing_is_where_you_stay() {
        let rows = [chooseable("one"), chooseable("two"), chooseable("three")];
        assert_eq!(standing(&rows, 2), Ok(2));
    }

    #[test]
    fn a_tab_with_nothing_to_act_on_stays_where_it_was_put() {
        assert_eq!(standing(&[heading("Nothing else is playing")], 0), Ok(0));
        assert_eq!(standing(&[], 0), Ok(0));
    }

    #[test]
    fn the_dpad_steps_over_the_name_of_what_a_list_is_about() {
        let rows = [chooseable("\u{2039} Pictures"), naming("holiday.jpg", "2.4 MB"),
                    chooseable("Open"), chooseable("Delete")];
        assert_eq!(walked(&rows, 0, 1), Ok(2));
        assert_eq!(walked(&rows, 2, -1), Ok(0));
    }

    #[test]
    fn a_step_past_the_end_of_a_list_stays_where_it_was() {
        let rows = [chooseable("Open"), chooseable("Delete"), heading("Nothing else")];
        assert_eq!(walked(&rows, 1, 1), Ok(1));
        assert_eq!(walked(&rows, 0, -1), Ok(0));
    }

    #[test]
    fn the_dpad_steps_over_the_panel_saying_a_list_is_empty() {
        let rows = [chooseable("\u{2039} Music"), nothing("Nothing in /home/music"),
                    chooseable("Open the folder")];
        assert_eq!(walked(&rows, 0, 1), Ok(2));
        assert_eq!(walked(&rows, 2, -1), Ok(0));
    }

    #[test]
    fn the_panel_saying_a_list_is_empty_is_not_dressed_as_an_option() {
        assert_eq!(nothing("Nothing is waiting").heading(), Ok(Heading::Yes));
        let Ok(sheet) = crate::style::sheet();

        assert!(sheet.contains("row.nothing {"));
    }

    #[test]
    fn the_line_to_type_into_is_where_a_menu_opens() {
        let Ok(line) = Row::line_to_type_in();
        let rows = [line, chooseable("Files")];
        assert_eq!(standing(&rows, 0), Ok(0));
    }

    #[test]
    fn a_bar_is_filled_as_far_as_the_thing_has_got() {
        let bar = |at, of| {
            let Ok(along) = along(crate::page::Bar { at, of });

            along
        };

        assert!(bar(0, 200) < f64::EPSILON, "nothing played and the bar is not empty");
        assert!((bar(100, 200) - 0.5).abs() < f64::EPSILON);
        assert!((bar(200, 200) - 1.0).abs() < f64::EPSILON, "played out and the bar is not full");
    }

    #[test]
    fn a_bar_with_nothing_to_be_along_is_empty_rather_than_a_division_by_nothing() {
        let bar = |at, of| {
            let Ok(along) = along(crate::page::Bar { at, of });

            along
        };

        assert!(bar(90, 0) < f64::EPSILON, "no length known yet");
        assert!((bar(500, 200) - 1.0).abs() < f64::EPSILON, "past the end");
    }
}
