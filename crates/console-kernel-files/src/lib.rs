//! Files that are themselves, whatever they are called.
//!
//! A path file system makes the name the key: open a file and what the kernel
//! resolved was a string, rename it and the old string points at nothing, and
//! a handle that outlives the rename either follows an inode the model never
//! mentions or breaks. For a capability kernel that is the wrong way round. A
//! handle has to name one thing for as long as it is held, and a string is the
//! one thing a second process can change underneath it. So a file here is an
//! object with an identity, and a path is a relationship between an identity
//! and a string that the identity does not depend on.
//!
//! The model is pidvcs's, the version control in `~/Documents/projects/pidvcs`,
//! which was written to show that files rather than paths can carry an
//! identity and that everything else falls out of it. An identity is a number
//! drawn once, with nothing about the file in it; the kernel has no randomness
//! yet, so whoever creates a file hands the number in. A version is immutable
//! and names its parent and its content, and the content lives in a store keyed
//! by its hash, so the same bytes are held once however many files hold them.
//! Which hash is also handed in, as a `Digest`: the one checksum crate in this
//! tree speaks MD5 and CRC32 for other people's formats, needs `std`, and is not
//! a name anybody should address content by.
//!
//! What that buys the kernel is three things it would otherwise have to build.
//! A handle names an identity, so it keeps working across a rename, because a
//! rename changes a relationship and not the file. History is free, because a
//! write never overwrites: it adds a version whose parent is the one before,
//! and reading the past is walking parents. And a directory is a view rather
//! than an owner -- the names table is keyed by identity, two identities may
//! claim one path, and `collisions` reports it instead of refusing it, because
//! only a projection onto a tree needs paths to be unique and that restriction
//! belongs to the projection.
//!
//! A change is pidvcs's commit: create, modify, rename, delete, with no fifth
//! operation, applied as one set. `apply` folds the set over a copy and hands
//! back the new state only when every operation took, so a set that fails in
//! the middle leaves nothing half done; the copy is the whole state, which is
//! the price of saying that with values while there is no disk to journal to.
//!
//! In Zircon's shape the file system is not inside the kernel: it is a process
//! serving files over channels, and a handle to a file is a channel end to that
//! server. This is what such a server holds, written for this kernel and in its
//! family, touching no machine, and `no_std` so either side can link it.

#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContentHash(pub [u8; 32]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VersionId(ContentHash);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path(pub String);

pub trait Digest {
    fn digest(&self, bytes: &[u8]) -> Result<ContentHash, Never>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub id: VersionId,
    pub parent: Option<VersionId>,
    pub content: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Create { file: FileId, at: Path, bytes: Vec<u8> },
    Modify { file: FileId, from: VersionId, bytes: Vec<u8> },
    Rename { file: FileId, to: Path },
    Delete { file: FileId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    pub at: Path,
    pub claimed_by: Vec<FileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesError {
    AlreadyExists(FileId),
    NotFound(FileId),
    Deleted(FileId),
    StaleVersion(FileId),
    MissingVersion(VersionId),
    MissingContent(ContentHash),
}

#[derive(Clone)]
pub struct Files<D> {
    digest: D,
    contents: BTreeMap<ContentHash, Vec<u8>>,
    versions: BTreeMap<VersionId, Version>,
    heads: BTreeMap<FileId, VersionId>,
    names: BTreeMap<FileId, Path>,
}

impl<D: Digest + Clone> Files<D> {
    pub fn new(digest: D) -> Result<Files<D>, Never> {
        Ok(Files {
            digest,
            contents: BTreeMap::new(),
            versions: BTreeMap::new(),
            heads: BTreeMap::new(),
            names: BTreeMap::new(),
        })
    }

    #[must_use = "the new state is the only place the change happened"]
    pub fn apply(&self, change: &[Operation]) -> Result<Files<D>, FilesError> {
        let mut draft = self.clone();

        for operation in change {
            draft.perform(operation)?;
        }

        Ok(draft)
    }

    pub fn head(&self, file: FileId) -> Result<VersionId, FilesError> {
        self.live(file)
    }

    pub fn path(&self, file: FileId) -> Result<&Path, FilesError> {
        self.live(file)?;

        match self.names.get(&file) {
            Some(at) => Ok(at),
            None => Err(FilesError::NotFound(file)),
        }
    }

    pub fn read(&self, file: FileId) -> Result<&[u8], FilesError> {
        let head = self.live(file)?;
        let version = self.version(head)?;

        self.content(version.content)
    }

    pub fn content(&self, hash: ContentHash) -> Result<&[u8], FilesError> {
        match self.contents.get(&hash) {
            Some(bytes) => Ok(bytes),
            None => Err(FilesError::MissingContent(hash)),
        }
    }

    pub fn stored(&self) -> Result<Vec<ContentHash>, Never> {
        Ok(self.contents.keys().copied().collect())
    }

    pub fn history(&self, file: FileId) -> Result<Vec<Version>, FilesError> {
        let mut walking = match self.heads.get(&file) {
            Some(head) => Some(*head),
            None => return Err(FilesError::NotFound(file)),
        };
        let mut history = Vec::new();

        while let Some(at) = walking {
            let version = self.version(at)?;

            history.push(version);
            walking = version.parent;
        }

        Ok(history)
    }

    pub fn collisions(&self) -> Result<Vec<Collision>, Never> {
        let mut claims: BTreeMap<&Path, Vec<FileId>> = BTreeMap::new();

        for (file, at) in &self.names {
            claims.entry(at).or_default().push(*file);
        }

        Ok(claims
            .into_iter()
            .filter(|(_, claimed_by)| claimed_by.get(1).is_some())
            .map(|(at, claimed_by)| Collision { at: at.clone(), claimed_by })
            .collect())
    }

    fn perform(&mut self, operation: &Operation) -> Result<(), FilesError> {
        match operation {
            Operation::Create { file, at, bytes } => {
                match self.heads.contains_key(file) {
                    true => return Err(FilesError::AlreadyExists(*file)),
                    false => {},
                }

                let Ok(version) = self.record(*file, None, bytes);
                let _fresh = self.heads.insert(*file, version);
                let _named = self.names.insert(*file, at.clone());
            },
            Operation::Modify { file, from, bytes } => {
                let head = self.live(*file)?;

                match head == *from {
                    true => {},
                    false => return Err(FilesError::StaleVersion(*file)),
                }

                let Ok(version) = self.record(*file, Some(head), bytes);
                let _replaced = self.heads.insert(*file, version);
            },
            Operation::Rename { file, to } => {
                self.live(*file)?;

                let _renamed = self.names.insert(*file, to.clone());
            },
            Operation::Delete { file } => {
                self.live(*file)?;

                let _removed = self.names.remove(file);
            },
        }

        Ok(())
    }

    fn record(&mut self, file: FileId, parent: Option<VersionId>, bytes: &[u8]) -> Result<VersionId, Never> {
        let Ok(content) = self.digest.digest(bytes);
        let _stored = self.contents.entry(content).or_insert_with(|| bytes.to_vec());
        let mut named = Vec::new();

        named.extend_from_slice(b"version\0");
        named.extend_from_slice(&file.0);

        match parent {
            Some(VersionId(ContentHash(parent))) => named.extend_from_slice(&parent),
            None => named.extend_from_slice(&[0; 32]),
        }

        named.extend_from_slice(&content.0);

        let Ok(hash) = self.digest.digest(&named);
        let id = VersionId(hash);
        let _fresh = self.versions.insert(id, Version { id, parent, content });

        Ok(id)
    }

    fn live(&self, file: FileId) -> Result<VersionId, FilesError> {
        match (self.names.contains_key(&file), self.heads.get(&file)) {
            (true, Some(head)) => Ok(*head),
            (false, Some(_)) => Err(FilesError::Deleted(file)),
            (true | false, None) => Err(FilesError::NotFound(file)),
        }
    }

    fn version(&self, id: VersionId) -> Result<Version, FilesError> {
        match self.versions.get(&id) {
            Some(version) => Ok(*version),
            None => Err(FilesError::MissingVersion(id)),
        }
    }
}

impl fmt::Display for FilesError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        to.write_str(match self {
            FilesError::AlreadyExists(_) => "a file with that identity already exists",
            FilesError::NotFound(_) => "there is no file with that identity",
            FilesError::Deleted(_) => "the file was deleted",
            FilesError::StaleVersion(_) => "the file has changed since the version the change was made against",
            FilesError::MissingVersion(_) => "a version the history names is not stored",
            FilesError::MissingContent(_) => "the content a version names is not stored",
        })
    }
}
