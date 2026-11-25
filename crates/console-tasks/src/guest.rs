//! The process and filesystem effects of running the archived tests in a guest.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_operating_system_linux::program_lifetime::alongside;
use console_tasks::task::Suite;

const GUEST: &str = "/etc/console-test-vm";
const WORKSPACE: &str = "/var/lib/console-test/workspace";
const UNIT: &str = "(kind(lib) | kind(bin) | kind(proc-macro))";
const DESKTOP: &str = "(package(=console-test-desktop) & binary(=what_a_run_leaves))";
const LIVE: &str = "(package(=console-input-controller) & binary(=really_running))";
const PANELS: &str = concat!(
  "(package(=console-calendar) & binary(=a_finger)) | ",
  "(package(=console-application-media-viewer) & binary(=a_finger)) | ",
  "(package(=console-panel) & binary(=every_panel_answers_a_finger))"
);
const ARCHITECTURE: &str =
  "package(=console-architecture) & (binary(=the_map) | binary(=the_rules))";
const VOCABULARY: &str = "package(=console-vocabulary) & binary(=the_words)";
const WAYLAND: &str = "package(=console-wayland) & kind(lib)";
const MEDIA: &str = concat!(
  "(package(=console-sound-effects) & binary(=balanced_output)) | ",
  "(package(=console-music-player) & test(loudness::tests::)) | ",
  "(package(=console-wallpaper) & test(=render::tests::a_fixture_frame_finishes_after_the_encoder_input_closes)) | ",
  "(package(=console-application-camera) & test(a_monochrome_stream_can_be_previewed_and_encoded))"
);

pub(super) fn link(arguments: &[String]) -> io::Result<()> {
  let _ = fs::read_to_string(GUEST)?;
  let archive = match arguments {
    [archive] => Path::new(archive),
    _ => {
      return Err(io::Error::other(
        "guest-workspace requires the test archive path",
      ));
    }
  };
  let reported = fs::read_to_string(archive.join("workspace-path"))?;
  let original = Path::new(reported.trim_end());
  let workspace = Path::new(WORKSPACE);

  match (original == workspace).then_some(()) {
    Some(()) => Ok(()),
    None => {
      let parent = original
        .parent()
        .ok_or_else(|| io::Error::other("the build workspace has no parent"))?;
      fs::create_dir_all(parent)?;
      symlink(workspace, original)
    }
  }
}

pub(super) fn run(arguments: &[String]) -> io::Result<()> {
  let guest = fs::read_to_string(GUEST)?;
  let (name, archive, source, filter_file) = match arguments {
    [name, archive, source] => (name, Path::new(archive), Path::new(source), None),
    [name, archive, source, filter] => (
      name,
      Path::new(archive),
      Path::new(source),
      Some(Path::new(filter)),
    ),
    _ => {
      return Err(io::Error::other(
        "guest-test requires the suite, archive and source",
      ));
    }
  };
  let Ok(selected) = Suite::from_word(name);
  let suite = selected
    .filter(|_| name == guest.trim_end())
    .ok_or_else(|| io::Error::other("the requested suite is not configured in this VM"))?;
  let workspace = Path::new(WORKSPACE);
  prepare(GuestFiles {
    workspace,
    archive,
    source,
  })?;

  match suite {
    Suite::Unit => nextest(workspace, UNIT, &[]),
    Suite::Integration => nextest(
      workspace,
      &format!("all() - ({UNIT} | {DESKTOP} | {PANELS} | {LIVE})"),
      &[],
    ),
    Suite::Architecture => nextest(workspace, ARCHITECTURE, &["--run-ignored", "all"]),
    Suite::Vocabulary => nextest(workspace, VOCABULARY, &["--success-output", "final"]),
    Suite::Desktop => {
      desktop(workspace)?;

      nextest(workspace, DESKTOP, &["--run-ignored", "all"])
    }
    Suite::Panels => panels(workspace, filter_file),
    Suite::Live => nextest(
      workspace,
      LIVE,
      &["--run-ignored", "all", "--test-threads", "1"],
    ),
    Suite::Wayland => nextest(
      workspace,
      WAYLAND,
      &["--run-ignored", "ignored-only", "--test-threads", "1"],
    ),
    Suite::Media => nextest(
      workspace,
      MEDIA,
      &["--run-ignored", "ignored-only", "--test-threads", "1"],
    ),
    Suite::Chromium
    | Suite::Browser
    | Suite::Servo
    | Suite::Rasterizer
    | Suite::NightLight
    | Suite::KernelGpuChecksums
    | Suite::SystemGpuCompute => Err(io::Error::other("this suite has its own guest executable")),
  }
}

fn panels(workspace: &Path, filter_file: Option<&Path>) -> io::Result<()> {
  let mut options = vec!["--run-ignored", "all", "--test-threads", "1"];
  let filter = match filter_file {
    None => None,
    Some(file) => {
      let name = fs::read_to_string(file)?;

      Some(name)
    }
  };
  options.extend(filter.as_deref().into_iter().flat_map(|name| ["--", name]));

  nextest(workspace, PANELS, &options)
}

struct GuestFiles<'a> {
  workspace: &'a Path,
  archive: &'a Path,
  source: &'a Path,
}

fn prepare(files: GuestFiles<'_>) -> io::Result<()> {
  let GuestFiles {
    workspace,
    archive,
    source,
  } = files;
  fs::create_dir_all(workspace)?;
  let Ok(mut copy) = Program::Cp.command();
  copy
    .args(["-a", "--no-preserve=ownership"])
    .arg(source.join("."))
    .arg(workspace);
  checked(&mut copy)?;

  let Ok(mut writable) = Program::Chmod.command();
  writable.args(["-R", "u+w"]).arg(workspace);
  checked(&mut writable)?;

  let Ok(mut extract) = Program::Tar.command();
  extract
    .args(["--zstd", "-xf"])
    .arg(archive.join("tests.tar.zst"))
    .current_dir(workspace);
  checked(&mut extract)?;

  let Ok(mut programs) = Program::Cp.command();
  programs
    .args(["-RT", "--preserve=timestamps", "--update=none"])
    .arg(archive.join("bin"))
    .arg(workspace.join("target/debug"));
  checked(&mut programs)
}

fn nextest(workspace: &Path, filter: &str, options: &[&str]) -> io::Result<()> {
  let Ok(mut command) = Program::CargoNextest.command();
  let Ok(()) = in_workspace(workspace, &mut command);
  command
    .args(["nextest", "run", "--no-fail-fast", "--no-tests", "fail"])
    .args(["--status-level", "fail", "--final-status-level", "fail"])
    .arg("--workspace-remap")
    .arg(workspace)
    .args([
      "--target-dir-remap",
      "target",
      "--cargo-metadata",
      "target/nextest/cargo-metadata.json",
      "--binaries-metadata",
      "target/nextest/binaries-metadata.json",
      "-E",
      filter,
    ])
    .args(options);

  checked(&mut command)
}

fn desktop(workspace: &Path) -> io::Result<()> {
  let Ok(mut session) = Program::Systemctl.command();
  session.args(["--user", "is-active", "basic.target"]);
  checked(&mut session)?;

  let mut command = Command::new(workspace.join("target/debug/console-check"));
  let Ok(()) = in_workspace(workspace, &mut command);
  let said = command.args(["--stage", "desktop", "--all"]).output()?;
  io::stdout().lock().write_all(&said.stdout)?;
  io::stderr().lock().write_all(&said.stderr)?;
  let report = str::from_utf8(&said.stdout)
    .map_err(|fault| io::Error::new(io::ErrorKind::InvalidData, fault))?;

  let passed = report
    .lines()
    .filter_map(|line| line.split_once(" ok, 0 failed,"))
    .any(|(count, _)| count.parse::<u32>().is_ok_and(|passed| passed > 0));

  match (said.status.success() && passed).then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other(
      "desktop checks failed or no desktop scenario ran",
    )),
  }
}

fn in_workspace(workspace: &Path, command: &mut Command) -> Result<(), Never> {
  let target = workspace.join("target");
  command
    .current_dir(workspace)
    .env("CARGO_TARGET_DIR", target);

  Ok(())
}

pub(super) fn checked(command: &mut Command) -> io::Result<()> {
  let name = PathBuf::from(command.get_program());
  let mut running = alongside(command).map_err(|fault| {
    io::Error::new(
      fault.kind(),
      format!("could not start {}: {fault}", name.display()),
    )
  })?;
  let status = running.wait()?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other(format!(
      "{} exited with {status}",
      name.display()
    ))),
  }
}
