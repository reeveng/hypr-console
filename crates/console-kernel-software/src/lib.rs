//! Software that is what went into it, and a program born holding only that.
//!
//! `console-kernel-files` is the store for what a person makes: a file there
//! has an identity that outlives its name and a history that grows. Software
//! is the opposite thing and wants the opposite store. Nobody edits a
//! compiler; a new one is a different object beside the old one, and the only
//! question worth asking of either is what it was made from. So the decision
//! this stands on is that the store for software is Nix's: an object is named
//! by a hash of everything that went into it, it never changes once it is
//! there, and a store path is spelled the way Nix spells one, `<hash>-<name>`.
//! That is what makes a piece of software a value, and a value is a thing that
//! can be tested here without a machine.
//!
//! An object records its references, the other store paths it needs, and the
//! hash is taken over those as well as over its content, its name and whether
//! it holds a program. Three things fall out of that and none of them is
//! checked anywhere else. An object can only be added after everything it
//! refers to, so the store is closed under references at every moment rather
//! than after a repair. It cannot refer to itself or to anything that refers
//! back, because it would need its own hash to say so, so the references are
//! a graph with no cycle in it and a closure is finite. And a path cannot be
//! forged: `add` works the hash out again and refuses a path the object does
//! not produce, so a name in this store is evidence and not a claim.
//!
//! A closure is everything reachable from a path through references, and it
//! is found by one walk rather than by a function calling itself, because the
//! kernel's stack is not a thing a long chain of dependencies should be able
//! to spend. An object arrives after everything it refers to, so the order of
//! arrival is already a sorted one: walk it backwards, and by the time a path
//! comes up everything that could refer to it has been seen. A list of what is
//! still to visit was the first shape and was rejected for its loop, which
//! ends when the list is empty and says nowhere how long that takes; this one
//! is a `for` over the store and ends where the store does.
//!
//! Nothing is deleted by name. A root is a path somebody still holds, counted,
//! so two holders are two and the path stays until both have let go; `collect`
//! removes exactly what no root's closure reaches and says what went. Removing
//! one path on request was rejected: it is the one operation that can leave an
//! object pointing at nothing, and every other line here exists so that cannot
//! happen.
//!
//! What this is for is `grants`. I1 says a process can name only what it was
//! given, and I3 that sharing is explicit, and for a program the whole of what
//! it should be given is already written down: its closure. `grants` answers
//! that list -- read on every path in the closure, execute as well on the ones
//! marked as holding a program, and nothing outside it -- so a program is
//! started holding its dependencies and no search path, no `/usr` and no
//! library somebody else installed. The list is plain data in this crate's own
//! small types and not `console-kernel-handles`' rights. In the shape that
//! kernel took from Fuchsia, as `console-kernel-files` says of itself, the
//! file system is a process serving channels and the kernel has never heard of
//! a store path; whoever starts a program reads this list and asks the kernel
//! for handles, and execute in particular is the kernel's to give, through the
//! executable resource, not this crate's to assert. Depending on the handle
//! kernel would have put a server's vocabulary inside the kernel's or the
//! kernel's table inside a server.
//!
//! The hash is handed in, and it is `console-kernel-files`' `Digest` rather
//! than a second trait of the same shape. The dependency runs sideways, from
//! one server's model to its neighbour's, for one trait and one type; a copy
//! was rejected because whoever boots the machine would then implement one
//! hash twice and the two stores could disagree about what the same bytes are
//! called. The trait wants a crate of its own that both take it from, and that
//! move belongs to the commit that moves it.
//!
//! What is Nix's here is the model and the spelling of a path. Its real
//! hashing is not: no SHA-256, no base32 with its own alphabet, no hash cut to
//! a hundred and sixty bits, none of the `source` and `output` fingerprint
//! strings, and no NAR. The hash is written in hexadecimal and the fingerprint
//! is this crate's own, so a path made here is not a path `/nix/store` would
//! recognise, and nothing here reads that directory.

#![no_std]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
use core::num::NonZeroU64;

use console_core_never::Never;
use console_kernel_files::{ContentHash, Digest};

const ALLOWED_PUNCTUATION: &str = "+-._?=";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Name(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StorePath {
  pub hash: ContentHash,
  pub name: Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
  Data,
  Program,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
  pub kind: Kind,
  pub references: BTreeSet<StorePath>,
  pub content: Vec<u8>,
}

enum StoredObject {
  Identical,
  Different,
}

impl From<bool> for StoredObject {
  fn from(identical: bool) -> Self {
    match bool::then_some(identical, ()) {
      Some(()) => Self::Identical,
      None => Self::Different,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Right {
  Read,
  Execute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
  pub path: StorePath,
  pub rights: BTreeSet<Right>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
  InvalidName,
  WrongHash(StorePath),
  Immutable(StorePath),
  MissingReference(StorePath),
  NotFound(StorePath),
  NotRetained(StorePath),
  TooManyRoots(StorePath),
}

#[derive(Clone)]
pub struct Store<D> {
  digest: D,
  objects: BTreeMap<StorePath, Object>,
  arrivals: Vec<StorePath>,
  roots: BTreeMap<StorePath, NonZeroU64>,
}

impl Name {
  pub fn new(spelled: String) -> Result<Name, StoreError> {
    let start = spelled.as_bytes().first().filter(|first| **first != b'.');
    let valid = start.filter(|_| {
      spelled
        .chars()
        .all(|letter| letter.is_ascii_alphanumeric() || ALLOWED_PUNCTUATION.contains(letter))
    });

    match valid {
      Some(_) => Ok(Name(spelled)),
      None => Err(StoreError::InvalidName),
    }
  }

  pub fn as_str(&self) -> Result<&str, Never> {
    Ok(&self.0)
  }
}

impl<D: Digest> Store<D> {
  pub fn new(digest: D) -> Result<Store<D>, Never> {
    #[cfg_attr(
      dylint_lib = "explicit066_no_heap_after_init",
      expect(
        explicit066_no_heap_after_init,
        reason = "store initialization creates the empty arrival index and Vec::new has zero capacity"
      )
    )]
    let arrivals = Vec::new();

    Ok(Store {
      digest,
      objects: BTreeMap::new(),
      arrivals,
      roots: BTreeMap::new(),
    })
  }

  pub fn path(&self, name: &Name, object: &Object) -> Result<StorePath, Never> {
    let Ok(content) = self.digest.digest(&object.content);
    let kind = match object.kind {
      Kind::Data => b"data\0\0\0\0",
      Kind::Program => b"program\0",
    };
    let prefix: [&[u8]; 4] = [b"software\0", kind, name.0.as_bytes(), b"\0"];
    let suffix: [&[u8]; 2] = [b"\0", &content.0];
    let references = object.references.iter().flat_map(|reference| {
      let fields: [&[u8]; 3] = [&reference.hash.0, reference.name.0.as_bytes(), b"\0"];

      fields
    });
    let Ok(hash) = self
      .digest
      .digest_parts(prefix.into_iter().chain(references).chain(suffix));

    Ok(StorePath {
      hash,
      name: name.clone(),
    })
  }

  pub fn add(&mut self, path: &StorePath, object: Object) -> Result<(), StoreError> {
    match self.objects.get(path) {
      Some(stored) => match StoredObject::from(*stored == object) {
        StoredObject::Identical => Ok(()),
        StoredObject::Different => Err(StoreError::Immutable(path.clone())),
      },
      None => {
        let Ok(produced) = self.path(&path.name, &object);

        match produced.cmp(path) {
          Ordering::Equal => {}
          Ordering::Less | Ordering::Greater => return Err(StoreError::WrongHash(path.clone())),
        }

        match object
          .references
          .iter()
          .find(|reference| !self.objects.contains_key(*reference))
        {
          Some(absent) => return Err(StoreError::MissingReference(absent.clone())),
          None => {}
        }

        let _fresh = self.objects.insert(path.clone(), object);

        self.arrivals.push(path.clone());

        Ok(())
      }
    }
  }

  pub fn read(&self, path: &StorePath) -> Result<&Object, StoreError> {
    match self.objects.get(path) {
      Some(object) => Ok(object),
      None => Err(StoreError::NotFound(path.clone())),
    }
  }

  pub fn paths(&self) -> Result<BTreeSet<StorePath>, Never> {
    Ok(self.objects.keys().cloned().collect())
  }

  pub fn closure(&self, path: &StorePath) -> Result<BTreeSet<StorePath>, StoreError> {
    self.read(path)?;

    let Ok(reached) = self.reachable(&[path]);

    Ok(reached)
  }

  pub fn grants(
    &self,
    path: &StorePath,
  ) -> Result<impl Iterator<Item = Result<Grant, StoreError>> + '_, StoreError> {
    let closure = self.closure(path)?;

    Ok(closure.into_iter().map(|reached| {
      let object = self.read(&reached)?;

      let rights = match object.kind {
        Kind::Data => BTreeSet::from([Right::Read]),
        Kind::Program => BTreeSet::from([Right::Read, Right::Execute]),
      };

      Ok(Grant {
        path: reached,
        rights,
      })
    }))
  }

  pub fn retain(&mut self, path: &StorePath) -> Result<(), StoreError> {
    self.read(path)?;

    let counted = match self.roots.get(path) {
      Some(count) => match count.checked_add(1) {
        Some(counted) => counted,
        None => return Err(StoreError::TooManyRoots(path.clone())),
      },
      None => NonZeroU64::MIN,
    };

    let _before = self.roots.insert(path.clone(), counted);

    Ok(())
  }

  pub fn release(&mut self, path: &StorePath) -> Result<(), StoreError> {
    let count = match self.roots.get(path) {
      Some(count) => *count,
      None => return Err(StoreError::NotRetained(path.clone())),
    };

    let _before = match count.get().checked_sub(1).and_then(NonZeroU64::new) {
      Some(left) => self.roots.insert(path.clone(), left),
      None => self.roots.remove(path),
    };

    Ok(())
  }

  pub fn roots(&self) -> Result<BTreeSet<StorePath>, Never> {
    Ok(self.roots.keys().cloned().collect())
  }

  pub fn collect(&mut self) -> Result<Vec<StorePath>, Never> {
    let roots: Vec<&StorePath> = self.roots.keys().collect();
    let Ok(live) = self.reachable(&roots);
    let dead: Vec<StorePath> = self
      .objects
      .keys()
      .filter(|path| !live.contains(*path))
      .cloned()
      .collect();

    for path in &dead {
      let _removed = self.objects.remove(path);
    }

    self.arrivals.retain(|path| live.contains(path));

    Ok(dead)
  }

  fn reachable(&self, from: &[&StorePath]) -> Result<BTreeSet<StorePath>, Never> {
    let mut reached: BTreeSet<&StorePath> = from.iter().copied().collect();

    for path in self.arrivals.iter().rev() {
      match self.objects.get(path).filter(|_| reached.contains(path)) {
        Some(object) => reached.extend(object.references.iter()),
        None => {}
      }
    }

    Ok(reached.into_iter().cloned().collect())
  }
}

impl fmt::Display for StorePath {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in self.hash.0 {
      write!(to, "{byte:02x}")?;
    }

    write!(to, "-{}", self.name.0)
  }
}

impl fmt::Display for StoreError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str(match self {
      StoreError::InvalidName => {
        "a store path's name is letters, digits and +-._?= and does not begin with a dot"
      }
      StoreError::WrongHash(_) => "the object does not produce the hash its path is named by",
      StoreError::Immutable(_) => "the path is already in the store with other content",
      StoreError::MissingReference(_) => "the object refers to a path that is not in the store",
      StoreError::NotFound(_) => "there is no object at that path",
      StoreError::NotRetained(_) => "nothing is holding that path as a root",
      StoreError::TooManyRoots(_) => "the path is held as a root more times than can be counted",
    })
  }
}
