//! Moving a device that is still called legion over to the console names.
//!
//! The rename in this repository is a rename of strings. On a machine that has
//! already been applied to, the old names are enabled units, installed
//! binaries, a checkout at `/etc/legion` and directories in a home with
//! someone's own answers in them. Deploying the renamed tree without this
//! leaves a machine running seven units nothing declares any more, beside
//! seven new ones that want the same devices.
//!
//! Nothing here is deleted. Everything the old names left behind is moved into
//! a private attic under `/var/tmp`, and the last line says where
//! it is. The `migrations.md` note in Orbit is where the rule this taught was
//! written down: the manifest installs a name and never sweeps one.
//!
//! It has been run, once, and the attic it left is still on the device. It
//! stays anyway. A migration is not deleted when it is spent -- a machine that
//! was never brought over is a machine this is the only way back for, and a
//! sweep that exists only in a commit message is a sweep no one can run.
//!
//! ## The plan is a value
//!
//! Everything after the question is [`plan`], a list of pieces in the order
//! they have to happen, worked out from the host, the person the desktop
//! belongs to, and where the attic is. That order is the whole of what this
//! program knows and every claim the old script made in a numbered comment --
//! the history before the engine, the units down before the tree moves, the
//! apply before the sweep, the enable last -- is a test over that list rather
//! than a sentence beside a line of shell.
//!
//! ## Run it from somewhere else
//!
//! The desktop is down between the disable and the enable, so a machine doing
//! this from its own screen has no way of finishing the job.

use console_core_arguments::{CommandLine, NoSubcommand, Operands};
use console_core_external_programs::{Program, shell};
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_places::{APPLICATION, Base};
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Choice, Command, Effect, Event, Exit, ExitStatus, Prompt};
use console_session::reaching;

use crate::deploying::How;
use crate::deploying::{CHECK, SAID_NO, TREE, YES, how};
use console_device_name::HOST;

pub const WAS: &str = "/etc/legion";

pub const UNITS: [&str; 7] = [
  "bar",
  "controller",
  "keyboard",
  "paper",
  "polkit",
  "session",
  "sky",
];

pub const THEN: &str = "legion";

pub const BESIDE: &str = ".librewolf";

pub fn homes() -> Result<Vec<(String, String)>, Never> {
  let under = Base::EVERY.into_iter().map(|base| {
    let Ok(usual) = base.usual();

    usual.to_string()
  });

  Ok(
    under
      .chain([BESIDE.to_string()])
      .map(|under_2| {
        (
          format!("{under}/{THEN}", under = under_2),
          format!("{under}/{APPLICATION}", under = under_2),
        )
      })
      .collect(),
  )
}

pub const SWEPT: [(&str, &str); 5] = [
  ("bin", "/usr/local/bin/legion /usr/local/bin/legion-*"),
  ("", "/usr/local/lib/legion"),
  (
    "share",
    "/usr/share/applications/legion-*.desktop /usr/share/backgrounds/legion.webp \
         /usr/share/icons/legion-placeholder.svg",
  ),
  (
    "etc",
    "/etc/sudoers.d/legion /etc/udev/rules.d/90-legion-backlight.rules \
         /etc/udev/rules.d/91-legion-touchpad.rules \
         /etc/chromium/policies/managed/legion-search.json",
  ),
  (
    "units",
    "/etc/systemd/user/legion.target /etc/systemd/user/legion-*.service \
         /etc/systemd/user/gamescope-session.service.d/legion.conf",
  ),
];

pub const PICTURES: (&str, &str) = (
  "/usr/share/backgrounds/legion",
  "/usr/share/backgrounds/console",
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
  pub says: Option<String>,
  pub runs: Command,
  pub shown: Shown,
  pub matters: Matters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
  Screen,
  Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matters {
  Yes,
  No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Going {
  pub host: String,
  pub how: How,
  pub whom: String,
  pub uid: u32,
  pub home: std::path::PathBuf,
  pub tree: Tree,
  pub old: Vec<String>,
  pub attic: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tree {
  Nowhere,
  Was,
  Now,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
  Owning,
  OwningUid,
  Numbering,
  Standing,
  Naming,
  Listing,
  Homing,
  Locally,
  Committed,
  Browsing,
  Wondering,
  Atticking,
  Walking(Vec<Piece>),
  Checking,
  Remoting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migrating {
  Nowhere,
  At(Step, Going),
}

pub const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "console-migrate",
  about: "move the device CONSOLE_HOST names off the names it was installed under",
  flags: &[CHECK, YES],
  operands: Operands::None,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
  pub host: String,
  pub how: How,
}

impl Migration {
  pub fn of(host: &str, line: &CommandLine<NoSubcommand>) -> Result<Migration, Never> {
    let (Ok(check), Ok(yes)) = (line.presence(CHECK), line.presence(YES));
    let Ok(how) = how(check, yes);

    Ok(Migration {
      host: host.to_string(),
      how,
    })
  }
}

pub struct Migrate;

impl Machine for Migrate {
  type Input = Migration;
  type State = Migrating;
  type Request = Event<console_core_never::Never>;
  type Effect = Effect<console_core_never::Never>;

  fn initialize(
    migration: &Migration,
    _previous: Option<Migrating>,
    effects: &mut Effects,
  ) -> Result<Migrating, Never> {
    let Ok(opening) = initial(migration);

    opening.offered(effects)
  }

  fn handle(
    state: Migrating,
    event: Event<console_core_never::Never>,
    effects: &mut Effects,
  ) -> Result<Migrating, Never> {
    let Ok(decided) = decide(&state, &event);

    decided.offered(effects)
  }
}

type Effects = Queue<Effect<console_core_never::Never>>;

fn initial(
  migration: &Migration,
) -> Result<Transition<Migrating, Effect<console_core_never::Never>>, Never> {
  let Ok(opening) = match ::core::primitive::bool::then_some(migration.host.trim().is_empty(), ())
  {
    ::core::option::Option::Some(()) => Transition::without_effects(Migrating::Nowhere),
    ::core::option::Option::None => Transition::without_effects(Migrating::At(
      Step::Owning,
      Going {
        host: migration.host.clone(),
        how: migration.how,
        whom: String::new(),
        uid: 0,
        home: std::path::PathBuf::new(),
        tree: Tree::Nowhere,
        old: Vec::new(),
        attic: String::new(),
      },
    )),
  };

  Ok(opening)
}

fn decide(
  state: &Migrating,
  event: &Event<console_core_never::Never>,
) -> Result<Transition<Migrating, Effect<console_core_never::Never>>, Never> {
  let Ok(turn) = match (state, event) {
    (Migrating::Nowhere, Event::Opened) => Transition::new(
      state.clone(),
      vec![Effect::Stop(Exit::Failure(format!(
        "{HOST} is not set, so there is no device to talk to. Set it to the device, \
                 as in {HOST}=root@handheld."
      )))],
    ),

    (Migrating::At(step, going), word) => at(step, going, word),

    (Migrating::Nowhere, _) => Transition::without_effects(state.clone()),
  };

  Ok(turn)
}

fn at(
  step: &Step,
  going: &Going,
  event: &Event<console_core_never::Never>,
) -> Result<Transition<Migrating, Effect<console_core_never::Never>>, Never> {
  let Ok(at) = here(step, going);

  match (step, event) {
    (Step::Owning, Event::Opened) => {
      let Ok(homes) = console_device_name::homes_command();
      let Ok(runs) = on(going, &homes);

      Transition::new(at.clone(), vec![Effect::Run(runs)])
    }

    (Step::Owning, Event::Replied(answer)) => {
      let Ok(owner) = console_device_name::sole_home(&answer.output);

      match (answer.status, owner) {
        (ExitStatus::Success, Some(whom)) => numbering(going, whom),
        _ => {
          let Ok(fallback) = console_device_name::fallback_command();
          let Ok(runs) = on(going, &fallback);

          Transition::new(
            Migrating::At(Step::OwningUid, going.clone()),
            vec![Effect::Run(runs)],
          )
        }
      }
    }

    (Step::OwningUid, Event::Replied(answer)) => {
      let Ok(owner) = console_device_name::account(&answer.output);

      match (answer.status, owner) {
        (ExitStatus::Success, Some(whom)) => numbering(going, whom),
        _ => stopped(
          step,
          going,
          &format!("no account on {} to own the desktop", going.host),
        ),
      }
    }

    (Step::Numbering, Event::Replied(answer)) => {
      let Ok(account) = match answer.status {
        ExitStatus::Success => login(AccountRecord {
          line: &answer.output,
          expected: &going.whom,
        }),
        ExitStatus::Failure(_) | ExitStatus::Interrupt => Ok(None),
      };

      let (uid, home) = match account {
        Some(resolved) => resolved,
        None => {
          return stopped(
            step,
            going,
            "the account could not be resolved to a UID and home",
          );
        }
      };

      let going_6 = Going {
        uid,
        home,
        ..going.clone()
      };

      let Ok(runs) = on(
        &going_6,
        &format!("test -d {TREE} && echo now; test -d {WAS} && echo was; true"),
      );
      Transition::new(
        Migrating::At(Step::Naming, going_6.clone()),
        vec![
          Effect::Print(format!("== what {} is called now", going_6.host)),
          Effect::Run(runs),
        ],
      )
    }

    (Step::Naming, Event::Replied(answer)) => {
      let Ok(asked) = parse_tree(&answer.output);
      let going_3 = Going {
        tree: asked,
        ..going.clone()
      };

      let Ok(runs) = once(&going_3, "systemctl --user list-unit-files --no-legend");
      let Ok(said) = where_(going_3.tree);
      Transition::new(
        Migrating::At(Step::Listing, going_3.clone()),
        vec![
          Effect::Print(format!("  the tree        {}", said)),
          Effect::Run(runs),
        ],
      )
    }

    (Step::Listing, Event::Replied(answer)) => {
      let Ok(old) = names_like(&answer.output, Like("legion"));
      let Ok(new) = names_like(&answer.output, Like("console"));
      let going_4 = Going {
        old: old.clone(),
        ..going.clone()
      };

      let Ok(said) = looking_in_a_home(&going_4);
      let Ok(runs) = on(&going_4, &said);
      let Ok(was) = listed(&old);
      let Ok(now) = listed(&new);

      Transition::new(
        Migrating::At(Step::Homing, going_4.clone()),
        vec![
          Effect::Print(format!("  units, old      {was}")),
          Effect::Print(format!("  units, new      {now}")),
          Effect::Run(runs),
        ],
      )
    }

    (Step::Homing, Event::Replied(answer)) => {
      let Ok(runs) = on(
        going,
        "ls -d /usr/local/bin/legion* /usr/local/lib/legion 2>/dev/null",
      );

      let Ok(asked) = lines(&answer.output);
      let Ok(said) = listed(&asked);
      Transition::new(
        Migrating::At(Step::Locally, going.clone()),
        vec![
          Effect::Print(format!("  in a home       {}", said)),
          Effect::Run(runs),
        ],
      )
    }

    (Step::Locally, Event::Replied(answer)) => after_local_listing(&at, going, &answer.output),

    (Step::Committed, Event::Replied(answer)) => {
      match ::core::primitive::bool::then_some(answer.output.trim().is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(runs) = once(going, &format!("pgrep -u {} -x librewolf", going.whom));

          Transition::new(
            Migrating::At(Step::Browsing, going.clone()),
            vec![Effect::Run(runs)],
          )
        }
        ::core::option::Option::None => stopped(
          step,
          going,
          "there are changes here that are not committed; the machine is brought to a \
                 commit, not to a working tree",
        ),
      }
    }

    (Step::Browsing, Event::Replied(answer)) => match answer.status {
      ExitStatus::Success => stopped(
        step,
        going,
        &format!(
          "librewolf is running on {}. Close it: its profile directory moves.",
          going.host
        ),
      ),
      ExitStatus::Failure(_) | ExitStatus::Interrupt => match going.how {
        How::Yes => {
          let Ok(said) = making_an_attic();

          let Ok(runs) = on(going, &said);
          Transition::new(
            Migrating::At(Step::Atticking, going.clone()),
            vec![Effect::Run(runs)],
          )
        }
        How::Confirm | How::Check => {
          let Ok(runs) = carding(going);

          Transition::new(
            Migrating::At(Step::Wondering, going.clone()),
            vec![Effect::Run(runs)],
          )
        }
      },
    },

    (Step::Wondering, Event::Replied(answer)) => match answer.status {
      ExitStatus::Success => taking(going),
      ExitStatus::Failure(Some(SAID_NO)) => stopped(step, going, "nothing done"),
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        let Ok(asking) = Prompt::unless(
          &format!(
            "\nmigrate {} to the console names? the desktop goes down for it [y/N]",
            going.host
          ),
          Choice::No,
        );

        Transition::new(
          at.clone(),
          vec![
            Effect::Print(format!(
              "{} could not raise a card, so the question is here instead",
              going.host
            )),
            Effect::Prompt(asking),
          ],
        )
      }
    },

    (Step::Wondering, Event::Chosen(Choice::Yes)) => taking(going),

    (Step::Wondering, Event::Chosen(Choice::No)) => stopped(step, going, "nothing done"),

    (Step::Atticking, Event::Replied(answer)) => {
      let attic = answer.output.trim().to_string();

      match ::core::primitive::bool::then_some(attic.is_empty(), ()) {
        ::core::option::Option::Some(()) => stopped(
          step,
          going,
          "the device could not make an attic to sweep into",
        ),
        ::core::option::Option::None => {
          let going_5 = Going {
            attic: attic.clone(),
            ..going.clone()
          };
          let Ok(walking) = plan(&going_5);
          let Ok(sending) = sending(&walking);

          Transition::new(
            Migrating::At(Step::Walking(walking.clone()), going_5),
            std::iter::once(Effect::Print(format!("\n== the attic is {attic}")))
              .chain(sending)
              .collect(),
          )
        }
      }
    }

    (Step::Walking(left), Event::Replied(answer)) => {
      let first = left.first();
      let rest: Vec<Piece> = left.iter().skip(1).cloned().collect();
      let matters = first.map(|piece| piece.matters);

      match (matters, answer.status) {
        (Some(Matters::Yes), ExitStatus::Failure(_)) => stopped(
          step,
          going,
          "a step of the migration would not go, and the machine is halfway",
        ),
        (Some(_), _) | (None, _) => match ::core::primitive::bool::then_some(rest.is_empty(), ())
        {
          ::core::option::Option::None => {
            let Ok(said) = sending(&rest);

            Transition::new(
              Migrating::At(Step::Walking(rest.clone()), going.clone()),
              said,
            )
          }
          ::core::option::Option::Some(()) => {
            let Ok(runs) = on(going, "console check");

            Transition::new(
              Migrating::At(Step::Checking, going.clone()),
              vec![
                Effect::Print("\n== what the machine says about itself".to_string()),
                Effect::Stream(runs),
              ],
            )
          }
        },
      }
    }

    (Step::Checking, Event::Replied(_)) => {
      let Ok(remoting) = Command::external(
        Program::Git,
        &[
          "remote",
          "set-url",
          "device",
          &format!("ssh://{}{TREE}", going.host),
        ],
      );

      Transition::new(
        Migrating::At(Step::Remoting, going.clone()),
        vec![Effect::Run(remoting)],
      )
    }

    (Step::Remoting, Event::Replied(_)) => Transition::new(
      at.clone(),
      vec![
        Effect::Print(format!(
          "\n{} is on the console names.\n\
                     What the old ones left is in {}. Look at the desktop, then empty it.\n\
                     Other clones need: git remote set-url device ssh://{}{TREE}",
          going.host, going.attic, going.host
        )),
        Effect::Stop(Exit::Success),
      ],
    ),

    (_, _) => Transition::without_effects(at.clone()),
  }
}

fn numbering(going: &Going, whom: &str) -> Result<Transition<Migrating, Effect<Never>>, Never> {
  let going_2 = Going {
    whom: whom.to_owned(),
    ..going.clone()
  };
  let Ok(getent) = Program::Getent.name();
  let Ok(asked) = shell::join([getent, "--", "passwd", whom]);
  let Ok(runs) = on(&going_2, &asked);

  Transition::new(
    Migrating::At(Step::Numbering, going_2),
    vec![Effect::Run(runs)],
  )
}

pub fn plan(going: &Going) -> Result<Vec<Piece>, Never> {
  let mut every = Vec::new();

  for stage in [
    the_engine,
    the_old_units,
    the_directories,
    the_old_names,
    the_new_units,
  ] {
    let Ok(pieces) = stage(going);

    every.extend(pieces);
  }

  Ok(every)
}

fn the_engine(going: &Going) -> Result<Vec<Piece>, Never> {
  let Ok(tree) = where_(going.tree);
  let Ok(runs) = on(
    going,
    &format!("git -C {tree} config receive.denyCurrentBranch updateInstead"),
  );
  let Ok(history) = run("\n== the history", runs, Matters::Yes);
  let Ok(pushes) = Command::external(
    Program::Git,
    &[
      "push",
      &format!("ssh://{}{tree}", going.host),
      "HEAD:master",
    ],
  );
  let Ok(pushing) = piece(pushes, Shown::Screen, Matters::Yes);
  let Ok(runs_2) = on(
    going,
    &format!("cd {tree} && cargo build --release --locked --bin console"),
  );
  let Ok(engine) = run("\n== the engine", runs_2, Matters::Yes);
  let Ok(runs_3) = on(
    going,
    &format!("install -m 755 {tree}/target/release/console /usr/local/bin/console"),
  );
  let Ok(installing) = piece(runs_3, Shown::Screen, Matters::Yes);

  Ok(vec![history, pushing, engine, installing])
}

fn the_old_units(going: &Going) -> Result<Vec<Piece>, Never> {
  let asked = UNITS
    .iter()
    .map(|unit| format!("systemctl --user disable --now legion-{unit}.service"))
    .chain([
      "systemctl --user disable --now syncthing.service".to_string(),
      "systemctl --user disable --now legion.target".to_string(),
      "systemctl --user daemon-reload".to_string(),
    ]);

  under(going, "\n== the old units go", asked)
}

fn the_directories(going: &Going) -> Result<Vec<Piece>, Never> {
  let mut every = Vec::new();

  let Ok(next) = message_piece("\n== the tree and the directories");
  every.push(next);

  match going.tree {
    Tree::Was => {
      let Ok(runs) = on(going, &format!("mv {WAS} {TREE}"));
      let Ok(next_2) = piece(runs, Shown::Back, Matters::Yes);

      every.push(next_2)
    }
    Tree::Now | Tree::Nowhere => {}
  }

  let Ok(homes) = homes();

  for (was, now) in homes {
    let Ok(console) = InternalProgram::Console.path();
    let Ok(as_them) = reaching::as_user(
      reaching::Whom(&going.whom),
      &[console, "migration-move-home", &was, &now],
    );
    let Ok(runs) = on(going, &as_them);
    let Ok(next_3) = letting(runs);

    every.push(next_3);
  }

  let Ok(said) = moving_the_pictures();
  let Ok(runs) = on(going, &said);
  let Ok(next_4) = letting(runs);
  every.push(next_4);

  let Ok(runs_2) = on(going, "console apply");
  let Ok(next_5) = run("\n== the apply", runs_2, Matters::Yes);
  every.push(next_5);

  Ok(every)
}

fn the_old_names(going: &Going) -> Result<Vec<Piece>, Never> {
  let mut every = Vec::new();

  let Ok(next) = message_piece("\n== the old names");
  every.push(next);

  for (into, patterns) in SWEPT {
    let destination = match into {
      "" => going.attic.clone(),
      category => format!("{}/{category}", going.attic),
    };
    let Ok(console) = InternalProgram::Console.path();
    let Ok(said) = shell::join(
      [console, "migration-sweep", destination.as_str()]
        .into_iter()
        .chain(patterns.split_whitespace()),
    );
    let Ok(runs) = on(going, &said);
    let Ok(next_2) = letting(runs);

    every.push(next_2);
  }

  let Ok(runs) = on(going, "udevadm control --reload");
  let Ok(next_3) = letting(runs);
  every.push(next_3);

  Ok(every)
}

fn the_new_units(going: &Going) -> Result<Vec<Piece>, Never> {
  let asked = std::iter::once("systemctl --user daemon-reload".to_string())
    .chain(
      UNITS
        .iter()
        .map(|unit| format!("systemctl --user enable console-{unit}.service")),
    )
    .chain([
      "systemctl --user enable syncthing.service".to_string(),
      "systemctl --user enable --now console.target".to_string(),
    ]);

  under(going, "\n== the new units", asked)
}

fn under(
  going: &Going,
  heading: &str,
  asked: impl Iterator<Item = String>,
) -> Result<Vec<Piece>, Never> {
  let Ok(said) = message_piece(heading);

  Ok(
    std::iter::once(said)
      .chain(asked.map(|command| {
        let Ok(runs) = once(going, &command);
        let Ok(next) = letting(runs);

        next
      }))
      .collect(),
  )
}

fn sending(left: &[Piece]) -> Result<Vec<Effect<console_core_never::Never>>, Never> {
  Ok(match left.first() {
    Some(piece) => piece
      .says
      .iter()
      .map(|says| Effect::Print(says.clone()))
      .chain(std::iter::once(match piece.shown {
        Shown::Screen => Effect::Stream(piece.runs.clone()),
        Shown::Back => Effect::Run(piece.runs.clone()),
      }))
      .collect(),
    None => Vec::new(),
  })
}

fn taking(
  going: &Going,
) -> Result<Transition<Migrating, Effect<console_core_never::Never>>, Never> {
  let Ok(said) = making_an_attic();
  let Ok(runs) = on(going, &said);
  Transition::new(
    Migrating::At(Step::Atticking, going.clone()),
    vec![Effect::Run(runs)],
  )
}

fn after_local_listing(
  state: &Migrating,
  going: &Going,
  output: &str,
) -> Result<Transition<Migrating, Effect<Never>>, Never> {
  let Ok(asked) = lines(output);
  let Ok(said) = listed(&asked);
  let told = Effect::Print(format!("  in /usr/local   {said}"));

  match going.how {
    How::Check => {
      return Transition::new(state.clone(), vec![told, Effect::Stop(Exit::Success)]);
    }
    How::Confirm | How::Yes => {}
  }

  let Ok(remaining) = nothing_to_do(going);

  match remaining {
    Some(reason) => {
      return Transition::new(
        state.clone(),
        vec![
          told,
          Effect::Print(format!("\n{reason}")),
          Effect::Stop(Exit::Success),
        ],
      );
    }
    None => {}
  }

  match going.tree {
    Tree::Nowhere => Transition::new(
      state.clone(),
      vec![
        told,
        Effect::Stop(Exit::Failure(format!(
          "there is no checkout on {} to migrate",
          going.host
        ))),
      ],
    ),
    Tree::Was | Tree::Now => {
      let Ok(standing) = Command::external(Program::Git, &["status", "--porcelain"]);

      Transition::new(
        Migrating::At(Step::Committed, going.clone()),
        vec![told, Effect::Run(standing)],
      )
    }
  }
}

fn nothing_to_do(going: &Going) -> Result<Option<String>, Never> {
  Ok(match (going.tree, going.old.is_empty()) {
    (Tree::Now, true) => Some(format!(
      "{} is already called console. Nothing to do.",
      going.host
    )),
    (Tree::Now, false) | (Tree::Was, _) | (Tree::Nowhere, _) => None,
  })
}

fn parse_tree(said: &str) -> Result<Tree, Never> {
  let Ok(words) = lines(said);
  let now = words.iter().any(|word| word == "now");
  let was = words.iter().any(|word| word == "was");

  Ok(match (now, was) {
    (true, _) => Tree::Now,
    (false, true) => Tree::Was,
    (false, false) => Tree::Nowhere,
  })
}

pub fn where_(tree: Tree) -> Result<&'static str, Never> {
  Ok(match tree {
    Tree::Now => TREE,
    Tree::Was => WAS,
    Tree::Nowhere => "none",
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Like<'a>(&'a str);

fn names_like(said: &str, like: Like<'_>) -> Result<Vec<String>, Never> {
  Ok(
    said
      .lines()
      .filter_map(|line| line.split_whitespace().next())
      .filter(|name| name.starts_with(like.0))
      .map(str::to_string)
      .collect(),
  )
}

fn lines(said: &str) -> Result<Vec<String>, Never> {
  Ok(
    said
      .lines()
      .map(str::trim)
      .filter(|line| !line.is_empty())
      .map(str::to_string)
      .collect(),
  )
}

fn listed(what: &[String]) -> Result<String, Never> {
  Ok(
    match ::core::primitive::bool::then_some(what.is_empty(), ()) {
      ::core::option::Option::Some(()) => "none".to_string(),
      ::core::option::Option::None => what.join(" "),
    },
  )
}

struct AccountRecord<'a> {
  line: &'a str,
  expected: &'a str,
}

fn login(
  AccountRecord { line, expected }: AccountRecord<'_>,
) -> Result<Option<(u32, std::path::PathBuf)>, Never> {
  let mut fields = line.trim_end().split(':');

  match bool::then_some(fields.next() == Some(expected), ()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let user_id = fields.nth(1).map(str::parse::<u32>);
  let directory = fields.nth(2).map(std::path::Path::new);

  Ok(match (user_id, directory) {
    (Some(Ok(uid)), Some(home)) => home.is_absolute().then(|| (uid, home.to_owned())),
    (Some(Err(_non_numeric_uid)), _) => None,
    (None, _) | (_, None) => None,
  })
}

struct HomeEntry<'a>(&'a std::path::Path);

impl<'a> HomeEntry<'a> {
  fn relative(text: &'a str) -> Result<Option<Self>, Never> {
    use std::path::{Component, Path};
    let entry = Path::new(text);
    let normal = !text.is_empty()
      && entry
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    Ok(bool::then_some(normal, Self(entry)))
  }
}

impl AsRef<std::path::Path> for HomeEntry<'_> {
  fn as_ref(&self) -> &std::path::Path {
    self.0
  }
}

fn looking_in_a_home(going: &Going) -> Result<String, Never> {
  let Ok(homes) = homes();
  let Ok(ls) = Program::Ls.name();
  let paths: Vec<_> = homes
    .iter()
    .filter_map(|(was, _)| {
      let Ok(entry) = HomeEntry::relative(was);
      entry.map(|relative| going.home.join(relative))
    })
    .collect();
  let words: Vec<_> = [ls, "-d", "--"]
    .into_iter()
    .chain(paths.iter().filter_map(|path| path.to_str()))
    .collect();

  reaching::as_user(reaching::Whom(&going.whom), &words)
}

fn moving_the_pictures() -> Result<String, Never> {
  let (was, now) = PICTURES;
  let Ok(console) = InternalProgram::Console.path();

  shell::join([console, "migration-move-pictures", was, now])
}

fn making_an_attic() -> Result<String, Never> {
  let Ok(mktemp) = Program::Mktemp.name();
  let Ok(template) = console_core_temporary_directories::migration_attic_template();

  shell::join([mktemp, "-d", "--", template])
}

const ASKS: &str = "Rename what is installed on this device? The desktop closes and comes back.";

const DOES: &str = "Rename";

fn carding(going: &Going) -> Result<Command, Never> {
  let Ok(in_session) = reaching::confirmation(
    reaching::Whom(&going.whom),
    reaching::Confirmation {
      label: DOES,
      question: ASKS,
    },
  );

  on(going, &in_session)
}

fn once(going: &Going, command: &str) -> Result<Command, Never> {
  let Ok(runuser) = Program::Runuser.name();
  let Ok(env) = Program::Env.name();

  on(
    going,
    &format!(
      "{runuser} -u {} -- {env} XDG_RUNTIME_DIR=/run/user/{} {command}",
      going.whom, going.uid
    ),
  )
}

fn on(going: &Going, command: &str) -> Result<Command, Never> {
  Command::external(Program::Ssh, &["--", going.host.as_str(), command])
}

fn piece(runs: Command, shown: Shown, matters: Matters) -> Result<Piece, Never> {
  Ok(Piece {
    says: None,
    runs,
    shown,
    matters,
  })
}

fn run(says: &str, runs: Command, matters: Matters) -> Result<Piece, Never> {
  Ok(Piece {
    says: Some(says.to_string()),
    runs,
    shown: Shown::Screen,
    matters,
  })
}

fn letting(runs: Command) -> Result<Piece, Never> {
  Ok(Piece {
    says: None,
    runs,
    shown: Shown::Back,
    matters: Matters::No,
  })
}

fn message_piece(says: &str) -> Result<Piece, Never> {
  let runs = Command::external(Program::True, &[])?;

  Ok(Piece {
    says: Some(says.to_string()),
    runs,
    shown: Shown::Back,
    matters: Matters::No,
  })
}

fn here(step: &Step, going: &Going) -> Result<Migrating, Never> {
  Ok(Migrating::At(step.clone(), going.clone()))
}

fn stopped(
  step: &Step,
  going: &Going,
  why: &str,
) -> Result<Transition<Migrating, Effect<console_core_never::Never>>, Never> {
  let Ok(at) = here(step, going);

  Transition::new(at, vec![Effect::Stop(Exit::Failure(why.to_string()))])
}

#[cfg(test)]
mod tests {
  use console_core_state_machine::run;
  use console_program_contract::Answer;

  use super::*;
  use std::error::Error;

  fn handheld(how: How) -> Result<Migration, Never> {
    Ok(Migration {
      host: "root@handheld".to_string(),
      how,
    })
  }

  fn position<T>(list: &[T], wanted: impl Fn(&T) -> bool) -> Result<Option<u32>, Never> {
    Ok(
      (0..)
        .zip(list)
        .find(|(_, one)| wanted(one))
        .map(|(at, _)| at),
    )
  }

  #[test]
  fn the_old_name_is_moved_out_of_the_bases_it_was_ever_under() {
    let Ok(homes) = homes();

    let was: Vec<&str> = homes.iter().map(|(was, _)| was.as_str()).collect();

    assert_eq!(
      was,
      vec![
        ".config/legion",
        ".local/state/legion",
        ".local/share/legion",
        ".cache/legion",
        ".librewolf/legion",
      ],
      "a machine applied to under the old name has these and no others, so a base that \
             arrives after the rename must not turn up here: nothing of the old name was ever \
             under it"
    );

    for (was_2, now) in homes {
      assert_eq!(
        now,
        was_2.replace(THEN, APPLICATION),
        "a move that renames more than the name"
      );
    }
  }

  fn going() -> Result<Going, Never> {
    Ok(Going {
      host: "root@handheld".to_string(),
      how: How::Yes,
      whom: "someone".to_string(),
      uid: 1000,
      home: <::std::path::PathBuf as ::core::convert::From<&::core::primitive::str>>::from(
        "/home/someone",
      ),
      tree: Tree::Was,
      old: vec!["legion-bar.service".to_string()],
      attic: "/home/console-test/console-migration-20260829-234115".to_string(),
    })
  }

  #[derive(Clone, Copy)]
  enum Reply {
    Success(&'static str),
    Failure(i32),
  }

  fn replies(given: &[Reply]) -> Result<Vec<Event<Never>>, Never> {
    let Ok(runs) = Command::external(Program::Ssh, &["root@handheld", "true"]);
    let mut events = vec![Event::Opened];

    for reply in given {
      let (output, status) = match *reply {
        Reply::Success(said) => (said.to_string(), ExitStatus::Success),
        Reply::Failure(code) => (String::new(), ExitStatus::Failure(Some(code))),
      };

      events.push(Event::Replied(Answer {
        job: None,
        command: runs.clone(),
        output,
        error_output: String::new(),
        status,
      }));
    }

    Ok(events)
  }

  const LOOKING: [Reply; 6] = [
    Reply::Success("someone/"),
    Reply::Success("someone:x:1000:1000::/home/someone:/bin/bash"),
    Reply::Success("was"),
    Reply::Success("legion-bar.service enabled\nconsole-bar.service enabled\n"),
    Reply::Success(""),
    Reply::Success("/usr/local/bin/legion-bar"),
  ];

  fn looking(then: &[Reply]) -> Result<Vec<Event<Never>>, Never> {
    let every: Vec<Reply> = LOOKING.iter().chain(then).copied().collect();

    replies(&every)
  }

  fn words(going: &Going) -> Result<Vec<String>, Never> {
    let Ok(plan) = plan(going);

    Ok(
      plan
        .iter()
        .map(|piece| piece.runs.arguments.join(" "))
        .collect(),
    )
  }

  fn where_in(going: &Going, like: &str) -> Result<Option<u32>, Never> {
    let Ok(words) = words(going);

    position(&words, |word| word.contains(like))
  }

  #[test]
  fn a_machine_that_names_no_device_reaches_for_nothing() {
    let Ok(said) = run::<Migrate>(
      &Migration {
        host: String::new(),
        how: How::Confirm,
      },
      &[Event::Opened],
    );
    let Ok(effects) = said.effects();

    assert!(
      effects
        .iter()
        .all(|effect| !matches!(effect, Effect::Run(_)))
    );
  }

  #[test]
  fn a_check_says_what_the_machine_is_called_and_changes_nothing() {
    let Ok(arguments) = handheld(How::Check);
    let Ok(events) = looking(&[]);
    let Ok(said) = run::<Migrate>(&arguments, &events);
    let Ok(effects) = said.effects();
    let asked: Vec<String> = effects
      .iter()
      .filter_map(|effect| match effect {
        Effect::Run(runs) | Effect::Stream(runs) => runs.arguments.last().cloned(),
        Effect::Spawn(_)
        | Effect::Fork(_)
        | Effect::Interrupt(_)
        | Effect::Prompt(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_)
        | Effect::Custom(_) => None,
      })
      .collect();

    assert!(
      asked.iter().all(|word| !word.contains("mv ")),
      "a check moved something"
    );
    assert!(
      asked.iter().all(|word| !word.contains("disable")),
      "a check stopped a unit"
    );
    assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Success))));
  }

  #[test]
  fn a_machine_already_on_the_new_names_is_told_so_and_left_alone() {
    let Ok(arguments) = handheld(How::Yes);
    let Ok(events) = replies(&[
      Reply::Success("someone/"),
      Reply::Success("someone:x:1000:1000::/home/someone:/bin/bash"),
      Reply::Success("now"),
      Reply::Success("console-bar.service enabled\n"),
      Reply::Success(""),
      Reply::Success(""),
    ]);
    let Ok(said) = run::<Migrate>(&arguments, &events);
    let Ok(effects) = said.effects();

    assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Success))));
    assert!(
      effects
        .iter()
        .any(|effect| matches!(effect, Effect::Print(line)
                if line.contains("already called console")))
    );
  }

  #[test]
  fn a_tree_with_uncommitted_work_is_refused_before_the_desktop_goes_down() {
    let Ok(said) = looking(&[Reply::Success(" M justfile\n")]);

    let Ok(arguments) = handheld(How::Yes);
    let Ok(said_2) = run::<Migrate>(&arguments, &said);
    let Ok(effects) = said_2.effects();

    assert!(matches!(
      effects.last(),
      Some(Effect::Stop(Exit::Failure(_)))
    ));
  }

  #[test]
  fn a_browser_that_is_running_is_refused_because_its_profile_moves() {
    let Ok(said) = looking(&[Reply::Success(""), Reply::Success("4242")]);

    let Ok(arguments) = handheld(How::Yes);
    let Ok(said_2) = run::<Migrate>(&arguments, &said);
    let Ok(effects) = said_2.effects();

    assert!(
      matches!(effects.last(), Some(Effect::Stop(Exit::Failure(why)))
                if why.contains("librewolf"))
    );
  }

  #[test]
  fn without_a_yes_the_question_goes_to_the_device_and_no_does_nothing() {
    let Ok(said) = looking(&[
      Reply::Success(""),
      Reply::Failure(1),
      Reply::Failure(SAID_NO),
    ]);

    let Ok(arguments) = handheld(How::Confirm);
    let Ok(said_2) = run::<Migrate>(&arguments, &said);
    let Ok(effects) = said_2.effects();

    assert!(
      matches!(effects.last(), Some(Effect::Stop(Exit::Failure(why)))
                if why == "nothing done")
    );
  }

  #[test]
  fn a_device_with_no_card_to_raise_asks_at_this_terminal_instead() {
    let Ok(said) = looking(&[
      Reply::Success(""),
      Reply::Failure(1),
      Reply::Failure(crate::deploying::NO_CARD),
    ]);

    let Ok(arguments) = handheld(How::Confirm);
    let Ok(said_2) = run::<Migrate>(&arguments, &said);
    let Ok(effects) = said_2.effects();

    assert!(
      effects
        .iter()
        .any(|effect| matches!(effect, Effect::Prompt(_)))
    );
  }

  #[test]
  fn the_history_reaches_the_tree_where_it_still_stands_before_the_engine_is_built() {
    let Ok(going) = going();
    let Ok(pushed) = where_in(&going, "HEAD:master");
    let Ok(built) = where_in(&going, "cargo build");

    assert!(
      pushed
        .zip(built)
        .is_some_and(|(first_step, second_step)| first_step < second_step)
    );
    let Ok(words) = words(&going);

    assert!(
      words
        .iter()
        .any(|word| word.contains(&format!("ssh://root@handheld{WAS}"))),
      "the history was pushed to a path the tree is not at yet"
    );
  }

  #[test]
  fn the_engine_is_built_at_the_old_path_and_installed_before_the_tree_moves() {
    let Ok(going) = going();
    let Ok(put) = where_in(&going, "install -m 755");
    let Ok(moved) = where_in(&going, &format!("mv {WAS} {TREE}"));

    assert!(
      put
        .zip(moved)
        .is_some_and(|(first_step, second_step)| first_step < second_step)
    );
  }

  #[test]
  fn every_old_unit_goes_down_before_the_tree_moves_under_it() {
    let Ok(going) = going();
    let Ok(moved) = where_in(&going, &format!("mv {WAS} {TREE}"));

    for unit in UNITS {
      let Ok(down) = where_in(&going, &format!("disable --now legion-{unit}.service"));

      assert!(down.is_some(), "legion-{unit} was never stopped");
      assert!(
        down
          .zip(moved)
          .is_some_and(|(first_step, second_step)| first_step < second_step)
      );
    }
  }

  #[test]
  fn the_apply_happens_after_the_directories_have_moved_and_before_the_sweep() {
    let Ok(going) = going();
    let Ok(moved) = where_in(&going, ".config/legion");
    let Ok(applied) = where_in(&going, "console apply");
    let Ok(swept) = where_in(&going, "/usr/local/bin/legion-*");

    assert!(
      moved
        .zip(applied)
        .is_some_and(|(first_step, second_step)| first_step < second_step)
    );
    assert!(
      applied
        .zip(swept)
        .is_some_and(|(first_step, second_step)| first_step < second_step)
    );
  }

  #[test]
  fn everything_swept_goes_into_the_attic_and_nothing_is_deleted() -> Result<(), Box<dyn Error>> {
    let Ok(going) = going();
    let Ok(words) = words(&going);

    for word in &words {
      assert!(
        !word.contains("rm -rf") && !word.split_whitespace().any(|part| part == "rm"),
        "the migration deleted something: {word}"
      );
    }

    for (_, patterns) in SWEPT {
      let first = patterns
        .split_whitespace()
        .next()
        .ok_or("an empty pattern")?;

      assert!(
        words
          .iter()
          .find(|word| word.contains(first))
          .is_some_and(|word| word.contains(&going.attic)),
        "{first} was not swept into the attic"
      );
    }

    Ok(())
  }

  #[test]
  fn the_new_units_are_enabled_last_and_the_target_last_of_all() {
    let Ok(going) = going();
    let Ok(every) = words(&going);
    let Ok(swept) = where_in(&going, "/etc/systemd/user/legion.target");
    let Ok(target) = position(&every, |word| word.contains("enable --now console.target"));

    for unit in UNITS {
      let Ok(up) = where_in(&going, &format!("enable console-{unit}.service"));

      assert!(
        up.zip(swept)
          .is_some_and(|(first_step, second_step)| second_step < first_step)
      );
      assert!(
        up.zip(target)
          .is_some_and(|(first_step, second_step)| first_step < second_step)
      );
    }

    assert!(
      every
        .last()
        .is_some_and(|last| last.contains("enable --now console.target"))
    );
    assert_eq!(
      every
        .iter()
        .filter(|word| word.contains("enable --now console.target"))
        .count(),
      1
    );
  }

  #[test]
  fn syncthing_goes_down_with_the_target_that_pulled_it_and_comes_back_with_the_new_one() {
    let Ok(going) = going();
    let Ok(down) = where_in(&going, "disable --now syncthing.service");
    let Ok(up) = where_in(&going, "enable syncthing.service");

    assert!(
      down
        .zip(up)
        .is_some_and(|(first_step, second_step)| first_step < second_step)
    );
  }

  #[test]
  fn a_machine_already_at_the_new_path_is_not_asked_to_move_a_tree_that_is_not_there() {
    let Ok(was) = going();
    let going = Going {
      tree: Tree::Now,
      ..was
    };

    assert_eq!(where_in(&going, &format!("mv {WAS}")), Ok(None));
    assert!(matches!(where_in(&going, "console apply"), Ok(Some(_))));
  }

  #[test]
  fn a_step_that_matters_stops_the_migration_where_it_stands() {
    let Ok(said) = looking(&[
      Reply::Success(""),
      Reply::Failure(1),
      Reply::Success("/home/console-test/console-migration-20260829-234115"),
      Reply::Failure(1),
    ]);

    let Ok(arguments) = handheld(How::Yes);
    let Ok(said_2) = run::<Migrate>(&arguments, &said);
    let Ok(effects) = said_2.effects();

    assert!(matches!(
      effects.last(),
      Some(Effect::Stop(Exit::Failure(_)))
    ));
  }
}
