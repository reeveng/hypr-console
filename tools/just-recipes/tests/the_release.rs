use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use console_core_external_programs::Program;

const PUBLISHER: &str = env!("CARGO_BIN_EXE_console-just-recipes");

#[test]
fn publishes_committed_snapshots_and_tags_public_commits() -> Result<(), Box<dyn Error>> {
  let fixture = Fixture::new()?;
  let source = &fixture.source;
  let public = &fixture.checkout;
  let remote = &fixture.remote;

  fs::write(source.join("snapshot"), "first committed snapshot\n")?;
  let earlier = commit(source)?;
  fs::write(source.join("snapshot"), "latest committed snapshot\n")?;
  let latest = commit(source)?;
  fs::write(source.join("snapshot"), "uncommitted content\n")?;
  fs::write(source.join("untracked"), "untracked content\n")?;
  fs::write(public.join("dirty"), "local public checkout content\n")?;

  fixture.success(&["publish", "0.1.1", &earlier])?;
  let first = git(remote, &["rev-parse", "main"])?;
  assert_eq!(
    git(remote, &["rev-parse", "console-v0.1.1^{commit}"])?,
    first
  );
  assert_eq!(
    git(remote, &["show", "main:snapshot"])?,
    "first committed snapshot"
  );
  assert!(git(remote, &["log", "-1", "--format=%B", "main"])?.contains(&earlier));

  fixture.success(&["publish", "console-v0.1.2"])?;
  let second = git(remote, &["rev-parse", "main"])?;
  assert_eq!(
    git(remote, &["rev-parse", "console-v0.1.2^{commit}"])?,
    second
  );
  assert_eq!(
    git(remote, &["show", "main:snapshot"])?,
    "latest committed snapshot"
  );
  assert!(git(remote, &["log", "-1", "--format=%B", "main"])?.contains(&latest));
  assert_eq!(
    git(remote, &["log", "-1", "--format=%ae", "main"])?,
    "console-public@example.invalid"
  );
  assert!(!git(remote, &["ls-tree", "--name-only", "main"])?.contains("untracked"));
  assert_eq!(
    fs::read_to_string(source.join("snapshot"))?,
    "uncommitted content\n"
  );
  assert_eq!(
    fs::read_to_string(public.join("dirty"))?,
    "local public checkout content\n"
  );

  fixture.success(&["release-tag", "0.1.3", &first, "--no-latest"])?;
  assert_eq!(
    git(remote, &["rev-parse", "console-v0.1.3^{commit}"])?,
    first
  );
  let annotation = git(remote, &["cat-file", "tag", "console-v0.1.3"])?;
  assert!(annotation.contains("Console-Latest: false"));
  fixture.success(&["release-tag", "0.1.4"])?;
  assert_eq!(
    git(remote, &["rev-parse", "console-v0.1.4^{commit}"])?,
    second
  );

  let duplicate = fixture.release(&["publish", "0.1.1"])?;
  assert!(!duplicate.status.success());
  assert_eq!(git(remote, &["rev-parse", "main"])?, second);
  assert_eq!(
    git(remote, &["rev-parse", "console-v0.1.1^{commit}"])?,
    first
  );
  Ok(())
}

#[test]
fn a_failed_privacy_check_leaves_public_refs_untouched() -> Result<(), Box<dyn Error>> {
  let fixture = Fixture::new()?;
  let main = git(&fixture.remote, &["rev-parse", "main"])?;
  fs::write(
    fixture.source.join("private"),
    "console-private@example.invalid\n",
  )?;
  let _private = commit(&fixture.source)?;

  let release = fixture.release(&["publish", "0.1.1"])?;
  let diagnostics = String::from_utf8(release.stderr)?;
  assert!(!release.status.success());
  assert!(diagnostics.contains("private contains the configured Git email"));
  assert_eq!(git(&fixture.remote, &["rev-parse", "main"])?, main);
  assert_eq!(git(&fixture.remote, &["tag", "--list"])?, "");
  Ok(())
}

struct Fixture {
  directory: PathBuf,
  source: PathBuf,
  checkout: PathBuf,
  remote: PathBuf,
}

impl Fixture {
  fn new() -> Result<Self, Box<dyn Error>> {
    let directory = console_core_temporary_directories::private("publish-release-test")?;
    let fixture = Self {
      source: directory.join("private"),
      checkout: directory.join("public checkout"),
      remote: directory.join("remote"),
      directory,
    };
    fs::create_dir_all(fixture.source.join(".github/workflows"))?;
    fs::create_dir(&fixture.checkout)?;
    fs::create_dir(&fixture.remote)?;
    git(
      &fixture.source,
      &["init", "--quiet", "--initial-branch=main"],
    )?;
    git(&fixture.source, &["config", "user.name", "Private Fixture"])?;
    git(
      &fixture.source,
      &["config", "user.email", "console-private@example.invalid"],
    )?;
    fs::write(fixture.source.join("desktop.conf"), "[desktop]\n")?;
    fs::write(
      fixture.source.join(".github/workflows/console-release.yml"),
      "name: Fixture\n",
    )?;
    git(
      &fixture.remote,
      &["init", "--quiet", "--bare", "--initial-branch=main"],
    )?;
    git(
      &fixture.checkout,
      &["init", "--quiet", "--initial-branch=main"],
    )?;
    git(
      &fixture.checkout,
      &["config", "user.name", "Public Fixture"],
    )?;
    git(
      &fixture.checkout,
      &["config", "user.email", "console-public@example.invalid"],
    )?;
    git(&fixture.checkout, &["remote", "add", "origin", "../remote"])?;
    fs::write(fixture.checkout.join("existing"), "public base\n")?;
    let _base = commit(&fixture.checkout)?;
    git(&fixture.checkout, &["push", "--quiet", "origin", "main"])?;

    Ok(fixture)
  }

  fn release(&self, arguments: &[&str]) -> std::io::Result<Output> {
    Command::new(PUBLISHER)
      .current_dir(&self.source)
      .env("TMPDIR", &self.directory)
      .env("GIT_CONFIG_GLOBAL", "/dev/null")
      .env("CARGO_NET_OFFLINE", "true")
      .args(arguments)
      .arg("--repository")
      .arg(&self.checkout)
      .output()
  }

  fn success(&self, arguments: &[&str]) -> Result<(), Box<dyn Error>> {
    let output = self.release(arguments)?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
  }
}

impl Drop for Fixture {
  fn drop(&mut self) {
    if let Err(fault) = fs::remove_dir_all(&self.directory) {
      eprintln!("{fault}");
    }
  }
}

fn git(directory: &Path, arguments: &[&str]) -> Result<String, Box<dyn Error>> {
  let Ok(mut command) = Program::Git.command();
  let output = command
    .current_dir(directory)
    .env("GIT_CONFIG_GLOBAL", "/dev/null")
    .args(arguments)
    .output()?;
  assert!(
    output.status.success(),
    "git {arguments:?} returned {output:?}"
  );
  Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
}

fn commit(directory: &Path) -> Result<String, Box<dyn Error>> {
  git(directory, &["add", "--all"])?;
  git(directory, &["commit", "--quiet", "-m", "Fixture snapshot"])?;
  git(directory, &["rev-parse", "HEAD"])
}
