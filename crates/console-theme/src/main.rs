//! Spend the palette.
//!
//!     console-theme          write the palette out of theme/palette.toml
//!     console-theme --check  say what it would change, change nothing
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

use console_colour::Short;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use measure::{Clears, Row, measure};
use spend::{How, Written};
use terminal::Terminal;

/// What the run was asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Doing {
    /// Write every file that does not already say this.
    Write,
    /// Say what would change, and change nothing.
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
                "console-theme takes --check and nothing else, not {other:?}"
            ));
        }
    };

    let root = repository()?;
    let declared = std::fs::read_to_string(root.join("theme/palette.toml"))
        .map_err(|fault| format!("theme/palette.toml could not be read: {fault}"))?;
    let spec: spec::Spec = toml::from_str(&declared)
        .map_err(|fault| format!("theme/palette.toml does not parse: {fault}"))?;

    // Every colour this needs is looked up before a single file is written, and
    // a name nobody declared stops it here. It used to panic where the name was
    // reached for, part way through writing the desktop's colours out -- so a
    // misspelled role left some files spent and the rest as they were, and said
    // so with a backtrace, inside `just deploy`.
    let said = |fault: Short| fault.0;
    let palette = palette::resolve(&spec.colour).map_err(said)?;
    let rows = measure(&spec, &palette).map_err(said)?;

    if let Some(complaint) = falls_short(&rows) {
        return Err(complaint);
    }

    let terminal = Terminal::of(&spec, &palette).map_err(said)?;
    let work = {
        let mut work =
            spend::everywhere(&root.join("files"), &palette, &terminal).map_err(said)?;
        work.push(Written {
            path: root.join("theme/report.md"),
            how: How::Whole,
            body: report::write(&spec, &palette, &rows, &terminal).map_err(said)?,
        });
        work.sort_by(|one, other| one.path.cmp(&other.path));
        work
    };

    let changed = work
        .iter()
        .map(|written| wanted(written).map(|body| (written, body)))
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .filter(|(written, body)| match std::fs::read(&written.path) {
            Ok(held) => held != body.as_bytes(),
            // Nothing there, or nothing this program may read. Either way what
            // is on the disk is not what this run wants, and the answer to both
            // is the same: write it, and let the write say why it could not.
            Err(_) => true,
        })
        .map(|(written, body)| match doing {
            Doing::Check => Ok(written.path.clone()),
            Doing::Write => put(&written.path, &body).map(|()| written.path.clone()),
        })
        .collect::<Result<Vec<PathBuf>, String>>()?;

    say(&spec, &rows);
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
console-theme          write the palette out of theme/palette.toml
console-theme --check  say what it would change, change nothing";

/// What a file should hold, whole or between its markers.
fn wanted(written: &Written) -> Result<String, String> {
    match written.how {
        How::Whole => Ok(written.body.clone()),
        How::Region => {
            let held = std::fs::read_to_string(&written.path).map_err(|fault| {
                format!("{} could not be read: {fault}", written.path.display())
            })?;
            region::spliced(&held, &written.body).ok_or_else(|| {
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
    if let Some(holding) = path.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?;
    }

    std::fs::write(path, body)
        .map_err(|fault| format!("{} could not be written: {fault}", path.display()))
}

/// Every pairing that does not reach what it declares, or nothing.
///
/// A palette that reads badly must not reach the device, so this is a gate
/// rather than a warning: one short pairing and not a single file is written.
fn falls_short(rows: &[Row]) -> Option<String> {
    let short: Vec<&Row> = rows.iter().filter(|row| row.short() == Clears::Short).collect();

    match short.as_slice() {
        [] => None,
        short => Some(
            short
                .iter()
                .map(|row| {
                    format!(
                        "  {} on {}: asked {}:1 and {}, got {:.2}:1 and Lc {:.1} ({})",
                        row.front,
                        row.back,
                        report::ratio(row.asked),
                        report::asked_lc(row.asked_lc),
                        row.got,
                        row.got_lc,
                        row.where_
                    )
                })
                .chain(["the palette does not clear what it declares; nothing written".to_string()])
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    }
}

/// What was measured, in a handful of lines.
fn say(spec: &spec::Spec, rows: &[Row]) {
    // The closest call is the one with the least room over what it was asked
    // for, which is not the same as the lowest ratio: the bar only has to be a
    // different colour from the wallpaper, and it always will be.
    let Some(worst) = rows.iter().min_by(|one, other| one.room().total_cmp(&other.room())) else {
        // A palette that declares no pairing is a palette nothing was measured
        // against, and saying so is the measurement.
        println!("nothing to measure: this palette declares no pairing");
        return;
    };

    println!(
        "{}: {} colours, {} pairings, all clearing both measures.",
        spec.meta.name,
        spec.colour.len(),
        rows.len()
    );
    println!(
        "  the closest ratio is {} on {}, asked for {}:1 and reaching {:.2}:1 ({}).",
        worst.front,
        worst.back,
        report::ratio(worst.asked),
        worst.got,
        worst.grade()
    );

    // And the same question in the other measure, which on a dark palette is
    // the one that answers differently: a shade with room to spare on the
    // ratio can be the one sitting closest to its Lc.
    if let Some(tightest) = rows
        .iter()
        .filter(|row| row.asked_lc > 0.0)
        .min_by(|one, other| one.room_lc().total_cmp(&other.room_lc()))
    {
        println!(
            "  the closest Lc is {} on {}, asked for {} and reaching {:.1} ({}).",
            tightest.front,
            tightest.back,
            report::asked_lc(tightest.asked_lc),
            tightest.got_lc,
            tightest.grade_lc()
        );
    }
}

/// The repository this is being run inside.
///
/// Found by walking up from wherever it was started rather than from the
/// binary's own path, because a compiled program can be installed anywhere and
/// the tree it writes into is the one somebody is standing in.
fn repository() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|fault| format!("no working directory: {fault}"))?;
    here.ancestors()
        .find(|at| at.join("theme/palette.toml").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "no theme/palette.toml above {}; run this inside the repository",
                here.display()
            )
        })
}
