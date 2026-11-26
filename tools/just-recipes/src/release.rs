//! Publish a checked snapshot on top of the public repository's main branch.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Output};

use console_core_arguments::{Flag, Operands, Presence, Reason, Takes, ValidationError};
use console_core_external_programs::Program;
use console_core_never::Never;

const MAIN: &str = "refs/heads/main";
const WORKFLOW: &str = ".github/workflows/console-release.yml";
const PRIVATE_IDENTIFIERS: Flag = Flag {
  spelling: "--private-identifiers",
  takes: Takes::Value("FILE"),
  about: "also check for identifiers from FILE, one per line",
};
const REPOSITORY: Flag = Flag {
  spelling: "--repository",
  takes: Takes::Value("PATH"),
  about: "public checkout with origin and a public Git identity (default sibling hypr-console)",
};
const NO_LATEST: Flag = Flag {
  spelling: "--no-latest",
  takes: Takes::None,
  about: "publish the release without marking it latest",
};
const PUBLISH: console_core_arguments::Command = console_core_arguments::Command {
  name: "just publish",
  about: "export a source commit (default HEAD) and push the public main branch and release tag",
  flags: &[REPOSITORY, NO_LATEST, PRIVATE_IDENTIFIERS],
  operands: Operands::Any("VERSION [SOURCE-REVISION]"),
};
const TAG: console_core_arguments::Command = console_core_arguments::Command {
  name: "just release-tag",
  about: "push a release tag for an existing public commit (default origin/main)",
  flags: &[REPOSITORY, NO_LATEST],
  operands: Operands::Any("VERSION [PUBLIC-REVISION]"),
};

struct Release {
  tag: String,
  revision: Option<String>,
  repository: Option<PathBuf>,
  no_latest: Presence,
  private_identifiers: Option<PathBuf>,
}

pub(super) fn publish_main(words: &[String]) -> Result<ExitCode, Never> {
  console_core_arguments::run_main(&PUBLISH, words, |words| parse(&PUBLISH, words), publish)
}

pub(super) fn tag_main(words: &[String]) -> Result<ExitCode, Never> {
  console_core_arguments::run_main(&TAG, words, |words| parse(&TAG, words), tag)
}

fn parse(
  command: &console_core_arguments::Command,
  words: &[String],
) -> Result<Release, ValidationError> {
  let line = console_core_arguments::read(command, words)?;
  let Ok(operands) = line.operands();
  let (version, revision) = match operands {
    [version] => (version, None),
    [version, revision] => (version, Some(revision.clone())),
    [] => {
      let Ok(refusal) = line.refusal(Reason::MissingOperands(vec!["VERSION"]));

      return Err(refusal);
    }
    [_, _, extra, ..] => {
      let Ok(refusal) = line.refusal(Reason::ExtraArgument(extra.clone()));

      return Err(refusal);
    }
  };
  let tag = match release_name(version) {
    Some(tag) => tag,
    None => {
      let Ok(refusal) = line.refusal(Reason::InvalidValue {
        of: "VERSION (X.Y.Z or console-vX.Y.Z)",
        value: version.clone(),
      });

      return Err(refusal);
    }
  };
  let Ok(repository) = line.value(REPOSITORY);
  let Ok(no_latest) = line.presence(NO_LATEST);
  let Ok(private_identifiers) = line.value(PRIVATE_IDENTIFIERS);

  Ok(Release {
    tag,
    revision,
    repository: repository.map(PathBuf::from),
    no_latest,
    private_identifiers: private_identifiers.map(PathBuf::from),
  })
}

fn release_name(version: &str) -> Option<String> {
  let number = version.strip_prefix("console-v").unwrap_or(version);
  let mut parts = number.split('.');
  let major = parts.next()?;
  let minor = parts.next()?;
  let patch = parts.next()?;
  match (
    parts.next(),
    [major, minor, patch].into_iter().all(version_component),
  ) {
    (None, true) => Some(format!("console-v{number}")),
    _invalid => None,
  }
}

fn version_component(component: &str) -> bool {
  !component.is_empty()
    && (component == "0" || !component.starts_with('0'))
    && component.bytes().all(|byte| byte.is_ascii_digit())
}

#[derive(Debug)]
enum ReleaseFault {
  Source(console_repository::NotFound),
  Io(io::Error),
  Git(ExitStatus, String),
  GitText(std::string::FromUtf8Error),
  Scratch(console_core_temporary_directories::Unmade),
  PublicCheckout(PathBuf, io::Error),
  SourceIsPublicCheckout,
  MissingWorkflow(String),
  UnsupportedSourceEntry(String),
  TrackedBuildDirectory,
  ExistingTag(String),
}

impl fmt::Display for ReleaseFault {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Source(fault) => write!(to, "{fault}"),
      Self::Io(fault) => write!(to, "{fault}"),
      Self::Git(status, details) => write!(to, "Git exited with {status}\n{details}"),
      Self::GitText(fault) => write!(to, "Git returned invalid text: {fault}"),
      Self::Scratch(fault) => write!(to, "{fault}"),
      Self::PublicCheckout(path, fault) => {
        write!(
          to,
          "could not open the hypr-console checkout at {}: {fault}; use --repository PATH to select it",
          path.display()
        )
      }
      Self::SourceIsPublicCheckout => {
        to.write_str("use a separate checkout for the public repository")
      }
      Self::MissingWorkflow(commit) => {
        write!(to, "source commit {commit} does not contain {WORKFLOW}")
      }
      Self::UnsupportedSourceEntry(entry) => {
        write!(to, "source contains a symlink or submodule: {entry}")
      }
      Self::TrackedBuildDirectory => {
        to.write_str("the source commit tracks the reserved target build directory")
      }
      Self::ExistingTag(tag) => write!(to, "the public repository already has tag {tag}"),
    }
  }
}

impl From<io::Error> for ReleaseFault {
  fn from(fault: io::Error) -> Self {
    Self::Io(fault)
  }
}

struct Scratch(PathBuf);

impl Scratch {
  fn create() -> Result<Self, ReleaseFault> {
    let base = Path::new("/var/tmp");
    let user = std::env::var_os("USER")
      .and_then(|user| Path::new(&user).file_name().map(|name| base.join(name)));
    let root = match std::env::var_os("TMPDIR").filter(|path| !path.is_empty()) {
      Some(path) => PathBuf::from(path),
      None => user
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| base.to_path_buf()),
    };
    let root = fs::canonicalize(root)?;
    console_core_temporary_directories::private_under(&root, "hypr-console-release")
      .map(Self)
      .map_err(ReleaseFault::Scratch)
  }
}

impl Drop for Scratch {
  fn drop(&mut self) {
    if let Err(fault) = fs::remove_dir_all(&self.0) {
      eprintln!(
        "could not remove release scratch directory {}: {fault}",
        self.0.display()
      );
    }
  }
}

fn publish(release: Release) -> Result<ExitCode, ReleaseFault> {
  let source = console_repository::root().map_err(ReleaseFault::Source)?;
  let revision = release.revision.as_deref().unwrap_or("HEAD");
  let commit = resolve(&source, revision)?;
  let scratch = Scratch::create()?;
  let snapshot = scratch.0.join("source");
  checkout_source(&source, &snapshot, &commit)?;

  let export = scratch.0.join("export");
  let checked = export_source(&snapshot, &export, release.private_identifiers.as_deref())?;
  if checked != ExitCode::SUCCESS {
    return Ok(checked);
  }

  let checkout = public_checkout(&source, release.repository.as_deref())?;
  let public = scratch.0.join("public");
  clone_public(&checkout, &public)?;
  require_new_tag(&public, &release.tag)?;
  let old_main = resolve(&public, "HEAD")?;
  for entry in fs::read_dir(&export)? {
    let entry = entry?;
    fs::rename(entry.path(), public.join(entry.file_name()))?;
  }
  checked_git(git(&public).args(["add", "--force", "--all"]))?;
  let message = format!("Publish {}\n\nConsole-Source-Commit: {commit}", release.tag);
  checked_git(git(&public).args(["commit", "--allow-empty", "-m", &message]))?;
  let exported = resolve(&public, "HEAD")?;
  let reference = create_tag(&public, &release, &exported)?;
  checked_git(git(&public).args([
    "push",
    "--atomic",
    "--no-follow-tags",
    &format!("--force-with-lease={MAIN}:{old_main}"),
    "origin",
    &format!("HEAD:{MAIN}"),
    &reference,
  ]))?;
  println!(
    "Pushed {} on {} from source {}.",
    release.tag, exported, commit
  );

  Ok(ExitCode::SUCCESS)
}

fn checkout_source(source: &Path, snapshot: &Path, commit: &str) -> Result<(), ReleaseFault> {
  let listing = git_text(git(source).args(["ls-tree", "-r", commit]))?;
  if let Some(entry) = listing
    .lines()
    .find(|line| line.starts_with("120000 ") || line.starts_with("160000 "))
  {
    return Err(ReleaseFault::UnsupportedSourceEntry(entry.to_owned()));
  }

  checked_git(
    git(source)
      .args(["clone", "--quiet", "--shared", "--no-checkout", "--"])
      .arg(source)
      .arg(snapshot),
  )?;
  checked_git(git(snapshot).args(["checkout", "--quiet", "--detach", commit]))?;
  if snapshot.join("target").try_exists()? {
    return Err(ReleaseFault::TrackedBuildDirectory);
  }
  if !snapshot.join(WORKFLOW).is_file() {
    return Err(ReleaseFault::MissingWorkflow(commit.to_owned()));
  }

  copy_identity(source, snapshot)
}

fn export_source(
  source: &Path,
  export: &Path,
  identifiers: Option<&Path>,
) -> Result<ExitCode, ReleaseFault> {
  let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
  let Ok(mut cargo) = Program::Cargo.command();
  cargo
    .current_dir(source)
    .args(["run", "--quiet", "--locked", "--manifest-path"])
    .arg(manifest)
    .args(["--bin", "console-manifest-publish", "--", "--export-only"])
    .arg(export);
  if let Some(identifiers) = identifiers {
    let identifiers = identifiers.canonicalize()?;
    cargo.arg("--private-identifiers").arg(identifiers);
  }
  let status = cargo.status()?;
  let code = status.code().and_then(|code| u8::try_from(code).ok());

  Ok(code.map(ExitCode::from).unwrap_or(ExitCode::FAILURE))
}

fn tag(release: Release) -> Result<ExitCode, ReleaseFault> {
  let source = console_repository::root().map_err(ReleaseFault::Source)?;
  let checkout = public_checkout(&source, release.repository.as_deref())?;
  let scratch = Scratch::create()?;
  let public = scratch.0.join("public");
  clone_public(&checkout, &public)?;
  require_new_tag(&public, &release.tag)?;

  let commit = match release.revision.as_deref() {
    None => resolve(&public, MAIN)?,
    Some(revision) => public_commit(&public, &checkout, revision)?,
  };
  let workflow = git(&public)
    .args(["cat-file", "-e", &format!("{commit}:{WORKFLOW}")])
    .output()?;
  if !workflow.status.success() {
    eprintln!(
      "Commit {commit} has no {WORKFLOW}. Pushing its tag will not build a GitHub release."
    );
  }
  let reference = create_tag(&public, &release, &commit)?;
  checked_git(git(&public).args(["push", "--no-follow-tags", "origin", &reference]))?;
  println!("Pushed {} on {}.", release.tag, commit);

  Ok(ExitCode::SUCCESS)
}

fn public_commit(public: &Path, checkout: &Path, revision: &str) -> Result<String, ReleaseFault> {
  match resolve(public, revision) {
    Ok(commit) => Ok(commit),
    Err(ReleaseFault::Git(..)) => {
      let commit = resolve(checkout, revision)?;
      checked_git(
        git(public)
          .args(["fetch", "--quiet", "--no-tags", "--"])
          .arg(checkout)
          .arg(&commit),
      )?;

      Ok(commit)
    }
    Err(fault) => Err(fault),
  }
}

fn public_checkout(source: &Path, requested: Option<&Path>) -> Result<PathBuf, ReleaseFault> {
  let default = source.with_file_name("hypr-console");
  let path = requested.unwrap_or(&default);
  let path = path
    .canonicalize()
    .map_err(|fault| ReleaseFault::PublicCheckout(path.to_path_buf(), fault))?;
  let root = git_text(git(&path).args(["rev-parse", "--show-toplevel"]))?;
  let root = PathBuf::from(root);
  let common_dir = ["rev-parse", "--path-format=absolute", "--git-common-dir"];
  if git_text(git(&root).args(common_dir))? == git_text(git(source).args(common_dir))? {
    return Err(ReleaseFault::SourceIsPublicCheckout);
  }

  Ok(root)
}

fn clone_public(checkout: &Path, snapshot: &Path) -> Result<(), ReleaseFault> {
  let fetch = git_text(git(checkout).args(["remote", "get-url", "origin"]))?;
  let push = git_text(git(checkout).args(["remote", "get-url", "--push", "origin"]))?;
  checked_git(
    git(checkout)
      .args([
        "clone",
        "--quiet",
        "--no-checkout",
        "--branch",
        "main",
        "--",
      ])
      .arg(remote_path(checkout, &fetch))
      .arg(snapshot),
  )?;
  checked_git(
    git(snapshot)
      .args(["remote", "set-url", "--push", "origin"])
      .arg(remote_path(checkout, &push)),
  )?;
  copy_identity(checkout, snapshot)
}

fn copy_identity(source: &Path, destination: &Path) -> Result<(), ReleaseFault> {
  for key in ["user.name", "user.email"] {
    let reply = git(source).args(["config", "--get", key]).output()?;
    match reply.status.code() {
      Some(0) => {
        let value = text(reply)?;
        checked_git(git(destination).args(["config", key, &value]))?;
      }
      Some(1) => {}
      _failed => return Err(git_failure(&reply)),
    }
  }

  Ok(())
}

fn remote_path(checkout: &Path, url: &str) -> PathBuf {
  let path = Path::new(url);
  let relative = checkout.join(path);
  match (path.is_relative() && relative.exists()).then_some(()) {
    Some(()) => relative,
    None => path.to_path_buf(),
  }
}

fn require_new_tag(public: &Path, tag: &str) -> Result<(), ReleaseFault> {
  let reference = format!("refs/tags/{tag}");
  let reply = git(public)
    .args(["show-ref", "--exists", &reference])
    .output()?;
  match reply.status.code() {
    Some(2) => Ok(()),
    Some(0) => Err(ReleaseFault::ExistingTag(tag.to_owned())),
    _failed => Err(git_failure(&reply)),
  }
}

fn create_tag(public: &Path, release: &Release, commit: &str) -> Result<String, ReleaseFault> {
  let tag = &release.tag;
  let message = match release.no_latest {
    Presence::Absent => format!("Release {tag}"),
    Presence::Present => format!("Release {tag}\n\nConsole-Latest: false"),
  };
  checked_git(git(public).args(["tag", "--annotate", "-m", &message, tag, commit]))?;

  Ok(format!("refs/tags/{tag}"))
}

fn resolve(repository: &Path, revision: &str) -> Result<String, ReleaseFault> {
  git_text(git(repository).args([
    "rev-parse",
    "--verify",
    "--end-of-options",
    &format!("{revision}^{{commit}}"),
  ]))
}

fn git(repository: &Path) -> Command {
  let Ok(mut git) = Program::Git.command();
  git.current_dir(repository);
  for variable in [
    "GIT_DIR",
    "GIT_COMMON_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  ] {
    git.env_remove(variable);
  }

  git
}

fn git_text(command: &mut Command) -> Result<String, ReleaseFault> {
  let output = checked_git(command)?;
  text(output)
}

fn text(output: Output) -> Result<String, ReleaseFault> {
  let text = String::from_utf8(output.stdout).map_err(ReleaseFault::GitText)?;

  Ok(text.strip_suffix('\n').unwrap_or(&text).to_owned())
}

fn checked_git(command: &mut Command) -> Result<Output, ReleaseFault> {
  let output = command.output()?;
  match output.status.success().then_some(()) {
    Some(()) => Ok(output),
    None => Err(git_failure(&output)),
  }
}

fn git_failure(output: &Output) -> ReleaseFault {
  let details = String::from_utf8(output.stderr.clone())
    .unwrap_or_else(|binary| format!("{:?}", binary.as_bytes()));

  ReleaseFault::Git(output.status, details)
}
