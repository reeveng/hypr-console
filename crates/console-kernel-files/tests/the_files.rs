use console_core_never::Never;
use console_kernel_files::{
  Collision, ContentHash, Digest, FileId, Files, FilesError, Operation, Path,
};

#[derive(Clone)]
struct Fnv;

impl Digest for Fnv {
  fn digest_parts<'a>(
    &self,
    parts: impl IntoIterator<Item = &'a [u8]>,
  ) -> Result<ContentHash, Never> {
    use console_core_checksums::fnv1a::{OFFSET64, update64};

    let hash = parts.into_iter().fold(OFFSET64, |state, part| {
      let Ok(next) = update64(state, part);

      next
    });

    let mut out = [0; 32];

    for (place, byte) in out.iter_mut().zip(hash.to_le_bytes()) {
      *place = byte;
    }

    Ok(ContentHash(out))
  }
}

fn empty() -> Result<Files<Fnv>, Never> {
  Files::new(Fnv)
}

fn at(path: &str) -> Result<Path, Never> {
  Ok(Path(path.to_string()))
}

fn create(file: FileId, path: &str, bytes: &[u8]) -> Result<Operation, Never> {
  let Ok(at) = at(path);

  Ok(Operation::Create {
    file,
    at,
    bytes: bytes.to_vec(),
  })
}

const ONE: FileId = FileId([1; 16]);
const TWO: FileId = FileId([2; 16]);
const THREE: FileId = FileId([3; 16]);

#[test]
fn a_rename_keeps_the_identity_and_its_history() -> Result<(), FilesError> {
  let Ok(files) = empty();
  let Ok(made) = create(ONE, "notes", b"first");
  let files_2 = files.apply(&[made])?;
  let from = files_2.head(ONE)?;
  let files_3 = files_2.apply(&[Operation::Modify {
    file: ONE,
    from,
    bytes: b"second".to_vec(),
  }])?;
  let before = files_3.head(ONE)?;
  let Ok(to) = at("letters/notes");
  let files_4 = files_3.apply(&[Operation::Rename {
    file: ONE,
    to: to.clone(),
  }])?;

  let path = files_4.path(ONE)?;
  assert_eq!(path, &to);
  let head = files_4.head(ONE)?;
  assert_eq!(head, before);
  let read_one = files_4.read(ONE)?;
  assert_eq!(read_one, b"second");
  let history = files_4.history(ONE)?;
  assert_eq!(history.len(), 2);

  Ok(())
}

#[test]
fn the_same_bytes_are_stored_once() -> Result<(), FilesError> {
  let Ok(files) = empty();
  let Ok(one) = create(ONE, "a", b"same");
  let Ok(two) = create(TWO, "b", b"same");
  let files_2 = files.apply(&[one, two])?;
  let Ok(stored) = files_2.hashes();

  assert_eq!(stored.len(), 1);
  let read_one = files_2.read(ONE)?;
  let read_two = files_2.read(TWO)?;
  assert_eq!(read_one, read_two);

  Ok(())
}

#[test]
fn two_identities_may_claim_one_path_and_it_is_reported() -> Result<(), FilesError> {
  let Ok(files) = empty();
  let Ok(one) = create(ONE, "shared", b"one");
  let Ok(two) = create(TWO, "shared", b"two");
  let Ok(three) = create(THREE, "alone", b"three");
  let files_2 = files.apply(&[one, two, three])?;
  let Ok(collisions) = files_2.collisions();
  let Ok(shared) = at("shared");

  assert_eq!(
    collisions,
    vec![Collision {
      at: shared,
      claimed_by: vec![ONE, TWO]
    }]
  );
  let read_one = files_2.read(ONE)?;
  assert_eq!(read_one, b"one");
  let read_two = files_2.read(TWO)?;
  assert_eq!(read_two, b"two");

  Ok(())
}

#[test]
fn a_change_that_fails_midway_changes_nothing() -> Result<(), FilesError> {
  let Ok(initial) = empty();
  let Ok(made) = create(ONE, "kept", b"kept");
  let files = initial.apply(&[made])?;
  let from = files.head(ONE)?;
  let Ok(moved) = at("moved");
  let Ok(added) = create(TWO, "added", b"added");

  let failed = files.apply(&[
    Operation::Rename {
      file: ONE,
      to: moved,
    },
    added,
    Operation::Modify {
      file: ONE,
      from,
      bytes: b"changed".to_vec(),
    },
    Operation::Delete { file: THREE },
  ]);

  assert!(matches!(failed, Err(FilesError::NotFound(THREE))));
  let Ok(kept) = at("kept");
  let path = files.path(ONE)?;
  assert_eq!(path, &kept);
  let read_one = files.read(ONE)?;
  assert_eq!(read_one, b"kept");
  assert_eq!(files.head(TWO), Err(FilesError::NotFound(TWO)));
  let Ok(stored) = files.hashes();
  assert_eq!(stored.len(), 1);

  Ok(())
}

#[test]
fn a_change_made_against_an_old_version_is_refused() -> Result<(), FilesError> {
  let Ok(files) = empty();
  let Ok(made) = create(ONE, "notes", b"first");
  let files_2 = files.apply(&[made])?;
  let from = files_2.head(ONE)?;
  let files_3 = files_2.apply(&[Operation::Modify {
    file: ONE,
    from,
    bytes: b"second".to_vec(),
  }])?;

  let stale = files_3.apply(&[Operation::Modify {
    file: ONE,
    from,
    bytes: b"third".to_vec(),
  }]);

  assert!(matches!(stale, Err(FilesError::StaleVersion(ONE))));

  Ok(())
}

#[test]
fn history_walks_parents_back_to_the_first_version_and_outlives_a_delete()
-> Result<(), FilesError> {
  let Ok(initial) = empty();
  let Ok(made) = create(ONE, "notes", b"first");
  let mut files = initial.apply(&[made])?;

  for bytes in [b"second".as_slice(), b"third".as_slice()] {
    let from = files.head(ONE)?;

    let changed = files.apply(&[Operation::Modify {
      file: ONE,
      from,
      bytes: bytes.to_vec(),
    }])?;

    files = changed;
  }

  let removed = files.apply(&[Operation::Delete { file: ONE }])?;
  let history = removed.history(ONE)?;
  let mut read = Vec::new();

  for version in &history {
    let content = removed.content(version.content)?;
    read.push(content);
  }

  assert_eq!(
    read,
    vec![
      b"third".as_slice(),
      b"second".as_slice(),
      b"first".as_slice()
    ]
  );
  let parents: Vec<_> = history.iter().map(|version| version.parent).collect();
  let ids: Vec<_> = history
    .iter()
    .skip(1)
    .map(|version| Some(version.id))
    .chain([None])
    .collect();
  assert_eq!(parents, ids);
  assert_eq!(removed.read(ONE), Err(FilesError::Deleted(ONE)));

  Ok(())
}
