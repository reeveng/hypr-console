use std::fs::{File, OpenOptions, Permissions};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

fn narrowed_afterwards(at: &Path) -> std::io::Result<File> {
    let made = File::create(at)?;

    //~v WARN EXPLICIT102_NO_MODE_AFTER_MAKING
    std::fs::set_permissions(at, Permissions::from_mode(0o600))?;

    Ok(made)
}

fn made_as_meant(at: &Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).mode(0o600).open(at)
}

fn changed_on_the_thing(held: &File) -> std::io::Result<()> {
    held.set_permissions(Permissions::from_mode(0o644))
}

fn main() {
    let at = Path::new("kept");
    let _ = narrowed_afterwards(at);
    let _ = made_as_meant(at).map(|held| changed_on_the_thing(&held));
}
