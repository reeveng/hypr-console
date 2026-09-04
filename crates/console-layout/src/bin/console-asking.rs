//! The card that asks which button that was.
//!
//!     console-asking screenshot
//!
//! Raised by the setup screen over the row being moved. What makes it work is
//! its own name: the compositor lists a layer under the program that drew it,
//! and the controller daemon reads `console-asking` being on the screen as
//! `Mode::Asking` -- which loads the profile that sends every button on this
//! device to a key nothing is listening for. So while this card is up the
//! front of the machine does nothing at all, which is the only state in which
//! "press the button you want" is a question somebody can answer: otherwise
//! pressing Legion left to bind it would leave for Game Mode.
//!
//! The press is read off the keyboard InputPlumber publishes, the same way the
//! controller daemon reads its own, and `console_pad::asking` says which
//! button each key stands for. The triggers are read off the pad beside it,
//! because a job can be put on a chord and a chord is a trigger held at the
//! moment of the press: they are the one thing the asking profile passes
//! through, and a card that could not see them could only ever bind a button
//! on its own.

use std::cell::RefCell;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use evdev::{AbsoluteAxisCode, Device, EventType};
use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, Box as GtkBox, Label, Orientation, glib};
use gtk4_layer_shell::{Layer as Shelf, LayerShell};

use console_controller::finding::{Says, gamepad, says};
use console_controller::mode::ASKING;
use console_controller::reading::CARRY_HELD;
use console_layout::rows::{Part, WAITING, aloud, every, lowered, parts, question};
use console_layout::table;
use console_pad::asking::Asking;
use console_pad::front::{one_said, wearing};
use console_pad::jobs::{ALONE, Binding, Held, Layer, Moved};
use console_pad::vocabulary::{button_name, button_of, spoken_for};

/// How long the question waits before it gives up and changes nothing.
///
/// Long enough to find a button on a device you have never held, short enough
/// that a card nobody meant to raise takes itself away rather than leaving the
/// machine inert until somebody works out why.
const PATIENCE: Duration = Duration::from_secs(12);

/// How long an answer stays on the screen before the card goes.
const READ_IT: Duration = Duration::from_millis(1400);

/// The same, where the answer has a second line under it.
///
/// Taking a button off another job is the one thing this card does that
/// somebody did not ask for by name, so the line saying which job lost it has
/// to be readable at the speed a person reads a thing they were not expecting.
const READ_BOTH: Duration = Duration::from_millis(2800);

/// What a trigger's range is taken to be when the pad will not say.
///
/// Nought and one, so any pull at all counts as held. Guessing a wide range on
/// a pad that reports a narrow one would be a card where holding the trigger
/// did nothing, and a chord nobody can enter is worse than one entered a
/// little too easily.
const UNSAID: (i32, i32) = (0, 1);

/// What is said to a press this desktop has no word for.
///
/// A device can send a button nothing here names, and the file is written in
/// the names -- so it is a button nothing could be bound to, and saying so is
/// the only honest answer. The two on this handheld nobody had held are in the
/// vocabulary for exactly this reason.
const NO_WORD: &str = "this desktop has no word for that button";

/// What the card is doing.
enum Doing {
    /// Waiting for the pad to be wearing the profile that makes it inert.
    Settling,
    /// Reading the devices, with the question up.
    ///
    /// Boxed because an evdev device is most of a kilobyte and the other two
    /// states are a moment in time.
    Asking(Box<Reading>),
    /// Said something, and going after however long that takes to read.
    Said(Instant, Duration),
}

/// The two devices a chord is read off, and what the triggers are doing.
struct Reading {
    keys: Device,
    /// The pad, for how far each trigger is pulled. Nothing else on it is
    /// read: every button is inert while this card is up, and the triggers are
    /// the one thing the asking profile lets through.
    ///
    /// Nothing at all where the pad could not be found, which is a card that
    /// still binds a button pressed on its own rather than one that does not
    /// open.
    pad: Option<Device>,
    /// The range the pad reports a trigger over.
    span: (i32, i32),
    /// What is being held, as of the last look.
    layer: Layer,
}

impl Reading {
    /// Both devices, found after the profile has changed.
    fn open() -> Option<Self> {
        let keys = keyboard()?;
        let (pad, span) = match pad() {
            Some((pad, span)) => (Some(pad), span),
            None => (None, UNSAID),
        };
        Some(Reading { keys, pad, span, layer: ALONE })
    }

    /// How far each trigger is pulled now.
    ///
    /// Looked at before the keyboard every turn, because a chord is a trigger
    /// held and then a button pressed: the pull arrives first, and reading the
    /// press first would read it as the button on its own.
    fn watched(&mut self) {
        let Some(pad) = &mut self.pad else { return };

        let Ok(arrived) = pad.fetch_events() else { return };

        for event in arrived {
            if event.event_type() != EventType::ABSOLUTE {
                continue;
            }

            let held = pulled(event.value(), self.span) == Held::Down;

            if event.code() == AbsoluteAxisCode::ABS_Z.0 {
                self.layer.l2 = held;
            } else if event.code() == AbsoluteAxisCode::ABS_RZ.0 {
                self.layer.r2 = held;
            }
        }
    }

    /// The code of a key that has just gone down, if one has.
    fn pressed(&mut self) -> Option<u16> {
        let arrived = match self.keys.fetch_events() {
            Ok(arrived) => arrived,
            // Nothing to read yet is what a device set nonblocking says almost
            // every time this is asked, and it is not a fault.
            Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => return None,

            Err(fault) => {
                eprintln!("reading the keyboard for a press: {fault}");
                return None;
            }
        };

        arrived
            .filter(|event| event.event_type() == EventType::KEY && event.value() == 1)
            .map(|event| event.code())
            .next()
    }
}

/// Whether a trigger is pulled far enough to be a layer.
///
/// Past half of what the pad says its range is, out of the same constant the
/// daemon reads it with: a chord bound by pulling a trigger this far has to be
/// a chord that plays when it is pulled that far again.
fn pulled(value: i32, (low, high): (i32, i32)) -> Held {
    let span = f64::from((high - low).max(1));

    match f64::from(value - low) / span > CARRY_HELD {
        true => Held::Down,
        false => Held::Up,
    }
}

struct Card {
    doing: Doing,
    since: Instant,
    part: Part,
    /// Every job on the screen, which is two things at once: the names the
    /// file is keyed by, and what each of them does. The second is only ever
    /// for saying out loud which job has just lost its button, and a card that
    /// could not say that would be a card that took one away in silence.
    parts: Vec<Part>,
    asking: Asking,
    saying: Label,
    hint: Label,
}

impl Card {
    /// One turn of the loop: whatever this card is doing, do a little of it.
    fn turn(&mut self) -> glib::ControlFlow {
        if self.since.elapsed() > PATIENCE && !matches!(self.doing, Doing::Said(_, _)) {
            return glib::ControlFlow::Break;
        }

        let heard = match &mut self.doing {
            Doing::Said(when, over) if when.elapsed() > *over => return glib::ControlFlow::Break,
            Doing::Said(_, _) => return glib::ControlFlow::Continue,
            Doing::Settling => {
                if inert() == Inert::Yes && let Some(reading) = Reading::open() {
                    self.hint.set_text(WAITING);
                    self.doing = Doing::Asking(Box::new(reading));
                }

                return glib::ControlFlow::Continue;
            }
            Doing::Asking(reading) => {
                reading.watched();
                reading.pressed().map(|code| (code, reading.layer))
            }
        };

        let Some((code, layer)) = heard else { return glib::ControlFlow::Continue };

        let Some(capability) = self.asking.pressed_code(code).map(str::to_string) else {
            return glib::ControlFlow::Continue;
        };

        match named(&capability) {
            Some(button) => {
                let (saying, under) = self.moving(&Binding::held(layer, button));
                self.said(&saying, &under);
            }
            None => self.said(NO_WORD, ""),
        }

        glib::ControlFlow::Continue
    }

    /// Move the job onto what was just pressed.
    ///
    /// Two lines back: what this row is on now, and, where the button belonged
    /// to something else, which job has been left without one. Said in the
    /// words on the row and on the button rather than in the profile's own --
    /// this card used to answer a press of Y with "West is already West",
    /// which names neither the button nor the job and reads as a machine that
    /// has broken.
    ///
    /// Writing the file is the whole of it. It used to write the file and then
    /// ask root to build the profiles again, because a button's meaning lived
    /// in a profile; it does not any more, and the daemon reads this file
    /// itself.
    fn moving(&self, onto: &Binding) -> (String, String) {
        let mut jobs = table::read();
        let moved = jobs.moving(&every(&self.parts), &self.part.slug, onto);
        let on = format!("{} is {}", self.part.does, aloud(onto));

        if moved == Moved::Already {
            return (format!("{on} already"), String::new());
        }

        if let Err(fault) = table::write(&jobs) {
            return (fault, String::new());
        }

        let under = match moved {
            Moved::TookFrom(taken) => {
                format!("{} has no button now", lowered(&self.does_of(&taken)))
            }
            _ => String::new(),
        };
        (on, under)
    }

    /// What a job does, in the words its row says it in.
    fn does_of(&self, slug: &str) -> String {
        self.parts
            .iter()
            .find(|part| part.slug == slug)
            .map_or_else(|| slug.to_string(), |part| part.does.clone())
    }

    fn said(&mut self, saying: &str, under: &str) {
        self.saying.set_text(saying);
        self.hint.set_text(under);
        let over = match under.is_empty() {
            true => READ_IT,
            false => READ_BOTH,
        };
        self.doing = Doing::Said(Instant::now(), over);
    }
}

/// Which button a press was, in the words on the machine.
///
/// A button this repository has no word for cannot be bound: the file is
/// written in those words and the daemon reads presses back through them, so a
/// name only InputPlumber knows would be a binding nothing could ever match.
fn named(capability: &str) -> Option<String> {
    let button = spoken_for(button_of(capability)?);

    match button_name(button) {
        Ok(_) => Some(button.to_string()),
        // Not a fault to report: a button this repository has no word for is a
        // button the card should offer nobody, which is what None says here.
        Err(_) => None,
    }
}

/// Whether the pad is wearing the profile that makes every button inert.
///
/// Asked of InputPlumber rather than assumed, because the daemon loads it a
/// poll after this card appears, and a question answered in that gap would be
/// answered by a button doing what it used to do.
fn inert() -> Inert {
    match one_said(&table::said(&wearing())).is_some_and(|path| path.ends_with("asking.yaml")) {
        true => Inert::Yes,
        false => Inert::No,
    }
}

/// Whether the profile in front is the one where every button does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inert {
    /// It is, so a press can be read as an answer rather than as a job.
    Yes,
    /// It is not, and a press would still be doing whatever it usually does.
    No,
}

/// The keyboard InputPlumber publishes, as it is now.
///
/// Found after the profile has changed and not before: a profile switch
/// destroys the published devices and builds them again, so the node that was
/// there when this card opened is a node that no longer exists.
///
/// By name, and not by the rule the daemon finds its own keyboard with. That
/// rule asks for F13, which is a key the desktop's profile lends to a paddle
/// and this one does not lend to anything.
fn keyboard() -> Option<Device> {
    let mut found: Vec<Device> = evdev::enumerate()
        .map(|(_, device)| device)
        .filter(|device| device.name().unwrap_or_default().contains("InputPlumber Keyboard"))
        .collect();
    let device = found.pop()?;

    match device.set_nonblocking(true) {
        Ok(()) => Some(device),

        Err(fault) => {
            eprintln!("the InputPlumber keyboard will not read without blocking: {fault}");
            None
        }
    }
}

/// The pad InputPlumber publishes, and the range it reports a trigger over.
///
/// The same rule the daemon finds it by: the one with a right stick that
/// nobody is holding. The physical controller has a place it is plugged in and
/// this one does not, which unlike a name is a thing the two cannot share.
fn pad() -> Option<(Device, (i32, i32))> {
    let seen: Vec<(String, Device)> = evdev::enumerate()
        .map(|(path, device)| (path.display().to_string(), device))
        .collect();
    let said: Vec<Says> = seen.iter().map(|(path, device)| says(path, device)).collect();
    let wanted = gamepad(&said)?.path.clone();
    let (_, device) = seen.into_iter().find(|(path, _)| *path == wanted)?;

    if let Err(fault) = device.set_nonblocking(true) {
        eprintln!("the pad will not read without blocking: {fault}");
        return None;
    }

    // Both triggers against the left one's range, the way the daemon reads
    // them: a pad reporting two different ranges for its two triggers would be
    // a pad worth asking about rather than one worth guessing at.
    let span = match device.get_absinfo() {
        Ok(mut every) => every
            .find(|(axis, _)| *axis == AbsoluteAxisCode::ABS_Z)
            .map_or(UNSAID, |(_, info)| (info.minimum(), info.maximum())),

        Err(fault) => {
            eprintln!("asking the pad what range it reports a trigger over: {fault}");
            UNSAID
        }
    };
    Some((device, span))
}

fn main() -> ExitCode {
    let Some(slug) = std::env::args().nth(1) else {
        eprintln!("usage: console-asking JOB");
        return ExitCode::from(2);
    };

    let front = table::front();
    let all = parts(&table::table(), &front);

    let Some(part) = all.iter().find(|part| part.slug == slug).cloned() else {
        eprintln!("console-asking: this desktop does nothing called {slug}");
        return ExitCode::FAILURE;
    };

    let Some(capabilities) = front.capabilities.clone() else {
        eprintln!("console-asking: InputPlumber did not say what this device sends");
        return ExitCode::FAILURE;
    };

    let app = Application::builder().application_id("console.asking").build();
    let held = Rc::new(RefCell::new(Some((part, Asking::of(&capabilities), all))));
    app.connect_activate(move |app| {
        let Some((part, asking, all)) = held.borrow_mut().take() else { return };

        raised(app, part, asking, all);
    });
    app.run_with_args::<&str>(&[]);
    ExitCode::SUCCESS
}

/// The card itself: what is being asked, and a line under it.
fn raised(app: &Application, part: Part, asking: Asking, parts: Vec<Part>) {
    let saying = Label::new(Some(&question(&part)));
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
    // The whole of what tells the controller daemon what this is.
    window.set_namespace(Some(ASKING));
    window.set_layer(Shelf::Overlay);
    // Not anchored, so the compositor centres it, and no keyboard interest at
    // all: the press this is waiting for is read off the device itself, and a
    // card that took the focus would take it from the panel underneath for the
    // sake of keys it never reads.
    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
    window.present();

    dressed();
    let mut card = Card {
        doing: Doing::Settling,
        since: Instant::now(),
        part,
        parts,
        asking,
        saying,
        hint,
    };
    let window = window.clone();
    glib::timeout_add_local(Duration::from_millis(60), move || match card.turn() {
        glib::ControlFlow::Continue => glib::ControlFlow::Continue,
        glib::ControlFlow::Break => {
            window.close();
            glib::ControlFlow::Break
        }
    });
}

/// The one stylesheet every surface on this desktop is drawn in.
fn dressed() {
    let Some(display) = gtk4::gdk::Display::default() else { return };

    let sheet = gtk4::CssProvider::new();
    sheet.load_from_data(&console_panel::style::sheet());
    gtk4::style_context_add_provider_for_display(
        &display,
        &sheet,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
