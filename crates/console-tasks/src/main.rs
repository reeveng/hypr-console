//! `cargo x`: the tasks, run.
//!
//! Everything here is an edge -- git asked, `cargo metadata` read, a file
//! read to see whether it walks out of its crate, a step started and waited
//! for. What any of it means is decided in the library. Every step runs in a
//! control group of its own, the way `just alone` ran it, so what a run starts
//! at whatever depth is stopped with it rather than found weeks later.

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_metadata::DependencyKind;
use console_core_arguments::CommandLine;
use console_core_atomic_writes::Stored;
use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;
use console_operating_system_linux::program_lifetime::alongside;
use console_operating_system_linux::program_runtime::{self, Clock, Interpreter};
use console_operating_system_linux::systemd::{Scopes, nothing_left_in, scopes};
use console_program_contract::{Event, Exit};
use console_tasks::reached::{
  self, Affected, Change, Dependency, Kind, LOCK, MANIFEST, Member, Package, Scope,
};
use console_tasks::ready::{Before, Checkout, Ready, ReadyEffect, ReadyEvent, Run, Step, Tree};
use console_tasks::task::{self, Suite, Task};

mod guest;

const MOST: &str = "MemoryHigh=32G";

const THREADS: &str = "TasksMax=8192";

const ARCHITECTURE: &str = "target/architecture";

const FACTS: &str = "target/architecture/facts";

const CHECKED: &str = "target/architecture/dylint/target";

const MAP_SOURCE: &str = "docs/architecture/map.dot";

const MAP_PICTURE: &str = "docs/architecture/map.svg";

const PASSED: &str = "console-ready";

const KERNEL: &str = "console-kernel";

const FIRMWARE: &str = "x86_64-unknown-uefi";

const WALKS_OUT: &str = "../..";

enum TaskError {
  Unstarted(String, io::Error),
  Failed(String),
  Unread(String),
  Metadata(String),
  NoRoot(String),
  Unwritten(String),
  Ready(String),
  Test(String),
  NoToolchain,
}

impl fmt::Display for TaskError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      TaskError::Unstarted(what, why) => write!(to, "could not start {what}: {why}"),
      TaskError::Failed(what) => write!(to, "{what} did not pass"),
      TaskError::Unread(what) => write!(to, "could not read what {what} answered"),
      TaskError::Metadata(why) => {
        write!(to, "cargo metadata said something this cannot read: {why}")
      }
      TaskError::NoRoot(why) => write!(to, "not inside this tree: {why}"),
      TaskError::Unwritten(why) => write!(to, "the pass could not be written down: {why}"),
      TaskError::Ready(why) | TaskError::Test(why) => to.write_str(why),
      TaskError::NoToolchain => write!(
        to,
        "rustup is not on PATH, and the lint suite asks it which toolchain it is"
      ),
    }
  }
}

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let line = match console_core_arguments::read_with::<Task, String>(&task::COMMAND, &words) {
    Ok(line) => line,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };
  let ran = run(&line);

  match ran {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("cargo x: {fault}");

      ExitCode::FAILURE
    }
  }
}

fn run(line: &CommandLine<Task>) -> Result<(), TaskError> {
  let Ok(named) = line.subcommand();
  let Ok(rest) = line.operands();
  let found = match named {
    Some(task) => task,
    None => Task::Help,
  };

  match found {
    Task::Help => {
      let Ok(said) = task::help();

      println!("{said}");

      Ok(())
    }
    Task::Test => {
      let root = workspace_root()?;
      let run = test_run(rest)?;

      ready(&root, &run)
    }
    Task::GuestTest => guest::run(rest).map_err(|fault| TaskError::Test(fault.to_string())),
    Task::GuestWorkspace => guest::link(rest).map_err(|fault| TaskError::Test(fault.to_string())),
    Task::Ready => {
      let root = workspace_root()?;

      ready(&root, &Run::Deploy)
    }
    Task::Reached => {
      let root = workspace_root()?;
      let since = rest.first().cloned();
      let scope = reached_since(&root, since)?;
      let Ok(flags) = scope.flags();

      println!("{}", flags.join(" "));

      Ok(())
    }
    Task::Map => {
      let root = workspace_root()?;

      map(&root)
    }
    Task::Rules => match ::core::primitive::bool::then_some(rest.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(flags) = Scope::Workspace.flags();

        rules(&flags)
      }
      ::core::option::Option::None => rules(rest),
    },
    Task::Alone => alone(rest, &[]),
  }
}

fn workspace_root() -> Result<PathBuf, TaskError> {
  console_repository::root().map_err(|why| TaskError::NoRoot(why.to_string()))
}

fn alone(words: &[String], environment: &[(&str, &str)]) -> Result<(), TaskError> {
  let unit = format!("console-run-{}", std::process::id());
  let Ok(available) = scopes();
  let wrapped: Vec<String> = match available {
    Scopes::Available => {
      let Ok(said) = Program::SystemdRun.words(
        [
          "--user",
          "--scope",
          "--quiet",
          &format!("--unit={unit}"),
          "-p",
          MOST,
          "-p",
          THREADS,
          "--",
        ]
        .into_iter()
        .map(str::to_string)
        .chain(words.iter().cloned())
        .collect(),
      );

      said
    }
    Scopes::Unavailable => {
      eprintln!("no user manager here, so this run is not in a group of its own");

      words.to_vec()
    }
  };
  let ran = started(&wrapped, environment);
  let Ok(()) = nothing_left_in(&unit);

  ran
}

fn started(words: &[String], environment: &[(&str, &str)]) -> Result<(), TaskError> {
  let said = words.join(" ");
  let (program, rest) = words
    .split_first()
    .ok_or_else(|| TaskError::Failed(said.clone()))?;
  let mut command = Command::new(program);

  command.args(rest).envs(environment.iter().copied());

  let began = alongside(&mut command);
  let mut running = began.map_err(|why| TaskError::Unstarted(said.clone(), why))?;
  let waited = running.wait();
  let status = waited.map_err(|why| TaskError::Unstarted(said.clone(), why))?;

  match ::core::primitive::bool::then_some(status.success(), ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(TaskError::Failed(said)),
  }
}

fn cargo(arguments: &[&str]) -> Result<(), TaskError> {
  let Ok(words) =
    Program::Cargo.words(arguments.iter().map(|word| (*word).to_string()).collect());

  alone(&words, &[])
}

fn cargo_over(arguments: &[&str], scope: &Scope, after: &[&str]) -> Result<(), TaskError> {
  let Ok(flags) = scope.flags();
  let Ok(words) = Program::Cargo.words(
    arguments
      .iter()
      .map(|word| (*word).to_string())
      .chain(flags)
      .chain(after.iter().map(|word| (*word).to_string()))
      .collect(),
  );

  alone(&words, &[])
}

fn answer(program: Program, arguments: &[&str]) -> Result<String, TaskError> {
  let Ok(mut asking) = program.command();
  let Ok(name) = program.name();
  let said = format!("{name} {}", arguments.join(" "));
  let output = asking
    .args(arguments)
    .output()
    .map_err(|why| TaskError::Unstarted(said.clone(), why))?;

  match ::core::primitive::bool::then_some(output.status.success(), ()) {
    ::core::option::Option::Some(()) => {
      String::from_utf8(output.stdout).map_err(|_not_text| TaskError::Unread(said))
    }
    ::core::option::Option::None => Err(TaskError::Failed(said)),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
  Yes,
  No,
}

fn asked(program: Program, arguments: &[&str]) -> Result<Reply, TaskError> {
  let Ok(mut asking) = program.command();
  let status = asking
    .args(arguments)
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status()
    .map_err(|why| TaskError::Unstarted(arguments.join(" "), why))?;

  Ok(
    match ::core::primitive::bool::then_some(status.success(), ()) {
      ::core::option::Option::Some(()) => Reply::Yes,
      ::core::option::Option::None => Reply::No,
    },
  )
}

fn test_run(arguments: &[String]) -> Result<Run, TaskError> {
  let suites = match arguments {
    [] => task::QUICK.to_vec(),
    [suite, name] => {
      let Ok(panel) = Suite::Panels.word();

      return match (suite == panel).then_some(()) {
        Some(()) => match name.is_empty().then_some(()) {
          Some(()) => Ok(Run::Tests(vec![Suite::Panels])),
          None => Ok(Run::PanelTest(name.clone())),
        },
        None => invalid_test_selection(),
      };
    }
    [name] => match name.as_str() {
      "quick" => task::QUICK.to_vec(),
      "e2e" => task::E2E.to_vec(),
      "all" => Suite::VARIANTS.to_vec(),
      word => {
        let Ok(selected) = Suite::from_word(word);

        match selected {
          Some(suite) => vec![suite],
          None => return invalid_test_selection(),
        }
      }
    },
    _ => return invalid_test_selection(),
  };

  Ok(Run::Tests(suites))
}

fn invalid_test_selection<T>() -> Result<T, TaskError> {
  let choices: Vec<&str> = Suite::VARIANTS
    .iter()
    .map(|suite| {
      let Ok(word) = suite.word();
      word
    })
    .collect();

  Err(TaskError::Test(format!(
    "expected quick, all, e2e, or one of {}",
    choices.join(", ")
  )))
}

fn tree_state() -> Result<Checkout, TaskError> {
  let now = answer(Program::Git, &["rev-parse", "HEAD^{tree}"])?;
  let loose = answer(Program::Git, &["status", "--porcelain"])?;
  let tree = match ::core::primitive::bool::then_some(loose.trim().is_empty(), ()) {
    ::core::option::Option::Some(()) => Tree::Committed,
    ::core::option::Option::None => Tree::Loose,
  };

  Ok(Checkout {
    tree_id: now.trim().to_string(),
    tree,
  })
}

fn passed_at() -> Result<PathBuf, TaskError> {
  let common = answer(Program::Git, &["rev-parse", "--git-common-dir"])?;

  Ok(Path::new(common.trim()).join(PASSED))
}

fn ready(root: &Path, run: &Run) -> Result<(), TaskError> {
  let completed =
    program_runtime::execute::<Ready, _>("cargo x", run, &mut ReadyInterpreter { root })
      .map_err(|fault| TaskError::Ready(fault.to_string()))?;

  match completed.exit {
    Exit::Success => Ok(()),
    Exit::Failure(fault) => Err(TaskError::Ready(fault)),
  }
}

struct ReadyInterpreter<'a> {
  root: &'a Path,
}

impl Interpreter for ReadyInterpreter<'_> {
  type Event = ReadyEvent;
  type Effect = ReadyEffect;
  type Error = Never;

  fn interpret(
    &mut self,
    effect: &ReadyEffect,
    _clock: &Clock,
  ) -> Result<Vec<Event<ReadyEvent>>, Never> {
    let reply = match self.perform(effect) {
      Ok(reply) => reply,
      Err(fault) => ReadyEvent::Failed(fault.to_string()),
    };

    Ok(vec![Event::Custom(reply)])
  }
}

impl ReadyInterpreter<'_> {
  fn perform(&self, effect: &ReadyEffect) -> Result<ReadyEvent, TaskError> {
    match effect {
      ReadyEffect::Inspect => inspect_ready(),
      ReadyEffect::FindScope(since) => {
        reached_since(self.root, since.clone()).map(ReadyEvent::ScopeFound)
      }
      ReadyEffect::Check(step) => Ok(ReadyEvent::Checked {
        step: step.clone(),
        result: ask(self.root, step).map_err(|fault| fault.to_string()),
      }),
      ReadyEffect::RecheckTree => tree_state().map(ReadyEvent::TreeRechecked),
      ReadyEffect::RecordPass { path, tree_id } => {
        console_core_atomic_writes::whole(path, format!("{tree_id}\n").as_bytes())
          .map_err(|why| TaskError::Unwritten(why.to_string()))?;

        Ok(ReadyEvent::PassRecorded)
      }
    }
  }
}

fn inspect_ready() -> Result<ReadyEvent, TaskError> {
  let checkout = tree_state()?;
  let pass_file = passed_at()?;
  let Ok(held) = console_core_atomic_writes::read(&pass_file);
  let last_pass = match held {
    Stored::Text(said) => Some(said),
    Stored::Absent => None,
    Stored::Failed(why) => {
      return Err(TaskError::Unread(format!("{}: {why}", pass_file.display())));
    }
  };

  Ok(ReadyEvent::Inspected {
    before: Before {
      checkout,
      pass_file,
    },
    last_pass,
  })
}

fn ask(root: &Path, step: &Step) -> Result<(), TaskError> {
  match step {
    Step::RetiredWords => cargo(&[
      "run",
      "--quiet",
      "--release",
      "--manifest-path",
      "tools/rename-words/Cargo.toml",
      "--",
      "check",
    ]),
    Step::Test(suite) => nix_test(root, *suite, None),
    Step::PanelTest(name) => nix_test(root, Suite::Panels, Some(name)),
    Step::Clippy(scope) => cargo_over(
      &["clippy", "--quiet", "--locked"],
      scope,
      &["--all-targets", "--all-features", "--", "-D", "warnings"],
    ),
    Step::Rules(scope) => {
      let Ok(flags) = scope.flags();

      rules(&flags)
    }
  }
}

fn nix_test(root: &Path, suite: Suite, panel: Option<&str>) -> Result<(), TaskError> {
  let Ok(name) = suite.word();
  let file = root.join("nix/test-checks.nix");
  let Ok(mut command) =
    console_package_manager_nix::build_test(console_package_manager_nix::TestBuild {
      file: &file,
      attribute: name,
      panel_filter: panel,
    });

  guest::checked(&mut command).map_err(|fault| TaskError::Test(fault.to_string()))
}

fn toolchain() -> Result<(), TaskError> {
  let Ok(name) = Program::Rustup.name();
  let Ok(found) = installed(name);

  match found {
    Installed::Yes => Ok(()),
    Installed::No => Err(TaskError::NoToolchain),
  }
}

fn rules(flags: &[String]) -> Result<(), TaskError> {
  toolchain()?;

  let metadata = answer(
    Program::Cargo,
    &[
      "metadata",
      "--no-deps",
      "--offline",
      "--locked",
      "--format-version",
      "1",
      "--manifest-path",
      "tools/explicit-rust/Cargo.toml",
    ],
  )?;
  let libraries = console_tasks::rules::libraries(&metadata)
    .map_err(|why| TaskError::Metadata(format!("the lint workspace: {why:?}")))?;
  let Ok(selection) = console_tasks::rules::selection(&libraries);

  println!(
    "rules: requiring all {} named lint libraries",
    libraries.len()
  );

  let Ok(words) = Program::Cargo.words(
    ["dylint"]
      .into_iter()
      .map(str::to_string)
      .chain(selection.iter().cloned())
      .chain(
        flags
          .iter()
          .filter(|flag| flag.as_str() == "--no-deps")
          .cloned(),
      )
      .chain(
        ["--", "--locked", "--all-targets", "--all-features"]
          .into_iter()
          .map(str::to_string),
      )
      .chain(
        flags
          .iter()
          .filter(|flag| flag.as_str() != "--no-deps")
          .cloned(),
      )
      .collect(),
  );

  alone(&words, &[])?;

  let firmware = flags
    .iter()
    .find(|flag| flag.as_str() == "--workspace" || flag.as_str() == KERNEL);

  match firmware {
    Some(_asked) => {
      let words_2: Vec<String> = ["dylint"]
        .into_iter()
        .map(str::to_string)
        .chain(selection)
        .chain(
          flags
            .iter()
            .filter(|flag| flag.as_str() == "--no-deps")
            .cloned(),
        )
        .chain(
          ["--", "--locked", "-p", KERNEL, "--target", FIRMWARE]
            .into_iter()
            .map(str::to_string),
        )
        .collect();
      let handed: Vec<&str> = words_2.iter().map(String::as_str).collect();

      cargo(&handed)
    }
    None => Ok(()),
  }
}

fn map(root: &Path) -> Result<(), TaskError> {
  toolchain()?;

  let kept = std::fs::read_dir(FACTS);

  match kept {
    Ok(_facts) => {}
    Err(_none_kept) => checked_from_nothing()?,
  }

  let facts = root.join(FACTS).display().to_string();
  let Ok(dylint) = Program::Cargo.words(
    [
      "dylint",
      "--quiet",
      "--path",
      "tools/explicit-rust",
      "--pattern",
      "architecture_facts",
      "--",
      "--quiet",
      "--locked",
      "--workspace",
      "--lib",
      "--bins",
      "--all-features",
    ]
    .into_iter()
    .map(str::to_string)
    .collect(),
  );

  alone(
    &dylint,
    &[
      ("CARGO_TARGET_DIR", ARCHITECTURE),
      ("CONSOLE_ARCHITECTURE_FACTS", &facts),
    ],
  )?;
  cargo(&[
    "run",
    "--quiet",
    "--locked",
    "--bin",
    "console-architecture",
    "--",
    &facts,
  ])?;

  let drawn = asked(Program::Dot, &["-Tsvg", MAP_SOURCE, "-o", MAP_PICTURE])?;

  match drawn {
    Reply::Yes => Ok(()),
    Reply::No => {
      eprintln!("map: dot would not draw, so {MAP_PICTURE} is as it was");

      Ok(())
    }
  }
}

fn checked_from_nothing() -> Result<(), TaskError> {
  let checked: Vec<PathBuf> = match std::fs::read_dir(CHECKED) {
    Ok(entries) => entries
      .flatten()
      .filter_map(|entry| match entry.file_type() {
        Ok(kind) => match ::core::primitive::bool::then_some(kind.is_dir(), ()) {
          ::core::option::Option::Some(()) => Some(entry.path()),
          ::core::option::Option::None => None,
        },
        Err(_unreadable) => None,
      })
      .collect(),
    Err(_absent) => Vec::new(),
  };

  for target in checked {
    let at = target.display().to_string();

    cargo(&["clean", "--quiet", "--workspace", "--target-dir", &at])?;
  }

  Ok(())
}

fn reached_since(root: &Path, since: Option<String>) -> Result<Scope, TaskError> {
  let revision = match since {
    Some(previous) => previous,
    None => return Ok(Scope::Workspace),
  };
  let held = asked(
    Program::Git,
    &["cat-file", "-e", &format!("{revision}^{{tree}}")],
  )?;

  match held {
    Reply::Yes => {}
    Reply::No => return Ok(Scope::Workspace),
  }

  let checkout = tree_state()?;
  let between = answer(
    Program::Git,
    &["diff", "--name-only", &revision, &checkout.tree_id],
  )?;
  let uncommitted = answer(Program::Git, &["diff", "--name-only", "HEAD"])?;
  let untracked = answer(
    Program::Git,
    &["ls-files", "--others", "--exclude-standard"],
  )?;
  let changed: Vec<String> = between
    .lines()
    .chain(uncommitted.lines())
    .chain(untracked.lines())
    .map(str::to_string)
    .collect();
  let packages = metadata(root)?;
  let built_from = affected_by_what_every_crate_is_built_from(&revision, &changed, &packages)?;
  let Ok(walking) = walking_out(root, &packages);
  let Ok(readers) = reached::readers(&packages, &walking);
  let Ok(scope) = reached::reached(&changed, &packages, &readers, &built_from);

  Ok(scope)
}

fn affected_by_what_every_crate_is_built_from(
  since: &str,
  changed: &[String],
  packages: &[Package],
) -> Result<Affected, TaskError> {
  let lock = match ::core::primitive::bool::then_some(changed.iter().any(|path| path == LOCK), ())
  {
    ::core::option::Option::Some(()) => {
      let before = from_history(&format!("{since}:{LOCK}"))?;
      let after = on_disk(LOCK)?;
      let Ok(affected) = reached::affected_by_lock(Change {
        before: &before,
        after: &after,
      });

      affected
    }
    ::core::option::Option::None => {
      let Ok(none) = Affected::none();

      none
    }
  };
  let manifest =
    match ::core::primitive::bool::then_some(changed.iter().any(|path| path == MANIFEST), ()) {
      ::core::option::Option::Some(()) => {
        let before = from_history(&format!("{since}:{MANIFEST}"))?;
        let after = on_disk(MANIFEST)?;
        let Ok(affected) = reached::affected_by_manifest(
          Change {
            before: &before,
            after: &after,
          },
          packages,
        );

        affected
      }
      ::core::option::Option::None => {
        let Ok(none) = Affected::none();

        none
      }
    };
  let Ok(both) = lock.and(manifest);

  Ok(both)
}

fn from_history(at: &str) -> Result<String, TaskError> {
  let asked = answer(Program::Git, &["show", at]);

  match asked {
    Ok(said) => Ok(said),
    Err(TaskError::Failed(_not_in_that_tree)) => Ok(String::new()),
    Err(fault) => Err(fault),
  }
}

fn on_disk(file: &str) -> Result<String, TaskError> {
  let Ok(held) = console_core_atomic_writes::read(Path::new(file));

  match held {
    Stored::Text(said) => Ok(said),
    Stored::Absent => Ok(String::new()),
    Stored::Failed(why) => Err(TaskError::Unread(format!("{file}: {why}"))),
  }
}

fn walking_out(root: &Path, packages: &[Package]) -> Result<BTreeSet<String>, Never> {
  Ok(packages
        .iter()
        .filter(|package| package.member == Member::Yes)
        .flat_map(|package| {
            #[cfg_attr(dylint_lib = "explicit086_no_climbing_join", expect(explicit086_no_climbing_join, reason = "package directories come from package(), which strips the checkout root and checks every remaining path component"))]
            let folder = root.join(&package.directory);
            let Ok(sources) = console_core_directory_listing::files(&folder.join("src"));
            let Ok(tests) = console_core_directory_listing::files(&folder.join("tests"));

            sources
                .into_iter()
                .chain(tests)
                .chain(std::iter::once(folder.join("build.rs")))
                .map(|file| (package.directory.clone(), file))
                .collect::<Vec<(String, PathBuf)>>()
        })
        .filter(|(_folder, file)| {
            let Ok(held) = console_core_atomic_writes::read(file);

            match held {
                Stored::Text(said) => said.contains(WALKS_OUT),
                Stored::Absent => false,
                Stored::Failed(_unread) => false,
            }
        })
        .map(|(folder, _file)| folder)
        .collect())
}

fn metadata(root: &Path) -> Result<Vec<Package>, TaskError> {
  let said = answer(
    Program::Cargo,
    &["metadata", "--format-version", "1", "--offline", "--locked"],
  )?;
  let read = console_tasks::rules::read_metadata(&said)
    .map_err(|why| TaskError::Metadata(format!("{why:?}")))?;
  let members: BTreeSet<_> = read.workspace_members.into_iter().collect();

  Ok(
    read
      .packages
      .into_iter()
      .filter(|entry| entry.source.is_none())
      .filter_map(|entry| {
        let Ok(found) = package(entry, root, &members);

        found
      })
      .collect(),
  )
}

fn package(
  read: cargo_metadata::Package,
  root: &Path,
  members: &BTreeSet<cargo_metadata::PackageId>,
) -> Result<Option<Package>, Never> {
  let folder = match read.manifest_path.as_std_path().parent() {
    Some(folder) => folder,
    None => return Ok(None),
  };
  let directory = match folder.strip_prefix(root) {
    Ok(relative) => match ::core::primitive::bool::then_some(
      !relative.as_os_str().is_empty()
        && relative
          .components()
          .all(|component| matches!(component, Component::Normal(_))),
      (),
    ) {
      ::core::option::Option::Some(()) => relative.to_str(),
      ::core::option::Option::None => None,
    },
    Err(_outside_checkout) => None,
  };
  let member = match ::core::primitive::bool::then_some(members.contains(&read.id), ()) {
    ::core::option::Option::Some(()) => Member::Yes,
    ::core::option::Option::None => Member::No,
  };

  Ok(directory.map(|under_root| {
    Package {
      name: read.name.to_string(),
      directory: under_root.to_string(),
      member,
      dependencies: read
        .dependencies
        .into_iter()
        .filter(|needed| needed.path.is_some())
        .map(|needed| {
          let kind = match needed.kind {
            DependencyKind::Development => Kind::Development,
            DependencyKind::Normal | DependencyKind::Build | DependencyKind::Unknown => {
              Kind::Normal
            }
          };

          Dependency {
            name: needed.name.to_string(),
            kind,
          }
        })
        .collect(),
    }
  }))
}
