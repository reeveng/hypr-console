//! Fetch a published Legion Go release and apply its tag as one generation.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::{self, DirBuilder};
use std::io::ErrorKind;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::{SocketAddr, UnixListener};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use console_core_external_programs::Program;

#[path = "console-update/pattern.rs"]
mod pattern;

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
const NOTICED: &str = "noticed-tag";
const UPDATER: &str = "/usr/local/bin/console-update";
const CURL_COULD_NOT_RESOLVE_HOST: i32 = 6;
const USAGE: &str = "Usage: console-update [legion-go-vX.Y.Z | --check | --prompt | --rollback | --enroll-pattern | --forget-pattern]";

type UpdateError = Box<dyn Error>;

enum Request {
  Latest,
  Check,
  Prompt,
  Tag(String),
  Rollback,
  EnrollPattern,
  ForgetPattern,
  PatternStatus,
  PatternEntry,
  VerifyPattern,
}

enum NoticeDelivery {
  Posted,
  Deferred,
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

fn run() -> Result<(), UpdateError> {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let request = match arguments.as_slice() {
    [] => Request::Latest,
    [word] if word == "--help" => {
      println!("{USAGE}");
      return Ok(());
    }
    [word] if word == "--check" => Request::Check,
    [word] if word == "--prompt" => Request::Prompt,
    [word] if word == "--rollback" => Request::Rollback,
    [word] if word == "--enroll-pattern" => Request::EnrollPattern,
    [word] if word == "--forget-pattern" => Request::ForgetPattern,
    [word] if word == "--pattern-status" => Request::PatternStatus,
    [word] if word == "--install-with-pattern" => Request::PatternEntry,
    [word] if word == "--verify-pattern-request" => Request::VerifyPattern,
    [word] if version(word).is_some() => Request::Tag(word.clone()),
    _ => return Err(USAGE.into()),
  };
  let live = Path::new(LIVE);
  if matches!(request, Request::Prompt) {
    return prompt(live);
  }

  if !rustix::process::geteuid().is_root() {
    return Err("run console-update as root".into());
  }
  if fs::read_to_string("/sys/class/dmi/id/product_name")?.trim() != "83E1" {
    return Err("this release is for the first-generation Legion Go (83E1)".into());
  }

  if matches!(request, Request::PatternEntry) {
    if std::env::var("SUDO_UID").as_deref() != Ok("1000") {
      return Err("only the desktop account can submit an update pattern".into());
    }
    // Use only the system environment when acting on a desktop request.
    return Err(
      Command::new(UPDATER)
        .arg("--verify-pattern-request")
        .env_clear()
        .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin")
        .env("HOME", "/root")
        .current_dir("/")
        .exec()
        .into(),
    );
  }

  let order = if matches!(request, Request::VerifyPattern) {
    Some(pattern::Order::read()?)
  } else {
    None
  };

  let releases = Path::new(RELEASES);
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
  let metadata = fs::symlink_metadata(releases)?;
  if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o077 != 0 {
    return Err("the release state directory is not private to root".into());
  }
  if matches!(request, Request::PatternStatus) {
    let status = if pattern::enrolled(releases)? {
      "enrolled"
    } else {
      "absent"
    };
    println!("{status}");
    return Ok(());
  }

  let _lock = UnixListener::bind_addr(&SocketAddr::from_abstract_name(b"console/update")?)?;
  match request {
    Request::EnrollPattern => return pattern::enroll(releases),
    Request::ForgetPattern => return pattern::forget(releases),
    _ => {}
  }
  if let Some(order) = &order {
    pattern::verify(releases, order)?;
  }

  let pending = optional(&releases.join(PENDING))?;
  if matches!(request, Request::Check) {
    return check(live, releases, pending.is_some());
  }
  if let Some(pending) = pending {
    let change = Switch::read(&pending)?;
    if let Some(order) = &order {
      let target = if git(live, &["rev-parse", "HEAD"])? == change.after {
        live.to_owned()
      } else {
        slot_path(releases, &change.slot)?
      };
      if change.restoring || !marker(&target)?.is_some_and(|(tag, _)| tag == order.tag) {
        return Err("an administrator must resume the pending release".into());
      }
    }
    return settle(live, releases, change);
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

  let tag = match &request {
    Request::Tag(tag) => Some(tag.as_str()),
    _ => None,
  };
  let release = release(tag)?;
  if let Some(order) = order {
    if order.tag != release.tag || !newer(live, &release.tag)? {
      return Err("the selected release changed. Open Console Updates again".into());
    }
    if let Ok(mut desktop) = desktop_command() {
      let _notice = desktop
        .args(["notify-send", "Installing Console update", &release.tag])
        .status();
    }
  }
  if let Some(current) = marker(live)? {
    let (installed_tag, installed_commit) = current;
    if git(live, &["rev-parse", "HEAD"])? != installed_commit {
      return Err("the current checkout has diverged from its installed release".into());
    }
    if release.tag == installed_tag {
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

fn check(live: &Path, releases: &Path, pending: bool) -> Result<(), UpdateError> {
  if pending {
    println!("A release is pending. Run console-update as root to resume it.");
    return Ok(());
  }
  let Some(latest) = latest_update(live)? else {
    println!("Console is up to date.");
    return Ok(());
  };
  offer_update(releases, &latest.tag, notify_update)
}

fn offer_update(
  releases: &Path,
  tag: &str,
  notify: impl FnOnce(&str) -> Result<NoticeDelivery, UpdateError>,
) -> Result<(), UpdateError> {
  let held = optional(&releases.join(HELD))?;
  let noticed = releases.join(NOTICED);
  if held
    .as_deref()
    .is_some_and(|held| version(held.trim()) >= version(tag))
    || optional(&noticed)?
      .as_deref()
      .is_some_and(|noticed| noticed.trim() == tag)
  {
    return Ok(());
  }

  match notify(tag)? {
    NoticeDelivery::Posted => write(&noticed, tag),
    NoticeDelivery::Deferred => Ok(()),
  }
}

fn notify_update(tag: &str) -> Result<NoticeDelivery, UpdateError> {
  let Ok(systemctl) = Program::Systemctl.name();
  let daemon = desktop_command()?
    .args([
      systemctl,
      "--user",
      "is-active",
      "--quiet",
      "console-notify.service",
    ])
    .output()?;
  match daemon.status.success() {
    true => {}
    false => return Ok(NoticeDelivery::Deferred),
  }

  let Ok(notify_send) = Program::NotifySend.name();
  let body = format!("{tag} is available. Open Console Updates to choose when to install it.");
  checked(desktop_command()?.args([
    notify_send,
    "--app-name=Console Updates",
    "--expire-time=0",
    "Console update available",
    &body,
  ]))?;
  Ok(NoticeDelivery::Posted)
}

fn newer(live: &Path, tag: &str) -> Result<bool, UpdateError> {
  Ok(marker(live)?.is_none_or(|(installed, _)| version(tag) > version(&installed)))
}

fn latest_update(live: &Path) -> Result<Option<Release>, UpdateError> {
  let latest = release(None)?;
  Ok(newer(live, &latest.tag)?.then_some(latest))
}

fn prompt_release(
  update: Result<Option<Release>, UpdateError>,
  notify: impl FnOnce(&str, &str) -> Result<(), UpdateError>,
) -> Result<Option<Release>, UpdateError> {
  match update {
    Ok(Some(release)) => Ok(Some(release)),
    Ok(None) => {
      notify("Console is up to date", "")?;
      Ok(None)
    }
    Err(fault) => {
      if let Err(notice) = notify("Could not check for Console updates", &fault.to_string()) {
        eprintln!("console-update: could not show the check failure: {notice}");
      }
      Err(fault)
    }
  }
}

fn prompt(live: &Path) -> Result<(), UpdateError> {
  let update = prompt_release(latest_update(live), |title, body| {
    let Ok(mut notify) = Program::NotifySend.command();
    checked(notify.args(["--app-name=Console Updates", title, body]))
  })?;
  let Some(latest) = update else {
    return Ok(());
  };
  let tag = latest.tag;
  match output(Command::new("/usr/bin/sudo").args(["-n", "--", UPDATER, "--pattern-status"]))?
    .as_str()
  {
    "enrolled" => {
      return checked(Command::new("/usr/local/bin/console-update-pattern").arg(&tag));
    }
    "absent" => {}
    _ => return Err("the updater returned an invalid pattern status".into()),
  }
  let question = format!("Install {tag} now? This will restart Console services.");
  let accepted = Command::new("/usr/local/bin/console-confirm")
    .env(console_core_internal_programs::CONFIRM_DOES, "Install")
    .arg(question)
    .status()?;
  if accepted.success() {
    checked(Command::new("pkexec").args([UPDATER, &tag]))?;
  }
  Ok(())
}

fn release(tag: Option<&str>) -> Result<Release, UpdateError> {
  let url = match tag {
    Some(tag) => format!("{RELEASE_API}/tags/{tag}"),
    None => format!("{RELEASE_API}/latest"),
  };
  let response = curl(&url).args(["--max-filesize", "1048576"]).output()?;
  if response.status.code() == Some(CURL_COULD_NOT_RESOLVE_HOST) {
    return Err("Could not resolve api.github.com. Check your connection and try again.".into());
  }
  if !response.status.success() {
    return Err(
      format!(
        "GitHub release metadata: {}",
        String::from_utf8_lossy(&response.stderr).trim()
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

fn prepare(stage: &Path, release: &Release) -> Result<(), UpdateError> {
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

fn slot_path(releases: &Path, slot: &str) -> Result<PathBuf, UpdateError> {
  let parts = slot
    .strip_prefix("release-")
    .ok_or("bad release storage name")?;
  let (process, time) = parts.split_once('-').ok_or("bad release storage name")?;
  process.parse::<u32>()?;
  time.parse::<u128>()?;
  Ok(releases.join(slot).join("tree"))
}

impl Switch {
  fn read(text: &str) -> Result<Self, UpdateError> {
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

  fn record(&self, releases: &Path) -> Result<(), UpdateError> {
    let action = if self.restoring { "restore" } else { "apply" };
    write(
      &releases.join(PENDING),
      &format!("{}\n{}\n{}\n{action}\n", self.slot, self.before, self.after),
    )
  }
}

fn begin(live: &Path, releases: &Path, slot: String) -> Result<(), UpdateError> {
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

fn settle(live: &Path, releases: &Path, mut change: Switch) -> Result<(), UpdateError> {
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

fn remember_held(live: &Path, releases: &Path) -> Result<(), UpdateError> {
  if let Some((tag, _)) = marker(live)? {
    write(&releases.join(HELD), &tag)?;
  }
  Ok(())
}

fn apply(live: &Path, other: &Path, wanted: &str, previous: &str) -> Result<(), UpdateError> {
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
  checked(desktop_command()?.args(["/usr/local/bin/console", "health"]))
}

fn desktop_command() -> Result<Command, UpdateError> {
  let account = desktop_account()?;
  let user = account
    .split(':')
    .next()
    .filter(|user| !user.is_empty())
    .ok_or("the desktop account was not found")?;
  let mut command = Command::new("runuser");
  command.args([
    "-u",
    user,
    "--",
    "env",
    "XDG_RUNTIME_DIR=/run/user/1000",
    "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus",
  ]);
  Ok(command)
}

fn desktop_account() -> Result<String, UpdateError> {
  output(Command::new("getent").args(["passwd", "1000"]))
}

fn marker(root: &Path) -> Result<Option<(String, String)>, UpdateError> {
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

fn optional(path: &Path) -> Result<Option<String>, UpdateError> {
  match fs::read_to_string(path) {
    Ok(text) => Ok(Some(text)),
    Err(fault) if fault.kind() == ErrorKind::NotFound => Ok(None),
    Err(fault) => Err(fault.into()),
  }
}

fn write(path: &Path, text: &str) -> Result<(), UpdateError> {
  console_core_atomic_writes::whole(path, text.as_bytes()).map_err(Into::into)
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, UpdateError> {
  output(
    Command::new("git")
      .env("GIT_TERMINAL_PROMPT", "0")
      .arg("-C")
      .arg(root)
      .args(arguments),
  )
}

fn output(command: &mut Command) -> Result<String, UpdateError> {
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

fn checked(command: &mut Command) -> Result<(), UpdateError> {
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

  #[test]
  fn a_background_check_leaves_an_interrupted_install_for_the_user() {
    assert!(
      check(
        Path::new("/unused-checkout"),
        Path::new("/unused-storage"),
        true
      )
      .is_ok()
    );
  }

  #[test]
  fn a_failed_check_shows_its_reason_and_a_successful_check_can_say_up_to_date() {
    let reason = "Could not resolve api.github.com. Check your connection and try again.";
    let failed = prompt_release(Err(reason.into()), |title, body| {
      assert_eq!(title, "Could not check for Console updates");
      assert_eq!(body, reason);
      Ok(())
    });
    assert_eq!(
      failed.err().map(|fault| fault.to_string()).as_deref(),
      Some(reason)
    );

    let checked = prompt_release(Ok(None), |title, body| {
      assert_eq!(title, "Console is up to date");
      assert!(body.is_empty());
      Ok(())
    });
    assert!(matches!(checked, Ok(None)));
  }

  #[test]
  fn an_update_notice_is_remembered_only_after_delivery() -> Result<(), UpdateError> {
    let releases = std::env::temp_dir().join(format!(
      "console-update-notice-{}-{}",
      std::process::id(),
      SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    DirBuilder::new().mode(0o700).create(&releases)?;
    let tag = "legion-go-v1.2.3";
    let noticed = releases.join(NOTICED);

    offer_update(&releases, tag, |_| Ok(NoticeDelivery::Deferred))?;
    assert_eq!(optional(&noticed)?, None);

    let failed = offer_update(&releases, tag, |_| Err("notification failed".into()));
    assert!(failed.is_err());
    assert_eq!(optional(&noticed)?, None);

    offer_update(&releases, tag, |available| {
      assert_eq!(available, tag);
      Ok(NoticeDelivery::Posted)
    })?;
    assert_eq!(optional(&noticed)?.as_deref(), Some(tag));
    offer_update(&releases, tag, |_| Err("duplicate notification".into()))?;

    let next = "legion-go-v1.2.4";
    write(&releases.join(HELD), next)?;
    offer_update(&releases, next, |_| Err("held release notification".into()))?;
    assert_eq!(optional(&noticed)?.as_deref(), Some(tag));

    fs::remove_dir_all(releases)?;
    Ok(())
  }
}
