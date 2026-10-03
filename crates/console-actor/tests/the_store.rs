//! A key in, the bytes it was last set to out, and nothing outside the folder.

use console_actor::{KeyValueStore, PlatformError};
use console_core_temporary_directories::fresh;

type Failure = Box<dyn std::error::Error>;

#[test]
fn what_is_set_is_what_is_got_until_it_is_removed() -> Result<(), Failure> {
    let directory = fresh("store-round")?;
    let Ok(store) = KeyValueStore::file_system(directory.join("kept"));

    let unset = store.get("books")?;

    assert_eq!(unset, None, "a key nobody set answered something");

    store.set("books", b"page 412")?;

    let set = store.get("books")?;

    assert_eq!(set, Some(b"page 412".to_vec()));

    store.remove("books")?;

    let removed = store.get("books")?;

    assert_eq!(removed, None, "a removed key still answered");

    Ok(())
}

#[test]
fn a_key_is_a_name_and_never_a_path() -> Result<(), Failure> {
    let directory = fresh("store-names")?;
    let Ok(store) = KeyValueStore::file_system(directory.join("kept"));

    for key in ["../escaped", "a/b", "", ".", "..", "/etc/passwd", "trailing/"] {
        assert!(matches!(store.set(key, b"x"), Err(PlatformError::BadArgument(_))), "{key:?} was taken as a key");
    }

    Ok(())
}

#[test]
fn a_key_that_will_not_be_read_is_a_fault_and_not_an_empty_store() -> Result<(), Failure> {
    let directory = fresh("store-unreadable")?;

    std::fs::create_dir_all(directory.join("books"))?;

    let Ok(store) = KeyValueStore::file_system(directory);

    assert!(matches!(store.get("books"), Err(PlatformError::Get(_, _))), "an unreadable key read as one nobody set");

    Ok(())
}
