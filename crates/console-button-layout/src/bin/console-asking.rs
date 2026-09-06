//! The card that asks which button that was.
//!
//!     console-asking screenshot
//!
//! Raised by the setup screen over the row being moved. While it is up the
//! front of the machine does nothing at all, which is the only state in which
//! "press the button you want" is a question somebody can answer: otherwise
//! pressing Legion left to bind it would leave for Game Mode, X would raise the
//! keyboard over the question, and the shoulders would carry the window away.
//!
//! That is `console_input_claim`, and it used to be a profile. The card
//! loaded one that sent every button to a key nothing was listening for and
//! then read the keys rather than the pad, because the profile had taken the
//! pad away from it. It worked, and it was a profile load, which destroys the
//! pad and builds another every time the card is raised. The claim asks the
//! kernel for the same thing and gets it without touching the device: while it
//! is held nothing else receives a press, and it is let go when this program
//! goes, however it goes.
//!
//! Two things follow from reading the pad under the profile the desktop already
//! wears. A press arrives already named, so nothing here has to keep a table of
//! borrowed keys. And a button this desktop cannot route does not arrive at all
//! -- which is the honest answer rather than a lost one, because a button
//! nothing can route is a button nothing could ever be bound to, and the notice
//! after an apply is where that is already said.
//!
//! The name still matters: the compositor lists a layer under the program that
//! drew it, and the controller daemon reads `console-asking` being on the
//! screen as `Mode::Asking` and stands down. The claim is what makes that true
//! rather than agreed.

use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use evdev::{AbsoluteAxisCode, EventType};
use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, Box as GtkBox, Label, Orientation, glib};
use gtk4_layer_shell::{Layer as Shelf, LayerShell};

use console_controller::mode::ASKING;
use console_controller::reading::CARRY_HELD;
use console_button_layout::rows::{Part, WAITING, aloud, every, lowered, parts, question};
use console_button_layout::table;
use console_gamepad::jobs::{ALONE, Binding, Held, Layer, Moved};
use console_gamepad::vocabulary::{button_name, spoken_for};
use console_input_claim::{self as claim, CONTROLLER, Claim, Said, Went, Which};
use console_never::Never;

const PATIENCE: Duration = Duration::from_secs(12);

const READ_IT: Duration = Duration::from_millis(1400);

const READ_BOTH: Duration = Duration::from_millis(2800);

const UNSAID: (i32, i32) = (0, 1);

const NO_WORD: &str = "this desktop has no word for that button";

enum Doing {
    Settling,
    Asking(Box<Reading>),
    Said(Instant, Duration),
}

struct Reading {
    claim: Claim,
    span: (i32, i32),
    layer: Layer,
}

impl Reading {
    fn open(complained: &mut Quiet) -> Result<Option<Self>, Never> {
        let claim = match Claim::of(&CONTROLLER) {
            Ok(claim) => claim,
            Err(refused) => {
                match *complained {
                    Quiet::Yes => {}
                    Quiet::No => {
                        let Ok(said) = refused.said();

                        eprintln!("console-asking: {said}");
                        *complained = Quiet::Yes;
                    }
                }

                return Ok(None);
            }
        };
        let told = match claim.spans(Which::Pad) {
            Ok(told) => told,
            Err(refused) => {
                let Ok(said) = refused.said();

                eprintln!("console-asking: {said}; a chord cannot be seen");
                Vec::new()
            }
        };
        let span = told
            .into_iter()
            .find(|(axis, _)| *axis == AbsoluteAxisCode::ABS_Z)
            .map_or(UNSAID, |(_, span)| span);

        Ok(Some(Reading { claim, span, layer: ALONE }))
    }

    fn pressed(&mut self) -> Result<Option<(String, Layer)>, Never> {
        let Ok(heard) = self.claim.arrived();

        let mut down = None;

        for (which, event) in heard.events {
            let Ok(said) = claim::said(which, event.event_type(), event.code(), event.value());

            match said {
                Said::Pressed { button, went: Went::Down } => {
                    down = down.or_else(|| {
                        let Ok(spoken) = spoken_for(button);

                        Some(spoken.to_string())
                    });
                }
                Said::Pressed { button: _, went: Went::Up } => {}
                Said::Trigger { trigger: _, went: _ } | Said::Unnamed { code: _, went: _ } => {}
                Said::Nothing => {
                    let Ok(()) = self.watched(event.event_type(), event.code(), event.value());
                },
            }
        }

        Ok(down.map(|button| (button, self.layer)))
    }

    fn watched(&mut self, kind: EventType, code: u16, value: i32) -> Result<(), Never> {
        match kind {
            EventType::ABSOLUTE => {}
            _ => return Ok(()),
        }

        let Ok(pulled) = pulled(value, self.span);

        let held = pulled == Held::Down;

        match code {
            _ if code == AbsoluteAxisCode::ABS_Z.0 => self.layer.l2 = held,
            _ if code == AbsoluteAxisCode::ABS_RZ.0 => self.layer.r2 = held,
            _ => {}
        }

        Ok(())
    }
}

fn pulled(value: i32, (low, high): (i32, i32)) -> Result<Held, Never> {
    let span = f64::from(high.saturating_sub(low).max(1));

    Ok(match f64::from(value.saturating_sub(low)) / span > CARRY_HELD {
        true => Held::Down,
        false => Held::Up,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quiet {
    Yes,
    No,
}

struct Card {
    doing: Doing,
    since: Instant,
    part: Part,
    parts: Vec<Part>,
    complained: Quiet,
    saying: Label,
    hint: Label,
}

impl Card {
    fn turn(&mut self) -> Result<glib::ControlFlow, Never> {
        let waited_too_long =
            self.since.elapsed() > PATIENCE && !matches!(self.doing, Doing::Said(_, _));

        match waited_too_long {
            true => return Ok(glib::ControlFlow::Break),
            false => {}
        }

        let heard = match &mut self.doing {
            Doing::Said(when, over) if when.elapsed() > *over => {
                return Ok(glib::ControlFlow::Break);
            }
            Doing::Said(_, _) => return Ok(glib::ControlFlow::Continue),
            Doing::Settling => {
                let Ok(opened) = Reading::open(&mut self.complained);

                match opened {
                    Some(reading) => {
                        self.hint.set_text(WAITING);
                        self.doing = Doing::Asking(Box::new(reading));
                    }
                    None => {}
                }

                return Ok(glib::ControlFlow::Continue);
            }
            Doing::Asking(reading) => {
                let Ok(heard) = reading.pressed();

                heard
            }
        };

        let Some((button, layer)) = heard else { return Ok(glib::ControlFlow::Continue) };

        let Ok(named) = named(&button);

        match named {
            Some(button) => {
                let Ok(held) = Binding::held(layer, button);
                let Ok((saying, under)) = self.moving(&held);
                let Ok(()) = self.said(&saying, &under);
            }
            None => {
                let Ok(()) = self.said(NO_WORD, "");
            }
        }

        Ok(glib::ControlFlow::Continue)
    }

    fn moving(&self, onto: &Binding) -> Result<(String, String), Never> {
        let Ok(mut jobs) = table::read();
        let Ok(every) = every(&self.parts);
        let Ok(said) = aloud(onto);

        let Ok(moved) = jobs.moving(&every, &self.part.slug, onto);
        let on = format!("{} is {}", self.part.does, said);

        match moved {
            Moved::Already => return Ok((format!("{on} already"), String::new())),
            Moved::Onto | Moved::TookFrom(_) => {}
        }

        match table::write(&jobs) {
            Ok(()) => {}
            Err(fault) => return Ok((fault, String::new())),
        }

        let under = match moved {
            Moved::TookFrom(taken) => {
                let Ok(does) = self.does_of(&taken);
                let Ok(lowered) = lowered(&does);

                format!("{lowered} has no button now")
            }
            Moved::Onto => String::new(),
            Moved::Already => String::new(),
        };
        Ok((on, under))
    }

    fn does_of(&self, slug: &str) -> Result<String, Never> {
        Ok(self.parts
            .iter()
            .find(|part| part.slug == slug)
            .map_or_else(|| slug.to_string(), |part| part.does.clone()))
    }

    fn said(&mut self, saying: &str, under: &str) -> Result<(), Never> {
        self.saying.set_text(saying);
        self.hint.set_text(under);
        let over = match under.is_empty() {
            true => READ_IT,
            false => READ_BOTH,
        };
        self.doing = Doing::Said(Instant::now(), over);

        Ok(())
    }
}

fn named(button: &str) -> Result<Option<String>, Never> {
    Ok(match button_name(button) {
        Ok(_) => Some(button.to_string()),
        Err(_) => None,
    })
}

fn main() -> ExitCode {
    let Some(slug) = std::env::args().nth(1) else {
        eprintln!("usage: console-asking JOB");
        return ExitCode::from(2);
    };

    let Ok(table) = table::table();
    let Ok(front) = table::front();
    let Ok(all) = parts(&table, &front);

    let Some(part) = all.iter().find(|part| part.slug == slug).cloned() else {
        eprintln!("console-asking: this desktop does nothing called {slug}");
        return ExitCode::FAILURE;
    };

    let app = Application::builder().application_id("console.asking").build();
    let held = Rc::new(RefCell::new(Some((part, all))));
    app.connect_activate(move |app| {
        let Some((part, all)) = held.borrow_mut().take() else { return };

        let Ok(()) = raised(app, part, all);
    });
    app.run_with_args::<&str>(&[]);
    ExitCode::SUCCESS
}

fn raised(app: &Application, part: Part, parts: Vec<Part>) -> Result<(), Never> {
    let Ok(asked) = question(&part);

    let saying = Label::new(Some(&asked));
    saying.set_widget_name("sure");
    let hint = Label::new(Some("\u{2026}"));
    hint.set_widget_name("about");

    let card = GtkBox::new(Orientation::Vertical, 8);
    card.set_widget_name("note");
    card.set_halign(Align::Center);
    card.set_valign(Align::Center);
    card.append(&saying);
    card.append(&hint);

    let window = ApplicationWindow::builder().application(app).child(&card).build();
    window.init_layer_shell();
    window.set_namespace(Some(ASKING));
    window.set_layer(Shelf::Overlay);
    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
    window.present();

    let Ok(()) = dressed();
    let mut card = Card {
        doing: Doing::Settling,
        since: Instant::now(),
        part,
        parts,
        complained: Quiet::No,
        saying,
        hint,
    };
    let window = window.clone();
    glib::timeout_add_local(Duration::from_millis(60), move || {
        let Ok(turned) = card.turn();

        match turned {
            glib::ControlFlow::Continue => glib::ControlFlow::Continue,
            glib::ControlFlow::Break => {
                window.close();
                glib::ControlFlow::Break
            }
        }
    });

    Ok(())
}

fn dressed() -> Result<(), Never> {
    let Some(display) = gtk4::gdk::Display::default() else { return Ok(()) };

    let sheet = gtk4::CssProvider::new();
    let Ok(style) = console_panel::style::sheet();

    sheet.load_from_data(&style);
    gtk4::style_context_add_provider_for_display(
        &display,
        &sheet,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    Ok(())
}
