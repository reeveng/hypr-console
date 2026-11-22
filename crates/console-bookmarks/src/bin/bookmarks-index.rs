//! What the browser says its bookmarks are, written where the menu looks.
//!
//! Read on the standard input rather than asked for, because the only thing
//! that can answer is the browser and the browser is what starts this: the
//! add-on gathers them and hands them over the privileged door in `around.js`,
//! one line per bookmark. Nothing here opens a browser, a profile or a
//! database, and a machine with no browser on it runs this never.

use console_bookmarks::Where;

const WHO: &str = "bookmarks-index";

const BOOKMARK_BYTES: u32 = 16 * 1024 * 1024;

fn main() {
  let said = match console_core_bounded_reads::text(std::io::stdin().lock(), BOOKMARK_BYTES) {
    Ok(said) => said,
    Err(fault) => {
      eprintln!("{WHO}: reading what the browser said: {fault}");

      return;
    }
  };

  let Ok(bookmarks) = console_bookmarks::read(&said);
  let Ok(bookmarks_2) = console_bookmarks::apart(bookmarks);
  let Ok(among) = console_bookmarks::among();
  let Ok(icons) = console_bookmarks::icons();

  let (among_3, icons_3) = match (among, icons) {
    (Some(among_2), Some(icons_2)) => (among_2, icons_2),
    (None, _) | (_, None) => {
      eprintln!("{WHO}: no home to keep them in; leaving the menu as it is");

      return;
    }
  };

  match console_bookmarks::keep(
    Where {
      among: &among_3,
      icons: &icons_3,
    },
    &bookmarks_2,
  ) {
    Ok(()) => {}
    Err(fault) => eprintln!("{WHO}: {fault}"),
  }
}
