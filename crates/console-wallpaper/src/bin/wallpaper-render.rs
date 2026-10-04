//! Write the wallpapers.
//!
//!     wallpaper-render                    render what the table names and this has not
//!     wallpaper-render --again            render all of them, whether or not they are here
//!     wallpaper-render --dropped          render what is in Pictures/Wallpapers
//!     wallpaper-render --take PATH...     render these, whatever and wherever they are
//!     wallpaper-render --into DIR         write them somewhere else
//!     wallpaper-render --cube GRADE PATH  one grade as a cube, to look at
//!     wallpaper-render --try SRC GRADE TO render one source, to look at
//!
//! A GRADE is four numbers: keep,pull,floor,ceiling.
//!
//! The pictures the machine comes with are named in `theme/sky.toml` and
//! fetched, because they are someone else's work and this repository is
//! source. Hers are whatever she has put in `Pictures/Wallpapers`, and they go
//! somewhere an update cannot replace them.


use console_core_arguments::{Command, CommandLine, Flag, NoSubcommand, Operands, Reason, Takes, ValidationError};
use console_core_geometry::Size;
use console_core_internal_programs::{WALLPAPER_DROPPED, WALLPAPER_TAKE};
use console_core_never::Never;
use console_core_number_conversion::Float;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_wallpaper::palette;
use console_screen::{CONFIGURATION, Screen};
use console_wallpaper::choose::{Picture, Set};
use console_wallpaper::grade::{Grade, Ramp, cube};
use console_wallpaper::render::{self, Stir};
use console_wallpaper::{Unpainted, place, source};

const SIDE: u32 = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Again {
    Yes,
    No,
}

enum Action {
    Set { again: Again },
    Take(Vec<PathBuf>),
    Dropped,
    Cube { how: String, into: PathBuf },
    Try { source: PathBuf, how: String, into: PathBuf },
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let Ok(code) = console_core_arguments::run_main(&COMMAND, &words, asked, |(effect, into)| run(effect, into));

    code
}

#[derive(Debug)]
enum Unrendered {
    NoOnes,
    Read(PathBuf, std::io::Error),
    Painting(Unpainted),
    Undeclared(console_screen::Undeclared),
    NotANumber(String),
    NotAGrade,
    Holding(PathBuf, std::io::Error),
    Unwritten(console_core_atomic_writes::Unwritten),
    Untabled(toml::de::Error),
    NoCache,
    NotTheOne(String, String),
    SomeLeft(Vec<String>),
}

impl std::fmt::Display for Unrendered {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unrendered::NoOnes => write!(to, "this machine will not say whose it is"),
            Unrendered::Read(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Unrendered::Painting(fault) => write!(to, "{fault}"),
            Unrendered::Undeclared(fault) => write!(to, "{fault}"),
            Unrendered::NotANumber(word) => write!(to, "{word} is not a number"),
            Unrendered::NotAGrade => {
                write!(to, "a grade is four numbers: keep,pull,floor,ceiling")
            }
            Unrendered::Holding(at, fault) => {
                write!(to, "{} could not be made: {fault}", at.display())
            }
            Unrendered::Unwritten(fault) => write!(to, "{fault}"),
            Unrendered::Untabled(fault) => write!(to, "the table does not parse: {fault}"),
            Unrendered::NoCache => write!(
                to,
                "nothing says where a cache is, so there is nowhere to keep what is fetched"
            ),
            Unrendered::NotTheOne(wanted, found) => write!(
                to,
                "the source is not the one written down.\n    wanted {wanted}\n    found  {found}\n\
                 Look at it, and if it is right put the new sum in theme/sky.toml."
            ),
            Unrendered::SomeLeft(left) => {
                write!(to, "what could not be rendered:\n{}", left.join("\n"))
            }
        }
    }
}

impl std::error::Error for Unrendered {}

impl From<Unpainted> for Unrendered {
    fn from(fault: Unpainted) -> Self {
        Unrendered::Painting(fault)
    }
}

impl From<console_screen::Undeclared> for Unrendered {
    fn from(fault: console_screen::Undeclared) -> Self {
        Unrendered::Undeclared(fault)
    }
}

const AGAIN: Flag = Flag { spelling: "--again", takes: Takes::None, about: "render all of them, whether or not they are here" };

const INTO: Flag = Flag { spelling: "--into", takes: Takes::Value("DIR"), about: "write them somewhere else" };

const CUBE: Flag = Flag {
    spelling: "--cube",
    takes: Takes::None,
    about: "one GRADE as a cube at PATH, to look at; a grade is four numbers, keep,pull,floor,ceiling",
};

const TRY: Flag = Flag { spelling: "--try", takes: Takes::None, about: "render one SOURCE in one GRADE to TO, to look at" };

const COMMAND: Command = Command {
    name: "wallpaper-render",
    about: "render what the table names and this has not",
    flags: &[AGAIN, WALLPAPER_DROPPED, WALLPAPER_TAKE, INTO, CUBE, TRY],
    operands: Operands::Any("PATH"),
};

fn asked(words: &[String]) -> Result<(Action, Option<PathBuf>), ValidationError> {
    let line = console_core_arguments::read(&COMMAND, words)?;
    let Ok(into) = line.value(INTO);
    let chosen = chosen(&line)?;

    let effect = match chosen {
        None => {
            let [] = line.exactly([])?;

            Action::Set { again: Again::No }
        }
        Some(AGAIN) => {
            let [] = line.exactly([])?;

            Action::Set { again: Again::Yes }
        }
        Some(WALLPAPER_DROPPED) => {
            let [] = line.exactly([])?;

            Action::Dropped
        }
        Some(WALLPAPER_TAKE) => {
            let paths = taken(&line)?;

            Action::Take(paths)
        }
        Some(CUBE) => {
            let [how, at] = line.exactly(["GRADE", "PATH"])?;

            Action::Cube { how: how.clone(), into: PathBuf::from(at) }
        }
        Some(_try) => {
            let [from, how, at] = line.exactly(["SOURCE", "GRADE", "TO"])?;

            Action::Try { source: PathBuf::from(from), how: how.clone(), into: PathBuf::from(at) }
        }
    };

    Ok((effect, into.map(PathBuf::from)))
}

fn chosen(line: &CommandLine<NoSubcommand>) -> Result<Option<Flag>, ValidationError> {
    match line.one_of(&[AGAIN, WALLPAPER_DROPPED, WALLPAPER_TAKE, CUBE, TRY]) {
        Ok(flag) => Ok(Some(flag)),
        Err(ValidationError { reason: Reason::MissingFlag(_), .. }) => Ok(None),
        Err(refusal) => Err(refusal),
    }
}

fn taken(line: &CommandLine<NoSubcommand>) -> Result<Vec<PathBuf>, ValidationError> {
    let Ok(operands) = line.operands();

    match operands.is_empty() {
        true => {
            let Ok(refusal) = line.refusal(Reason::MissingOperands(vec!["PATH"]));

            Err(refusal)
        }
        false => Ok(operands.iter().map(PathBuf::from).collect()),
    }
}

fn run(effect: Action, into: Option<PathBuf>) -> Result<(), Unrendered> {
    match effect {
        Action::Cube { how, into } => {
            let ramp = read_ramp()?;
            let grade = read_grade(&how)?;

            write_cube(&ramp, &grade, &into)
        }
        Action::Try { source, how, into } => {
            let grade = read_grade(&how)?;

            press_one(&source, &grade, &into)
        }
        Action::Set { again } => press_set(again, into),
        Action::Take(paths) => press_hers(&paths, into),
        Action::Dropped => {
            let Ok(dropped) = place::dropped();

            let at = dropped.ok_or(Unrendered::NoOnes)?;

            let reading = std::fs::read_dir(&at)
                .map_err(|fault| Unrendered::Read(at.clone(), fault))?;
            let mut found: Vec<PathBuf> = Vec::new();

            for entry in reading {
                let entry =
                    entry.map_err(|fault| Unrendered::Read(at.clone(), fault))?;

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


fn read(named: &str) -> Result<String, Unrendered> {
    let Ok(tree) = place::tree();

    let at = tree.join(named);

    std::fs::read_to_string(&at).map_err(|fault| Unrendered::Read(at, fault))
}

fn read_ramp() -> Result<Ramp, Unrendered> {
    let report = read("theme/report.md")?;
    let colors = palette::read(&report)?;

    Ramp::read(&|name| colors.get(name).cloned()).map_err(Unrendered::Painting)
}

fn read_grade(how: &str) -> Result<Grade, Unrendered> {
    let numbers: Vec<f64> = how
        .split(',')
        .map(|word| {
            word.trim()
                .parse()
                .map_err(|_| Unrendered::NotANumber(word.to_string()))
        })
        .collect::<Result<_, _>>()?;

    match numbers.as_slice() {
        [keep, pull, floor, ceiling] => {
            Ok(Grade { keep: *keep, pull: *pull, floor: *floor, ceiling: *ceiling })
        }
        _ => Err(Unrendered::NotAGrade),
    }
}

fn write_cube(ramp: &Ramp, how: &Grade, into: &Path) -> Result<(), Unrendered> {
    match into.parent() {
        Some(holding) => {
            let _ = std::fs::create_dir_all(holding);
        }
        None => {},
    }

    let Ok(written) = cube(ramp, how, SIDE);

    console_core_atomic_writes::whole(into, written.as_bytes()).map_err(Unrendered::Unwritten)
}

fn screen() -> Result<Screen, Unrendered> {
    let configuration = read(CONFIGURATION)?;

    Screen::read(&configuration).map_err(Unrendered::Undeclared)
}

fn write_one(
    source: &Path,
    grade: &Grade,
    stir: &Stir,
    size: Size<u32>,
    into: &Path,
) -> Result<render::Rendered, Unrendered> {
    match into.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unrendered::Holding(holding.to_path_buf(), fault))?,
        None => {},
    }

    let cube = into.with_extension("cube");
    let ramp = read_ramp()?;

    write_cube(&ramp, grade, &cube)?;
    let rendered = render::render(source, &cube, size, stir);
    let _ = std::fs::remove_file(&cube);
    let rendered = rendered?;

    console_core_atomic_writes::whole(&into.with_extension("webp"), &rendered.animation)
        .map_err(Unrendered::Unwritten)?;
    console_core_atomic_writes::whole(&into.with_extension("still.webp"), &rendered.still)
        .map_err(Unrendered::Unwritten)?;
    Ok(rendered)
}

fn say(name: &str, rendered: &render::Rendered) -> Result<(), Never> {
    let (from, to) = rendered.slice;
    let Ok(wide) = rendered.animation.len().float();

    println!(
        "  {name}: frames {from} to {to}, {:.0}% of the picture moves, {:.0} KiB.",
        rendered.largest * 100.0,
        wide / 1024.0
    );

    Ok(())
}

fn press_set(again: Again, into: Option<PathBuf>) -> Result<(), Unrendered> {
    let written = read("theme/sky.toml")?;
    let table: Set = toml::from_str(&written).map_err(Unrendered::Untabled)?;
    let into = match into {
        Some(into) => into,
        None => PathBuf::from(place::CAME_WITH),
    };
    let asked = screen()?;
    let Ok(size) = asked.pixels();

    println!("{} pictures, at {}x{}.", table.pictures.len(), size.width, size.height);
    let mut rendered: u32 = 0;
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

                rendered = rendered.saturating_add(1);
            }
            Err(fault) => left.push(format!("  {}: {fault}", picture.name)),
        }
    }

    match rendered > 0 {
        true => {
            let Ok(()) = forget_the_cache();
        }
        false => {},
    }

    match left.is_empty() {
        true => Ok(()),
        false => Err(Unrendered::SomeLeft(left)),
    }
}

fn press_named(
    picture: &Picture,
    stir: &Stir,
    size: Size<u32>,
    at: &Path,
) -> Result<render::Rendered, Unrendered> {
    let Ok(kept) = source::cache_path();

    let kept = match kept {
        Some(kept) => kept,
        None => return Err(Unrendered::NoCache),
    };

    let held = kept.join(&picture.name);

    let got =
        source::get(source::Source { from: &picture.from, wanted: &picture.sha256 }, &held)?;

    match got {
        source::NameRequestResult::Replaced { wanted, found } => {
            return Err(Unrendered::NotTheOne(wanted, found));
        }
        source::NameRequestResult::Fetched | source::NameRequestResult::Cached => (),
    }

    let grade = match picture.grade {
        Some(grade) => grade,
        None => Grade::default(),
    };

    write_one(&held, &grade, stir, size, at)
}

fn press_hers(paths: &[PathBuf], into: Option<PathBuf>) -> Result<(), Unrendered> {
    let into = match into {
        Some(at) => at,
        None => {
            let Ok(hers) = place::user();

            hers.ok_or(Unrendered::NoOnes)?
        }
    };
    let asked = screen()?;
    let Ok(size) = asked.pixels();

    let stir = Stir::default();
    let grade = Grade::default();

    let mut left = Vec::new();
    let mut rendered: u32 = 0;

    for path in paths {
        let name = match path.file_stem().and_then(|name| name.to_str()) {
            Some(name) => name,
            None => {
                left.push(format!("  {}: that is not a name", path.display()));
                continue;
            }
        };

        match write_one(path, &grade, &stir, size, &into.join(name)) {
            Ok(done) => {
                let Ok(()) = say(name, &done);

                rendered = rendered.saturating_add(1);
            }
            Err(fault) => left.push(format!("  {name}: {fault}")),
        }
    }

    match rendered > 0 {
        true => {
            let Ok(()) = forget_the_cache();
        }
        false => {},
    }

    match left.is_empty() {
        true => Ok(()),
        false => Err(Unrendered::SomeLeft(left)),
    }
}

fn press_one(source: &Path, grade: &Grade, into: &Path) -> Result<(), Unrendered> {
    let stir = Stir::default();
    let asked = screen()?;
    let Ok(size) = asked.pixels();
    let rendered = write_one(source, grade, &stir, size, into)?;

    let Ok(()) = say(&into.display().to_string(), &rendered);

    Ok(())
}

fn forget_the_cache() -> Result<(), Never> {
    let Ok(said) = console_core_places::home();

    let home = match said {
        Some(home) => home,
        None => return Ok(()),
    };

    let _ = std::fs::remove_dir_all(home.join(".cache/awww"));

    Ok(())
}
