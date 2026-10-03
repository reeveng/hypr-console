//! Where a snapshot rests while the power is off.
//!
//! effect's `KeyValueStore`, with its file-system layer and nothing else: a key
//! is a file in one directory and a value is that file's bytes, written whole
//! or not at all through `console-core-atomic-writes`. A key is one file name
//! and never a path, so no machine can name a file outside the directory it was
//! handed.
//!
//! It is here rather than beside `Machine` because a machine is arithmetic and
//! this is the disk; the actor is what saves, so the actor is what holds it.
//!
//! Absent and unreadable are two answers. A key nobody has set is `None`, which
//! means the machine starts fresh; a key that is there and cannot be read is a
//! fault, because starting fresh over it would overwrite what somebody saved.

use std::ffi::OsStr;
use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::{Ungone, Unwritten, gone, whole_with_folders};
use console_core_never::Never;

#[derive(Debug)]
pub enum PlatformError {
    BadArgument(String),
    Get(PathBuf, ErrorKind),
    Set(Unwritten),
    Remove(Ungone),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::BadArgument(key) => write!(to, "{key:?} is not a key: a key is one file name"),
            PlatformError::Get(at, kind) => write!(to, "{}: reading it: {kind}", at.display()),
            PlatformError::Set(fault) => write!(to, "{fault}"),
            PlatformError::Remove(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for PlatformError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyValueStore {
    directory: PathBuf,
}

impl KeyValueStore {
    pub fn file_system(directory: PathBuf) -> Result<Self, Never> {
        Ok(KeyValueStore { directory })
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PlatformError> {
        let at = self.at(key)?;

        match std::fs::read(&at) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(fault) => match fault.kind() == ErrorKind::NotFound {
                true => Ok(None),
                false => Err(PlatformError::Get(at, fault.kind())),
            },
        }
    }

    pub fn set(&self, key: &str, bytes: &[u8]) -> Result<(), PlatformError> {
        let at = self.at(key)?;

        whole_with_folders(&at, bytes).map_err(PlatformError::Set)
    }

    pub fn remove(&self, key: &str) -> Result<(), PlatformError> {
        let at = self.at(key)?;

        gone(&at).map_err(PlatformError::Remove)
    }

    fn at(&self, key: &str) -> Result<PathBuf, PlatformError> {
        match Path::new(key).file_name() == Some(OsStr::new(key)) {
            true => Ok(self.directory.join(key)),
            false => Err(PlatformError::BadArgument(key.to_string())),
        }
    }
}
