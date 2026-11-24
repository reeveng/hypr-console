//! Fetch a published Legion Go release and apply its tag as one generation.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder};
use std::io::ErrorKind;
use std::os::fd::AsFd;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::{SocketAddr, UnixListener};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_state_machine::SingleEffectMachine;

#[path = "console-update/installation.rs"]
mod installation;
#[path = "console-update/menu.rs"]
mod menu;
#[path = "console-update/notices.rs"]
mod notices;
#[path = "console-update/pattern.rs"]
mod pattern;
#[path = "console-update/release_metadata.rs"]
mod release_metadata;
#[path = "console-update/release_switch.rs"]
mod release_switch;

use notices::Delivery as NoticeDelivery;
use release_switch::{
  Direction, Finalize, LiveRevision, ReleaseSwitch, Report, State, SwitchEffect,
};

const REPOSITORY: &str = "https://github.com/reeveng/hypr-console";
const RELEASE_API: &str = "https://api.github.com/repos/reeveng/hypr-console/releases";
const ASSET: &str = "legion-go-x86_64.tar.gz";
const LIVE: &str = console_repository::DEVICE_ROOT;
const RELEASES: &str = "/etc/.console-releases";
const MARK: &str = ".git/console-release";
const PENDING: &str = "pending";
const PREVIOUS: &str = "previous";
const HELD: &str = "held-tag";
const NOTICED: &str = "noticed-tag";
const CURL_COULD_NOT_RESOLVE_HOST: i32 = 6;
const PREPARATION_NOTICE_MS: u32 = 1_800_000;
const USAGE: &str = "Usage: console-update [legion-go-vX.Y.Z | --check | --prompt | --rollback | --enroll-pattern | --forget-pattern]";

type UpdateError = Box<dyn Error>;

enum Request {
  Help,
  Latest,
  Check,
  Prompt,
  Tag(String),
  Rollback,
  RollbackStatus,
  Undo(String),
  EnrollPattern,
  ForgetPattern,
  PatternStatus,
  PatternEntry,
  VerifyPattern,
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
  direction: Direction,
}

fn main() -> ExitCode {
  let Ok(caller) = console_core_places::sudo_invoker();

  match run(caller.as_deref()) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("console-update: {fault}");
      ExitCode::FAILURE
    }
  }
}

fn parse_request(arguments: &[OsString]) -> Result<Request, UpdateError> {
  match arguments {
    [] => Ok(Request::Latest),
    [word] => match word.to_str() {
      Some("--help") => Ok(Request::Help),
      Some("--check") => Ok(Request::Check),
      Some("--prompt") => Ok(Request::Prompt),
      Some("--rollback") => Ok(Request::Rollback),
      Some("--rollback-status") => Ok(Request::RollbackStatus),
      Some("--enroll-pattern") => Ok(Request::EnrollPattern),
      Some("--forget-pattern") => Ok(Request::ForgetPattern),
      Some("--pattern-status") => Ok(Request::PatternStatus),
      Some("--install-with-pattern") => Ok(Request::PatternEntry),
      Some("--verify-pattern-request") => Ok(Request::VerifyPattern),
      Some(tag) => match version(tag) {
        Ok(_version) => Ok(Request::Tag(tag.to_owned())),
        Err(InvalidVersion) => Err(UpdateError::from(USAGE)),
      },
      None => Err(UpdateError::from(USAGE)),
    },
    [flag, operand] => match (flag.to_str(), operand.to_str()) {
      (Some("--undo"), Some(target)) => {
        let _slot = undo_target(target)?;
        Ok(Request::Undo(target.to_owned()))
      }
      _ => Err(UpdateError::from(USAGE)),
    },
    [_, _, _, ..] => Err(UpdateError::from(USAGE)),
  }
}

fn run(caller: Option<&OsStr>) -> Result<(), UpdateError> {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let mut request = parse_request(&arguments)?;
  let live = Path::new(LIVE);

  match request {
    Request::Help => {
      println!("{USAGE}");
      return Ok(());
    }
    Request::Prompt => return menu::show(live),
    Request::Latest
    | Request::Check
    | Request::Tag(_)
    | Request::Rollback
    | Request::RollbackStatus
    | Request::Undo(_)
    | Request::EnrollPattern
    | Request::ForgetPattern
    | Request::PatternStatus
    | Request::PatternEntry
    | Request::VerifyPattern => {}
  }

  check_device()?;

  let order = match request {
    Request::PatternEntry => return from_desktop(caller),
    Request::VerifyPattern => {
      let input = std::io::stdin().as_fd().try_clone_to_owned()?;
      let order = pattern::Order::read(input, std::time::Instant::now)?;
      Some(order)
    }
    Request::Help
    | Request::Prompt
    | Request::Latest
    | Request::Check
    | Request::Tag(_)
    | Request::Rollback
    | Request::RollbackStatus
    | Request::Undo(_)
    | Request::EnrollPattern
    | Request::ForgetPattern
    | Request::PatternStatus => None,
  };
  let releases = Path::new(RELEASES);
  prepare_release_directory(releases)?;

  match request {
    Request::PatternStatus => {
      let enrolled = pattern::enrolled(releases)?;
      let status = match enrolled {
        pattern::Enrollment::Present => "enrolled",
        pattern::Enrollment::Absent => "absent",
      };
      println!("{status}");
      return Ok(());
    }
    Request::Help
    | Request::Prompt
    | Request::Latest
    | Request::Check
    | Request::Tag(_)
    | Request::Rollback
    | Request::RollbackStatus
    | Request::Undo(_)
    | Request::EnrollPattern
    | Request::ForgetPattern
    | Request::PatternEntry
    | Request::VerifyPattern => {}
  }

  let lock_address = SocketAddr::from_abstract_name(b"console/update")?;
  let _lock = UnixListener::bind_addr(&lock_address)?;

  match request {
    Request::EnrollPattern => return pattern::enroll(releases),
    Request::ForgetPattern => return pattern::forget(releases),
    Request::Help
    | Request::Prompt
    | Request::Latest
    | Request::Check
    | Request::Tag(_)
    | Request::Rollback
    | Request::RollbackStatus
    | Request::Undo(_)
    | Request::PatternStatus
    | Request::PatternEntry
    | Request::VerifyPattern => {}
  }

  match &order {
    Some(requested) => {
      pattern::verify(releases, requested)?;

      match &requested.action {
        pattern::Action::Install(_) => {}
        pattern::Action::Undo(target) => request = Request::Undo(target.clone()),
      }
    }
    None => {}
  }

  let pending = optional(&releases.join(PENDING))?;

  match request {
    Request::Check => return check(live, releases, pending.as_deref()),
    Request::RollbackStatus => return report_previous(live, releases, pending.as_deref()),
    Request::Help
    | Request::Prompt
    | Request::Latest
    | Request::Tag(_)
    | Request::Rollback
    | Request::Undo(_)
    | Request::EnrollPattern
    | Request::ForgetPattern
    | Request::PatternStatus
    | Request::PatternEntry
    | Request::VerifyPattern => {}
  }

  match pending {
    Some(saved_switch) => {
      match matches!(request, Request::Undo(_)).then_some(()) {
        Some(()) => return Err(UpdateError::from(UndoFault::Pending)),
        None => {}
      }

      let change = Switch::read(&saved_switch)?;

      match &order {
        Some(requested) => authenticate_resume(live, releases, &change, requested)?,
        None => {}
      }

      return settle(live, releases, change);
    }
    None => {}
  }

  request_release(live, releases, request, order)
}

fn check_device() -> Result<(), UpdateError> {
  match bool::then_some(rustix::process::geteuid().is_root(), ()) {
    Some(()) => {}
    None => return Err(UpdateError::from("run console-update as root")),
  }

  let product = fs::read_to_string("/sys/class/dmi/id/product_name")?;

  match product.trim() {
    "83E1" => Ok(()),
    _ => Err(UpdateError::from(
      "this release is for the first-generation Legion Go (83E1)",
    )),
  }
}

fn from_desktop(caller: Option<&OsStr>) -> Result<(), UpdateError> {
  let denied = || UpdateError::from("only the desktop account can submit an update pattern");

  match caller.and_then(OsStr::to_str) {
    Some("1000") => {}
    Some(_) | None => return Err(denied()),
  }

  let Ok(updater) = InternalProgram::Update.path();
  Err(UpdateError::from(
    Command::new(updater)
      .arg("--verify-pattern-request")
      .env_clear()
      .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin")
      .env("HOME", "/root")
      .current_dir("/")
      .exec(),
  ))
}

fn prepare_release_directory(releases: &Path) -> Result<(), UpdateError> {
  match DirBuilder::new().mode(0o700).create(releases) {
    Ok(()) => {}
    Err(fault) => match bool::then_some(fault.kind() == ErrorKind::AlreadyExists, ()) {
      Some(()) => {}
      None => return Err(UpdateError::from(fault)),
    },
  }

  let metadata = fs::symlink_metadata(releases)?;
  let private = metadata.is_dir() && metadata.uid() == 0 && metadata.mode() & 0o077 == 0;

  match bool::then_some(private, ()) {
    Some(()) => Ok(()),
    None => Err(UpdateError::from(
      "the release state directory is not private to root",
    )),
  }
}

fn authenticate_resume(
  live: &Path,
  releases: &Path,
  change: &Switch,
  order: &pattern::Order,
) -> Result<(), UpdateError> {
  let head = git(live, &["rev-parse", "HEAD"])?;
  let target = match bool::then_some(head == change.after, ()) {
    Some(()) => live.to_owned(),
    None => slot_path(releases, &change.slot)?,
  };
  let state = match change.direction {
    Direction::Apply => marker(&target)?,
    Direction::Restore => None,
  };

  let matches_order = match (&order.action, state) {
    (pattern::Action::Install(requested), Some((tag, _))) => *requested == tag,
    (pattern::Action::Install(_), None) | (pattern::Action::Undo(_), _) => false,
  };

  match matches_order.then_some(()) {
    Some(()) => Ok(()),
    None => Err(UpdateError::from(
      "an administrator must resume the pending release",
    )),
  }
}

fn request_release(
  live: &Path,
  releases: &Path,
  request: Request,
  order: Option<pattern::Order>,
) -> Result<(), UpdateError> {
  let status = git(live, &["status", "--porcelain"])?;

  match status.as_str() {
    "" => {}
    _ => {
      return Err(UpdateError::from(
        "the current checkout has uncommitted changes",
      ));
    }
  }

  let tag = match &request {
    Request::Rollback => {
      let previous = optional(&releases.join(PREVIOUS))?;
      let saved = previous.ok_or(UndoFault::Absent)?;
      return restore_release(live, releases, saved.trim());
    }
    Request::Undo(target) => return undo(live, releases, target),
    Request::Tag(tag) => Some(tag.as_str()),
    Request::Latest
    | Request::Check
    | Request::Help
    | Request::Prompt
    | Request::RollbackStatus
    | Request::EnrollPattern
    | Request::ForgetPattern
    | Request::PatternStatus
    | Request::PatternEntry
    | Request::VerifyPattern => None,
  };
  let release = release(tag)?;

  match order {
    Some(requested) => validate_order(live, &release, &requested)?,
    None => {}
  }

  let installed = marker(live)?;

  match installed {
    Some((installed_tag, installed_commit)) => {
      let head = git(live, &["rev-parse", "HEAD"])?;

      match bool::then_some(head == installed_commit, ()) {
        Some(()) => {}
        None => {
          return Err(UpdateError::from(
            "the current checkout has diverged from its installed release",
          ));
        }
      }

      match bool::then_some(release.tag == installed_tag, ()) {
        Some(()) => {
          println!("Console is on {installed_tag}.");
          return Ok(());
        }
        None => {}
      }
    }
    None => {}
  }

  let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?;
  let slot = format!("release-{}-{}", std::process::id(), stamp.as_nanos());
  install_release(live, releases, &slot, &release)
}

fn validate_order(
  live: &Path,
  release: &Release,
  order: &pattern::Order,
) -> Result<(), UpdateError> {
  const CHANGED: &str = "the selected release changed. Open Console Updates again";

  let matches_order = match &order.action {
    pattern::Action::Install(tag) => *tag == release.tag,
    pattern::Action::Undo(_) => false,
  };

  match matches_order.then_some(()) {
    Some(()) => {}
    None => return Err(UpdateError::from(CHANGED)),
  }

  let age = newer(live, &release.tag)?;

  match age {
    ReleaseAge::Available => Ok(()),
    ReleaseAge::Current => Err(UpdateError::from(CHANGED)),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
  major: u32,
  minor: u32,
  patch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InvalidVersion;

fn version(tag: &str) -> Result<Version, InvalidVersion> {
  let numbers = tag.strip_prefix("legion-go-v").ok_or(InvalidVersion)?;
  let mut parts = numbers.split('.');
  let parse = |part: Option<&str>| {
    let digits = part.ok_or(InvalidVersion)?;
    digits.parse::<u32>().map_err(|_| InvalidVersion)
  };
  let major = parse(parts.next())?;
  let minor = parse(parts.next())?;
  let patch = parse(parts.next())?;

  match (parts.next().is_none() && tag == format!("legion-go-v{major}.{minor}.{patch}"))
    .then_some(())
  {
    Some(()) => Ok(Version {
      major,
      minor,
      patch,
    }),
    None => Err(InvalidVersion),
  }
}

fn check(live: &Path, releases: &Path, pending: Option<&str>) -> Result<(), UpdateError> {
  match pending {
    Some(_) => {
      println!("A release is pending. Run console-update as root to resume it.");
      return Ok(());
    }
    None => {}
  }

  let latest = latest_update(live)?;

  match latest {
    Some(release) => offer_update(releases, &release.tag, notify_update),
    None => {
      println!("Console is up to date.");
      Ok(())
    }
  }
}

fn offer_update(
  releases: &Path,
  tag: &str,
  notify: impl Fn(&str) -> Result<NoticeDelivery, UpdateError>,
) -> Result<(), UpdateError> {
  use notices::{Effect, Notice, Report, State};

  let Ok(first) = Notice::initialize(&(), None);
  let outcome = Notice::run_effects(first, 3, &mut (), |(), effect| match effect {
    Effect::Inspect => Report::Inspected(notice_eligibility(releases, tag)),
    Effect::Post => Report::Posted(notify(tag)),
    Effect::Remember => Report::Remembered(write(&releases.join(NOTICED), tag)),
  });

  match outcome {
    Ok(State::Finished(result)) => result,
    Ok(State::Inspecting | State::Posting | State::Remembering) | Err(_) => {
      Err(UpdateError::from("update notification did not finish"))
    }
  }
}

fn notice_eligibility(releases: &Path, tag: &str) -> Result<notices::Eligibility, UpdateError> {
  use notices::Eligibility;

  let held = optional(&releases.join(HELD))?;
  let available = version(tag).map_err(|_| "invalid offered release tag")?;
  let held_back = match held {
    Some(saved_tag) => match version(saved_tag.trim()) {
      Ok(held_version) => held_version >= available,
      Err(InvalidVersion) => false,
    },
    None => false,
  };

  match ::core::primitive::bool::then_some(held_back, ()) {
    Some(()) => return Ok(Eligibility::Suppressed),
    None => {}
  }

  let seen = optional(&releases.join(NOTICED))?;
  Ok(
    match ::core::primitive::bool::then_some(
      seen
        .as_deref()
        .is_some_and(|seen_tag| seen_tag.trim() == tag),
      (),
    ) {
      Some(()) => Eligibility::Suppressed,
      None => Eligibility::Offer,
    },
  )
}

fn notify_update(tag: &str) -> Result<NoticeDelivery, UpdateError> {
  let Ok(systemctl) = Program::Systemctl.name();
  let mut desktop = desktop_command()?;
  let daemon = desktop
    .args([
      systemctl,
      "--user",
      "is-active",
      "--quiet",
      "console-notify.service",
    ])
    .output()?;

  match ::core::primitive::bool::then_some(daemon.status.success(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(NoticeDelivery::Deferred),
  }

  let body = format!("{tag} is available. Open Console Updates to choose when to install it.");
  let options = console_operating_system_linux::notification::Options {
    app_name: Some("Console Updates"),
    expire_ms: Some(0),
    ..Default::default()
  };
  let Ok(arguments) = options.invocation("Console update available", Some(&body));
  let mut notify = desktop_command()?;
  checked(notify.args(arguments))?;
  Ok(NoticeDelivery::Posted)
}

enum ReleaseAge {
  Available,
  Current,
}

fn newer(live: &Path, tag: &str) -> Result<ReleaseAge, UpdateError> {
  let installed = marker(live)?;
  let available = version(tag).map_err(|_| "invalid offered release tag")?;
  let offered = match installed {
    Some((current, _)) => {
      let installed_version =
        version(&current).map_err(|_| "invalid installed release record")?;
      available > installed_version
    }
    None => true,
  };

  Ok(match bool::then_some(offered, ()) {
    Some(()) => ReleaseAge::Available,
    None => ReleaseAge::Current,
  })
}

fn latest_update(live: &Path) -> Result<Option<Release>, UpdateError> {
  let latest = release(None)?;
  let age = newer(live, &latest.tag)?;

  Ok(match age {
    ReleaseAge::Available => Some(latest),
    ReleaseAge::Current => None,
  })
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
      match notify("Could not check for Console updates", &fault.to_string()) {
        Err(notice) => {
          eprintln!("console-update: could not show the check failure: {notice}");
        }
        ::core::result::Result::Ok(_) => {}
      }

      Err(fault)
    }
  }
}

fn prompt(live: &Path) -> Result<(), UpdateError> {
  let update = prompt_release(latest_update(live), |title, body| {
    let options = console_operating_system_linux::notification::Options {
      app_name: Some("Console Updates"),
      ..Default::default()
    };
    let Ok(mut notify) = options.command(title, Some(body));
    checked(&mut notify)
  })?;

  let latest = match update {
    Some(matched_latest) => matched_latest,
    ::core::option::Option::None => return Ok(()),
  };

  let tag = latest.tag;

  let Ok(updater) = InternalProgram::Update.path();
  let Ok(mut sudo) = Program::Sudo.command_from(Path::new("/usr/bin"));

  let status = output(sudo.args(["-n", "--", updater, "--pattern-status"]))?;

  match status.as_str() {
    "enrolled" => {
      let Ok(dialog) = InternalProgram::UpdatePattern.path();
      return checked(Command::new(dialog).arg(&tag));
    }
    "absent" => {}
    _ => {
      return Err(UpdateError::from(
        "the updater returned an invalid pattern status",
      ));
    }
  }

  let question = format!("Install {tag} now? This will restart Console services.");
  let Ok(confirm) = InternalProgram::Confirm.path();
  let accepted = Command::new(confirm)
    .env(console_core_internal_programs::CONFIRM_DOES, "Install")
    .arg(question)
    .status()?;

  match { ::core::primitive::bool::then_some(accepted.success(), ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(mut pkexec) = Program::Pkexec.command();
      checked(pkexec.args([updater, &tag]))?;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn release(tag: Option<&str>) -> Result<Release, UpdateError> {
  let url = match tag {
    Some(tag_2) => format!("{RELEASE_API}/tags/{tag}", tag = tag_2),
    None => format!("{RELEASE_API}/latest"),
  };
  let Ok(mut metadata) = curl(&url);
  let maximum = release_metadata::MAX_METADATA.to_string();
  let response = metadata.args(["--max-filesize", &maximum]).output()?;

  match {
    ::core::primitive::bool::then_some(
      response.status.code() == Some(CURL_COULD_NOT_RESOLVE_HOST),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Err(UpdateError::from(
        "Could not resolve api.github.com. Check your connection and try again.",
      ));
    }
    ::core::option::Option::None => {}
  }

  match { ::core::primitive::bool::then_some(!response.status.success(), ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(details) = diagnostic(&response.stderr);
      return Err(UpdateError::from(format!(
        "GitHub release metadata: {details}"
      )));
    }
    ::core::option::Option::None => {}
  }

  parse_release(&response.stdout, tag)
}

fn parse_release(response: &[u8], requested: Option<&str>) -> Result<Release, UpdateError> {
  use release_metadata::Publication;

  let published = release_metadata::decode(response)?;
  let unsuitable = "GitHub did not report an immutable release with a Legion Go bundle";

  match published.publication {
    Publication::ImmutableRelease => {}
    Publication::Ineligible => return Err(UpdateError::from(unsuitable)),
  }

  let mut uploaded = published
    .assets
    .into_iter()
    .filter(|asset| asset.name == ASSET && asset.state == "uploaded");
  let asset = uploaded.next().ok_or(unsuitable)?;

  match { ::core::primitive::bool::then_some(uploaded.next().is_some(), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(UpdateError::from(
        "GitHub did not report exactly one release bundle",
      ));
    }
    ::core::option::Option::None => {}
  }

  let digest = asset
    .digest
    .as_deref()
    .and_then(|digest| digest.strip_prefix("sha256:"))
    .ok_or("the release has no SHA-256 digest")?;
  let bytes = asset.size.ok_or("the release has no asset size")?;
  let _tag_version = version(&published.tag_name)
    .map_err(|_| "GitHub returned invalid Legion Go release metadata")?;

  match {
    ::core::primitive::bool::then_some(
      requested.is_some_and(|tag| tag != published.tag_name)
        || !matches!(hex::<64>(digest), Ok(Hex::Valid))
        || bytes == 0
        || bytes >= 2_147_483_648,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Err(UpdateError::from(
        "GitHub returned invalid Legion Go release metadata",
      ));
    }
    ::core::option::Option::None => {}
  }

  Ok(Release {
    tag: published.tag_name,
    digest: digest.to_owned(),
    bytes,
  })
}

#[must_use]
fn curl(url: &str) -> Result<Command, Never> {
  let Ok(mut curl) = Program::Curl.command();
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
  Ok(curl)
}

fn install_release(
  live: &Path,
  releases: &Path,
  slot: &str,
  release: &Release,
) -> Result<(), UpdateError> {
  use installation::{Effect, Installation, State};

  let storage = ReleaseSlot::parse(slot)?;
  let stage = releases.join(storage);
  fs::create_dir(&stage)?;
  let Ok(first) = Installation::initialize(&(), None);
  let mut progress = ProgressNotice::default();
  let outcome = Installation::run_effects(first, 3, &mut progress, |notice, effect| {
    let (title, expiry) = match effect {
      Effect::Download => ("Downloading Console update", Some(PREPARATION_NOTICE_MS)),
      Effect::Verify => ("Verifying Console update", Some(PREPARATION_NOTICE_MS)),
      Effect::Install => ("Installing Console update", None),
    };
    println!("{title} {}.", release.tag);
    let Ok(()) = notice.show(Progress {
      title,
      release: &release.tag,
      expire_ms: expiry,
    });

    let result = match effect {
      Effect::Download => download(&stage, release),
      Effect::Verify => verify_bundle(&stage, release),
      Effect::Install => begin(live, releases, slot),
    };

    match (effect, result) {
      (Effect::Download | Effect::Verify, Err(fault)) => {
        let Ok(()) = notice.show(Progress {
          title: "Could not prepare Console update",
          release: &release.tag,
          expire_ms: None,
        });
        fs::remove_dir_all(&stage)
          .map_err(UpdateError::from)
          .and(Err(fault))
      }
      (_, result_2) => result_2,
    }
  });

  match outcome {
    Ok(State::Finished(result)) => result,
    Ok(State::Downloading | State::Verifying | State::Installing) | Err(_) => {
      Err(UpdateError::from("the release installation did not finish"))
    }
  }
}

#[derive(Default)]
struct ProgressNotice {
  id: Option<u32>,
}

struct Progress<'a> {
  title: &'a str,
  release: &'a str,
  expire_ms: Option<u32>,
}

impl ProgressNotice {
  fn show(
    &mut self,
    Progress {
      title,
      release,
      expire_ms,
    }: Progress<'_>,
  ) -> Result<(), Never> {
    let options = console_operating_system_linux::notification::Options {
      app_name: Some("Console Updates"),
      expire_ms,
      id_output: console_operating_system_linux::notification::IdOutput::Requested,
      replace_id: self.id,
      ..Default::default()
    };
    let Ok(arguments) = options.invocation(title, Some(release));

    match desktop_command().and_then(|mut desktop| output(desktop.args(arguments))) {
      Ok(reply) => {
        self.id = match reply.parse() {
          Ok(id) => Some(id),
          Err(fault) => {
            eprintln!("console-update: could not read the progress notification ID: {fault}");
            None
          }
        };
      }
      Err(fault) => eprintln!("console-update: could not show update progress: {fault}"),
    }

    Ok(())
  }
}

fn download(stage: &Path, release: &Release) -> Result<(), UpdateError> {
  let archive = stage.join(ASSET);
  let url = format!("{REPOSITORY}/releases/download/{}/{ASSET}", release.tag);
  let Ok(mut download) = curl(&url);
  checked(
    download
      .args(["--max-filesize", &release.bytes.to_string(), "--output"])
      .arg(&archive),
  )
}

fn verify_bundle(stage: &Path, release: &Release) -> Result<(), UpdateError> {
  let archive = stage.join(ASSET);
  let Ok(mut checksum) = Program::Sha256sum.command();
  let digest = output(checksum.arg(&archive))?;
  let file = fs::metadata(&archive)?;
  let same = file.len() == release.bytes
    && digest.split_whitespace().next() == Some(release.digest.as_str());

  match bool::then_some(same, ()) {
    Some(()) => {}
    None => {
      return Err(UpdateError::from(
        "the download does not match GitHub's published asset digest",
      ));
    }
  }

  let unpacked = unpack_bundle(stage)?;
  let checkout = stage.join("tree");
  fs::create_dir(&checkout)?;
  let Ok(mut initialize) = Program::Git.command();
  checked(
    initialize
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

  match {
    let unpacked_revision = fs::read_to_string(unpacked.join("revision"))?;

    ::core::primitive::bool::then_some(unpacked_revision.trim() != commit, ())
  } {
    ::core::option::Option::Some(()) => {
      return Err(UpdateError::from(
        "the bundle was built from a different commit than the published tag",
      ));
    }
    ::core::option::Option::None => {}
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

fn unpack_bundle(stage: &Path) -> Result<PathBuf, UpdateError> {
  let archive = stage.join(ASSET);
  let Ok(mut list) = Program::Tar.command();
  let listing = output(list.arg("-tzf").arg(&archive))?;
  let mut members = BTreeSet::new();

  for entry in listing.lines() {
    let member = BundleMember::parse(entry)?;
    let unique = members.insert(member);

    match bool::then_some(unique, ()) {
      Some(()) => {}
      None => {
        return Err(UpdateError::from(format!(
          "unexpected bundle member {entry:?}"
        )));
      }
    }
  }

  let Ok(console) = InternalProgram::Console.name();
  let Ok(updater) = InternalProgram::Update.name();
  let required = members.contains(&BundleMember("revision"))
    && members.contains(&BundleMember(format!("bin/{console}").as_str()))
    && members.contains(&BundleMember(format!("bin/{updater}").as_str()));

  match bool::then_some(required, ()) {
    Some(()) => {}
    None => {
      return Err(UpdateError::from(
        "the bundle is missing its revision or one of its installers",
      ));
    }
  }

  let unpacked = stage.join("unpacked");
  fs::create_dir(&unpacked)?;
  let Ok(mut tar) = Program::Tar.command();
  checked(
    tar
      .args(["--no-same-owner", "--no-same-permissions", "-xzf"])
      .arg(&archive)
      .arg("-C")
      .arg(&unpacked),
  )?;

  for entry in members {
    let regular = fs::symlink_metadata(unpacked.join(entry)).is_ok_and(|file| file.is_file());

    match bool::then_some(regular, ()) {
      Some(()) => {}
      None => {
        return Err(UpdateError::from(
          "the bundle contains an entry that is not a regular file",
        ));
      }
    }
  }

  Ok(unpacked)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BundleMember<'a>(&'a str);

impl<'a> BundleMember<'a> {
  fn parse(path: &'a str) -> Result<Self, UpdateError> {
    let program = path.strip_prefix("bin/").is_some_and(|name| {
      !name.is_empty()
        && name
          .bytes()
          .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    });

    match bool::then_some(path == "revision" || program, ()) {
      Some(()) => Ok(Self(path)),
      None => Err(UpdateError::from(format!(
        "unexpected bundle member {path:?}"
      ))),
    }
  }
}

impl AsRef<Path> for BundleMember<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

enum Hex {
  Valid,
  Invalid,
}

fn hex<const LENGTH: u8>(text: &str) -> Result<Hex, Never> {
  let valid = u8::try_from(text.len()) == Ok(LENGTH)
    && text
      .bytes()
      .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));

  Ok(match bool::then_some(valid, ()) {
    Some(()) => Hex::Valid,
    None => Hex::Invalid,
  })
}

fn slot_path(releases: &Path, slot: &str) -> Result<PathBuf, UpdateError> {
  let storage = ReleaseSlot::parse(slot)?;

  Ok(releases.join(storage).join("tree"))
}

#[derive(Clone)]
struct PreviousRelease {
  target: String,
  label: String,
}

#[derive(Debug)]
enum UndoFault {
  InvalidTarget,
  Absent,
  Changed,
  Pending,
}

impl std::fmt::Display for UndoFault {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    to.write_str(match self {
      Self::InvalidTarget => "invalid undo target",
      Self::Absent => "there is no previous release",
      Self::Changed => "the previous release changed. Open Console Updates again",
      Self::Pending => "a release switch is already pending",
    })
  }
}

impl Error for UndoFault {}

fn undo_target(target: &str) -> Result<&str, UndoFault> {
  let inner = target.strip_prefix("undo:").ok_or(UndoFault::InvalidTarget)?;
  let (slot, revision) = inner.split_once(':').ok_or(UndoFault::InvalidTarget)?;
  let _slot = ReleaseSlot::parse(slot).map_err(|_| UndoFault::InvalidTarget)?;

  match hex::<40>(revision) {
    Ok(Hex::Valid) => Ok(slot),
    Ok(Hex::Invalid) => Err(UndoFault::InvalidTarget),
  }
}

fn previous_release(live: &Path, releases: &Path) -> Result<Option<PreviousRelease>, UpdateError> {
  let previous = optional(&releases.join(PREVIOUS))?;
  let slot = match previous.as_deref() {
    Some(slot) => slot.trim(),
    None => return Ok(None),
  };
  let checkout = slot_path(releases, slot)?;
  let revision = git(&checkout, &["rev-parse", "HEAD"])?;
  let current = git(live, &["rev-parse", "HEAD"])?;

  match bool::then_some(current == revision, ()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let installed = marker(&checkout)?;
  let label = match installed {
    Some((tag, _)) => tag,
    None => "Previous installation".to_owned(),
  };
  Ok(Some(PreviousRelease {
    target: format!("undo:{slot}:{revision}"),
    label,
  }))
}

fn report_previous(live: &Path, releases: &Path, pending: Option<&str>) -> Result<(), UpdateError> {
  match pending {
    Some(_) => println!("pending"),
    None => {
      let previous = previous_release(live, releases)?;

      match previous {
        Some(previous_release) => println!("{}\n{}", previous_release.target, previous_release.label),
        None => println!("none"),
      }
    }
  }

  Ok(())
}

fn undo(live: &Path, releases: &Path, expected: &str) -> Result<(), UpdateError> {
  let previous = previous_release(live, releases)?;
  let saved = previous.ok_or(UndoFault::Absent)?;
  let changed = expected != saved.target;

  match changed.then_some(()) {
    Some(()) => return Err(UpdateError::from(UndoFault::Changed)),
    None => {}
  }

  let slot = undo_target(&saved.target)?;
  restore_release(live, releases, slot)
}

fn restore_release(live: &Path, releases: &Path, slot: &str) -> Result<(), UpdateError> {
  remember_held(live, releases)?;
  println!("Restoring the previous Console release.");
  begin(live, releases, slot)
}

struct ReleaseSlot<'a>(&'a str);

impl<'a> ReleaseSlot<'a> {
  fn parse(slot: &'a str) -> Result<Self, UpdateError> {
    let parts = slot
      .strip_prefix("release-")
      .ok_or("bad release storage name")?;
    let (process, time) = parts.split_once('-').ok_or("bad release storage name")?;
    process.parse::<u32>()?;
    time.parse::<u128>()?;

    Ok(Self(slot))
  }
}

impl AsRef<Path> for ReleaseSlot<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

impl Switch {
  fn read(text: &str) -> Result<Self, UpdateError> {
    let [slot, before, after, action]: [&str; 4] = text
      .lines()
      .collect::<Vec<_>>()
      .try_into()
      .map_err(|_| "invalid pending release record")?;

    match {
      ::core::primitive::bool::then_some(
        !matches!(hex::<40>(before), Ok(Hex::Valid))
          || !matches!(hex::<40>(after), Ok(Hex::Valid))
          || before == after,
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(UpdateError::from("invalid pending release record"));
      }
      ::core::option::Option::None => {}
    }

    let direction = match action {
      "apply" => Direction::Apply,
      "restore" => Direction::Restore,
      _ => return Err(UpdateError::from("invalid pending release record")),
    };

    Ok(Self {
      slot: slot.to_owned(),
      before: before.to_owned(),
      after: after.to_owned(),
      direction,
    })
  }

  fn record(&self, releases: &Path) -> Result<(), UpdateError> {
    let action = match self.direction {
      Direction::Apply => "apply",
      Direction::Restore => "restore",
    };
    write(
      &releases.join(PENDING),
      &format!("{}\n{}\n{}\n{action}\n", self.slot, self.before, self.after),
    )
  }
}

fn begin(live: &Path, releases: &Path, slot: &str) -> Result<(), UpdateError> {
  let other = slot_path(releases, slot)?;
  let before = git(live, &["rev-parse", "HEAD"])?;
  let after = git(&other, &["rev-parse", "HEAD"])?;

  match { ::core::primitive::bool::then_some(before == after, ()) } {
    ::core::option::Option::Some(()) => {
      println!("The checkout already has this release's revision.");
      return Ok(());
    }
    ::core::option::Option::None => {}
  }

  let change = Switch {
    slot: slot.to_owned(),
    before,
    after,
    direction: Direction::Apply,
  };
  change.record(releases)?;
  settle(live, releases, change)
}

fn settle(live: &Path, releases: &Path, mut change: Switch) -> Result<(), UpdateError> {
  const MAX_EFFECTS_WITH_RESTORE: u32 = 7;
  let other = slot_path(releases, &change.slot)?;
  let Ok(first) = ReleaseSwitch::initialize(&change.direction, None);
  let outcome = ReleaseSwitch::run_effects(
    first,
    MAX_EFFECTS_WITH_RESTORE,
    &mut change,
    |pending, effect| match switch_effect(live, releases, &other, pending, effect) {
      Ok(report) => report,
      Err(fault) => Report::Failed(fault),
    },
  );

  match outcome {
    Ok(State::Finished(result)) => result,
    Ok(
      State::Activating
      | State::Verifying(_)
      | State::CheckingFailure(_)
      | State::RecordingRestore(_)
      | State::Restoring(_)
      | State::Finalizing(_),
    )
    | Err(_) => Err(UpdateError::from("the release switch did not finish")),
  }
}

fn switch_effect(
  live: &Path,
  releases: &Path,
  other: &Path,
  change: &mut Switch,
  effect: SwitchEffect,
) -> Result<Report, UpdateError> {
  match effect {
    SwitchEffect::Activate => Ok(Report::Activated(activate(
      live,
      other,
      Revisions {
        wanted: &change.after,
        previous: &change.before,
      },
    ))),
    SwitchEffect::Verify => {
      let Ok(console) = InternalProgram::Console.path();
      Ok(Report::Verified(desktop_command().and_then(
        |mut command| checked(command.args([console, "health"])),
      )))
    }
    SwitchEffect::ReadLiveRevision => {
      let current = git(live, &["rev-parse", "HEAD"])?;
      let observed = match ::core::primitive::bool::then_some(current == change.after, ()) {
        Some(()) => LiveRevision::Requested,
        None => LiveRevision::Other,
      };

      Ok(Report::Observed(observed))
    }
    SwitchEffect::RecordRestore => {
      eprintln!("The release did not pass verification. Restoring the previous release.");
      remember_held(live, releases)?;
      change.direction = Direction::Restore;
      change.record(releases)?;

      Ok(Report::RestoreRecorded)
    }
    SwitchEffect::Restore => {
      activate(
        live,
        other,
        Revisions {
          wanted: &change.before,
          previous: &change.after,
        },
      )?;

      Ok(Report::Restored)
    }
    SwitchEffect::Finalize(outcome) => {
      finalize_switch(releases, other, change, outcome)?;

      Ok(Report::Finalized)
    }
  }
}

fn finalize_switch(
  releases: &Path,
  other: &Path,
  change: &Switch,
  outcome: Finalize,
) -> Result<(), UpdateError> {
  match outcome {
    Finalize::Activated => {
      let previous = optional(&releases.join(PREVIOUS))?;
      write(&releases.join(PREVIOUS), &change.slot)?;
      fs::remove_file(releases.join(PENDING))?;

      match previous.filter(|saved| saved.trim() != change.slot) {
        Some(saved) => {
          retire_release(releases, saved.trim())?;
        }
        None => {}
      }

      println!("Console release {} is active.", change.after);
    }
    Finalize::Restored => fs::remove_file(releases.join(PENDING))?,
    Finalize::FailedActivation => {
      fs::remove_file(releases.join(PENDING))?;
      let previous = optional(&releases.join(PREVIOUS))?;

      match ::core::primitive::bool::then_some(
        previous
          .as_deref()
          .is_none_or(|slot| slot.trim() != change.slot),
        (),
      ) {
        Some(()) => {
          let tree = other.parent().ok_or("invalid release path")?;
          fs::remove_dir_all(tree)?;
        }
        None => {}
      }
    }
  }

  Ok(())
}

fn retire_release(releases: &Path, slot: &str) -> Result<(), UpdateError> {
  let retired = slot_path(releases, slot)?;
  let known = marker(&retired)?;

  match known {
    Some(_) => {
      let tree = retired.parent().ok_or("invalid retired release path")?;
      fs::remove_dir_all(tree)?;
    }
    None => {}
  }

  Ok(())
}

fn remember_held(live: &Path, releases: &Path) -> Result<(), UpdateError> {
  match {
    let checked_value_5 = marker(live)?;
    checked_value_5
  } {
    Some((tag, _)) => {
      write(&releases.join(HELD), &tag)?;
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

struct Revisions<'a> {
  wanted: &'a str,
  previous: &'a str,
}

fn activate(live: &Path, other: &Path, revisions: Revisions<'_>) -> Result<(), UpdateError> {
  let Revisions { wanted, previous } = revisions;
  let current = git(live, &["rev-parse", "HEAD"])?;
  let saved = git(other, &["rev-parse", "HEAD"])?;
  let switching = current == previous && saved == wanted;

  match {
    ::core::primitive::bool::then_some(
      !switching && !(current == wanted && saved == previous),
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Err(UpdateError::from(
        "the checkout revisions changed while a release was pending",
      ));
    }
    ::core::option::Option::None => {}
  }

  let (target, old) = match ::core::primitive::bool::then_some(switching, ()) {
    ::core::option::Option::Some(()) => (other, live),
    ::core::option::Option::None => (live, other),
  };
  let record = marker(target)?;
  let checkout = match record {
    Some(_) => target,
    None => old,
  };
  const CONSOLE: &str = {
    let Ok(name) = InternalProgram::Console.name();
    name
  };
  let engine = checkout.join("target/release").join(CONSOLE);
  let mut apply = Command::new(engine);
  apply.args(["apply", "--prebuilt"]);

  match ::core::primitive::bool::then_some(switching, ()) {
    ::core::option::Option::Some(()) => {
      apply.arg("--switch-from").arg(other);
    }
    ::core::option::Option::None => {}
  }

  checked(&mut apply)?;
  let activated = git(live, &["rev-parse", "HEAD"])?;

  match ::core::primitive::bool::then_some(activated != wanted, ()) {
    ::core::option::Option::Some(()) => {
      return Err(UpdateError::from(
        "the manifest engine did not activate the requested checkout",
      ));
    }
    ::core::option::Option::None => {}
  }

  Ok(())
}

fn desktop_command() -> Result<Command, UpdateError> {
  let account = desktop_account()?;
  let user = account
    .split(':')
    .next()
    .filter(|user| !user.is_empty())
    .ok_or("the desktop account was not found")?;
  let Ok(mut command) = Program::Runuser.command();
  let Ok(env) = Program::Env.name();
  command.args([
    "-u",
    user,
    "--",
    env,
    "XDG_RUNTIME_DIR=/run/user/1000",
    "DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus",
  ]);
  Ok(command)
}

fn desktop_account() -> Result<String, UpdateError> {
  let Ok(mut getent) = Program::Getent.command();
  output(getent.args(["passwd", "1000"]))
}

fn marker(root: &Path) -> Result<Option<(String, String)>, UpdateError> {
  let record = optional(&root.join(MARK))?;
  record
    .map(|text| {
      let [tag, commit]: [&str; 2] = text
        .lines()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "invalid installed release record")?;
      let _tag_version = version(tag).map_err(|_| "invalid installed release record")?;

      match {
        ::core::primitive::bool::then_some(!matches!(hex::<40>(commit), Ok(Hex::Valid)), ())
      } {
        ::core::option::Option::Some(()) => {
          return Err(UpdateError::from("invalid installed release record"));
        }
        ::core::option::Option::None => {}
      }

      Ok((tag.to_owned(), commit.to_owned()))
    })
    .transpose()
}

fn optional(path: &Path) -> Result<Option<String>, UpdateError> {
  match fs::read_to_string(path) {
    Ok(text) => Ok(Some(text)),
    Err(fault) => match bool::then_some(fault.kind() == ErrorKind::NotFound, ()) {
      Some(()) => Ok(None),
      None => Err(UpdateError::from(fault)),
    },
  }
}

fn write(path: &Path, text: &str) -> Result<(), UpdateError> {
  console_core_atomic_writes::whole(path, text.as_bytes()).map_err(Into::into)
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, UpdateError> {
  let Ok(mut command) = Program::Git.command();
  output(
    command
      .env("GIT_TERMINAL_PROMPT", "0")
      .arg("-C")
      .arg(root)
      .args(arguments),
  )
}

fn output(command: &mut Command) -> Result<String, UpdateError> {
  let result = command.output()?;

  match { ::core::primitive::bool::then_some(!result.status.success(), ()) } {
    ::core::option::Option::Some(()) => {
      let Ok(details) = diagnostic(&result.stderr);
      return Err(UpdateError::from(format!(
        "{command:?} exited with {}: {details}",
        result.status
      )));
    }
    ::core::option::Option::None => {}
  }

  let output = String::from_utf8(result.stdout)?;

  Ok(output.trim().to_owned())
}

fn diagnostic(bytes: &[u8]) -> Result<Cow<'_, str>, Never> {
  Ok(match std::str::from_utf8(bytes) {
    Ok(message) => Cow::Borrowed(message.trim()),
    Err(_stderr_not_utf8) => Cow::Owned(format!("{bytes:?}")),
  })
}

fn checked(command: &mut Command) -> Result<(), UpdateError> {
  let status = command.status()?;

  match { ::core::primitive::bool::then_some(status.success(), ()) } {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(UpdateError::from(format!(
      "{command:?} exited with {status}"
    ))),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const TAG: &str = "legion-go-v0.2.0";
  const DIGEST: &str = "aaaaaaaabbbbbbbbccccccccddddddddeeeeeeeeffffffff0000000011111111";

  fn release_metadata() -> Result<serde_json::Value, Never> {
    Ok(serde_json::json!({
      "immutable": true,
      "draft": false,
      "prerelease": false,
      "tag_name": TAG,
      "author": {"login": "release-bot"},
      "assets": [
        {"name": ASSET, "state": "uploaded", "digest": format!("sha256:{DIGEST}"), "size": 42, "download_count": 12},
        {"name": ASSET, "state": "new"},
        {"name": "release-notes.txt", "state": "uploaded"}
      ]
    }))
  }

  #[test]
  fn the_release_parser_reads_the_one_uploaded_bundle() -> Result<(), UpdateError> {
    let Ok(fixture) = release_metadata();
    let metadata = serde_json::to_vec(&fixture)?;
    let release = parse_release(&metadata, Some(TAG))?;
    assert_eq!(release.tag, TAG);
    assert_eq!(release.digest, DIGEST);
    assert_eq!(release.bytes, 42);
    assert!(
      matches!(parse_release(&metadata, Some("legion-go-v0.1.0")), Err(fault) if fault.to_string() == "GitHub returned invalid Legion Go release metadata")
    );
    let oversized = vec![b' '; 1_048_577];
    let rejected = parse_release(&oversized, None);
    assert!(
      matches!(rejected, Err(fault) if fault.to_string() == "GitHub release metadata exceeds 1 MiB")
    );
    Ok(())
  }

  #[test]
  fn the_release_parser_rejects_unsafe_or_ambiguous_metadata() -> Result<(), UpdateError> {
    use serde_json::json;

    let unsuitable = "GitHub did not report an immutable release with a Legion Go bundle";
    let invalid = "GitHub returned invalid Legion Go release metadata";

    for (field, value, reason) in [
      ("immutable", json!(false), unsuitable),
      ("draft", json!(true), unsuitable),
      ("prerelease", json!(true), unsuitable),
      (
        "tag_name",
        json!("legion-go-v0.2.0/../../checkout"),
        invalid,
      ),
      ("assets", json!([]), unsuitable),
    ] {
      let Ok(mut metadata) = release_metadata();
      let entry = metadata
        .get_mut(field)
        .ok_or("missing release fixture field")?;
      *entry = value;
      let vec = serde_json::to_vec(&metadata)?;
      assert!(
        matches!(parse_release(&vec, None), Err(fault) if fault.to_string() == reason),
        "{field}"
      );
    }

    for (field, value, reason) in [
      ("state", json!("new"), unsuitable),
      ("digest", json!(null), "the release has no SHA-256 digest"),
      ("digest", json!("sha256:short"), invalid),
      ("size", json!(0), invalid),
      ("size", json!(2_147_483_648_u64), invalid),
    ] {
      let Ok(mut metadata) = release_metadata();
      let assets = metadata
        .get_mut("assets")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("missing release fixture assets")?;
      let asset = assets
        .first_mut()
        .ok_or("missing first release fixture asset")?;
      let entry = asset.get_mut(field).ok_or("missing asset fixture field")?;
      *entry = value;
      let vec = serde_json::to_vec(&metadata)?;
      assert!(
        matches!(parse_release(&vec, None), Err(fault) if fault.to_string() == reason),
        "{field}"
      );
    }

    let Ok(mut metadata) = release_metadata();
    let assets = metadata
      .get_mut("assets")
      .and_then(serde_json::Value::as_array_mut)
      .ok_or("missing release fixture assets")?;

    match assets.as_mut_slice() {
      [first, second, ..] => *second = first.clone(),
      _ => return Err(UpdateError::from("missing duplicate asset fixture")),
    }

    let vec = serde_json::to_vec(&metadata)?;
    assert!(
      matches!(parse_release(&vec, None), Err(fault) if fault.to_string() == "GitHub did not report exactly one release bundle")
    );
    Ok(())
  }

  #[test]
  fn release_tags_have_an_order_and_cannot_form_paths() -> Result<(), InvalidVersion> {
    let later = version("legion-go-v0.10.1")?;
    let earlier = version("legion-go-v0.9.9")?;
    assert!(later > earlier);
    assert_eq!(version("legion-go-v0.1.0/../checkout"), Err(InvalidVersion));
    assert_eq!(version("legion-go-v0.1.0-rc1"), Err(InvalidVersion));
    assert_eq!(version("legion-go-v0.1.00"), Err(InvalidVersion));
    Ok(())
  }

  #[test]
  fn release_members_are_only_the_revision_and_regular_binary_names() {
    assert!(matches!(BundleMember::parse("revision"), Ok(_)));
    assert!(matches!(BundleMember::parse("bin/console"), Ok(_)));

    for path in ["bin/../revision", "/bin/console", "bin/nested/console"] {
      let rejected = format!("unexpected bundle member {path:?}");
      assert!(matches!(BundleMember::parse(path), Err(fault) if fault.to_string() == rejected));
    }
  }

  #[test]
  fn a_background_check_leaves_an_interrupted_install_for_the_user() -> Result<(), UpdateError> {
    check(
      Path::new("/unused-checkout"),
      Path::new("/unused-storage"),
      Some("pending release"),
    )
  }

  #[test]
  fn a_failed_check_shows_its_reason_and_a_successful_check_can_say_up_to_date() {
    let reason = "Could not resolve api.github.com. Check your connection and try again.";
    let failed = prompt_release(Err(UpdateError::from(reason)), |title, body| {
      assert_eq!(title, "Could not check for Console updates");
      assert_eq!(body, reason);
      Ok(())
    });
    assert!(matches!(failed, Err(fault) if fault.to_string() == reason));

    let checked = prompt_release(Ok(None), |title, body| {
      assert_eq!(title, "Console is up to date");
      assert!(body.is_empty());
      Ok(())
    });
    assert!(matches!(checked, Ok(None)));
  }

  #[test]
  fn an_update_notice_is_remembered_only_after_delivery() -> Result<(), UpdateError> {
    let releases = console_core_temporary_directories::private("console-update-notice")?;
    let tag = "legion-go-v1.2.3";
    let noticed = releases.join(NOTICED);

    offer_update(&releases, tag, |_| Ok(NoticeDelivery::Deferred))?;
    let deferred = optional(&noticed)?;
    assert_eq!(deferred, None);

    let failed = offer_update(&releases, tag, |_| {
      Err(UpdateError::from("notification failed"))
    });
    assert!(matches!(failed, Err(fault) if fault.to_string() == "notification failed"));
    let failed_delivery = optional(&noticed)?;
    assert_eq!(failed_delivery, None);

    offer_update(&releases, tag, |available| {
      assert_eq!(available, tag);
      Ok(NoticeDelivery::Posted)
    })?;
    let delivered = optional(&noticed)?;
    assert_eq!(delivered.as_deref(), Some(tag));
    offer_update(&releases, tag, |_| {
      Err(UpdateError::from("duplicate notification"))
    })?;

    let next = "legion-go-v1.2.4";
    write(&releases.join(HELD), next)?;
    offer_update(&releases, next, |_| {
      Err(UpdateError::from("held release notification"))
    })?;
    let after_hold = optional(&noticed)?;
    assert_eq!(after_hold.as_deref(), Some(tag));

    fs::remove_dir_all(releases)?;
    Ok(())
  }
}
