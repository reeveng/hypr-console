//! A picture, as characters, in the terminal.
//!
//! ```text
//! music-cover FILE [ROWS]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use console_core_never::Never;
use console_music::ascii;

const ROWS: u32 = 40;

#[derive(Debug)]
enum Hidden {
    NoFileSaid,
    NoPicture(PathBuf),
}

impl std::fmt::Display for Hidden {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Hidden::NoFileSaid => write!(to, "music-cover FILE [ROWS]"),
            Hidden::NoPicture(path) => write!(to, "no picture in {}", path.display()),
        }
    }
}

impl Hidden {
    fn code(&self) -> Result<ExitCode, Never> {
        Ok(match self {
            Hidden::NoFileSaid => ExitCode::from(2),
            Hidden::NoPicture(_) => ExitCode::FAILURE,
        })
    }
}

fn main() -> ExitCode {
    match drawn() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");

            let Ok(code) = why.code();

            code
        }
    }
}

fn drawn() -> Result<(), Hidden> {
    let said: Vec<String> = std::env::args().skip(1).collect();

    let path = match said.first().map(PathBuf::from) {
        Some(path) => path,
        None => return Err(Hidden::NoFileSaid),
    };

    let rows = match said.get(1).map(|said| said.parse::<u32>()) {
        None => ROWS,
        Some(Ok(rows)) => rows,

        Some(Err(fault)) => {
            eprintln!("music-cover: not a number of rows: {fault}; drawing {ROWS}");
            ROWS
        }
    };

    let Ok(read) = ascii::read(&path, rows);

    let cover = match read {
        Some(cover) => cover,
        None => return Err(Hidden::NoPicture(path)),
    };

    let Ok(cols) = console_core_number_conversion::index(cover.cols);

    for line in cover.cells.chunks(cols) {
        for cell in line {
            let (r, g, b) = cell.rgb;
            print!("\x1b[1;38;2;{r};{g};{b}m{}", cell.ch);
        }

        println!("\x1b[0m");
    }

    Ok(())
}
