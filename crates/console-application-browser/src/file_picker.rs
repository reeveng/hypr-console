//! A page can receive a file only after the user selects one in this view.

use std::fs::{self, FileType};
use std::io;
use std::path::{Path, PathBuf};

use console_core_color::palette::Wearing;
use console_core_fonts::Font;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_never::Never;
use console_core_shapes::Edge;
use console_draw_painting::{self as drawing, Frame};
use console_input_key_symbols::Keysym;

use crate::settings::{Alignment, label_at};
use crate::toolbar::panel;

const ROW: u32 = 32;
const HEADER: u32 = 64;

struct Entry {
  path: PathBuf,
  label: String,
  kind: EntryKind,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EntryKind {
  File,
  Folder,
}

impl EntryKind {
  #[must_use]
  fn from_file_type(
    kind: FileType,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match (kind.is_dir(), kind.is_file()) {
      (true, _) => Some(Self::Folder),
      (false, true) => Some(Self::File),
      (false, false) => None,
    })
  }
}

pub(crate) enum Choice {
  Waiting,
  Cancelled,
  PathEntry,
  Selected(PathBuf),
}

pub(crate) struct PickRequest {
  pub tab: u32,
  pub chooser: u64,
}

pub(crate) struct Picker {
  request: PickRequest,
  folder: PathBuf,
  entries: Vec<Entry>,
  selected: u32,
  offset: u32,
}

pub(crate) struct VerticalOffset(pub u32);

fn displayed_name(name: &std::ffi::OsStr) -> Result<String, Never> {
  Ok(match name.to_str() {
    Some(text) => text.to_owned(),
    None => format!("{name:?}"),
  })
}

fn list(folder: &Path) -> io::Result<Vec<Entry>> {
  let directory = fs::read_dir(folder)?;
  let mut entries = Vec::new();

  for read in directory.take(4096) {
    let item = match read {
      Ok(item) => item,
      Err(_unreadable_entry) => continue,
    };
    let path = item.path();
    let kind = match item.file_type() {
      Ok(kind) => kind,
      Err(_unreadable_type) => continue,
    };
    let target = match bool::then_some(kind.is_symlink(), ()) {
      Some(()) => match fs::metadata(&path) {
        Ok(metadata) => metadata.file_type(),
        Err(_unreadable_target) => continue,
      },
      None => kind,
    };
    let Ok(entry_kind) = EntryKind::from_file_type(target);
    let file_kind = match entry_kind {
      Some(kind_2) => kind_2,
      None => continue,
    };
    let Ok(mut label) = displayed_name(&item.file_name());

    match file_kind {
      EntryKind::Folder => label.push('/'),
      EntryKind::File => {}
    }

    entries.push(Entry {
      path,
      label,
      kind: file_kind,
    });
  }

  entries.sort_by(|left, right| {
    right
      .kind
      .cmp(&left.kind)
      .then(left.label.cmp(&right.label))
  });

  match folder.parent() {
    Some(parent) => {
      entries.insert(
        0,
        Entry {
          path: parent.to_owned(),
          label: String::from("Up one folder"),
          kind: EntryKind::Folder,
        },
      );
    }
    None => {}
  }

  Ok(entries)
}

impl Picker {
  pub(crate) fn new(tab: u32, chooser: u64) -> io::Result<Self> {
    let Ok(downloads) = console_core_places::Folder::Downloads.user();
    let download_folder = match downloads {
      Some(folder) => match list(&folder) {
        Ok(entries) => Some((folder, entries)),
        Err(fault) => match matches!(
          fault.kind(),
          io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        )
        .then_some(())
        {
          Some(()) => None,
          None => return Err(fault),
        },
      },
      None => None,
    };
    let (folder, entries) = match download_folder {
      Some(opened) => opened,
      None => {
        let Ok(home) = console_core_places::home();
        let folder = home.ok_or_else(|| io::Error::other("no folder for the upload chooser"))?;
        let entries = list(&folder)?;
        (folder, entries)
      }
    };
    Ok(Self {
      request: PickRequest { tab, chooser },
      folder,
      entries,
      selected: 0,
      offset: 0,
    })
  }

  pub(crate) fn request(&self) -> Result<&PickRequest, Never> {
    Ok(&self.request)
  }

  fn rows(height: u32) -> Result<u32, Never> {
    Ok(height.saturating_sub(HEADER).div_euclid(ROW).max(1))
  }

  fn show_selected(&mut self, height: u32) -> Result<(), Never> {
    let Ok(rows) = Self::rows(height);
    let earliest = self.selected.saturating_add(1).saturating_sub(rows);
    self.offset = self.offset.clamp(earliest, self.selected);
    Ok(())
  }

  pub(crate) fn key(&mut self, key: Keysym, height: u32) -> io::Result<Choice> {
    let Ok(count) = console_core_number_conversion::fitted::<_, u32>(self.entries.len());
    let last = count.saturating_sub(1);
    let Ok(page) = Self::rows(height);

    match key {
      Keysym::Escape => return Ok(Choice::Cancelled),
      Keysym::Up => self.selected = self.selected.saturating_sub(1),
      Keysym::Down => self.selected = self.selected.saturating_add(1).min(last),
      Keysym::Page_Up => self.selected = self.selected.saturating_sub(page),
      Keysym::Page_Down => self.selected = self.selected.saturating_add(page).min(last),
      Keysym::Return | Keysym::KP_Enter => return self.enter(),
      _ => {}
    }

    let Ok(()) = self.show_selected(height);
    Ok(Choice::Waiting)
  }

  pub(crate) fn click(
    &mut self,
    VerticalOffset(y): VerticalOffset,
    height: u32,
  ) -> io::Result<Choice> {
    match (y < HEADER).then_some(()) {
      Some(()) => return Ok(Choice::PathEntry),
      None => {}
    }

    let row = self
      .offset
      .saturating_add(y.saturating_sub(HEADER).div_euclid(ROW));
    let Ok(index) = console_core_number_conversion::index(row);

    match self.entries.get(index) {
      Some(_entry) => {
        self.selected = row;
        let Ok(()) = self.show_selected(height);
        self.enter()
      }
      None => Ok(Choice::Waiting),
    }
  }

  fn enter(&mut self) -> io::Result<Choice> {
    let Ok(index) = console_core_number_conversion::index(self.selected);
    let entry = match self.entries.get(index) {
      Some(entry) => entry,
      None => return Ok(Choice::Waiting),
    };

    match entry.kind {
      EntryKind::File => return Ok(Choice::Selected(entry.path.clone())),
      EntryKind::Folder => {}
    }

    let entries = list(&entry.path)?;
    self.folder = entry.path.clone();
    self.entries = entries;
    self.selected = 0;
    self.offset = 0;
    Ok(Choice::Waiting)
  }

  pub(crate) fn paint(
    &self,
    pixels: &mut [u8],
    frame: Frame,
    palette: &Wearing,
    font: &Font,
  ) -> Result<(), drawing::Cannot> {
    let Ok(width) = console_core_number_conversion::fitted::<_, i32>(frame.points.width);
    let Ok(height) = console_core_number_conversion::fitted::<_, i32>(frame.points.height);
    let area = |y, height_2| Rectangle {
      origin: Point { x: 0, y },
      size: Size {
        width,
        height: height_2,
      },
    };
    let Ok(background) = panel(area(0, height), 0, palette.night, Edge::None);
    let Ok(heading) = panel(area(0, 64), 0, palette.panel, Edge::None);
    let mut shapes = vec![background, heading];
    let Ok(folder) = displayed_name(self.folder.as_os_str());
    let Ok(()) = label_at(
      &mut shapes,
      area(0, 32),
      &format!("Upload file · {folder}"),
      font,
      palette.text,
      Alignment::Start,
    );
    let Ok(()) = label_at(
      &mut shapes,
      area(32, 32),
      "Enter path · Ctrl+L    Cancel · Esc",
      font,
      palette.soft,
      Alignment::Start,
    );

    let Ok(start) = console_core_number_conversion::index(self.offset);
    let Ok(rows) = Self::rows(frame.points.height);
    let Ok(count) = console_core_number_conversion::index(rows);
    let visible = self.entries.iter().skip(start).take(count);

    for (index, entry) in (self.offset..).zip(visible) {
      let row = index
        .saturating_sub(self.offset)
        .saturating_mul(ROW)
        .saturating_add(HEADER);
      let Ok(y) = console_core_number_conversion::fitted::<_, i32>(row);
      let bounds = area(y, 32);

      match { ::core::primitive::bool::then_some(index == self.selected, ()) } {
        ::core::option::Option::Some(()) => {
          let Ok(selected) = panel(bounds, 0, palette.ground, Edge::None);
          shapes.push(selected);
        }
        ::core::option::Option::None => {}
      }

      let Ok(()) = label_at(
        &mut shapes,
        bounds,
        &entry.label,
        font,
        palette.text,
        Alignment::Start,
      );
    }

    drawing::onto(pixels, frame, &shapes)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::downloads::Staging;
  use std::os::unix::fs::symlink;

  #[test]
  fn the_picker_lists_link_targets_by_type_and_ignores_broken_links() -> io::Result<()> {
    let fixture = Staging::temporary("browser-file-picker")?;
    let parent = fixture.as_ref();
    let folder = parent.join("picker");
    fs::create_dir(&folder)?;
    let directory = parent.join("directory");
    fs::create_dir(&directory)?;
    let file = parent.join("file");
    console_core_atomic_writes::whole(&file, b"")
      .map_err(|fault| io::Error::other(fault.to_string()))?;
    symlink(&directory, folder.join("z-directory"))?;
    symlink(&file, folder.join("a-file"))?;
    symlink(parent.join("missing"), folder.join("dangling"))?;

    let entries = list(&folder)?;
    let labels: Vec<_> = entries.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(labels, ["Up one folder", "z-directory/", "a-file"]);
    assert!(matches!(
      entries.as_slice(),
      [
        _,
        Entry {
          kind: EntryKind::Folder,
          ..
        },
        Entry {
          kind: EntryKind::File,
          ..
        }
      ]
    ));
    Ok(())
  }
}
