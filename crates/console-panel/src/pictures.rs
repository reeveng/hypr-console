//! Every picture a list wants, in one file, already the size it is drawn at.
//!
//! A row keeps a square at its front and most rows have something to put in it:
//! an application's icon, a photograph's thumbnail. Opening one is a file
//! opened, a format worked out and an image decoded, and the menu does sixty of
//! them between the card existing and the card being on the screen -- on the
//! loop that draws, because that is where the rows are built. Nearly all of
//! them are SVG, which is a parser and a rasteriser per row.
//!
//! It was the slowest part of opening a panel by a long way, and none of the
//! work was new: the same sixty icons, at the same size, every time anyone
//! opened the menu.
//!
//! So they are decoded once, into one file, at the size a row draws them. An
//! opening reads that file once and hands out slices of it. No format is worked
//! out, nothing is scaled, and sixty opens become one.
//!
//! ## The size is half of what a picture is called
//!
//! A row draws an icon at the size a row is, and the home screen draws the
//! same icon at a third of the width of the screen. One store holding one size
//! answers the second of those with the first one's picture scaled up, which is
//! the blurred square this was written to avoid -- and a second store beside it
//! is two files, two makers and two stale answers. So what a picture is called
//! here is the file it was made from and the side it was made at, and a size
//! nobody has asked for yet is simply a picture that is not in the store. The
//! whole of that is [`keyed`], and it is why the store carries a version: the
//! first one's names were paths, and a path is now half a name.
//!
//! ## Why raw pixels and not sixty small PNGs
//!
//! Sixty PNGs is sixty opens and sixty decodes, which is the same shape as the
//! fault, cheaper. What is wanted here is one read: the pictures are small at
//! this size -- a square of thirty-two is four kilobytes -- and the whole store
//! is smaller than the icon index the menu already reads.
//!
//! ## What keeps it honest
//!
//! ## Who reads it
//!
//! Two drawings now: the panel on GTK, which wants a texture, and the surface
//! this desktop draws itself, which wants the bytes. `ready` and `pixels` are
//! the same lookup ending in the two answers, and neither copies more than the
//! one picture asked for. What a surface must never do is decode: the store is
//! a file read once, and a picture that is not in it is asked for from the
//! maker and drawn on the next frame rather than in the middle of this one.
//!
//! ## Read again when it has been written again
//!
//! The store used to be read once per process and that was true for as long as
//! a panel was a process: it came up, read what was there, drew and went away.
//! `console-panels` holds every panel now, so the process that reads the store
//! is the one that was already running when the maker wrote it -- a menu opened
//! for the first time asks for sixty pictures, and the opening after it drew
//! none of them, because what the first opening had cached was the absence.
//! So what is held is the file's own stamp beside the bytes, and a store
//! written since is read again. Nothing above here knows the file was read
//! twice.
//!
//! It is a cache, under `~/.cache/console`, and it says what it was made from:
//! a picture is found by the path it was made out of, and the store is rebuilt
//! when the list it was made for is newer than it is. A picture that is not in
//! it is opened the old way, so a store that is missing, stale in part, or
//! written by an older version of this is slower and never wrong.

use console_core_arguments::{Flag, Takes};
use console_core_geometry::Size;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_places::Base;
use console_operating_system_linux::program_runtime::commands::{Linux, Timing};
use console_program_contract::{Command, Executable};
use console_program_runtime::commands::{self as execution_commands, Launch, Streams};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

use console_core_shapes::Pixels;

pub const MAGIC: &[u8] = b"panel-pictures 2\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Side(pub u32);

pub fn key(of: &str, side: Side) -> Result<String, Never> {
  Ok(format!("{} {of}", side.0))
}

pub fn parse_key(key: &str) -> Result<Option<(&str, Side)>, Never> {
  let (side, of) = match key.split_once(' ') {
    Some((side, of)) => (side, of),
    None => return Ok(None),
  };

  Ok(match side.parse() {
    Ok(side_2) => Some((of, Side(side_2))),
    Err(_not_a_size) => None,
  })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
  pub of: String,
  pub pixels: Pixels,
}

pub const SIDE: Flag = Flag {
  spelling: "--side",
  takes: Takes::Value("PIXELS"),
  about: "the side the caller draws at, the rows' if none is said",
};

pub fn store() -> Result<Option<PathBuf>, Never> {
  let ours = Base::Cache.ours()?;

  Ok(ours.map(|ours_2| ours_2.join("pictures")))
}

pub fn serialize(pictures: &[Picture]) -> Result<Vec<u8>, Never> {
  let mut head: Vec<u8> = Vec::new();
  let mut body: Vec<u8> = Vec::new();
  let Ok(many) = fitted::<_, u32>(pictures.len());
  head.extend_from_slice(MAGIC);
  head.extend_from_slice(&many.to_le_bytes());

  for picture in pictures {
    let name = picture.of.as_bytes();
    let Ok(called) = fitted::<_, u32>(name.len());
    head.extend_from_slice(&called.to_le_bytes());
    head.extend_from_slice(name);

    let Ok(at) = fitted(body.len());
    let Ok(long) = fitted(picture.pixels.bytes.len());

    for number in [
      picture.pixels.width,
      picture.pixels.height,
      picture.pixels.stride,
      at,
      long,
    ] {
      head.extend_from_slice(&number.to_le_bytes());
    }

    body.extend_from_slice(&picture.pixels.bytes);
  }

  head.extend_from_slice(&body);

  Ok(head)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
  pub width: u32,
  pub height: u32,
  pub stride: u32,
  pub at: u64,
  pub long: u64,
}

impl Where {
  pub fn in_store<'a>(&self, bytes: &'a [u8]) -> Result<Option<&'a [u8]>, Never> {
    let Ok(from) = index(self.at);
    let Ok(end) = index(self.at.saturating_add(self.long));

    Ok(bytes.get(from..end))
  }

  pub fn pixels(&self, bytes: &[u8]) -> Result<Option<Pixels>, Never> {
    let Ok(held) = self.in_store(bytes);

    Ok(held.map(|held_2| Pixels {
      width: self.width,
      height: self.height,
      stride: self.stride,
      bytes: held_2.to_vec(),
    }))
  }
}

pub fn read(bytes: &[u8]) -> Result<Option<BTreeMap<String, Where>>, Never> {
  let after = match bytes.strip_prefix(MAGIC) {
    Some(after) => after,
    None => return Ok(None),
  };

  let (count, mut rest) = match number(after) {
    Ok(Some((count, rest))) => (count, rest),
    Ok(None) | Err(_) => return Ok(None),
  };

  let mut held: BTreeMap<String, Where> = BTreeMap::new();
  let mut entries = Vec::new();

  for _ in 0..count {
    let (long, after_3) = match number(rest) {
      Ok(Some((long, after_2))) => (long, after_2),
      Ok(None) | Err(_) => return Ok(None),
    };

    let Ok(long_2) = index(long);

    let (name, after_4) = match after_3.split_at_checked(long_2) {
      Some(split) => split,
      None => return Ok(None),
    };

    let of = match String::from_utf8(name.to_vec()) {
      Ok(of) => of,
      Err(_not_text) => return Ok(None),
    };

    let (wide, after_6) = match number(after_4) {
      Ok(Some((wide, after_5))) => (wide, after_5),
      Ok(None) | Err(_) => return Ok(None),
    };

    let (tall, after_8) = match number(after_6) {
      Ok(Some((tall, after_7))) => (tall, after_7),
      Ok(None) | Err(_) => return Ok(None),
    };

    let (stride, after_10) = match number(after_8) {
      Ok(Some((stride, after_9))) => (stride, after_9),
      Ok(None) | Err(_) => return Ok(None),
    };

    let (at, after_12) = match number(after_10) {
      Ok(Some((at, after_11))) => (at, after_11),
      Ok(None) | Err(_) => return Ok(None),
    };

    let (length, after_14) = match number(after_12) {
      Ok(Some((length, after_13))) => (length, after_13),
      Ok(None) | Err(_) => return Ok(None),
    };

    entries.push((of, wide, tall, stride, at, length));
    rest = after_14;
  }

  let Ok(stored) = fitted::<_, u64>(bytes.len());
  let Ok(after_the_list) = fitted::<_, u64>(rest.len());
  let began = stored.saturating_sub(after_the_list);

  for (of, wide, tall, stride, at, long) in entries {
    let long_3 = u64::from(long);

    let at_3 = match began.checked_add(u64::from(at)) {
      Some(at_2) => at_2,
      None => return Ok(None),
    };

    let end = match at_3.checked_add(long_3) {
      Some(end) => end,
      None => return Ok(None),
    };

    match ::core::primitive::bool::then_some(end > stored || long_3 == 0, ()) {
      ::core::option::Option::Some(()) => return Ok(None),
      ::core::option::Option::None => {}
    }

    let Ok(required) = Pixels::required_bytes(
      Size {
        width: wide,
        height: tall,
      },
      stride,
    );

    match required {
      Some(required_2) => match required_2.cmp(&long_3) {
        core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {}
        core::cmp::Ordering::Greater => return Ok(None),
      },
      None => return Ok(None),
    }

    held.insert(
      of,
      Where {
        width: wide,
        height: tall,
        stride,
        at: at_3,
        long: long_3,
      },
    );
  }

  Ok(Some(held))
}

type Cache = (Vec<u8>, BTreeMap<String, Where>);

struct Stored {
  written: SystemTime,
  contents: Cache,
}

fn held_store() -> Result<Option<MutexGuard<'static, Option<Stored>>>, Never> {
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
      explicit044_no_ambient_value,
      reason = "one file, read for as long as the process draws, and the process that draws is now the one holding every panel: what it read at the first opening has to be given up when the maker writes a newer store, so the holder is a lock beside the stamp it was read at and nothing above it knows the file was read twice"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "this lock owns the process picture store and lends entries to callers; the byte snapshot is replaced only after validation"
    )
  )]
  static HELD: Mutex<Option<Stored>> = Mutex::new(None);

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the store guard covers a map lookup or a memory copy; opening and reading the on-disk store occur outside the guard"
    )
  )]
  let held = HELD.lock();

  Ok(match held {
    Ok(checked) => Some(checked),
    Err(_faulted_cache) => None,
  })
}

fn with_cache<T>(
  read_cache: impl FnOnce(&Cache) -> Result<T, Never>,
) -> Result<Option<T>, Never> {
  let Ok(configured) = store();
  let path = match configured {
    Some(directory) => directory,
    None => return Ok(None),
  };
  let stamp = match std::fs::metadata(&path).and_then(|metadata| metadata.modified()) {
    Ok(written) => written,
    Err(_the_store_has_not_been_created) => return Ok(None),
  };
  let Ok(held) = held_store();
  let prior = match held {
    Some(guard) => match guard.as_ref() {
      Some(record) => match (record.written == stamp).then_some(()) {
        Some(()) => return read_cache(&record.contents).map(Some),
        None => Some(record.written),
      },
      None => None,
    },
    None => return Ok(None),
  };
  let bytes = match std::fs::read(&path) {
    Ok(read) => read,
    Err(_the_store_was_replaced) => return Ok(None),
  };
  let checked = std::fs::metadata(&path).and_then(|metadata| metadata.modified());

  match checked {
    Ok(written) => match (written == stamp).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    },
    Err(_the_store_was_removed) => return Ok(None),
  }

  let Ok(index) = read(&bytes);
  let contents = match index {
    Some(entries) => (bytes, entries),
    None => return Ok(None),
  };
  let Ok(holder) = held_store();
  let mut guard = match holder {
    Some(ready) => ready,
    None => return Ok(None),
  };
  let current = guard.as_ref().map(|record| record.written);

  match (current == prior).then_some(()) {
    Some(()) => {
      *guard = Some(Stored {
        written: stamp,
        contents,
      })
    }
    None => {}
  }

  match guard.as_ref() {
    Some(record) => read_cache(&record.contents).map(Some),
    None => Ok(None),
  }
}

pub fn a_row() -> Result<Side, Never> {
  let Ok(side) = fitted::<i32, u32>(crate::strip::PICTURE);

  Ok(Side(side))
}

pub fn pixels(of: &Path) -> Result<Option<Pixels>, Never> {
  let Ok(side) = a_row();

  pixels_at(of, side)
}

pub fn missing_pictures(wanted: &[String]) -> Result<Vec<String>, Never> {
  let Ok(side) = a_row();

  missing_at(wanted, side)
}

pub fn make(wanted: &[String]) -> Result<(), Never> {
  let Ok(side) = a_row();

  make_at(wanted, side)
}

pub fn pixels_at(of: &Path, side: Side) -> Result<Option<Pixels>, Never> {
  let named = match of.to_str() {
    Some(text) => text,
    None => return Ok(None),
  };
  let Ok(named_key) = key(named, side);
  let Ok(found) = with_cache(|(bytes, entries)| match entries.get(&named_key) {
    Some(entry) => entry.pixels(bytes),
    None => Ok(None),
  });

  Ok(found.flatten())
}

pub fn missing_at(wanted: &[String], side: Side) -> Result<Vec<String>, Never> {
  let Ok(short) = with_cache(|(_, entries)| {
    Ok(
      wanted
        .iter()
        .filter(|path| {
          let Ok(named) = key(path, side);
          !entries.contains_key(&named)
        })
        .cloned()
        .collect(),
    )
  });

  Ok(match short {
    Some(unmade) => unmade,
    None => wanted.to_vec(),
  })
}

pub fn make_at(wanted: &[String], side: Side) -> Result<(), Never> {
  #[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
      explicit044_no_ambient_value,
      reason = "what this process has already asked the maker for, so a second row wanting the same picture does not start a second maker; the rows are built in several places with nothing between them, which is the argument `opening`'s head makes about the same panel"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit068_no_global_mutable_state",
    expect(
      explicit068_no_global_mutable_state,
      reason = "the set deduplicates picture loads already in flight across all surfaces in the process"
    )
  )]
  static ASKED: std::sync::Mutex<Option<std::collections::BTreeSet<String>>> =
    std::sync::Mutex::new(None);

  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "this request set is held only while remembering which icons were submitted; the maker is launched after unlocking"
    )
  )]
  let locking = ASKED.lock();
  let mut requested = match locking {
    Ok(entries) => entries,
    Err(_the_lock_is_poisoned) => return Ok(()),
  };

  let asked = requested.get_or_insert_with(std::collections::BTreeSet::new);
  let mut fresh: Vec<String> = Vec::new();

  for of in wanted {
    let Ok(named) = key(of, side);

    match ::core::primitive::bool::then_some(asked.insert(named), ()) {
      ::core::option::Option::Some(()) => fresh.push(of.clone()),
      ::core::option::Option::None => {}
    }
  }

  drop(requested);

  let missing = fresh.as_slice();

  match ::core::primitive::bool::then_some(missing.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(()),
    ::core::option::Option::None => {}
  }

  let command = Command {
    program: Executable::Internal(InternalProgram::PanelPictures),
    arguments: [SIDE.spelling.to_string(), side.0.to_string()]
      .into_iter()
      .chain(missing.iter().cloned())
      .collect(),
  };
  let mut platform = Linux {
    timing: Timing::Background,
  };

  match execution_commands::spawn(&mut platform, &command, Streams::QUIET, Launch::Background) {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("no pictures made: {fault}"),
  }

  Ok(())
}

fn number(bytes: &[u8]) -> Result<Option<(u32, &[u8])>, Never> {
  let (four, rest) = match bytes.split_at_checked(4) {
    Some((four, rest)) => (four, rest),
    None => return Ok(None),
  };

  let four_3: [u8; 4] = match four.try_into() {
    Ok(four_2) => four_2,
    Err(_not_four_bytes) => return Ok(None),
  };

  Ok(Some((u32::from_le_bytes(four_3), rest)))
}

#[cfg(test)]
mod tests {
  use super::*;

  type Failure = Box<dyn std::error::Error>;

  const LARGE: u32 = 32 * 32 * 4;

  fn a_picture(of: &str, edge: u32) -> Result<Picture, Never> {
    let side = edge.min(32);
    let Ok(bytes) = index(side.saturating_mul(side).saturating_mul(4).min(LARGE));

    let mut picture_bytes = vec![7; 4096];
    picture_bytes.truncate(bytes);
    let Ok(pixels) = Pixels::packed(
      Size {
        width: side,
        height: side,
      },
      picture_bytes,
    );

    Ok(Picture {
      of: of.to_string(),
      pixels,
    })
  }

  #[test]
  fn a_store_says_where_each_picture_is_and_how_big() -> Result<(), Failure> {
    let Ok(one) = a_picture("/usr/share/icons/one.svg", 32);
    let Ok(two) = a_picture("/two.png", 16);
    let Ok(bytes) = serialize(&[one, two]);
    let Ok(held) = read(&bytes);
    let held_2 = held.ok_or("a written store does not read back")?;

    assert_eq!(held_2.len(), 2);

    let one_2 = held_2
      .get("/usr/share/icons/one.svg")
      .ok_or("the first picture")?;
    assert_eq!((one_2.width, one_2.height, one_2.stride), (32, 32, 128));
    assert_eq!(one_2.in_store(&bytes), Ok(Some([7u8; 4096].as_slice())));

    let two_2 = held_2.get("/two.png").ok_or("the second picture")?;

    assert_eq!(two_2.in_store(&bytes), Ok(Some([7u8; 1024].as_slice())));

    Ok(())
  }

  #[test]
  fn half_a_store_is_no_store_rather_than_half_the_pictures() {
    let Ok(one) = a_picture("/one.svg", 32);
    let Ok(bytes) = serialize(&[one]);

    for cut in [
      0,
      4,
      MAGIC.len(),
      MAGIC.len().saturating_add(4),
      bytes.len().saturating_sub(1),
    ] {
      assert_eq!(
        bytes.get(..cut).map(read),
        Some(Ok(None)),
        "a store cut at {cut} was read as a store"
      );
    }
  }

  #[test]
  fn a_file_that_is_not_this_file_is_not_read_at_all() {
    assert_eq!(read(b"not this at all"), Ok(None));
    assert_eq!(read(b"panel-pictures 2\n"), Ok(None));
  }

  #[test]
  fn a_picture_that_points_outside_the_store_is_refused() {
    let Ok(one) = a_picture("/one.svg", 32);
    let Ok(mut bytes) = serialize(&[one]);
    let far = (1_000_000u32).to_le_bytes();
    let Ok(behind) = index(LARGE.saturating_add(4));

    for (slot, byte) in bytes
      .iter_mut()
      .rev()
      .skip(behind)
      .take(4)
      .zip(far.iter().rev())
    {
      *slot = *byte;
    }

    assert_eq!(read(&bytes), Ok(None));
  }

  #[test]
  fn a_picture_whose_rows_do_not_fit_its_own_pixels_is_refused() {
    let Ok(mut picture) = a_picture("/one.svg", 32);
    picture.pixels.stride = 4096;
    let Ok(bytes) = serialize(&[picture]);

    assert_eq!(read(&bytes), Ok(None));
  }

  #[test]
  fn an_empty_store_is_a_store_with_nothing_in_it() -> Result<(), Failure> {
    let Ok(bytes) = serialize(&[]);
    let Ok(held) = read(&bytes);
    let pictures = held.ok_or("an empty store is not a store")?;

    assert!(pictures.is_empty());

    Ok(())
  }
}
