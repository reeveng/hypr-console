use console_core_never::Never;
use console_kernel_files::{Collision, ContentHash, Digest, FileId, Files, FilesError, Operation, Path};

#[derive(Clone)]
struct Fnv;

impl Digest for Fnv {
    fn digest(&self, bytes: &[u8]) -> Result<ContentHash, Never> {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;

        for byte in bytes {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
        }

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
    Ok(Path(path.into()))
}

fn create(file: FileId, path: &str, bytes: &[u8]) -> Result<Operation, Never> {
    let Ok(at) = at(path);

    Ok(Operation::Create { file, at, bytes: bytes.to_vec() })
}

const ONE: FileId = FileId([1; 16]);
const TWO: FileId = FileId([2; 16]);
const THREE: FileId = FileId([3; 16]);

#[test]
fn a_rename_keeps_the_identity_and_its_history() -> Result<(), FilesError> {
    let Ok(files) = empty();
    let Ok(made) = create(ONE, "notes", b"first");
    let files = files.apply(&[made])?;
    let from = files.head(ONE)?;
    let files = files.apply(&[Operation::Modify { file: ONE, from, bytes: b"second".to_vec() }])?;
    let before = files.head(ONE)?;
    let Ok(to) = at("letters/notes");
    let files = files.apply(&[Operation::Rename { file: ONE, to: to.clone() }])?;

    assert_eq!(files.path(ONE)?, &to);
    assert_eq!(files.head(ONE)?, before);
    assert_eq!(files.read(ONE)?, b"second");
    assert_eq!(files.history(ONE)?.len(), 2);

    Ok(())
}

#[test]
fn the_same_bytes_are_stored_once() -> Result<(), FilesError> {
    let Ok(files) = empty();
    let Ok(one) = create(ONE, "a", b"same");
    let Ok(two) = create(TWO, "b", b"same");
    let files = files.apply(&[one, two])?;
    let Ok(stored) = files.stored();

    assert_eq!(stored.len(), 1);
    assert_eq!(files.read(ONE)?, files.read(TWO)?);

    Ok(())
}

#[test]
fn two_identities_may_claim_one_path_and_it_is_reported() -> Result<(), FilesError> {
    let Ok(files) = empty();
    let Ok(one) = create(ONE, "shared", b"one");
    let Ok(two) = create(TWO, "shared", b"two");
    let Ok(three) = create(THREE, "alone", b"three");
    let files = files.apply(&[one, two, three])?;
    let Ok(collisions) = files.collisions();
    let Ok(shared) = at("shared");

    assert_eq!(collisions, vec![Collision { at: shared, claimed_by: vec![ONE, TWO] }]);
    assert_eq!(files.read(ONE)?, b"one");
    assert_eq!(files.read(TWO)?, b"two");

    Ok(())
}

#[test]
fn a_change_that_fails_midway_changes_nothing() -> Result<(), FilesError> {
    let Ok(files) = empty();
    let Ok(made) = create(ONE, "kept", b"kept");
    let files = files.apply(&[made])?;
    let from = files.head(ONE)?;
    let Ok(moved) = at("moved");
    let Ok(added) = create(TWO, "added", b"added");

    let failed = files.apply(&[
        Operation::Rename { file: ONE, to: moved },
        added,
        Operation::Modify { file: ONE, from, bytes: b"changed".to_vec() },
        Operation::Delete { file: THREE },
    ]);

    assert_eq!(failed.err(), Some(FilesError::NotFound(THREE)));
    let Ok(kept) = at("kept");
    assert_eq!(files.path(ONE)?, &kept);
    assert_eq!(files.read(ONE)?, b"kept");
    assert_eq!(files.head(TWO), Err(FilesError::NotFound(TWO)));
    let Ok(stored) = files.stored();
    assert_eq!(stored.len(), 1);

    Ok(())
}

#[test]
fn a_change_made_against_an_old_version_is_refused() -> Result<(), FilesError> {
    let Ok(files) = empty();
    let Ok(made) = create(ONE, "notes", b"first");
    let files = files.apply(&[made])?;
    let from = files.head(ONE)?;
    let files = files.apply(&[Operation::Modify { file: ONE, from, bytes: b"second".to_vec() }])?;

    let stale = files.apply(&[Operation::Modify { file: ONE, from, bytes: b"third".to_vec() }]);

    assert_eq!(stale.err(), Some(FilesError::StaleVersion(ONE)));

    Ok(())
}

#[test]
fn history_walks_parents_back_to_the_first_version_and_outlives_a_delete() -> Result<(), FilesError> {
    let Ok(files) = empty();
    let Ok(made) = create(ONE, "notes", b"first");
    let mut files = files.apply(&[made])?;

    for bytes in [b"second".as_slice(), b"third".as_slice()] {
        let from = files.head(ONE)?;

        files = files.apply(&[Operation::Modify { file: ONE, from, bytes: bytes.to_vec() }])?;
    }

    let files = files.apply(&[Operation::Delete { file: ONE }])?;
    let history = files.history(ONE)?;
    let mut read = Vec::new();

    for version in &history {
        read.push(files.content(version.content)?);
    }

    assert_eq!(read, vec![b"third".as_slice(), b"second".as_slice(), b"first".as_slice()]);
    let parents: Vec<_> = history.iter().map(|version| version.parent).collect();
    let ids: Vec<_> = history.iter().skip(1).map(|version| Some(version.id)).chain([None]).collect();
    assert_eq!(parents, ids);
    assert_eq!(files.read(ONE), Err(FilesError::Deleted(ONE)));

    Ok(())
}
