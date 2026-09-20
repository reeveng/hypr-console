//! What the browser says its bookmarks are, written where the menu looks.
//!
//! Read on the standard input rather than asked for, because the only thing
//! that can answer is the browser and the browser is what starts this: the
//! add-on gathers them and hands them over the privileged door in `around.js`,
//! one line per bookmark. Nothing here opens a browser, a profile or a
//! database, and a machine with no browser on it runs this never.

use std::io::Read;

use console_bookmarks::Where;

const WHO: &str = "bookmarks-index";

fn main() {
    let mut said = String::new();

    match std::io::stdin().read_to_string(&mut said) {
        Ok(_how_much) => {},
        Err(fault) => {
            eprintln!("{WHO}: reading what the browser said: {fault}");

            return;
        }
    }

    let Ok(bookmarks) = console_bookmarks::read(&said);
    let Ok(bookmarks) = console_bookmarks::apart(bookmarks);
    let Ok(among) = console_bookmarks::among();
    let Ok(icons) = console_bookmarks::icons();

    let (among, icons) = match (among, icons) {
        (Some(among), Some(icons)) => (among, icons),
        (None, _) | (_, None) => {
            eprintln!("{WHO}: no home to keep them in; leaving the menu as it is");

            return;
        }
    };

    match console_bookmarks::keep(Where { among: &among, icons: &icons }, &bookmarks) {
        Ok(()) => {},
        Err(fault) => eprintln!("{WHO}: {fault}"),
    }
}
