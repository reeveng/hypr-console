//! Resolve a desktop entry and prepare it for launch.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;
use console_core_ini_files::{Key, Under, field};
use console_core_never::Never;

use crate::{entry::DesktopEntry, words};

pub fn find_desktop_file(desktop: &str, roots: &[PathBuf]) -> Result<Option<PathBuf>, Never> {
  let name = Path::new(desktop);
  let filename = match name.file_name().filter(|part| *part == name.as_os_str()) {
    Some(filename) => DesktopFile(filename),
    None => return Ok(None),
  };

  Ok(
    roots
      .iter()
      .map(|root| root.join(&filename))
      .find(|candidate| {
        candidate
          .metadata()
          .is_ok_and(|metadata| metadata.is_file())
      }),
  )
}

struct DesktopFile<'a>(&'a std::ffi::OsStr);

impl AsRef<Path> for DesktopFile<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

pub fn entry(at: &Path) -> Result<Command, Never> {
  console_gio::launch(at)
}

pub fn action(at: &Path, names: &[&str]) -> std::io::Result<Option<Vec<String>>> {
  let said = std::fs::read_to_string(at)?;
  let Ok(actions) = field(&said, Under("Desktop Entry"), Key("Actions"));
  let wanted: BTreeSet<String> = names.iter().map(|name| name.to_ascii_lowercase()).collect();
  let selected = actions
    .into_iter()
    .flat_map(|listed| listed.split(';'))
    .find(|id| wanted.contains(&id.to_ascii_lowercase()));
  let id = match selected {
    Some(id) => id,
    None => return Ok(None),
  };

  let group = format!("Desktop Action {id}");
  let Ok(exec) = field(&said, Under(&group), Key("Exec"));
  let command = exec.ok_or_else(|| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!("desktop action {id} has no Exec command"),
    )
  })?;
  let Ok(stripped) = words::without_field_codes(command);
  let Ok(parsed) = words::split(&stripped);
  let arguments = parsed.filter(|words| !words.is_empty()).ok_or_else(|| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!("invalid Exec command for desktop action {id}"),
    )
  })?;

  Ok(Some(arguments))
}

pub fn for_file(at: &Path, path: &str) -> std::io::Result<Vec<String>> {
  let said = std::fs::read_to_string(at)?;
  let Ok(entry) = DesktopEntry::read(&said);
  let command = entry
    .command
    .filter(|command| !command.is_empty())
    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no Exec command"))?;
  let Ok(parsed) = words::split(command);
  let parsed_2 = parsed.ok_or_else(|| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!("invalid Exec command {command:?}"),
    )
  })?;
  let Ok(arguments) = file_arguments(parsed_2, path);

  let Ok(ready) = match entry.terminal.map(str::to_ascii_lowercase).as_deref() {
    Some("true") => in_terminal(arguments),
    Some(_) | None => Ok(arguments),
  };

  Ok(ready)
}

pub(crate) fn in_terminal(arguments: Vec<String>) -> Result<Vec<String>, Never> {
  let Ok(mut terminal) = Program::Alacritty.arguments(&["-e"]);

  terminal.extend(arguments);

  Ok(terminal)
}

const FILE_FIELDS: [&str; 4] = ["%f", "%F", "%u", "%U"];

fn file_arguments(mut words: Vec<String>, path: &str) -> Result<Vec<String>, Never> {
  let mut fields = words
    .iter_mut()
    .filter(|word| FILE_FIELDS.contains(&word.as_str()))
    .peekable();

  match fields.peek() {
    Some(_) => fields.for_each(|field| *field = path.to_owned()),
    None => words.push(path.to_owned()),
  }

  Ok(words)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_file_goes_where_the_entry_says_it_goes() -> Result<(), Never> {
    let image = ["gimp", "%U"].map(str::to_owned).to_vec();
    let film = ["mpv", "--fullscreen", "%f"].map(str::to_owned).to_vec();

    let Ok(image_arguments) = file_arguments(image, "/home/ada/beach.jpg");
    let Ok(film_arguments) = file_arguments(film, "/x/film.mkv");

    assert_eq!(image_arguments, ["gimp", "/home/ada/beach.jpg"]);
    assert_eq!(film_arguments, ["mpv", "--fullscreen", "/x/film.mkv"]);

    Ok(())
  }

  #[test]
  fn an_entry_without_a_file_field_gets_the_file_at_the_end() -> Result<(), Never> {
    let arguments = vec![String::from("librewolf")];

    let Ok(arguments_with_file) = file_arguments(arguments, "/x/page.html");
    assert_eq!(arguments_with_file, ["librewolf", "/x/page.html"]);

    Ok(())
  }

  #[test]
  fn a_word_that_only_looks_like_a_code_is_left_alone() -> Result<(), Never> {
    let arguments = ["run", "--rate=%50", "%f"].map(str::to_owned).to_vec();

    let Ok(arguments_with_file) = file_arguments(arguments, "/x/y");
    assert_eq!(arguments_with_file, ["run", "--rate=%50", "/x/y"]);

    Ok(())
  }
}
