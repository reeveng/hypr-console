use std::fmt;
use std::fs::File;
use std::io;
use std::process::{ExitCode, ExitStatus};
use std::time::Duration;

use console_compositor::setup::{
  self, Mode, ModeMatch, Output, OutputRule, Outputs, WorkspaceLocations, workspace_selector,
};
use console_compositor::{DispatchResult, HyprctlError, Operation, Script};
use console_core_internal_programs::{CONFIRM_DOES, InternalProgram};
use console_settings::display::{self, Side};
pub(super) use console_settings::display::{screen, selected};
use console_waiting::Schedule;

const APPLY_WAIT: Duration = Duration::from_secs(2);
const KEEP_WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(super) enum Fault {
  Edit(io::Error),
  Editing,
  Compositor(HyprctlError),
  Observation(console_compositor::BatchError),
  Rejected(String),
  Disconnected,
  NotApplied,
  Confirmation(io::Error),
  ConfirmationStatus(ExitStatus),
  MissingConnector,
  UnknownOutput,
  UnsupportedMode,
  InvalidSide,
  MissingReference,
  Overlap,
  MissingExtendedDisplay,
  UnavailableMirror,
  Usage,
}

impl fmt::Display for Fault {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Edit(fault) | Self::Confirmation(fault) => fmt::Display::fmt(fault, output),
      Self::Editing => output.write_str("another display change is in progress"),
      Self::Compositor(fault) => fmt::Display::fmt(fault, output),
      Self::Observation(fault) => fmt::Display::fmt(fault, output),
      Self::Rejected(reason) => output.write_str(reason),
      Self::Disconnected => output.write_str("the display finished disconnecting"),
      Self::NotApplied => output.write_str("the display did not accept the requested settings"),
      Self::ConfirmationStatus(status) => write!(output, "the confirmation could not open: {status}"),
      Self::MissingConnector => output.write_str("a display connector is required"),
      Self::UnknownOutput => output.write_str("the display is no longer connected"),
      Self::UnsupportedMode => output.write_str("this display does not advertise that mode"),
      Self::InvalidSide => output.write_str("choose Left, Right, Above, or Below"),
      Self::MissingReference => output.write_str("the reference display is unavailable"),
      Self::Overlap => output.write_str("that position overlaps another display"),
      Self::MissingExtendedDisplay => output.write_str("another active display is required"),
      Self::UnavailableMirror => output.write_str("the display cannot mirror that source"),
      Self::Usage => output.write_str("usage: console-scale display CONNECTOR mode MODE | place CONNECTOR SIDE | extend | mirror CONNECTOR"),
    }
  }
}

pub(super) fn edit() -> Result<File, Fault> {
  let edit = setup::try_edit().map_err(Fault::Edit)?;
  edit.ok_or(Fault::Editing)
}

pub(super) fn edit_for_restore() -> Result<File, Fault> {
  let deadline = KEEP_WAIT.saturating_add(APPLY_WAIT.saturating_mul(2));
  let Ok(schedule) = Schedule::asking_every(deadline, POLL);
  let Ok(available) = console_waiting::until_some(schedule, || {
    Ok(match setup::try_edit() {
      Ok(Some(editing)) => Some(Ok(editing)),
      Ok(None) => None,
      Err(fault) => Some(Err(Fault::Edit(fault))),
    })
  });

  match available {
    Some(result) => result,
    None => Err(Fault::Editing),
  }
}

#[must_use]
pub(super) fn failed(fault: &str) -> core::result::Result<ExitCode, console_core_never::Never> {
  eprintln!("{fault}");
  Ok(ExitCode::FAILURE)
}

enum Applied {
  Ready,
  Pending,
}

fn expected(
  rule: OutputRule<'_>,
  current: &Output,
  outputs: &[Output],
) -> core::result::Result<Applied, console_core_never::Never> {
  let Ok(reading) = current.rule(outputs);
  let Ok(current_mode) = current.current_mode();
  let Ok(agreement) = current_mode.matches(Mode {
    size: rule.mode,
    refresh: rule.refresh,
  });
  let matches = agreement == ModeMatch::Same
    && (reading.scale.0 - rule.scale.0).abs() < 0.0001
    && reading.transform == rule.transform
    && reading.mirror == rule.mirror
    && (rule.mirror.is_some() || reading.position == rule.position);

  Ok(match bool::then_some(matches, ()) {
    Some(()) => Applied::Ready,
    None => Applied::Pending,
  })
}

pub(super) fn apply(rule: OutputRule<'_>, id: i64) -> Result<(), Fault> {
  let Ok(script) = setup::configure_outputs(&[rule]);
  let Ok(result) = script.run();

  match result {
    DispatchResult::Success => {}
    DispatchResult::Failure(reason) => return Err(Fault::Rejected(reason)),
  }

  let Ok(patience) = Schedule::asking_every(APPLY_WAIT, POLL);
  let Ok(applied) = console_waiting::until_some(patience, || {
    let outputs = match console_compositor::ask(Outputs) {
      Ok(outputs) => outputs,
      Err(fault) => return Ok(Some(Err(Fault::Compositor(fault)))),
    };

    let current = match outputs
      .iter()
      .find(|output| output.connector == rule.connector && output.id == id)
    {
      Some(current) => current,
      None => return Ok(Some(Err(Fault::Disconnected))),
    };

    Ok(match expected(rule, current, &outputs) {
      Ok(Applied::Ready) => Some(Ok(())),
      Ok(Applied::Pending) => None,
    })
  });

  match applied {
    Some(present) => present,
    None => Err(Fault::NotApplied),
  }
}

#[must_use]
pub(super) fn restored(
  previous: OutputRule<'_>,
  wanted: OutputRule<'_>,
  id: i64,
  fault: &str,
) -> core::result::Result<ExitCode, console_core_never::Never> {
  Ok(match restore(previous, wanted, id) {
    Ok(()) => {
      let Ok(explicit_value) = failed(fault);
      explicit_value
    }
    Err(restore) => {
      let Ok(explicit_value) = failed(&format!(
        "{fault}. Could not restore the display: {restore}"
      ));
      explicit_value
    }
  })
}

fn restore(mut previous: OutputRule<'_>, wanted: OutputRule<'_>, id: i64) -> Result<(), Fault> {
  let live = console_compositor::ask(Outputs).map_err(Fault::Compositor)?;

  match {
    bool::then_some(
      !live
        .iter()
        .any(|output| output.id == id && output.connector == previous.connector),
      (),
    )
  } {
    Some(()) => {
      return Ok(());
    }
    None => {}
  }

  previous.touch = wanted.touch;

  previous.mirror = previous
    .mirror
    .filter(|mirror| live.iter().any(|output| output.connector == *mirror));

  apply(previous, id)
}

enum Confirmation {
  Keep,
  Revert,
}

fn keep() -> Result<Confirmation, Fault> {
  let Ok(mut command) = InternalProgram::Confirm.command();
  let mut dialog = command
    .env(CONFIRM_DOES, "Keep")
    .arg("Keep these display settings? Reverting in 15 seconds.")
    .spawn()
    .map_err(Fault::Confirmation)?;
  let Ok(patience) = Schedule::asking_every(KEEP_WAIT, POLL);
  let Ok(answer) = console_waiting::until_some_handed(patience, &mut dialog, |dialog_2| {
    Ok(match dialog_2.try_wait() {
      Ok(Some(status)) => Some(match status.code() {
        Some(0) => Ok(Confirmation::Keep),
        Some(1) => Ok(Confirmation::Revert),
        Some(_) | None => Err(Fault::ConfirmationStatus(status)),
      }),
      Ok(None) => None,
      Err(fault) => Some(Err(Fault::Confirmation(fault))),
    })
  });

  let outcome = match answer {
    Some(Ok(confirmed)) => return Ok(confirmed),
    Some(Err(fault)) => Err(fault),
    None => Ok(Confirmation::Revert),
  };

  match dialog.kill() {
    Ok(()) => {}
    Err(fault) => match (fault.kind() == io::ErrorKind::InvalidInput).then_some(()) {
      Some(()) => {}
      None => return Err(Fault::Confirmation(fault)),
    },
  }

  let _status = dialog.wait().map_err(Fault::Confirmation)?;
  outcome
}

fn restore_workspaces(before: &[setup::WorkspaceLocation]) -> Result<(), Fault> {
  match before {
    [] => {
      return Ok(());
    }
    [_, ..] => {}
  }

  let (after, outputs) =
    console_compositor::ask_together(WorkspaceLocations, Outputs).map_err(Fault::Observation)?;
  let connected: std::collections::HashSet<_> = outputs
    .iter()
    .filter(|output| output.mirror.is_none())
    .map(|output| output.connector.as_str())
    .collect();
  let mut locations = std::collections::HashMap::new();

  for now in &after {
    locations
      .entry(now.workspace.id)
      .or_insert_with(std::collections::BTreeSet::new)
      .insert(now.connector.as_str());
  }

  let moved: Vec<_> = before
    .iter()
    .filter(|saved| connected.contains(saved.connector.as_str()))
    .filter(|saved| {
      locations
        .get(&saved.workspace.id)
        .is_some_and(|current| current.len() > 1 || !current.contains(saved.connector.as_str()))
    })
    .map(|saved| {
      let Ok(selector) = workspace_selector(saved.workspace.id, &saved.workspace.named);
      (selector, &saved.connector)
    })
    .collect();

  match bool::then_some(moved.is_empty(), ()) {
    Some(()) => {
      return Ok(());
    }
    None => {}
  }

  let operations: Vec<_> = moved
    .iter()
    .map(|(workspace, output)| Operation::MoveWorkspaceToOutput { workspace, output })
    .collect();
  let Ok(script) = Script::operations(&operations);
  let Ok(answer) = script.run();

  match answer {
    DispatchResult::Success => Ok(()),
    DispatchResult::Failure(reason) => Err(Fault::Rejected(reason)),
  }
}

enum Change<'a> {
  Unchanged,
  Wanted(OutputRule<'a>),
}

fn with_mode<'a>(
  display: &Output,
  rule: OutputRule<'a>,
  written: &str,
) -> Result<OutputRule<'a>, Fault> {
  let mode = Mode::read(written).map_err(|_invalid| Fault::UnsupportedMode)?;
  let Ok(current) = display.current_mode();
  let Ok(agreement) = current.matches(mode);

  match agreement {
    ModeMatch::Same => return Ok(rule),
    ModeMatch::Different => {}
  }

  match display.available_modes.contains(&mode).then_some(()) {
    Some(()) => {}
    None => return Err(Fault::UnsupportedMode),
  }

  let scale = match (mode.size == display.mode).then_some(()) {
    Some(()) => rule.scale,
    None => {
      let Ok(adjusted) = display::pixel_aligned_scale(mode.size, display.scale);
      adjusted
    }
  };
  Ok(OutputRule {
    mode: mode.size,
    refresh: mode.refresh,
    scale,
    ..rule
  })
}

fn change<'a>(
  display: &'a Output,
  outputs: &'a [Output],
  arguments: &[String],
) -> Result<Change<'a>, Fault> {
  let Ok(mut rule) = display.rule(outputs);
  let (action, operands) = arguments.split_first().ok_or(Fault::Usage)?;

  match (action.as_str(), operands) {
    ("mode", [mode]) => {
      let adjusted = with_mode(display, rule, mode)?;
      rule = adjusted;
    }
    ("place", [anchor, side]) => {
      let side_2 = Side::ALL
        .into_iter()
        .find(|one| one.word() == Ok(side.as_str()))
        .ok_or(Fault::InvalidSide)?;
      let Ok(mut anchors) = display::outputs_for_layout(display, outputs);
      let anchor_2 = anchors
        .find(|output| &output.connector == anchor)
        .ok_or(Fault::MissingReference)?;
      let Ok(at) = display::placed(display, anchor_2, side_2);
      let Ok(placement) = display::placement(display, at, outputs);

      match placement {
        display::Placement::Overlapping => {
          return Err(Fault::Overlap);
        }
        display::Placement::Available => {}
      }

      rule.position = at;
      rule.mirror = None;
    }
    ("extend", []) => match bool::then_some(rule.mirror.is_some(), ()) {
      Some(()) => {
        let Ok(extended) = display::extend(display, outputs);
        let checked_value = extended.ok_or(Fault::MissingExtendedDisplay)?;
        rule.position = checked_value;
        rule.mirror = None;
      }
      None => {}
    },
    ("mirror", [source]) => {
      let Ok(mut sources) = display::mirror_sources(display, outputs);
      let source_2 = sources
        .find(|output| &output.connector == source)
        .ok_or(Fault::UnavailableMirror)?;
      rule.position = source_2.position;
      rule.mirror = Some(&source_2.connector);
    }
    _invalid => return Err(Fault::Usage),
  }

  Ok(match expected(rule, display, outputs) {
    Ok(Applied::Ready) => Change::Unchanged,
    Ok(Applied::Pending) => Change::Wanted(rule),
  })
}

enum TrialFault {
  Change(Fault),
  Rollback {
    initial: Option<Fault>,
    restore: Fault,
  },
}

impl From<Fault> for TrialFault {
  fn from(fault: Fault) -> Self {
    Self::Change(fault)
  }
}

impl fmt::Display for TrialFault {
  fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Change(fault) => fmt::Display::fmt(fault, output),
      Self::Rollback {
        initial: Some(fault),
        restore,
      } => write!(output, "{fault}. Could not restore the display: {restore}"),
      Self::Rollback {
        initial: None,
        restore,
      } => write!(output, "Could not restore the display: {restore}"),
    }
  }
}

fn trial(arguments: &[String]) -> Result<ExitCode, TrialFault> {
  let (connector, arguments_2) = arguments.split_first().ok_or(Fault::MissingConnector)?;
  let _editing = edit()?;
  let outputs = console_compositor::ask(Outputs).map_err(Fault::Compositor)?;
  let Ok(chosen) = selected(&outputs, Some(connector));
  let display = chosen.ok_or(Fault::UnknownOutput)?;
  let change = change(display, &outputs, arguments_2)?;
  let wanted = match change {
    Change::Unchanged => return Ok(ExitCode::SUCCESS),
    Change::Wanted(rule) => rule,
  };

  let Ok(previous) = display.rule(&outputs);
  let workspaces = match bool::then_some(previous.mirror != wanted.mirror, ()) {
    Some(()) => console_compositor::ask(WorkspaceLocations).map_err(Fault::Compositor)?,
    None => Vec::new(),
  };
  let initial = match apply(wanted, display.id).and_then(|()| keep()) {
    Ok(Confirmation::Keep) => return Ok(ExitCode::SUCCESS),
    Ok(Confirmation::Revert) => None,
    Err(fault) => Some(fault),
  };
  let restored =
    restore(previous, wanted, display.id).and_then(|()| restore_workspaces(&workspaces));

  match (initial, restored) {
    (None, Ok(())) => {
      println!("Previous display settings restored");
      Ok(ExitCode::SUCCESS)
    }
    (Some(fault), Ok(())) => Err(TrialFault::Change(fault)),
    (initial_2, Err(restore)) => Err(TrialFault::Rollback {
      initial: initial_2,
      restore,
    }),
  }
}

#[must_use]
pub(super) fn run(
  arguments: &[String],
) -> core::result::Result<ExitCode, console_core_never::Never> {
  Ok(match trial(arguments) {
    Ok(code) => code,
    Err(fault) => {
      let Ok(explicit_value) = failed(&fault.to_string());
      explicit_value
    }
  })
}
