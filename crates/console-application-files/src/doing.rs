//! What can be done with one thing, and what is being carried.

use std::path::PathBuf;

use console_core_never::Never;
use console_core_words::Words;

use crate::listing::{Entry, Still};
use crate::paths;
use crate::unzipping::Packed;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum FileAction {
  #[words(says = "Copy")]
  Copy,
  #[words(says = "Delete")]
  Delete,
  #[words(says = "Edit")]
  Edit,
  #[words(says = "Edit with")]
  EditWith,
  #[words(says = "Move")]
  Move,
  #[words(says = "Open")]
  Open,
  #[words(says = "Open with")]
  OpenWith,
  #[words(says = "Rename")]
  Rename,
  #[words(says = "Select")]
  Select,
  #[words(says = "Send to device")]
  SendToDevice,
  #[words(says = "Unzip")]
  Unzip,
  #[words(says = "Use as wallpaper")]
  Wallpaper,
}

impl FileAction {
  pub fn wants(self) -> Result<Wants, Never> {
    Ok(match self {
      FileAction::Wallpaper => Wants::APicture,
      FileAction::Unzip => Wants::AnArchive,
      FileAction::Edit | FileAction::EditWith => Wants::AnEditor,
      FileAction::Open | FileAction::OpenWith | FileAction::SendToDevice => Wants::AFile,
      FileAction::Copy
      | FileAction::Delete
      | FileAction::Move
      | FileAction::Rename
      | FileAction::Select => Wants::Anything,
    })
  }

  pub fn asks(self) -> Result<Asks, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self == FileAction::Delete, ()) {
        ::core::option::Option::Some(()) => Asks::First,
        ::core::option::Option::None => Asks::None,
      },
    )
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wants {
  APicture,
  AnArchive,
  AnEditor,
  AFile,
  Anything,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asks {
  First,
  None,
}

pub fn ways(entry: &Entry) -> Result<Vec<FileAction>, Never> {
  let still = entry.a_picture()?;
  let packed = entry.an_archive()?;
  let editor = console_default_applications::editors::Kind::of(&entry.kind)?;
  let editable = editor.is_some();
  let mut ways: Vec<FileAction> = Vec::new();

  for deed in EVERY {
    let wants = deed.wants()?;
    let allowed = (!entry.folder || wants == Wants::Anything)
      && (wants != Wants::APicture || still == Still::APicture)
      && (wants != Wants::AnArchive || packed == Packed::AnArchive)
      && (wants != Wants::AnEditor || editable);

    match ::core::primitive::bool::then_some(allowed, ()) {
      ::core::option::Option::Some(()) => ways.push(deed),
      ::core::option::Option::None => {}
    }
  }

  Ok(ways)
}

pub const EVERY: [FileAction; 12] = [
  FileAction::Open,
  FileAction::OpenWith,
  FileAction::Edit,
  FileAction::EditWith,
  FileAction::SendToDevice,
  FileAction::Unzip,
  FileAction::Select,
  FileAction::Rename,
  FileAction::Copy,
  FileAction::Move,
  FileAction::Wallpaper,
  FileAction::Delete,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramUse {
  Open,
  Edit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Carrying {
  ToMove,
  ToCopy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Holding {
  pub name: String,
  pub paths: Vec<PathBuf>,
  pub moving: Carrying,
}

impl Holding {
  pub fn of(entry: &Entry, path: PathBuf, moving: Carrying) -> Result<Self, Never> {
    Ok(Holding {
      name: entry.name.clone(),
      paths: vec![path],
      moving,
    })
  }

  pub fn many(paths: Vec<PathBuf>, moving: Carrying) -> Result<Self, Never> {
    let Ok(name) = items(&paths);

    Ok(Holding {
      name,
      paths,
      moving,
    })
  }

  pub fn says(&self) -> Result<String, Never> {
    let word = match self.moving {
      Carrying::ToMove => "Move",
      Carrying::ToCopy => "Paste",
    };

    Ok(format!("{word} {} Here", self.name))
  }
}

pub fn items(paths: &[PathBuf]) -> Result<String, Never> {
  match paths {
    [one] => paths::display_name(one),
    [] | [_, _, ..] => Ok(format!("{} Items", paths.len())),
  }
}

pub const SURE: &str = "Delete this?";

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  fn file(name: &str) -> Result<Entry, Never> {
    let Ok(entry) = Entry::file(name, 1);

    Ok(entry)
  }

  fn folder(name: &str) -> Result<Entry, Never> {
    let Ok(entry) = Entry::folder(name);

    Ok(entry)
  }

  fn of_kind(entry: Entry, kind: &str) -> Result<Entry, Never> {
    let Ok(entry_2) = entry.of_kind(kind);

    Ok(entry_2)
  }

  fn held(name: &str, moving: Carrying) -> Result<Holding, paths::InvalidName> {
    let child = paths::ChildName::new(name)?;
    let Ok(file) = file(name);
    let Ok(holding) = Holding::of(&file, Path::new("/home/ada").join(child), moving);

    Ok(holding)
  }

  fn open_to(entry: &Entry) -> Result<Vec<FileAction>, Never> {
    let Ok(ways) = ways(entry);

    Ok(ways)
  }

  fn word(deed: FileAction) -> Result<&'static str, Never> {
    let Ok(says) = deed.says();

    Ok(says)
  }

  #[test]
  fn a_file_can_be_opened_and_a_folder_is_walked_into() {
    let Ok(beach_jpg) = file("beach.jpg");
    let Ok(file_ways) = open_to(&beach_jpg);

    assert!(file_ways.contains(&FileAction::Open));
    assert!(file_ways.contains(&FileAction::SendToDevice));

    let Ok(holiday) = folder("Holiday");
    let Ok(folder_ways) = open_to(&holiday);

    assert!(!folder_ways.contains(&FileAction::Open));
    assert!(!folder_ways.contains(&FileAction::SendToDevice));

    let Ok(folder) = folder("Holiday");
    let Ok(other_folder_ways) = open_to(&folder);

    assert!(!other_folder_ways.contains(&FileAction::OpenWith));
  }

  #[test]
  fn a_folder_can_be_renamed_carried_and_thrown_away_like_anything_else() {
    for deed in [
      FileAction::Copy,
      FileAction::Delete,
      FileAction::Move,
      FileAction::Rename,
    ] {
      let Ok(holiday) = folder("Holiday");
      let Ok(ways) = open_to(&holiday);
      let Ok(word) = word(deed);

      assert!(ways.contains(&deed), "{}", word);
    }
  }

  #[test]
  fn open_is_the_first_way_and_delete_is_the_last() {
    let Ok(beach_jpg) = file("beach.jpg");
    let Ok(ways) = open_to(&beach_jpg);

    assert_eq!(ways.first(), Some(&FileAction::Open));
    assert_eq!(ways.last(), Some(&FileAction::Delete));
  }

  #[test]
  fn editable_files_offer_open_and_edit_independently() {
    for (name, mime) in [("beach.jpg", "image/jpeg"), ("index.html", "text/html")] {
      let Ok(entry) = file(name);
      let Ok(entry_2) = entry.of_kind(mime);
      let Ok(ways) = ways(&entry_2);

      assert!(ways.contains(&FileAction::Open));
      assert!(ways.contains(&FileAction::Edit));
      assert!(ways.contains(&FileAction::EditWith));
    }

    let Ok(entry) = file("archive.zip");
    let Ok(entry_3) = entry.of_kind("application/zip");
    let Ok(ways) = ways(&entry_3);

    assert!(!ways.contains(&FileAction::Edit));
    assert!(!ways.contains(&FileAction::EditWith));
  }

  #[test]
  fn only_a_picture_can_be_made_the_wallpaper() {
    let Ok(beach_jpg) = file("beach.jpg");
    let Ok(photograph) = of_kind(beach_jpg, "image/jpeg");
    let Ok(beach_mp4) = file("beach.mp4");
    let Ok(film) = of_kind(beach_mp4, "video/mp4");
    let Ok(photo_ways) = open_to(&photograph);

    assert!(photo_ways.contains(&FileAction::Wallpaper));

    let Ok(film_ways) = open_to(&film);

    assert!(!film_ways.contains(&FileAction::Wallpaper));

    let Ok(notes_txt) = file("notes.txt");
    let Ok(text_ways) = open_to(&notes_txt);

    assert!(!text_ways.contains(&FileAction::Wallpaper));

    let Ok(holiday) = folder("Holiday");
    let Ok(folder_ways) = open_to(&holiday);

    assert!(!folder_ways.contains(&FileAction::Wallpaper));
  }

  #[test]
  fn only_an_archive_is_offered_the_way_out_of_one() {
    let Ok(wickedwhims_zip) = file("WickedWhims.zip");
    let Ok(mod_) = of_kind(wickedwhims_zip, "application/zip");
    let Ok(beach_jpg) = file("beach.jpg");
    let Ok(photograph) = of_kind(beach_jpg, "image/jpeg");
    let Ok(archive_ways) = open_to(&mod_);

    assert!(archive_ways.contains(&FileAction::Unzip));

    let Ok(photo_ways) = open_to(&photograph);

    assert!(!photo_ways.contains(&FileAction::Unzip));

    let Ok(notes_txt) = file("notes.txt");
    let Ok(text_ways) = open_to(&notes_txt);

    assert!(!text_ways.contains(&FileAction::Unzip));

    let Ok(mods) = folder("Mods");
    let Ok(folder_ways) = open_to(&mods);

    assert!(!folder_ways.contains(&FileAction::Unzip));
  }

  #[test]
  fn unzip_is_offered_before_the_ways_of_carrying_a_thing_about() {
    let Ok(wickedwhims_zip) = file("WickedWhims.zip");
    let Ok(mod_) = of_kind(wickedwhims_zip, "application/zip");
    let Ok(ways) = open_to(&mod_);

    assert!(
      ways.contains(&FileAction::Unzip),
      "an archive offers no way out of itself"
    );
    assert!(
      ways.contains(&FileAction::Rename),
      "an archive cannot be renamed"
    );
    assert!(
      ways
        .iter()
        .take_while(|deed| **deed != FileAction::Rename)
        .any(|deed| *deed == FileAction::Unzip),
      "unzipping is offered after the ways of carrying it about"
    );
  }

  #[test]
  fn only_throwing_something_away_is_asked_about() {
    assert_eq!(FileAction::Delete.asks(), Ok(Asks::First));

    for deed in [
      FileAction::Copy,
      FileAction::Edit,
      FileAction::EditWith,
      FileAction::Move,
      FileAction::Open,
      FileAction::OpenWith,
      FileAction::Rename,
      FileAction::Select,
      FileAction::SendToDevice,
      FileAction::Unzip,
    ] {
      let Ok(word) = word(deed);

      assert_eq!(deed.asks(), Ok(Asks::None), "{} asks and should not", word);
    }
  }

  #[test]
  fn what_is_held_says_which_of_the_two_things_it_is_waiting_to_do()
  -> Result<(), paths::InvalidName> {
    let beach_jpg = held("beach.jpg", Carrying::ToCopy)?;

    assert_eq!(beach_jpg.says(), Ok("Paste beach.jpg Here".to_string()));

    let moving = held("beach.jpg", Carrying::ToMove)?;

    assert_eq!(moving.says(), Ok("Move beach.jpg Here".to_string()));

    Ok(())
  }

  #[test]
  fn many_things_carried_are_said_as_a_count_and_one_as_its_name() {
    let paths = vec![PathBuf::from("/p/beach.jpg"), PathBuf::from("/p/dune.jpg")];
    let Ok(many) = Holding::many(paths, Carrying::ToCopy);
    let Ok(one) = Holding::many(vec![PathBuf::from("/p/beach.jpg")], Carrying::ToMove);

    assert_eq!(many.says(), Ok("Paste 2 Items Here".to_string()));
    assert_eq!(one.says(), Ok("Move beach.jpg Here".to_string()));
  }

  #[test]
  fn the_question_is_a_sentence_and_carries_no_name() {
    assert!(SURE.ends_with('?'));
    assert!(!SURE.contains('{'));
  }

  #[test]
  fn the_question_asks_in_the_word_the_row_that_raised_it_said() {
    let Ok(says) = FileAction::Delete.says();

    assert!(
      SURE.to_lowercase().contains(&says.to_lowercase()),
      "{SURE:?} asks about something the row called {says:?}"
    );
  }
}
