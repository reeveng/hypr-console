use crate::xkb::code::Code;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use console_core_never::Never;
use console_core_number_conversion::index;
use hashbrown::HashMap;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::slice;

pub(crate) struct CodeLoader {
  include_paths: Vec<PathBuf>,
  cache: HashMap<Key, Vec<Option<CodeFromPath>>>,
  partial_path: Vec<u8>,
  full_path: PathBuf,
}

#[derive(Clone)]
struct CodeFromPath {
  path: PathBuf,
  code: Code,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct Key {
  ty: CodeType,
  path: Interned,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) enum CodeType {
  Keycodes,
  Types,
  Compat,
  Symbols,
  Geometry,
  Keymap,
  Rules,
}

#[derive(Debug)]
pub(crate) struct CodeLoaderError {
  path: String,
  err: io::Error,
}

impl std::fmt::Display for CodeLoaderError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "could not read file {}", self.path)
  }
}

impl std::error::Error for CodeLoaderError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(&self.err)
  }
}

type Loaded = Result<(PathBuf, Code), CodeLoaderError>;

enum Cached {
  Found(CodeFromPath),
  Absent,
  Unread,
}

impl CodeType {
  fn push_sub_dir(self, directory: &mut PathBuf) -> Result<(), Never> {
    match self {
      CodeType::Keycodes => directory.push("keycodes"),
      CodeType::Types => directory.push("types"),
      CodeType::Compat => directory.push("compat"),
      CodeType::Symbols => directory.push("symbols"),
      CodeType::Geometry => directory.push("geometry"),
      CodeType::Keymap => directory.push("keymap"),
      CodeType::Rules => directory.push("rules"),
    }

    Ok(())
  }
}

impl CodeLoader {
  pub(crate) fn new(include_paths: &[PathBuf]) -> Result<Self, Never> {
    Ok(Self {
      include_paths: include_paths.to_vec(),
      cache: Default::default(),
      partial_path: vec![],
      full_path: Default::default(),
    })
  }

  pub(crate) fn load(
    &mut self,
    interner: &Interner,
    ty: CodeType,
    path: Interned,
  ) -> Result<CodeIter<'_>, Never> {
    let key = Key { ty, path };
    let entry = self.cache.entry(key).or_default();
    let Ok(name) = interner.get(path);
    self.partial_path.clear();
    self.partial_path.extend_from_slice(name);
    let wanted = Path::new(OsStr::from_bytes(&self.partial_path));

    let walk = match ::core::primitive::bool::then_some(wanted.is_absolute(), ()) {
      ::core::option::Option::Some(()) => CodeIterTy::Absolute { path: Some(wanted) },
      ::core::option::Option::None => {
        let Ok(relative) = RelativeName::new(wanted);

        match relative {
          Some(relative_name) => CodeIterTy::Relative(Relative {
            at: 0,
            directories: self.include_paths.iter(),
            path: relative_name,
            ty,
            full_path: &mut self.full_path,
          }),
          None => CodeIterTy::Rejected { path: Some(wanted) },
        }
      }
    };

    Ok(CodeIter { entry, ty: walk })
  }
}

pub(crate) struct CodeIter<'a> {
  entry: &'a mut Vec<Option<CodeFromPath>>,
  ty: CodeIterTy<'a>,
}

enum CodeIterTy<'a> {
  Absolute { path: Option<&'a Path> },
  Relative(Relative<'a>),
  Rejected { path: Option<&'a Path> },
}

struct Relative<'a> {
  at: u32,
  directories: slice::Iter<'a, PathBuf>,
  path: RelativeName<'a>,
  ty: CodeType,
  full_path: &'a mut PathBuf,
}

struct RelativeName<'a>(&'a Path);

impl<'a> RelativeName<'a> {
  #[must_use]
  fn new(path: &'a Path) -> Result<Option<Self>, Never> {
    let mut components = path.components();

    Ok(match components.next() {
      None => None,
      Some(first) => std::iter::once(first)
        .chain(components)
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
        .then_some(Self(path)),
    })
  }
}

impl AsRef<Path> for RelativeName<'_> {
  fn as_ref(&self) -> &Path {
    self.0
  }
}

impl Iterator for CodeIter<'_> {
  type Item = Loaded;

  fn next(&mut self) -> Option<Self::Item> {
    match &mut self.ty {
      CodeIterTy::Absolute { path } => {
        let wanted = path.take()?;
        let Ok(remembered) = cached(self.entry, 0);

        match remembered {
          Cached::Found(code) => Some(Ok((code.path, code.code))),
          Cached::Absent => None,
          Cached::Unread => {
            let Ok(outcome) = read(self.entry, wanted);
            outcome
          }
        }
      }
      CodeIterTy::Relative(relative) => {
        let Ok(found) = relative.next(self.entry);
        found
      }
      CodeIterTy::Rejected { path } => {
        let wanted = path.take()?;
        Some(Err(CodeLoaderError {
          path: wanted.display().to_string(),
          err: io::Error::new(
            io::ErrorKind::InvalidInput,
            "relative include must stay inside its search directory",
          ),
        }))
      }
    }
  }
}

impl Relative<'_> {
  fn next(&mut self, entry: &mut Vec<Option<CodeFromPath>>) -> Result<Option<Loaded>, Never> {
    for directory in self.directories.by_ref() {
      let Ok(remembered) = cached(entry, self.at);
      self.at = self.at.saturating_add(1);

      match remembered {
        Cached::Found(code) => return Ok(Some(Ok((code.path, code.code)))),
        Cached::Absent => {}
        Cached::Unread => {
          self.full_path.clone_from(directory);
          let Ok(()) = self.ty.push_sub_dir(self.full_path);
          self.full_path.push(&self.path);
          let Ok(outcome) = read(entry, self.full_path.as_path());

          match outcome {
            Some(found) => return Ok(Some(found)),
            None => {}
          }
        }
      }
    }

    Ok(None)
  }
}

fn cached(entry: &[Option<CodeFromPath>], position: u32) -> Result<Cached, Never> {
  let Ok(at) = index(position);

  Ok(match entry.get(at) {
    Some(Some(code)) => Cached::Found(code.clone()),
    Some(None) => Cached::Absent,
    None => Cached::Unread,
  })
}

fn read(entry: &mut Vec<Option<CodeFromPath>>, path: &Path) -> Result<Option<Loaded>, Never> {
  let bytes = match std::fs::read(path) {
    Ok(read_bytes) => read_bytes,
    Err(err) => match ::core::primitive::bool::then_some(err.kind() == io::ErrorKind::NotFound, ()) {
      ::core::option::Option::Some(()) => {
        entry.push(None);
        return Ok(None);
      }
      ::core::option::Option::None => {
        return Ok(Some(Err(CodeLoaderError {
          path: path.display().to_string(),
          err,
        })));
      }
    },
  };
  let Ok(code) = Code::owning(bytes);
  let res = CodeFromPath {
    path: path.to_path_buf(),
    code,
  };
  entry.push(Some(res.clone()));
  Ok(Some(Ok((res.path, res.code))))
}
