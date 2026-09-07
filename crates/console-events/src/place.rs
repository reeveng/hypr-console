//! Where the pool is, said once.

use std::path::PathBuf;

pub fn socket() -> Result<PathBuf, String> {
    let run = std::env::var("XDG_RUNTIME_DIR")
        .map_err(|fault| format!("XDG_RUNTIME_DIR: {fault}"))?;

    Ok(PathBuf::from(run).join("console").join("events.sock"))
}
