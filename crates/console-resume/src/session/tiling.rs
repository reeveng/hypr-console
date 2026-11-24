//! Reapply a Dwindle split from the rectangles the compositor reported.

use std::collections::BTreeMap;

use console_compositor::setup::{WorkspaceLocation, WorkspaceLocations};
use console_compositor::{DispatchResult, Filling, Floating, Operation, Window, WindowAction};
use console_core_never::Never;
use console_core_number_conversion::Float;

#[derive(Debug, PartialEq, Eq)]
enum Axis {
  Horizontal,
  Vertical,
}

struct Split<'a> {
  axis: Axis,
  first: &'a Window,
  first_size: i64,
  second_size: i64,
  gap: i64,
}

fn split<'a>(one: &'a Window, other: &'a Window) -> Result<Option<Split<'a>>, Never> {
  match {
    ::core::primitive::bool::then_some(
      one.at.1 == other.at.1 && one.size.1 == other.size.1 && one.at.0 != other.at.0,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      let (first, second) =
        match { ::core::primitive::bool::then_some(one.at.0 < other.at.0, ()) } {
          ::core::option::Option::Some(()) => (one, other),
          ::core::option::Option::None => (other, one),
        };
      return Ok(Some(Split {
        axis: Axis::Horizontal,
        first,
        first_size: first.size.0,
        second_size: second.size.0,
        gap: second
          .at
          .0
          .saturating_sub(first.at.0.saturating_add(first.size.0)),
      }));
    }
    ::core::option::Option::None => {}
  }

  match {
    ::core::primitive::bool::then_some(
      one.at.0 == other.at.0 && one.size.0 == other.size.0 && one.at.1 != other.at.1,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      let (first, second) =
        match { ::core::primitive::bool::then_some(one.at.1 < other.at.1, ()) } {
          ::core::option::Option::Some(()) => (one, other),
          ::core::option::Option::None => (other, one),
        };
      return Ok(Some(Split {
        axis: Axis::Vertical,
        first,
        first_size: first.size.1,
        second_size: second.size.1,
        gap: second
          .at
          .1
          .saturating_sub(first.at.1.saturating_add(first.size.1)),
      }));
    }
    ::core::option::Option::None => {}
  }

  Ok(None)
}

#[derive(PartialEq, Eq)]
enum Placement {
  Tiled,
  Other,
}

fn placement(window: &Window) -> Result<Placement, Never> {
  Ok(
    match (window.floating == Floating::No && window.filling != Filling::Screen).then_some(()) {
      Some(()) => Placement::Tiled,
      None => Placement::Other,
    },
  )
}

fn apply(operations: &[Operation<'_>]) -> Result<(), Never> {
  let Ok(script) = console_compositor::Script::operations(operations);

  match script.run() {
    Ok(DispatchResult::Success) => {}
    Ok(DispatchResult::Failure(why)) => {
      console_core_logging::warn!("console-resume: tile split: {why}");
    }
    Err(fault) => {
      console_core_logging::debug!("console-resume: tile split could not be requested: {fault}");
    }
  }

  Ok(())
}

struct Pair<'a> {
  windows: [&'a Window; 2],
  split: Split<'a>,
}

fn pair<'a>(windows: impl Iterator<Item = &'a Window>) -> Result<Option<Pair<'a>>, Never> {
  let mut tiles = windows.filter(|window| placement(window) == Ok(Placement::Tiled));
  let (first, second) = match (tiles.next(), tiles.next(), tiles.next()) {
    (Some(first), Some(second), None) => (first, second),
    _ => return Ok(None),
  };
  let Ok(found) = split(first, second);

  Ok(found.map(|split| Pair {
    windows: [first, second],
    split,
  }))
}

struct Windows<'a> {
  saved: &'a [Window],
  open: &'a [Window],
  bound: &'a BTreeMap<String, String>,
}

pub(super) fn restore(saved: &[Window], bound: &BTreeMap<String, String>) -> Result<(), Never> {
  match (bound.len() < 2).then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  let (locations, open) = match (
    console_compositor::ask(WorkspaceLocations),
    console_compositor::ask(console_compositor::Clients),
  ) {
    (Ok(locations), Ok(open)) => (locations, open),
    (Err(_workspaces_unavailable), _) => return Ok(()),
    (_, Err(_windows_unavailable)) => return Ok(()),
  };
  let windows = Windows {
    saved,
    open: &open,
    bound,
  };

  for location in locations {
    match location.layout.as_deref() {
      Some("dwindle") => {
        let Ok(()) = restore_workspace(&location, &windows);
      }
      Some(_) | None => {}
    }
  }

  Ok(())
}

fn restore_workspace(location: &WorkspaceLocation, windows: &Windows<'_>) -> Result<(), Never> {
  let Ok(selector) = console_compositor::setup::workspace_selector(
    location.workspace.id,
    &location.workspace.named,
  );
  let Ok(saved) = pair(windows.saved.iter().filter(|window| {
    let Ok(workspace) = super::workspace_selector(window);
    workspace == selector
  }));
  let Ok(live) = pair(
    windows
      .open
      .iter()
      .filter(|window| window.workspace == location.workspace.id),
  );
  let (wanted, current) = match (saved, live) {
    (Some(wanted), Some(current)) => (wanted, current),
    (None, _) | (_, None) => return Ok(()),
  };
  let [one, other] = wanted.windows;
  let [first, second] = current.windows;
  let addresses = [&first.address, &second.address];

  match (
    windows.bound.get(&one.address),
    windows.bound.get(&other.address),
  ) {
    (Some(address), Some(neighbor)) => {
      match (addresses.contains(&address) && addresses.contains(&neighbor)).then_some(()) {
        Some(()) => {}
        None => return Ok(()),
      }
    }
    (None, _) | (_, None) => return Ok(()),
  }

  let focus = match windows.bound.get(&wanted.split.first.address) {
    Some(address) => address,
    None => return Ok(()),
  };

  change_split(
    focus,
    SplitChange {
      wanted: &wanted.split,
      current: &current.split,
    },
  )
}

struct SplitChange<'a, 'b> {
  wanted: &'a Split<'b>,
  current: &'a Split<'b>,
}

fn change_split(
  focus: &str,
  SplitChange { wanted, current }: SplitChange<'_, '_>,
) -> Result<(), Never> {
  let Ok(first_size) = wanted.first_size.float();
  let Ok(second_size) = wanted.second_size.float();
  let Ok(gap) = wanted.gap.float();

  match (first_size <= 0.0 || second_size <= 0.0 || gap < 0.0).then_some(()) {
    Some(()) => return Ok(()),
    None => {}
  }

  let ratio = ((2.0 * first_size + gap) / (first_size + second_size + gap)).clamp(0.1, 1.9);
  let ratio_command = format!("splitratio {ratio:.4} exact");
  let mut operations = vec![Operation::Window {
    address: focus,
    action: WindowAction::Focus,
  }];

  match (wanted.axis != current.axis).then_some(()) {
    Some(()) => operations.push(Operation::Layout("togglesplit")),
    None => {}
  }

  match (focus != current.first.address).then_some(()) {
    Some(()) => operations.push(Operation::Layout("swapsplit")),
    None => {}
  }

  operations.push(Operation::Layout(&ratio_command));

  apply(&operations)
}
