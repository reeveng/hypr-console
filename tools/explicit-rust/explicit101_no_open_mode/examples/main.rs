use std::fs::{DirBuilder, File, OpenOptions, Permissions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

fn for_everybody() -> std::io::Result<File> {
    //~v WARN EXPLICIT101_NO_OPEN_MODE
    OpenOptions::new().write(true).create_new(true).mode(0o666).open("settings")
}

fn because_it_would_not_open() -> Permissions {
    //~v WARN EXPLICIT101_NO_OPEN_MODE
    Permissions::from_mode(0o777)
}

fn as_its_owner() -> Permissions {
    //~v WARN EXPLICIT101_NO_OPEN_MODE
    Permissions::from_mode(0o4755)
}

fn a_shared_place() -> std::io::Result<()> {
    //~v WARN EXPLICIT101_NO_OPEN_MODE
    DirBuilder::new().mode(0o777).create("shared")
}

fn kept() -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).mode(0o600).open("settings")
}

fn read_by_others() -> Permissions {
    Permissions::from_mode(0o644)
}

fn shared_on_purpose() -> std::io::Result<()> {
    DirBuilder::new().mode(0o1777).create("shared")
}

struct Player;

impl Player {
    fn mode(&self, _which: u32) -> u8 {
        1
    }
}

fn main() {
    let _ = for_everybody();
    let _ = because_it_would_not_open();
    let _ = as_its_owner();
    let _ = a_shared_place();
    let _ = kept();
    let _ = read_by_others();
    let _ = shared_on_purpose();
    let _ = Player.mode(0o777);
}
