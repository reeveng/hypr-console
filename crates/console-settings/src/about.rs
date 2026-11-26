//! The installed Console build and basic information about its host.

use std::path::Path;

use console_core_never::Never;
use console_manifest_engine::generations::{self, Generation};
use console_panel::page::{Aside, Row};

pub fn rows() -> Result<Vec<Row>, Never> {
  let Ok(heading) = Row::naming("About Console", Aside(""));
  let generation = match generations::read(Path::new(generations::KEPT)) {
    Ok(records) => {
      let Ok(newest) = generations::newest(&records);
      newest.cloned()
    }
    Err(fault) => {
      console_core_logging::warn!("settings-panel: reading installed Console build: {fault}");
      None
    }
  };
  let Ok(build) = build_label(generation.as_ref());
  let Ok(console) = Row::text("Console build", Aside(&build));
  let Ok(system) = operating_system();
  let Ok(system_row) = Row::text("Operating system", Aside(&system));
  let Ok(kernel) = fact(Path::new("/proc/sys/kernel/osrelease"));
  let Ok(kernel_row) = Row::text("Kernel", Aside(&kernel));
  let Ok(hardware) = fact(Path::new("/sys/devices/virtual/dmi/id/product_version"));
  let Ok(hardware_row) = Row::text("Hardware model", Aside(&hardware));

  Ok(vec![heading, console, system_row, kernel_row, hardware_row])
}

fn build_label(generation: Option<&Generation>) -> Result<String, Never> {
  let Ok(root) = console_repository::installed_root();
  let marker = root.join(console_repository::RELEASE_MARKER);
  let Ok(record) = console_core_atomic_writes::read(&marker);
  let release = match record {
    console_core_atomic_writes::Stored::Text(text) => {
      let Ok(tag) = release_tag(&text, generation);
      tag.map(str::to_owned)
    }
    console_core_atomic_writes::Stored::Absent => None,
    console_core_atomic_writes::Stored::Failed(fault) => {
      console_core_logging::warn!("settings-panel: reading Console release record: {fault}");
      None
    }
  };

  Ok(match (release, generation) {
    (Some(tag), _) => tag,
    (None, Some(installed)) => match installed.state {
      generations::State::Finished => format!("Local build · {}", installed.commit),
      generations::State::Started => format!("Update incomplete · {}", installed.commit),
    },
    (None, None) => "Version unrecorded".to_owned(),
  })
}

fn release_tag<'a>(
  record: &'a str,
  generation: Option<&Generation>,
) -> Result<Option<&'a str>, Never> {
  let mut lines = record.lines();
  let (tag, commit) = match (lines.next(), lines.next()) {
    (Some(tag), Some(commit)) => (tag, commit),
    _ => return Ok(None),
  };

  let valid_commit = commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit());
  let matching_build = generation.is_none_or(|held| {
    held.state == generations::State::Finished
      && !held.commit.is_empty()
      && commit.starts_with(&held.commit)
  });

  Ok(
    ((tag.starts_with("console-v") || tag.starts_with("legion-go-v"))
      && valid_commit
      && matching_build)
      .then_some(tag),
  )
}

fn operating_system() -> Result<String, Never> {
  let fields = match std::fs::read_to_string("/etc/os-release") {
    Ok(fields) => fields,
    Err(fault) => {
      console_core_logging::warn!("settings-panel: reading operating system name: {fault}");
      return Ok("Unknown".to_owned());
    }
  };
  let name = fields
    .lines()
    .find_map(|line| line.strip_prefix("PRETTY_NAME="));

  Ok(
    name
      .map_or("Unknown", |value| value.trim_matches('"'))
      .to_owned(),
  )
}

fn fact(at: &Path) -> Result<String, Never> {
  let Ok(held) = console_core_atomic_writes::read(at);

  Ok(match held {
    console_core_atomic_writes::Stored::Text(text) => text.trim().to_owned(),
    console_core_atomic_writes::Stored::Absent => "Unknown".to_owned(),
    console_core_atomic_writes::Stored::Failed(fault) => {
      console_core_logging::warn!("settings-panel: reading {}: {fault}", at.display());
      "Unknown".to_owned()
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_release_marker_must_match_the_finished_generation() {
    let marker = "console-v0.1.8\n1234567890123456789012345678901234567890\n";
    let Ok(pending) = generations::next(&[], generations::Commit("1234567"), None);
    let Ok(installed) = pending.finished();
    let Ok(later) = generations::next(&[], generations::Commit("9876543"), None);
    let Ok(other) = later.finished();

    assert_eq!(release_tag(marker, None), Ok(Some("console-v0.1.8")));
    assert_eq!(
      release_tag(marker, Some(&installed)),
      Ok(Some("console-v0.1.8"))
    );
    assert_eq!(release_tag(marker, Some(&pending)), Ok(None));
    assert_eq!(release_tag(marker, Some(&other)), Ok(None));
  }
}
