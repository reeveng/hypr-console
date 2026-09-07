//! Write the wallpapers.
//!
//!     sky-press                    press what the table names and this has not
//!     sky-press --again            press all of them, whether or not they are here
//!     sky-press --dropped          press what is in Pictures/Wallpapers
//!     sky-press --take PATH...     press these, whatever and wherever they are
//!     sky-press --into DIR         write them somewhere else
//!     sky-press --cube GRADE PATH  one grade as a cube, to look at
//!     sky-press --try SRC GRADE TO press one source, to look at
//!
//! A GRADE is four numbers: keep,pull,floor,ceiling.
//!
//! The pictures the machine comes with are named in `theme/sky.toml` and
//! fetched, because they are somebody else's work and this repository is
//! source. Hers are whatever she has put in `Pictures/Wallpapers`, and they go
//! somewhere an update cannot replace them.


use console_core_never::Never;
use console_core_number_conversion::Float;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_wallpaper::palette;
use console_screen::{CONFIG, Screen};
use console_wallpaper::choose::{Picture, Set};
use console_wallpaper::grade::{Grade, Ramp, cube};
use console_wallpaper::press::{self, Stir};
use console_wallpaper::{place, source};

const SIDE: usize = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Again {
    Yes,
    No,
}

enum Doing {
    Set { again: Again },
    Take(Vec<PathBuf>),
    Dropped,
    Cube { how: String, into: PathBuf },
    Try { source: PathBuf, how: String, into: PathBuf },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let mut words: Vec<&str> = words.iter().map(String::as_str).collect();

    let mut into: Option<PathBuf> = None;

    match words.iter().position(|word| *word == "--into") {
        Some(at) => {
            let named = words.get(at.saturating_add(1)).ok_or("--into wants a directory")?;
            into = Some(PathBuf::from(named));
            words.drain(at..=at.saturating_add(1));
        }
        None => {},
    }

    let doing = match words.as_slice() {
        [] => Doing::Set { again: Again::No },
        ["--again"] => Doing::Set { again: Again::Yes },
        ["--dropped"] => Doing::Dropped,
        ["--help"] | ["-h"] => {
            println!("{HELP}");
            return Ok(());
        }
        ["--take", rest @ ..] if !rest.is_empty() => {
            Doing::Take(rest.iter().map(PathBuf::from).collect())
        }
        ["--cube", how, at] => Doing::Cube { how: (*how).to_string(), into: PathBuf::from(at) },
        ["--try", from, how, at] => Doing::Try {
            source: PathBuf::from(from),
            how: (*how).to_string(),
            into: PathBuf::from(at),
        },
        _ => return Err(HELP.to_string()),
    };

    match doing {
        Doing::Cube { how, into } => {
            let ramp = read_ramp()?;
            let grade = read_grade(&how)?;

            write_cube(&ramp, &grade, &into)
        }
        Doing::Try { source, how, into } => {
            let grade = read_grade(&how)?;

            press_one(&source, &grade, &into)
        }
        Doing::Set { again } => press_set(again, into),
        Doing::Take(paths) => press_hers(&paths, into),
        Doing::Dropped => {
            let Ok(dropped) = place::dropped();

            let at = dropped.ok_or("this machine will not say whose it is")?;

            let reading = std::fs::read_dir(&at)
                .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
            let mut found: Vec<PathBuf> = Vec::new();

            for entry in reading {
                let entry = entry
                    .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;

                match entry.path().is_file() {
                    true => found.push(entry.path()),
                    false => {},
                }
            }

            found.sort();

            match found.is_empty() {
                true => {
                    println!("there is nothing in {}", at.display());
                    Ok(())
                }
                false => press_hers(&found, into),
            }
        }
    }
}

const HELP: &str = "\
sky-press                     press what the table names and this has not
sky-press --again             press all of them, whether or not they are here
sky-press --dropped           press what is in Pictures/Wallpapers
sky-press --take PATH...      press these, whatever and wherever they are
sky-press --into DIR          write them somewhere else
sky-press --cube GRADE PATH   one grade as a cube, to look at
sky-press --try SRC GRADE TO  press one source, to look at

A GRADE is four numbers: keep,pull,floor,ceiling.";

fn read(named: &str) -> Result<String, String> {
    let Ok(tree) = place::tree();

    let at = tree.join(named);

    std::fs::read_to_string(&at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))
}

fn read_ramp() -> Result<Ramp, String> {
    let report = read("theme/report.md")?;
    let colours = palette::read(&report)?;

    Ramp::read(&|name| colours.get(name).cloned())
}

fn read_grade(how: &str) -> Result<Grade, String> {
    let numbers: Vec<f64> = how
        .split(',')
        .map(|word| word.trim().parse().map_err(|_| format!("{word} is not a number")))
        .collect::<Result<_, _>>()?;

    match numbers.as_slice() {
        [keep, pull, floor, ceiling] => {
            Ok(Grade { keep: *keep, pull: *pull, floor: *floor, ceiling: *ceiling })
        }
        _ => Err("a grade is four numbers: keep,pull,floor,ceiling".to_string()),
    }
}

fn write_cube(ramp: &Ramp, how: &Grade, into: &Path) -> Result<(), String> {
    match into.parent() {
        Some(holding) => {
            let _ = std::fs::create_dir_all(holding);
        }
        None => {},
    }

    let Ok(written) = cube(ramp, how, SIDE);

    std::fs::write(into, written)
        .map_err(|fault| format!("{} could not be written: {fault}", into.display()))
}

fn screen() -> Result<Screen, String> {
    let config = read(CONFIG)?;

    Screen::read(&config)
}

fn write_one(
    source: &Path,
    grade: &Grade,
    stir: &Stir,
    size: (u32, u32),
    into: &Path,
) -> Result<press::Pressed, String> {
    match into.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?,
        None => {},
    }

    let cube = into.with_extension("cube");
    let ramp = read_ramp()?;

    write_cube(&ramp, grade, &cube)?;
    let pressed = press::press(source, &cube, size, stir);
    let _ = std::fs::remove_file(&cube);
    let pressed = pressed?;

    std::fs::write(into.with_extension("webp"), &pressed.animation)
        .map_err(|fault| format!("{} could not be written: {fault}", into.display()))?;
    std::fs::write(into.with_extension("still.webp"), &pressed.still)
        .map_err(|fault| format!("{} could not be written: {fault}", into.display()))?;
    Ok(pressed)
}

fn say(name: &str, pressed: &press::Pressed) -> Result<(), Never> {
    let (from, to) = pressed.slice;
    let Ok(wide) = pressed.animation.len().float();

    println!(
        "  {name}: frames {from} to {to}, {:.0}% of the picture moves, {:.0} KiB.",
        pressed.largest * 100.0,
        wide / 1024.0
    );

    Ok(())
}

fn press_set(again: Again, into: Option<PathBuf>) -> Result<(), String> {
    let written = read("theme/sky.toml")?;
    let table: Set = toml::from_str(&written)
        .map_err(|fault| format!("the table does not parse: {fault}"))?;
    let into = into.unwrap_or_else(|| PathBuf::from(place::CAME_WITH));
    let asked = screen()?;
    let Ok(size) = asked.pixels();

    println!("{} pictures, at {}x{}.", table.pictures.len(), size.0, size.1);
    let mut pressed: usize = 0;
    let mut left = Vec::new();

    for picture in &table.pictures {
        let at = into.join(&picture.name);

        match again == Again::No && at.with_extension("webp").is_file() {
            true => continue,
            false => {},
        }

        match press_named(picture, &table.stir, size, &at) {
            Ok(done) => {
                let Ok(()) = say(&picture.name, &done);

                pressed = pressed.saturating_add(1);
            }
            Err(fault) => left.push(format!("  {}: {fault}", picture.name)),
        }
    }

    match pressed > 0 {
        true => {
            let Ok(()) = forget_the_cache();
        }
        false => {},
    }

    match left.is_empty() {
        true => Ok(()),
        false => Err(format!("what could not be pressed:\n{}", left.join("\n"))),
    }
}

fn press_named(
    picture: &Picture,
    stir: &Stir,
    size: (u32, u32),
    at: &Path,
) -> Result<press::Pressed, String> {
    let Ok(kept) = source::kept();

    let held = kept.join(&picture.name);

    let got = source::get(&picture.from, &picture.sha256, &held)?;

    match got {
        source::Got::Changed { wanted, found } => {
            return Err(format!(
                "the source is not the one written down.\n    wanted {wanted}\n    found  {found}\n\
                 Look at it, and if it is right put the new sum in theme/sky.toml."
            ));
        }
        source::Got::Fetched | source::Got::Held => (),
    }

    write_one(&held, &picture.grade.unwrap_or_default(), stir, size, at)
}

fn press_hers(paths: &[PathBuf], into: Option<PathBuf>) -> Result<(), String> {
    let into = match into {
        Some(at) => at,
        None => {
            let Ok(hers) = place::hers();

            hers.ok_or("this machine will not say whose it is")?
        }
    };
    let asked = screen()?;
    let Ok(size) = asked.pixels();

    let stir = Stir::default();
    let grade = Grade::default();

    let mut left = Vec::new();
    let mut pressed: usize = 0;

    for path in paths {
        let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
            left.push(format!("  {}: that is not a name", path.display()));
            continue;
        };

        match write_one(path, &grade, &stir, size, &into.join(name)) {
            Ok(done) => {
                let Ok(()) = say(name, &done);

                pressed = pressed.saturating_add(1);
            }
            Err(fault) => left.push(format!("  {name}: {fault}")),
        }
    }

    match pressed > 0 {
        true => {
            let Ok(()) = forget_the_cache();
        }
        false => {},
    }

    match left.is_empty() {
        true => Ok(()),
        false => Err(format!("what could not be pressed:\n{}", left.join("\n"))),
    }
}

fn press_one(source: &Path, grade: &Grade, into: &Path) -> Result<(), String> {
    let stir = Stir::default();
    let asked = screen()?;
    let Ok(size) = asked.pixels();
    let pressed = write_one(source, grade, &stir, size, into)?;

    let Ok(()) = say(&into.display().to_string(), &pressed);

    Ok(())
}

fn forget_the_cache() -> Result<(), Never> {
    let Ok(home) = std::env::var("HOME") else { return Ok(()) };

    let _ = std::fs::remove_dir_all(Path::new(&home).join(".cache/awww"));

    Ok(())
}
