//! Observable milestones for one panel request.
//!
//! Accepted means the host recognized the request and owns its execution.
//! Drawn means the compositor confirmed presentation of that request's frame.
//! A failed or discarded drawing never produces Drawn. Each request has its own
//! socket; an optional file belongs to its requester and records the same
//! milestones, so a check can wait without guessing from timing measurements.
//! `handoff::request` delivers them directly to a callback registered before
//! sending the request. Stand-ins can instead set CONSOLE_PANEL_COMPLETION
//! to a fresh path for this request and observe one milestone per line there.

use std::io::Write;
use std::path::{Path, PathBuf};

use console_core_never::Never;

pub const ENVIRONMENT: &str = "CONSOLE_PANEL_COMPLETION";

pub const ACCEPTED: &str = "accepted";

pub const DRAWN: &str = "drawn";

pub const FAILED: &str = "failed";

pub const GONE: &str = "gone";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
  Accepted,
  Visible,
  Failed,
  Gone,
}

impl Progress {
  #[must_use]
  pub fn word(self) -> Result<&'static str, Never> {
    Ok(match self {
      Progress::Accepted => ACCEPTED,
      Progress::Visible => DRAWN,
      Progress::Failed => FAILED,
      Progress::Gone => GONE,
    })
  }

  #[must_use]
  pub fn read(word: &str) -> Result<Option<Self>, Never> {
    Ok(match word {
      ACCEPTED => Some(Progress::Accepted),
      DRAWN => Some(Progress::Visible),
      FAILED => Some(Progress::Failed),
      GONE => Some(Progress::Gone),
      _unknown => None,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  allow(
    explicit026_env_read_once,
    reason = "the panel completion protocol owns the optional per-request observation file"
  )
)]
pub fn where_to() -> Result<Option<PathBuf>, Never> {
  Ok(
    std::env::var_os(ENVIRONMENT)
      .filter(|value| !value.is_empty())
      .map(PathBuf::from),
  )
}

pub fn record(at: Option<&Path>, progress: Progress) -> std::io::Result<()> {
  let destination = match at {
    Some(path) => path,
    None => return Ok(()),
  };
  let Ok(word) = progress.word();

  #[cfg_attr(
    dylint_lib = "explicit040_no_torn_write",
    allow(
      explicit040_no_torn_write,
      reason = "milestones are appended by one requester after their events arrive; replacing the file would erase earlier milestones"
    )
  )]
  let mut file = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(destination)?;

  file.write_all(format!("{word}\n").as_bytes())
}
