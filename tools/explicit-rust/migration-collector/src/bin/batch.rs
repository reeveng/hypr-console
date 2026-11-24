//! Apply Dylint's captured machine-applicable suggestions in verified crate batches.

#[path = "../batch.rs"]
mod batch;
#[path = "../edits.rs"]
pub mod edits;

use batch::Snapshot;
use edits::Outcome;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{self, BufWriter, ErrorKind, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Args {
    action: String,
    root: PathBuf,
    packages: Vec<String>,
    excluded: Vec<String>,
    rules: Vec<String>,
    selected_rules_only: bool,
    output: Option<PathBuf>,
    capture: Option<PathBuf>,
    baseline: Option<PathBuf>,
    restore: Option<PathBuf>,
    apply: bool,
    no_build: bool,
    size: usize,
    minimum: usize,
    maximum: usize,
    verify: Vec<String>,
}

fn arguments() -> Outcome<Option<Args>> {
    let mut args = env::args().skip(1);
    let mut result = Args {
        action: "audit".into(),
        root: env::current_dir()?,
        packages: vec![],
        excluded: vec![],
        rules: vec![],
        selected_rules_only: false,
        output: None,
        capture: None,
        baseline: None,
        restore: None,
        apply: false,
        no_build: false,
        size: 5000,
        minimum: 1,
        maximum: 100,
        verify: vec!["test".into(), "clippy".into()],
    };
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "audit" | "fix" => result.action = argument,
            "fix-all" => {
                result.action = argument;
                result.apply = true;
                result.verify.clear();
            }
            "--root" => result.root = args.next().ok_or("--root needs a path")?.into(),
            "--package" => result
                .packages
                .push(args.next().ok_or("--package needs a name")?),
            "--exclude" => result
                .excluded
                .push(args.next().ok_or("--exclude needs a package name")?),
            "--rules" => {
                result.rules = args
                    .next()
                    .ok_or("--rules needs comma-separated library names")?
                    .split(',')
                    .map(str::to_owned)
                    .collect()
            }
            "--selected-rules-only" => result.selected_rules_only = true,
            "--output" => result.output = Some(args.next().ok_or("--output needs a path")?.into()),
            "--capture" => {
                result.capture = Some(args.next().ok_or("--capture needs capture.json")?.into())
            }
            "--baseline" => {
                result.baseline = Some(args.next().ok_or("--baseline needs remaining.json")?.into())
            }
            "--restore" => {
                result.restore = Some(
                    args.next()
                        .ok_or("--restore needs a backup directory")?
                        .into(),
                )
            }
            "--batch-size" => {
                result.size = args.next().ok_or("--batch-size needs a number")?.parse()?
            }
            "--min-fixes" => {
                result.minimum = args.next().ok_or("--min-fixes needs a number")?.parse()?
            }
            "--max-batches" => {
                result.maximum = args.next().ok_or("--max-batches needs a number")?.parse()?
            }
            "--verify" => {
                result.verify = args
                    .next()
                    .ok_or("--verify needs test,clippy or none")?
                    .split(',')
                    .filter(|mode| *mode != "none")
                    .map(str::to_owned)
                    .collect()
            }
            "--apply" => result.apply = true,
            "--no-build" => result.no_build = true,
            "--help" | "-h" => {
                println!(
                    "batch audit|fix|fix-all [--apply] [--package NAME] [--exclude NAME] [--rules LIB,...] [--selected-rules-only] [--batch-size 5000] [--min-fixes 1] [--max-batches 100] [--capture capture.json] [--baseline remaining.json] [--output DIR] [--verify test,clippy|none] [--no-build]\nbatch --restore BACKUP\n\nAudit requires zero findings unless --baseline is supplied, in which case it requires zero new findings. Fix previews Dylint's captured machine-applicable suggestions. Fix-all applies them and succeeds once the automatic queue is empty. --selected-rules-only limits the inventory to the libraries named by --rules. Batches start with whole crates and smaller groups are retried after a failure. A batch below --min-fixes suggestion groups is skipped. Every applied batch gets a compiler check and fresh inventory. Tests and strict Clippy follow the final batch for fix --apply. Exit 2 means findings remain within the chosen action's scope. Exit 1 means a failed check, tool, or rejected fix. Backups live under TMPDIR/builds/explicit-batch."
                );
                return Ok(None);
            }
            other => return Err(format!("unknown option: {other}").into()),
        }
    }
    if result.size == 0 || result.maximum == 0 {
        return Err("batch size and maximum must be positive".into());
    }
    if result.minimum == 0 || result.minimum > result.size {
        return Err("--min-fixes must be positive and at most --batch-size".into());
    }
    if result.selected_rules_only && result.rules.is_empty() {
        return Err("--selected-rules-only requires --rules".into());
    }
    if result.baseline.is_some() && result.action != "audit" {
        return Err("--baseline requires audit".into());
    }
    if result
        .verify
        .iter()
        .any(|mode| mode != "test" && mode != "clippy")
    {
        return Err("--verify accepts test,clippy or none".into());
    }
    result.root = result.root.canonicalize()?;
    Ok(Some(result))
}

fn metadata(root: &Path, manifest: Option<&str>) -> Outcome<Value> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["metadata", "--offline", "--locked", "--format-version=1"]);
    if let Some(manifest) = manifest {
        command.args(["--no-deps", "--manifest-path", manifest]);
    } else {
        command.arg("--all-features");
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

#[derive(Clone, Default)]
struct Packages {
    paths: BTreeMap<String, PathBuf>,
    forks: BTreeSet<String>,
}

impl Packages {
    fn names(&self, forks: bool) -> Vec<String> {
        self.paths
            .keys()
            .filter(|name| self.forks.contains(*name) == forks)
            .cloned()
            .collect()
    }
}

fn workspace(root: &Path) -> Outcome<Packages> {
    let metadata = metadata(root, None)?;
    owned_packages(root, &metadata)
}

fn owned_packages(root: &Path, metadata: &Value) -> Outcome<Packages> {
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("metadata has no members")?;
    let mut packages = Packages::default();
    for package in metadata["packages"]
        .as_array()
        .ok_or("metadata has no packages")?
    {
        let member = members.contains(&package["id"]);
        if !member && !package["source"].is_null() {
            continue;
        }
        let directory = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("package has no manifest")?,
        )
        .parent()
        .ok_or("manifest has no parent")?;
        if !member && !directory.starts_with(root.join("forks")) {
            continue;
        }
        let directory = directory.canonicalize()?;
        directory.strip_prefix(root)?;
        let name = package["name"].as_str().ok_or("package has no name")?;
        if packages.paths.insert(name.to_owned(), directory).is_some() {
            return Err(format!("multiple owned packages named {name}").into());
        }
        if !member {
            packages.forks.insert(name.to_owned());
        }
    }
    Ok(packages)
}

fn libraries(root: &Path) -> Outcome<Vec<String>> {
    let metadata = metadata(root, Some("tools/explicit-rust/Cargo.toml"))?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("lint metadata has no members")?;
    let mut names: Vec<_> = metadata["packages"]
        .as_array()
        .ok_or("lint metadata has no packages")?
        .iter()
        .filter(|package| members.contains(&package["id"]))
        .filter_map(|package| {
            package["name"]
                .as_str()
                .filter(|name| name.starts_with("explicit"))
        })
        .map(str::to_owned)
        .collect();
    names.sort();
    if names.is_empty() {
        return Err("no named lint libraries discovered".into());
    }
    Ok(names)
}

fn machine_lock() -> Outcome<Option<fs::File>> {
    // The local cargo-dylint wrapper serializes checks with flock. Acquire that
    // same lock before snapshotting, so time spent in its queue cannot stale
    // the capture. Other installations continue through their ordinary wrapper.
    if env::var_os("CARGO_DYLINT_LOCK_HELD").is_some_and(|held| held == "1") {
        return Ok(None);
    }
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    let Some(home) = cargo_home else {
        return Ok(None);
    };
    let Ok(file) = fs::File::open(home.join("bin/cargo-dylint")) else {
        return Ok(None);
    };
    let mut text = String::new();
    if file.take(8192).read_to_string(&mut text).is_err()
        || !text.starts_with("#!")
        || !text.contains("CARGO_DYLINT_LOCK_HELD")
    {
        return Ok(None);
    }
    let Some(name) = text
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("exec flock ")
                .and_then(|rest| rest.split_whitespace().next())
        })
        .filter(|name| Path::new(name).is_absolute())
    else {
        return Ok(None);
    };
    let file = fs::OpenOptions::new().read(true).write(true).open(name)?;
    let mut update = Instant::now();
    println!("waiting for the shared Dylint lock: {name}");
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(file)),
            Err(fs::TryLockError::WouldBlock) => {
                if update.elapsed() >= Duration::from_secs(30) {
                    println!("waiting for the shared Dylint lock: {name}");
                    update = Instant::now();
                }
                thread::sleep(Duration::from_millis(250));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn run(
    root: &Path,
    directory: &Path,
    label: &str,
    arguments: &[String],
    lint_config: Option<&str>,
    lock_held: bool,
) -> Outcome<PathBuf> {
    fs::write(
        directory.join(format!("{label}.command.json")),
        serde_json::to_vec_pretty(arguments)?,
    )?;
    let output = directory.join(format!("{label}.jsonl"));
    let errors = directory.join(format!("{label}.stderr"));
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(arguments)
        .env("CARGO_NET_OFFLINE", "true")
        .stdout(Stdio::from(fs::File::create(&output)?))
        .stderr(Stdio::from(fs::File::create(&errors)?));
    if let Some(lint_config) = lint_config {
        let mut flags = env::var_os("DYLINT_RUSTFLAGS").unwrap_or_default();
        flags.push(" --cap-lints=warn");
        command
            .env("DYLINT_RUSTFLAGS", flags)
            .env("DYLINT_TOML", lint_config);
    }
    if lock_held {
        command.env("CARGO_DYLINT_LOCK_HELD", "1");
    }
    if label == "fork-rules" {
        // Cargo's workspace wrapper is skipped for selected nonmembers.
        // Keep these compilations apart from Cargo's unlinted fork artifacts.
        command
            .env("RUSTC_WRAPPER", env::current_exe()?)
            .env("EXPLICIT_BATCH_FORK_COMPILER", "1")
            .env("CARGO_TARGET_DIR", root.join("target/explicit-fork-audit"));
    }
    println!(
        "{label}: cargo {}; logs: {}",
        arguments.first().map_or("", String::as_str),
        directory.display()
    );
    let mut process = command.spawn()?;
    let mut update = Instant::now();
    let status = loop {
        if let Some(status) = process.try_wait()? {
            break status;
        }
        if update.elapsed() >= Duration::from_secs(30) {
            println!("{label}: still running; {}", errors.display());
            update = Instant::now();
        }
        thread::sleep(Duration::from_millis(250));
    };
    if !status.success() {
        return Err(format!(
            "{label} failed ({status}); see {} and {}",
            output.display(),
            errors.display()
        )
        .into());
    }
    Ok(output)
}

fn scope(names: &[String]) -> Vec<String> {
    names
        .iter()
        .flat_map(|name| ["-p".into(), name.clone()])
        .collect()
}

fn base(libraries: &[String], no_build: bool) -> Vec<String> {
    let mut args = vec!["dylint".into(), "--no-deps".into()];
    if no_build {
        args.push("--no-build".into());
        if env::var_os("DYLINT_LIBRARY_PATH").is_some_and(|path| !path.is_empty()) {
            args.push("--no-metadata".into());
        }
    }
    for name in libraries {
        args.extend(["--lib".into(), name.clone()]);
    }
    args.extend(["--", "--offline", "--locked", "--message-format=json"].map(str::to_owned));
    args
}

fn files(
    root: &Path,
    packages: &BTreeMap<String, PathBuf>,
    libraries: &[String],
) -> Outcome<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    for directory in packages.values() {
        batch::walk(directory, &mut files)?;
        files.insert(directory.join("Cargo.toml"));
    }
    files.insert(root.join("Cargo.toml"));
    files.insert(root.join("Cargo.lock"));

    let lints = root.join("tools/explicit-rust");
    for name in ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
        files.insert(lints.join(name));
    }
    batch::walk(&lints.join("shared"), &mut files)?;
    for library in libraries {
        let directory = lints.join(library);
        files.insert(directory.join("Cargo.toml"));
        batch::walk(&directory.join("src"), &mut files)?;
    }
    for name in ["dylint.toml", ".cargo/config", ".cargo/config.toml"] {
        let path = root.join(name);
        if path.is_file() {
            files.insert(path);
        }
    }
    Ok(files)
}

fn lint_environment() -> BTreeMap<&'static str, String> {
    [
        "DYLINT_TOML",
        "DYLINT_LIBRARY_PATH",
        "DYLINT_RUSTFLAGS",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
    ]
    .into_iter()
    .filter_map(|name| {
        let value = env::var_os(name)?;
        Some((name, edits::fingerprint(value.as_encoded_bytes())))
    })
    .collect()
}

fn lint_config(root: &Path) -> Outcome<String> {
    if let Ok(config) = env::var("DYLINT_TOML") {
        return Ok(config);
    }
    match fs::read_to_string(root.join("dylint.toml")) {
        Ok(config) => Ok(config),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

struct Capture {
    snapshot: Snapshot,
    host: Vec<Value>,
    firmware: Vec<Value>,
    remaining: Value,
    settings: Value,
}

fn verify_fork_libraries(
    packages: &Packages,
    records: &[Value],
    libraries: &[String],
) -> Outcome<()> {
    let mut missing: BTreeSet<_> = packages
        .names(true)
        .iter()
        .map(|name| packages.paths[name].join("Cargo.toml"))
        .collect();
    let expected: BTreeSet<_> = libraries.iter().map(String::as_str).collect();
    for artifact in records
        .iter()
        .filter(|row| row["reason"] == "compiler-artifact")
    {
        let Some(manifest) = artifact["manifest_path"].as_str().map(Path::new) else {
            continue;
        };
        if !missing.contains(manifest) {
            continue;
        }
        for filename in artifact["filenames"].as_array().into_iter().flatten() {
            let Some(file) = filename.as_str().map(Path::new) else {
                continue;
            };
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let stem = stem.strip_prefix("lib").unwrap_or(stem);
            let Ok(contents) = fs::read_to_string(file.with_file_name(format!("{stem}.d"))) else {
                continue;
            };
            if !contents
                .lines()
                .any(|line| line == "# env-dep:CARGO_PRIMARY_PACKAGE=1")
            {
                continue;
            }
            let Some(loaded) = contents
                .lines()
                .find_map(|line| line.strip_prefix("# env-dep:DYLINT_LIBS="))
            else {
                continue;
            };
            let loaded: Vec<String> = serde_json::from_str(loaded)?;
            let named: BTreeSet<_> = loaded
                .iter()
                .filter_map(|path| {
                    Path::new(path)
                        .file_name()?
                        .to_str()?
                        .strip_prefix("lib")?
                        .split_once('@')
                })
                .map(|(name, _)| name)
                .collect();
            if named == expected {
                missing.remove(manifest);
                break;
            }
        }
    }
    if let Some(manifest) = missing.first() {
        return Err(format!(
            "no verified Dylint library artifact for {}",
            manifest.display()
        )
        .into());
    }
    Ok(())
}

fn read_capture(
    root: &Path,
    snapshot: Snapshot,
    diagnostics: &[PathBuf],
    packages: &Packages,
    settings: Value,
    verify_forks: Option<&[String]>,
) -> Outcome<Capture> {
    let builds = audit_plan(packages);
    if diagnostics.len() != builds.len() {
        return Err("capture is missing diagnostics for a requested target".into());
    }
    let read = |file: &PathBuf| -> Outcome<Vec<Value>> {
        let records = batch::messages(std::slice::from_ref(file))?;
        if !records
            .iter()
            .any(|record| record["reason"] == "build-finished" && record["success"] == true)
        {
            return Err(
                format!("no successful build-finished message in {}", file.display()).into(),
            );
        }
        Ok(records)
    };
    let mut host = Vec::new();
    let mut firmware = Vec::new();
    for (build, file) in builds.iter().zip(diagnostics) {
        if file.file_name().and_then(|name| name.to_str())
            != Some(&format!("{}.jsonl", build.label))
        {
            return Err(format!("capture is missing {} diagnostics", build.label).into());
        }
        let records = read(file)?;
        if build.label == "fork-rules"
            && let Some(libraries) = verify_forks
        {
            verify_fork_libraries(packages, &records, libraries)?;
        }
        let destination = if build.firmware {
            &mut firmware
        } else {
            &mut host
        };
        destination.extend(records);
    }
    batch::verify(root, &snapshot)?;
    let remaining = batch::inventory(root, host.iter().chain(&firmware), &snapshot)?;
    Ok(Capture {
        snapshot,
        host,
        firmware,
        remaining,
        settings,
    })
}

fn lint_settings(root: &Path) -> Outcome<Value> {
    let config = lint_config(root)?;
    let toolchain = fs::read(root.join("tools/explicit-rust/rust-toolchain.toml"))?;
    let flags: BTreeMap<_, _> = [
        "DYLINT_RUSTFLAGS",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_TARGET",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
        "CARGO_TARGET_X86_64_UNKNOWN_UEFI_RUSTFLAGS",
    ]
    .into_iter()
    .map(|name| {
        let value = env::var_os(name).unwrap_or_default();
        (name, edits::fingerprint(value.as_encoded_bytes()))
    })
    .collect();
    let mut cargo_config = BTreeMap::new();
    for path in [".cargo/config", ".cargo/config.toml"] {
        match fs::read(root.join(path)) {
            Ok(contents) => {
                cargo_config.insert(path, edits::fingerprint(&contents));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(json!({
        "config": edits::fingerprint(config.as_bytes()),
        "cargo_config": cargo_config,
        "host": [env::consts::ARCH, env::consts::OS],
        "flags": flags,
        "toolchain": edits::fingerprint(&toolchain),
    }))
}

fn report(directory: &Path, capture: &Capture, libraries: &[String]) -> Outcome<()> {
    let mut annotated = capture.remaining.clone();
    annotated["rules"] = json!(libraries);
    annotated["lint_settings"] = capture.settings.clone();
    fs::write(
        directory.join("remaining.json"),
        serde_json::to_vec_pretty(&annotated)?,
    )?;
    let coverage: Vec<_> = libraries
    .iter()
    .map(|rule| {
      let total = capture.remaining["by_rule"][rule].as_u64().unwrap_or(0);
      let automatic = capture.remaining["automatic_by_rule"][rule].as_u64().unwrap_or(0);
      json!({
          "rule": rule, "findings": total, "automatic": automatic, "review": total - automatic,
      })
    })
    .collect();
    fs::write(
        directory.join("coverage.json"),
        serde_json::to_vec_pretty(&coverage)?,
    )?;
    Ok(())
}

fn report_findings<'a>(
    directory: &Path,
    findings: impl ExactSizeIterator<Item = &'a Value> + Clone,
) -> Outcome<()> {
    const PREVIEW_LIMIT: usize = 12;

    let total = findings.len();
    if total == 0 {
        return Ok(());
    }

    let path = directory.join("findings.txt");
    let mut file = BufWriter::new(fs::File::create(&path)?);
    batch::write_findings(&mut file, findings.clone())?;
    file.flush()?;

    let preview = total.min(PREVIEW_LIMIT);
    let mut terminal = io::stdout().lock();
    batch::write_findings(&mut terminal, findings.take(preview))?;
    writeln!(
        terminal,
        "Showing {preview} of {total} findings. Full guidance: {}",
        path.display(),
    )?;
    Ok(())
}

fn firmware_scope() -> Vec<String> {
    [
        "-p",
        "console-kernel",
        "--target",
        "x86_64-unknown-uefi",
        "--lib",
        "--bins",
    ]
    .map(str::to_owned)
    .to_vec()
}

struct AuditBuild {
    label: &'static str,
    args: Vec<String>,
    firmware: bool,
}

fn audit_plan(packages: &Packages) -> Vec<AuditBuild> {
    let mut builds = Vec::new();
    for (forks, label, targets) in [
        (false, "rules", &["--all-features", "--all-targets"][..]),
        (false, "default-rules", &["--all-targets"][..]),
        (true, "fork-rules", &["--lib"][..]),
    ] {
        let selected = packages.names(forks);
        if selected.is_empty() {
            continue;
        }
        let mut args = scope(&selected);
        args.extend(targets.iter().map(|target| (*target).to_owned()));
        builds.push(AuditBuild {
            label,
            args,
            firmware: false,
        });
    }
    if packages.paths.contains_key("console-kernel") {
        for (label, features) in [
            ("firmware", &["--all-features"][..]),
            ("default-firmware", &[][..]),
        ] {
            let mut args = firmware_scope();
            args.extend(features.iter().map(|feature| (*feature).to_owned()));
            builds.push(AuditBuild {
                label,
                args,
                firmware: true,
            });
        }
    }
    builds
}

fn capture(
    root: &Path,
    packages: &Packages,
    libraries: &[String],
    no_build: bool,
    directory: &Path,
    lock_held: bool,
) -> Outcome<Capture> {
    let files = files(root, &packages.paths, libraries)?;
    let snapshot = batch::snapshot(root, &files)?;
    // Dylint otherwise runs cargo metadata in each rustc process to find this file.
    let config = lint_config(root)?;
    let settings = lint_settings(root)?;
    let names: Vec<_> = packages.paths.keys().cloned().collect();
    let mut diagnostics = Vec::new();
    for build in audit_plan(packages) {
        if build.firmware {
            batch::verify(root, &snapshot)?;
        }
        let mut args = base(libraries, no_build);
        args.extend(build.args);
        diagnostics.push(run(
            root,
            directory,
            build.label,
            &args,
            Some(&config),
            lock_held,
        )?);
    }
    if lint_settings(root)? != settings || lint_config(root)? != config {
        return Err("lint settings changed during capture".into());
    }
    let capture = read_capture(
        root,
        snapshot,
        &diagnostics,
        packages,
        settings,
        Some(libraries),
    )?;
    fs::write(
        directory.join("snapshot.json"),
        serde_json::to_vec_pretty(&capture.snapshot)?,
    )?;
    report(directory, &capture, libraries)?;
    fs::write(
        directory.join("capture.json"),
        serde_json::to_vec_pretty(&json!({
            "root": root, "packages": names, "libraries": libraries, "complete": true,
            "diagnostics": diagnostics, "snapshot": directory.join("snapshot.json"),
            "lint_environment": lint_environment(),
            "lint_settings": capture.settings,
            "fork_libraries_verified": true,
        }))?,
    )?;
    println!(
        "{} unique findings; {} named libraries",
        capture.remaining["total"],
        libraries.len()
    );
    Ok(capture)
}

fn check(root: &Path, directory: &Path, packages: &Packages, label: &str) -> Outcome<()> {
    for (forks, build_label, targets) in [
        (
            false,
            label.to_owned(),
            &["--all-features", "--all-targets"][..],
        ),
        (true, format!("{label}-forks"), &["--lib"][..]),
    ] {
        let names = packages.names(forks);
        if names.is_empty() {
            continue;
        }
        let mut args = ["check", "--offline", "--locked", "--message-format=json"]
            .map(str::to_owned)
            .to_vec();
        args.extend(scope(&names));
        args.extend(targets.iter().map(|target| (*target).to_owned()));
        run(root, directory, &build_label, &args, None, false)?;
    }
    if packages.paths.contains_key("console-kernel") {
        let mut args = ["check", "--offline", "--locked", "--message-format=json"]
            .map(str::to_owned)
            .to_vec();
        args.extend(firmware_scope());
        args.push("--all-features".into());
        run(
            root,
            directory,
            &format!("{label}-firmware"),
            &args,
            None,
            false,
        )?;
    }
    Ok(())
}

fn main_run() -> Outcome<i32> {
    let Some(args) = arguments()? else {
        return Ok(0);
    };
    if let Some(directory) = &args.restore {
        batch::restore(directory)?;
        return Ok(0);
    }
    let mut all_packages = workspace(&args.root)?;
    for name in &args.excluded {
        if !all_packages.paths.contains_key(name) {
            return Err(format!("unknown excluded package: {name}").into());
        }
    }
    all_packages
        .paths
        .retain(|name, _| !args.excluded.contains(name));
    let mut selected = all_packages.clone();
    if !args.packages.is_empty() {
        for name in &args.packages {
            if !all_packages.paths.contains_key(name) {
                return Err(format!("unknown package: {name}").into());
            }
        }
        selected
            .paths
            .retain(|name, _| args.packages.contains(name));
    }
    if selected.paths.is_empty() {
        return Err("no packages selected".into());
    }
    let mut libraries = libraries(&args.root)?;
    for rule in &args.rules {
        if !libraries.contains(rule) {
            return Err(format!("unknown lint library: {rule}").into());
        }
    }
    if args.selected_rules_only {
        libraries.retain(|rule| args.rules.contains(rule));
    }
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| args.root.join("target/explicit-batch"));
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let directory = output.join(format!("run-{nonce}-{}", std::process::id()));
    fs::create_dir_all(&directory)?;
    println!("run artifacts: {}", directory.display());
    let names: Vec<_> = selected.paths.keys().cloned().collect();
    let fixing = args.action != "audit";
    if fixing && args.apply {
        check(&args.root, &directory, &selected, "baseline")?;
    }
    let lock = if args.capture.is_some() && !args.apply {
        None
    } else {
        machine_lock()?
    };
    let lock_held = lock.is_some();
    let mut current = if let Some(saved) = &args.capture {
        let saved: Value = serde_json::from_slice(&fs::read(saved)?)?;
        if saved["complete"] != true
            || saved["root"] != json!(args.root)
            || saved["packages"] != json!(names)
            || saved["libraries"] != json!(libraries)
        {
            return Err("capture scope, root, library list or completeness does not match".into());
        }
        let Some(saved_environment) = saved.get("lint_environment") else {
            return Err("capture predates lint input tracking; recapture before reuse".into());
        };
        if saved_environment != &json!(lint_environment()) {
            return Err(
                "capture lint configuration or flags changed; recapture before fixing".into(),
            );
        }
        let settings = lint_settings(&args.root)?;
        if saved["lint_settings"] != settings {
            return Err("capture settings changed; recapture before reuse".into());
        }
        if !selected.names(true).is_empty() && saved["fork_libraries_verified"] != true {
            return Err("capture lacks fork coverage verification; recapture before reuse".into());
        }
        let snapshot: Snapshot = serde_json::from_slice(&fs::read(
            saved["snapshot"].as_str().ok_or("missing snapshot path")?,
        )?)?;
        let current_files = files(&args.root, &selected.paths, &libraries)?;
        if current_files.len() != snapshot.len()
            || current_files.iter().any(|path| {
                path.strip_prefix(&args.root).ok().is_none_or(|relative| {
                    !snapshot.contains_key(relative.to_string_lossy().as_ref())
                })
            })
        {
            return Err("capture source set changed; recapture before fixing".into());
        }
        let paths: Vec<PathBuf> = serde_json::from_value(saved["diagnostics"].clone())?;
        read_capture(&args.root, snapshot, &paths, &selected, settings, None)?
    } else {
        capture(
            &args.root,
            &selected,
            &libraries,
            args.no_build,
            &directory,
            lock_held,
        )?
    };
    let before = current.remaining["total"].clone();
    let mut history = vec![];
    let mut rejected = BTreeSet::new();
    let mut pending = Vec::new();
    let mut budget = args.size;
    let mut number = 0;
    let mut automatic_done = false;
    while fixing && history.len() <= args.maximum {
        let host =
            batch::automatic_packages(&args.root, &current.host, &selected.paths, &args.rules);
        let firmware =
            batch::automatic_packages(&args.root, &current.firmware, &selected.paths, &args.rules);
        let mut costs = host.clone();
        for (package, cost) in &firmware {
            *costs.entry(package.clone()).or_default() += cost;
        }
        automatic_done = costs.is_empty();
        let eligible = costs
            .iter()
            .filter(|(name, _)| !rejected.contains(*name))
            .map(|(name, cost)| (name.clone(), *cost))
            .collect();
        let chosen = batch::choose_batch(&eligible, budget);
        let planned: usize = chosen.iter().map(|name| costs[name]).sum();
        let host_packages: Vec<_> = chosen
            .iter()
            .filter(|name| host.contains_key(*name))
            .cloned()
            .collect();
        let fix_firmware = chosen.iter().any(|name| firmware.contains_key(name));
        fs::write(
            directory.join("automatic.json"),
            serde_json::to_vec_pretty(&json!({
                "by_package": costs, "next_batch": chosen,
                "host": host_packages, "firmware": fix_firmware,
                "suggestion_groups": planned, "minimum": args.minimum, "skipped": rejected,
                "excluded": args.excluded,
            }))?,
        )?;
        if planned < args.minimum {
            println!(
                "Stopping with {planned} suggestion groups available in a batch (minimum {}).",
                args.minimum
            );
            break;
        }
        if !args.apply || history.len() == args.maximum {
            break;
        }
        number += 1;
        batch::verify(&args.root, &current.snapshot)?;
        let step = directory.join(format!("batch-{number:04}"));
        fs::create_dir(&step)?;
        let temporary =
            env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let backup = temporary
            .join("builds/explicit-batch")
            .join(format!("{nonce}-{number:04}"));
        let chosen_packages: BTreeMap<_, _> = chosen
            .iter()
            .map(|name| (name.clone(), selected.paths[name].clone()))
            .collect();
        let fixes = batch::plan_suggestions(
            &args.root,
            current.host.iter().chain(&current.firmware),
            &chosen_packages,
            &args.rules,
        );
        let window = pending.pop().unwrap_or(0..fixes.len());
        let chosen_snapshot = fixes.snapshot(&args.root, &current.snapshot, window.clone())?;
        let (applied, changed) = fixes.apply(
            &args.root,
            &chosen_snapshot,
            window.clone(),
            args.minimum,
            &backup,
        )?;
        fs::write(
            step.join("backup.json"),
            serde_json::to_vec_pretty(&json!({"backup": backup, "changed": changed}))?,
        )?;
        let attempt = if changed.is_empty() {
            Err("the suggestions produced no edits above the minimum".into())
        } else {
            println!(
                "Applied {applied} suggestion groups across {} files.",
                changed.len()
            );
            check(&args.root, &step, &all_packages, "check").and_then(|()| {
                capture(
                    &args.root,
                    &selected,
                    &libraries,
                    args.no_build,
                    &step,
                    lock_held,
                )
            })
        };
        let previous_total = current.remaining["total"]
            .as_u64()
            .ok_or("no current finding count")?;
        let verified = attempt.and_then(|next| {
            let total = next.remaining["total"]
                .as_u64()
                .ok_or("no verified finding count")?;
            if total >= previous_total {
                return Err(format!("findings went from {previous_total} to {total}").into());
            }
            Ok((next, total))
        });
        let (next, next_total) = match verified {
            Ok(next) => next,
            Err(error) => {
                if !changed.is_empty() {
                    batch::restore(&backup).map_err(|restore| {
                        format!("{error}; could not restore {}: {restore}", backup.display())
                    })?;
                }
                eprintln!(
                    "Rejected batch {number} after {error}. Logs in {}",
                    step.display()
                );
                if chosen.len() > 1 {
                    budget = planned / 2;
                } else if window.len() / 2 >= args.minimum {
                    let midpoint = window.start + window.len() / 2;
                    pending.push(midpoint..window.end);
                    pending.push(window.start..midpoint);
                } else if pending.is_empty() {
                    rejected.extend(chosen);
                    budget = args.size;
                }
                continue;
            }
        };
        current = next;
        rejected.clear();
        pending.clear();
        budget = args.size;
        history.push(json!({
            "batch": number, "packages": chosen, "files": changed.len(), "backup": backup,
            "before": previous_total, "after": next_total, "suggestion_groups": applied,
        }));
        report(&directory, &current, &libraries)?;
        fs::write(
            directory.join("progress.json"),
            serde_json::to_vec_pretty(&history)?,
        )?;
    }
    report(&directory, &current, &libraries)?;
    drop(lock);
    if !history.is_empty() {
        for mode in &args.verify {
            for forks in [false, true] {
                let names = selected.names(forks);
                if names.is_empty() {
                    continue;
                }
                let mut command = vec![mode.clone(), "--offline".into(), "--locked".into()];
                command.extend(scope(&names));
                if forks {
                    command.push("--lib".into());
                } else {
                    command.push("--all-features".into());
                    if mode == "test" {
                        command.extend(["--lib", "--tests"].map(str::to_owned));
                    } else {
                        command.push("--all-targets".into());
                    }
                }
                if mode == "clippy" {
                    command.extend(["--", "-D", "warnings"].map(str::to_owned));
                }
                let label = if forks {
                    format!("final-{mode}-forks")
                } else {
                    format!("final-{mode}")
                };
                run(&args.root, &directory, &label, &command, None, false)?;
            }
        }
    }
    println!(
        "{before} before; {} remaining; {} applied batches. Report: {}",
        current.remaining["total"],
        history.len(),
        directory.join("remaining.json").display()
    );
    if let Some(path) = args.baseline {
        let baseline: Value = serde_json::from_slice(&fs::read(path)?)?;
        if baseline["rules"] != json!(libraries) || baseline["lint_settings"] != current.settings {
            return Err("baseline rules or settings do not match this audit".into());
        }
        let new = batch::regressions(&baseline, &current.remaining)?;
        let report = directory.join("regressions.json");
        fs::write(&report, serde_json::to_vec_pretty(&new)?)?;
        println!(
            "{} new findings against the baseline. Report: {}",
            new.len(),
            report.display()
        );
        report_findings(&directory, new.iter().copied())?;
        return Ok(if new.is_empty() { 0 } else { 2 });
    }
    let findings = current.remaining["findings"].as_array().ok_or("missing findings")?;
    report_findings(&directory, findings.iter())?;
    Ok(if !rejected.is_empty() {
        1
    } else if current.remaining["total"].as_u64() == Some(0)
        || (args.action == "fix-all" && automatic_done)
    {
        0
    } else {
        2
    })
}

fn fork_compiler() -> Outcome<i32> {
    let mut args = env::args_os().skip(1);
    let compiler = args.next().ok_or("Cargo did not supply a compiler")?;
    let mut command = match env::var_os("RUSTC_WORKSPACE_WRAPPER") {
        Some(driver)
            if driver != compiler
                && env::var_os("CARGO_PRIMARY_PACKAGE").is_some()
                && env::var_os("DYLINT_LIBS").is_some() =>
        {
            let mut command = Command::new(driver);
            command.arg(compiler);
            command
        }
        _ => Command::new(compiler),
    };
    Ok(command.args(args).status()?.code().unwrap_or(1))
}

fn main() {
    let result = if env::var_os("EXPLICIT_BATCH_FORK_COMPILER").is_some() {
        fork_compiler()
    } else {
        main_run()
    };
    match result {
        Ok(status) => std::process::exit(status),
        Err(error) => {
            eprintln!("batch: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn inventory_selects_workspace_members_and_resolved_local_forks() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap();
        let packages = owned_packages(
            &root,
            &json!({
                "workspace_members": ["kernel", "member"],
                "packages": [
                    {"id": "kernel", "name": "console-kernel", "source": null,
                      "manifest_path": root.join("crates/console-kernel/Cargo.toml")},
                    {"id": "member", "name": "console-onscreen", "source": null,
                      "manifest_path": root.join("crates/console-onscreen/Cargo.toml")},
                    {"id": "fork", "name": "console-core-bitmap-font", "source": null,
                      "manifest_path": root.join("forks/console-core-bitmap-font/Cargo.toml")},
                    {"id": "tool", "name": "text_adapt", "source": null,
                      "manifest_path": root.join("tools/explicit-rust/migration-collector/Cargo.toml")},
                    {"id": "registry", "name": "sample", "source": "registry+https://example.org"}
                ],
            }),
        )
        .unwrap();
        assert_eq!(
            packages.names(false),
            ["console-kernel", "console-onscreen"]
        );
        assert_eq!(packages.names(true), ["console-core-bitmap-font"]);
        let builds = audit_plan(&packages);
        assert_eq!(
            builds.iter().map(|build| build.label).collect::<Vec<_>>(),
            [
                "rules",
                "default-rules",
                "fork-rules",
                "firmware",
                "default-firmware"
            ],
        );
        assert_eq!(builds[2].args, ["-p", "console-core-bitmap-font", "--lib"]);
        for (build, all_features) in builds.iter().zip([true, false, false, true, false]) {
            assert_eq!(
                build.args.iter().any(|arg| arg == "--all-features"),
                all_features
            );
        }
    }
}
