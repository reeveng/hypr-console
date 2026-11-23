//! A configuration file merged into what has been read so far, with every file it includes, in order.
//!
//! A file is merged once. Its canonical path is written down before it is read,
//! and a file met again by any road is passed over, which is how a `conf.d`
//! that includes its own parent ends. Includes are taken depth first: the file
//! an include names, or every file in the directory it names in lexicographic
//! order, is merged before the part after the include, because that order is
//! what lets a later `<reset-dirs/>` take back an earlier `<dir>`. The walk keeps
//! a stack of the files it is inside rather than calling itself, so how deep the
//! includes go is a length rather than a stack frame per file.
//!
//! A fault in the file asked for is the caller's, and ends the merge where it
//! was met. A fault in an included file ends that file where it was met and is
//! said, unless the include said a missing file is no matter; a fault in a file
//! of an included directory is said and the next file is merged. That is
//! fontconfig-parser's order of events, and the parts merged before a fault
//! stay merged, as they did there. What is said goes to the log when the `log`
//! feature is on and nowhere when it is off.

use crate::parser::parse_config;
use crate::{
  Alias, Bases, CacheDir, Configuration, Dir, Error, Include, IncludeFailure, Match, RemapDir,
  SelectFont,
};

use console_core_iteration::{Endless, Step};
use console_core_never::Never;
use std::collections::HashSet;
use std::fs::FileType;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConfigPart {
  Description(String),
  SelectFont(SelectFont),
  Dir(Dir),
  CacheDir(CacheDir),
  Include(Include),
  Match(Match),
  #[cfg_attr(feature = "serde", serde(rename = "Config"))]
  Configuration(Configuration),
  Alias(Alias),
  RemapDir(RemapDir),
  ResetDirs,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontConfig {
  pub select_fonts: Vec<SelectFont>,
  pub dirs: Vec<DirData>,
  pub cache_dirs: Vec<PathBuf>,
  pub remap_dirs: Vec<RemapDirData>,
  pub matches: Vec<Match>,
  pub config: Configuration,
  pub aliases: Vec<Alias>,
  pub config_files: HashSet<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DirData {
  pub path: PathBuf,

  pub salt: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemapDirData {
  pub path: PathBuf,

  pub salt: String,

  pub as_path: String,
}

enum Opened {
  Read(Vec<Result<ConfigPart, Error>>),
  AlreadyMerged,
}

enum Included {
  File(Opened),
  Directory(Vec<PathBuf>),
  Neither,
}

enum ConfigEntry {
  File,
  Directory,
  Link,
  Other,
}

impl From<FileType> for ConfigEntry {
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

enum MergeVisit {
  First,
  Repeated,
}

impl From<bool> for MergeVisit {
  fn from(first_visit: bool) -> Self {
    match bool::then_some(first_visit, ()) {
      Some(()) => Self::First,
      None => Self::Repeated,
    }
  }
}

struct Inclusion {
  include_path: PathBuf,
  ignore_missing: IncludeFailure,
}

enum Origin {
  Requested,
  Included(Inclusion),
  Member(PathBuf),
}

enum Work {
  Parts(std::vec::IntoIter<Result<ConfigPart, Error>>),
  Members(std::vec::IntoIter<PathBuf>),
}

struct Frame {
  origin: Origin,
  path: PathBuf,
  work: Work,
}

enum Next {
  Part(PathBuf, ConfigPart),
  Member(PathBuf),
  ParseError(Error),
  Finished,
  Empty,
}

struct Merging<'a> {
  config: &'a mut FontConfig,
  bases: &'a Bases,
  stack: Vec<Frame>,
}

impl FontConfig {
  pub fn merge_config<P: AsRef<Path> + ?Sized>(
    &mut self,
    config_path: &P,
    bases: &Bases,
  ) -> Result<(), Error> {
    let path = config_path.as_ref();
    let opened = self.open(path)?;
    let parts = match opened {
      Opened::Read(parts) => parts,
      Opened::AlreadyMerged => return Ok(()),
    };
    let asked = Frame {
      origin: Origin::Requested,
      path: path.to_path_buf(),
      work: Work::Parts(parts.into_iter()),
    };
    let merging = Merging {
      config: self,
      bases,
      stack: Vec::from([asked]),
    };

    match console_core_iteration::iterate(merging, step) {
      Ok(merged) => merged,
      Err(Endless) => Ok(()),
    }
  }

  fn open(&mut self, path: &Path) -> Result<Opened, Error> {
    let canonical = std::fs::canonicalize(path)?;

    match MergeVisit::from(self.config_files.insert(canonical)) {
      MergeVisit::First => {}
      MergeVisit::Repeated => return Ok(Opened::AlreadyMerged),
    }

    let config = std::fs::read_to_string(path)?;
    let options = roxmltree::ParsingOptions {
      dtd: roxmltree::Dtd::Allow,
      ..Default::default()
    };
    let document = roxmltree::Document::parse_with_options(&config, options)?;
    let parts = parse_config(&document)?;

    Ok(Opened::Read(parts.collect()))
  }

  fn opened_include(&mut self, include_path: &Path) -> Result<Included, Error> {
    let metadata = std::fs::metadata(include_path)?;
    let file_type = metadata.file_type();

    match ConfigEntry::from(file_type) {
      ConfigEntry::File => {
        let opened = self.open(include_path)?;

        Ok(Included::File(opened))
      }
      ConfigEntry::Directory => {
        let members = members_of(include_path)?;

        Ok(Included::Directory(members))
      }
      ConfigEntry::Link | ConfigEntry::Other => Ok(Included::Neither),
    }
  }
}

fn step(mut merging: Merging) -> Result<Step<Merging, Result<(), Error>>, Never> {
  let Ok(next) = take_next(&mut merging.stack);

  match next {
    Next::Empty => return Ok(Step::Halt(Ok(()))),
    Next::Finished => {
      let _finished = merging.stack.pop();
    }
    Next::Part(config_path, part) => {
      let Ok(()) = merging.apply(part, &config_path);
    }
    Next::Member(member) => {
      let Ok(()) = merging.member(member);
    }
    Next::ParseError(fault) => {
      let ended = merging.stack.pop();

      match ended.map(|frame| frame.origin) {
        Some(Origin::Requested) => return Ok(Step::Halt(Err(fault))),
        Some(origin @ (Origin::Included(_) | Origin::Member(_))) => {
          let Ok(()) = report(origin, fault);
        }
        None => {}
      }
    }
  }

  Ok(Step::Again(merging))
}

fn take_next(stack: &mut [Frame]) -> Result<Next, Never> {
  let frame = match stack.last_mut() {
    Some(frame) => frame,
    None => return Ok(Next::Empty),
  };

  Ok(match &mut frame.work {
    Work::Parts(parts) => match parts.next() {
      Some(Ok(part)) => Next::Part(frame.path.clone(), part),
      Some(Err(fault)) => Next::ParseError(fault),
      None => Next::Finished,
    },
    Work::Members(members) => match members.next() {
      Some(member) => Next::Member(member),
      None => Next::Finished,
    },
  })
}

impl Merging<'_> {
  fn apply(&mut self, part: ConfigPart, config_path: &Path) -> Result<(), Never> {
    match part {
      ConfigPart::Alias(alias) => self.config.aliases.push(alias),
      ConfigPart::Configuration(mut config) => {
        self.config.config.rescans.append(&mut config.rescans);
        self.config.config.blanks.append(&mut config.blanks);
      }
      ConfigPart::Description(_) => {}
      ConfigPart::Dir(dir) => {
        let Ok(path) = dir.calculate_path(config_path, self.bases);

        self.config.dirs.push(DirData {
          path,
          salt: dir.salt,
        });
      }
      ConfigPart::CacheDir(dir) => {
        let Ok(path) = dir.calculate_path(config_path, self.bases);

        self.config.cache_dirs.push(path);
      }
      ConfigPart::Match(matched) => self.config.matches.push(matched),
      ConfigPart::ResetDirs => self.config.dirs.clear(),
      ConfigPart::SelectFont(select) => self.config.select_fonts.push(select),
      ConfigPart::RemapDir(remap) => {
        let Ok(path) = remap.calculate_path(config_path, self.bases);

        self.config.remap_dirs.push(RemapDirData {
          path,
          salt: remap.salt,
          as_path: remap.as_path,
        });
      }
      ConfigPart::Include(include) => {
        let Ok(include_path) = include.calculate_path(config_path, self.bases);

        return self.include(Inclusion {
          include_path,
          ignore_missing: include.ignore_missing,
        });
      }
    }

    Ok(())
  }

  fn include(&mut self, inclusion: Inclusion) -> Result<(), Never> {
    let opened = self.config.opened_include(&inclusion.include_path);
    let include_path = inclusion.include_path.clone();
    let origin = Origin::Included(inclusion);

    match opened {
      Ok(Included::File(Opened::Read(parts))) => self.stack.push(Frame {
        origin,
        path: include_path,
        work: Work::Parts(parts.into_iter()),
      }),
      Ok(Included::Directory(members)) => self.stack.push(Frame {
        origin,
        path: include_path,
        work: Work::Members(members.into_iter()),
      }),
      Ok(Included::File(Opened::AlreadyMerged) | Included::Neither) => {}
      Err(fault) => {
        let Ok(()) = report(origin, fault);
      }
    }

    Ok(())
  }

  fn member(&mut self, member: PathBuf) -> Result<(), Never> {
    match self.config.open(&member) {
      Ok(Opened::Read(parts)) => self.stack.push(Frame {
        origin: Origin::Member(member.clone()),
        path: member,
        work: Work::Parts(parts.into_iter()),
      }),
      Ok(Opened::AlreadyMerged) => {}
      Err(fault) => {
        let Ok(()) = report(Origin::Member(member), fault);
      }
    }

    Ok(())
  }
}

fn members_of(directory: &Path) -> Result<Vec<PathBuf>, Error> {
  let listing = std::fs::read_dir(directory)?;
  let mut members = Vec::new();

  for entry in listing {
    match entry {
      Ok(entry_2) => match entry_2.file_type() {
        Ok(file_type) => match ConfigEntry::from(file_type) {
          ConfigEntry::File | ConfigEntry::Link => members.push(entry_2.path()),
          ConfigEntry::Directory | ConfigEntry::Other => {}
        },
        Err(fault) => {
          let Ok(()) = said(format_args!(
            "Skipping {}: {}",
            entry_2.path().display(),
            fault
          ));
        }
      },
      Err(fault) => {
        let Ok(()) = said(format_args!(
          "Skipping an entry of {}: {}",
          directory.display(),
          fault
        ));
      }
    }
  }

  members.sort();

  Ok(members)
}

fn report(origin: Origin, fault: Error) -> Result<(), Never> {
  match origin {
    Origin::Requested => Ok(()),
    Origin::Included(Inclusion {
      include_path,
      ignore_missing,
    }) => match ignore_missing {
      IncludeFailure::Ignore => Ok(()),
      IncludeFailure::Report => said(format_args!(
        "Failed to include {}: {}",
        include_path.display(),
        fault
      )),
    },
    Origin::Member(member) => said(format_args!(
      "Failed to merge {}: {}",
      member.display(),
      fault
    )),
  }
}

#[cfg(feature = "log")]
fn said(sentence: core::fmt::Arguments) -> Result<(), Never> {
  console_core_logging::warn!("{}", sentence);

  Ok(())
}

#[cfg(not(feature = "log"))]
fn said(_unlogged: core::fmt::Arguments) -> Result<(), Never> {
  Ok(())
}

impl From<SelectFont> for ConfigPart {
  fn from(part: SelectFont) -> Self {
    ConfigPart::SelectFont(part)
  }
}

impl From<Dir> for ConfigPart {
  fn from(part: Dir) -> Self {
    ConfigPart::Dir(part)
  }
}

impl From<CacheDir> for ConfigPart {
  fn from(part: CacheDir) -> Self {
    ConfigPart::CacheDir(part)
  }
}

impl From<Include> for ConfigPart {
  fn from(part: Include) -> Self {
    ConfigPart::Include(part)
  }
}

impl From<Match> for ConfigPart {
  fn from(part: Match) -> Self {
    ConfigPart::Match(part)
  }
}

impl From<Configuration> for ConfigPart {
  fn from(part: Configuration) -> Self {
    ConfigPart::Configuration(part)
  }
}

impl From<Alias> for ConfigPart {
  fn from(part: Alias) -> Self {
    ConfigPart::Alias(part)
  }
}

impl From<RemapDir> for ConfigPart {
  fn from(part: RemapDir) -> Self {
    ConfigPart::RemapDir(part)
  }
}
