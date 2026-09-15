//! Where the pool is, said once.

use std::path::PathBuf;

use crate::Unserved;

pub fn socket() -> Result<PathBuf, Unserved> {
    let Ok(ours) = console_core_places::runtime_ours();

    let ours = match ours {
        Some(ours) => ours,
        None => return Err(Unserved::Sessionless),
    };

    Ok(ours.join("events.sock"))
}
