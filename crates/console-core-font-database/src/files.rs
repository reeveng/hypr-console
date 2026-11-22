//! Fonts read from disk: one file, every font under a directory, and a file mapped once and shared.
//!
//! A directory is walked depth first in the order it lists, descending into a
//! directory the moment it is met, and a file is a font when its extension says
//! TrueType or OpenType, single or collection, in either case. The walk keeps a
//! stack of the directories it is inside rather than calling itself, so how
//! deep a tree goes is a length rather than a stack frame per level.
//!
//! A link is followed and the walk goes on from where it leads, under that
//! name, and nothing reached through a link is visited a second time. What has
//! been visited is only written down once the first link is met: from then on a
//! path is skipped when it was seen before, and before then nothing is, which
//! is how the upstream reader walked and so is the set of faces it found. A
//! directory or entry that cannot be read is passed over and said in the log.
//!
//! A face read from a file keeps only its path and reads the file again each time
//! its bytes are asked for. `make_shared_face_data` maps it once and hands every
//! face of that file the same mapping; the caller promises nothing changes the
//! file while any face shares it, which is the promise a mapping needs.

#[cfg(feature = "memmap")]
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs::FileType;
use std::path::{Path, PathBuf};

use console_core_iteration::Step;
use console_core_never::Never;

use crate::{Database, Emptiness, Source, faces};
#[cfg(feature = "memmap")]
use crate::{SharedFontData, shared_font_data};

pub(crate) type Seen = BTreeSet<PathBuf>;

enum EntryKind {
  File,
  Directory,
  Link,
  Other,
}

impl From<FileType> for EntryKind {
  fn from(file_type: FileType) -> Self {
    let known_kind = file_type
      .is_file()
      .then_some(Self::File)
      .or_else(|| file_type.is_dir().then_some(Self::Directory))
      .or_else(|| file_type.is_symlink().then_some(Self::Link));

    match known_kind {
      Some(kind) => kind,
      None => Self::Other,
    }
  }
}

enum Visit {
  First,
  Repeated,
}

impl From<bool> for Visit {
  fn from(first_visit: bool) -> Self {
    match bool::then_some(first_visit, ()) {
      Some(()) => Self::First,
      None => Self::Repeated,
    }
  }
}

struct Walk<'a> {
  database: &'a mut Database,
  pending: Vec<std::fs::ReadDir>,
  seen: &'a mut Seen,
}

impl Database {
  pub fn load_font_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), std::io::Error> {
    let file_path = path.as_ref();
    let data = std::fs::read(file_path)?;
    let Ok(()) = self.load_fonts_from_file(file_path, &data);

    Ok(())
  }

  pub fn load_fonts_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<(), Never> {
    let mut seen = Seen::new();

    self.load_fonts_dir_impl(dir.as_ref(), &mut seen)
  }

  fn load_fonts_from_file(&mut self, path: &Path, data: &[u8]) -> Result<(), Never> {
    let source = Source::File(PathBuf::from(path));
    let named = format_args!("'{}'", path.display());
    let Ok(read) = faces::read(&source, data, &named);

    for face in read {
      let Ok(_id) = self.push_face_info(face);
    }

    Ok(())
  }

  pub(crate) fn load_fonts_dir_impl(&mut self, dir: &Path, seen: &mut Seen) -> Result<(), Never> {
    let directory = match std::fs::read_dir(dir) {
      Ok(directory) => directory,
      Err(fault) => {
        console_core_logging::debug!("Skipping font directory '{}': {}", dir.display(), fault);

        return Ok(());
      }
    };
    let walk = Walk {
      database: self,
      pending: Vec::from([directory]),
      seen,
    };

    match console_core_iteration::iterate(walk, step) {
      Ok(()) => {}
      Err(endless) => console_core_logging::error!(
        "Font directory walk under '{}' ended: {}",
        dir.display(),
        endless
      ),
    }

    Ok(())
  }

  #[cfg(feature = "memmap")]
  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "this opt-in API maps the file only when its caller guarantees that the bytes stay unchanged until every face releases the mapping"
    )
  )]
  #[allow(
    clippy::missing_safety_doc,
    reason = "the module head states the promise: nothing changes the font file while any face shares its mapping"
  )]
  pub unsafe fn make_shared_face_data(
    &mut self,
    id: crate::ID,
  ) -> Result<Option<(SharedFontData, u32)>, Never> {
    let face_info = match self.faces.get(id.0) {
      Some(face_info) => face_info,
      None => return Ok(None),
    };
    let face_index = face_info.index;
    let path = match &face_info.source {
      Source::Binary(data) | Source::SharedFile(_, data) => {
        return Ok(Some((SharedFontData::clone(data), face_index)));
      }
      Source::File(path) => path.clone(),
    };
    let opened = std::fs::File::open(&path);
    // SAFETY: the caller keeps the file unchanged for as long as any face shares the mapping.
    let mapped = opened.and_then(|file| unsafe { memmap2::MmapOptions::new().map(&file) });

    let data = match mapped {
      Ok(data) => data,
      Err(fault) => {
        console_core_logging::warn!("Cannot map shared font '{}': {}", path.display(), fault);

        return Ok(None);
      }
    };
    let Ok(shared_data) = shared_font_data(data);
    let shared_source = Source::SharedFile(path.clone(), SharedFontData::clone(&shared_data));

    for (_, face) in self.faces.iter_mut() {
      match &face.source {
        Source::File(face_path) => match face_path.cmp(&path) {
          Ordering::Equal => face.source = shared_source.clone(),
          Ordering::Less | Ordering::Greater => {}
        },
        Source::Binary(_) | Source::SharedFile(..) => {}
      }
    }

    Ok(Some((shared_data, face_index)))
  }

  #[cfg(feature = "memmap")]
  pub fn make_face_data_unshared(&mut self, id: crate::ID) -> Result<(), Never> {
    let face_info = match self.faces.get(id.0) {
      Some(face_info) => face_info,
      None => return Ok(()),
    };
    let shared_path = match &face_info.source {
      Source::SharedFile(path, _) => path.clone(),
      Source::File(_) | Source::Binary(_) => return Ok(()),
    };
    let unshared_source = Source::File(shared_path.clone());

    for (_, face) in self.faces.iter_mut() {
      match &face.source {
        Source::SharedFile(face_path, _) => match face_path.cmp(&shared_path) {
          Ordering::Equal => face.source = unshared_source.clone(),
          Ordering::Less | Ordering::Greater => {}
        },
        Source::File(_) | Source::Binary(_) => {}
      }
    }

    Ok(())
  }
}

fn step(mut walk: Walk) -> Result<Step<Walk, ()>, Never> {
  let next = match walk.pending.last_mut() {
    Some(directory) => directory.next(),
    None => return Ok(Step::Halt(())),
  };

  match next {
    Some(Ok(entry)) => {
      let Ok(()) = visit(&mut walk, entry);
    }
    Some(Err(fault)) => {
      console_core_logging::debug!("Skipping unreadable font directory entry: {}", fault)
    }
    None => {
      let _finished = walk.pending.pop();
    }
  }

  Ok(Step::Again(walk))
}

fn visit(walk: &mut Walk, entry: std::fs::DirEntry) -> Result<(), Never> {
  let Ok(reached) = reach(entry, walk.seen);
  let (path, kind) = match reached {
    Some(reached_2) => reached_2,
    None => return Ok(()),
  };

  match kind {
    EntryKind::File => {
      let Ok(font) = font_extension(&path);

      match font {
        Extension::Font => match walk.database.load_font_file(&path) {
          Ok(()) => {}
          Err(fault) => {
            console_core_logging::warn!("Failed to load '{}' cause {}.", path.display(), fault)
          }
        },
        Extension::Other => {}
      }
    }
    EntryKind::Directory => match std::fs::read_dir(&path) {
      Ok(directory) => walk.pending.push(directory),
      Err(fault) => {
        console_core_logging::debug!("Skipping font directory '{}': {}", path.display(), fault)
      }
    },
    EntryKind::Link | EntryKind::Other => {}
  }

  Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Extension {
  Font,
  Other,
}

fn font_extension(path: &Path) -> Result<Extension, Never> {
  Ok(
    match path.extension().and_then(|extension| extension.to_str()) {
      Some("ttf" | "ttc" | "TTF" | "TTC" | "otf" | "otc" | "OTF" | "OTC") => Extension::Font,
      Some(_) | None => Extension::Other,
    },
  )
}

fn reach(
  entry: std::fs::DirEntry,
  seen: &mut Seen,
) -> Result<Option<(PathBuf, EntryKind)>, Never> {
  let path = entry.path();
  let file_type = match entry.file_type() {
    Ok(file_type) => file_type,
    Err(fault) => {
      console_core_logging::debug!("Cannot inspect font entry '{}': {}", path.display(), fault);

      return Ok(None);
    }
  };

  match EntryKind::from(file_type) {
    EntryKind::Link => followed(path, seen),
    kind @ (EntryKind::File | EntryKind::Directory | EntryKind::Other) => {
      match Emptiness::from(seen.is_empty()) {
        Emptiness::Empty => Ok(Some((path, kind))),
        Emptiness::NonEmpty => match Visit::from(seen.insert(path.clone())) {
          Visit::First => Ok(Some((path, kind))),
          Visit::Repeated => Ok(None),
        },
      }
    }
  }
}

fn followed(link: PathBuf, seen: &mut Seen) -> Result<Option<(PathBuf, EntryKind)>, Never> {
  let target = match std::fs::metadata(&link) {
    Ok(target) => target,
    Err(fault) => {
      console_core_logging::debug!("Cannot follow font link '{}': {}", link.display(), fault);

      return Ok(None);
    }
  };
  let canonical = match std::fs::canonicalize(&link) {
    Ok(canonical) => canonical,
    Err(fault) => {
      console_core_logging::debug!("Cannot resolve font link '{}': {}", link.display(), fault);

      return Ok(None);
    }
  };

  Ok(match Visit::from(seen.insert(canonical.clone())) {
    Visit::First => Some((canonical, EntryKind::from(target.file_type()))),
    Visit::Repeated => None,
  })
}

pub(crate) fn with_file_data<P, T>(path: &Path, p: P) -> Result<Option<T>, Never>
where
  P: FnOnce(&[u8]) -> T,
{
  let read = std::fs::read(path);

  Ok(match read {
    Ok(data) => Some(p(&data)),
    Err(fault) => {
      console_core_logging::warn!("Cannot read font '{}': {}", path.display(), fault);

      None
    }
  })
}
