//! Fetch a published Legion Go release and apply its tag as one generation.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{self, DirBuilder};
use std::io::ErrorKind;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{SocketAddr, UnixListener};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

const REPOSITORY: &str = "https://github.com/reeveng/hypr-console";
const RELEASE_API: &str = "https://api.github.com/repos/reeveng/hypr-console/releases";
const ASSET: &str = "legion-go-x86_64.tar.gz";
const LIVE: &str = console_repository::DEVICE_ROOT;
const RELEASES: &str = "/etc/.console-releases";
const ENGINE: &str = "target/release/console";
const MARK: &str = ".git/console-release";
const PENDING: &str = "pending";
const PREVIOUS: &str = "previous";
const HELD: &str = "held-tag";

type Fault = Box<dyn Error>;

enum Request {
  Latest,
  Automatic,
  Tag(String),
  Rollback,
}

struct Release {
  tag: String,
  digest: String,
  bytes: u64,
}

struct Switch {
  slot: String,
  before: String,
  after: String,
  restoring: bool,
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("console-update: {fault}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), Fault> {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let request = match arguments.as_slice() {
    [] => Request::Latest,
    [word] if word == "--help" => {
      println!("Usage: console-update [legion-go-vX.Y.Z | --automatic | --rollback]");
      return Ok(());
    }
    [word] if word == "--automatic" => Request::Automatic,
    [word] if word == "--rollback" => Request::Rollback,
    [word] if version(word).is_some() => Request::Tag(word.clone()),
    _ => {
      return Err("Usage: console-update [legion-go-vX.Y.Z | --automatic | --rollback]".into());
    }
  };

  if output(Command::new("id").arg("-u"))? != "0" {
    return Err("run console-update as root".into());
  }
  if fs::read_to_string("/sys/class/dmi/id/product_name")?.trim() != "83E1" {
    return Err("this release is for the first-generation Legion Go (83E1)".into());
  }

  let _lock = UnixListener::bind_addr(&SocketAddr::from_abstract_name(b"console/update")?)?;
  let releases = Path::new(RELEASES);
  let live = Path::new(LIVE);
  DirBuilder::new()
    .mode(0o700)
    .create(releases)
    .or_else(|fault| {
      if fault.kind() == ErrorKind::AlreadyExists {
        Ok(())
      } else {
        Err(fault)
      }
    })?;

  if let Some(pending) = optional(&releases.join(PENDING))? {
    return settle(live, releases, Switch::read(&pending)?);
  }
  if !git(live, &["status", "--porcelain"])?.is_empty() {
    return Err("the current checkout has uncommitted changes".into());
  }

  if matches!(request, Request::Rollback) {
    let saved = optional(&releases.join(PREVIOUS))?.ok_or("there is no previous release")?;
    remember_held(live, releases)?;
    println!("Restoring the previous Console release.");
    return begin(live, releases, saved.trim().to_owned());
  }

  let automatic = matches!(request, Request::Automatic);
  if automatic && !idle()? {
    println!("The desktop is in use. The next update check will try again.");
    return Ok(());
  }
  let tag = match &request {
    Request::Tag(tag) => Some(tag.as_str()),
    _ => None,
  };
  let release = release(tag)?;
  if automatic
    && optional(&releases.join(HELD))?
      .as_deref()
      .is_some_and(|held| version(held.trim()) >= version(&release.tag))
  {
    println!(
      "{} was rolled back. The next stable tag can be installed automatically.",
      release.tag
    );
    return Ok(());
  }
  if let Some(current) = marker(live)? {
    let (installed_tag, installed_commit) = current;
    if git(live, &["rev-parse", "HEAD"])? != installed_commit {
      return Err("the current checkout has diverged from its installed release".into());
    }
    if release.tag == installed_tag
      || (automatic && version(&release.tag) <= version(&installed_tag))
    {
      println!("Console is on {installed_tag}.");
      return Ok(());
    }
  }

  println!("Downloading {}.", release.tag);
  let slot = format!(
    "release-{}-{}",
    std::process::id(),
    SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
  );
  let stage = releases.join(&slot);
  fs::create_dir(&stage)?;
  match prepare(&stage, &release) {
    Ok(()) => begin(live, releases, slot),
    Err(fault) => {
      fs::remove_dir_all(stage)?;
      Err(fault)
    }
  }
}

fn version(tag: &str) -> Option<(u32, u32, u32)> {
  let mut parts = tag.strip_prefix("legion-go-v")?.split('.');
  let major = parts.next()?.parse().ok()?;
  let minor = parts.next()?.parse().ok()?;
  let patch = parts.next()?.parse().ok()?;
  (parts.next().is_none() && tag == format!("legion-go-v{major}.{minor}.{patch}"))
    .then_some((major, minor, patch))
}

fn idle() -> Result<bool, Fault> {
  if !user_unit_active("console.target")? {
    return Ok(false);
  }
  let session = output(Command::new("loginctl").args([
    "show-seat",
    "seat0",
    "-p",
    "ActiveSession",
    "--value",
  ]))?;
  if session.is_empty() {
    return Ok(false);
  }
  let hint = output(Command::new("loginctl").args([
    "show-session",
    &session,
    "-p",
    "IdleHint",
    "--value",
  ]))?;
  if hint == "yes" {
    return Ok(true);
  }
  user_unit_active("console-lock.service")
}

fn user_unit_active(unit: &str) -> Result<bool, Fault> {
  let status = Command::new("systemctl")
    .args([
      "--user",
      "--machine=1000@.host",
      "is-active",
      "--quiet",
      unit,
    ])
    .status()?;
  Ok(status.success())
}

fn release(tag: Option<&str>) -> Result<Release, Fault> {
  let url = match tag {
    Some(tag) => format!("{RELEASE_API}/tags/{tag}"),
    None => format!("{RELEASE_API}/latest"),
  };
  let response = curl(&url).args(["--max-filesize", "1048576"]).output()?;
  if !response.status.success() {
    return Err(
      format!(
        "GitHub release metadata: {}",
        String::from_utf8_lossy(&response.stderr)
      )
      .into(),
    );
  }

  let json = std::process::Stdio::piped();
  let mut selection = Command::new("jq")
    .args([
      "-er",
      "--arg",
      "asset",
      ASSET,
      r#"select(.immutable and (.draft | not) and (.prerelease | not)) |
         .tag_name as $tag |
         .assets[] | select(.name == $asset and .state == "uploaded") |
         $tag, .digest, .size"#,
    ])
    .stdin(json)
    .stdout(std::process::Stdio::piped())
    .spawn()?;
  {
    use std::io::Write;
    selection
      .stdin
      .take()
      .ok_or("could not open the release parser")?
      .write_all(&response.stdout)?;
  }
  let selected = selection.wait_with_output()?;
  if !selected.status.success() {
    return Err("GitHub did not report an immutable release with a Legion Go bundle".into());
  }
  let selected = String::from_utf8(selected.stdout)?;
  let [published, digest, bytes]: [&str; 3] = selected
    .lines()
    .collect::<Vec<_>>()
    .try_into()
    .map_err(|_| "GitHub did not report exactly one release bundle")?;
  let digest = digest
    .strip_prefix("sha256:")
    .ok_or("the release has no SHA-256 digest")?;
  let bytes: u64 = bytes.parse()?;
  if version(published).is_none()
    || tag.is_some_and(|requested| requested != published)
    || !hex(digest, 64)
    || bytes == 0
    || bytes >= 2_147_483_648
  {
    return Err("GitHub returned invalid Legion Go release metadata".into());
  }
  Ok(Release {
    tag: published.to_owned(),
    digest: digest.to_owned(),
    bytes,
  })
}

fn curl(url: &str) -> Command {
  let mut curl = Command::new("curl");
  curl.args([
    "--fail",
    "--silent",
    "--show-error",
    "--location",
    "--proto",
    "=https",
    "--proto-redir",
    "=https",
    "--connect-timeout",
    "15",
    "--max-time",
    "600",
    "--retry",
    "2",
    url,
  ]);
  curl
}

fn prepare(stage: &Path, release: &Release) -> Result<(), Fault> {
  let archive = stage.join(ASSET);
  let url = format!("{REPOSITORY}/releases/download/{}/{ASSET}", release.tag);
  checked(
    curl(&url)
      .args(["--max-filesize", &release.bytes.to_string(), "--output"])
      .arg(&archive),
  )?;
  let digest = output(Command::new("sha256sum").arg(&archive))?;
  if fs::metadata(&archive)?.len() != release.bytes
    || digest.split_whitespace().next() != Some(release.digest.as_str())
  {
    return Err("the download does not match GitHub's published asset digest".into());
  }

  let listing = output(Command::new("tar").arg("-tzf").arg(&archive))?;
  let mut members = BTreeSet::new();
  for entry in listing.lines() {
    if !member(entry) || !members.insert(entry) {
      return Err(format!("unexpected bundle member {entry:?}").into());
    }
  }
  if !members.contains("revision")
    || !members.contains("bin/console")
    || !members.contains("bin/console-update")
  {
    return Err("the bundle is missing its revision or one of its installers".into());
  }
  let unpacked = stage.join("unpacked");
  fs::create_dir(&unpacked)?;
  checked(
    Command::new("tar")
      .args(["--no-same-owner", "--no-same-permissions", "-xzf"])
      .arg(&archive)
      .arg("-C")
      .arg(&unpacked),
  )?;
  if members
    .iter()
    .any(|entry| !fs::symlink_metadata(unpacked.join(entry)).is_ok_and(|file| file.is_file()))
  {
    return Err("the bundle contains an entry that is not a regular file".into());
  }

  let checkout = stage.join("tree");
  fs::create_dir(&checkout)?;
  checked(
    Command::new("git")
      .arg("-C")
      .arg(&checkout)
      .args(["init", "--quiet"]),
  )?;
  let tag = format!("refs/tags/{}", release.tag);
  git(
    &checkout,
    &[
      "-c",
      "http.lowSpeedLimit=100",
      "-c",
      "http.lowSpeedTime=30",
      "fetch",
      "--quiet",
      "--no-tags",
      REPOSITORY,
      &tag,
    ],
  )?;
  git(
    &checkout,
    &["checkout", "--quiet", "--detach", "FETCH_HEAD"],
  )?;
  let commit = git(&checkout, &["rev-parse", "HEAD"])?;
  if fs::read_to_string(unpacked.join("revision"))?.trim() != commit {
    return Err("the bundle was built from a different commit than the published tag".into());
  }

  fs::create_dir(checkout.join("target"))?;
  fs::rename(unpacked.join("bin"), checkout.join("target/release"))?;
  write(
    &checkout.join(MARK),
    &format!("{}\n{commit}\n", release.tag),
  )?;
  fs::remove_file(archive)?;
  fs::remove_dir_all(unpacked)?;
  Ok(())
}

fn member(path: &str) -> bool {
  path == "revision"
    || path.strip_prefix("bin/").is_some_and(|name| {
      !name.is_empty()
        && name
          .bytes()
          .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}

fn hex(text: &str, length: usize) -> bool {
  text.len() == length
    && text
      .bytes()
      .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn slot_path(releases: &Path, slot: &str) -> Result<PathBuf, Fault> {
  let parts = slot
    .strip_prefix("release-")
    .ok_or("bad release storage name")?;
  let (process, time) = parts.split_once('-').ok_or("bad release storage name")?;
  process.parse::<u32>()?;
  time.parse::<u128>()?;
  Ok(releases.join(slot).join("tree"))
}

impl Switch {
  fn read(text: &str) -> Result<Self, Fault> {
    let [slot, before, after, action]: [&str; 4] = text
      .lines()
      .collect::<Vec<_>>()
      .try_into()
      .map_err(|_| "invalid pending release record")?;
    if !hex(before, 40)
      || !hex(after, 40)
      || before == after
      || !matches!(action, "apply" | "restore")
    {
      return Err("invalid pending release record".into());
    }
    Ok(Self {
      slot: slot.to_owned(),
      before: before.to_owned(),
      after: after.to_owned(),
      restoring: action == "restore",
    })
  }

  fn record(&self, releases: &Path) -> Result<(), Fault> {
    let action = if self.restoring { "restore" } else { "apply" };
    write(
      &releases.join(PENDING),
      &format!("{}\n{}\n{}\n{action}\n", self.slot, self.before, self.after),
    )
  }
}

fn begin(live: &Path, releases: &Path, slot: String) -> Result<(), Fault> {
  let other = slot_path(releases, &slot)?;
  let before = git(live, &["rev-parse", "HEAD"])?;
  let after = git(&other, &["rev-parse", "HEAD"])?;
  if before == after {
    println!("The checkout already has this release's revision.");
    return Ok(());
  }
  let change = Switch {
    slot,
    before,
    after,
    restoring: false,
  };
  change.record(releases)?;
  settle(live, releases, change)
}

fn settle(live: &Path, releases: &Path, mut change: Switch) -> Result<(), Fault> {
  let other = slot_path(releases, &change.slot)?;
  if change.restoring {
    apply(live, &other, &change.before, &change.after)?;
    fs::remove_file(releases.join(PENDING))?;
    return Ok(());
  }

  match apply(live, &other, &change.after, &change.before) {
    Ok(()) => {
      let previous = optional(&releases.join(PREVIOUS))?;
      write(&releases.join(PREVIOUS), &change.slot)?;
      fs::remove_file(releases.join(PENDING))?;
      // Keep the original local checkout as a separate recovery copy.
      if let Some(previous) = previous.filter(|previous| previous.trim() != change.slot) {
        let retired = slot_path(releases, previous.trim())?;
        if marker(&retired)?.is_some() {
          fs::remove_dir_all(retired.parent().ok_or("invalid retired release path")?)?;
        }
      }
      println!("Console release {} is active.", change.after);
      Ok(())
    }
    Err(fault) => {
      if git(live, &["rev-parse", "HEAD"])? == change.after {
        eprintln!("The release did not pass verification. Restoring the previous release.");
        remember_held(live, releases)?;
        change.restoring = true;
        change.record(releases)?;
        apply(live, &other, &change.before, &change.after)?;
      }
      fs::remove_file(releases.join(PENDING))?;
      let previous = optional(&releases.join(PREVIOUS))?;
      if previous
        .as_deref()
        .is_none_or(|slot| slot.trim() != change.slot)
      {
        fs::remove_dir_all(other.parent().ok_or("invalid release path")?)?;
      }
      Err(fault)
    }
  }
}

fn remember_held(live: &Path, releases: &Path) -> Result<(), Fault> {
  if let Some((tag, _)) = marker(live)? {
    write(&releases.join(HELD), &tag)?;
  }
  Ok(())
}

fn apply(live: &Path, other: &Path, wanted: &str, previous: &str) -> Result<(), Fault> {
  let current = git(live, &["rev-parse", "HEAD"])?;
  let saved = git(other, &["rev-parse", "HEAD"])?;
  let switching = current == previous && saved == wanted;
  if !switching && !(current == wanted && saved == previous) {
    return Err("the checkout revisions changed while a release was pending".into());
  }
  let (target, old) = if switching {
    (other, live)
  } else {
    (live, other)
  };
  let engine = if marker(target)?.is_some() {
    target.join(ENGINE)
  } else {
    old.join(ENGINE)
  };
  let mut apply = Command::new(engine);
  apply.args(["apply", "--prebuilt"]);
  if switching {
    apply.arg("--switch-from").arg(other);
  }
  checked(&mut apply)?;
  if git(live, &["rev-parse", "HEAD"])? != wanted {
    return Err("the manifest engine did not activate the requested checkout".into());
  }
  let account = output(Command::new("getent").args(["passwd", "1000"]))?;
  let user = account
    .split(':')
    .next()
    .filter(|user| !user.is_empty())
    .ok_or("the desktop account was not found")?;
  checked(Command::new("runuser").args([
    "-u",
    user,
    "--",
    "env",
    "XDG_RUNTIME_DIR=/run/user/1000",
    "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus",
    "/usr/local/bin/console",
    "health",
  ]))
}

fn marker(root: &Path) -> Result<Option<(String, String)>, Fault> {
  optional(&root.join(MARK))?
    .map(|text| {
      let [tag, commit]: [&str; 2] = text
        .lines()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "invalid installed release record")?;
      if version(tag).is_none() || !hex(commit, 40) {
        return Err("invalid installed release record".into());
      }
      Ok((tag.to_owned(), commit.to_owned()))
    })
    .transpose()
}

fn optional(path: &Path) -> Result<Option<String>, Fault> {
  match fs::read_to_string(path) {
    Ok(text) => Ok(Some(text)),
    Err(fault) if fault.kind() == ErrorKind::NotFound => Ok(None),
    Err(fault) => Err(fault.into()),
  }
}

fn write(path: &Path, text: &str) -> Result<(), Fault> {
  console_core_atomic_writes::whole(path, text.as_bytes()).map_err(Into::into)
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, Fault> {
  output(
    Command::new("git")
      .env("GIT_TERMINAL_PROMPT", "0")
      .arg("-C")
      .arg(root)
      .args(arguments),
  )
}

fn output(command: &mut Command) -> Result<String, Fault> {
  let result = command.output()?;
  if !result.status.success() {
    return Err(
      format!(
        "{command:?} exited with {}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr).trim()
      )
      .into(),
    );
  }
  Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}

fn checked(command: &mut Command) -> Result<(), Fault> {
  let status = command.status()?;
  if status.success() {
    Ok(())
  } else {
    Err(format!("{command:?} exited with {status}").into())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn release_tags_have_an_order_and_cannot_form_paths() {
    assert!(version("legion-go-v0.10.1") > version("legion-go-v0.9.9"));
    assert_eq!(version("legion-go-v0.1.0/../checkout"), None);
    assert_eq!(version("legion-go-v0.1.0-rc1"), None);
    assert_eq!(version("legion-go-v0.1.00"), None);
  }

  #[test]
  fn release_members_are_only_the_revision_and_regular_binary_names() {
    assert!(member("revision"));
    assert!(member("bin/console"));
    assert!(!member("bin/../revision"));
    assert!(!member("/bin/console"));
    assert!(!member("bin/nested/console"));
  }
}
