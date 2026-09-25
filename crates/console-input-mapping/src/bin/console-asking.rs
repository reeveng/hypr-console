//! The card that asks which button that was.
//!
//!     console-asking screenshot pad
//!     console-asking screenshot keyboard
//!
//! Raised by the setup screen over the row being moved. While it is up the
//! front of the machine does nothing at all, which is the only state in which
//! "press the button you want" is a question someone can answer: otherwise
//! pressing Legion left to bind it would leave for Game Mode, X would raise the
//! keyboard over the question, and the shoulders would carry the window away.
//!
//! That is `console_input_focus`, and it used to be a profile. The card
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
//! nothing can route is a button nothing could ever be bound to, and the notification
//! after an apply is where that is already said.
//!
//! The name still matters: the compositor lists a layer under the program that
//! drew it, and the controller daemon reads `console-asking` being on the
//! screen as `Mode::Asking` and stands down. The claim is what makes that true
//! rather than agreed.
//!
//! ## DeviceKind input it is asking about
//!
//! The word after the job. A keyboard is claimed the same way and read
//! differently: what arrives is a key rather than a button, so what is held is
//! the modifiers and the press is whatever is not one. The claim matters more
//! here than it does on the pad -- Super and I with no claim would open the
//! settings while someone was trying to say where the settings should be.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_input_event_devices::{AbsoluteAxisCode, EventType, KeyCode};

use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Size};
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Font, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{self as painting, Frame, Run};
use console_draw_surface::{
    Anchor, Keyboard, Margin, Room, Surface, Under as Beneath, Wanted,
};

use console_input_controller::mode::ASKING;
use console_input_controller::reading::{Trigger, pulled};
use console_input_mapping::rows::{Part, aloud, every, parts, question};
use console_input_mapping::table;
use console_input_bindings::bound::{Binding, Input};
use console_input_bindings::keys;
use console_input_bindings::moved::Moved;
use console_input_gamepad::vocabulary::{button_name, spoken_for};
use console_input_focus::{self as claim, CONTROLLER, Claim, InputEvent, Direction, DeviceKind};
use console_core_never::Never;

const PATIENCE: Duration = Duration::from_secs(12);

const READ_IT: Duration = Duration::from_millis(1400);

const READ_BOTH: Duration = Duration::from_millis(2800);

const UNSAID: (i32, i32) = (0, 1);

const NO_WORD: &str = "Unknown button";

const TYPING: [DeviceKind; 1] = [DeviceKind::Typing];

enum Action {
    Settling,
    Prompting(Box<Reading>),
    Replied(Instant, Duration),
}

struct Reading {
    claim: Claim,
    span: (i32, i32),
    on: Input,
    held: Vec<String>,
}

impl Reading {
    fn open(on: Input, complained: &mut Silent) -> Result<Option<Self>, Never> {
        let wanted: &[DeviceKind] = match on {
            Input::Pad => &CONTROLLER,
            Input::Keyboard => &TYPING,
        };

        let claim = match Claim::of(wanted) {
            Ok(claim) => claim,
            Err(refused) => {
                match *complained {
                    Silent::Yes => {}
                    Silent::No => {
                        let Ok(said) = refused.said();

                        eprintln!("console-asking: {said}");
                        *complained = Silent::Yes;
                    }
                }

                return Ok(None);
            }
        };
        let told = match claim.spans(DeviceKind::Pad) {
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

        Ok(Some(Reading { claim, span, on, held: Vec::new() }))
    }

    fn pressed(&mut self) -> Result<Option<Binding>, Never> {
        let Ok(heard) = self.claim.arrived();

        let mut down: Option<String> = None;

        for (which, event) in heard.events {
            let Ok(said) = claim::said(which, event.kind, event.code, event.value);

            match said {
                InputEvent::Pressed { button, direction } => {
                    let Ok(spoken) = spoken_for(button);

                    match direction {
                        Direction::Down => down = down.or_else(|| Some(spoken.to_string())),
                        Direction::Up => self.held.retain(|held| held != spoken),
                    }
                }
                InputEvent::Typed { code, direction } => {
                    let Ok(typed) = self.typed(code, direction);

                    down = down.or(typed);
                }
                InputEvent::Trigger { trigger: _, direction: _ } | InputEvent::Unnamed { code: _, direction: _ } => {}
                InputEvent::None => {
                    let Ok(()) = self.watched(event.kind, event.code, event.value);
                },
            }
        }

        let pressed = match down {
            Some(pressed) => pressed,
            None => return Ok(None),
        };
        let held: Vec<&str> = self.held.iter().map(String::as_str).collect();
        let Ok(binding) = Binding::holding(self.on, &held, &pressed);

        self.held.push(pressed);

        Ok(Some(binding))
    }

    fn typed(&mut self, code: u16, direction: Direction) -> Result<Option<String>, Never> {
        let Ok(modifier) = keys::modifier_of(KeyCode(code));

        match (modifier, direction) {
            (Some(word), Direction::Down) => {
                match self.held.iter().any(|held| held == word) {
                    true => {},
                    false => self.held.push(word.to_string()),
                }

                Ok(None)
            }
            (Some(word), Direction::Up) => {
                self.held.retain(|held| held != word);

                Ok(None)
            }
            (None, Direction::Down) => keys::spoken(KeyCode(code)),
            (None, Direction::Up) => Ok(None),
        }
    }

    fn watched(&mut self, kind: EventType, code: u16, value: i32) -> Result<(), Never> {
        match kind {
            EventType::ABSOLUTE => {}
            _ => return Ok(()),
        }

        let Ok(pulled) = pulled(value, self.span);
        let held = pulled == Trigger::Pressed;

        const LEFT: u16 = AbsoluteAxisCode::ABS_Z.0;
        const RIGHT: u16 = AbsoluteAxisCode::ABS_RZ.0;

        let trigger = match code {
            LEFT => Some("l2"),
            RIGHT => Some("r2"),
            _ => None,
        };

        match (trigger, held) {
            (Some(word), true) => {
                match self.held.iter().any(|down| down == word) {
                    true => {},
                    false => self.held.push(word.to_string()),
                }
            }
            (Some(word), false) => self.held.retain(|down| down != word),
            (None, _) => {}
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Silent {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Under<'a>(&'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turned {
    Again,
    Over,
}

struct Card {
    effect: Action,
    since: Instant,
    part: Part,
    parts: Vec<Part>,
    complained: Silent,
    saying: String,
    hint: String,
}

impl Card {
    fn turn(&mut self) -> Result<Turned, Never> {
        let waited_too_long =
            self.since.elapsed() > PATIENCE && !matches!(self.effect, Action::Replied(_, _));

        match waited_too_long {
            true => return Ok(Turned::Over),
            false => {}
        }

        let heard = match &mut self.effect {
            Action::Replied(when, over) => match when.elapsed() > *over {
                true => return Ok(Turned::Over),
                false => return Ok(Turned::Again),
            },
            Action::Settling => {
                let Ok(opened) = Reading::open(self.part.on, &mut self.complained);

                match opened {
                    Some(reading) => {
                        let Ok(waiting) = self.part.waiting();

                        self.hint = waiting.to_string();
                        self.effect = Action::Prompting(Box::new(reading));
                    }
                    None => {}
                }

                return Ok(Turned::Again);
            }
            Action::Prompting(reading) => {
                let Ok(heard) = reading.pressed();

                heard
            }
        };

        let binding = match heard {
            Some(binding) => binding,
            None => return Ok(Turned::Again),
        };

        let Ok(known) = known(&binding);

        match known {
            Known::Yes => {
                let Ok((saying, under)) = self.moving(&binding);
                let Ok(()) = self.said(&saying, Under(&under));
            }
            Known::No => {
                let Ok(()) = self.said(NO_WORD, Under(""));
            }
        }

        Ok(Turned::Again)
    }

    fn moving(&self, onto: &Binding) -> Result<(String, String), Never> {
        let Ok(mut jobs) = table::read();
        let Ok(table) = table::table();
        let Ok(every) = every(&table);
        let Ok(said) = aloud(onto);

        let Ok(moved) = jobs.adding(&every, &self.part.slug, onto);
        let on = format!("{} is {}", self.part.does, said);

        match moved {
            Moved::Already => return Ok((format!("{on} already"), String::new())),
            Moved::Onto | Moved::TookFrom(_) => {}
        }

        match table::write(&jobs) {
            Ok(()) => {}
            Err(fault) => return Ok((fault.to_string(), String::new())),
        }

        let under = match moved {
            Moved::TookFrom(taken) => {
                let Ok(does) = self.does_of(&taken);
                let Ok(word) = self.part.on.word();

                format!("\u{201c}{does}\u{201d} now has no {word}")
            }
            Moved::Onto => {
                let Ok(over) = self.over(onto);

                over
            }
            Moved::Already => String::new(),
        };

        Ok((on, under))
    }

    fn over(&self, onto: &Binding) -> Result<String, Never> {
        #[cfg_attr(
            dylint_lib = "explicit028_no_search_in_a_loop",
            allow(
                explicit028_no_search_in_a_loop,
                reason = "the parts of one controller against the words held down on one binding, which is what fits on a card someone is looking at"
            )
        )]
        let still: Vec<String> = self
            .parts
            .iter()
            .filter(|part| part.slug != self.part.slug)
            .filter(|part| {
                part.plays.iter().any(|one| {
                    one.binding.held.is_empty()
                        && onto.held.contains(&one.binding.pressed)
                })
            })
            .map(|part| part.does.clone())
            .collect();

        Ok(match still.first() {
            Some(first) => format!("\u{201c}{first}\u{201d} also runs"),
            None => String::new(),
        })
    }

    fn does_of(&self, slug: &str) -> Result<String, Never> {
        Ok(self.parts
            .iter()
            .find(|part| part.slug == slug)
            .map_or_else(|| slug.to_string(), |part| part.does.clone()))
    }

    fn said(&mut self, saying: &str, under: Under<'_>) -> Result<(), Never> {
        let under = under.0;

        self.saying = saying.to_string();
        self.hint = under.to_string();

        let over = match under.is_empty() {
            true => READ_IT,
            false => READ_BOTH,
        };

        self.effect = Action::Replied(Instant::now(), over);

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Known {
    Yes,
    No,
}

fn known(binding: &Binding) -> Result<Known, Never> {
    let said = match binding.on {
        Input::Pad => match button_name(&binding.pressed) {
            Ok(_named) => true,
            Err(_unnamed) => false,
        },
        Input::Keyboard => {
            let Ok(code) = keys::code(&binding.pressed);

            code.is_some()
        }
    };

    Ok(match said {
        true => Known::Yes,
        false => Known::No,
    })
}

fn main() -> ExitCode {
    let mut asked = std::env::args().skip(1);

    let slug = match asked.next() {
        Some(slug) => slug,
        None => {
            eprintln!("usage: console-asking JOB [pad|keyboard]");
            return ExitCode::from(2);
        }
    };

    let on = match asked.next().as_deref() {
        Some("keyboard") => Input::Keyboard,
        Some(_) | None => Input::Pad,
    };

    let Ok(table) = table::table();
    let Ok(front) = table::front();
    let Ok(all) = parts(&table, &front, on);

    let part = match all.iter().find(|part| part.slug == slug).cloned() {
        Some(part) => part,
        None => {
            eprintln!("console-asking: this desktop does nothing called {slug}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(()) = raised(part, all);

    ExitCode::SUCCESS
}

const FONT: &str = "Noto Sans";
const SAYING_TALL: u32 = 22;
const HINT_TALL: u32 = 16;
const CARD_ROUND: u32 = 16;
const CARD_EDGE: u32 = 2;
const PAD: i32 = 24;
const BETWEEN: i32 = 8;
const WIDEST: u32 = 600;
const A_FRAME: Duration = Duration::from_millis(60);

fn font(tall: u32) -> Result<Font, Never> {
    Ok(Font { family: FONT.to_string(), height: tall })
}

fn drawn(card: &Card, room: Size<u32>, wearing: &Wearing) -> Result<Vec<Shape>, Never> {
    let Ok(saying_font) = font(SAYING_TALL);
    let Ok(hint_font) = font(HINT_TALL);
    let wide = WIDEST.min(room.width);

    let saying = painting::measured(
        Run { said: &card.saying, weight: Weight::Bold, width: wide },
        &saying_font,
    )?;
    let hint = painting::measured(
        Run { said: &card.hint, weight: Weight::Plain, width: wide },
        &hint_font,
    )?;

    let Ok(pad) = fitted::<i32, u32>(PAD);
    let Ok(between) = fitted::<i32, u32>(BETWEEN);
    let widest = saying.width.max(hint.width);
    let card_wide = widest.saturating_add(pad.saturating_mul(2));
    let card_tall = saying
        .height
        .saturating_add(between)
        .saturating_add(hint.height)
        .saturating_add(pad.saturating_mul(2));

    let Ok(card_wide_i) = fitted::<u32, i32>(card_wide);
    let Ok(card_tall_i) = fitted::<u32, i32>(card_tall);
    let Ok(room_wide) = fitted::<u32, i32>(room.width);
    let Ok(room_tall) = fitted::<u32, i32>(room.height);
    let at_x = room_wide.saturating_sub(card_wide_i).saturating_div(2);
    let at_y = room_tall.saturating_sub(card_tall_i).saturating_div(2);

    let Ok(saying_wide) = fitted::<u32, i32>(saying.width);
    let Ok(hint_wide) = fitted::<u32, i32>(hint.width);
    let Ok(saying_tall) = fitted::<u32, i32>(saying.height);
    let middle = at_x.saturating_add(card_wide_i.saturating_div(2));

    Ok(vec![
        Shape::Panel(Panel {
            at: Point { x: at_x, y: at_y },
            size: Size { width: card_wide, height: card_tall },
            round: Round(CARD_ROUND),
            fill: wearing.panel,
            edge: Edge::Of { wide: CARD_EDGE, color: wearing.coral },
        }),
        Shape::Text(Text {
            at: Point {
                x: middle.saturating_sub(saying_wide.saturating_div(2)),
                y: at_y.saturating_add(PAD),
            },
            width: saying.width,
            said: card.saying.clone(),
            weight: Weight::Bold,
            font: saying_font,
            ink: wearing.text,
        }),
        Shape::Text(Text {
            at: Point {
                x: middle.saturating_sub(hint_wide.saturating_div(2)),
                y: at_y.saturating_add(PAD).saturating_add(saying_tall).saturating_add(BETWEEN),
            },
            width: hint.width,
            said: card.hint.clone(),
            weight: Weight::Plain,
            font: hint_font,
            ink: wearing.soft,
        }),
    ])
}

fn raised(part: Part, parts: Vec<Part>) -> Result<(), Never> {
    let Ok(asked) = question(&part);

    let wearing = match Wearing::worn() {
        Ok(wearing) => wearing,
        Err(why) => {
            eprintln!("console-asking: no palette: {why}");

            return Ok(());
        }
    };

    let mut surface = match Surface::connect() {
        Ok(surface) => surface,
        Err(fault) => {
            eprintln!("console-asking: no surface: {fault}");

            return Ok(());
        }
    };

    match surface.show(&Wanted {
        namespace: ASKING.to_string(),
        anchor: Anchor::Whole,
        size: Size { width: 0, height: 0 },
        margin: Margin { top: 0, right: 0, bottom: 0, left: 0 },
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Beneath::None,
    }) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-asking: no surface to draw on: {fault}");

            return Ok(());
        }
    }

    let mut card = Card {
        effect: Action::Settling,
        since: Instant::now(),
        part,
        parts,
        complained: Silent::No,
        saying: asked,
        hint: "\u{2026}".to_string(),
    };

    let mut drew: Option<Vec<Shape>> = None;

    loop {
        let Ok(turned) = card.turn();

        match turned {
            Turned::Over => return Ok(()),
            Turned::Again => {},
        }

        let logical = match surface.logical() {
            Ok(Some(logical)) => logical,
            Ok(None) => {
                let _ = surface.wait(&[], Some(A_FRAME));

                continue;
            }
            Err(_the_compositor_has_gone) => return Ok(()),
        };

        let Ok(shapes) = drawn(&card, logical, &wearing);

        match drew.as_ref() == Some(&shapes) {
            true => {},
            false => {
                let _ = surface.resize(logical);

                let _ = surface.draw(|pixels, device, _scale| {
                    let frame = Frame { device, points: logical };
                    let _ = painting::onto(pixels, frame, &shapes);

                    Ok(())
                });

                drew = Some(shapes);
            }
        }

        let _ = surface.wait(&[], Some(A_FRAME));
    }
}
