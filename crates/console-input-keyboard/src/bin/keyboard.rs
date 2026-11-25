//! The on-screen keyboard.
//!
//! Started once with the session and kept for it. Most of that time there is
//! nothing of it on the screen: `--hidden` starts it away, and the controller
//! shows and hides it with a word down a socket, because a keyboard that was started and
//! stopped would pay for a compositor connection, ten composed keymaps and a
//! font every time someone wanted to type a word.
//!
//!     console-keyboard --landscape-layers landscape,thai,landscapespecial
//!
//! The unit runs this, and `keyboard-toggle` sends the word. It reads the
//! palette on the way in rather than being handed it: there was a second
//! program that did the reading and exec'd this one, from when this one was C
//! and could not be given a Rust crate to ask. Both ends are Rust in one crate
//! now, and the indirection cost more than it carried -- a unit that names a
//! program that starts the program that matters is a unit `named_by` cannot see
//! through, so a new keyboard could be installed and the old one go on running.
//!
//! The palette it reads is the one its own tree spends, not the one at `/`.
//! There is a second tree: the nested desktop stages the whole of this desktop
//! under a directory of its own and runs it there, and a keyboard that joined
//! the palette onto `/` found the machine's own `/usr/local`, read nothing,
//! kept the colors in `configuration::Scheme::default` and drew a keyboard in green
//! and red that no check could believe. `console_core_color::palette::beside` is
//! where that is decided, and on the device the two answers are one file.
//!
//! The loop is a `State` and its methods rather than one `while` body: what a
//! turn touches is a dozen things that change together, and written as locals
//! they nested five deep. `once_around` is the turn -- draw what changed, wait,
//! act on what woke it -- and the wait is the only place this program blocks.
//! An error out of that wait ends the run rather than being printed and gone
//! round again: what arrives there is the connection dying, and the next turn
//! would not block, so the keyboard would spin and say so forever.
//!
//! Settings and `language-switch` wake this loop when the layout changes.
//! The chosen alphabet and the list of available layers are read together on
//! that wake, before handling any other input.
//!
//! A key held down -- under a thumb, or selected with A, B or the menu button
//! held -- types again for as long as it is held, and `key_repeat` says when.
//! The wait carries the next repeat as its timeout, so a hold wakes the loop
//! and nothing else has to. A turn reads the presses before it asks for the
//! repeat, so a thumb that came up in the same wake a repeat fell due in stops
//! it rather than typing one more.

use console_core_arguments::{Flag, Operands, Presence, Takes};
use console_core_geometry::{Point, Size};
use console_core_highlight::Activation;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixDatagram;
use std::process::ExitCode;

use console_core_color::palette::spent;
use console_input_alphabets::{self as alphabets, Orientation as Holding};
use console_input_focus::{self as claim, CONTROLLER, Claim, Received, Spans};
use console_input_keyboard::configuration::{self, Configuration};
use console_input_keyboard::drawing::{BufferScale, Stride, Surface};
use console_input_keyboard::gamepad::{self, Change, Element, KeyboardCommand};
use console_input_keyboard::key_repeat::{KeyRepeat, Press, RepeatMode, TYPING};
use console_input_keyboard::layout::{
  Drops, Kind, LayoutKind, find_layout, key, layout_keys, modifiers, of, toward, under,
};
use console_input_keyboard::paint;
use console_input_keyboard::remote::{self, Command};
use console_input_keyboard::surface::{
  Closed, Drawn, Keep, PointerEvent, Screen, Showing, SurfaceError,
};
use console_input_keyboard::typing::{self, After, Typist};
use console_response_times::{Wait, Waiting};
use console_screen::Shape;
use std::time::Instant;

#[path = "keyboard/preparing.rs"]
mod preparing;

const NOWHERE: (i32, i32) = (0, 0);

const REMOTE_COMMANDS_PER_TURN: u16 = 64;

const LIST_LAYERS: Flag = Flag {
  spelling: "--list-layers",
  takes: Takes::None,
  about: "the layers this keyboard can show, one to a line",
};

const LANDSCAPE_LAYERS: Flag = Flag {
  spelling: "--landscape-layers",
  takes: Takes::Value("LAYERS"),
  about: "the layers to walk while the screen is wider than it is tall, separated by commas",
};

const ON_THE_SCREEN: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-keyboard",
  about: "the keyboard on the screen, started away and shown by keyboard-toggle",
  flags: &[LIST_LAYERS, LANDSCAPE_LAYERS],
  operands: Operands::None,
};

fn display() -> Result<Option<console_screen::Screen>, Never> {
  Ok(match console_screen::here() {
    Ok(asked) => asked,
    Err(fault) => {
      eprintln!("console-keyboard: which way up the screen stands: {fault}");

      None
    }
  })
}

fn shape_of(screen: Option<console_screen::Screen>) -> Result<Shape, Never> {
  match screen {
    Some(screen_2) => screen_2.shape(),
    None => Ok(Shape::Wider),
  }
}

fn main() -> ExitCode {
  let Ok(mut waiting) = Waiting::on(Wait {
    who: "keyboard",
    what: "starting",
  });
  let words: Vec<String> = std::env::args().skip(1).collect();
  let line = match console_core_arguments::read(&ON_THE_SCREEN, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let Ok(listing) = line.presence(LIST_LAYERS);

  match listing {
    Presence::Present => {
      for which in LayoutKind::ALL {
        let Ok(of) = of(which);

        println!("{}", of.name);
      }

      return ExitCode::SUCCESS;
    }
    Presence::Absent => {}
  }

  let Ok(landscape) = line.value(LANDSCAPE_LAYERS);
  let passed: Vec<String> = landscape
    .into_iter()
    .flat_map(|layers| [LANDSCAPE_LAYERS.spelling.to_string(), layers.to_string()])
    .collect();
  let Ok(flags) = dressed(&passed);

  let Ok(()) = waiting.mark("palette");

  let configuration = match configuration::from_environment(&flags) {
    Ok(configuration) => configuration,
    Err(why) => {
      eprintln!("console-keyboard: {why:?}");
      return ExitCode::FAILURE;
    }
  };

  let Ok(()) = waiting.mark("asked");

  match run(&configuration, waiting) {
    Ok(()) => ExitCode::SUCCESS,
    Err(why) => {
      eprintln!("console-keyboard: {why}");
      ExitCode::FAILURE
    }
  }
}

fn dressed(passed: &[String]) -> Result<Vec<String>, Never> {
  let palette = match spent() {
    Ok(palette) => palette,
    Err(why) => {
      eprintln!("console-keyboard: {why}");
      std::collections::BTreeMap::new()
    }
  };
  let Ok(missing) = console_input_keyboard::palette::missing_colors(&palette);

  match ::core::primitive::bool::then_some(missing.is_empty(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      eprintln!(
        "console-keyboard: the palette has no {}, so the keyboard keeps its own color \
                 for those",
        missing.join(", ")
      );
    }
  }

  console_input_keyboard::palette::arguments(&palette, passed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Input {
  Controller(Element),
  Touch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Repeat {
  Command(KeyboardCommand),
  Key(u32),
}

struct State<'a> {
  screen: Screen,
  told: UnixDatagram,
  configuration: &'a Configuration,
  height: u32,
  typist: Typist,
  repeat: KeyRepeat<Input, Repeat>,
  pressed: Option<u32>,
  shown: Option<KeyboardState>,
  kept: Option<KeyboardState>,
  preparing: preparing::Opening,
  showing: Option<Waiting>,
  reading: Option<Reading>,
  complained: Logged,
  worn: LayoutKind,
  selected: Option<u32>,
  standing: Shape,
}

impl State<'_> {
  fn while_open(&mut self) -> Result<(), Unopened> {
    let ended = console_core_iteration::iterate(self, |state| {
      Ok(
        match ::core::primitive::bool::then_some(state.screen.closed() == Ok(Closed::No), ()) {
          ::core::option::Option::Some(()) => match state.once_around() {
            Ok(()) => Step::Again(state),
            Err(fault) => Step::Halt(Err(fault)),
          },
          ::core::option::Option::None => Step::Halt(Ok(())),
        },
      )
    });

    match ended {
      Ok(ended_2) => ended_2,
      Err(_endless) => Ok(()),
    }
  }

  fn once_around(&mut self) -> Result<(), Unopened> {
    let Ok(showing_now) = self.screen.visibility();

    self.drawing(showing_now)?;
    let Ok(configured) = self.screen.size();

    match (showing_now, configured) {
      (Showing::Yes, Some(_)) | (Showing::No, _) => {
        let Ok(()) = sync_reading(showing_now, &mut self.reading, &mut self.complained);
      }
      (Showing::Yes, None) => {}
    }

    let events = self.waiting_on()?;
    let now = Instant::now();

    self.handle_commands(events)?;

    let Ok(()) = self.wants(now);
    let Ok(()) = self.pointer_events(now);
    let Ok(()) = self.repeats(now);

    self.screen.send_keys()?;

    match (self.typist.last_alphabet == self.worn).then_some(()) {
      Some(()) => {}
      None => {
        self.worn = self.typist.last_alphabet;
        let Ok(()) = keeps_its_own(self.worn);
      }
    }

    Ok(())
  }

  fn reload_layouts(&mut self) -> Result<(), Never> {
    let Ok((asked, _height)) = orientation(self.configuration, self.standing);
    let Ok(walk) = layout_walk(&asked);
    let Ok(wanted) = left_on(self.standing);
    let Ok(()) = self.typist.reload_layouts(walk, wanted);

    self.worn = self.typist.last_alphabet;
    let Ok(()) = self.repeat.stop();
    self.selected = None;
    self.pressed = None;
    self.shown = None;
    self.kept = None;
    self.preparing = preparing::Opening::None;
    let Ok(()) = self.screen.forget_kept();

    Ok(())
  }

  fn drawing(&mut self, showing_now: Showing) -> Result<(), SurfaceError> {
    let Ok(configured) = self.screen.size();
    let size = match (showing_now, configured) {
      (Showing::Yes, Some(available)) => available,
      (Showing::No, _) | (Showing::Yes, None) => return Ok(()),
    };
    let Ok(scale) = self.screen.scale();
    let Ok(language) = self.typist.next_language();
    let frame = KeyboardState {
      showing: self.typist.showing,
      held: self.typist.held,
      pressed: self.pressed,
      selected: self.selected,
      size,
      scale,
      language,
    };

    let Ok(plan) = frame.plan(self.shown, self.kept);
    let (drawn, reused) = match plan {
      DrawPlan::Current => return Ok(()),
      DrawPlan::Recall => self.recall(frame)?,
      DrawPlan::Paint(keep) => self.paint_or_prepare(frame, keep)?,
    };

    match drawn {
      Drawn::Yes => self.shown = Some(frame),
      Drawn::NotYet => return Ok(()),
    }

    match self.showing.take() {
      Some(mut waiting) => {
        match reused {
          Reused::Yes => {
            let Ok(()) = waiting.counted("prepared_frame", 1);
          }
          Reused::No => {}
        }

        let Ok(()) = waiting.mark("drawn");
        let Ok(()) = waiting.finish();
      }
      None => {}
    }

    Ok(())
  }

  fn recall(&mut self, frame: KeyboardState) -> Result<(Drawn, Reused), SurfaceError> {
    let reused = self.screen.draw_kept()?;

    match reused {
      Drawn::Yes => Ok((Drawn::Yes, Reused::Yes)),
      Drawn::NotYet => {
        let drawn = self.paint_frame(frame, Keep::Resting)?;

        Ok((drawn, Reused::No))
      }
    }
  }

  fn paint_or_prepare(
    &mut self,
    frame: KeyboardState,
    keep: Keep,
  ) -> Result<(Drawn, Reused), SurfaceError> {
    let Ok(warmed) = self.preparing.take(frame);

    match warmed {
      Some(prepared) => {
        let drawn = self.screen.draw_prepared(prepared.pixels)?;
        let Ok(()) = prepared.fonts.use_on_draw_thread();

        match drawn {
          Drawn::Yes => {
            self.kept = Some(frame);

            return Ok((Drawn::Yes, Reused::Yes));
          }
          Drawn::NotYet => {}
        }
      }
      None => {}
    }

    let drawn = self.paint_frame(frame, keep)?;

    Ok((drawn, Reused::No))
  }

  fn paint_frame(&mut self, frame: KeyboardState, keep: Keep) -> Result<Drawn, SurfaceError> {
    let configuration = self.configuration;
    let drawn = self.screen.draw(keep, |pixels, wide, tall| {
      let Ok(()) = frame.paint(
        configuration,
        pixels,
        Size {
          width: wide,
          height: tall,
        },
      );
    })?;

    match (&drawn, keep) {
      (Drawn::Yes, Keep::Resting) => self.kept = Some(frame),
      (Drawn::NotYet, _) | (Drawn::Yes, Keep::Transient) => {}
    }

    Ok(drawn)
  }

  fn waiting_on(&mut self) -> Result<u32, SurfaceError> {
    let now = Instant::now();
    let mut watching: Vec<BorrowedFd<'_>> = vec![self.told.as_fd()];

    match self.reading.as_ref() {
      Some(reading) => {
        let Ok(fds) = reading.claim.descriptors();

        watching.extend(fds);
      }
      None => {}
    }

    let Ok(until) = self.repeat.until(now);

    self.screen.wait_with(&watching, until)
  }

  fn handle_commands(&mut self, events: u32) -> Result<(), Unopened> {
    match ::core::primitive::bool::then_some(events & 1 != 0, ()) {
      ::core::option::Option::Some(()) => {
        let heard = receive_commands(&self.told).map_err(Unopened::Commands)?;

        for asked in heard {
          let Ok(showing_now) = self.screen.visibility();

          match (asked, showing_now) {
            (Command::Show, Showing::No) | (Command::Toggle, Showing::No) => {
              self.onto_the_screen()?
            }
            (Command::Show, Showing::Yes) | (Command::Hide, Showing::No) => {}
            (Command::Hide, Showing::Yes) | (Command::Toggle, Showing::Yes) => {
              let Ok(()) = self.away();
            }
            (Command::ReloadLayouts, _) => {
              let Ok(()) = self.reload_layouts();
            }
          }
        }
      }
      ::core::option::Option::None => {}
    }

    Ok(())
  }

  fn wants(&mut self, now: Instant) -> Result<(), Never> {
    let mut changes: Vec<Change> = Vec::new();
    let mut unplugged: Vec<claim::DeviceKind> = Vec::new();

    match self.reading.as_mut() {
      Some(at_hand) => {
        let Ok(heard) = at_hand.claim.receive();
        let Ok(changed) = from_controller(&heard, &at_hand.spans);

        changes.extend(changed);
        unplugged.extend(heard.unplugged);
      }
      None => {}
    }

    for change in changes {
      let Ok(()) = self.changed(change, now);
    }

    match ::core::primitive::bool::then_some(unplugged.is_empty(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        eprintln!("console-keyboard: the controller is missing; taking it again");
        self.reading = None;

        let Ok(()) = self.repeat.stop();
      }
    }

    Ok(())
  }

  fn changed(&mut self, change: Change, now: Instant) -> Result<(), Never> {
    match change {
      Change::Released { element } => self.repeat.released(Input::Controller(element)),
      Change::Pressed { element, command } => {
        let Ok(cadence) = command.cadence();
        let Ok(press) = self.repeat.pressed(
          Input::Controller(element),
          Repeat::Command(command),
          cadence,
          now,
        );

        match press {
          Press::AlreadyDown => Ok(()),
          Press::New => {
            let Ok(mode) = self.command(command);

            self.repeating(mode)
          }
        }
      }
    }
  }

  fn repeats(&mut self, now: Instant) -> Result<(), Never> {
    let Ok(due) = self.repeat.due(now);

    let again = match due {
      Some(again) => again,
      None => return Ok(()),
    };

    let Ok(mode) = match again {
      Repeat::Command(command) => self.command(command),
      Repeat::Key(at) => self.touched(at),
    };

    self.repeating(mode)
  }

  fn repeating(&mut self, mode: RepeatMode) -> Result<(), Never> {
    match mode {
      RepeatMode::Repeating => Ok(()),
      RepeatMode::Once => self.repeat.stop(),
    }
  }

  fn command(&mut self, want: KeyboardCommand) -> Result<RepeatMode, Never> {
    let Ok(showing_now) = self.screen.visibility();

    match showing_now {
      Showing::No => return Ok(RepeatMode::Once),
      Showing::Yes => {}
    }

    let (wide, tall) = match self.screen.size() {
      Ok(Some((wide, tall))) => (wide, tall),
      Ok(None) | Err(_) => return Ok(RepeatMode::Once),
    };

    let Ok(layout) = self.typist.layout();
    let Ok(keys) = layout_keys(
      layout,
      Size {
        width: f64::from(wide),
        height: f64::from(tall),
      },
    );

    Ok(match want {
      KeyboardCommand::Toggle => {
        let Ok(()) = self.away();

        self.selected = None;

        RepeatMode::Once
      }
      KeyboardCommand::Up
      | KeyboardCommand::Down
      | KeyboardCommand::Left
      | KeyboardCommand::Right => {
        let Ok(direction) = want.direction();

        let (across, down) = match direction {
          Some(both) => both,
          None => NOWHERE,
        };

        let Ok(onto) = toward(&keys, self.selected, Point { x: across, y: down });

        self.selected = onto;

        RepeatMode::Repeating
      }
      KeyboardCommand::Select => {
        match ::core::primitive::bool::then_some(self.selected.is_none(), ()) {
          ::core::option::Option::Some(()) => {
            let Ok(onto) = toward(&keys, None, Point { x: 0, y: 0 });

            self.selected = onto;

            return Ok(RepeatMode::Once);
          }
          ::core::option::Option::None => {}
        }

        let Ok(mode) = self.activate_key(Activation::Focused);

        mode
      }
      KeyboardCommand::Backspace => {
        let Ok(()) = self.typist.tap(key::BACKSPACE);

        RepeatMode::Repeating
      }
      KeyboardCommand::Enter => {
        let Ok(()) = self.typist.tap(key::ENTER);

        RepeatMode::Repeating
      }
      KeyboardCommand::Shift => {
        let Ok(_) =
          self
            .typist
            .pressed(Kind::Mod(modifiers::SHIFT), modifiers::NONE, Drops::None);

        RepeatMode::Once
      }
      KeyboardCommand::PreviousLanguage | KeyboardCommand::NextLanguage => {
        let Ok(mut waiting) = Waiting::here(Wait {
          who: "keyboard",
          what: "language",
        });
        let Ok(_) = self
          .typist
          .pressed(Kind::Language, modifiers::NONE, Drops::None);
        let Ok(()) = waiting.mark("keymap");
        let Ok(()) = waiting.finish();

        self.selected = None;

        RepeatMode::Once
      }
    })
  }

  fn pointer_events(&mut self, now: Instant) -> Result<(), Never> {
    let (wide, tall) = match self.screen.size() {
      Ok(Some((wide, tall))) => (wide, tall),
      Ok(None) | Err(_) => {
        let Ok(_) = self.screen.pointer_events();

        return Ok(());
      }
    };

    let Ok(pointer_events) = self.screen.pointer_events();

    for event in pointer_events {
      match event {
        PointerEvent::Down { x, y } => {
          let Ok(layout) = self.typist.layout();
          let Ok(keys) = layout_keys(
            layout,
            Size {
              width: f64::from(wide),
              height: f64::from(tall),
            },
          );
          let hit = match under(&keys, Point { x, y }) {
            Ok(Some(hit)) => hit,
            Ok(None) | Err(_) => continue,
          };

          let Ok(mode) = self.touched(hit.at);

          match mode {
            RepeatMode::Repeating => {
              let Ok(_) = self
                .repeat
                .pressed(Input::Touch, Repeat::Key(hit.at), TYPING, now);
            }
            RepeatMode::Once => {
              let Ok(()) = self.repeat.stop();
            }
          }
        }
        PointerEvent::Moved { .. } => {}
        PointerEvent::Up => {
          self.pressed = None;

          let Ok(()) = self.repeat.released(Input::Touch);
        }
      }
    }

    Ok(())
  }

  fn touched(&mut self, at: u32) -> Result<RepeatMode, Never> {
    self.activate_key(Activation::At(at))
  }

  fn activate_key(&mut self, activation: Activation<u32>) -> Result<RepeatMode, Never> {
    let at = {
      let Ok(optional_at) = activation.resolve(|| self.selected, Some);

      match optional_at {
        Some(matched_at) => matched_at,
        ::core::option::Option::None => return Ok(RepeatMode::Once),
      }
    };

    let Ok(layout) = self.typist.layout();
    let Ok(place) = index(at);

    let key = match layout.keys.get(place) {
      Some(key) => key,
      None => return Ok(RepeatMode::Once),
    };

    let pointing = matches!(activation, Activation::At(_));

    match ::core::primitive::bool::then_some(pointing, ()) {
      ::core::option::Option::Some(()) => {
        self.pressed = Some(at);
      }
      ::core::option::Option::None => {}
    }

    let kind = key.kind;
    let force = key.force;
    let reset = key.reset;
    let before = self.typist.showing;
    let changing =
      match ::core::primitive::bool::then_some(pointing && matches!(kind, Kind::Language), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(waiting) = Waiting::here(Wait {
            who: "keyboard",
            what: "language",
          });

          Some(waiting)
        }
        ::core::option::Option::None => None,
      };
    let Ok(after) = self.typist.pressed(kind, force, reset);

    match changing {
      Some(mut waiting) => {
        let Ok(()) = waiting.mark("keymap");
        let Ok(()) = waiting.finish();
      }
      None => {}
    }

    match ::core::primitive::bool::then_some(
      after == After::Draw && !matches!(kind, Kind::Code { .. }),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        match ::core::primitive::bool::then_some(pointing, ()) {
          ::core::option::Option::Some(()) => {
            self.pressed = None;
          }
          ::core::option::Option::None => {
            self.selected = None;
          }
        }
      }
      ::core::option::Option::None => {}
    }

    typing::repeat_mode(kind, before, self.typist.showing)
  }

  fn onto_the_screen(&mut self) -> Result<(), SurfaceError> {
    let Ok(mut waiting) = Waiting::here(Wait {
      who: "keyboard",
      what: "showing",
    });

    self.screen.show(self.height)?;

    let Ok(()) = waiting.mark("surface");

    self.showing = Some(waiting);

    Ok(())
  }

  fn away(&mut self) -> Result<(), Never> {
    let Ok(()) = self.screen.hide();

    self.shown = None;
    self.showing = None;
    self.pressed = None;

    let Ok(()) = self.repeat.stop();

    Ok(())
  }
}

#[derive(Debug)]
enum Unopened {
  NoKeymaps(console_input_keyboard::keymap::Error),
  NoLayer(Vec<String>),
  Sessionless,
  Deaf(std::path::PathBuf, std::io::Error),
  Commands(io::Error),
  NoTyping,
  Screen(SurfaceError),
}

impl std::fmt::Display for Unopened {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unopened::NoKeymaps(why) => {
        write!(to, "no keymaps: {why:?}. Is xkeyboard-config installed?")
      }
      Unopened::NoLayer(asked) => write!(
        to,
        "no layer called any of {asked:?}. --list-layers says what there is"
      ),
      Unopened::Sessionless => write!(
        to,
        "XDG_RUNTIME_DIR: nothing says where to listen for being shown or hidden"
      ),
      Unopened::Deaf(at, why) => write!(
        to,
        "{}: nothing can ask it to show or hide: {why}",
        at.display()
      ),
      Unopened::Commands(why) => write!(to, "could not read a keyboard command: {why}"),
      Unopened::NoTyping => write!(
        to,
        "this compositor has no zwp_virtual_keyboard_v1, so nothing here could type"
      ),
      Unopened::Screen(why) => write!(to, "{why}"),
    }
  }
}

impl std::error::Error for Unopened {}

impl std::convert::From<SurfaceError> for Unopened {
  fn from(why: SurfaceError) -> Self {
    Unopened::Screen(why)
  }
}

fn run(configuration: &Configuration, mut waiting: Waiting) -> Result<(), Unopened> {
  let Ok(root) = console_input_keyboard::keymap::default_symbols_root();
  let alphabets =
    console_input_keyboard::keymap::available("evdev", &root).map_err(Unopened::NoKeymaps)?;

  let Ok(()) = waiting.mark("keymaps");

  let Ok(output) = display();
  let Ok(shape) = shape_of(output);
  let Ok((asked, height)) = orientation(configuration, shape);
  let Ok(opening) = left_on(shape);
  let Ok(walk) = layout_walk(&asked);

  match ::core::primitive::bool::then_some(walk.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Err(Unopened::NoLayer(asked)),
    ::core::option::Option::None => {}
  }

  let told = bind_socket()?;
  let mut screen = Screen::connect()?;
  let Ok(predicted) = preparing::geometry(output, height);

  match predicted {
    Some((_, scale)) => {
      let Ok(()) = screen.expect_scale(scale);
    }
    None => {}
  }

  let Ok(()) = waiting.mark("compositor");

  match ::core::primitive::bool::then_some(configuration.hidden, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => screen.show(height)?,
  }

  let Ok(typing) = screen.typing();
  let typing_2 = typing.ok_or(Unopened::NoTyping)?;
  let manager = typing_2.clone();
  let Ok(hand) = screen.hand();
  let Ok(seat) = screen.seat();
  let Ok(typist) = Typist::new(
    &manager,
    seat,
    &hand,
    alphabets,
    walk,
    opening,
    Instant::now(),
  );
  let worn = typist.last_alphabet;
  let Ok(language) = typist.next_language();
  let preparing = match predicted {
    Some((size, scale)) => {
      let view = KeyboardState {
        showing: typist.showing,
        held: typist.held,
        pressed: None,
        selected: None,
        size,
        scale,
        language,
      };
      let Ok(preparing) = preparing::Opening::start(configuration, view);

      preparing
    }
    None => preparing::Opening::None,
  };

  let Ok(()) = waiting.mark("typist");
  let Ok(()) = waiting.mark("pad");
  let Ok(()) = waiting.finish();

  let mut state = State {
    screen,
    told,
    configuration,
    height,
    typist,
    repeat: KeyRepeat::default(),
    pressed: None,
    shown: None,
    kept: None,
    preparing,
    showing: None,
    reading: None,
    complained: Logged::No,
    worn,
    selected: None,
    standing: shape,
  };

  state.while_open()
}

struct Reading {
  claim: Claim,
  spans: Spans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Logged {
  Yes,
  No,
}

fn sync_reading(
  showing: Showing,
  reading: &mut Option<Reading>,
  complained: &mut Logged,
) -> Result<(), Never> {
  match showing {
    Showing::No => *reading = None,
    Showing::Yes => match reading {
      Some(_) => {}
      None => {
        let Ok(taking) = taking(complained);

        *reading = taking;
      }
    },
  }

  Ok(())
}

fn taking(complained: &mut Logged) -> Result<Option<Reading>, Never> {
  Ok(match Claim::of(&CONTROLLER) {
    Ok(claim) => {
      let Ok(holding) = claim.devices();

      for (which, path) in holding {
        let Ok(said) = which.said();

        eprintln!("console-keyboard: reading the {said} at {path}");
      }

      let spans = match claim.spans(claim::DeviceKind::Pad) {
        Ok(spans) => spans,

        Err(refused) => {
          let Ok(said) = refused.message();

          eprintln!("console-keyboard: {said}; the sticks will not walk");

          Spans::new()
        }
      };
      *complained = Logged::No;

      Some(Reading { claim, spans })
    }

    Err(refused) => {
      match *complained {
        Logged::Yes => {}
        Logged::No => {
          let Ok(said) = refused.message();

          eprintln!("console-keyboard: {said}, so it is touch only");

          *complained = Logged::Yes;
        }
      }

      None
    }
  })
}

fn from_controller(heard: &Received, spans: &Spans) -> Result<Vec<Change>, Never> {
  Ok(
    heard
      .events
      .iter()
      .filter_map(|(which, event)| {
        let Ok(change) = gamepad::change(*which, *event, spans);

        change
      })
      .collect(),
  )
}

#[derive(Clone, Copy, PartialEq)]
struct KeyboardState {
  showing: LayoutKind,
  held: u8,
  pressed: Option<u32>,
  selected: Option<u32>,
  size: (u32, u32),
  scale: i32,
  language: Option<LayoutKind>,
}

enum Reused {
  Yes,
  No,
}

enum DrawPlan {
  Current,
  Recall,
  Paint(Keep),
}

impl KeyboardState {
  fn plan(self, shown: Option<Self>, kept: Option<Self>) -> Result<DrawPlan, Never> {
    Ok(
      match (shown == Some(self), kept == Some(self), self.pressed) {
        (true, _, _) => DrawPlan::Current,
        (false, true, _) => DrawPlan::Recall,
        (false, false, None) => DrawPlan::Paint(Keep::Resting),
        (false, false, Some(_)) => DrawPlan::Paint(Keep::Transient),
      },
    )
  }

  fn paint(
    self,
    configuration: &Configuration,
    pixels: &mut [u8],
    device: Size<u32>,
  ) -> Result<(), Never> {
    let Ok(layout) = of(self.showing);
    let language = self.language.map(|which| {
      let Ok(of) = of(which);
      let Ok(written) = of.alphabet.written();

      written
    });
    let Ok(along) = fitted(device.width.saturating_mul(4));
    let Ok(down) = fitted(device.height);
    let mut onto = match Surface::new(pixels, Stride(along), down, BufferScale(self.scale)) {
      Ok(Some(onto)) => onto,
      Ok(None) | Err(_) => return Ok(()),
    };
    let room = Size {
      width: f64::from(self.size.0),
      height: f64::from(self.size.1),
    };
    let Ok(keys) = layout_keys(layout, room);
    paint::keyboard(
      &mut onto,
      &paint::Look {
        configuration,
        layout,
        keys: &keys,
        pressed: self.pressed,
        held: self.held,
        selected: self.selected,
        language,
        room,
      },
    )
  }
}

fn left_on(shape: Shape) -> Result<Option<LayoutKind>, Never> {
  let Ok(home) = console_core_places::home();

  let home_3 = match home {
    Some(home_2) => home_2,
    None => return Ok(None),
  };

  let Ok(alphabet) = alphabets::wearing::read(&home_3, alphabets::wearing::SCREEN);

  let arrangement = match shape {
    Shape::Wider => alphabet.width,
    Shape::Taller => alphabet.upright,
  };

  find_layout(arrangement)
}

fn keeps_its_own(showing: LayoutKind) -> Result<(), Never> {
  let Ok(home) = console_core_places::home();

  let home_3 = match home {
    Some(home_2) => home_2,
    None => return Ok(()),
  };

  let Ok(of) = of(showing);
  let named = alphabets::EVERY
    .iter()
    .find(|alphabet| alphabet.width == of.name || alphabet.upright == of.name);

  let alphabet = match named {
    Some(alphabet) => alphabet,
    None => return Ok(()),
  };

  match alphabets::wearing::remember(&home_3, alphabets::wearing::SCREEN, alphabet) {
    Ok(()) => {}
    Err(fault) => {
      console_core_logging::warn!("could not remember the on-screen keyboard layout: {fault}")
    }
  }

  Ok(())
}

fn orientation(configuration: &Configuration, shape: Shape) -> Result<(Vec<String>, u32), Never> {
  let (given, holding, height) = match shape {
    Shape::Wider => (
      &configuration.landscape_layers,
      Holding::Across,
      configuration.landscape_height,
    ),
    Shape::Taller => (
      &configuration.layers,
      Holding::Upright,
      configuration.height,
    ),
  };

  let asked = match ::core::primitive::bool::then_some(given.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(chosen) = alphabets::current();
      let Ok(walk) = alphabets::walk(&chosen, holding);

      walk
    }
    ::core::option::Option::None => given.clone(),
  };

  Ok((asked, height))
}

fn layout_walk(names: &[String]) -> Result<Vec<LayoutKind>, Never> {
  Ok(
    names
      .iter()
      .filter_map(|name| {
        let Ok(named) = find_layout(name);

        named
      })
      .collect(),
  )
}

fn bind_socket() -> Result<UnixDatagram, Unopened> {
  let Ok(at) = remote::socket();
  let at_2 = at.ok_or(Unopened::Sessionless)?;

  match at_2.parent() {
    Some(above) => {
      std::fs::create_dir_all(above).map_err(|why| Unopened::Deaf(above.to_path_buf(), why))?
    }
    None => {}
  }

  std::fs::remove_file(&at_2)
    .or_else(
      |fault| match bool::then_some(fault.kind() == io::ErrorKind::NotFound, ()) {
        Some(()) => Ok(()),
        None => Err(fault),
      },
    )
    .map_err(|why| Unopened::Deaf(at_2.clone(), why))?;

  let told = UnixDatagram::bind(&at_2).map_err(|why| Unopened::Deaf(at_2.clone(), why))?;

  told
    .set_nonblocking(true)
    .map_err(|why| Unopened::Deaf(at_2, why))?;

  Ok(told)
}

fn receive_commands(told: &UnixDatagram) -> io::Result<Vec<Command>> {
  let mut commands = Vec::new();

  for _ in 0..REMOTE_COMMANDS_PER_TURN {
    let mut said = [0_u8; 16];
    let got = match told.recv(&mut said) {
      Ok(bytes) => {
        let Ok(length) = fitted::<_, u16>(bytes);

        length
      }
      Err(fault) => match (
        fault.kind() == io::ErrorKind::WouldBlock,
        fault.kind() == io::ErrorKind::Interrupted,
      ) {
        (true, _) => break,
        (false, true) => continue,
        (false, false) => return Err(fault),
      },
    };
    let Ok(end) = index(got);
    let bytes = said
      .get(..end)
      .ok_or_else(|| io::Error::other("the keyboard command exceeded its receive buffer"))?;
    let word = match std::str::from_utf8(bytes) {
      Ok(word) => word,
      Err(fault) => {
        eprintln!("console-keyboard: something was said that is not a word: {fault}");

        continue;
      }
    };
    let Ok(asked) = Command::read(word);

    commands.extend(asked);
  }

  Ok(commands)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_backlog_of_remote_commands_keeps_its_order_across_turns() -> io::Result<()> {
    let (sending, receiving) = UnixDatagram::pair()?;

    receiving.set_nonblocking(true)?;

    for _ in 0..REMOTE_COMMANDS_PER_TURN {
      sending.send(b"show")?;
    }

    sending.send(b"hide")?;

    let first = receive_commands(&receiving)?;
    let Ok(count) = fitted::<_, u16>(first.len());
    let second = receive_commands(&receiving)?;
    let third = receive_commands(&receiving)?;

    assert_eq!(count, REMOTE_COMMANDS_PER_TURN);
    assert!(first.iter().all(|command| *command == Command::Show));
    assert_eq!(second, [Command::Hide]);
    assert!(third.is_empty());

    Ok(())
  }

  const HANDHELD: console_screen::Screen = console_screen::Screen {
    mode: Size {
      width: 1600,
      height: 2560,
    },
    refresh: 144,
    scale: 2.5,
    transform: console_screen::Transform::Degrees90,
  };

  #[test]
  fn the_board_stands_the_way_the_screen_does_and_not_always_across() {
    let upright = console_screen::Screen {
      transform: console_screen::Transform::Normal,
      ..HANDHELD
    };

    assert_eq!(shape_of(Some(HANDHELD)), Ok(Shape::Wider));
    assert_eq!(
      shape_of(Some(upright)),
      Ok(Shape::Taller),
      "a screen turned upright got the board laid across it"
    );
    assert_eq!(
      shape_of(None),
      Ok(Shape::Wider),
      "a compositor that will not say keeps the board the device is held for"
    );
  }
}
