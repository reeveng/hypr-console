use std::collections::BTreeSet;

use console_core_never::Never;
use console_kernel_files::{ContentHash, Digest};
use console_kernel_software::{Grant, Kind, Name, Object, Right, Store, StoreError, StorePath};

#[derive(Clone)]
struct Hashed;

impl Digest for Hashed {
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

fn empty() -> Result<Store<Hashed>, Never> {
  Store::new(Hashed)
}

fn object(kind: Kind, content: &[u8], references: &[&StorePath]) -> Result<Object, Never> {
  Ok(Object {
    kind,
    references: references
      .iter()
      .map(|reference| (*reference).clone())
      .collect(),
    content: content.to_vec(),
  })
}

fn put(store: &mut Store<Hashed>, named: &str, object: Object) -> Result<StorePath, StoreError> {
  let name = Name::new(named.to_string())?;
  let Ok(path) = store.path(&name, &object);

  store.add(&path, object)?;

  Ok(path)
}

fn data(
  store: &mut Store<Hashed>,
  named: &str,
  references: &[&StorePath],
) -> Result<StorePath, StoreError> {
  let Ok(made) = object(Kind::Data, named.as_bytes(), references);

  put(store, named, made)
}

fn program(
  store: &mut Store<Hashed>,
  named: &str,
  references: &[&StorePath],
) -> Result<StorePath, StoreError> {
  let Ok(made) = object(Kind::Program, named.as_bytes(), references);

  put(store, named, made)
}

fn set(paths: &[&StorePath]) -> Result<BTreeSet<StorePath>, Never> {
  Ok(paths.iter().map(|path| (*path).clone()).collect())
}

#[test]
fn a_path_is_spelled_as_its_hash_then_its_name() -> Result<(), StoreError> {
  let name = Name::new("hello-2.12".to_string())?;
  let path = StorePath {
    hash: ContentHash([0xab; 32]),
    name,
  };

  assert_eq!(path.to_string(), format!("{}-hello-2.12", "ab".repeat(32)));

  Ok(())
}

#[test]
fn a_fingerprint_hashes_the_same_bytes_across_field_boundaries() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let library = data(&mut store, "lib", &[])?;
  let name = Name::new("tool".to_string())?;
  let Ok(program) = object(Kind::Program, b"body", &[&library]);
  let Ok(path) = store.path(&name, &program);
  let Ok(content) = Hashed.digest(b"body");
  let legacy: [&[u8]; 4] = [
    b"software\0program\0tool\0",
    &library.hash.0,
    b"lib\0\0",
    &content.0,
  ];
  let Ok(expected) = Hashed.digest(&legacy.concat());

  assert_eq!(path.hash, expected);

  Ok(())
}

#[test]
fn a_name_that_could_not_be_part_of_a_path_is_refused() -> Result<(), StoreError> {
  assert_eq!(Name::new("".to_string()), Err(StoreError::InvalidName));
  assert_eq!(
    Name::new("bin/sh".to_string()),
    Err(StoreError::InvalidName)
  );
  assert_eq!(
    Name::new(".hidden".to_string()),
    Err(StoreError::InvalidName)
  );
  assert_eq!(
    Name::new("two words".to_string()),
    Err(StoreError::InvalidName)
  );
  let allowed = Name::new("gcc-13.2.0+lib_x?y=z".to_string())?;
  let Ok(spelled) = allowed.as_str();
  assert_eq!(spelled, "gcc-13.2.0+lib_x?y=z");

  Ok(())
}

#[test]
fn adding_the_same_object_twice_is_one_object() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let first = data(&mut store, "libc", &[])?;
  let second = data(&mut store, "libc", &[])?;
  let Ok(paths) = store.paths();
  let Ok(only) = set(&[&first]);

  assert_eq!(first, second);
  assert_eq!(paths, only);

  Ok(())
}

#[test]
fn a_path_in_the_store_never_changes_what_it_holds() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let path = data(&mut store, "libc", &[])?;
  let Ok(other) = object(Kind::Data, b"something else", &[]);

  let changed = store.add(&path, other);

  assert_eq!(changed, Err(StoreError::Immutable(path.clone())));
  let held = store.read(&path)?;
  assert_eq!(held.content, b"libc");

  Ok(())
}

#[test]
fn an_object_is_refused_until_everything_it_refers_to_is_there() -> Result<(), StoreError> {
  let Ok(mut elsewhere) = empty();
  let libc = data(&mut elsewhere, "libc", &[])?;
  let Ok(mut store) = empty();

  let early = program(&mut store, "shell", &[&libc]);

  assert_eq!(early, Err(StoreError::MissingReference(libc.clone())));
  let Ok(paths) = store.paths();
  assert!(paths.is_empty());
  let added = data(&mut store, "libc", &[])?;
  assert_eq!(added, libc);
  let shell = program(&mut store, "shell", &[&libc])?;
  let held = store.read(&shell)?;
  let Ok(references) = set(&[&libc]);
  assert_eq!(held.references, references);

  Ok(())
}

#[test]
fn a_path_its_object_does_not_produce_is_refused() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let libc = data(&mut store, "libc", &[])?;
  let name = Name::new("shell".to_string())?;
  let Ok(honest) = object(Kind::Data, b"shell", &[&libc]);
  let Ok(path) = store.path(&name, &honest);
  let Ok(other_content) = object(Kind::Data, b"a different shell", &[&libc]);
  let Ok(other_kind) = object(Kind::Program, b"shell", &[&libc]);
  let Ok(other_references) = object(Kind::Data, b"shell", &[]);
  let login = Name::new("login".to_string())?;
  let renamed = StorePath {
    hash: path.hash,
    name: login,
  };

  for forged in [other_content, other_kind, other_references] {
    let refused = store.add(&path, forged);

    assert_eq!(refused, Err(StoreError::WrongHash(path.clone())));
  }

  let refused = store.add(&renamed, honest.clone());
  assert_eq!(refused, Err(StoreError::WrongHash(renamed.clone())));
  assert_eq!(store.read(&path), Err(StoreError::NotFound(path.clone())));
  let Ok(paths) = store.paths();
  let Ok(only) = set(&[&libc]);
  assert_eq!(paths, only);
  store.add(&path, honest)?;

  Ok(())
}

#[test]
fn the_closure_of_a_diamond_reaches_the_shared_object_once() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let libc = data(&mut store, "libc", &[])?;
  let parser = data(&mut store, "parser", &[&libc])?;
  let widgets = data(&mut store, "widgets", &[&libc])?;
  let shell = program(&mut store, "shell", &[&parser, &widgets])?;
  let unrelated = data(&mut store, "fonts", &[])?;

  let closure = store.closure(&shell)?;

  let Ok(diamond) = set(&[&libc, &parser, &widgets, &shell]);
  assert_eq!(closure, diamond);
  let side = store.closure(&parser)?;
  let Ok(half) = set(&[&libc, &parser]);
  assert_eq!(side, half);
  let alone = store.closure(&unrelated)?;
  let Ok(itself) = set(&[&unrelated]);
  assert_eq!(alone, itself);

  Ok(())
}

#[test]
fn the_closure_of_a_long_chain_is_the_whole_chain() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let bottom = data(&mut store, "link-0", &[])?;
  let mut chain = vec![bottom];

  for step in 1..20_000_u32 {
    let below: Vec<&StorePath> = chain.last().into_iter().collect();
    let next = data(&mut store, &format!("link-{step}"), &below)?;

    chain.push(next);
  }

  let (top, middle) = match (chain.last(), chain.get(7_000)) {
    (Some(top), Some(middle)) => (top, middle),
    (Some(_) | None, Some(_) | None) => return Err(StoreError::InvalidName),
  };

  let whole = store.closure(top)?;
  let part = store.closure(middle)?;

  assert_eq!(whole, chain.iter().cloned().collect());
  assert_eq!(part, chain.iter().take(7_001).cloned().collect());

  Ok(())
}

#[test]
fn the_closure_of_a_path_that_is_not_there_is_a_fault() -> Result<(), StoreError> {
  let Ok(mut elsewhere) = empty();
  let absent = data(&mut elsewhere, "libc", &[])?;
  let Ok(store) = empty();

  assert_eq!(
    store.closure(&absent),
    Err(StoreError::NotFound(absent.clone()))
  );
  assert_eq!(
    store.grants(&absent).map(|_grants| ()),
    Err(StoreError::NotFound(absent))
  );

  Ok(())
}

#[test]
fn collecting_keeps_everything_a_root_reaches_and_takes_the_rest() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let libc = data(&mut store, "libc", &[])?;
  let parser = data(&mut store, "parser", &[&libc])?;
  let shell = program(&mut store, "shell", &[&parser])?;
  let fonts = data(&mut store, "fonts", &[])?;
  let editor = program(&mut store, "editor", &[&libc, &fonts])?;
  let game = program(&mut store, "game", &[&shell])?;
  store.retain(&shell)?;

  let Ok(went) = store.collect();

  let Ok(taken) = set(&[&fonts, &editor, &game]);
  assert_eq!(went.iter().cloned().collect::<BTreeSet<_>>(), taken);
  assert_eq!(went.len(), taken.len());
  let Ok(left) = store.paths();
  let Ok(kept) = set(&[&libc, &parser, &shell]);
  assert_eq!(left, kept);
  let closure = store.closure(&shell)?;
  assert_eq!(closure, kept);
  let Ok(again) = store.collect();
  assert!(again.is_empty());

  Ok(())
}

#[test]
fn what_is_left_after_collecting_still_refers_only_to_what_is_there() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let libc = data(&mut store, "libc", &[])?;
  let parser = data(&mut store, "parser", &[&libc])?;
  let shell = program(&mut store, "shell", &[&parser, &libc])?;
  let _editor = program(&mut store, "editor", &[&parser])?;
  let _game = program(&mut store, "game", &[&shell])?;
  store.retain(&parser)?;

  let Ok(went) = store.collect();

  assert_eq!(went.len(), 3);
  let Ok(left) = store.paths();

  for path in &left {
    let held = store.read(path)?;

    assert!(held.references.is_subset(&left));
  }

  Ok(())
}

#[test]
fn a_path_retained_twice_stays_until_it_is_released_twice() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let libc = data(&mut store, "libc", &[])?;
  let shell = program(&mut store, "shell", &[&libc])?;
  store.retain(&shell)?;
  store.retain(&shell)?;

  store.release(&shell)?;
  let Ok(went) = store.collect();
  assert!(went.is_empty());
  let Ok(roots) = store.roots();
  let Ok(held) = set(&[&shell]);
  assert_eq!(roots, held);

  store.release(&shell)?;
  let Ok(went_after) = store.collect();
  assert_eq!(went_after.len(), 2);
  let Ok(left) = store.paths();
  assert!(left.is_empty());
  assert_eq!(
    store.release(&shell),
    Err(StoreError::NotRetained(shell.clone()))
  );
  assert_eq!(store.retain(&shell), Err(StoreError::NotFound(shell)));

  Ok(())
}

#[test]
fn a_program_is_granted_its_closure_and_nothing_else() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let libc = data(&mut store, "libc", &[])?;
  let interpreter = program(&mut store, "interpreter", &[&libc])?;
  let script = data(&mut store, "script", &[])?;
  let tool = program(&mut store, "tool", &[&interpreter, &script])?;
  let _secrets = data(&mut store, "secrets", &[])?;
  let _other = program(&mut store, "other", &[&libc])?;
  let _wrapper = program(&mut store, "wrapper", &[&tool])?;

  let grants = store.grants(&tool)?;
  let granted = grants.collect::<Result<Vec<_>, _>>()?;

  let read = BTreeSet::from([Right::Read]);
  let run = BTreeSet::from([Right::Read, Right::Execute]);
  let mut expected = vec![
    Grant {
      path: libc,
      rights: read.clone(),
    },
    Grant {
      path: interpreter,
      rights: run.clone(),
    },
    Grant {
      path: script,
      rights: read,
    },
    Grant {
      path: tool.clone(),
      rights: run,
    },
  ];
  expected.sort_by(|one, other| one.path.cmp(&other.path));
  assert_eq!(granted, expected);
  let closure = store.closure(&tool)?;
  assert_eq!(
    granted
      .iter()
      .map(|grant| grant.path.clone())
      .collect::<BTreeSet<_>>(),
    closure
  );

  Ok(())
}

#[test]
fn data_is_never_granted_to_be_run() -> Result<(), StoreError> {
  let Ok(mut store) = empty();
  let libc = data(&mut store, "libc", &[])?;
  let fonts = data(&mut store, "fonts", &[&libc])?;

  let grants = store.grants(&fonts)?;
  let granted = grants.collect::<Result<Vec<_>, _>>()?;

  assert_eq!(granted.len(), 2);
  assert!(
    granted
      .iter()
      .all(|grant| !grant.rights.contains(&Right::Execute))
  );

  Ok(())
}
