//! Package queries and transactions for the Arch package manager.

use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::process::{ExitStatus, Stdio};

use console_core_external_programs::Program;
use console_core_never::Never;

mod progress;

pub use progress::PacmanOutput;

pub fn installed() -> Result<Vec<String>, Never> {
  packages("-Qq")
}

pub fn explicit() -> Result<Vec<String>, Never> {
  packages("-Qeq")
}

fn packages(query: &str) -> Result<Vec<String>, Never> {
  let Ok(mut command) = Program::Pacman.command();
  let said = match command.arg(query).output() {
    Ok(done) => match String::from_utf8(done.stdout) {
      Ok(said) => said,
      Err(fault) => {
        console_core_logging::warn!("pacman returned invalid UTF-8: {fault}");

        return Ok(Vec::new());
      }
    },
    Err(_would_not_start) => return Ok(Vec::new()),
  };

  Ok(said.split_whitespace().map(str::to_owned).collect())
}

pub fn install<'a>(
  packages: impl IntoIterator<Item = &'a str>,
  mut heard: impl FnMut(&str, PacmanOutput),
) -> io::Result<ExitStatus> {
  install_with(packages, &mut heard, |listener, line, event| {
    listener(line, event);
    Ok(())
  })
}

pub fn install_with<'a, State>(
  packages: impl IntoIterator<Item = &'a str>,
  state: &mut State,
  heard: impl Fn(&mut State, &str, PacmanOutput) -> Result<(), Never>,
) -> io::Result<ExitStatus> {
  let Ok(mut command) = Program::Pacman.command();
  command
    .args(["-S", "--needed", "--noconfirm"])
    .args(packages)
    .stdout(Stdio::piped());

  let mut child = console_operating_system_linux::program_lifetime::alongside(&mut command)
    .inspect_err(|fault| console_core_logging::error!("pacman: {fault}"))?;
  let Ok(output) = child.take_stdout();

  match output {
    Some(output_2) => {
      for line in BufReader::new(output_2).lines().map_while(Result::ok) {
        let Ok(event) = progress::parse_line(&line);
        let Ok(()) = heard(state, &line, event);
      }
    }
    None => {}
  }

  child.wait()
}

pub fn mark_explicit<'a>(packages: impl IntoIterator<Item = &'a str>) -> io::Result<ExitStatus> {
  let Ok(mut command) = Program::Pacman.command();

  command
    .args(["-D", "--asexplicit", "--quiet"])
    .args(packages)
    .status()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
  Owned,
  Unowned,
  Unknown,
}

#[derive(Debug, Clone, Copy)]
enum Query {
  Succeeded,
  Failed,
}

struct QueryOutput<'a> {
  status: Query,
  stdout: &'a str,
  stderr: &'a str,
}

pub fn owner(path: &Path) -> Result<Ownership, Never> {
  let Ok(mut command) = Program::Pacman.command();
  let output = match command.env("LC_ALL", "C").arg("-Qoq").arg(path).output() {
    Ok(output) => output,
    Err(_would_not_start) => return Ok(Ownership::Unknown),
  };
  let out = match std::str::from_utf8(&output.stdout) {
    Ok(out) => out,
    Err(_invalid_text) => return Ok(Ownership::Unknown),
  };
  let said = match std::str::from_utf8(&output.stderr) {
    Ok(text) => text,
    Err(fault) => {
      console_core_logging::warn!(
        "pacman returned invalid UTF-8 in its ownership diagnostic: {fault}"
      );

      ""
    }
  };
  let query = match ::core::primitive::bool::then_some(output.status.success(), ()) {
    Some(()) => Query::Succeeded,
    None => Query::Failed,
  };

  read_owner(QueryOutput {
    status: query,
    stdout: out,
    stderr: said,
  })
}

fn read_owner(output: QueryOutput<'_>) -> Result<Ownership, Never> {
  let named = !output.stdout.trim().is_empty();
  let disowned = output.stderr.contains("No package owns");

  Ok(match (output.status, named, disowned) {
    (Query::Succeeded, true, false) => Ownership::Owned,
    (Query::Failed, false, true) => Ownership::Unowned,
    (Query::Succeeded, false, _)
    | (Query::Succeeded, true, true)
    | (Query::Failed, _, false)
    | (Query::Failed, true, true) => Ownership::Unknown,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_pacman_saying_so_plainly_is_an_answer_about_an_owner() {
    assert_eq!(
      read_owner(QueryOutput {
        status: Query::Succeeded,
        stdout: "inputplumber",
        stderr: "",
      }),
      Ok(Ownership::Owned)
    );
    assert_eq!(
      read_owner(QueryOutput {
        status: Query::Failed,
        stdout: "",
        stderr: "error: No package owns /etc/old.conf",
      }),
      Ok(Ownership::Unowned)
    );
    assert_eq!(
      read_owner(QueryOutput {
        status: Query::Failed,
        stdout: "",
        stderr: "pacman: No such file or directory",
      }),
      Ok(Ownership::Unknown)
    );
    assert_eq!(
      read_owner(QueryOutput {
        status: Query::Failed,
        stdout: "",
        stderr: "error: failed to read file '/etc': Permission denied",
      }),
      Ok(Ownership::Unknown)
    );
    assert_eq!(
      read_owner(QueryOutput {
        status: Query::Succeeded,
        stdout: "",
        stderr: "",
      }),
      Ok(Ownership::Unknown),
    );
  }
}
