//! A picture, as characters, in the terminal.
//!
//! ```text
//! music-cover FILE [--rows ROWS]
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use console_core_arguments::{Command, Flag, Operands, Takes, ValidationError, read};
use console_core_never::Never;
use console_music::ascii;

const ROWS: u32 = 40;

const FILE: [&str; 1] = ["FILE"];

const HIGH: Flag = Flag { spelling: "--rows", takes: Takes::Value("ROWS"), about: "how many rows high to draw it, forty if none is said" };

const COMMAND: Command = Command {
    name: "music-cover",
    about: "a picture, as characters, in the terminal",
    flags: &[HIGH],
    operands: Operands::Named(&FILE),
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Picture,
    NoPicture,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cover {
    path: PathBuf,
    rows: u32,
}

fn cover(words: &[String]) -> Result<Cover, ValidationError> {
    let read = read(&COMMAND, words);
    let line = read?;
    let operands = line.exactly(FILE);
    let [path] = operands?;
    let high = line.parsed::<u32>(HIGH);
    let asked = high?;

    let rows = match asked {
        Some(rows) => rows,
        None => ROWS,
    };

    Ok(Cover { path: PathBuf::from(path), rows })
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();

    let Cover { path, rows } = match cover(&words) {
        Ok(cover) => cover,
        Err(refusal) => {
            let Ok(code) = refusal.print();

            return ExitCode::from(code);
        }
    };

    let Ok(outcome) = draw(&path, rows);

    match outcome {
        Outcome::Picture => ExitCode::SUCCESS,
        Outcome::NoPicture => {
            eprintln!("no picture in {}", path.display());

            ExitCode::FAILURE
        }
    }
}

fn draw(path: &Path, rows: u32) -> Result<Outcome, Never> {
    let Ok(picture) = ascii::read(path, rows);

    let cover = match picture {
        Some(cover) => cover,
        None => return Ok(Outcome::NoPicture),
    };

    let Ok(columns) = console_core_number_conversion::index(cover.columns);

    for line in cover.cells.chunks(columns) {
        for cell in line {
            let (red, green, blue) = cell.rgb;
            print!("\x1b[1;38;2;{red};{green};{blue}m{}", cell.character);
        }

        println!("\x1b[0m");
    }

    Ok(Outcome::Picture)
}
