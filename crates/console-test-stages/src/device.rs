//! The machine itself, pressed through InputPlumber and looked at over ssh.
//!
//! Nothing here makes an input device. InputPlumber is asked to emit the event
//! it would have read from the hardware, through the profile that is loaded,
//! which is its own supported way of doing this and is what a chord on the
//! device already uses. So there is no second pad for the daemons to find and
//! nothing to clean up if a check stops halfway.
//!
//! Completion checks ask immediately, then use the shared wait only while
//! pending. Query time spends the deadline, query failures propagate, and dry
//! runs advance virtual time through the same wait without sleeping.
//!
//! Every press is a chord, and that is not a shorthand for a press. InputPlumber
//! has two ways to be asked for an event and only one of them works: `SendEvent`
//! reaches a `blocking_send` on a tokio worker and panics the daemon's own task
//! rather than emitting anything, on every version this desktop has run, so a
//! button cannot be held down and a trigger cannot be pulled. `SendButtonChord`
//! presses and lets go by itself and is sound. So a walk is taps rather than a
//! hold, a key is a chord of `Keyboard:` capabilities, and the two that need a
//! button held say they cannot rather than sending something the daemon drops on
//! the floor. The backlog is the rest, and this comes back the day it is fixed.
//!
//! A name asked of the kernel is not always the name in `[build]`. `comm` is
//! sixteen bytes with the terminator, so anything longer arrives cut to
//! fifteen: `controller-desktop` reads as `controller-desk`, and `pgrep -x`
//! matches on the same field and finds nothing at all for a pattern past that
//! length. It was `stick-scroll` and it fitted, which is why nothing here knew.
//! So a check that holds a program's name against what a machine reports asks
//! for it through `comm` rather than writing the name twice.
//!
//! A chord also lets go in its own time, and the call comes back before it does.
//! Asking for one puts the button down, answers, and lets go eighty milliseconds
//! later; a second chord inside that window is not a second press, because the
//! axis is already where it would put it and there is nothing for the kernel to
//! report. Eighteen of them in a shell loop take sixty-six milliseconds and reach
//! the pad as one press and one release, which is what `290` was walking on: a
//! walk of eighteen squares that moved one, or four on a slower loop, with no
//! error anywhere to say which. So a walk waits 120 milliseconds between one chord and
//! the next. The number is the daemon's rather than this file's, and reading it
//! again means asking for a chord and watching the pad's own node for what came
//! out of it.
//!
//! What a press then does to the machine is another matter, and it is someone
//! else's machine. Two things here are the bookkeeping that lets a run give it
//! back the way it found it: a window opened through `open` is remembered, so
//! `close_window` can end the process the run started rather than something
//! the person was using, and a level is read as a `Level` rather than as a
//! number, so a reading no one got is never mistaken for a screen at zero.
//!
//! A device behind Tailscale SSH can stop answering until somebody approves a
//! login in a browser, and ssh says where on its error stream -- which this
//! kept and never showed, so a whole evening of timeouts read as bad Wi-Fi. The
//! link is printed on its own line whenever ssh says one.

use console_compositor::{Query, Request};
use console_core_external_programs::{Program, shell};
use console_core_internal_programs::InternalProgram;
use console_core_number_conversion::fitted;
pub use shell::quote as shell_quote;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use console_core_geometry::Point;
use console_core_never::Never;
pub use console_input_gamepad::devices::TriggerTravel;
use console_input_gamepad::front;
use console_input_gamepad::profile::{Kind, Profile};
use console_input_gamepad::router::every_profile;
use console_input_gamepad::vocabulary;
use console_session::reaching::{self, Whom};

use crate::Error;
use crate::checking::{CheckResult, cannot, failed, less_than, more_than};
use crate::picture::{Picture, where_};

const EMPTY_WORKSPACE: &str = "empty";

const NO_WINDOWS: i64 = 0;

pub const COMM: u32 = 15;

pub fn comm(named: &str) -> Result<&str, Never> {
  let Ok(comm) = console_core_number_conversion::index(COMM);

  Ok(match named.get(..comm) {
    Some(cut) => cut,
    None => named,
  })
}

pub fn host() -> Result<String, Error> {
  let said = console_device_name::device()?;

  match ::core::primitive::bool::then_some(said.trim().is_empty(), ()) {
    ::core::option::Option::Some(()) => Err(Error::Hostless),
    ::core::option::Option::None => Ok(said),
  }
}

const PIECES: [&str; 20] = [
  "console-input-controller",
  "console-input-keyboard",
  "console-bar",
  "console-clipboard",
  "console-notify",
  "console-panels",
  "console-polkit",
  "console-session",
  "console-paper",
  "console-wallpaper",
  "console-events",
  "console-home",
  "console-control-center",
  "console-overview",
  "console-idle",
  "console-warm",
  "console-light",
  "console-wifi",
  "console-stream-privacy",
  "syncthing",
];

const BY_A_SWITCH: [&str; 2] = ["console-warm", "console-light"];

const SENDEVENT: &str = "InputPlumber's SendEvent panics on its own runtime rather than \
     emitting anything, so nothing here can hold a button down or pull a trigger; \
     press instead, and see the backlog";

fn logins(said: &str) -> Result<Vec<&str>, Never> {
  Ok(
    said
      .split_whitespace()
      .filter(|word| word.starts_with(LOGIN))
      .collect(),
  )
}

const LOGIN: &str = "https://login.tailscale.com/";

pub const SETTLED: f64 = 0.6;

pub const PATIENCE: f64 = 4.0;

pub const OPENING: f64 = 12.0;

pub const LEAVING: f64 = 12.0;

const WALLPAPER_SURFACE: &str = "hyprpaper";

fn spoken_as(kind: Kind, name: &str) -> Result<Option<String>, Never> {
  Ok(match kind {
    Kind::Key => Some(format!("Keyboard:{name}")),
    Kind::MouseButton => Some(format!("Mouse:Button:{name}")),
    Kind::GamepadButton => Some(format!("Gamepad:Button:{name}")),
    Kind::MouseMotion | Kind::GamepadAxis | Kind::GamepadTrigger => None,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Value<'a>(&'a str);

#[derive(Debug, Default)]
struct Came {
  which: String,
  where_: String,
}

struct RecordedCommand<'a> {
  label: &'a str,
  script: &'a str,
}

#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "`dry` is how this stage was started and the absences are what it has learnt since -- whose machine it is, a picture taken, a profile kept, the screen, a watcher; a dry stage that has read the screen is the ordinary one"
  )
)]
pub struct Device<'watch> {
  host: String,
  whom: Option<String>,
  dry_run: DryRun,
  done: Vec<String>,
  profiles: BTreeMap<String, Profile>,
  taken: Option<Picture>,
  kept: Option<PathBuf>,
  opened: Vec<String>,
  pushed: Pushed,
  screen: Option<console_screen::Screen>,
  watching: Option<&'watch std::sync::Mutex<crate::watching::Watching>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pushed {
  Some,
  None,
}

impl From<f64> for Pushed {
  fn from(amount: f64) -> Self {
    match amount.classify() {
      core::num::FpCategory::Zero => Self::None,
      core::num::FpCategory::Nan
      | core::num::FpCategory::Infinite
      | core::num::FpCategory::Subnormal
      | core::num::FpCategory::Normal => Self::Some,
    }
  }
}

#[derive(Clone, Copy)]
pub enum Wait {
  For(Duration),
  TooLong,
  Invalid,
}

impl From<f64> for Wait {
  fn from(seconds: f64) -> Self {
    match Duration::try_from_secs_f64(seconds) {
      Ok(span) => Self::For(span),
      Err(_out_of_range) => match bool::then_some(seconds > 0.0, ()) {
        Some(()) => Self::TooLong,
        None => Self::Invalid,
      },
    }
  }
}

impl From<Duration> for Wait {
  fn from(span: Duration) -> Self {
    Self::For(span)
  }
}

impl Wait {
  fn limit(self) -> Result<Duration, Never> {
    Ok(match self {
      Self::For(span) => span,
      Self::TooLong => Duration::MAX,
      Self::Invalid => Duration::ZERO,
    })
  }
}

fn lookup<'a>(table: &'a [(&'a str, &'a str)], spoken: &str) -> Result<Option<&'a str>, Never> {
  Ok(
    table
      .iter()
      .find(|(said, _)| *said == spoken)
      .map(|(_, name)| *name),
  )
}

fn suggestions(table: &[(&str, &str)]) -> Result<String, Never> {
  let every: Vec<&str> = table.iter().map(|(spoken, _)| *spoken).collect();

  Ok(format!("try one of {}", every.join(", ")))
}

fn menus_up(seen: &mut Device) -> Result<Ready, Never> {
  let Ok(menus) = seen.menus();

  Ok(
    match ::core::primitive::bool::then_some(menus.is_empty(), ()) {
      ::core::option::Option::Some(()) => Ready::NotYet,
      ::core::option::Option::None => Ready::Yes,
    },
  )
}

pub use console_waiting::{Outcome, Ready};

pub const A_MOMENT: f64 = 2.0;

pub const A_PICTURE: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryRun {
  Pretend,
  Really,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
  At(i64),
  Unsaid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
  Up,
  Down,
}

impl Level {
  pub fn expect_moved(self, way: Way, from: Level, unsaid: &str) -> CheckResult {
    let (was, now) = match (from, self) {
      (Level::At(was), Level::At(now)) => (was, now),
      (Level::Unsaid, _) | (_, Level::Unsaid) => return failed(unsaid.to_string()),
    };

    let why = || format!("it was {was} and is {now}");

    match way {
      Way::Up => more_than(now, was, why),
      Way::Down => less_than(now, was, why),
    }
  }
}

fn brightness_in(said: &str) -> Result<Level, Never> {
  let line = match said.lines().next() {
    Some(line) => line,
    None => return Ok(Level::Unsaid),
  };

  Ok(match line.trim().parse() {
    Ok(brightness) => Level::At(brightness),
    Err(_unreadable) => Level::Unsaid,
  })
}

fn volume_in(said: &str) -> Result<Level, Never> {
  let word = said
    .lines()
    .next()
    .and_then(|line| line.split_whitespace().nth(4));

  let word_3 = match word {
    Some(word_2) => word_2,
    None => return Ok(Level::Unsaid),
  };

  Ok(match word_3.trim_end_matches('%').parse() {
    Ok(volume) => Level::At(volume),
    Err(_unreadable) => Level::Unsaid,
  })
}

impl<'watch> Device<'watch> {
  pub fn new(host: &str, dry_run: DryRun) -> Result<Self, Error> {
    let Ok(root) = crate::root();
    let profiles = every_profile(&root)?;

    Ok(Device {
      host: host.to_string(),
      dry_run,
      done: Vec::new(),
      profiles,
      taken: None,
      kept: None,
      opened: Vec::new(),
      whom: None,
      pushed: Pushed::None,
      screen: None,
      watching: None,
    })
  }

  pub fn dry_run(&self) -> Result<DryRun, Never> {
    Ok(self.dry_run)
  }

  pub fn done(&self) -> Result<&[String], Never> {
    Ok(&self.done)
  }

  pub fn whoever(&mut self) -> Result<String, Never> {
    match &self.whom {
      Some(known) => return Ok(known.clone()),
      None => {}
    }

    #[cfg_attr(
      dylint_lib = "explicit026_env_read_once",
      allow(
        explicit026_env_read_once,
        reason = "CONSOLE_USER says whose the device is when the device cannot be asked, and asking the device is what this file does"
      )
    )]
    let told = match std::env::var("CONSOLE_USER") {
      Ok(said) => match ::core::primitive::bool::then_some(said.trim().is_empty(), ()) {
        ::core::option::Option::Some(()) => None,
        ::core::option::Option::None => Some(said.trim().to_string()),
      },
      Err(_unset) => None,
    };

    let discovered = match told {
      Some(said) => said,
      None => {
        let Ok(homes) = console_device_name::homes();
        let Ok(listed) = self.discovery_output(&homes);

        let Ok(home) = console_device_name::sole_home(&listed);

        match home {
          Some(name) => name.to_owned(),
          None => {
            let Ok(fallback) = console_device_name::fallback();
            let Ok(found) = self.discovery_output(&fallback);

            let Ok(owner) = console_device_name::account(&found);

            match owner {
              Some(present) => present,
              None => "",
            }
            .to_owned()
          }
        }
      }
    };

    let said = match ::core::primitive::bool::then_some(discovered.trim().is_empty(), ()) {
      ::core::option::Option::Some(()) => console_manifest_migrations::USER.to_string(),
      ::core::option::Option::None => discovered.trim().to_string(),
    };

    self.whom = Some(said.clone());

    Ok(said)
  }

  pub fn home(&mut self) -> Result<String, Never> {
    let Ok(whom) = self.whoever();

    Ok(format!("/home/{whom}"))
  }

  pub fn set_watching(
    &mut self,
    watching: &'watch std::sync::Mutex<crate::watching::Watching>,
  ) -> Result<(), Never> {
    self.watching = Some(watching);

    Ok(())
  }

  fn along(&mut self) -> Result<Option<console_notifications::updating::Progress>, Never> {
    let watching = match self.watching {
      Some(watching) => watching,
      None => return Ok(None),
    };

    let Ok(held) = crate::watching::lock(watching);

    match held {
      Some(mut acquired) => acquired.tick(),
      None => Ok(None),
    }
  }

  pub fn ssh(&mut self, command: &str) -> Result<String, Never> {
    let Ok(along) = self.along();
    let asked = match along {
      Some(progress) => {
        let Ok(writing) = crate::watching::writing(&progress);
        format!("{command}; {writing}")
      }
      None => command.to_owned(),
    };

    Ok(
      match self.ssh_output(RecordedCommand {
        label: command,
        script: &asked,
      }) {
        Ok(Some(output)) => {
          let ssh_text = self.ssh_text(&output.stdout, "stdout")?;

          ssh_text.trim().to_owned()
        }
        Ok(None) => String::new(),
        Err(fault) => {
          console_core_logging::warn!("console-test-stages: ssh {}: {fault}", self.host);
          String::new()
        }
      },
    )
  }

  fn ssh_output(
    &mut self,
    recorded: RecordedCommand<'_>,
  ) -> std::io::Result<Option<std::process::Output>> {
    self.done.push(recorded.label.to_owned());

    match self.dry_run {
      DryRun::Pretend => return Ok(None),
      DryRun::Really => {}
    }

    let Ok(mut asking) = Program::Ssh.command();
    let output = asking
      .args(["-o", "BatchMode=yes", "--", &self.host, recorded.script])
      .output()?;
    let Ok(complained) = self.ssh_text(&output.stderr, "stderr");
    let Ok(links) = logins(complained);

    for link in links {
      eprintln!(
        "\nconsole-test-stages: {} will not answer until this is approved in a browser:\n\n    {link}\n",
        self.host
      );
    }

    Ok(Some(output))
  }

  pub fn ssh_args(&mut self, arguments: &[&str]) -> Result<String, Error> {
    let Ok(command) = shell::join(arguments);
    let Ok(along) = self.along();
    let asked = match along {
      Some(progress) => {
        let Ok(console) = InternalProgram::Console.path();
        let amount = progress.permille.to_string();
        let Ok(wrapped) = shell::join(
          [console, "device-progress", "then", &amount, &progress.label]
            .into_iter()
            .chain(arguments.iter().copied()),
        );
        wrapped
      }
      None => command.clone(),
    };
    let remote = self
      .ssh_output(RecordedCommand {
        label: &command,
        script: &asked,
      })
      .map_err(Error::Machine)?;

    let output = match remote {
      Some(matched_output) => matched_output,
      ::core::option::Option::None => return Ok(String::new()),
    };

    let Ok(stdout) = self.ssh_text(&output.stdout, "stdout");
    let Ok(stderr) = self.ssh_text(&output.stderr, "stderr");

    match ::core::primitive::bool::then_some(output.status.success(), ()) {
      ::core::option::Option::Some(()) => Ok(stdout.trim().to_owned()),
      ::core::option::Option::None => Err(Error::RemoteCommand {
        host: self.host.clone(),
        command: match arguments.first().copied() {
          ::core::option::Option::Some(present) => present,
          ::core::option::Option::None => "",
        }
        .to_owned(),
        status: output.status,
        stderr: stderr.trim().to_owned(),
      }),
    }
  }

  fn discovery_output(&mut self, arguments: &[&str]) -> Result<String, Never> {
    Ok(match self.ssh_args(arguments) {
      Ok(output) => output,
      Err(fault) => {
        console_core_logging::debug!("discovering the account on {}: {fault}", self.host);
        String::new()
      }
    })
  }

  pub fn inspect(&mut self, arguments: &[&str]) -> Result<String, Error> {
    let Ok(console) = InternalProgram::Console.path();
    let mut words = vec![console, "device-inspect"];
    words.extend_from_slice(arguments);

    self.ssh_args(&words)
  }

  fn inspect_as_user(&mut self, arguments: &[&str]) -> Result<String, Error> {
    let Ok(whom) = self.whoever();
    let Ok(console) = InternalProgram::Console.path();
    let mut command = vec![console, "device-inspect"];
    command.extend_from_slice(arguments);
    let Ok(words) = reaching::user_words(Whom(&whom), &command);

    self.ssh_args(&words)
  }

  fn ssh_text<'text>(&self, bytes: &'text [u8], stream: &str) -> Result<&'text str, Never> {
    Ok(match std::str::from_utf8(bytes) {
      Ok(text) => text,
      Err(fault) => {
        console_core_logging::warn!(
          "console-test-stages: ssh {} returned invalid UTF-8 on {stream}: {fault}",
          self.host
        );

        ""
      }
    })
  }

  pub fn in_session(&mut self, arguments: &[&str]) -> Result<String, Error> {
    let Ok(whom) = self.whoever();
    let Ok(command) = reaching::session_words(None, arguments);
    let Ok(words) = reaching::user_words(Whom(&whom), &command);

    self.ssh_args(&words)
  }

  fn session_output(&mut self, arguments: &[&str]) -> Result<String, Never> {
    Ok(match self.in_session(arguments) {
      Ok(said) => said,
      Err(fault) => {
        console_core_logging::warn!("console-test-stages: {fault}");
        String::new()
      }
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit082_no_built_shell_line",
    expect(
      explicit082_no_built_shell_line,
      reason = "test scenarios explicitly supply a shell program; the SSH transport quotes it as one word and forwards its text verbatim"
    )
  )]
  pub fn user(&mut self, command: &str) -> Result<String, Never> {
    let Ok(whom) = self.whoever();
    let owner = format!("--uid={whom}");
    let Ok(asked) = shell::join([
      "machinectl",
      "shell",
      &owner,
      ".host",
      "/bin/sh",
      "-c",
      command,
    ]);

    self.ssh(&asked)
  }

  pub fn read_descriptions(
    &mut self,
    at: &str,
  ) -> Result<Vec<console_panel::description::Description>, Never> {
    let Ok(console) = InternalProgram::Console.path();
    let Ok(said) = self.session_output(&[console, "device-runtime-file", "read", at]);

    Ok(
      said
        .lines()
        .filter_map(|line| match console_panel::description::read(line) {
          Ok(drew) => Some(drew),
          Err(_not_a_draw) => None,
        })
        .collect(),
    )
  }

  fn compositor_command(&mut self, arguments: &[&str]) -> Result<String, Never> {
    let Ok(program) = console_compositor::session::control_name();
    let mut words = vec![program];
    words.extend_from_slice(arguments);

    self.session_output(&words)
  }

  pub fn compositor_text(&mut self, query: Query) -> Result<String, Never> {
    let Ok(arguments) = query.words();

    self.compositor_command(arguments)
  }

  fn dispatch(&mut self, lua: &str) -> Result<String, Never> {
    let Ok(verb) = Request::Dispatch.word();

    self.compositor_command(&[verb, lua])
  }

  fn loaded(&mut self) -> Result<Option<Profile>, Never> {
    let Ok(profile) = self.profile();
    let loaded = profile.to_lowercase();

    Ok(
      self
        .profiles
        .values()
        .find(|known| known.name.to_lowercase() == loaded)
        .cloned(),
    )
  }

  fn capability(&mut self, button: &str) -> Result<Option<String>, Never> {
    let Ok(loaded) = self.loaded();

    capability_under(loaded.as_ref(), button)
  }

  fn chorded(&mut self, capabilities: &[String]) -> Result<String, Never> {
    self.taken = None;

    let Ok(words) = calling(capabilities);
    let Ok(asked) = shell::join(&words);

    self.ssh(&format!("{asked} 2>&1"))
  }

  pub fn press(&mut self, button: &str) -> Result<(), Never> {
    let Ok(found) = self.capability(button);

    let capability = match found {
      Some(capability) => capability,
      None => return Ok(()),
    };

    let Ok(_) = self.chorded(&[capability]);

    Ok(())
  }

  pub fn presses(&mut self, button: &str, times: u32) -> Result<(), Error> {
    self.taken = None;

    let Ok(found) = self.capability(button);

    let capability = match found {
      Some(matched_capability) => matched_capability,
      ::core::option::Option::None => return Ok(()),
    };

    let count = times.to_string();
    let Ok(console) = InternalProgram::Console.path();
    self.ssh_args(&[console, "device-repeat-chord", &count, &capability])?;

    Ok(())
  }

  pub fn hold(&mut self, _button: &str) -> CheckResult {
    cannot(SENDEVENT)
  }

  pub fn release(&mut self, _button: Option<&str>) -> CheckResult {
    cannot(SENDEVENT)
  }

  pub fn trigger(&mut self, which: &str, TriggerTravel(amount): TriggerTravel) -> CheckResult {
    let Ok(found) = lookup(&vocabulary::TRIGGERS, which);

    let named = match found {
      Some(named) => named,
      None => {
        let Ok(said) = suggestions(&vocabulary::TRIGGERS);

        return failed(format!("there is no trigger called {which:?}; {said}"));
      }
    };

    match ::core::primitive::bool::then_some((0.0..=1.0).contains(&amount), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        return failed(format!("a trigger is pulled between 0 and 1, not {amount}"));
      }
    }

    let Ok(()) = self.pushed(Pushed::from(amount));

    self.axis(
      &format!("Gamepad:Trigger:{named}"),
      Value(&format!("d {amount:.3}")),
    )
  }

  pub fn thumbstick(&mut self, which: &str, to: Point<f64>) -> CheckResult {
    let Ok(found) = lookup(&vocabulary::AXES, which);

    let named = match found {
      Some(named) => named,
      None => {
        let Ok(said) = suggestions(&vocabulary::AXES);

        return failed(format!("there is no thumbstick called {which:?}; {said}"));
      }
    };

    for amount in [to.x, to.y] {
      match ::core::primitive::bool::then_some((-1.0..=1.0).contains(&amount), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          return failed(format!("push a thumbstick between -1 and 1, not {amount}"));
        }
      }
    }

    let Ok(()) = self.pushed(Pushed::from(to.x.abs().max(to.y.abs())));

    let (across, down) = (to.x, to.y);

    self.axis(
      &format!("Gamepad:Axis:{named}"),
      Value(&format!("\"(dd)\" {across:.3} {down:.3}")),
    )
  }

  pub fn let_go(&mut self) -> CheckResult {
    self.pushed = Pushed::None;

    for (spoken, _) in vocabulary::AXES {
      self.thumbstick(spoken, Point { x: 0.0, y: 0.0 })?;
    }

    for (spoken, _) in vocabulary::TRIGGERS {
      self.trigger(spoken, TriggerTravel(0.0))?;
    }

    Ok(())
  }

  fn pushed(&mut self, outcome: Pushed) -> Result<(), Never> {
    self.taken = None;

    match outcome {
      Pushed::None => {}
      Pushed::Some => self.pushed = Pushed::Some,
    }

    Ok(())
  }

  fn axis(&mut self, capability: &str, value: Value<'_>) -> CheckResult {
    let Ok(quoted) = shell_quote(capability);
    let Ok(words) = front::event_arguments();
    let Ok(call) = shell::join(words);
    let asked = format!("{call} {quoted} {} 2>&1", value.0);
    let Ok(said) = self.ssh(&asked);

    match ::core::primitive::bool::then_some(said.contains("Call failed"), ()) {
      ::core::option::Option::Some(()) => {
        failed(format!("{capability} was not sent: {}", said.trim()))
      }
      ::core::option::Option::None => Ok(()),
    }
  }

  pub fn tap(&mut self, _at: Point<i32>) -> CheckResult {
    cannot(
      "InputPlumber will not parse its own Touchpad: capabilities back, \
             so the trackpad cannot be sent; point or touch instead",
    )
  }

  pub fn touch(&mut self, at: (u32, u32)) -> CheckResult {
    self.tapped(&[&at.0.to_string(), &at.1.to_string()])
  }

  fn tapped(&mut self, arguments: &[&str]) -> CheckResult {
    self.taken = None;

    let Ok(whom) = self.whoever();
    let Ok(tap) = InternalProgram::Tap.name();
    let mut command = vec![tap];
    command.extend_from_slice(arguments);
    let Ok(words) = reaching::session_words(Some(Whom(&whom)), &command);
    self.ssh_args(&words)?;

    Ok(())
  }

  pub fn swipe(&mut self, from: (u32, u32), to: (u32, u32)) -> CheckResult {
    self.tapped(&[
      "--swipe",
      &from.0.to_string(),
      &from.1.to_string(),
      &to.0.to_string(),
      &to.1.to_string(),
    ])
  }

  pub fn point(&mut self, at: (u32, u32)) -> CheckResult {
    self.pointing(None, at, &[])
  }

  pub fn click(&mut self, at: (u32, u32)) -> CheckResult {
    self.pointing(None, at, &["--click"])
  }

  pub fn scroll(&mut self, at: (u32, u32), notches: i32) -> CheckResult {
    self.pointing(None, at, &["--scroll", &notches.to_string()])
  }

  pub fn point_in(&mut self, namespace: &str, at: (u32, u32)) -> CheckResult {
    self.pointing(Some(namespace), at, &[])
  }

  pub fn click_in(&mut self, namespace: &str, at: (u32, u32)) -> CheckResult {
    self.pointing(Some(namespace), at, &["--click"])
  }

  pub fn scroll_in(&mut self, namespace: &str, at: (u32, u32), notches: i32) -> CheckResult {
    self.pointing(Some(namespace), at, &["--scroll", &notches.to_string()])
  }

  fn pointing(&mut self, inside: Option<&str>, at: (u32, u32), doing: &[&str]) -> CheckResult {
    self.taken = None;

    let x = at.0.to_string();
    let y = at.1.to_string();
    let Ok(point) = InternalProgram::Point.name();
    let mut words = vec![point];

    match inside {
      Some(namespace) => words.extend(["--in", namespace]),
      None => {}
    }

    words.extend([x.as_str(), y.as_str()]);
    words.extend_from_slice(doing);

    match self.in_session(&words) {
      Ok(_) => Ok(()),
      Err(fault @ Error::RemoteCommand { status, .. }) => match status.code() {
        Some(97) => cannot("this device has no console-point; deploy before pointing at it"),
        Some(_) | None => failed(format!("the pointer could not be put at {at:?}: {fault}")),
      },
      Err(fault) => failed(format!("the pointer could not be put at {at:?}: {fault}")),
    }
  }

  pub fn home_awake(&mut self) -> Result<Ready, Never> {
    let Ok(said) = self.user("test -e \"$XDG_RUNTIME_DIR/console/home-awake\" && echo awake");

    Ok(
      match ::core::primitive::bool::then_some(said.contains("awake"), ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      },
    )
  }

  pub fn home_carrying(&mut self) -> Result<Ready, Never> {
    let Ok(said) =
      self.user("test -e \"$XDG_RUNTIME_DIR/console/home-carrying\" && echo carrying");

    Ok(
      match ::core::primitive::bool::then_some(said.contains("carrying"), ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      },
    )
  }

  pub fn layer(&mut self, namespace: &str) -> Result<Option<(u32, u32, u32, u32)>, Never> {
    let Ok(said) = self.compositor_text(Query::Layers);
    let Ok(read) = parse_answer(console_compositor::Layers, &said);

    let surfaces = match read {
      Some(surfaces) => surfaces,
      None => return Ok(None),
    };

    for layer in &surfaces {
      match ::core::primitive::bool::then_some(layer.namespace == namespace, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => continue,
      }

      let Ok(said_where) = layer.corner();

      let corner = match said_where {
        Some(corner) => corner,
        None => return Ok(None),
      };

      let Ok(x) = fitted::<i64, u32>(corner.x);
      let Ok(y) = fitted::<i64, u32>(corner.y);
      let Ok(wide) = fitted::<i64, u32>(corner.width);
      let Ok(tall) = fitted::<i64, u32>(corner.height);

      return Ok(Some((x, y, wide, tall)));
    }

    Ok(None)
  }

  pub fn load_profile(&mut self, name: &str) -> Result<(), Never> {
    let Ok(quoted) = shell_quote(name);
    let Ok(_) = self.user(&format!("controller-profile {quoted}"));

    Ok(())
  }

  pub fn exec_cmd(&mut self, command: &str) -> Result<String, Never> {
    let Ok(lua) =
      console_compositor::lua::exec(command, console_compositor::lua::Exec::Dispatcher);
    self.dispatch(&lua)
  }

  pub fn addresses(&mut self) -> Result<Vec<String>, Never> {
    let Ok(clients) = self.clients();

    Ok(
      clients
        .iter()
        .filter_map(|client| {
          let Ok(address) = address(client);

          address
        })
        .collect(),
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit028_no_search_in_a_loop",
    allow(
      explicit028_no_search_in_a_loop,
      reason = "the windows a check opened against the windows the compositor says are up, which is tens of either"
    )
  )]
  pub fn opened(&mut self) -> Result<Vec<String>, Never> {
    let Ok(open) = self.addresses();

    Ok(
      self
        .opened
        .iter()
        .filter(|which| open.contains(which))
        .cloned()
        .collect(),
    )
  }

  pub fn window_gone(&mut self, which: &str, seconds: impl Into<Wait>) -> Result<Outcome, Never> {
    let going = which.to_string();

    self.until::<Never>(
      |seen| {
        let Ok(open) = seen.addresses();

        Ok(
          match ::core::primitive::bool::then_some(open.contains(&going), ()) {
            ::core::option::Option::Some(()) => Ready::NotYet,
            ::core::option::Option::None => Ready::Yes,
          },
        )
      },
      seconds,
    )
  }

  fn pid_at(&mut self, which: &str) -> Result<Option<i64>, Never> {
    let Ok(clients) = self.clients();

    Ok(
      clients
        .iter()
        .find(|client| {
          let Ok(found) = address(client);

          found.as_deref() == Some(which)
        })
        .and_then(|client| client.get("pid"))
        .and_then(serde_json::Value::as_i64),
    )
  }

  pub fn close_window(&mut self, which: &str) -> Result<Outcome, Never> {
    let closing = which.to_string();

    self.opened.retain(|address| *address != closing);

    match self.dry_run {
      DryRun::Pretend => return Ok(Outcome::Happened),
      DryRun::Really => {}
    }

    let Ok(pid) = self.pid_at(&closing);

    let pid_3 = match pid {
      Some(pid_2) => pid_2,
      None => return Ok(Outcome::Happened),
    };

    let Ok(_) = self.user(&format!("kill {pid}", pid = pid_3));

    self.window_gone(&closing, LEAVING)
  }

  pub fn go_to(&mut self, workspace: &str) -> Result<Outcome, Never> {
    let wanted = workspace.to_string();
    let Ok(lua) = console_compositor::onto(&wanted, console_compositor::Carrying::None);
    let Ok(_) = self.dispatch(&lua);

    self.until::<Never>(
      |seen| {
        let Ok(now) = seen.workspace();

        Ok(
          match ::core::primitive::bool::then_some(now == wanted, ()) {
            ::core::option::Option::Some(()) => Ready::Yes,
            ::core::option::Option::None => Ready::NotYet,
          },
        )
      },
      A_MOMENT,
    )
  }

  pub fn to_an_empty_workspace(&mut self) -> Result<Outcome, Never> {
    let Ok(lua) = console_compositor::onto(EMPTY_WORKSPACE, console_compositor::Carrying::None);
    let Ok(_) = self.dispatch(&lua);

    self.until::<Never>(
      |seen| {
        let Ok(held) = seen.windows_here();

        Ok(match held {
          NO_WINDOWS => Ready::Yes,
          _ => Ready::NotYet,
        })
      },
      A_MOMENT,
    )
  }

  pub fn open(&mut self, command: &str, seconds: impl Into<Wait>) -> Result<Outcome, Never> {
    let Ok(which) = self.opening(command, seconds);

    Ok(match which {
      Some(_) => Outcome::Happened,
      None => Outcome::RanOut,
    })
  }

  pub fn opening(
    &mut self,
    command: &str,
    seconds: impl Into<Wait>,
  ) -> Result<Option<String>, Never> {
    match self.dry_run {
      DryRun::Pretend => {
        let Ok(_) = self.exec_cmd(command);

        return Ok(Some(String::new()));
      }
      DryRun::Really => {}
    }

    let Ok(was) = self.addresses();
    let Ok(_) = self.exec_cmd(command);
    let mut came = Came::default();
    let Ok(arrived) = self.until_handed::<Came, Never>(
      &mut came,
      |came_2, seen| {
        seen.taken = None;

        let Ok(now) = seen.clients();
        let new = now.iter().find(|client| {
          let Ok(address) = address(client);

          address.is_some_and(|found| !was.contains(&found))
        });

        let new_3 = match new {
          Some(new_2) => new_2,
          None => return Ok(Ready::NotYet),
        };

        let Ok(found) = address(new_3);

        came_2.which = match found {
          Some(found_2) => found_2,
          None => String::new(),
        };

        came_2.where_ = match new_3
          .get("workspace")
          .and_then(|workspace| workspace.get("name"))
          .and_then(|name| name.as_str())
        {
          Some(named) => named.to_string(),
          None => String::new(),
        };

        Ok(Ready::Yes)
      },
      seconds,
    );

    match arrived {
      Outcome::Happened => {}
      Outcome::RanOut => return Ok(None),
    }

    self.opened.push(came.which.clone());

    let Ok(there) = self.go_to(&came.where_);

    Ok(match there {
      Outcome::Happened => Some(came.which),
      Outcome::RanOut => None,
    })
  }

  pub fn settle(&mut self, seconds: impl Into<Wait>) -> Result<(), Never> {
    match self.dry_run {
      DryRun::Pretend => {}
      DryRun::Really => {
        let given: Wait = <_ as ::core::convert::Into<crate::device::Wait>>::into(seconds);

        let span = match given {
          Wait::For(span) => span,
          Wait::TooLong | Wait::Invalid => {
            console_core_logging::warn!("console-test-stages: invalid settle duration");
            return Ok(());
          }
        };

        #[cfg_attr(
          dylint_lib = "explicit021_no_sleeping",
          allow(
            explicit021_no_sleeping,
            reason = "this is the gap `until` asks between two questions to the handheld, and a check that calls it directly is waiting on a number rather than on the thing -- which is what EXPLICIT022 denies, at the call, where the decision is"
          )
        )]
        std::thread::sleep(span);
      }
    }

    Ok(())
  }

  pub fn workspace(&mut self) -> Result<String, Never> {
    let Ok(said) = self.compositor_text(Query::ActiveWorkspace);
    let Ok(found) = read(&said);

    let found_3 = match found {
      Some(found_2) => found_2,
      None => return Ok(String::new()),
    };

    let Ok(named) = console_compositor::workspace(&found_3);

    Ok(match named {
      Some(named_2) => named_2.to_string(),
      None => String::new(),
    })
  }

  fn clients(&mut self) -> Result<Vec<serde_json::Value>, Never> {
    let Ok(said) = self.compositor_text(Query::Clients);
    let Ok(found) = read(&said);

    let found_3 = match found {
      Some(found_2) => found_2,
      None => return Ok(Vec::new()),
    };

    let Ok(clients) = console_compositor::clients(&found_3);

    Ok(clients.cloned().collect())
  }

  pub fn windows_open(&mut self) -> Result<Vec<console_compositor::Window>, Never> {
    let Ok(said) = self.compositor_text(Query::Clients);
    let Ok(found) = read(&said);

    Ok(match found {
      Some(found_2) => {
        let Ok(open) = console_compositor::windows_open(&found_2);

        open
      }
      None => Vec::new(),
    })
  }

  pub fn windows(&mut self) -> Result<Vec<String>, Never> {
    let Ok(clients) = self.clients();
    let mut named: Vec<String> = clients
      .iter()
      .map(
        |client| match client.get("class").and_then(|class| class.as_str()) {
          Some(class) => class.to_string(),
          None => String::new(),
        },
      )
      .collect();

    named.sort();

    Ok(named)
  }

  pub fn titles(&mut self) -> Result<Vec<String>, Never> {
    let Ok(clients) = self.clients();

    Ok(
      clients
        .iter()
        .map(
          |client| match client.get("title").and_then(|title| title.as_str()) {
            Some(title) => title.to_string(),
            None => String::new(),
          },
        )
        .collect(),
    )
  }

  pub fn types(&mut self, words: &str) -> Result<String, Error> {
    let Ok(wtype) = Program::Wtype.name();

    self.in_session(&[wtype, "--", words])
  }

  pub fn press_chord(&mut self, held: &[&str], key: &str) -> CheckResult {
    let mut chord = Vec::new();

    for one in held.iter().chain(std::iter::once(&key)) {
      let Ok(found) = vocabulary::key_capability(one);

      match found {
        Some(capability) => chord.push(capability),
        None => return failed(format!("there is no key called {one:?} to press")),
      }
    }

    let Ok(said) = self.chorded(&chord);

    match ::core::primitive::bool::then_some(said.contains("Call failed"), ()) {
      ::core::option::Option::Some(()) => {
        failed(format!("{chord:?} was not sent: {}", said.trim()))
      }
      ::core::option::Option::None => Ok(()),
    }
  }

  pub fn windows_here(&mut self) -> Result<i64, Never> {
    let Ok(said) = self.compositor_text(Query::ActiveWorkspace);
    let Ok(found) = read(&said);

    let found_3 = match found {
      Some(found_2) => found_2,
      None => return Ok(0),
    };

    let Ok(held) = console_compositor::windows(&found_3);

    Ok(match held {
      Some(held_2) => held_2,
      None => NO_WINDOWS,
    })
  }

  pub fn keyboard(&mut self) -> Result<Ready, Never> {
    let Ok(said) = self.compositor_text(Query::Layers);

    Ok(
      match ::core::primitive::bool::then_some(said.contains("console-keyboard"), ()) {
        ::core::option::Option::Some(()) => Ready::Yes,
        ::core::option::Option::None => Ready::NotYet,
      },
    )
  }

  pub fn profile(&mut self) -> Result<String, Never> {
    match self.dry_run {
      DryRun::Pretend => return Ok(String::new()),
      DryRun::Really => {}
    }

    let Ok(words) = front::profile_name_command();
    let Ok(asked) = shell::join(words);
    let Ok(said) = self.ssh(&asked);
    let Ok(name) = front::quoted_string(&said);

    Ok(
      match name {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => "",
      }
      .to_owned(),
    )
  }

  pub fn brightness(&mut self) -> Result<Level, Never> {
    let Ok(said) = self.ssh("cat /sys/class/backlight/*/brightness");

    brightness_in(&said)
  }

  pub fn brightness_to(&mut self, level: i64) -> Result<(), Never> {
    let Ok(_) = self.ssh(&format!(
      "printf '%s\\n' {level} | tee /sys/class/backlight/*/brightness >/dev/null"
    ));

    Ok(())
  }

  pub fn volume(&mut self) -> Result<Level, Never> {
    let Ok(said) = self.user("pactl get-sink-volume @DEFAULT_SINK@");

    volume_in(&said)
  }

  pub fn volume_to(&mut self, level: i64) -> Result<(), Never> {
    let Ok(_) = self.user(&format!("pactl set-sink-volume @DEFAULT_SINK@ {level}%"));

    Ok(())
  }

  pub fn services(&mut self) -> Result<Vec<String>, Never> {
    let asking: Vec<&str> = PIECES
      .into_iter()
      .filter(|piece| !BY_A_SWITCH.contains(piece))
      .collect();
    let Ok(said) = self.user(&format!("systemctl --user is-active {}", asking.join(" ")));

    Ok(said.split_whitespace().map(str::to_string).collect())
  }

  pub fn restarts(&mut self) -> Result<Vec<String>, Never> {
    let Ok(said) = self.user(&format!(
      "systemctl --user show --value -p NRestarts {}",
      PIECES.join(" ")
    ));

    Ok(said.split_whitespace().map(str::to_string).collect())
  }

  pub fn icon_theme(&mut self) -> Result<String, Never> {
    self.user("sed -n 's/^gtk-icon-theme-name=//p' ~/.config/gtk-4.0/settings.ini | head -1")
  }

  pub fn icons_missing(&mut self, theme: &str, names: &[&str]) -> Result<Vec<String>, Error> {
    let mut arguments = vec!["icons-missing", theme];
    arguments.extend_from_slice(names);
    let said = self.inspect_as_user(&arguments)?;

    Ok(said.lines().map(str::to_owned).collect())
  }

  pub fn files(&mut self, where_: &str) -> Result<Vec<String>, Never> {
    let Ok(quoted) = shell_quote(where_);
    let Ok(said) = self.user(&format!("ls -1 {quoted} 2>/dev/null"));
    let mut found: Vec<String> = said
      .lines()
      .filter(|line| !line.is_empty())
      .map(str::to_string)
      .collect();

    found.sort();

    Ok(found)
  }

  pub fn journal(&mut self, unit: &str, lines: u32) -> Result<String, Never> {
    self.user(&format!(
      "journalctl --user -u {unit} -n {lines} --no-pager"
    ))
  }

  pub fn menus(&mut self) -> Result<Vec<String>, Never> {
    let Ok(said) = self.compositor_text(Query::Layers);
    let Ok(read) = read(&said);

    let found = match read {
      Some(found) => found,
      None => return Ok(Vec::new()),
    };

    let mut named = Vec::new();

    let surfaces = match console_compositor::said_of(console_compositor::Layers, found) {
      Ok(surfaces) => surfaces,
      Err(_unreadable) => Vec::new(),
    };

    for layer in &surfaces {
      let namespace = layer.namespace.as_str();

      match ::core::primitive::bool::then_some(
        console_input_controller::mode::SYSTEM_SURFACES.contains(&namespace)
          || namespace == WALLPAPER_SURFACE,
        (),
      ) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => named.push(namespace.to_string()),
      }
    }

    named.sort();

    Ok(named)
  }

  pub fn until<Why>(
    &mut self,
    mut what: impl FnMut(&mut Self) -> Result<Ready, Why>,
    seconds: impl Into<Wait>,
  ) -> Result<Outcome, Why> {
    self.until_handed(&mut what, |what_2, seen| what_2(seen), seconds)
  }

  pub fn until_handed<M, Why>(
    &mut self,
    handed: &mut M,
    mut what: impl FnMut(&mut M, &mut Self) -> Result<Ready, Why>,
    seconds: impl Into<Wait>,
  ) -> Result<Outcome, Why> {
    let given: Wait = <_ as ::core::convert::Into<crate::device::Wait>>::into(seconds);
    let Ok(patience) = given.limit();
    let dry_run = self.dry_run;
    let mut inputs = (handed, self, &mut what);

    match dry_run {
      DryRun::Pretend => {
        let mut clock = console_waiting::clock::TestClock::default();

        crate::completion::wait(
          &mut clock,
          &mut inputs,
          patience,
          |(handed_2, seen, ask)| ask(handed_2, seen),
          crate::stopping::stop_state,
        )
      }
      DryRun::Really => {
        let Ok(mut clock) = console_waiting::clock::LiveClock::started();

        crate::completion::wait(
          &mut clock,
          &mut inputs,
          patience,
          |(handed_3, seen, ask)| ask(handed_3, seen),
          crate::stopping::stop_state,
        )
      }
    }
  }

  pub fn press_and_wait(
    &mut self,
    button: &str,
    reading: fn(&mut Self) -> Result<Level, Never>,
  ) -> Result<Level, Never> {
    let Ok(was) = reading(self);
    let Ok(()) = self.press(button);
    let Ok(_) = self.changed(reading, &was, PATIENCE);

    Ok(was)
  }

  pub fn changed<T: PartialEq, Why>(
    &mut self,
    mut reading: impl FnMut(&mut Self) -> Result<T, Why>,
    from: &T,
    seconds: impl Into<Wait>,
  ) -> Result<Outcome, Why> {
    self.until_handed(
      &mut reading,
      |reading_2, seen| {
        let now = reading_2(seen)?;

        Ok(match ::core::primitive::bool::then_some(now == *from, ()) {
          ::core::option::Option::Some(()) => Ready::NotYet,
          ::core::option::Option::None => Ready::Yes,
        })
      },
      seconds,
    )
  }

  pub fn wait_for_menu(&mut self, seconds: impl Into<Wait>) -> Result<Outcome, Never> {
    self.until(menus_up, seconds)
  }

  pub fn wait_for_close(&mut self, seconds: impl Into<Wait>) -> Result<Outcome, Never> {
    self.until(
      |seen| {
        let Ok(up) = menus_up(seen);

        up.flipped()
      },
      seconds,
    )
  }

  pub fn wallpaper(&mut self) -> Result<String, Error> {
    let Ok(paper) = InternalProgram::Paper.path();

    self.in_session(&[paper, "query"])
  }

  fn fetch_picture(&mut self, shot: &std::path::Path) -> Result<(), Never> {
    let Ok(mut fetching) = Program::Scp.command();
    let fetched = fetching
      .args(["-q", &format!("{}:/tmp/console-check.png", self.host)])
      .arg(shot)
      .status();
    let Ok(_) = self.ssh("rm -f /tmp/console-check.png");

    match fetched {
      Ok(status) => match bool::then_some(status.success(), ()) {
        Some(()) => {}
        None => {
          console_core_logging::warn!("console-test-stages: scp {}: {status}", self.host);
        }
      },
      Err(fault) => {
        console_core_logging::warn!("console-test-stages: scp {}: {fault}", self.host);
      }
    }

    Ok(())
  }

  fn picture(&mut self) -> Result<&Picture, Error> {
    match ::core::primitive::bool::then_some(self.taken.is_none(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(_) = self.exec_cmd("grim /tmp/console-check.png");
        let Ok(written) = self.until::<Never>(
          |seen| {
            let Ok(said) =
              seen.ssh("test -s /tmp/console-check.png && echo written || echo not-yet");

            Ok(
              match ::core::primitive::bool::then_some(said.trim() == "written", ()) {
                ::core::option::Option::Some(()) => Ready::Yes,
                ::core::option::Option::None => Ready::NotYet,
              },
            )
          },
          A_PICTURE,
        );

        match written {
          Outcome::Happened => {}
          Outcome::RanOut => return Err(Error::DeviceWroteNoPicture),
        }

        let here = console_core_temporary_directories::fresh("shot").map_err(Error::Scratch)?;
        let shot = here.join("screen.png");
        let Ok(()) = self.fetch_picture(&shot);

        let picture = match Picture::read(&shot) {
          Ok(picture) => picture,
          Err(fault) => {
            let Ok(()) = crate::remove_scratch(&here);

            return Err(fault);
          }
        };

        self.taken = Some(picture);
        self.kept = Some(here);
      }
      ::core::option::Option::None => {}
    }

    self.taken.as_ref().ok_or(Error::DevicePictureGone)
  }

  pub fn background(&mut self) -> Result<String, Error> {
    let picture = self.picture()?;
    let Ok(most_common) = picture.most_common();

    Ok(most_common)
  }

  pub fn color(&mut self, at: Point<f64>) -> Result<String, Error> {
    let screen = self.current_screen()?;
    let Ok(logical) = screen.logical();
    let picture = self.picture()?;

    where_(picture, at, logical)
  }

  fn current_screen(&mut self) -> Result<console_screen::Screen, Error> {
    match self.screen {
      Some(known) => Ok(known),
      None => {
        let Ok(shown) = self.shown();

        let screen = match shown {
          Some(screen) => screen,
          None => crate::screen()?,
        };

        self.screen = Some(screen);

        Ok(screen)
      }
    }
  }

  pub fn standing(&mut self) -> Result<console_screen::Shape, Never> {
    let Ok(shown) = self.shown();

    Ok(match shown {
      Some(screen) => {
        let Ok(shape) = screen.shape();

        shape
      }
      None => console_screen::Shape::Wider,
    })
  }

  fn shown(&mut self) -> Result<Option<console_screen::Screen>, Never> {
    let Ok(said) = self.compositor_text(Query::Monitors);
    let Ok(read) = read(&said);

    let found = match read {
      Some(found) => found,
      None => return Ok(None),
    };

    let Ok(monitors) = console_compositor::monitors(&found);

    console_screen::shown(&monitors)
  }

  pub fn patch(&mut self, at: Point<f64>) -> Result<String, Error> {
    let picture = self.picture()?;
    let Ok(average) = picture.average(at, crate::picture::PATCH);

    Ok(average)
  }

  pub fn again(&mut self) -> Result<(), Never> {
    self.taken = None;

    Ok(())
  }

  pub fn fresh(&mut self) -> Result<(), Never> {
    self.taken = None;
    self.screen = None;

    match self.dry_run {
      DryRun::Pretend => return Ok(()),
      DryRun::Really => {}
    }

    match self.pushed {
      Pushed::None => {}
      Pushed::Some => match self.let_go() {
        Ok(()) => {}
        Err(why) => {
          eprintln!("console-test-stages: the pad would not let go: {why:?}");
        }
      },
    }

    for _ in 0..3 {
      let Ok(menus) = self.menus();

      match ::core::primitive::bool::then_some(menus.is_empty(), ()) {
        ::core::option::Option::Some(()) => break,
        ::core::option::Option::None => {}
      }

      let Ok(()) = self.press("b");
      let Ok(_closed) = self.wait_for_close(A_MOMENT);
    }

    let Ok(profile) = self.profile();

    match ::core::primitive::bool::then_some(profile == "Router", ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let Ok(()) = self.load_profile(console_input_gamepad::router::NAME);
        let Ok(loaded) = self.until::<Never>(
          |seen| {
            let Ok(now) = seen.profile();

            Ok(
              match ::core::primitive::bool::then_some(now == "Router", ()) {
                ::core::option::Option::Some(()) => Ready::Yes,
                ::core::option::Option::None => Ready::NotYet,
              },
            )
          },
          A_MOMENT,
        );

        match loaded {
          Outcome::Happened => {}
          Outcome::RanOut => {
            eprintln!("console-test-stages: the router profile would not load");
          }
        }
      }
    }

    Ok(())
  }

  pub fn close(&mut self) -> Result<(), Never> {
    match self.kept.take() {
      Some(here) => {
        let Ok(()) = crate::remove_scratch(&here);
      }
      None => {}
    }

    Ok(())
  }
}

pub fn capability_under(here: Option<&Profile>, button: &str) -> Result<Option<String>, Never> {
  let itself = match vocabulary::button_name(button) {
    Ok(name) => Some(format!("Gamepad:Button:{name}")),
    Err(_unnamed) => None,
  };

  let here_3 = match here {
    Some(here_2) => here_2,
    None => return Ok(itself),
  };

  let named = match here_3.for_button(button) {
    Ok(named) => named,
    Err(_no_such_button) => return Ok(itself),
  };

  let sent = named
    .iter()
    .flat_map(|mapping| &mapping.targets)
    .find_map(|target| {
      let Ok(spoken) = spoken_as(target.kind, &target.name);

      spoken
    });

  Ok(match (sent, named.is_empty()) {
    (Some(sent_2), _) => Some(sent_2),
    (None, false) => None,
    (None, true) => itself,
  })
}

fn address(client: &serde_json::Value) -> Result<Option<String>, Never> {
  Ok(
    client
      .get("address")
      .and_then(|address| address.as_str())
      .map(str::to_string),
  )
}

fn parse_answer<Q: console_compositor::Question>(
  question: Q,
  said: &str,
) -> Result<Option<Q::Reply>, Never> {
  Ok(match console_compositor::read(question, said) {
    Ok(answer) => Some(answer),
    Err(why) => {
      eprintln!(
        "console-test-stages: the device answered with something the compositor did not: {why}"
      );

      None
    }
  })
}

fn read(said: &str) -> Result<Option<serde_json::Value>, Never> {
  Ok(match console_compositor::read_value(said) {
    Ok(parsed) => Some(parsed),
    Err(why) => {
      eprintln!(
        "console-test-stages: the device answered with something the compositor did not: {why}"
      );

      None
    }
  })
}

fn calling(capabilities: &[String]) -> Result<Vec<String>, Never> {
  front::chord_arguments(capabilities)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  fn dry() -> Result<Device<'static>, crate::Error> {
    Device::new("root@handheld", DryRun::Pretend)
  }

  fn root() -> Result<std::path::PathBuf, Never> {
    let Ok(root) = crate::root();

    Ok(root)
  }

  #[test]
  fn the_pieces_asked_about_are_the_ones_the_manifest_enables()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(root) = root();
    let held = std::fs::read_to_string(root.join("desktop.conf"))?;

    let Ok(services) =
      console_core_ini_files::lines(&held, console_core_ini_files::Under("services"));

    let mut named: Vec<String> = services
      .into_iter()
      .filter(|line| line.ends_with(".service"))
      .map(|line| line.trim_end_matches(".service").to_string())
      .collect();

    let mut wanted: Vec<String> = PIECES.iter().map(|piece| piece.to_string()).collect();
    named.sort();
    wanted.sort();

    assert_eq!(
      named, wanted,
      "the desktop the manifest enables is not the desktop the checks ask about"
    );

    Ok(())
  }

  #[test]
  fn a_patience_on_the_handheld_checks_immediately_and_at_the_deadline()
  -> Result<(), Box<dyn Error>> {
    let mut stage = dry()?;
    let mut asks = 0_u32;
    let Ok(waited) = stage.until_handed::<u32, Never>(
      &mut asks,
      |asks_2, _seen| {
        *asks_2 = (*asks_2).saturating_add(1);

        Ok(Ready::NotYet)
      },
      PATIENCE,
    );

    assert_eq!(waited, Outcome::RanOut);
    assert_eq!(
      asks, 81,
      "one immediate question and one after each 50 ms gap, including the deadline"
    );

    Ok(())
  }

  #[test]
  fn a_thing_already_there_on_the_handheld_is_asked_about_once() -> Result<(), Box<dyn Error>> {
    let mut stage = dry()?;
    let mut asks = 0_u32;
    let Ok(waited) = stage.until_handed::<u32, Never>(
      &mut asks,
      |asks_2, _seen| {
        *asks_2 = (*asks_2).saturating_add(1);

        Ok(Ready::Yes)
      },
      PATIENCE,
    );

    assert_eq!(waited, Outcome::Happened);
    assert_eq!(asks, 1);

    Ok(())
  }

  #[test]
  fn an_immediate_completion_needs_no_patience() -> Result<(), Box<dyn Error>> {
    let mut stage = dry()?;
    let Ok(waited) = stage.until::<Never>(|_seen| Ok(Ready::Yes), 0.0);

    assert_eq!(waited, Outcome::Happened);

    Ok(())
  }

  #[test]
  fn a_word_with_a_quote_in_it_is_still_one_word() {
    assert_eq!(super::shell_quote("plain"), Ok("'plain'".to_string()));
    assert_eq!(super::shell_quote("it's"), Ok(r"'it'\''s'".to_string()));
  }

  #[test]
  fn a_chord_says_how_many_it_is_before_it_says_what_they_are() {
    assert_eq!(
      super::calling(&["Gamepad:Button:South".to_string()]).as_deref(),
      Ok(
        [
          "busctl",
          "--system",
          "call",
          front::BUS,
          front::OBJECT,
          front::INTERFACE,
          "SendButtonChord",
          "as",
          "1",
          "Gamepad:Button:South",
        ]
        .map(str::to_owned)
        .as_slice()
      )
    );
  }

  #[test]
  fn a_key_under_a_modifier_is_the_modifier_and_then_the_key() -> Result<(), Box<dyn Error>> {
    let Ok(one) = vocabulary::key_capability("super");
    let Ok(other) = vocabulary::key_capability("i");
    let one_2 = one.ok_or("a modifier")?;
    let other_2 = other.ok_or("a letter")?;

    assert_eq!(
      super::calling(&[one_2, other_2]).as_deref(),
      Ok(
        [
          "busctl",
          "--system",
          "call",
          front::BUS,
          front::OBJECT,
          front::INTERFACE,
          "SendButtonChord",
          "as",
          "2",
          "Keyboard:KeyLeftMeta",
          "Keyboard:KeyI",
        ]
        .map(str::to_owned)
        .as_slice()
      )
    );

    Ok(())
  }

  #[test]
  fn a_key_nothing_names_is_answered_with_nothing_rather_than_a_guess() {
    assert_eq!(vocabulary::key_capability("logo"), Ok(None));
    assert_eq!(vocabulary::key_capability("slash"), Ok(None));
  }

  #[test]
  fn a_button_is_sent_as_what_the_loaded_profile_makes_of_it() -> Result<(), Box<dyn Error>> {
    let Ok(root) = root();
    let profiles = every_profile(&root)?;
    assert_eq!(
      capability_under(profiles.get("router"), "right-paddle-top"),
      Ok(Some("Keyboard:KeyF15".to_string()))
    );

    Ok(())
  }

  #[test]
  fn a_button_that_means_nothing_in_a_picker_is_still_sent() -> Result<(), Box<dyn Error>> {
    let Ok(root) = root();
    let profiles = every_profile(&root)?;
    assert_eq!(
      capability_under(profiles.get("router"), "view"),
      Ok(Some("Gamepad:Button:Select".to_string()))
    );

    Ok(())
  }

  #[test]
  fn a_button_a_profile_says_nothing_about_is_sent_as_itself() -> Result<(), Box<dyn Error>> {
    let Ok(root) = root();
    let profiles = every_profile(&root)?;
    assert_eq!(
      capability_under(profiles.get("game"), "a"),
      Ok(Some("Gamepad:Button:South".to_string()))
    );

    Ok(())
  }

  #[test]
  fn a_level_the_machine_would_not_say_is_not_a_level_of_nought() {
    assert_eq!(brightness_in("24000\n"), Ok(Level::At(24000)));
    assert_eq!(brightness_in(""), Ok(Level::Unsaid));
    assert_eq!(brightness_in("no such file"), Ok(Level::Unsaid));

    let said = "Volume: front-left: 26214 /  40% / -23.86 dB,   front-right: 26214 /  40%";

    assert_eq!(volume_in(said), Ok(Level::At(40)));
    assert_eq!(volume_in(""), Ok(Level::Unsaid));
    assert_eq!(volume_in("Volume: unknown"), Ok(Level::Unsaid));
  }

  #[test]
  fn a_name_past_fifteen_bytes_is_what_the_kernel_would_have_cut_it_to() {
    assert_eq!(comm("controller-desktop"), Ok("controller-desk"));
    assert_eq!(comm("controller-profile"), Ok("controller-prof"));
    assert_eq!(comm("console-panels"), Ok("console-panels"));
    assert_eq!(comm("Hyprland"), Ok("Hyprland"));
  }

  #[test]
  fn nothing_is_sent_on_a_dry_run() -> Result<(), Box<dyn Error>> {
    let mut device = dry()?;
    let Ok(()) = device.press("a");
    assert!(!device.done.is_empty(), "the command is still read");

    Ok(())
  }

  #[test]
  fn a_login_tailscale_asks_for_is_found_in_what_ssh_said() {
    let said = "# Tailscale SSH requires an additional check.\n# To authenticate, visit: https://login.tailscale.com/a/l1b811fa83a140e\nConnection to 100.109.165.76 port 22 timed out\n";

    assert_eq!(
      logins(said),
      Ok(vec!["https://login.tailscale.com/a/l1b811fa83a140e"])
    );
    assert_eq!(
      logins("ssh: connect to host the-handheld port 22: Connection timed out"),
      Ok(Vec::new())
    );
  }
}
