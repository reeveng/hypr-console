//! A picture, as characters, in the terminal.
//!
//! ```text
//! cover-ascii FILE [ROWS]
//! ```

use std::path::PathBuf;

use console_music::ascii;

fn main() {
    let said: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = said.first().map(PathBuf::from) else {
        eprintln!("cover-ascii FILE [ROWS]");
        std::process::exit(2);
    };
    let rows = said.get(1).and_then(|said| said.parse().ok()).unwrap_or(40);

    let Some(cover) = ascii::read(&path, rows) else {
        eprintln!("no picture in {}", path.display());
        std::process::exit(1);
    };

    for line in cover.cells.chunks(cover.cols) {
        for cell in line {
            let (r, g, b) = cell.rgb;
            print!("\x1b[1;38;2;{r};{g};{b}m{}", cell.ch);
        }
        println!("\x1b[0m");
    }
}
