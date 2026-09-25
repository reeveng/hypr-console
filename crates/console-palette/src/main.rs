//! The palette, written into every file that spends a color.
//!
//!     console-palette          write the palette out of theme/palette.toml
//!     console-palette --check  say what it would change, change nothing
//!
//! `theme/palette.toml` is the one place a color is decided. Everything on
//! the machine reads from there, and almost nothing on the machine holds a hex.
//!
//! Nothing here is installed. This writes into `files/` and `console apply`
//! puts those on the machine, so the palette goes through the same manifest as
//! everything else and `console check` reports a drifted color like any other
//! drift.

mod measure;
mod palette;
mod region;
mod report;
mod configuration;
mod spend;
mod terminal;

use console_core_color::Short;
use console_core_never::Never;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use measure::{Clears, Row, measure};
use spend::{How, Written};
use terminal::Terminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Write,
    Check,
}

#[derive(Debug)]
enum Unspent {
    Arguments(Vec<String>),
    Rootless(console_repository::NotFound),
    Undeclared(std::io::Error),
    Unparsed(toml::de::Error),
    Color(Short),
    FallsShort(String),
    Read(PathBuf, std::io::Error),
    NoRegion(PathBuf),
    Holding(PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for Unspent {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unspent::Arguments(other) => write!(
                to,
                "console-palette takes --check and nothing else, not {other:?}"
            ),
            Unspent::Rootless(fault) => write!(to, "{fault}"),
            Unspent::Undeclared(fault) => {
                write!(to, "theme/palette.toml could not be read: {fault}")
            }
            Unspent::Unparsed(fault) => write!(to, "theme/palette.toml does not parse: {fault}"),
            Unspent::Color(fault) => write!(to, "{fault}"),
            Unspent::FallsShort(complaint) => write!(to, "{complaint}"),
            Unspent::Read(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Unspent::NoRegion(at) => write!(
                to,
                "{} has no single {}..{} to write into",
                at.display(),
                region::BEGIN,
                region::END
            ),
            Unspent::Holding(at, fault) => {
                write!(to, "{} could not be made: {fault}", at.display())
            }
            Unspent::Writing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unspent {}

impl From<console_repository::NotFound> for Unspent {
    fn from(fault: console_repository::NotFound) -> Self {
        Unspent::Rootless(fault)
    }
}

impl From<Short> for Unspent {
    fn from(fault: Short) -> Self {
        Unspent::Color(fault)
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, Unspent> {
    let doing = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => Effect::Write,
        [flag] => match flag.as_str() {
            "--check" => Effect::Check,
            "--help" | "-h" => {
                println!("{}", HELP);
                return Ok(ExitCode::SUCCESS);
            }
            _ => return Err(Unspent::Arguments(vec![flag.clone()])),
        },
        other => return Err(Unspent::Arguments(other.to_vec())),
    };

    let root = console_repository::root()?;
    let declared = std::fs::read_to_string(root.join("theme/palette.toml"))
        .map_err(Unspent::Undeclared)?;
    let configuration: configuration::Configuration = toml::from_str(&declared).map_err(Unspent::Unparsed)?;

    let palette = palette::resolve(&configuration.color)?;
    let rows = measure(&configuration, &palette)?;

    let Ok(short) = falls_short(&rows);

    match short {
        Some(complaint) => return Err(Unspent::FallsShort(complaint)),
        None => {},
    }

    let terminal = Terminal::of(&configuration, &palette)?;
    let work = {
        let mut work = spend::everywhere(&root.join("files"), &palette, &terminal)?;
        let body = report::write(&configuration, &palette, &rows, &terminal)?;

        work.push(Written {
            path: root.join("theme/report.md"),
            how: How::Whole,
            body,
        });
        work.sort_by(|one, other| one.path.cmp(&other.path));
        work
    };

    let asked = work
        .iter()
        .map(|written| wanted(written).map(|body| (written, body)))
        .collect::<Result<Vec<_>, Unspent>>()?;
    let changed = asked
        .into_iter()
        .filter(|(written, body)| match std::fs::read(&written.path) {
            Ok(held) => held != body.as_bytes(),
            Err(_) => true,
        })
        .map(|(written, body)| match doing {
            Effect::Check => Ok(written.path.clone()),
            Effect::Write => put(&written.path, &body).map(|()| written.path.clone()),
        })
        .collect::<Result<Vec<PathBuf>, Unspent>>()?;

    let Ok(()) = say(&configuration, &rows);

    let named = |path: &Path| {
        match path.strip_prefix(&root) {
            Ok(under) => under.display().to_string(),
            Err(_outside_the_tree) => path.display().to_string(),
        }
    };

    match (doing, changed.as_slice()) {
        (_, []) => println!("  every file already says this."),
        (Effect::Check, paths) => {
            paths
                .iter()
                .for_each(|path| println!("  would rewrite {}", named(path)));
            return Ok(ExitCode::FAILURE);
        }
        (Effect::Write, paths) => {
            paths
                .iter()
                .for_each(|path| println!("  wrote {}", named(path)));
        }
    }

    Ok(ExitCode::SUCCESS)
}

const HELP: &str = "\
console-palette          write the palette out of theme/palette.toml
console-palette --check  say what it would change, change nothing";

fn wanted(written: &Written) -> Result<String, Unspent> {
    match written.how {
        How::Whole => Ok(written.body.clone()),
        How::Region => {
            let held = std::fs::read_to_string(&written.path)
                .map_err(|fault| Unspent::Read(written.path.clone(), fault))?;
            let Ok(spliced) = region::spliced(&held, region::Body(&written.body));

            spliced.ok_or_else(|| Unspent::NoRegion(written.path.clone()))
        }
    }
}

fn put(path: &Path, body: &str) -> Result<(), Unspent> {
    match path.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unspent::Holding(holding.to_path_buf(), fault))?,
        None => {},
    }

    console_core_atomic_writes::whole(path, body.as_bytes()).map_err(Unspent::Writing)
}

fn falls_short(rows: &[Row]) -> Result<Option<String>, Never> {
    let short: Vec<&Row> = rows.iter().filter(|row| row.short() == Ok(Clears::Short)).collect();

    match short.as_slice() {
        [] => Ok(None),
        short => Ok(Some(
            short
                .iter()
                .map(|row| {
                    let Ok(asked) = report::ratio(row.asked);

                    let Ok(lc) = report::asked_lc(row.asked_lc);

                    format!(
                        "  {} on {}: asked {asked}:1 and {lc}, got {:.2}:1 and Contrast {:.1} ({})",
                        row.front, row.back, row.got, row.got_lc, row.where_
                    )
                })
                .chain(["the palette does not clear what it declares; nothing written".to_string()])
                .collect::<Vec<_>>()
                .join("\n"),
        )),
    }
}

fn say(configuration: &configuration::Configuration, rows: &[Row]) -> Result<(), Never> {
    let closest = rows.iter().min_by(|one, other| {
        let Ok(one) = one.room();

        let Ok(other) = other.room();

        one.total_cmp(&other)
    });

    let worst = match closest {
        Some(worst) => worst,
        None => {
            println!("nothing to measure: this palette declares no pairing");

            return Ok(());
        }
    };

    println!(
        "{}: {} colors, {} pairings, all clearing both measures.",
        configuration.meta.name,
        configuration.color.len(),
        rows.len()
    );
    let asked = report::ratio(worst.asked)?;

    let grade = worst.grade()?;

    println!(
        "  the closest ratio is {} on {}, asked for {asked}:1 and reaching {:.2}:1 ({grade}).",
        worst.front, worst.back, worst.got
    );

    let tightest = rows.iter().filter(|row| row.asked_lc > 0.0).min_by(|one, other| {
        let Ok(one) = one.room_lc();

        let Ok(other) = other.room_lc();

        one.total_cmp(&other)
    });

    match tightest {
        Some(tightest) => {
            let asked = report::asked_lc(tightest.asked_lc)?;

            let grade = tightest.grade_lc()?;

            println!(
                "  the closest Contrast is {} on {}, asked for {asked} and reaching {:.1} ({grade}).",
                tightest.front, tightest.back, tightest.got_lc
            );
        }
        None => {},
    }

    Ok(())
}

