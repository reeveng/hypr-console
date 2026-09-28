//! `cargo x`: the tasks, run.
//!
//! Everything here is an edge -- git asked, `cargo metadata` read, a file
//! read to see whether it walks out of its crate, a step started and waited
//! for. What any of it means is decided in the library. Every step runs in a
//! control group of its own, the way `just alone` ran it, so what a run starts
//! at whatever depth is stopped with it rather than found weeks later.

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use console_core_atomic_writes::Stored;
use console_core_external_programs::{Installed, Program, installed};
use console_core_never::Never;
use console_program_lifetime::{Scopes, alongside, nothing_left_in, scopes};
use console_tasks::reached::{self, CRATES, Dependency, Kind, Package, Scope};
use console_tasks::ready::{self, Earlier, Record, Same, Step, Tree};
use console_tasks::task::{self, Task};

const MOST: &str = "MemoryHigh=32G";

const THREADS: &str = "TasksMax=8192";

const ARCHITECTURE: &str = "target/architecture";

const FACTS: &str = "target/architecture/facts";

const CHECKED: &str = "target/architecture/dylint/target";

const MAP_FACTS: &str = "docs/architecture/facts.jsonl";

const MAP_SOURCE: &str = "docs/architecture/map.dot";

const MAP_PICTURE: &str = "docs/architecture/map.svg";

const PASSED: &str = "console-ready";

const KERNEL: &str = "console-kernel";

const FIRMWARE: &str = "x86_64-unknown-uefi";

const WALKS_OUT: &str = "../..";

enum TaskError {
    Unknown(String),
    Unstarted(String, io::Error),
    Failed(String),
    Unread(String),
    Metadata(String),
    NoRoot(String),
    Unwritten(String),
    MapChanged,
    NoToolchain,
}

impl fmt::Display for TaskError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::Unknown(word) => write!(to, "no task is called {word}; `cargo x help` lists them"),
            TaskError::Unstarted(what, why) => write!(to, "could not start {what}: {why}"),
            TaskError::Failed(what) => write!(to, "{what} did not pass"),
            TaskError::Unread(what) => write!(to, "could not read what {what} answered"),
            TaskError::Metadata(why) => write!(to, "cargo metadata said something this cannot read: {why}"),
            TaskError::NoRoot(why) => write!(to, "not inside this tree: {why}"),
            TaskError::Unwritten(why) => write!(to, "the pass could not be written down: {why}"),
            TaskError::MapChanged => write!(to, "the map drawn now is not the one committed; commit docs/architecture"),
            TaskError::NoToolchain => write!(to, "rustup is not on PATH, and the lint suite asks it which toolchain it is"),
        }
    }
}

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let ran = run(&asked);

    match ran {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("cargo x: {fault}");

            ExitCode::FAILURE
        }
    }
}

fn run(asked: &[String]) -> Result<(), TaskError> {
    let (word, rest) = match asked.split_first() {
        Some((word, rest)) => (word.as_str(), rest),
        None => ("help", asked),
    };
    let Ok(named) = Task::from_word(word);
    let found = named.ok_or_else(|| TaskError::Unknown(word.to_string()))?;
    let root = console_repository::root().map_err(|why| TaskError::NoRoot(why.to_string()))?;

    match found {
        Task::Help => {
            let Ok(said) = task::help();

            println!("{said}");

            Ok(())
        }
        Task::Test => test(),
        Task::Ready => ready(&root),
        Task::Reached => {
            let since = rest.first().cloned();
            let scope = reached_since(&root, since)?;
            let Ok(flags) = scope.flags();

            println!("{}", flags.join(" "));

            Ok(())
        }
        Task::Map => map(&root),
        Task::Rules => match rest.is_empty() {
            true => {
                let Ok(flags) = Scope::Workspace.flags();

                rules(&flags)
            }
            false => rules(rest),
        },
        Task::Alone => alone(rest, &[]),
    }
}

fn alone(words: &[String], environment: &[(&str, &str)]) -> Result<(), TaskError> {
    let unit = format!("console-run-{}", std::process::id());
    let Ok(available) = scopes();
    let wrapped: Vec<String> = match available {
        Scopes::Available => {
            let Ok(said) = Program::SystemdRun.words(
                ["--user", "--scope", "--quiet", &format!("--unit={unit}"), "-p", MOST, "-p", THREADS, "--"]
                    .into_iter()
                    .map(str::to_string)
                    .chain(words.iter().cloned())
                    .collect(),
            );

            said
        }
        Scopes::Unavailable => {
            eprintln!("no user manager here, so this run is not in a group of its own");

            words.to_vec()
        }
    };
    let ran = started(&wrapped, environment);
    let Ok(()) = nothing_left_in(&unit);

    ran
}

fn started(words: &[String], environment: &[(&str, &str)]) -> Result<(), TaskError> {
    let said = words.join(" ");
    let (program, rest) = words.split_first().ok_or_else(|| TaskError::Failed(said.clone()))?;
    let mut command = Command::new(program);

    command.args(rest).envs(environment.iter().copied());

    let began = alongside(&mut command);
    let mut running = began.map_err(|why| TaskError::Unstarted(said.clone(), why))?;
    let waited = running.wait();
    let status = waited.map_err(|why| TaskError::Unstarted(said.clone(), why))?;

    match status.success() {
        true => Ok(()),
        false => Err(TaskError::Failed(said)),
    }
}

fn cargo(arguments: &[&str]) -> Result<(), TaskError> {
    let Ok(words) = Program::Cargo.words(arguments.iter().map(|word| (*word).to_string()).collect());

    alone(&words, &[])
}

fn cargo_over(arguments: &[&str], scope: &Scope, after: &[&str]) -> Result<(), TaskError> {
    let Ok(flags) = scope.flags();
    let Ok(words) = Program::Cargo.words(
        arguments
            .iter()
            .map(|word| (*word).to_string())
            .chain(flags)
            .chain(after.iter().map(|word| (*word).to_string()))
            .collect(),
    );

    alone(&words, &[])
}

fn answer(program: Program, arguments: &[&str]) -> Result<String, TaskError> {
    let Ok(mut asking) = program.command();
    let Ok(name) = program.name();
    let said = format!("{name} {}", arguments.join(" "));
    let output = asking.args(arguments).output().map_err(|why| TaskError::Unstarted(said.clone(), why))?;

    match output.status.success() {
        true => String::from_utf8(output.stdout).map_err(|_not_text| TaskError::Unread(said)),
        false => Err(TaskError::Failed(said)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reply {
    Yes,
    No,
}

fn asked(program: Program, arguments: &[&str]) -> Result<Reply, TaskError> {
    let Ok(mut asking) = program.command();
    let status = asking
        .args(arguments)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|why| TaskError::Unstarted(arguments.join(" "), why))?;

    Ok(match status.success() {
        true => Reply::Yes,
        false => Reply::No,
    })
}

fn test() -> Result<(), TaskError> {
    cargo(&["build", "--quiet", "--workspace", "--all-features"])?;
    cargo_over(&["nextest", "run", "--cargo-quiet"], &Scope::Workspace, &NEXTEST)
}

const NEXTEST: [&str; 6] = ["--all-features", "--no-tests=warn", "--status-level", "fail", "--final-status-level", "fail"];

fn tree_state() -> Result<(String, Tree), TaskError> {
    let now = answer(Program::Git, &["rev-parse", "HEAD^{tree}"])?;
    let loose = answer(Program::Git, &["status", "--porcelain"])?;
    let tree = match loose.trim().is_empty() {
        true => Tree::Committed,
        false => Tree::Loose,
    };

    Ok((now.trim().to_string(), tree))
}

fn passed_at() -> Result<PathBuf, TaskError> {
    let common = answer(Program::Git, &["rev-parse", "--git-common-dir"])?;

    Ok(Path::new(common.trim()).join(PASSED))
}

fn ready(root: &Path) -> Result<(), TaskError> {
    let (now, before) = tree_state()?;
    let at = passed_at()?;
    let Ok(held) = console_core_atomic_writes::read(&at);
    let recorded = match held {
        Stored::Text(said) => Some(said),
        Stored::Absent => None,
        Stored::Failed(why) => return Err(TaskError::Unread(format!("{}: {why}", at.display()))),
    };
    let Ok(earlier) = ready::earlier(before, &now, recorded.as_deref());
    let since = match earlier {
        Earlier::Passed => {
            println!("ready already passed on this tree ({now}), nothing to ask again");

            return Ok(());
        }
        Earlier::Ask { since } => since,
    };
    let scope = reached_since(root, since)?;
    let Ok(flags) = scope.flags();

    println!("the tests, clippy and the rules are asked of: {}", flags.join(" "));

    let Ok(steps) = ready::steps(&scope);

    for step in steps {
        ask(root, &step)?;
    }

    written_down(&at, &now, before)
}

fn written_down(at: &Path, then: &str, before: Tree) -> Result<(), TaskError> {
    let (now, after) = tree_state()?;
    let same = match now == then {
        true => Same::Yes,
        false => Same::No,
    };
    let Ok(record) = ready::record(before, after, same);

    match record {
        Record::Write => console_core_atomic_writes::whole(at, format!("{now}\n").as_bytes())
            .map_err(|why| TaskError::Unwritten(why.to_string())),
        Record::Leave => {
            println!("ready passed, but the tree was not committed and still, so the pass is not written down");

            Ok(())
        }
    }
}

fn ask(root: &Path, step: &Step) -> Result<(), TaskError> {
    match step {
        Step::Map => map(root),
        Step::MapUnchanged => {
            let same = asked(Program::Git, &["diff", "--quiet", "--", MAP_FACTS, MAP_SOURCE])?;

            match same {
                Reply::Yes => Ok(()),
                Reply::No => Err(TaskError::MapChanged),
            }
        }
        Step::RetiredWords => cargo(&["run", "--quiet", "--release", "--manifest-path", "tools/rename-words/Cargo.toml", "--", "check"]),
        Step::WordsInProportion => cargo(&["test", "--quiet", "--locked", "-p", "console-vocabulary", "--test", "the_words"]),
        Step::Build => cargo(&["build", "--quiet", "--locked", "--workspace", "--all-features"]),
        Step::Tests(scope) => cargo_over(&["nextest", "run", "--cargo-quiet", "--locked"], scope, &NEXTEST),
        Step::Clippy(scope) => cargo_over(&["clippy", "--quiet", "--locked"], scope, &["--all-targets", "--all-features", "--", "-D", "warnings"]),
        Step::Checks => cargo(&["run", "--quiet", "--bin", "console-check"]),
        Step::Rules(scope) => {
            let Ok(flags) = scope.flags();

            rules(&flags)
        }
    }
}

fn toolchain() -> Result<(), TaskError> {
    let Ok(name) = Program::Rustup.name();
    let Ok(found) = installed(name);

    match found {
        Installed::Yes => Ok(()),
        Installed::No => Err(TaskError::NoToolchain),
    }
}

fn rules(flags: &[String]) -> Result<(), TaskError> {
    toolchain()?;

    let Ok(words) = Program::Cargo.words(
        ["dylint", "--all", "--", "--locked", "--all-targets", "--all-features"]
            .into_iter()
            .map(str::to_string)
            .chain(flags.iter().cloned())
            .collect(),
    );

    alone(&words, &[])?;

    let firmware = flags.iter().find(|flag| flag.as_str() == "--workspace" || flag.as_str() == KERNEL);

    match firmware {
        Some(_asked) => cargo(&["dylint", "--all", "--", "--locked", "-p", KERNEL, "--target", FIRMWARE]),
        None => Ok(()),
    }
}

fn map(root: &Path) -> Result<(), TaskError> {
    toolchain()?;

    match std::fs::remove_dir_all(FACTS) {
        Ok(()) => {}
        Err(why) => match why.kind() == io::ErrorKind::NotFound {
            true => {}
            false => return Err(TaskError::Unstarted(format!("emptying {FACTS}"), why)),
        },
    }

    let checked: Vec<PathBuf> = match std::fs::read_dir(CHECKED) {
        Ok(entries) => entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir()).collect(),
        Err(_absent) => Vec::new(),
    };

    for target in checked {
        let at = target.display().to_string();

        cargo(&["clean", "--quiet", "--workspace", "--target-dir", &at])?;
    }

    let facts = root.join(FACTS).display().to_string();
    let Ok(dylint) = Program::Cargo.words(
        ["dylint", "--quiet", "--path", "tools/explicit-rust", "--pattern", "architecture_facts", "--", "--quiet", "--locked", "--workspace", "--lib", "--bins", "--all-features"]
            .into_iter()
            .map(str::to_string)
            .collect(),
    );

    alone(&dylint, &[("CARGO_TARGET_DIR", ARCHITECTURE), ("CONSOLE_ARCHITECTURE_FACTS", &facts)])?;
    cargo(&["run", "--quiet", "--locked", "--bin", "console-architecture", "--", &facts])?;

    let drawn = asked(Program::Dot, &["-Tsvg", MAP_SOURCE, "-o", MAP_PICTURE])?;

    match drawn {
        Reply::Yes => Ok(()),
        Reply::No => {
            eprintln!("map: dot would not draw, so {MAP_PICTURE} is as it was");

            Ok(())
        }
    }
}

fn reached_since(root: &Path, since: Option<String>) -> Result<Scope, TaskError> {
    let since = match since {
        Some(since) => since,
        None => return Ok(Scope::Workspace),
    };
    let held = asked(Program::Git, &["cat-file", "-e", &format!("{since}^{{tree}}")])?;

    match held {
        Reply::Yes => {}
        Reply::No => return Ok(Scope::Workspace),
    }

    let (now, _tree) = tree_state()?;
    let between = answer(Program::Git, &["diff", "--name-only", &since, &now])?;
    let uncommitted = answer(Program::Git, &["diff", "--name-only", "HEAD"])?;
    let untracked = answer(Program::Git, &["ls-files", "--others", "--exclude-standard"])?;
    let changed: Vec<String> = between.lines().chain(uncommitted.lines()).chain(untracked.lines()).map(str::to_string).collect();
    let packages = metadata()?;
    let Ok(walking) = walking_out(root, &packages);
    let Ok(readers) = reached::readers(&packages, &walking);
    let Ok(scope) = reached::reached(&changed, &packages, &readers);

    Ok(scope)
}

fn walking_out(root: &Path, packages: &[Package]) -> Result<BTreeSet<String>, Never> {
    Ok(packages
        .iter()
        .flat_map(|package| {
            let folder = root.join(CRATES).join(&package.folder);
            let Ok(sources) = console_core_directory_listing::files(&folder.join("src"));
            let Ok(tests) = console_core_directory_listing::files(&folder.join("tests"));

            sources
                .into_iter()
                .chain(tests)
                .chain(std::iter::once(folder.join("build.rs")))
                .map(|file| (package.folder.clone(), file))
                .collect::<Vec<(String, PathBuf)>>()
        })
        .filter(|(_folder, file)| {
            let Ok(held) = console_core_atomic_writes::read(file);

            match held {
                Stored::Text(said) => said.contains(WALKS_OUT),
                Stored::Absent => false,
                Stored::Failed(_unread) => false,
            }
        })
        .map(|(folder, _file)| folder)
        .collect())
}

fn metadata() -> Result<Vec<Package>, TaskError> {
    let said = answer(Program::Cargo, &["metadata", "--format-version", "1", "--no-deps", "--offline"])?;
    let read: serde_json::Value = serde_json::from_str(&said).map_err(|why| TaskError::Metadata(why.to_string()))?;
    let packages = read.get("packages").and_then(serde_json::Value::as_array).ok_or_else(|| TaskError::Metadata("no packages".to_string()))?;

    Ok(packages
        .iter()
        .filter_map(|read| {
            let Ok(found) = package(read);

            found
        })
        .collect())
}

fn text<'a>(read: &'a serde_json::Value, key: &str) -> Result<Option<&'a str>, Never> {
    Ok(read.get(key).and_then(serde_json::Value::as_str))
}

fn package(read: &serde_json::Value) -> Result<Option<Package>, Never> {
    let Ok(name) = text(read, "name");
    let Ok(manifest) = text(read, "manifest_path");
    let folder = manifest.map(Path::new).and_then(Path::parent).and_then(Path::file_name).and_then(std::ffi::OsStr::to_str);
    let listed = read.get("dependencies").and_then(serde_json::Value::as_array);

    Ok(name.zip(folder).zip(listed).map(|((name, folder), listed)| Package {
        name: name.to_string(),
        folder: folder.to_string(),
        dependencies: listed
            .iter()
            .filter(|needed| needed.get("path").is_some_and(|path| !path.is_null()))
            .filter_map(|needed| {
                let Ok(name) = text(needed, "name");
                let Ok(kind) = text(needed, "kind");
                let kind = match kind {
                    Some("dev") => Kind::Development,
                    Some(_other) => Kind::Normal,
                    None => Kind::Normal,
                };

                name.map(|name| Dependency { name: name.to_string(), kind })
            })
            .collect(),
    }))
}
