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

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_garden::palette;
use console_screen::{CONFIG, Screen};
use console_sky::choose::{Picture, Set};
use console_sky::grade::{Grade, Ramp, cube};
use console_sky::press::{self, Stir};
use console_sky::{place, source};

/// How fine the grade's lattice is. The grade is smooth, so interpolating
/// between lattice points cannot be told from working it out for every pixel.
const SIDE: usize = 33;

/// What a run was asked to do.
enum Doing {
    /// Press what the table names.
    Set { again: bool },
    /// Press these files, wherever they came from.
    Take(Vec<PathBuf>),
    /// Press what is in the drop.
    Dropped,
    /// Write one grade out, to look at.
    Cube { how: String, into: PathBuf },
    /// Press one source, to look at.
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

    // Pulled out before the rest is read, so it can be given with any of them.
    let mut into: Option<PathBuf> = None;
    if let Some(at) = words.iter().position(|word| *word == "--into") {
        let named = words.get(at + 1).ok_or("--into wants a directory")?;
        into = Some(PathBuf::from(named));
        words.drain(at..=at + 1);
    }

    let doing = match words.as_slice() {
        [] => Doing::Set { again: false },
        ["--again"] => Doing::Set { again: true },
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
        Doing::Cube { how, into } => write_cube(&read_ramp()?, &read_grade(&how)?, &into),
        Doing::Try { source, how, into } => press_one(&source, &read_grade(&how)?, &into),
        Doing::Set { again } => press_set(again, into),
        Doing::Take(paths) => press_hers(&paths, into),
        Doing::Dropped => {
            let at = place::dropped().ok_or("this machine will not say whose it is")?;
            let mut found: Vec<PathBuf> = std::fs::read_dir(&at)
                .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?
                .filter_map(|entry| entry.ok().map(|found| found.path()))
                .filter(|path| path.is_file())
                .collect();
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

// ---------------------------------------------------------------- the palette

/// A file out of the tree the palette and the table live in.
fn read(named: &str) -> Result<String, String> {
    let at = place::tree().join(named);
    std::fs::read_to_string(&at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))
}

fn read_ramp() -> Result<Ramp, String> {
    let colours = palette::read(&read("theme/report.md")?)?;
    Ramp::read(&|name| colours.get(name).cloned())
}

fn read_grade(how: &str) -> Result<Grade, String> {
    let numbers: Vec<f64> = how
        .split(',')
        .map(|word| word.trim().parse().map_err(|_| format!("{word} is not a number")))
        .collect::<Result<_, _>>()?;
    match numbers[..] {
        [keep, pull, floor, ceiling] => Ok(Grade { keep, pull, floor, ceiling }),
        _ => Err("a grade is four numbers: keep,pull,floor,ceiling".to_string()),
    }
}

fn write_cube(ramp: &Ramp, how: &Grade, into: &Path) -> Result<(), String> {
    if let Some(holding) = into.parent() {
        let _ = std::fs::create_dir_all(holding);
    }
    std::fs::write(into, cube(ramp, how, SIDE))
        .map_err(|fault| format!("{} could not be written: {fault}", into.display()))
}

/// The screen the pictures are drawn for.
fn screen() -> Result<Screen, String> {
    Screen::read(&read(CONFIG)?)
}

// ----------------------------------------------------------------- the press

/// One source, pressed and written as a moving picture and a still one.
fn write_one(
    source: &Path,
    grade: &Grade,
    stir: &Stir,
    size: (u32, u32),
    into: &Path,
) -> Result<press::Pressed, String> {
    if let Some(holding) = into.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?;
    }
    let cube = into.with_extension("cube");
    write_cube(&read_ramp()?, grade, &cube)?;
    let pressed = press::press(source, &cube, size, stir);
    let _ = std::fs::remove_file(&cube);
    let pressed = pressed?;

    std::fs::write(into.with_extension("webp"), &pressed.animation)
        .map_err(|fault| format!("{} could not be written: {fault}", into.display()))?;
    std::fs::write(into.with_extension("still.webp"), &pressed.still)
        .map_err(|fault| format!("{} could not be written: {fault}", into.display()))?;
    Ok(pressed)
}

/// What a pressed picture came out as, said the way the garden says it.
fn say(name: &str, pressed: &press::Pressed) {
    let (from, to) = pressed.slice;
    println!(
        "  {name}: frames {from} to {to}, {:.0}% of the picture moves, {:.0} KiB.",
        pressed.largest * 100.0,
        pressed.animation.len() as f64 / 1024.0
    );
}

/// Every picture the table names.
fn press_set(again: bool, into: Option<PathBuf>) -> Result<(), String> {
    let table: Set = toml::from_str(&read("theme/sky.toml")?)
        .map_err(|fault| format!("the table does not parse: {fault}"))?;
    let into = into.unwrap_or_else(|| PathBuf::from(place::CAME_WITH));
    let size = screen()?.pixels();

    println!("{} pictures, at {}x{}.", table.pictures.len(), size.0, size.1);
    let mut pressed = 0;
    let mut left = Vec::new();
    for picture in &table.pictures {
        let at = into.join(&picture.name);
        if !again && at.with_extension("webp").is_file() {
            continue;
        }
        match press_named(picture, &table.stir, size, &at) {
            Ok(done) => {
                say(&picture.name, &done);
                pressed += 1;
            }
            Err(fault) => left.push(format!("  {}: {fault}", picture.name)),
        }
    }

    if pressed > 0 {
        forget_the_cache();
    }
    match left.is_empty() {
        true => Ok(()),
        false => Err(format!("what could not be pressed:\n{}", left.join("\n"))),
    }
}

/// One of the table's pictures: fetched if it has to be, then pressed.
fn press_named(
    picture: &Picture,
    stir: &Stir,
    size: (u32, u32),
    at: &Path,
) -> Result<press::Pressed, String> {
    let held = source::kept().join(&picture.name);
    match source::get(&picture.from, &picture.sha256, &held)? {
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

/// Pictures of hers, from wherever she put them.
///
/// The grade is the default one, because there is nowhere for her to have said
/// otherwise and the settings offers to turn it off rather than to tune it.
/// Somebody who wants a particular grade for a particular picture of hers can
/// write it into the table like any other.
fn press_hers(paths: &[PathBuf], into: Option<PathBuf>) -> Result<(), String> {
    let into = match into {
        Some(at) => at,
        None => place::hers().ok_or("this machine will not say whose it is")?,
    };
    let size = screen()?.pixels();
    let stir = Stir::default();
    let grade = Grade::default();

    let mut left = Vec::new();
    let mut pressed = 0;
    for path in paths {
        let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
            left.push(format!("  {}: that is not a name", path.display()));
            continue;
        };
        match write_one(path, &grade, &stir, size, &into.join(name)) {
            Ok(done) => {
                say(name, &done);
                pressed += 1;
            }
            Err(fault) => left.push(format!("  {name}: {fault}")),
        }
    }

    if pressed > 0 {
        forget_the_cache();
    }
    match left.is_empty() {
        true => Ok(()),
        false => Err(format!("what could not be pressed:\n{}", left.join("\n"))),
    }
}

/// One source, pressed where somebody can look at it.
fn press_one(source: &Path, grade: &Grade, into: &Path) -> Result<(), String> {
    let stir = Stir::default();
    let pressed = write_one(source, grade, &stir, screen()?.pixels(), into)?;
    say(&into.display().to_string(), &pressed);
    Ok(())
}

/// Throw away what the wallpaper daemon remembers about these pictures.
///
/// awww names a cache entry after a picture's path, its size and how it was
/// fitted, and after nothing that is inside the file. A picture pressed again
/// at the same path is served out of the old picture's frames: the still it
/// decodes is the new one and the rectangles played over it are the old one's,
/// and the screen fills with blocks of the two mixed together. `docs/theme.md`
/// has the whole of it. Thrown away here rather than after somebody has seen
/// it happen.
fn forget_the_cache() {
    let Ok(home) = std::env::var("HOME") else { return };
    let _ = std::fs::remove_dir_all(Path::new(&home).join(".cache/awww"));
}
