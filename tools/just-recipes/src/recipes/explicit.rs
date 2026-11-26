use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::{self, Installed, Program};

use super::{NIGHTLY, cargo, checked, temporary_dir};

const RULES: &str = "tools/explicit-rust";
const STRUCTURAL: &str = "explicit019_structural_only = true";
const INDEX: &str = "explicit014_no_index_slice";
const INDEX_CONFIG: &str = "explicit014_no_index_slice/decoder-bounds.toml";
const SHADOW_CONFIG: &str = "explicit073_no_shadowing/av1-macros.toml";
const REVIEWED: &[&str] = &[
  "explicit007_no_bool_return/font-type1-outcomes.toml",
  "explicit008_no_bool_param/reviewed/av1-rectangular-scale.toml",
  "explicit008_no_bool_param/reviewed/av1-superblock-size.toml",
  "explicit008_no_bool_param/reviewed/av1-adst-row-order.toml",
  "explicit008_no_bool_param/reviewed/av1-dc-shortcut.toml",
];

pub(super) fn migration(name: &str, flags: &[String]) -> io::Result<()> {
  let (bin, arguments) = match (name, flags) {
    ("explicit-rust", [action, rest @ ..]) if action == "fix-all" => ("batch", rest),
    ("explicit-rust", _) => {
      return Err(io::Error::other(
        "Run just explicit-rust fix-all [OPTIONS].",
      ));
    }
    ("explicit-batch", _) => ("batch", flags),
    ("explicit-repair", _) => ("repair", flags),
    ("explicit-apis", _) => ("apis", flags),
    _invalid => {
      return Err(io::Error::other(format!(
        "unknown migration recipe {name:?}"
      )));
    }
  };
  let mut run = cargo(&[NIGHTLY, "run", "--offline", "--locked", "--manifest-path"]);
  run
    .arg(Path::new(RULES).join("migration-collector/Cargo.toml"))
    .arg("--target-dir")
    .arg(temporary_dir().join("builds/explicit-migration-collector"))
    .args(["--bin", bin, "--"]);

  if name == "explicit-rust" {
    run.arg("fix-all");
    if env::var_os("DYLINT_TOML").is_none() && !Path::new("dylint.toml").try_exists()? {
      run.env("DYLINT_TOML", configuration(&[STRUCTURAL], REVIEWED)?);
    }
  }

  checked(run.args(arguments))
}

pub(super) fn fix(name: &str, flags: &[String]) -> io::Result<()> {
  match name {
    "explicit-fix" => dylint(None, None, &[], flags),
    "explicit-fix-all" => fix_all(None, flags),
    "explicit-fix-reviewed" => {
      let configs = [
        "explicit017_question_mark_alone/av1-first-slice.toml",
        SHADOW_CONFIG,
        "explicit091_closes_on_exec/local-pipes.toml",
        INDEX_CONFIG,
      ];
      let settings = [
        "explicit051_public_compatibility = true",
        "explicit024_typed_coordinates = true",
        "explicit004_test_results = true",
        "explicit004_fixed_array_bounds = true",
        STRUCTURAL,
      ];
      let files: Vec<_> = configs
        .into_iter()
        .chain(REVIEWED.iter().copied())
        .collect();
      fix_all(Some(&configuration(&settings, &files)?), flags)
    }
    "explicit-fix-branches" => {
      let config = env::var_os("DYLINT_TOML").unwrap_or_else(|| STRUCTURAL.into());
      dylint(Some("explicit019_no_if"), Some(&config), &[], flags)
    }
    "explicit-wrap" => dylint(
      Some("explicit015_no_bare_arithmetic"),
      Some("explicit015_wrapping = true".as_ref()),
      &["--all-targets", "-p"],
      flags,
    ),
    "explicit-index-bulk" => with_file(
      INDEX,
      INDEX_CONFIG,
      &[
        "--all-targets",
        "--all-features",
        "-p",
        "console-core-video-decoding-av1",
        "-p",
        "console-core-video-decoding-advanced-video-coding",
        "-p",
        "console-core-video-decoding-high-efficiency-video-coding",
        "-p",
        "console-core-video-decoding-vp9",
        "-p",
        "console-core-paths",
        "-p",
        "console-core-path-rasterizer",
        "-p",
        "console-core-portable-document-syntax",
        "-p",
        "console-core-font-reading",
        "-p",
        "console-core-scalable-vector-graphics-values",
      ],
      flags,
    ),
    "explicit-av1-transform-bounds" => with_file(
      INDEX,
      "explicit014_no_index_slice/av1-transform-bounds.toml",
      &[
        "-p",
        "console-core-video-decoding-av1",
        "--lib",
        "--all-features",
      ],
      flags,
    ),
    "explicit-shadow-av1" => with_file(
      "explicit073_no_shadowing",
      SHADOW_CONFIG,
      &[
        "-p",
        "console-core-video-decoding-av1",
        "--all-targets",
        "--all-features",
      ],
      flags,
    ),
    _invalid => Err(io::Error::other(format!("unknown fix recipe {name:?}"))),
  }
}

fn fix_all(config: Option<&str>, flags: &[String]) -> io::Result<()> {
  dylint(
    None,
    config.map(std::ffi::OsStr::new),
    &["--workspace", "--all-targets", "--all-features"],
    flags,
  )?;
  dylint(
    None,
    config.map(std::ffi::OsStr::new),
    &[
      "-p",
      "console-kernel",
      "--target",
      "x86_64-unknown-uefi",
      "--lib",
      "--bins",
      "--all-features",
    ],
    flags,
  )
}

fn with_file(rule: &str, file: &str, arguments: &[&str], flags: &[String]) -> io::Result<()> {
  let config = fs::read_to_string(Path::new(RULES).join(file))?;
  dylint(Some(rule), Some(config.as_ref()), arguments, flags)
}

fn dylint(
  rule: Option<&str>,
  config: Option<&std::ffi::OsStr>,
  defaults: &[&str],
  flags: &[String],
) -> io::Result<()> {
  let mut run = cargo(&["dylint", "--fix", "--no-deps"]);
  match rule {
    Some(rule) => run.args(["--lib", rule]),
    None => run.arg("--all"),
  };
  if let Some(config) = config {
    run.env("DYLINT_TOML", config);
  }
  let mut rustflags = env::var_os("DYLINT_RUSTFLAGS").unwrap_or_default();
  if !rustflags.is_empty() {
    rustflags.push(" ");
  }
  rustflags.push("--cap-lints=warn");

  checked(
    run
      .arg("--")
      .args(defaults)
      .args(flags)
      .env("DYLINT_RUSTFLAGS", rustflags),
  )
}

fn configuration(settings: &[&str], files: &[&str]) -> io::Result<String> {
  let mut config = String::new();
  for setting in settings {
    config.push_str(setting);
    config.push('\n');
  }
  for file in files {
    let contents = fs::read_to_string(Path::new(RULES).join(file))?;
    config.push_str(&contents);
    config.push('\n');
  }

  Ok(config)
}

pub(super) fn summary(name: &str) -> io::Result<()> {
  let counts = match name {
    "explicit" => count_warnings(explicit_command()?, lint_warning),
    _literals => count_warnings(
      cargo(&[
        "clippy",
        "--quiet",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--message-format=short",
        "--",
        "-W",
        "clippy::default_numeric_fallback",
      ]),
      literal_warning,
    ),
  }?;
  let mut sorted: Vec<_> = counts
    .into_iter()
    .map(|(line, count)| (count, line))
    .collect();
  sorted.sort_unstable_by(|first, second| second.cmp(first));

  for (count, line) in sorted {
    println!("{count:>7} {line}");
  }

  Ok(())
}

fn explicit_command() -> io::Result<Command> {
  let cargo_home = match env::var_os("CARGO_HOME").filter(|path| !path.is_empty()) {
    Some(path) => PathBuf::from(path),
    None => env::home_dir()
      .ok_or_else(|| io::Error::other("cannot determine CARGO_HOME"))?
      .join(".cargo"),
  };
  let bin = cargo_home.join("bin");
  let Ok(rustup) = Program::Rustup.name();
  let Ok(installed) = console_core_external_programs::installed(rustup);
  if installed == Installed::No && !bin.join(rustup).is_file() {
    return Err(io::Error::other("rustup is required to run Explicit Rust"));
  }
  let path = env::var_os("PATH").unwrap_or_default();
  let folders = std::iter::once(bin).chain(env::split_paths(&path));
  let path = env::join_paths(folders).map_err(io::Error::other)?;
  let executable = env::current_exe()?;
  let mut run = Command::new(executable);
  run
    .arg("rules")
    .env("PATH", path)
    .env("RUSTFLAGS", "--cap-lints warn");

  Ok(run)
}

fn count_warnings(
  mut command: Command,
  count: impl Fn(&str) -> Option<String>,
) -> io::Result<BTreeMap<String, u32>> {
  let (reader, writer) = io::pipe()?;
  let stderr = writer.try_clone()?;
  let mut child = command.stdout(writer).stderr(stderr).spawn()?;
  let description = format!("{command:?}");
  drop(command);
  let mut counts = BTreeMap::new();
  let mut errors = Vec::new();

  for line in BufReader::new(reader).lines() {
    let line = line?;
    if let Some(key) = count(&line) {
      let total = counts.entry(key).or_insert(0_u32);
      *total = total.saturating_add(1);
    }
    if errors.len() < 20 && (line.starts_with("error") || line.contains(": error")) {
      errors.push(line);
    }
  }
  let exit = child.wait()?;
  if !exit.success() {
    for error in errors {
      eprintln!("{error}");
    }
    return Err(io::Error::other(format!(
      "{description} exited with {exit}"
    )));
  }

  Ok(counts)
}

fn lint_warning(line: &str) -> Option<String> {
  if !line.starts_with("warning: ") || line.contains("generated") {
    return None;
  }
  let mut normalized = String::new();
  let mut parts = line.split('`');
  normalized.push_str(parts.next()?);
  for (quoted, rest) in std::iter::from_fn(|| Some((parts.next()?, parts.next()))) {
    match rest {
      Some(text) => {
        normalized.push('X');
        normalized.push_str(text);
      }
      None => {
        normalized.push('`');
        normalized.push_str(quoted);
      }
    }
  }

  Some(normalized)
}

fn literal_warning(line: &str) -> Option<String> {
  if !line.contains("default numeric fallback") {
    return None;
  }

  line.split('/').nth(1).map(str::to_owned)
}
