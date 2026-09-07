//! Spend the palette.
//!
//!     console-palette          write the palette out of theme/palette.toml
//!     console-palette --check  say what it would change, change nothing
//!
//! `theme/palette.toml` is the one place a colour is decided. Everything on
//! the machine reads from there, and almost nothing on the machine holds a hex.
//!
//! Nothing here is installed. This writes into `files/` and `console apply`
//! puts those on the machine, so the palette goes through the same manifest as
//! everything else and `console check` reports a drifted colour like any other
//! drift.

mod measure;
mod palette;
mod region;
mod report;
mod spec;
mod spend;
mod terminal;

use console_core_colour::Short;
use console_core_never::Never;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use measure::{Clears, Row, measure};
use spend::{How, Written};
use terminal::Terminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doing {
    Write,
    Check,
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

fn run() -> Result<ExitCode, String> {
    let doing = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => Doing::Write,
        [flag] if flag == "--check" => Doing::Check,
        [flag] if flag == "--help" || flag == "-h" => {
            println!("{}", HELP);
            return Ok(ExitCode::SUCCESS);
        }
        other => {
            return Err(format!(
                "console-palette takes --check and nothing else, not {other:?}"
            ));
        }
    };

    let root = console_repository::root()?;
    let declared = std::fs::read_to_string(root.join("theme/palette.toml"))
        .map_err(|fault| format!("theme/palette.toml could not be read: {fault}"))?;
    let spec: spec::Spec = toml::from_str(&declared)
        .map_err(|fault| format!("theme/palette.toml does not parse: {fault}"))?;

    let said = |fault: Short| fault.0;
    let palette = palette::resolve(&spec.colour).map_err(said)?;
    let rows = measure(&spec, &palette).map_err(said)?;

    let Ok(short) = falls_short(&rows);

    match short {
        Some(complaint) => return Err(complaint),
        None => {},
    }

    let terminal = Terminal::of(&spec, &palette).map_err(said)?;
    let work = {
        let mut work =
            spend::everywhere(&root.join("files"), &palette, &terminal).map_err(said)?;
        let body = report::write(&spec, &palette, &rows, &terminal).map_err(said)?;

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
        .collect::<Result<Vec<_>, String>>()?;
    let changed = asked
        .into_iter()
        .filter(|(written, body)| match std::fs::read(&written.path) {
            Ok(held) => held != body.as_bytes(),
            Err(_) => true,
        })
        .map(|(written, body)| match doing {
            Doing::Check => Ok(written.path.clone()),
            Doing::Write => put(&written.path, &body).map(|()| written.path.clone()),
        })
        .collect::<Result<Vec<PathBuf>, String>>()?;

    let Ok(()) = say(&spec, &rows);

    let named = |path: &Path| {
        path.strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string()
    };

    match (doing, changed.as_slice()) {
        (_, []) => println!("  every file already says this."),
        (Doing::Check, paths) => {
            paths
                .iter()
                .for_each(|path| println!("  would rewrite {}", named(path)));
            return Ok(ExitCode::FAILURE);
        }
        (Doing::Write, paths) => {
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

fn wanted(written: &Written) -> Result<String, String> {
    match written.how {
        How::Whole => Ok(written.body.clone()),
        How::Region => {
            let held = std::fs::read_to_string(&written.path).map_err(|fault| {
                format!("{} could not be read: {fault}", written.path.display())
            })?;
            let Ok(spliced) = region::spliced(&held, &written.body);

            spliced.ok_or_else(|| {
                format!(
                    "{} has no single {}..{} to write into",
                    written.path.display(),
                    region::BEGIN,
                    region::END
                )
            })
        }
    }
}

fn put(path: &Path, body: &str) -> Result<(), String> {
    match path.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?,
        None => {},
    }

    std::fs::write(path, body)
        .map_err(|fault| format!("{} could not be written: {fault}", path.display()))
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
                        "  {} on {}: asked {asked}:1 and {lc}, got {:.2}:1 and Lc {:.1} ({})",
                        row.front, row.back, row.got, row.got_lc, row.where_
                    )
                })
                .chain(["the palette does not clear what it declares; nothing written".to_string()])
                .collect::<Vec<_>>()
                .join("\n"),
        )),
    }
}

fn say(spec: &spec::Spec, rows: &[Row]) -> Result<(), Never> {
    let closest = rows.iter().min_by(|one, other| {
        let Ok(one) = one.room();

        let Ok(other) = other.room();

        one.total_cmp(&other)
    });

    let Some(worst) = closest else {
        println!("nothing to measure: this palette declares no pairing");

        return Ok(());
    };

    println!(
        "{}: {} colours, {} pairings, all clearing both measures.",
        spec.meta.name,
        spec.colour.len(),
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
                "  the closest Lc is {} on {}, asked for {asked} and reaching {:.1} ({grade}).",
                tightest.front, tightest.back, tightest.got_lc
            );
        }
        None => {},
    }

    Ok(())
}

