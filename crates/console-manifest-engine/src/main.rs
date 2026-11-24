//! This machine, brought to match /etc/console/desktop.conf.
//!
//!     console list      what the desktop is made of
//!     console check     where the machine has drifted from it
//!     console apply     bring the machine back to it
//!     console room      whether there is room on the disk for the next apply
//!     console save      take a file edited in place back into the source
//!
//! The manifest is the source of truth and this is only the engine that reads
//! it. Anything installed or enabled outside it is invisible here, which is the
//! point: a desktop assembled by hand is one no one can put back together.

mod alone;
mod boots;
mod build;
mod building;
mod buttons;
mod confirmation;
mod device_command;
mod enough;
mod generations;
mod going;
mod health;
mod install;
mod installing;
mod laying;
mod machine;
mod migrating;
mod migration_files;
mod packages;
mod program_staging;
mod pruning;
mod room;
mod screen;
mod snapshot;
mod units;
mod up_to_date;

use console_manifest_engine::{machines, manifest, modes, unapplied};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::install::User;
use building::Names;
use console_core_arguments::{
  Command, Flag, Operands, Presence, Reason, Subcommand, Takes, ValidationError, read_with,
};
use console_core_atomic_writes::Stored;
use console_core_external_programs::Program;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_words::Words;
use console_how_far::Progress;
use console_manifest_migrations::done::Outstanding;
use console_package_manager_pacman::{self as pacman, PacmanOutput};
use laying::{Deploy, Put};
use machine::Ran;
use manifest::{Manifest, Section};
use unapplied::Unapplied;
use up_to_date::UpToDate;

const ROOT: &str = console_repository::DEVICE_ROOT;

const DECLINED: &str = "exec-condition";

const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const OFF: &str = "\x1b[0m";

const COLUMN: u32 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Action {
  #[words(word = "list", about = "what the desktop is made of")]
  List,
  #[words(
    word = "check",
    about = "where the machine has drifted from it, which is what no word does"
  )]
  Check,
  #[words(word = "apply", about = "bring the machine back to it")]
  Apply,
  #[words(
    word = "room",
    about = "whether there is room on the disk for the next apply"
  )]
  Room,
  #[words(
    word = "buttons",
    about = "write the profiles again, with this device's buttons in them"
  )]
  Buttons,
  #[words(word = "health", about = "whether what the desktop runs is running")]
  Health,
  #[words(
    word = "save",
    about = "take the files at PATH, or every one edited in place, back into the source"
  )]
  Save,
  #[words(word = "migrate", about = "run what this machine has not run")]
  Migrate,
  #[words(
    word = "boot-started",
    about = "what a boot says about the newest generation as it starts"
  )]
  BootStarted,
  #[words(
    word = "boot-complete",
    about = "what a boot says about the newest generation once it is up"
  )]
  BootComplete,
  #[words(
    word = "boot-failed",
    about = "what a boot says about the newest generation when it fell over"
  )]
  BootFailed,
}

impl Subcommand for Action {
  fn variants() -> Result<impl Iterator<Item = Self>, Never> {
    Ok(Action::VARIANTS.iter().copied())
  }

  fn spelling(self) -> Result<&'static str, Never> {
    self.word()
  }

  fn about(self) -> Result<&'static str, Never> {
    Action::about(self)
  }
}

const TREE: Flag = Flag {
  spelling: "--root",
  takes: Takes::Value("PATH"),
  about: "read the desktop out of PATH rather than this machine's own",
};

const PENDING: Flag = Flag {
  spelling: "--pending",
  takes: Takes::None,
  about: "with migrate, say what has not run and run none of it",
};

const PREBUILT: Flag = Flag {
  spelling: "--prebuilt",
  takes: Takes::None,
  about: "with apply, install the programs already in target/release",
};

const SWITCH_FROM: Flag = Flag {
  spelling: "--switch-from",
  takes: Takes::Value("PATH"),
  about: "with prebuilt apply, exchange PATH and the live checkout after taking snapshots",
};

const COMMAND: Command = Command {
  name: "console",
  about: "the machine as desktop.conf says it should be",
  flags: &[TREE, PENDING, PREBUILT, SWITCH_FROM],
  operands: Operands::Any("PATH"),
};

struct Arguments {
  action: Action,
  root: PathBuf,
  pending: Presence,
  programs: build::Programs,
  switch_from: Option<PathBuf>,
  paths: Vec<String>,
}

fn asked(words: &[String]) -> Result<Arguments, ValidationError> {
  let line = read_with::<Action, String>(&COMMAND, words)?;
  let Ok(said) = line.subcommand();
  let Ok(root) = line.value(TREE);
  let Ok(pending) = line.presence(PENDING);
  let Ok(prebuilt) = line.presence(PREBUILT);
  let Ok(switch_from) = line.value(SWITCH_FROM);
  let Ok(paths) = line.operands();
  let action = match said {
    Some(action) => action,
    None => Action::Check,
  };
  let programs = match (prebuilt, action) {
    (Presence::Absent, _) => build::Programs::Compile,
    (Presence::Present, Action::Apply) => build::Programs::Prebuilt,
    (Presence::Present, other) => {
      let Ok(name) = other.spelling();
      let Ok(refusal) = line.refusal(Reason::InvalidValue {
        of: PREBUILT.spelling,
        value: name.to_string(),
      });

      return Err(refusal);
    }
  };
  let switch_from_2 = match (switch_from, programs, root) {
    (None, _, _) => None,
    (Some(at), build::Programs::Prebuilt, None) => Some(PathBuf::from(at)),
    (Some(at), _, _) => {
      let Ok(refusal) = line.refusal(Reason::InvalidValue {
        of: SWITCH_FROM.spelling,
        value: at.to_string(),
      });

      return Err(refusal);
    }
  };

  match action {
    Action::Save => {}
    Action::List
    | Action::Check
    | Action::Apply
    | Action::Room
    | Action::Buttons
    | Action::Health
    | Action::Migrate
    | Action::BootStarted
    | Action::BootComplete
    | Action::BootFailed => {
      let [] = line.exactly([])?;
    }
  }

  Ok(Arguments {
    action,
    root: match root {
      Some(root_2) => PathBuf::from(root_2),
      None => PathBuf::from(ROOT),
    },
    pending,
    programs,
    switch_from: switch_from_2,
    paths: paths.to_vec(),
  })
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();

  let Ok(direct) = device_command::dispatch(&words);

  match direct {
    Some(code) => return code,
    None => {}
  }

  let asked = match asked(&words) {
    Ok(asked) => asked,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  let boot = match asked.action {
    Action::BootStarted => Some(boots::Event::Started),
    Action::BootComplete => Some(boots::Event::Complete),
    Action::BootFailed => Some(boots::Event::Failed),
    Action::List
    | Action::Check
    | Action::Apply
    | Action::Room
    | Action::Buttons
    | Action::Health
    | Action::Save
    | Action::Migrate => None,
  };

  match boot {
    Some(boot_2) => {
      let Ok(said) = report(recorded(boot_2));

      return said;
    }
    None => {}
  }

  let root = asked.root;
  let manifest_root = match asked.switch_from.as_deref() {
    Some(candidate) => candidate,
    None => &root,
  };
  let manifest = match read(manifest_root) {
    Ok(manifest) => manifest,
    Err(fault) => {
      eprintln!("{fault}");
      return ExitCode::FAILURE;
    }
  };

  match asked.action {
    Action::List => {
      let Ok(()) = list(&manifest);
    }
    Action::Check => {
      let Ok(said) = check(&root, &manifest);

      return said;
    }
    Action::Apply => {
      let Ok(said) = report(apply(Apply {
        root: &root,
        manifest: &manifest,
        programs: asked.programs,
        switch_from: asked.switch_from.as_deref(),
      }));

      return said;
    }
    Action::Room => {
      let Ok(said) = room(&root);

      return said;
    }
    Action::Buttons => {
      let Ok(said) = report(rebuttoned(&root, &manifest));

      return said;
    }
    Action::Health => {
      let Ok(said) = health(&root, &manifest);

      return said;
    }
    Action::Save => {
      let Ok(said) = report(save(&root, &manifest, &asked.paths));

      return said;
    }
    Action::Migrate => {
      let Ok(said) = report(migrate(asked.pending));

      return said;
    }
    Action::BootStarted | Action::BootComplete | Action::BootFailed => {}
  }

  ExitCode::SUCCESS
}

fn read(root: &Path) -> Result<Manifest, Unapplied> {
  let at = root.join(manifest::MARK);
  let held = std::fs::read_to_string(&at).map_err(|fault| Unapplied::Read(at.clone(), fault))?;
  let read = Manifest::read(&held)?;
  let mine = quirks(root)?;

  read.and(manifest::Configuration(machines::AT), &mine)
}

fn quirks(root: &Path) -> Result<String, Unapplied> {
  let at = root.join(machines::AT);
  let Ok(held) = console_core_atomic_writes::read(&at);

  match held {
    Stored::Text(said) => machines::here(&said, Path::new(machines::FIRMWARE)),
    Stored::Absent => Ok(String::new()),
    Stored::Failed(fault) => Err(Unapplied::Unsaid(at.clone(), fault)),
  }
}

fn report(done: Result<(), Unapplied>) -> Result<ExitCode, Never> {
  Ok(match done {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("{RED}{fault}{OFF}");
      ExitCode::FAILURE
    }
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusLine<'a> {
  state: &'a str,
  about: &'a str,
}

fn line(color: &str, said: StatusLine<'_>) -> Result<(), Never> {
  let StatusLine { state, about } = said;
  let Ok(written) = fitted::<_, u32>(state.chars().count());
  let Ok(short) = console_core_number_conversion::index(COLUMN.saturating_sub(written));
  let pad = " ".repeat(short);

  println!("  {color}{state}{OFF}{pad}  {about}");

  Ok(())
}

fn status_color(ok: UpToDate) -> Result<&'static str, Never> {
  Ok(match ok {
    UpToDate::Yes => GREEN,
    UpToDate::No => RED,
  })
}

fn list(manifest: &Manifest) -> Result<(), Never> {
  let Ok(sections) = manifest.sections();

  for (section, entries) in sections {
    let Ok(name) = section.name();

    println!("{YELLOW}[{name}]{OFF}");

    for entry in entries {
      let Ok(written) = manifest.write_policy(entry);

      match written {
        manifest::Written::Once => println!("  {entry} {}", manifest::ONCE),
        manifest::Written::Always => println!("  {entry}"),
      }
    }

    println!();
  }

  Ok(())
}

fn check(root: &Path, manifest: &Manifest) -> Result<ExitCode, Never> {
  let source = root.join("files");
  let Ok(whoever) = machine::whoever();
  let Ok(have) = confirmation::to("reading packages", || {
    let Ok(have) = pacman::installed();

    have
  });
  let Ok(asked_for) = confirmation::to("reading wanted", || {
    let Ok(asked_for) = pacman::explicit();

    asked_for
  });
  let Ok(named) = manifest.of(Section::Packages);
  let Ok(built) = manifest.builds();
  let Ok(files) = manifest.of(Section::Files);
  let Ok(services) = manifest.of(Section::Services);
  let Ok(masked) = manifest.of(Section::Masked);

  let drift = [
    confirmation::to("packages", || {
      let Ok(drift) = under("packages", named, |package| {
        let Ok(held) = packages::package_state(&have, &asked_for, package);
        let Ok(settled) = held.up_to_date();
        let Ok(name) = held.name();

        (settled, String::from(name), package.clone())
      });

      drift
    }),
    confirmation::to("built", || {
      let Ok(drift) = under("built", built, |name| {
        let Ok(state) = build::state(root, name);
        let Ok(settled) = state.up_to_date();
        let Ok(said) = state.name();
        let Ok(live) = build::live(name);

        (settled, String::from(said), live)
      });

      drift
    }),
    confirmation::to("files", || {
      let Ok(drift) = under("files", files, |path| {
        let Ok(written) = manifest.write_policy(path);
        let Ok(state) = install::state(&source, path, User(whoever), written);
        let Ok(settled) = state.up_to_date();
        let Ok(said) = state.name();

        (settled, String::from(said), path.clone())
      });

      drift
    }),
    units_drift(services),
    masked_drift(masked),
    confirmation::to("left", || {
      let Ok(drift) = left_drift(root, manifest, whoever);

      drift
    }),
  ]
  .into_iter()
  .map(|drift| {
    let Ok(drift_2) = drift;

    drift_2
  })
  .fold(0_u32, u32::saturating_add);

  let Ok(home) = home();
  let Ok(standing) = buttons::check_buttons(root, &home);
  let Ok(()) = front(&standing);

  Ok(match drift {
    0 => {
      println!("{GREEN}The machine matches the manifest.{OFF}");
      ExitCode::SUCCESS
    }
    drift_3 => {
      println!(
        "{RED}{drift} differences.{OFF} `console apply` settles them.",
        drift = drift_3
      );
      ExitCode::FAILURE
    }
  })
}

fn units_drift(services: &[String]) -> Result<u32, Never> {
  confirmation::to("services", || {
    let Ok(drift) = under("services", services, |unit| {
      let Ok((enabled, active)) = machine::unit_state(unit);
      let ok = match ::core::primitive::bool::then_some(
        enabled == "enabled" && active == "active",
        (),
      ) {
        ::core::option::Option::Some(()) => UpToDate::Yes,
        ::core::option::Option::None => UpToDate::No,
      };

      (ok, format!("{enabled}, {active}"), unit.clone())
    });

    drift
  })
}

fn left_drift(root: &Path, manifest: &Manifest, whoever: &str) -> Result<u32, Never> {
  let pending = match pruning::pending(root, manifest, User(whoever)) {
    Ok(pending) => pending,
    Err(fault) => {
      println!("{YELLOW}left{OFF}");
      println!("  {RED}{fault}{OFF}\n");

      return Ok(1);
    }
  };

  let showing: Vec<&(pruning::Left, pruning::Standing)> = pending
    .left
    .iter()
    .filter(|(_, stands)| *stands != pruning::Standing::Gone)
    .collect();

  match ::core::primitive::bool::then_some(showing.is_empty() && pending.unread.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(0),
    ::core::option::Option::None => {}
  }

  println!("{YELLOW}left{OFF}");

  for unread in &pending.unread {
    println!("  {YELLOW}{unread}{OFF}");
  }

  let mut drift = 0_u32;

  for (placed, stands) in showing {
    let Ok(said) = stands.name();
    let Ok(about) = placed.about();

    let color = match stands {
      pruning::Standing::Left => {
        drift = drift.saturating_add(1);

        RED
      }
      pruning::Standing::Gone
      | pruning::Standing::Edited
      | pruning::Standing::WrittenOnce
      | pruning::Standing::Packaged
      | pruning::Standing::Invalid
      | pruning::Standing::OwnerUnknown => YELLOW,
    };

    let Ok(()) = line(color, StatusLine { state: said, about });
  }

  println!();

  Ok(drift)
}

fn pruned(root: &Path, manifest: &Manifest, whoever: &str) -> Result<(), Unapplied> {
  let pending = pruning::pending(root, manifest, User(whoever))?;

  for unread in &pending.unread {
    println!("{YELLOW}{unread}{OFF}");
  }

  let mut attic: Option<PathBuf> = None;
  let mut taken: Vec<String> = Vec::new();

  for (placed, stands) in &pending.left {
    let Ok(about) = placed.about();

    match (placed, stands) {
      (pruning::Left::Enabled(unit), pruning::Standing::Left) => {
        let Ok(()) = line(
          YELLOW,
          StatusLine {
            state: "disabling",
            about,
          },
        );
        let Ok(_) = machine::user_systemctl(&["disable", "--now", unit]);
      }
      (pruning::Left::Masked(unit), pruning::Standing::Left) => {
        let Ok(()) = line(
          YELLOW,
          StatusLine {
            state: "unmasking",
            about,
          },
        );
        let Ok(_) = machine::user_systemctl(&["unmask", unit]);
      }
      (pruning::Left::Program(_) | pruning::Left::File { .. }, _)
      | (
        pruning::Left::Enabled(_) | pruning::Left::Masked(_),
        pruning::Standing::Gone
        | pruning::Standing::Edited
        | pruning::Standing::WrittenOnce
        | pruning::Standing::Packaged
        | pruning::Standing::Invalid
        | pruning::Standing::OwnerUnknown,
      ) => {}
    }
  }

  for (placed, stands) in &pending.left {
    let Ok(about) = placed.about();

    let on = match placed {
      pruning::Left::Program(live) => live.clone(),
      pruning::Left::File { declared, .. } => {
        let Ok(on) = install::on_machine(declared, User(whoever));

        on
      }
      pruning::Left::Enabled(_) | pruning::Left::Masked(_) => continue,
    };

    match stands {
      pruning::Standing::Left => {}
      pruning::Standing::Gone => continue,
      pruning::Standing::Edited
      | pruning::Standing::WrittenOnce
      | pruning::Standing::Packaged
      | pruning::Standing::Invalid
      | pruning::Standing::OwnerUnknown => {
        let Ok(said) = stands.name();
        let Ok(()) = line(YELLOW, StatusLine { state: said, about });

        continue;
      }
    }

    let into = match &attic {
      Some(into) => into.clone(),
      None => {
        let into = migrating::attic()?;

        attic = Some(into.clone());

        into
      }
    };

    let under = pruning::take(Path::new(&on), &into)?;

    println!("  {on} -> {}", under.display());

    taken.push(on);
  }

  match ::core::primitive::bool::then_some(taken.iter().any(|on| on.contains("/systemd/")), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(_) = machine::user_systemctl(&["daemon-reload"]);
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn masked_drift(masked: &[String]) -> Result<u32, Never> {
  confirmation::to("masked", || {
    let Ok(drift) = under("masked", masked, |unit| {
      let Ok((enabled, _)) = machine::unit_state(unit);
      let ok = match ::core::primitive::bool::then_some(enabled == "masked", ()) {
        ::core::option::Option::Some(()) => UpToDate::Yes,
        ::core::option::Option::None => UpToDate::No,
      };
      let said = match ::core::primitive::bool::then_some(enabled.is_empty(), ()) {
        ::core::option::Option::Some(()) => "not masked".to_string(),
        ::core::option::Option::None => enabled,
      };

      (ok, said, unit.clone())
    });

    drift
  })
}

fn front(standing: &buttons::Standing) -> Result<(), Never> {
  println!("{YELLOW}buttons{OFF}");

  let Ok(settled) = standing.up_to_date();

  match (standing.asked, settled) {
    (false, _) => {
      let Ok(()) = line(
        YELLOW,
        StatusLine {
          state: "not asked",
          about: "InputPlumber did not say what this device sends",
        },
      );
    }
    (true, UpToDate::Yes) => {
      let Ok(()) = line(
        GREEN,
        StatusLine {
          state: "all here",
          about: "every button this desktop binds",
        },
      );
    }
    (true, UpToDate::No) => {
      for lost in &standing.missing {
        let Ok(()) = line(
          RED,
          StatusLine {
            state: "not here",
            about: lost,
          },
        );
      }
    }
  }

  match ::core::primitive::bool::then_some(standing.moved > 0, ()) {
    ::core::option::Option::Some(()) => {
      let many = standing.moved;
      let Ok(()) = line(
        GREEN,
        StatusLine {
          state: "moved",
          about: &format!("{many} of them are elsewhere on this device"),
        },
      );
    }
    ::core::option::Option::None => {}
  }

  match standing.touchscreen {
    Some(console_input_gamepad::devices::Has::No) => {
      let Ok(()) = line(
        YELLOW,
        StatusLine {
          state: "no touchscreen",
          about: "nothing here can be driven by a finger",
        },
      );
    }
    Some(console_input_gamepad::devices::Has::Yes) | None => {}
  }

  println!();

  Ok(())
}

fn under<T>(
  name: &str,
  entries: impl IntoIterator<Item = T>,
  state: impl Fn(T) -> (UpToDate, String, String),
) -> Result<u32, Never> {
  println!("{YELLOW}{name}{OFF}");

  let Ok(drift) = fitted::<_, u32>(
    entries
      .into_iter()
      .map(state)
      .filter(|(ok, said, about)| {
        let Ok(color) = status_color(*ok);
        let Ok(()) = line(color, StatusLine { state: said, about });

        *ok == UpToDate::No
      })
      .count(),
  );

  println!();

  Ok(drift)
}

struct Updating {
  finished: bool,
}

impl Updating {
  fn started() -> Result<Self, Never> {
    let Ok(()) = machine::in_the_session(&["console-updating", "start"]);

    Ok(Updating { finished: false })
  }

  fn finish(mut self) -> Result<(), Never> {
    self.finished = true;

    let Ok(()) = machine::in_the_session(&["console-updating", "done"]);

    Ok(())
  }
}

impl Drop for Updating {
  fn drop(&mut self) {
    match ::core::primitive::bool::then_some(!self.finished, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = machine::in_the_session(&["console-updating", "failed"]);
      }
      ::core::option::Option::None => {}
    }
  }
}

fn the_battery() -> Result<console_battery::Charge, Never> {
  let said = console_battery::charge()?;

  console_battery::Charge::of(&said)
}

fn the_room(at: &Path) -> Result<room::Left, Never> {
  let Ok(said) = machine::disk_free(at);

  room::free_in(&said.out)
}

fn where_it_went() -> Result<Vec<room::Place>, Never> {
  let Ok(whoever) = machine::whoever();
  let Ok(home) = machine::home_path(User(whoever));
  let Ok(share) = console_core_places::Base::Share.under(&home);

  let roots = vec![home.display().to_string(), share.display().to_string()];
  let asked: Vec<&str> = roots.iter().map(String::as_str).collect();
  let Ok(said) = machine::sizes_under(&asked);

  room::places_in(&said.out, &roots)
}

fn the_levels() -> Result<console_battery::Levels, Never> {
  use console_battery::Levels;

  let Ok(whoever) = machine::whoever();
  let Ok(home) = machine::home_path(User(whoever));
  let Ok(at) = console_defaults::under(&home);

  let Ok(held) = console_core_atomic_writes::read(&at);

  match held {
    console_core_atomic_writes::Stored::Text(said) => Levels::read(&said),
    console_core_atomic_writes::Stored::Absent => Ok(Levels::default()),
    console_core_atomic_writes::Stored::Failed(fault) => {
      println!(
        "{YELLOW}{} will not be read ({fault}), so the battery levels this apply is \
                 judged against are the ones no one chose{OFF}",
        at.display()
      );

      Ok(Levels::default())
    }
  }
}

fn health_check(root: &Path, manifest: &Manifest) -> Result<health::Standing, Unapplied> {
  let source = root.join("files");
  let Ok(user) = machine::whoever();
  let mut standing = health::Standing::default();

  let kept = generations::read(Path::new(generations::KEPT))?;

  let Ok(unfinished) = generations::unfinished(&kept);

  standing.unfinished = match unfinished {
    Some(generation) => {
      let Ok(named) = generation.label();

      Some(named)
    }
    None => None,
  };

  let Ok(plan) = console_core_atomic_writes::read(Path::new(machine::PLAN));

  match plan {
    console_core_atomic_writes::Stored::Absent => {}
    console_core_atomic_writes::Stored::Text(said) => {
      standing.midway = said
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(_, at)| at.to_string())
        .collect();
    }
    console_core_atomic_writes::Stored::Failed(fault) => {
      standing.midway = vec![format!("{} ({fault})", machine::PLAN)];
    }
  }

  let Ok(files) = manifest.of(Section::Files);
  let Ok(built) = manifest.builds();
  let Ok(services) = manifest.of(Section::Services);
  let claimed: Vec<String> = files
    .iter()
    .cloned()
    .chain(built.clone().map(|name| {
      let Ok(live) = build::live(name);

      live
    }))
    .collect();

  for live in &claimed {
    let Ok(on) = install::on_machine(live, User(user));
    let at = Path::new(&on);
    let Ok(staged) = laying::staged(at);
    let Ok(kept_2) = laying::backup_path(at);

    let over =
      [staged, kept_2].into_iter().flatten().any(|beside| {
        match std::fs::symlink_metadata(beside) {
          Ok(_leftover) => true,
          Err(_no_accessible_leftover) => false,
        }
      });

    match ::core::primitive::bool::then_some(over, ()) {
      ::core::option::Option::Some(()) => standing.leftovers.push(live.clone()),
      ::core::option::Option::None => {}
    }
  }

  for live in files {
    let Ok(written) = manifest.write_policy(live);
    let Ok(state) = install::state(&source, live, User(user), written);
    let Ok(settled) = state.up_to_date();

    match ::core::primitive::bool::then_some(
      state != install::State::ParseError && settled == UpToDate::No,
      (),
    ) {
      ::core::option::Option::Some(()) => standing.adrift.push(live.clone()),
      ::core::option::Option::None => {}
    }
  }

  for name in built {
    let Ok(state) = build::state(root, name);
    let Ok(settled) = state.up_to_date();

    match ::core::primitive::bool::then_some(settled == UpToDate::No, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(live) = build::live(name);

        standing.adrift.push(live);
      }
      ::core::option::Option::None => {}
    }
  }

  for unit in services {
    let described = |unit_2: &str| {
      let Ok(said) = machine::mine(&["show", "-p", "Description", "--value", unit_2]);

      health::Piece::new(unit_2, health::Called(&said.out))
    };
    let Ok(active) = machine::mine(&["is-active", unit]);

    match ::core::primitive::bool::then_some(active.out != "active", ()) {
      ::core::option::Option::Some(()) => {
        let Ok(result) = machine::mine(&["show", "-p", "Result", "--value", unit]);

        match ::core::primitive::bool::then_some(result.out.trim() == DECLINED, ()) {
          ::core::option::Option::Some(()) => continue,
          ::core::option::Option::None => {}
        }

        let Ok(piece) = described(unit);

        standing.failing.push(piece);
        continue;
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(!unit.ends_with(".service"), ()) {
      ::core::option::Option::Some(()) => continue,
      ::core::option::Option::None => {}
    }

    let Ok(restarts) = machine::mine(&["show", "-p", "NRestarts", "--value", unit]);

    match restarts.out.parse::<u32>() {
      Ok(0) => {}
      Ok(times) => {
        let Ok(piece) = described(unit);

        standing.restarted.push((piece, times));
      }
      Err(_not_a_number) => {
        eprintln!("console health: {unit} would not say how often it has restarted")
      }
    }
  }

  let Ok(left) = the_room(root);

  match left {
    room::Left::Reported(free) => {
      let Ok(went) = match ::core::primitive::bool::then_some(free < room::A_BUILD, ()) {
        ::core::option::Option::Some(()) => where_it_went(),
        ::core::option::Option::None => Ok(Vec::new()),
      };
      let Ok(asking) = room::on_a_machine_standing(free, &went);

      match asking {
        room::Room::No(said) => standing.cramped = Some(said),
        room::Room::Enough => {}
      }
    }
    room::Left::Unknown(said) => {
      eprintln!("console health: the disk would not say how much room is left ({said})");
    }
  }

  Ok(standing)
}

fn room(root: &Path) -> Result<ExitCode, Never> {
  let Ok(left) = the_room(root);

  let free = match left {
    room::Left::Reported(free) => free,
    room::Left::Unknown(said) => {
      eprintln!("{RED}the disk would not say how much room is left ({said}){OFF}");

      return Ok(ExitCode::FAILURE);
    }
  };
  let Ok(went) = match ::core::primitive::bool::then_some(free < room::STANDING, ()) {
    ::core::option::Option::Some(()) => where_it_went(),
    ::core::option::Option::None => Ok(Vec::new()),
  };
  let Ok(asking) = room::before_an_apply(free, &went);

  match asking {
    room::Room::No(said) => {
      eprintln!("{RED}room{OFF} {said}");

      return Ok(ExitCode::FAILURE);
    }
    room::Room::Enough => {}
  }

  let Ok(standing) = room::on_a_machine_standing(free, &went);
  let Ok(size) = room::words(free);
  let Ok(wanted) = room::words(room::A_BUILD);

  Ok(match standing {
    room::Room::Enough => {
      println!("{GREEN}room{OFF} {size} left, and an apply wants {wanted} of it");

      ExitCode::SUCCESS
    }
    room::Room::No(said) => {
      println!("{YELLOW}room{OFF} {said}");

      ExitCode::SUCCESS
    }
  })
}

fn health(root: &Path, manifest: &Manifest) -> Result<ExitCode, Never> {
  let standing = match health_check(root, manifest) {
    Ok(standing) => standing,
    Err(fault) => {
      eprintln!("{RED}{fault}{OFF}");

      return Ok(ExitCode::FAILURE);
    }
  };
  let kind = "health";

  let Ok(card) = console_notifications::saying::StatePath::new(kind);

  let Ok(said) = standing.problem();

  let (summary, body) = match said {
    Some((summary, body)) => (summary, body),
    None => {
      let Ok(()) = console_notifications::saying::withdraw(&card);
      let Ok(counting) = console_notifications::saying::StatePath::counting(kind);
      let Ok(()) = counting.forget();

      println!(
        "{GREEN}health{OFF} this machine is what the manifest says, and every piece of it is up"
      );

      return Ok(ExitCode::SUCCESS);
    }
  };

  println!("{RED}{summary}{OFF}\n{body}");

  let Ok(said_2) = console_notifications::saying::for_the_journal(
    kind,
    console_notifications::saying::Content {
      summary: &summary,
      body: &body,
    },
  );
  let Ok(()) = console_notifications::saying::journal(&said_2);
  let Ok(counting) = console_notifications::saying::StatePath::counting(kind);
  let Ok(again) = counting.again();
  let Ok(once) = console_notifications::saying::once(
    console_notifications::saying::Content {
      summary: &summary,
      body: &body,
    },
    again,
  );

  match once {
    Some(notification) => {
      let Ok(()) = console_notifications::saying::raise_kept(notification, &card);
    }
    None => {}
  }

  Ok(ExitCode::FAILURE)
}

fn migrate(pending: Presence) -> Result<(), Unapplied> {
  match pending {
    Presence::Present => {
      let pending_2 = migrating::outstanding()?;

      match pending_2 {
        Outstanding::Run(migrations) => {
          for migration in migrations {
            println!("{} {}", migration.moment, migration.says);
          }
        }
        Outstanding::Remember(_every_one_of_them) => {
          println!(
            "this machine has never applied, so nothing is swept and the history is \
                         marked done by the apply that installs it"
          );
        }
      }

      Ok(())
    }
    Presence::Absent => {
      let Ok(root_is) = nix_is_root();

      match ::core::primitive::bool::then_some(root_is == Root::No, ()) {
        ::core::option::Option::Some(()) => return Err(Unapplied::AsRoot("migrate")),
        ::core::option::Option::None => {}
      }

      let Ok(whoever) = machine::whoever();

      migrating::run(User(whoever))
    }
  }
}

struct Laying {
  deploy: Deploy,
  onto: machine::Here,
}

struct Fetching<'a, 'b> {
  moving: &'a mut going::Moving<'b>,
  done: u32,
}

#[derive(Clone, Copy)]
struct Apply<'a> {
  root: &'a Path,
  manifest: &'a Manifest,
  programs: build::Programs,
  switch_from: Option<&'a Path>,
}

fn apply(asked: Apply<'_>) -> Result<(), Unapplied> {
  let Apply {
    root,
    manifest,
    programs,
    switch_from,
  } = asked;
  let Ok(root_is) = nix_is_root();

  match ::core::primitive::bool::then_some(root_is == Root::No, ()) {
    ::core::option::Option::Some(()) => return Err(Unapplied::AsRoot("apply")),
    ::core::option::Option::None => {}
  }

  let _alone = alone::taking()?;

  enough_to_apply(root)?;
  let manifest_root = match switch_from {
    Some(candidate) => candidate,
    None => root,
  };

  match programs {
    build::Programs::Compile => {}
    build::Programs::Prebuilt => {
      let Ok(names) = manifest.builds();
      build::verify_prebuilt(manifest_root, names)?;
    }
  }

  let Ok(inhibited) = console_awake::inhibit(console_awake::InhibitReason::FromStopping);
  let _staying = match inhibited {
    console_awake::InhibitResult::Acquired(held) => Some(held),
    console_awake::InhibitResult::Failed(said) => {
      println!("{YELLOW}{said}{OFF}");
      None
    }
  };
  let (was, running) = start_generation(manifest_root)?;

  match switch_from {
    Some(candidate) => switch_checkout(root, candidate)?,
    None => {}
  }

  let Ok(whoever) = machine::whoever();

  migrating::run(User(whoever))?;
  pruned(root, manifest, whoever)?;

  let Ok(saying) = Updating::started();
  let Ok(applying) = confirmation::started();
  let Ok(mut going) = going::Going::starting();

  install_generation(asked, &mut going)?;

  let Ok(uncommitted) = machine::uncommitted(root);

  for open in uncommitted {
    println!("{YELLOW}not committed{OFF} {open}");
  }

  let Ok(after) = marked(root);
  let Ok(snapshots) = snapshot::after(&was, &after);

  for post in snapshots {
    match post {
      snapshot::Snapshot::Made { .. } => {}
      snapshot::Snapshot::Not { configuration, why } => {
        eprintln!(
          "console apply: could not take the {configuration} snapshot after apply: {why}"
        );
      }
    }
  }

  let Ok(done) = running.finished();

  generations::remember(Path::new(generations::KEPT), &done)?;

  let Ok(()) = timed(going, applying, whoever);
  let Ok(()) = saying.finish();
  let Ok(()) = told_the_front(root);

  println!("\n{GREEN}Done.{OFF}");

  Ok(())
}

fn install_generation(
  Apply {
    root,
    manifest,
    programs,
    ..
  }: Apply<'_>,
  going: &mut going::Going,
) -> Result<(), Unapplied> {
  packages_held(going, manifest)?;

  let Ok(()) = going.through(going::SWEEPING, || {
    let Ok(()) = swept(manifest);
  });

  let mut laying = Laying {
    deploy: Deploy::default(),
    onto: machine::Here,
  };
  let source = root.join("files");
  let written = staged_files(root, &source, manifest, programs, going, &mut laying)?;
  let Ok(swapped) = going.through_handed(going::SWAPPING, &mut laying, |laying_4| {
    laying_4.deploy.swap(&mut laying_4.onto)
  });

  swapped?;

  let Ok(()) = told_the_rest(going);

  match ::core::primitive::bool::then_some(
    written.iter().any(|path| path.contains("/systemd/")),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(_) = machine::user_systemctl(&["daemon-reload"]);
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = buttons::wear_again();

  let Ok(services) = manifest.of(Section::Services);
  let started = Started {
    source: &source,
    written: &written,
  };
  let Ok(asked_to_run) = services_running(going, services, started);
  let Ok(fell) = fallen(&asked_to_run);

  match ::core::primitive::bool::then_some(!fell.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = undone(&mut laying, &written, &asked_to_run, &fell);

      return Err(Unapplied::Restore(fell.clone()));
    }
    ::core::option::Option::None => {}
  }

  let Ok(()) = going.through_handed(going::RELEASE, &mut laying, |laying_5| {
    let Ok(()) = laying_5.deploy.settle(&mut laying_5.onto);
  });
  let Ok(()) = mask_units(manifest);

  Ok(())
}

fn switch_checkout(root: &Path, candidate: &Path) -> Result<(), Unapplied> {
  let Ok(mut exchange) = Program::Mv.command();
  let exchanged = exchange
    .args(["--exchange", "--no-copy", "-T", "--"])
    .arg(candidate)
    .arg(root)
    .status()
    .map_err(Unapplied::CheckoutSwitch)?;

  match ::core::primitive::bool::then_some(exchanged.success(), ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Unapplied::CheckoutSwitch(std::io::Error::other(
      format!("mv exited with {exchanged}"),
    ))),
  }
}

fn start_generation(
  root: &Path,
) -> Result<(Vec<snapshot::Snapshot>, generations::Generation), Unapplied> {
  let Ok(what) = marked(root);
  let Ok(was) = snapshot::before(&what);
  let Ok(()) = told_what_there_is_to_come_back_to(&was);
  let Ok(commit) = commit(root);
  let kept = generations::read(Path::new(generations::KEPT))?;
  let Ok(before) = snapshot::root(&was);
  let Ok(running) = generations::next(&kept, generations::Commit(&commit), before);

  generations::remember(Path::new(generations::KEPT), &running)?;

  let Ok(about) = running.label();
  let Ok(()) = line(
    YELLOW,
    StatusLine {
      state: "generation",
      about: &about,
    },
  );

  Ok((was, running))
}

fn staged_files(
  root: &Path,
  source: &Path,
  manifest: &Manifest,
  programs: build::Programs,
  going: &mut going::Going,
  laying: &mut Laying,
) -> Result<Vec<String>, Unapplied> {
  let Ok(built) = going.during_handed(going::BUILDING, laying, |laying_2, moving| {
    stage_programs(
      root,
      manifest,
      programs,
      &mut laying_2.deploy,
      &mut laying_2.onto,
      moving,
    )
  });

  let staged = match built {
    Ok(built_2) => {
      let Ok(files) = going.during_handed(going::FILES, laying, |laying_3, moving| {
        write(
          source,
          manifest,
          &mut laying_3.deploy,
          &mut laying_3.onto,
          moving,
        )
      });

      match files {
        Ok(written) => Ok((built_2, written)),
        Err(fault) => Err(fault),
      }
    }
    Err(fault) => Err(fault),
  };

  match staged {
    Ok((built_3, files)) => Ok(built_3.into_iter().chain(files).collect()),
    Err(fault) => {
      let Ok(()) = laying.deploy.abandon(&mut laying.onto);

      Err(fault)
    }
  }
}

fn recorded(event: boots::Event) -> Result<(), Unapplied> {
  use console_core_state_machine::{Machine, Transition};

  let at = Path::new(generations::KEPT);
  let kept = generations::read(at)?;
  let Ok(first) = boots::BootRecord::initial_transition(&boots::Recording { event, kept }, None);
  let ended = console_core_iteration::iterate(first, |turn| {
    let Transition {
      state: current,
      effects,
    } = turn;
    let requested: Result<[boots::Effect; 1], _> = effects.try_into();
    let (state, effect) = match (current, requested) {
      (boots::State::Finished(result), _) => return Ok(Step::Halt(result)),
      (waiting, Ok([effect])) => (waiting, effect),
      (_, Err(_unexpected)) => return Ok(Step::Halt(Err(Unapplied::BootRecordingStopped))),
    };
    let report = match effect {
      boots::Effect::Remember(generation) => {
        boots::Report::Remembered(remember_boot(at, &generation))
      }
      boots::Effect::Select(selection) => boots::Report::Selected(select_boot(selection)),
    };
    let Ok(next) = boots::BootRecord::transition(state, report);

    Ok(Step::Again(next))
  });

  ended.map_err(|_endless| Unapplied::BootRecordingStopped)?
}

fn remember_boot(at: &Path, generation: &generations::Generation) -> Result<(), Unapplied> {
  generations::remember(at, generation)?;
  let Ok(named) = generation.label();

  println!(
    "generation {named}: tried {} of {}, {:?}",
    generation.tries,
    boots::TRIES,
    generation.boot
  );

  Ok(())
}

fn select_boot(selection: boots::Selection) -> Result<(), Unapplied> {
  match selection {
    boots::Selection::Clear => one_shot(None),
    boots::Selection::Snapshot(generations::SnapshotNumber(number)) => {
      let config = Path::new(console_boot_entries::CONFIG);
      let Ok(held) = console_core_atomic_writes::read(config);
      let said = match held {
        Stored::Text(said) => said,
        Stored::Absent => return Err(Unapplied::NoBootEntry(number)),
        Stored::Failed(fault) => return Err(Unapplied::Unsaid(config.to_path_buf(), fault)),
      };
      let Ok(entries) = console_boot_entries::entries(&said);
      let Ok(found) = console_boot_entries::snapshot(&entries, number);
      let entry = match found {
        Some(entry) => entry,
        None => return Err(Unapplied::NoBootEntry(number)),
      };

      println!(
        "the next boot goes back to snapshot {number}, {}",
        entry.identifier
      );

      one_shot(Some(&entry.identifier))
    }
  }
}

fn one_shot(identifier: Option<&str>) -> Result<(), Unapplied> {
  let Ok(command) = console_boot_entries::one_shot(identifier);
  let Ok(program) = command.program.name();
  let mut starting = std::process::Command::new(program);
  let Ok(answered) = machine::capture(starting.args(&command.arguments));

  match answered.ran {
    Ran::Fine => Ok(()),
    Ran::Badly => Err(Unapplied::OneShotRefused(answered.said.trim().to_string())),
  }
}

fn timed(going: going::Going, applying: std::time::Instant, whoever: &str) -> Result<(), Never> {
  let Ok(stages) = going.finish();
  let Ok(took) = confirmation::ended("apply", applying);

  let Ok(home) = machine::home_path(User(whoever));

  confirmation::record_timings(&home, &stages, took)
}

fn on_the_ordinary_root() -> Result<(), Unapplied> {
  let mounts = Path::new(snapshot::MOUNTS);

  let said = match std::fs::read_to_string(mounts) {
    Ok(said) => said,
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => return Ok(()),
        ::core::option::Option::None => return Err(Unapplied::Read(mounts.to_path_buf(), fault)),
      }
    }
  };
  let Ok(running) = snapshot::running(&said);

  match running {
    snapshot::Running::Ordinary => Ok(()),
    snapshot::Running::Snapshot(generations::SnapshotNumber(number)) => {
      Err(Unapplied::OnASnapshot(number))
    }
  }
}

fn enough_to_apply(root: &Path) -> Result<(), Unapplied> {
  on_the_ordinary_root()?;

  let Ok(charge) = the_battery();
  let Ok(levels) = the_levels();
  let Ok(enough) = enough::enough(charge, levels);

  match enough {
    enough::Enough::No(said) => return Err(Unapplied::NotEnough(said)),
    enough::Enough::Yes => {}
  }

  let Ok(left) = the_room(root);
  let Ok(room) = match left {
    room::Left::Reported(free) => {
      let Ok(went) = match ::core::primitive::bool::then_some(free < room::A_BUILD, ()) {
        ::core::option::Option::Some(()) => where_it_went(),
        ::core::option::Option::None => Ok(Vec::new()),
      };

      room::before_an_apply(free, &went)
    }
    room::Left::Unknown(said) => {
      println!(
        "{YELLOW}the disk would not say how much room is left ({said}), so this apply is \
                 going ahead without knowing whether what it writes will fit{OFF}"
      );

      Ok(room::Room::Enough)
    }
  };

  match room {
    room::Room::No(said) => return Err(Unapplied::NoRoom(said)),
    room::Room::Enough => {}
  }

  Ok(())
}

fn packages_held(going: &mut going::Going, manifest: &Manifest) -> Result<(), Unapplied> {
  let Ok(named) = manifest.of(Section::Packages);
  let Ok(have) = going.through(going::READING, || {
    let Ok(have) = pacman::installed();

    have
  });
  let Ok(asked_for) = going.through(going::WANTED, || {
    let Ok(asked_for) = pacman::explicit();

    asked_for
  });
  let Ok(missing) = packages::missing_packages(named, &have);
  let Ok(installed) = going.during(going::PACKAGES, |moving| {
    match ::core::primitive::bool::then_some(missing.is_empty(), ()) {
      ::core::option::Option::Some(()) => return true,
      ::core::option::Option::None => {}
    }

    let Ok(many) = fitted::<_, u32>(missing.len());
    let Ok(()) = moving.say(&format!("{YELLOW}installing{OFF} {}", missing.join(" ")));

    let mut fetching = Fetching { moving, done: 0 };
    let installed = pacman::install_with(missing, &mut fetching, |progress, line, said| {
      let Ok(()) = progress.moving.say(line);

      match said {
        PacmanOutput::Downloading(name) => {
          progress.done = progress.done.saturating_add(1);

          let Ok(far) = installing::fetched(Progress {
            done: progress.done,
            many,
          });
          let Ok(()) = progress
            .moving
            .far(going::StageFraction(far), &format!("fetching {name}"));
        }

        PacmanOutput::Installing {
          done,
          many: many_2,
          name,
        } => {
          let Ok(far) = installing::overall_fraction(Progress { done, many: many_2 });
          let Ok(counted) = console_how_far::format_count(Progress { done, many: many_2 });
          let Ok(()) = progress
            .moving
            .far(going::StageFraction(far), &format!("{counted} {name}"));
        }

        PacmanOutput::Other => {}
      }

      Ok(())
    });

    installed.is_ok_and(|status| status.success())
  });

  match ::core::primitive::bool::then_some(installed, ()) {
    Some(()) => {}
    None => return Err(Unapplied::PacmanRefused),
  }

  let Ok(borrowed) = packages::borrowed(named, &have, &asked_for);
  let Ok(kept) = going.through(going::KEEPING, || {
    match ::core::primitive::bool::then_some(borrowed.is_empty(), ()) {
      ::core::option::Option::Some(()) => return true,
      ::core::option::Option::None => {}
    }

    println!("{YELLOW}keeping{OFF} {}", borrowed.join(" "));

    pacman::mark_explicit(borrowed).is_ok_and(|status| status.success())
  });

  match ::core::primitive::bool::then_some(kept, ()) {
    Some(()) => {}
    None => return Err(Unapplied::PacmanUntold),
  }

  Ok(())
}

fn told_the_rest(going: &mut going::Going) -> Result<(), Never> {
  let Ok(()) = going.through(going::ADD_ON, || {
    let Ok(()) = packed_the_add_on();
  });
  let Ok(()) = going.through(going::BROWSERS, || {
    let Ok(()) = told_the_browsers();
  });
  let Ok(()) = going.through(going::PROFILES, || {
    let Ok(wrote) = buttons::wrote_router();

    match wrote {
      Some(live) => println!("{YELLOW}writing{OFF} {live}"),
      None => {}
    }
  });
  let Ok(()) = going.through(going::SCREEN, || {
    let Ok(home) = home();
    let Ok(wrote) = screen::wrote(Path::new(&home));

    match wrote {
      Some(live) => println!("{YELLOW}writing{OFF} {live}"),
      None => {}
    }
  });
  let Ok(()) = going.through(going::WALLPAPERS, || {
    let Ok(()) = pressed_the_wallpapers();
  });

  Ok(())
}

struct Started<'a> {
  source: &'a Path,
  written: &'a [String],
}

fn services_running<'a>(
  going: &mut going::Going,
  services: &'a [String],
  started: Started<'_>,
) -> Result<Vec<&'a String>, Never> {
  let Started { source, written } = started;
  let mut asked_to_run: Vec<&String> = Vec::new();
  let Ok(()) = going.during_handed(
    going::SERVICES,
    &mut asked_to_run,
    |asked_to_run_2, moving| {
      let Ok(many) = fitted::<_, u32>(services.len());

      for (done, unit) in services.iter().enumerate() {
        let Ok(done_2) = fitted::<_, u32>(done);
        let Ok(()) = moving.at(Progress { done: done_2, many }, unit);
        let Ok((enabled, active)) = machine::unit_state(unit);

        match ::core::primitive::bool::then_some(enabled != "enabled", ()) {
          ::core::option::Option::Some(()) => {
            let Ok(()) = moving.say(&format!("{YELLOW}enabling{OFF} {unit}"));
            let Ok(_) = machine::user_systemctl(&["enable", unit]);
          }
          ::core::option::Option::None => {}
        }

        let Ok(restart) = restarted_by(source, unit, written);

        match active.as_str() {
          "active" => match ::core::primitive::bool::then_some(restart == Restart::Wanted, ()) {
            ::core::option::Option::Some(()) => {
              let Ok(()) = moving.say(&format!("{YELLOW}restarting{OFF} {unit}"));
              let Ok(_) = machine::user_systemctl(&["restart", unit]);

              asked_to_run_2.push(unit);
            }
            ::core::option::Option::None => {}
          },
          _ => {
            let Ok(()) = moving.say(&format!("{YELLOW}starting{OFF} {unit}"));
            let Ok(_) = machine::user_systemctl(&["start", unit]);

            asked_to_run_2.push(unit);
          }
        }
      }
    },
  );

  Ok(asked_to_run)
}

fn undone(
  laying: &mut Laying,
  written: &[String],
  asked_to_run: &[&String],
  fell: &[String],
) -> Result<(), Never> {
  println!("\n{RED}did not come up{OFF} {}", fell.join(" "));

  let Ok(undone) = laying.deploy.undo(&mut laying.onto);

  for one in undone {
    match one.put {
      Put::Back => println!("{YELLOW}put back{OFF} {}", one.at),
      Put::NotBack(fault) => println!("{RED}{fault}{OFF}"),
    }
  }

  match ::core::primitive::bool::then_some(
    written.iter().any(|path| path.contains("/systemd/")),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(_) = machine::user_systemctl(&["daemon-reload"]);
    }
    ::core::option::Option::None => {}
  }

  for unit in asked_to_run {
    println!("{YELLOW}restarting{OFF} {unit}");

    let Ok(_) = machine::user_systemctl(&["restart", unit]);
  }

  Ok(())
}

fn mask_units(manifest: &Manifest) -> Result<(), Never> {
  let Ok(masked) = manifest.of(Section::Masked);

  for unit in masked {
    let Ok((enabled, _)) = machine::unit_state(unit);

    match ::core::primitive::bool::then_some(enabled != "masked", ()) {
      ::core::option::Option::Some(()) => {
        println!("{YELLOW}masking{OFF} {unit}");

        let Ok(_) = machine::user_systemctl(&["mask", unit]);
      }
      ::core::option::Option::None => {}
    }
  }

  Ok(())
}

fn commit(root: &Path) -> Result<String, Never> {
  let Ok(git) = Program::Git.name();
  let Ok(asked) = machine::run(&[
    git,
    "-C",
    &root.display().to_string(),
    "rev-parse",
    "--short",
    "HEAD",
  ]);

  Ok(asked.out)
}

fn marked(root: &Path) -> Result<String, Never> {
  let Ok(said) = commit(root);

  Ok(
    match ::core::primitive::bool::then_some(said.is_empty(), ()) {
      ::core::option::Option::Some(()) => console_boot_entries::APPLY.to_string(),
      ::core::option::Option::None => format!("{} {said}", console_boot_entries::APPLY),
    },
  )
}

fn told_what_there_is_to_come_back_to(was: &[snapshot::Snapshot]) -> Result<(), Never> {
  println!("{YELLOW}before{OFF}");

  for held in was {
    let Ok(said) = held.describe();

    match held {
      snapshot::Snapshot::Made { .. } => {
        let Ok(()) = line(
          GREEN,
          StatusLine {
            state: "kept",
            about: &said,
          },
        );
      }
      snapshot::Snapshot::Not { .. } => {
        let Ok(()) = line(
          YELLOW,
          StatusLine {
            state: "no snapshot",
            about: &said,
          },
        );
      }
    }
  }

  Ok(())
}

fn packed_the_add_on() -> Result<(), Never> {
  println!("{YELLOW}packing{OFF} the browser's own add-on");

  let Ok(whom) = machine::whoever();
  let home = format!("HOME=/home/{whom}");
  let Ok(runuser) = Program::Runuser.name();
  let Ok(env) = Program::Env.name();
  let Ok(_) = machine::run_seen(&[runuser, "-u", whom, "--", env, &home, "console-web"]);

  Ok(())
}

fn pressed_the_wallpapers() -> Result<(), Never> {
  println!("{YELLOW}pressing{OFF} the wallpapers the table names and this has not");

  let Ok(_) = machine::run_seen(&["wallpaper-render"]);

  Ok(())
}

fn told_the_browsers() -> Result<(), Never> {
  println!("{YELLOW}telling{OFF} the browsers");

  let Ok(whoever) = machine::whoever();
  let home = format!("HOME=/home/{whoever}");
  let Ok(env) = Program::Env.name();
  let Ok(_) = machine::run_seen(&[env, &home, "console-engine"]);

  Ok(())
}

fn told_the_front(root: &Path) -> Result<(), Never> {
  let Ok(home) = home();
  let Ok(standing) = buttons::check_buttons(root, &home);
  let Ok(settled) = standing.up_to_date();

  match ::core::primitive::bool::then_some(!standing.asked || settled == UpToDate::Yes, ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let Ok(summary) = standing.summary();
  let Ok(body) = standing.body();

  println!("{YELLOW}saying{OFF} {summary}");

  let Ok(()) = machine::in_the_session(&["console-say", "buttons", &summary, &body]);

  match ::core::primitive::bool::then_some(!standing.told, ()) {
    ::core::option::Option::Some(()) => {
      println!("{YELLOW}asking{OFF} which buttons this device has");

      let Ok(()) = machine::in_the_session(&["mapping-panel", "--first"]);
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn home() -> Result<String, Never> {
  let Ok(whoever) = machine::whoever();

  Ok(format!("/home/{whoever}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restart {
  Wanted,
  No,
}

fn restarted_by(source: &Path, unit: &str, written: &[String]) -> Result<Restart, Never> {
  let its_own = format!("/etc/systemd/user/{unit}");

  match ::core::primitive::bool::then_some(written.contains(&its_own), ()) {
    ::core::option::Option::Some(()) => return Ok(Restart::Wanted),
    ::core::option::Option::None => {}
  }

  let from = match install::source_of(source, &its_own) {
    Ok(from) => from,
    Err(_invalid_unit_path) => return Ok(Restart::Wanted),
  };
  let held = match console_core_atomic_writes::read_text(&from) {
    Ok(said) => said,
    Err(_unreadable) => return Ok(Restart::Wanted),
  };
  let Ok(named) = units::named_by(&held);
  #[cfg_attr(
    dylint_lib = "explicit028_no_search_in_a_loop",
    allow(
      explicit028_no_search_in_a_loop,
      reason = "the units one file declares, which is one or two of them, asked of what this apply wrote"
    )
  )]
  let its_program = named.iter().any(|program| written.contains(program));

  Ok(match ::core::primitive::bool::then_some(its_program, ()) {
    ::core::option::Option::Some(()) => Restart::Wanted,
    ::core::option::Option::None => Restart::No,
  })
}

fn fallen(units: &[&String]) -> Result<Vec<String>, Never> {
  Ok(
    units
      .iter()
      .filter(|unit| {
        let Ok((_, active)) = machine::unit_state(unit);

        active == "failed"
      })
      .map(|unit| unit.to_string())
      .collect(),
  )
}

fn swept(manifest: &Manifest) -> Result<(), Never> {
  let Ok(files) = manifest.of(Section::Files);
  let Ok(built) = manifest.builds();
  let claimed = files.iter().cloned().chain(built.map(|name| {
    let Ok(live) = build::live(name);

    live
  }));

  for live in claimed {
    let Ok(()) = machine::drop_staged(&live);
    let Ok(()) = machine::drop_kept(&live);
  }

  Ok(())
}

fn stage_programs(
  root: &Path,
  manifest: &Manifest,
  programs: build::Programs,
  deploy: &mut Deploy,
  here: &mut machine::Here,
  moving: &mut going::Moving,
) -> Result<Vec<String>, Unapplied> {
  use console_core_state_machine::{Machine, Transition};
  use program_staging::{Effect, Input, ProgramStaging, Report, State};

  let Ok(names) = manifest.builds();
  let programs_to_stage = names.collect::<Vec<_>>();
  let Ok(first) = ProgramStaging::initial_transition(
    &Input {
      programs,
      names: &programs_to_stage,
    },
    None,
  );
  let ended = console_core_iteration::iterate(
    (first, deploy, here, moving),
    |(turn, deploy_2, here_2, moving_2)| {
      let Transition {
        state: current,
        effects,
      } = turn;
      let pending: Result<[Effect<'_>; 1], _> = effects.try_into();
      let (state, effect) = match (current, pending) {
        (State::Finished(result), _) => return Ok(Step::Halt(result)),
        (waiting @ (State::Preparing | State::Staging), Ok([effect])) => (waiting, effect),
        (State::Preparing | State::Staging, Err(_effects)) => {
          return Ok(Step::Halt(Err(Unapplied::ProgramStagingStopped)));
        }
      };
      let reply = match effect {
        Effect::Build(requested) => Report::Prepared(
          compile(root, requested.iter().copied(), moving_2).map(|()| {
            let Ok(artifacts) = changed_artifacts(root, requested);

            artifacts
          }),
        ),
        Effect::UsePrebuilt(requested) => {
          let Ok(()) = moving_2.say("Using the programs supplied with this release.");
          let Ok(artifacts) = changed_artifacts(root, requested);

          Report::Prepared(Ok(artifacts))
        }
        Effect::Stage(artifacts) => {
          Report::Staged(stage_artifacts(artifacts, deploy_2, here_2, moving_2))
        }
      };
      let Ok(next) = ProgramStaging::transition(state, reply);

      Ok(Step::Again((next, deploy_2, here_2, moving_2)))
    },
  );

  ended.map_err(|_endless| Unapplied::ProgramStagingStopped)?
}

fn changed_artifacts(
  root: &Path,
  names: &[manifest::BuildName<'_>],
) -> Result<Vec<program_staging::Artifact>, Never> {
  Ok(
    names
      .iter()
      .copied()
      .filter(|name| {
        let Ok(state) = build::state(root, *name);
        let Ok(settled) = state.up_to_date();

        settled == UpToDate::No
      })
      .map(|name| {
        let Ok(live) = build::live(name);
        let Ok(built) = build::built_path(root, name);

        program_staging::Artifact { built, live }
      })
      .collect(),
  )
}

fn stage_artifacts(
  artifacts: Vec<program_staging::Artifact>,
  deploy: &mut Deploy,
  here: &mut machine::Here,
  moving: &mut going::Moving,
) -> Result<Vec<String>, Unapplied> {
  let Ok(many) = fitted::<_, u32>(artifacts.len());

  let mut staged: Vec<String> = Vec::new();

  for (done, program_staging::Artifact { built, live }) in artifacts.into_iter().enumerate() {
    let Ok(()) = moving.say(&format!("{YELLOW}staging{OFF} {live}"));
    let Ok(done_2) = fitted::<_, u32>(done);
    let Ok(()) = moving.at(Progress { done: done_2, many }, &live);

    deploy.stage(here, &built, &live)?;

    staged.push(live);
  }

  Ok(staged)
}

fn compile<'a>(
  root: &Path,
  names: impl Iterator<Item = manifest::BuildName<'a>> + Clone,
  moving: &mut going::Moving,
) -> Result<(), Unapplied> {
  let announced = names
    .clone()
    .map(|name| {
      let Ok(text) = name.as_str();

      text
    })
    .collect::<Vec<_>>();
  let Ok(()) = moving.say(&format!("{YELLOW}building{OFF} {}", announced.join(" ")));
  let Ok(how) = build::how(names);
  let Ok(cargo_name) = Program::Cargo.name();
  let arguments: Vec<&str> = [cargo_name]
    .into_iter()
    .chain(how.iter().map(String::as_str))
    .collect();
  let built = cargo(root, &arguments, moving)?;

  match ::core::primitive::bool::then_some(built.success(), ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(Unapplied::CargoRefused),
  }
}

fn cargo(
  root: &Path,
  arguments: &[&str],
  moving: &mut going::Moving,
) -> Result<std::process::ExitStatus, Unapplied> {
  use std::io::{BufRead, BufReader, IsTerminal};

  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Err(Unapplied::NoProgram),
  };

  let mut starting = std::process::Command::new(program);
  starting
    .args(rest)
    .current_dir(root)
    .stderr(std::process::Stdio::piped());

  match ::core::primitive::bool::then_some(std::io::stderr().is_terminal(), ()) {
    ::core::option::Option::Some(()) => {
      starting.args(["--color", "always"]);
    }
    ::core::option::Option::None => {}
  }

  let mut child = console_operating_system_linux::program_lifetime::alongside(&mut starting)
    .map_err(Unapplied::CargoUnrun)?;

  let Ok(erring) = child.take_stderr();

  match erring {
    Some(said) => {
      let (say, heard) = std::sync::mpsc::sync_channel(64);
      let reading = std::thread::spawn(move || {
        for line in BufReader::new(said).lines().map_while(Result::ok) {
          match say.send(line) {
            Ok(()) => {}
            Err(_closed) => break,
          }
        }
      });
      let _disconnected = console_core_iteration::iterate(
        (&mut *moving, 0.0, String::new()),
        |(moving_2, steps, crate_name)| {
          let (step, crate_name_2) = match heard.recv_timeout(building::TICK) {
            Ok(line) => {
              let Ok(names) = building::names_a_crate(&line);
              let Ok(()) = moving_2.say(&line);

              match names {
                Names::ACrate(name) => (1.0, name),
                Names::SomethingElse => (0.0, crate_name),
              }
            }

            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (building::A_TICK, crate_name),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(Step::Halt(())),
          };
          let steps_2 = steps + step;

          let Ok(far) = building::far(building::CrateSteps(steps_2));
          let Ok(()) = moving_2.far(far, &crate_name_2);

          Ok(Step::Again((moving_2, steps_2, crate_name_2)))
        },
      );

      #[cfg_attr(
        dylint_lib = "explicit057_no_unbounded_wait",
        expect(
          explicit057_no_unbounded_wait,
          reason = "the loop has observed channel disconnection, so the sole sender and its stderr reader have already stopped"
        )
      )]
      match reading.join() {
        Ok(()) => {}
        Err(_reader_panicked) => eprintln!("console: the cargo progress reader panicked"),
      }
    }
    None => {}
  }

  child.wait().map_err(Unapplied::CargoUnwaited)
}

fn write(
  source: &Path,
  manifest: &Manifest,
  deploy: &mut Deploy,
  here: &mut machine::Here,
  moving: &mut going::Moving,
) -> Result<Vec<String>, Unapplied> {
  let mut staged = Vec::new();
  let Ok(whoever) = machine::whoever();
  let Ok(files) = manifest.of(Section::Files);
  let Ok(many) = fitted::<_, u32>(files.len());

  for (done, path) in files.iter().enumerate() {
    let Ok(done_2) = fitted::<_, u32>(done);
    let Ok(()) = moving.at(Progress { done: done_2, many }, path);

    let Ok(written) = manifest.write_policy(path);
    let Ok(state) = install::state(source, path, User(whoever), written);

    match state {
      install::State::Ok | install::State::WrittenOnce => {}
      install::State::Unsourced => {
        let Ok(()) = moving.say(&format!("{RED}no source for{OFF} {path}"));
      }
      install::State::Differs | install::State::NotFound | install::State::ParseError => {
        let Ok(()) = moving.say(&format!("{YELLOW}staging{OFF} {path}"));

        let from = install::source_of(source, path)?;

        deploy.stage(here, &from, path)?;
        staged.push(path.clone());
      }
    }
  }

  Ok(staged)
}

fn rebuttoned(_root: &Path, _manifest: &Manifest) -> Result<(), Unapplied> {
  let Ok(root_is) = nix_is_root();

  match ::core::primitive::bool::then_some(root_is == Root::No, ()) {
    ::core::option::Option::Some(()) => return Err(Unapplied::AsRoot("buttons")),
    ::core::option::Option::None => {}
  }

  let Ok(wrote) = buttons::wrote_router();

  match wrote {
    Some(live) => {
      println!("{YELLOW}writing{OFF} {live}");

      let Ok(()) = buttons::wear_again();
    }
    None => println!("This machine would not say what buttons it has."),
  }

  Ok(())
}

fn save(root: &Path, manifest: &Manifest, asked: &[String]) -> Result<(), Unapplied> {
  let _alone = alone::taking()?;
  let source = root.join("files");
  let Ok(whoever) = machine::whoever();
  let Ok(files) = manifest.of(Section::Files);
  let wanted: Vec<String> = match asked {
    [] => files
      .iter()
      .filter(|path| {
        let Ok(written) = manifest.write_policy(path);
        let Ok(state) = install::state(&source, path, User(whoever), written);

        state == install::State::Differs
      })
      .cloned()
      .collect(),
    asked_2 => asked_2.to_vec(),
  };

  match ::core::primitive::bool::then_some(wanted.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      println!("Nothing differs from the source.");
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let mut taken: Vec<PathBuf> = Vec::new();

  for path in &wanted {
    let Ok(declared) = install::as_declared(path, User(whoever));
    let Ok(on) = install::on_machine(&declared, User(whoever));
    let into = install::source_of(&source, &declared)?;

    let held = match console_core_atomic_writes::read_bytes(Path::new(&on)) {
      Ok(held) => held,
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ())
        {
          ::core::option::Option::Some(()) => {
            println!("{RED}not on the machine{OFF} {path}");
            continue;
          }
          ::core::option::Option::None => return Err(Unapplied::At(path.clone(), fault)),
        }
      }
    };
    let Ok(content) = install::content_as_declared(&held, User(whoever));

    console_core_atomic_writes::whole_with_folders(&into, &content)
      .map_err(|fault| Unapplied::Saving(path.clone(), fault))?;
    println!("{YELLOW}saved{OFF} {path}");
    taken.push(into);
  }

  let Ok(()) = machine::commit(root, "save", &taken);

  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Root {
  Yes,
  No,
}

fn nix_is_root() -> Result<Root, Never> {
  let Ok(id) = Program::Id.name();
  let Ok(said) = machine::run(&[id, "-u"]);

  Ok(
    match ::core::primitive::bool::then_some(said.out == "0", ()) {
      ::core::option::Option::Some(()) => Root::Yes,
      ::core::option::Option::None => Root::No,
    },
  )
}

#[cfg(test)]
mod tests {
  use super::{Reason, Unapplied, ValidationError, asked, build, on_the_ordinary_root};
  use std::path::Path;

  #[test]
  fn switching_checkouts_requires_prebuilt_apply_and_the_live_root() -> Result<(), ValidationError>
  {
    let words = |args: &[&str]| args.iter().map(|word| word.to_string()).collect::<Vec<_>>();

    let available = asked(&words(&[
      "apply",
      "--prebuilt",
      "--switch-from",
      "/candidate",
    ]))?;

    assert_eq!(available.programs, build::Programs::Prebuilt);
    assert_eq!(
      available.switch_from.as_deref(),
      Some(Path::new("/candidate"))
    );
    assert!(matches!(
      asked(&words(&["apply", "--switch-from", "/candidate"])),
      Err(ValidationError {
        reason: Reason::InvalidValue {
          of: "--switch-from",
          ..
        },
        ..
      })
    ));
    assert!(matches!(
      asked(&words(&["check", "--prebuilt"])),
      Err(ValidationError {
        reason: Reason::InvalidValue {
          of: "--prebuilt",
          ..
        },
        ..
      })
    ));
    assert!(matches!(
      asked(&words(&[
        "apply",
        "--prebuilt",
        "--switch-from",
        "/candidate",
        "--root",
        "/other"
      ])),
      Err(ValidationError {
        reason: Reason::InvalidValue {
          of: "--switch-from",
          ..
        },
        ..
      })
    ));

    Ok(())
  }

  #[test]
  fn the_snapshot_check_can_read_this_process_mounts() {
    let result = on_the_ordinary_root();

    assert!(
      matches!(result, Ok(()) | Err(Unapplied::OnASnapshot(_))),
      "{result:?}"
    );
  }
}
