use console_core_font_configuration::{Bases, Error, FontConfig};
use console_core_never::Never;
use std::path::PathBuf;

fn fonts_under(dirs: Vec<PathBuf>) -> Result<Vec<PathBuf>, Never> {
  enum Found {
    Directory,
    Font,
  }

  let mut fonts = Vec::new();
  let mut unread = dirs;

  'over_directories: for _visit in 0..u32::MAX {
    let dir = match unread.pop() {
      Some(dir) => dir,
      None => break 'over_directories,
    };
    let entries = match std::fs::read_dir(&dir) {
      Ok(entries) => entries,
      Err(_unreadable) => continue 'over_directories,
    };

    'over_entries: for entry in entries.flatten() {
      let kind = match entry.file_type() {
        Ok(kind) => kind,
        Err(_unreadable) => continue 'over_entries,
      };

      let found = kind
        .is_dir()
        .then_some(Found::Directory)
        .or_else(|| (kind.is_file() || kind.is_symlink()).then_some(Found::Font));

      match found {
        Some(Found::Directory) => unread.push(entry.path()),
        Some(Found::Font) => fonts.push(entry.path()),
        None => {}
      }
    }
  }

  Ok(fonts)
}

fn main() -> Result<(), Error> {
  let mut config = FontConfig::default();
  config.merge_config("/etc/fonts/fonts.conf", &Bases::default())?;

  println!("dirs: {:#?}", config.dirs);

  let Ok(fonts) = fonts_under(config.dirs.into_iter().map(|dir| dir.path).collect());

  println!("Find all {} fonts!", fonts.len());
  println!("fonts: {fonts:#?}");

  Ok(())
}
