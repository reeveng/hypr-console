//! Everything that touches the machine itself.
//!
//! Kept apart from the rest so that what decides and what does are two files.
//! Nothing here can be tested without a machine, which is exactly why nothing
//! here decides anything.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_places::Base;

use crate::install::{self, USER, User};
use crate::laying::{self, Back, Install};
use crate::modes;
use crate::unapplied::Unapplied;

pub struct Output {
  pub out: String,
}

pub fn run(arguments: &[&str]) -> Result<Output, Never> {
  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Ok(Output { out: String::new() }),
  };

  Ok(match Command::new(program).args(rest).output() {
    Ok(done) => match output_text(&done.stdout) {
      Ok(out) => Output { out },
      Err(fault) => {
        eprintln!("console: {program} returned invalid UTF-8: {fault}");

        Output { out: String::new() }
      }
    },
    Err(_would_not_start) => Output { out: String::new() },
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
  Fine,
  Badly,
}

pub struct CommandOutput {
  pub out: String,
  pub said: String,
  pub ran: Ran,
}

pub fn run_captured(arguments: &[&str]) -> Result<CommandOutput, Never> {
  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => {
      return Ok(CommandOutput {
        out: String::new(),
        said: String::new(),
        ran: Ran::Badly,
      });
    }
  };

  capture(Command::new(program).args(rest))
}

pub fn capture(command: &mut Command) -> Result<CommandOutput, Never> {
  let output = command.output();
  let program = command.get_program();

  Ok(match output {
    Ok(done) => match output_text(&done.stdout) {
      Ok(out) => {
        let said = match output_text(&done.stderr) {
          Ok(diagnostic) => diagnostic,
          Err(fault) => format!("{program:?} returned a diagnostic in invalid UTF-8: {fault}"),
        };
        let ran = match ::core::primitive::bool::then_some(done.status.success(), ()) {
          ::core::option::Option::Some(()) => Ran::Fine,
          ::core::option::Option::None => Ran::Badly,
        };

        CommandOutput { out, said, ran }
      }
      Err(fault) => CommandOutput {
        out: String::new(),
        said: format!("{program:?} returned invalid UTF-8: {fault}"),
        ran: Ran::Badly,
      },
    },
    Err(fault) => CommandOutput {
      out: String::new(),
      said: format!("{program:?}: {fault}"),
      ran: Ran::Badly,
    },
  })
}

fn output_text(bytes: &[u8]) -> Result<String, std::str::Utf8Error> {
  let text = std::str::from_utf8(bytes)?;

  Ok(text.trim().to_owned())
}

pub fn run_seen(arguments: &[&str]) -> Result<Ran, Never> {
  let (program, rest) = match arguments.split_first() {
    Some((program, rest)) => (program, rest),
    None => return Ok(Ran::Badly),
  };

  see(Command::new(program).args(rest))
}

pub fn see(command: &mut Command) -> Result<Ran, Never> {
  let done = match command.status() {
    Ok(done) => done.success(),
    Err(fault) => {
      eprintln!("console: starting {:?}: {fault}", command.get_program());

      false
    }
  };

  Ok(match ::core::primitive::bool::then_some(done, ()) {
    ::core::option::Option::Some(()) => Ran::Fine,
    ::core::option::Option::None => Ran::Badly,
  })
}

pub fn whoever() -> Result<&'static str, Never> {
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "an apply fixes the desktop owner on the first lookup; the process can only borrow that immutable result afterward"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
      explicit044_no_ambient_value,
      reason = "who the desktop belongs to is what `@user@` is filled in with, settled by the machine before anything is laid down and the same for every path an apply writes; the `crates.md` note in Orbit is why this question stays here rather than going to `console_core_places`"
    )
  )]
  static KNOWN: OnceLock<String> = OnceLock::new();

  Ok(KNOWN.get_or_init(|| {
    let Ok(one) = the_one_home();
    let desktop = one.or_else(|| {
      let Ok(already) = the_home_it_is_already_in();

      already
    });

    match desktop {
      Some(whose) => whose,
      None => {
        let Ok(numbered) = the_account_numbered_1000();

        match numbered {
          Some(whose) => whose,
          None => USER.to_string(),
        }
      }
    }
  }))
}

fn homes() -> Result<Vec<String>, Never> {
  let reading = match std::fs::read_dir("/home") {
    Ok(reading) => reading,
    Err(_unreadable) => return Ok(Vec::new()),
  };

  Ok(
    reading
      .flatten()
      .filter(|found| std::fs::metadata(found.path()).is_ok_and(|metadata| metadata.is_dir()))
      .filter_map(|found| match found.file_name().into_string() {
        Ok(name) => Some(name),

        Err(name) => {
          eprintln!("console: /home/{name:?}: not a name this desktop can act on");
          None
        }
      })
      .filter(|name| {
        let Ok(who) = who(name);

        who.is_some()
      })
      .collect(),
  )
}

fn the_one_home() -> Result<Option<String>, Never> {
  let Ok(mut homes) = homes();

  Ok(match homes.len() {
    1 => homes.pop(),
    _ => None,
  })
}

fn the_home_it_is_already_in() -> Result<Option<String>, Never> {
  let Ok(homes) = homes();
  let mut theirs: Vec<String> = homes
    .into_iter()
    .filter(|name| {
      let Ok(home) = home_path(User(name));
      let Ok(ours) = Base::Configuration.application_under(&home);

      std::fs::metadata(ours).is_ok_and(|metadata| metadata.is_dir())
    })
    .collect();

  Ok(match theirs.len() {
    1 => theirs.pop(),
    _ => None,
  })
}

fn the_account_numbered_1000() -> Result<Option<String>, Never> {
  let Ok(id) = Program::Id.name();
  let Ok(asked) = run(&[id, "-nu", "1000"]);
  let said = asked.out;

  match ::core::primitive::bool::then_some(said.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(None),
    ::core::option::Option::None => {}
  }

  println!("using the account numbered 1000 for the Console desktop: {said}");

  Ok(Some(said))
}

pub fn user_systemctl(words: &[&str]) -> Result<CommandOutput, Never> {
  let Ok(whoever) = whoever();
  let Ok(systemctl) = Program::Systemctl.name();

  let owned = format!("{whoever}@");
  let arguments: Vec<&str> = [systemctl, "--user", "-M", &owned]
    .into_iter()
    .chain(words.iter().copied())
    .collect();

  run_captured(&arguments)
}

pub fn mine(words: &[&str]) -> Result<Output, Never> {
  let Ok(systemctl) = Program::Systemctl.name();

  let arguments: Vec<&str> = [systemctl, "--user"]
    .into_iter()
    .chain(words.iter().copied())
    .collect();

  run(&arguments)
}

pub fn in_the_session(command: &[&str]) -> Result<(), Never> {
  let Ok(owner) = whoever();
  let Ok(who) = who(owner);

  let (uid, _taken_1) = match who {
    Some((uid, _taken_1)) => (uid, _taken_1),
    None => return Ok(()),
  };

  let runtime = format!("XDG_RUNTIME_DIR=/run/user/{uid}");
  let bus = format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{uid}/bus");
  let Ok(runuser) = Program::Runuser.name();
  let Ok(env) = Program::Env.name();
  let arguments: Vec<&str> = [runuser, "-u", owner, "--", env, &runtime, &bus]
    .into_iter()
    .chain(command.iter().copied())
    .collect();
  let Ok(_) = run(&arguments);

  Ok(())
}

pub fn unit_state(unit: &str) -> Result<(String, String), Never> {
  let Ok(enabled) = user_systemctl(&["is-enabled", unit]);
  let Ok(active) = user_systemctl(&["is-active", unit]);

  Ok((enabled.out, active.out))
}

pub fn disk_free(at: &Path) -> Result<Output, Never> {
  let Ok(df) = Program::Df.name();
  let where_it_is = at.display().to_string();

  run(&[df, "--block-size=1", "--output=avail", &where_it_is])
}

pub fn sizes_under(roots: &[&str]) -> Result<Output, Never> {
  let Ok(du) = Program::Du.name();

  let arguments: Vec<&str> = [du, "--block-size=1", "--one-file-system", "--max-depth=1"]
    .into_iter()
    .chain(roots.iter().copied())
    .collect();

  run(&arguments)
}

#[cfg_attr(
  dylint_lib = "explicit086_no_climbing_join",
  expect(
    explicit086_no_climbing_join,
    reason = "the account name is checked to be exactly one normal path component before joining"
  )
)]
pub fn home_path(user: User<'_>) -> Result<PathBuf, Never> {
  let name = std::ffi::OsStr::new(user.0);
  let path = Path::new(name);

  Ok(
    match ::core::primitive::bool::then_some(
      path.file_name() == Some(name) && !name.as_encoded_bytes().contains(&0),
      (),
    ) {
      ::core::option::Option::Some(()) => Path::new("/home").join(path),
      ::core::option::Option::None => PathBuf::from("/home/@user@"),
    },
  )
}

pub fn stage_file(from: &Path, live: &str) -> Result<(), Unapplied> {
  let Ok(whoever) = whoever();
  let Ok(on) = install::on_machine(live, User(whoever));
  let to = Path::new(&on);
  let complain =
    |what: &'static str, fault: std::io::Error| Unapplied::Staging(live.to_string(), what, fault);
  let held = staging_bytes(from).map_err(|fault| complain("reading it", fault))?;
  let Ok(holds) = install::parent_directories(&on);

  for holding in holds {
    match std::fs::create_dir(&holding) {
      Ok(()) => {}
      Err(fault) => match ::core::primitive::bool::then_some(
        fault.kind() == std::io::ErrorKind::AlreadyExists
          && std::fs::metadata(&holding).is_ok_and(|metadata| metadata.is_dir()),
        (),
      ) {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => return Err(complain("its directory", fault)),
      },
    }

    let Ok(owner) = install::owner_of(&holding, User(whoever));
    let Ok(who) = who(&owner);
    let (uid, gid) =
      who.ok_or_else(|| Unapplied::NoUserFor(live.to_string(), owner.to_string()))?;

    std::os::unix::fs::chown(&holding, Some(uid), Some(gid))
      .map_err(|fault| complain("its directory's owner", fault))?;
  }

  let Ok(staging) = laying::staged(to);

  let staged = match staging {
    Some(staged) => staged,
    None => {
      return Err(Unapplied::NothingBeside(
        live.to_string(),
        "stage one beside",
      ));
    }
  };

  let Ok(content) = install::content_on_machine(&held, User(whoever));

  let Ok(mode) = modes::of(live, &held);
  let Ok(permissions) = permissions(mode);
  console_core_atomic_writes::whole_with_permissions(&staged, &content, permissions)
    .map_err(|fault| Unapplied::Unwritten(live.to_string(), fault))?;

  let Ok(owner) = install::owner_of(live, User(whoever));
  let Ok(who) = who(&owner);
  let (uid, gid) =
    who.ok_or_else(|| Unapplied::NoUserFor(live.to_string(), owner.to_string()))?;

  std::os::unix::fs::chown(&staged, Some(uid), Some(gid))
    .map_err(|fault| complain("its owner", fault))
}

const MAX_STAGING_BYTES: u32 = 67_108_864;

fn staging_bytes(from: &Path) -> std::io::Result<Vec<u8>> {
  let held = console_core_atomic_writes::read_prefix(from, MAX_STAGING_BYTES.saturating_add(1))?;

  match u32::try_from(held.len()) {
    Ok(0..=MAX_STAGING_BYTES) => Ok(held),
    Ok(_) | Err(_) => Err(std::io::Error::from(std::io::ErrorKind::FileTooLarge)),
  }
}

pub fn swap_file(live: &str) -> Result<Back, Unapplied> {
  let Ok(whoever) = whoever();
  let Ok(on) = install::on_machine(live, User(whoever));
  let to = Path::new(&on);
  let complain =
    |what: &'static str, fault: std::io::Error| Unapplied::Staging(live.to_string(), what, fault);

  let Ok(beside) = laying::backup_path(to);
  let kept =
    beside.ok_or_else(|| Unapplied::NothingBeside(live.to_string(), "keep one beside"))?;

  let back = match std::fs::hard_link(to, &kept) {
    Ok(()) => Back::Retained,
    Err(fault) => {
      let kind = fault.kind();

      match (
        kind == std::io::ErrorKind::NotFound,
        kind == std::io::ErrorKind::AlreadyExists,
      ) {
        (true, _) => Back::Closed,
        (false, true) => {
          console_core_atomic_writes::gone(&kept).map_err(
            |console_core_atomic_writes::Ungone(_, removal)| {
              complain("removing an old backup", removal)
            },
          )?;
          std::fs::hard_link(to, &kept)
            .map_err(|link| complain("keeping what was there", link))?;

          Back::Retained
        }
        (false, false) => return Err(complain("keeping what was there", fault)),
      }
    }
  };

  let Ok(staging) = laying::staged(to);

  let staged = match staging {
    Some(staged) => staged,
    None => {
      return Err(Unapplied::NothingBeside(
        live.to_string(),
        "move one into place as",
      ));
    }
  };

  std::fs::rename(staged, to).map_err(|fault| complain("moving it into place", fault))?;

  match console_core_atomic_writes::sync_parent_directory(to) {
    Ok(()) => {}
    Err(fault) => eprintln!("console apply: {fault}"),
  }

  Ok(back)
}

pub fn put_back(laid: &Install) -> Result<(), Unapplied> {
  let Ok(whoever) = whoever();
  let Ok(on) = install::on_machine(&laid.at, User(whoever));
  let to = Path::new(&on);
  let complain =
    |what: &'static str, fault: std::io::Error| Unapplied::Staging(laid.at.clone(), what, fault);

  match laid.back {
    Back::Retained => {
      let Ok(beside) = laying::backup_path(to);

      let kept = match beside {
        Some(kept) => kept,
        None => {
          return Err(Unapplied::NothingKept(laid.at.clone()));
        }
      };

      std::fs::rename(kept, to).map_err(|fault| complain("putting back what was there", fault))
    }
    Back::Closed => {
      std::fs::remove_file(to).map_err(|fault| complain("taking away what this put there", fault))
    }
  }
}

fn drop_beside(
  live: &str,
  beside: fn(&Path) -> Result<Option<PathBuf>, Never>,
) -> Result<(), Never> {
  let Ok(whoever) = whoever();
  let Ok(on) = install::on_machine(live, User(whoever));
  let Ok(found) = beside(Path::new(&on));

  match found {
    Some(beside_2) => match console_core_atomic_writes::gone(&beside_2) {
      Ok(()) => {}
      Err(fault) => eprintln!("console apply: {fault}"),
    },
    None => {}
  }

  Ok(())
}

pub fn drop_staged(live: &str) -> Result<(), Never> {
  drop_beside(live, laying::staged)
}

pub fn drop_kept(live: &str) -> Result<(), Never> {
  drop_beside(live, laying::backup_path)
}

fn permissions(mode: u32) -> Result<std::fs::Permissions, Never> {
  use std::os::unix::fs::PermissionsExt;

  Ok(std::fs::Permissions::from_mode(mode))
}

pub fn handed_over(at: &Path) -> Result<(), Unapplied> {
  let Ok(whoever) = whoever();
  let Ok(who) = who(whoever);
  let (uid, gid) = who.ok_or_else(|| Unapplied::NoUser(whoever.to_string()))?;

  std::os::unix::fs::chown(at, Some(uid), Some(gid))
    .map_err(|fault| Unapplied::Owner(at.to_path_buf(), fault))
}

fn who(user: &str) -> Result<Option<(u32, u32)>, Never> {
  let number = |flag: &str| {
    let Ok(id) = Program::Id.name();
    let Ok(said) = run(&[id, flag, user]);

    let number = match said.out.parse::<u32>() {
      Ok(number) => number,
      Err(_not_a_number) => return None,
    };

    Some(number)
  };

  let uid = match number("-u") {
    Some(uid) => uid,
    None => return Ok(None),
  };

  let gid = match number("-g") {
    Some(gid) => gid,
    None => return Ok(None),
  };

  Ok(Some((uid, gid)))
}

pub fn commit(root: &Path, what: &str, wrote: &[std::path::PathBuf]) -> Result<(), Never> {
  match (wrote.is_empty(), std::fs::metadata(root.join(".git"))) {
    (false, Ok(_repository)) => {}
    (true, _) => return Ok(()),
    (false, Err(_no_repository_marker)) => return Ok(()),
  }

  let root_2 = root.display().to_string();
  let named: Vec<String> = wrote.iter().map(|at| at.display().to_string()).collect();
  let arguments = |verb: &[&str]| -> Vec<String> {
    let Ok(mut arguments) = Program::Git.arguments(&["-C", &root_2]);

    arguments.extend(verb.iter().map(|word| (*word).to_string()));
    arguments.push("--".to_string());
    arguments.extend(named.iter().cloned());
    arguments
  };
  let said = |arguments_2: &[String]| {
    let Ok(said) = run(&arguments_2.iter().map(String::as_str).collect::<Vec<_>>());

    said
  };
  let _ = said(&arguments(&["add"]));

  match ::core::primitive::bool::then_some(
    !said(&arguments(&["status", "--porcelain"])).out.is_empty(),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let _ = said(&arguments(&["commit", "-m", what]));
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

pub fn uncommitted(root: &Path) -> Result<Vec<String>, Never> {
  match std::fs::metadata(root.join(".git")) {
    Ok(_repository) => {}
    Err(_no_repository_marker) => return Ok(Vec::new()),
  }

  let root_2 = root.display().to_string();
  let Ok(git) = Program::Git.name();
  let Ok(said) = run(&[git, "-C", &root_2, "status", "--porcelain"]);

  Ok(
    said
      .out
      .lines()
      .map(|line| line.trim().to_string())
      .filter(|line| !line.is_empty())
      .collect(),
  )
}

pub struct Here;

impl laying::Lays for Here {
  fn stage(&mut self, from: &Path, live: &str) -> Result<(), Unapplied> {
    stage_file(from, live)
  }

  fn swap(&mut self, live: &str) -> Result<Back, Unapplied> {
    swap_file(live)
  }

  fn put_back(&mut self, laid: &Install) -> Result<(), Unapplied> {
    put_back(laid)
  }

  fn drop_staged(&mut self, live: &str) {
    let Ok(()) = drop_staged(live);
  }

  fn drop_kept(&mut self, live: &str) {
    let Ok(()) = drop_kept(live);
  }

  fn presence(&self, live: &str) -> Back {
    let Ok(whoever) = whoever();
    let Ok(on) = install::on_machine(live, User(whoever));

    match std::fs::symlink_metadata(&on) {
      Ok(_held) => Back::Retained,
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ())
        {
          ::core::option::Option::Some(()) => Back::Closed,
          ::core::option::Option::None => {
            eprintln!("console apply: {on}: {fault}");

            Back::Retained
          }
        }
      }
    }
  }

  fn note(&mut self, laid: &[Install]) -> Result<(), Unapplied> {
    wrote_plan(Path::new(PLAN), laid)
  }

  fn forget_note(&mut self) {
    let Ok(()) = forget_plan(Path::new(PLAN));
  }
}

pub const PLAN: &str = "/var/lib/console/laying";

fn line_of(laid: &Install) -> Result<String, Never> {
  let back = match laid.back {
    Back::Retained => "kept",
    Back::Closed => "gone",
  };

  Ok(format!(
    "{back} {}
",
    laid.at
  ))
}

fn wrote_plan(at: &Path, laid: &[Install]) -> Result<(), Unapplied> {
  match at.parent() {
    Some(holding) => std::fs::create_dir_all(holding)
      .map_err(|fault| Unapplied::Directory(at.to_path_buf(), fault))?,
    None => {}
  }

  let written: String = laid
    .iter()
    .map(|laid_2| {
      let Ok(line) = line_of(laid_2);

      line
    })
    .collect();
  console_core_atomic_writes::whole(at, written.as_bytes()).map_err(Unapplied::Wrote)
}

fn forget_plan(at: &Path) -> Result<(), Never> {
  match console_core_atomic_writes::gone(at) {
    Ok(()) => {}
    Err(fault) => eprintln!("console apply: {fault}"),
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    expect(
      explicit040_no_torn_write,
      reason = "the test creates and resizes a sparse fixture in its own scratch directory"
    )
  )]
  fn staging_reads_large_programs_without_truncating_them() -> Result<(), Failure> {
    let directory = console_core_temporary_directories::fresh("manifest-stage")?;
    let at = directory.join("program");
    let file = std::fs::File::create(&at)?;
    let length = console_core_atomic_writes::MAX_TEXT_BYTES.saturating_add(1);
    file.set_len(u64::from(length))?;
    let staged = staging_bytes(&at);

    file.set_len(u64::from(MAX_STAGING_BYTES).saturating_add(1))?;
    let oversized = staging_bytes(&at);
    std::fs::remove_dir_all(directory)?;

    let staged_bytes = staged?;
    let staged_length = u32::try_from(staged_bytes.len())?;
    assert_eq!(staged_length, length);
    assert_eq!(
      oversized.map_err(|fault| fault.kind()),
      Err(std::io::ErrorKind::FileTooLarge)
    );

    Ok(())
  }

  #[test]
  fn the_directory_that_marks_a_home_is_one_the_manifest_puts_there() -> Result<(), Failure> {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = crate_dir
      .parent()
      .and_then(Path::parent)
      .ok_or("the repository root")?;
    let held = std::fs::read_to_string(root.join("desktop.conf"))?;
    let Ok(ours) = Base::Configuration.application_under(&Path::new("/home").join(USER));
    let under = format!("{}/", ours.display());

    assert!(
      held.lines().any(|line| line.trim().starts_with(&under)),
      "nothing the manifest lays down is under {under}, so no home can be told by it"
    );

    Ok(())
  }
}
