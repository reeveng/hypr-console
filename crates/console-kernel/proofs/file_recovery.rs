//! Two-entry copy-on-write filesystem model. Persistence of the inactive
//! root precedes a durable atomic selector update. Recovery uses that selector.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct File {
  id: FileId,
  name: u16,
  contents: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Catalog {
  files: [Option<File>; 2],
  next: FileId,
}

#[derive(Clone, Copy)]
enum Change {
  Rename(u16),
  Write(u64),
  Delete,
}

impl Catalog {
  fn change(mut self, id: FileId, change: Change) -> Option<Self> {
    let slot = self
      .files
      .iter_mut()
      .find(|slot| slot.is_some_and(|file| file.id == id))?;
    let mut file = (*slot)?;
    *slot = match change {
      Change::Rename(name) => {
        file.name = name;
        Some(file)
      }
      Change::Write(contents) => {
        file.contents = contents;
        Some(file)
      }
      Change::Delete => None,
    };
    Some(self)
  }

  fn create(mut self, name: u16, contents: u64) -> Option<(Self, FileId)> {
    let id = self.next;
    let next = FileId(id.0.checked_add(1)?);
    let free = self.files.iter_mut().find(|entry| entry.is_none())?;
    *free = Some(File { id, name, contents });
    self.next = next;
    Some((self, id))
  }
}

#[derive(Clone, Copy)]
enum OtherRoot {
  Empty,
  Partial,
  Durable(Catalog),
}

#[derive(Clone, Copy)]
enum Selected {
  First,
  Second,
}

struct Media {
  first: OtherRoot,
  second: OtherRoot,
  selected: Selected,
}

struct Written<'a>(&'a mut Media);

impl Media {
  fn with_root(selected: Catalog) -> Self {
    Self {
      first: OtherRoot::Durable(selected),
      second: OtherRoot::Empty,
      selected: Selected::First,
    }
  }

  fn recover(&self) -> Option<Catalog> {
    let published = match self.selected {
      Selected::First => self.first,
      Selected::Second => self.second,
    };
    match published {
      OtherRoot::Durable(root) => Some(root),
      OtherRoot::Empty | OtherRoot::Partial => None,
    }
  }

  fn inactive(&mut self) -> &mut OtherRoot {
    match self.selected {
      Selected::First => &mut self.second,
      Selected::Second => &mut self.first,
    }
  }

  fn begin(&mut self) {
    *self.inactive() = OtherRoot::Partial;
  }

  fn persist(&mut self, next: Catalog) -> Written<'_> {
    *self.inactive() = OtherRoot::Durable(next);
    Written(self)
  }
}

impl Written<'_> {
  fn publish(self) {
    self.0.selected = match self.0.selected {
      Selected::First => Selected::Second,
      Selected::Second => Selected::First,
    };
  }
}

fn two_files() -> Catalog {
  let next: u64 = kani::any();
  let first: u64 = kani::any();
  let second: u64 = kani::any();
  kani::assume(first != second && first < next && second < next);
  let shared_name = kani::any();
  Catalog {
    files: [
      Some(File {
        id: FileId(first),
        name: shared_name,
        contents: kani::any(),
      }),
      Some(File {
        id: FileId(second),
        name: shared_name,
        contents: kani::any(),
      }),
    ],
    next: FileId(next),
  }
}

#[kani::proof]
#[kani::unwind(5)]
fn file_recovery_at_each_commit_cut_keeps_a_whole_catalog_and_stable_identity() {
  let old = two_files();
  let name = kani::any();
  let bytes = kani::any();
  let change = match kani::any::<u8>() % 3 {
    0 => Change::Rename(name),
    1 => Change::Write(bytes),
    _ => Change::Delete,
  };
  let target_first: bool = kani::any();
  let [first, second] = old.files;
  let original = if target_first {
    first.unwrap()
  } else {
    second.unwrap()
  };
  let next = old.change(original.id, change).unwrap();
  let updated = match change {
    Change::Rename(name) => Some(File { name, ..original }),
    Change::Write(contents) => Some(File {
      contents,
      ..original
    }),
    Change::Delete => None,
  };
  let expected = Catalog {
    files: if target_first {
      [updated, second]
    } else {
      [first, updated]
    },
    next: old.next,
  };
  let cut: u8 = kani::any();
  let mut media = Media::with_root(old);

  if cut != 0 {
    media.begin();
  }
  if cut > 1 {
    let ready = media.persist(next);
    if cut > 2 {
      ready.publish();
    }
  }

  let recovered = media.recover();
  assert_eq!(recovered, Some(if cut > 2 { expected } else { old }));
  kani::cover!(cut == 2);
  kani::cover!(cut > 2 && matches!(change, Change::Delete));
}

#[kani::proof]
#[kani::unwind(5)]
fn a_deleted_file_id_stays_retired_after_recovery_and_name_reuse() {
  let old = two_files();
  let [first, second] = old.files;
  let removed = first.unwrap();
  let mut media = Media::with_root(old);
  media.begin();
  media
    .persist(old.change(removed.id, Change::Delete).unwrap())
    .publish();
  let recovered = media.recover().unwrap();

  match recovered.create(removed.name, kani::any()) {
    Some((new, id)) => {
      assert_eq!(id, old.next);
      assert!(id.0 > removed.id.0 && id.0 > second.unwrap().id.0);
      assert_eq!(new.files[1], second);
      assert!(new.change(removed.id, Change::Delete).is_none());
      media.begin();
      assert_eq!(media.recover(), Some(recovered));
      media.persist(new).publish();
      assert_eq!(media.recover(), Some(new));
    }
    None => assert_eq!(old.next.0, u64::MAX),
  }
}
