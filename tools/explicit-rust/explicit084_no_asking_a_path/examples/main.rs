use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;

fn asked_first(at: &Path) -> Option<File> {
    //~v WARN EXPLICIT084_NO_ASKING_A_PATH
    match at.exists() {
        true => File::open(at).ok(),
        false => None,
    }
}

fn asked_what_it_is(at: &Path) -> u8 {
    //~v WARN EXPLICIT084_NO_ASKING_A_PATH
    match at.is_dir() {
        true => 1,
        false => 0,
    }
}

fn asked_the_long_way(at: &Path) -> std::io::Result<bool> {
    //~v WARN EXPLICIT084_NO_ASKING_A_PATH
    std::fs::exists(at)
}

fn opened(at: &Path) -> std::io::Result<Option<File>> {
    match File::open(at) {
        Ok(held) => Ok(Some(held)),
        Err(fault) => match fault.kind() == ErrorKind::NotFound {
            true => Ok(None),
            false => Err(fault),
        },
    }
}

struct Shelf;

impl Shelf {
    fn exists(&self) -> u8 {
        1
    }
}

fn main() {
    let at = Path::new("somewhere");
    let _ = asked_first(at);
    let _ = asked_what_it_is(at);
    let _ = asked_the_long_way(at);
    let _ = opened(at);
    let _ = Shelf.exists();
}
