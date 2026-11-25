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
//! Keyboard recording uses the compositor's focused seat and inhibits its
//! shortcuts while the prompt is open. The keycode identifies the physical key,
//! and the seat's modifier state includes layout options such as Caps as Super.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_input_event_devices::{AbsoluteAxisCode, KeyCode};

use console_core_arguments::{Command, Operands, Reason, ValidationError, read};
use console_core_color::palette::{Wearing, WearingError};
use console_core_geometry::{Point, Size};
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Font, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{self as painting, Rendered, Run, painter};
use console_draw_surface::{
  Anchor, Keyboard, Keysym, Margin, Room, ShortcutPress, Surface, SurfaceError, Under as Beneath,
  Wanted,
};

use console_core_iteration::{Endless, Step};
use console_core_never::Never;
use console_input_bindings::bound::{Binding, Input};
use console_input_bindings::keys;
use console_input_bindings::moved::Moved;
use console_input_controller::actions::Table;
use console_input_controller::mode::ASKING;
use console_input_controller::reading::{Trigger, pulled};
use console_input_focus::{self as claim, CONTROLLER, Claim, DeviceKind, Direction, InputEvent};
use console_input_gamepad::vocabulary::{button_name, spoken_for, trigger_spoken};
use console_input_mapping::rows::{Part, aloud, every, parts, question};
use console_input_mapping::table;
use console_input_mapping::update::REGISTER_SPECIAL_WORKSPACE;

const PATIENCE: Duration = Duration::from_secs(12);

const READ_IT: Duration = Duration::from_millis(1400);

const READ_BOTH: Duration = Duration::from_millis(2800);

const UNSAID: (i32, i32) = (0, 1);

const NO_WORD: &str = "Unknown button";

enum Action {
  Settling,
  Prompting(Box<Reading>),
  Keyboard,
  Replied(Instant, Duration),
}

struct Reading {
  claim: Claim,
  span: (i32, i32),
  held: Vec<String>,
}

impl Reading {
  fn open(complained: &mut Silent) -> Result<Option<Self>, Never> {
    let claim = match Claim::of(&CONTROLLER) {
      Ok(claim) => claim,
      Err(refused) => {
        match *complained {
          Silent::Yes => {}
          Silent::No => {
            let Ok(said) = refused.message();

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
        let Ok(said) = refused.message();

        eprintln!("console-asking: {said}; a chord cannot be seen");
        Vec::new()
      }
    };
    let span = told
      .into_iter()
      .find(|(axis, _)| *axis == AbsoluteAxisCode::ABS_Z)
      .map_or(UNSAID, |(_, span)| span);

    Ok(Some(Reading {
      claim,
      span,
      held: Vec::new(),
    }))
  }

  fn pressed(&mut self) -> Result<Option<Binding>, Never> {
    let Ok(heard) = self.claim.receive();

    let mut down: Option<String> = None;

    for (which, event) in heard.events {
      let Ok(said) = claim::translate(which, event.kind, event.code, event.value);

      match said {
        InputEvent::Pressed { button, direction } => {
          let Ok(spoken) = spoken_for(button);

          match direction {
            Direction::Down => down = down.or_else(|| Some(spoken.to_string())),
            Direction::Up => self.held.retain(|held| held != spoken),
          }
        }
        InputEvent::Pulled { trigger, value } => {
          let Ok(()) = self.handle_pull(trigger, value);
        }
        InputEvent::Trigger {
          trigger: _,
          direction: _,
        }
        | InputEvent::Typed {
          code: _,
          direction: _,
        }
        | InputEvent::Unnamed {
          code: _,
          direction: _,
        }
        | InputEvent::None => {}
      }
    }

    let pressed = match down {
      Some(pressed) => pressed,
      None => return Ok(None),
    };
    let held: Vec<&str> = self.held.iter().map(String::as_str).collect();
    let Ok(binding) = Binding::chord(Input::Pad, &held, &pressed);

    self.held.push(pressed);

    Ok(Some(binding))
  }

  fn handle_pull(&mut self, named: &str, value: i32) -> Result<(), Never> {
    let Ok(pulled) = pulled(value, self.span);
    let held = pulled == Trigger::Pressed;
    let Ok(trigger) = trigger_spoken(named);

    match (trigger, held) {
      (Some(word), true) => {
        match ::core::primitive::bool::then_some(self.held.iter().any(|down| down == word), ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => self.held.push(word.to_string()),
        }
      }
      (Some(word), false) => self.held.retain(|down| down != word),
      (None, _) => {}
    }

    Ok(())
  }
}

fn shortcut(press: ShortcutPress) -> Result<Option<Binding>, Never> {
  match press.key {
    Keysym::Super_L
    | Keysym::Super_R
    | Keysym::Control_L
    | Keysym::Control_R
    | Keysym::Shift_L
    | Keysym::Shift_R
    | Keysym::Alt_L
    | Keysym::Alt_R
    | Keysym::Meta_L
    | Keysym::Meta_R
    | Keysym::ISO_Level3_Shift => return Ok(None),
    _ => {}
  }

  let code = match u16::try_from(press.code) {
    Ok(code) => KeyCode(code),
    Err(_outside_evdev) => return Ok(None),
  };
  let Ok(known_key) = keys::key_name(code);
  let name = match known_key {
    Some(name) => name,
    None => return Ok(None),
  };
  let modifiers = u64::from(press.depressed_modifiers);
  let held: Vec<&str> = keys::MODIFIERS
    .iter()
    .filter_map(|(word, _, mask, _, _)| (modifiers & mask != 0).then_some(*word))
    .collect();
  let Ok(binding) = Binding::chord(Input::Keyboard, &held, &name);

  Ok(Some(binding))
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
  fn turn(&mut self, surface: &mut Surface) -> Result<Turned, Never> {
    let waited_too_long =
      self.since.elapsed() > PATIENCE && !matches!(self.effect, Action::Replied(_, _));

    match ::core::primitive::bool::then_some(waited_too_long, ()) {
      ::core::option::Option::Some(()) => return Ok(Turned::Over),
      ::core::option::Option::None => {}
    }

    let heard = match &mut self.effect {
      Action::Replied(when, over) => {
        match ::core::primitive::bool::then_some(when.elapsed() > *over, ()) {
          ::core::option::Option::Some(()) => return Ok(Turned::Over),
          ::core::option::Option::None => return Ok(Turned::Again),
        }
      }
      Action::Settling => {
        let Ok(opened) = Reading::open(&mut self.complained);

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
      Action::Keyboard => {
        let Ok(presses) = surface.shortcut_presses();

        presses.into_iter().find_map(|press| {
          let Ok(binding) = shortcut(press);

          binding
        })
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
        let Ok(()) = self.set_prompt(&saying, Under(&under));
      }
      Known::No => {
        let Ok(()) = self.set_prompt(NO_WORD, Under(""));
      }
    }

    Ok(Turned::Again)
  }

  fn moving(&self, onto: &Binding) -> Result<(String, String), Never> {
    let updated = table::change(|jobs| {
      let Ok(current) = Table::of(jobs);
      let Ok(every) = every(&current);
      let Ok(moved) = jobs.add(&every, &self.part.slug, onto);

      Ok(match moved {
        Moved::Already => table::Change::Keep(moved),
        Moved::Onto | Moved::TookFrom(_) => table::Change::Save(moved),
      })
    });
    let moved = match updated {
      Ok(result) => result,
      Err(fault) => return Ok((fault.to_string(), String::new())),
    };
    let Ok(said) = aloud(onto);
    let on = format!("{} is {}", self.part.does, said);

    match moved {
      Moved::Already => return Ok((format!("{on} already"), String::new())),
      Moved::Onto | Moved::TookFrom(_) => {}
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
        part
          .plays
          .iter()
          .any(|one| one.binding.held.is_empty() && onto.held.contains(&one.binding.pressed))
      })
      .map(|part| part.does.clone())
      .collect();

    Ok(match still.first() {
      Some(first) => format!("\u{201c}{first}\u{201d} also runs"),
      None => String::new(),
    })
  }

  fn does_of(&self, slug: &str) -> Result<String, Never> {
    Ok(
      self
        .parts
        .iter()
        .find(|part| part.slug == slug)
        .map_or_else(|| slug.to_string(), |part| part.does.clone()),
    )
  }

  fn set_prompt(&mut self, saying: &str, under: Under<'_>) -> Result<(), Never> {
    let under_2 = under.0;

    self.saying = saying.to_string();
    self.hint = under_2.to_string();

    let over = match ::core::primitive::bool::then_some(under_2.is_empty(), ()) {
      ::core::option::Option::Some(()) => READ_IT,
      ::core::option::Option::None => READ_BOTH,
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

  Ok(match ::core::primitive::bool::then_some(said, ()) {
    ::core::option::Option::Some(()) => Known::Yes,
    ::core::option::Option::None => Known::No,
  })
}

const JOB: &str = "JOB";

const INPUT: &str = "INPUT";

const NAMED: [&str; 2] = [JOB, INPUT];

const COMMAND: Command = Command {
  name: "console-asking",
  about: "the card that asks which button does JOB, pressed on INPUT, which is pad or keyboard",
  flags: &[REGISTER_SPECIAL_WORKSPACE],
  operands: Operands::Named(&NAMED),
};

enum Request {
  Capture(String, Input),
  RegisterSpecialWorkspace(String),
}

fn request(words: &[String]) -> Result<Request, ValidationError> {
  let read = read(&COMMAND, words);
  let line = read?;
  let Ok(workspace) = line.value(REGISTER_SPECIAL_WORKSPACE);

  match workspace {
    Some(name) => {
      let operands = line.exactly([]);
      let [] = operands?;
      return Ok(Request::RegisterSpecialWorkspace(name.to_owned()));
    }
    None => {}
  }

  let operands = line.exactly(NAMED);
  let [slug, input] = operands?;
  let Ok(on) = Input::from_word(input);

  match on {
    Some(on_2) => Ok(Request::Capture(slug.clone(), on_2)),
    None => {
      let Ok(refusal) = line.refusal(Reason::InvalidValue {
        of: INPUT,
        value: input.clone(),
      });

      Err(refusal)
    }
  }
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let (slug, on) = match request(&words) {
    Ok(Request::Capture(slug, on)) => (slug, on),
    Ok(Request::RegisterSpecialWorkspace(name)) => {
      let Ok(status) = register_special_workspace(&name);
      return status;
    }
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
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

  match raised(part, all) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("console-asking: {fault}");

      ExitCode::FAILURE
    }
  }
}

fn register_special_workspace(name: &str) -> Result<ExitCode, Never> {
  Ok(match table::register_special_workspace(name) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("console-asking: {fault}");
      ExitCode::FAILURE
    }
  })
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
  Ok(Font {
    family: FONT.to_string(),
    height: tall,
  })
}

fn card_background(
  room: Size<u32>,
  [saying, hint]: [Size<u32>; 2],
  wearing: &Wearing,
) -> Result<Panel, Never> {
  let Ok(pad) = fitted::<i32, u32>(PAD);
  let Ok(between) = fitted::<i32, u32>(BETWEEN);
  let width = saying
    .width
    .max(hint.width)
    .saturating_add(pad.saturating_mul(2));
  let height = saying
    .height
    .saturating_add(between)
    .saturating_add(hint.height)
    .saturating_add(pad.saturating_mul(2));
  let Ok(card_wide) = fitted::<u32, i32>(width);
  let Ok(card_tall) = fitted::<u32, i32>(height);
  let Ok(room_wide) = fitted::<u32, i32>(room.width);
  let Ok(room_tall) = fitted::<u32, i32>(room.height);

  Ok(Panel {
    at: Point {
      x: room_wide.saturating_sub(card_wide).saturating_div(2),
      y: room_tall.saturating_sub(card_tall).saturating_div(2),
    },
    size: Size { width, height },
    round: Round(CARD_ROUND),
    fill: wearing.panel,
    edge: Edge::Of {
      wide: CARD_EDGE,
      color: wearing.coral,
    },
  })
}

fn render(card: &Card, room: Size<u32>, wearing: &Wearing) -> Result<Vec<Shape>, Never> {
  let Ok(saying_font) = font(SAYING_TALL);
  let Ok(hint_font) = font(HINT_TALL);
  let wide = WIDEST.min(room.width);

  let saying = painting::measure_text(
    Run {
      said: &card.saying,
      weight: Weight::Bold,
      width: wide,
    },
    &saying_font,
  )?;
  let hint = painting::measure_text(
    Run {
      said: &card.hint,
      weight: Weight::Plain,
      width: wide,
    },
    &hint_font,
  )?;

  let Ok(background) = card_background(room, [saying, hint], wearing);
  let Ok(card_wide) = fitted::<u32, i32>(background.size.width);
  let Point { x: at_x, y: at_y } = background.at;
  let Ok(saying_wide) = fitted::<u32, i32>(saying.width);
  let Ok(hint_wide) = fitted::<u32, i32>(hint.width);
  let Ok(saying_tall) = fitted::<u32, i32>(saying.height);
  let middle = at_x.saturating_add(card_wide.saturating_div(2));

  Ok(vec![
    Shape::Panel(background),
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
        y: at_y
          .saturating_add(PAD)
          .saturating_add(saying_tall)
          .saturating_add(BETWEEN),
      },
      width: hint.width,
      said: card.hint.clone(),
      weight: Weight::Plain,
      font: hint_font,
      ink: wearing.soft,
    }),
  ])
}

enum Unraised {
  Palette(WearingError),
  Surface(SurfaceError),
  Endless(Endless),
}

impl std::fmt::Display for Unraised {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Palette(why) => write!(to, "could not load the palette: {why}"),
      Self::Surface(why) => write!(to, "could not display the prompt: {why}"),
      Self::Endless(why) => write!(to, "the prompt failed to finish: {why}"),
    }
  }
}

fn raised(part: Part, parts: Vec<Part>) -> Result<(), Unraised> {
  let Ok(asked) = question(&part);
  let wearing = Wearing::worn().map_err(Unraised::Palette)?;
  let mut surface = Surface::connect().map_err(Unraised::Surface)?;
  let keyboard = match part.on {
    Input::Keyboard => Keyboard::Takes,
    Input::Pad => Keyboard::Declines,
  };

  surface
    .show(&Wanted {
      namespace: ASKING.to_string(),
      anchor: Anchor::Whole,
      size: Size {
        width: 0,
        height: 0,
      },
      margin: Margin {
        top: 0,
        right: 0,
        bottom: 0,
        left: 0,
      },
      keyboard,
      room: Room::Over,
      under: Beneath::None,
    })
    .map_err(Unraised::Surface)?;

  let mut card = Card {
    effect: Action::Settling,
    since: Instant::now(),
    part,
    parts,
    complained: Silent::No,
    saying: asked,
    hint: "\u{2026}".to_string(),
  };

  match card.part.on {
    Input::Pad => {}
    Input::Keyboard => match surface.capture_shortcuts() {
      Ok(()) => {
        let Ok(waiting) = card.part.waiting();

        card.effect = Action::Keyboard;
        card.hint = waiting.to_string();
      }
      Err(fault) => {
        eprintln!("console-asking: {fault}");
        let Ok(()) = card.set_prompt("Keyboard shortcut capture is unavailable", Under(""));
      }
    },
  }

  let shown = console_core_iteration::iterate(
    (card, Rendered::default(), surface),
    |(card_2, drew, displayed)| framed(card_2, drew, displayed, &wearing),
  )
  .map_err(Unraised::Endless)?;

  shown.map_err(Unraised::Surface)
}

type Frame = Step<(Card, Rendered, Surface), Result<(), SurfaceError>>;

fn framed(
  mut card: Card,
  mut drew: Rendered,
  mut surface: Surface,
  wearing: &Wearing,
) -> Result<Frame, Never> {
  let Ok(turned) = card.turn(&mut surface);

  match turned {
    Turned::Over => return Ok(Step::Halt(Ok(()))),
    Turned::Again => {}
  }

  match draw_current(&card, &mut drew, &mut surface, wearing) {
    Ok(()) => {}
    Err(fault) => return Ok(Step::Halt(Err(fault))),
  }

  Ok(match surface.wait(&[], Some(A_FRAME)) {
    Ok(()) => Step::Again((card, drew, surface)),
    Err(fault) => Step::Halt(Err(fault)),
  })
}

fn draw_current(
  card: &Card,
  drew: &mut Rendered,
  surface: &mut Surface,
  wearing: &Wearing,
) -> Result<(), SurfaceError> {
  let Ok(size) = surface.logical();
  let logical = match size {
    Some(logical) => logical,
    None => return Ok(()),
  };
  let Ok(shapes) = render(card, logical, wearing);
  let Ok(wanted) = drew.wanted(shapes);

  match wanted {
    Some(frame) => {
      let Ok(()) = surface.resize(logical);
      let Ok(paint) = painter(logical, frame, "console-asking");

      surface.draw(paint)
    }
    None => Ok(()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_shortcut_uses_the_physical_key_and_the_seats_held_modifiers() -> Result<(), Never> {
    let recorded = shortcut(ShortcutPress {
      code: u32::from(KeyCode::KEY_I.0),
      key: Keysym::Greek_iota,
      depressed_modifiers: 64 | 4,
    })?;
    let expected = Binding::chord(Input::Keyboard, &["super", "ctrl"], "i")?;

    assert_eq!(recorded, Some(expected));
    Ok(())
  }

  #[test]
  fn caps_lock_remapped_to_super_waits_for_the_rest_of_the_shortcut() -> Result<(), Never> {
    let modifier = shortcut(ShortcutPress {
      code: u32::from(KeyCode::KEY_CAPSLOCK.0),
      key: Keysym::Super_L,
      depressed_modifiers: 0,
    })?;
    let key = shortcut(ShortcutPress {
      code: u32::from(KeyCode::KEY_CAPSLOCK.0),
      key: Keysym::Caps_Lock,
      depressed_modifiers: 0,
    })?;
    let expected = Binding::chord(Input::Keyboard, &[], "capslock")?;

    assert_eq!(modifier, None);
    assert_eq!(key, Some(expected));
    Ok(())
  }
}
