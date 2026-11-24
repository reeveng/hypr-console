//! Linux child lifetimes and session separation.
//!
//! The binding is installed between the fork and the exec: the kernel is
//! asked to send the child a SIGTERM when the thread that forked it exits,
//! and then the child checks that the process it was forked from is still
//! its parent, because a parent that died in the moment before the binding
//! was installed is a death the kernel will never report. A child that cannot
//! be bound, or whose parent is already gone, does not run at all -- running
//! unbound is the leak this crate exists to prevent.
//!
//! Both questions go through rustix, as every question to the kernel here
//! does. The first version declared `prctl` and `getppid` by hand with the
//! constants typed out, dropped `prctl`'s answer, and said what went wrong
//! with an `io::Error` carrying a sentence. That sentence is an allocation,
//! made in a child forked from a process with other threads, where the
//! allocator's lock may be held by a thread that no longer exists in it. An
//! errno allocates nothing, so the child says how it failed in the kernel's
//! own words.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Command;

use console_core_never::Never;
use rustix::io::Errno;
use rustix::process::{Signal, getpid, getppid, set_parent_process_death_signal};

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "parent-death binding must be installed in the child before exec; this boundary owns the hook, whose body only calls rustix prctl/getppid and returns allocation-free errno failures"
  )
)]
pub(crate) fn bind(command: &mut Command) -> Result<(), Never> {
  let whose = getpid();

  // SAFETY: what runs between the fork and the exec is two system calls,
  // prctl and getppid, made directly through rustix and both
  // async-signal-safe. Nothing here allocates or takes a lock: the answer
  // on either failure is an errno, which is a number.
  unsafe {
    command.pre_exec(move || {
      let bound = set_parent_process_death_signal(Some(Signal::TERM));
      bound?;

      match getppid() {
        Some(parent) => match ::core::primitive::bool::then_some(parent == whose, ()) {
          ::core::option::Option::Some(()) => Ok(()),
          ::core::option::Option::None => Err(io::Error::from(Errno::SRCH)),
        },
        None => Err(io::Error::from(Errno::SRCH)),
      }
    });
  }

  Ok(())
}

pub(crate) fn detach(command: &mut Command) -> Result<(), Never> {
  command.setsid(true);

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
      explicit026_env_read_once,
      reason = "this test alone reads its child-process marker, set only on the child Command; no parent environment is changed"
    )
  )]
  fn a_detached_child_leads_its_own_session() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::var_os("CONSOLE_TEST_DETACHED_SESSION") {
      Some(_child) => {
        let session = rustix::process::getsid(None)?;

        assert_eq!(session, getpid());

        return Ok(());
      }
      None => {}
    }

    let itself = std::env::current_exe()?;
    let mut command = Command::new(itself);
    command
      .args([
        "--exact",
        "program_lifetime::hooks::tests::a_detached_child_leads_its_own_session",
      ])
      .env("CONSOLE_TEST_DETACHED_SESSION", "1");
    let Ok(()) = detach(&mut command);
    let output = command.output()?;

    assert!(
      output.status.success(),
      "detached child failed: status={:?}, stdout={:?}, stderr={:?}",
      output.status,
      output.stdout,
      output.stderr,
    );

    Ok(())
  }

  #[test]
  fn a_bound_child_runs_while_whoever_started_it_is_still_here()
  -> Result<(), Box<dyn std::error::Error>> {
    let itself = std::env::current_exe()?;
    let mut command = Command::new(itself);

    command.args(["--list", "--exact", "nothing-is-called-this"]);

    let Ok(()) = bind(&mut command);
    let ran = command.output()?;

    assert!(
      ran.status.success(),
      "the binding stopped a child whose parent is alive: {ran:?}"
    );

    Ok(())
  }
}
