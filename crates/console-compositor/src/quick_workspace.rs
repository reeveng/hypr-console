//! Applications and shortcuts for workspaces that open over the current one.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;

use crate::{
  Clients, DispatchResult, HyprctlError, Pinned, Query, Question, Request, Window, ask_together,
  lua, request,
};

pub const MUSIC: &str = "console-music.desktop";
pub const NAME: &str = "console-quick";
pub const TOGGLE_NAMED: &str = "special-workspace";
const WORKSPACE_RECORD: &str = "@workspace";

pub type Applications = BTreeMap<String, Vec<String>>;
pub type Workspaces = BTreeMap<String, Applications>;

pub fn path() -> Result<Option<PathBuf>, Never> {
  Ok(
    {
      let checked_value = Base::Configuration.ours()?;
      checked_value
    }
    .map(|folder| folder.join("quick-workspace")),
  )
}

pub fn load(at: &Path) -> io::Result<Applications> {
  let mut workspaces = load_workspaces(at)?;

  Ok(match workspaces.remove(NAME) {
    Some(apps) => apps,
    None => Applications::new(),
  })
}

pub fn load_workspaces(at: &Path) -> io::Result<Workspaces> {
  let saved = match console_core_atomic_writes::read_text(at) {
    Ok(saved) => saved,
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(BTreeMap::new()),
      None => return Err(fault),
    },
  };

  let mut workspaces = Workspaces::new();

  for (workspace, id, classes) in saved.lines().filter_map(|line| {
    let Ok(app) = record(line);

    app
  }) {
    workspaces
      .entry(workspace.to_owned())
      .or_default()
      .insert(id.to_owned(), classes);
  }

  Ok(workspaces)
}

#[must_use]
fn token(word: &str) -> Result<Option<&str>, Never> {
  Ok((!word.is_empty() && !word.chars().any(char::is_control)).then_some(word))
}

#[must_use]
fn record(line: &str) -> Result<Option<(&str, &str, Vec<String>)>, Never> {
  let (workspace, entry) = match line.split_once('\t') {
    Some((WORKSPACE_RECORD, rest)) => match rest.split_once('\t') {
      Some(named) => named,
      None => return Ok(None),
    },
    Some(_) => (NAME, line),
    None => return Ok(None),
  };
  let (id, classes) = match entry.split_once('\t') {
    Some(app) => app,
    None => return Ok(None),
  };
  let Ok(name) = token(workspace);
  let Ok(desktop_id) = token(id);
  let parsed: Vec<_> = classes
    .split('\t')
    .filter_map(|part| {
      let Ok(valid) = token(part);

      valid.map(str::to_owned)
    })
    .collect();

  Ok(
    name
      .zip(desktop_id)
      .filter(|_| !parsed.is_empty())
      .map(|(workspace_name, desktop)| (workspace_name, desktop, parsed)),
  )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppSelection {
  Include,
  Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
  pub workspace: String,
  pub id: String,
  pub classes: Vec<String>,
  pub selection: AppSelection,
}

pub fn save_assignment(at: &Path, assignment: &Assignment) -> io::Result<()> {
  let Assignment {
    workspace,
    id,
    classes,
    selection,
  } = assignment;
  let filtered: Vec<_> = classes
    .iter()
    .filter_map(|part| {
      let Ok(valid) = token(part);

      valid.map(str::to_owned)
    })
    .collect();
  let Ok(name) = token(workspace);
  let Ok(desktop_id) = token(id);

  match (name.is_some()
    && desktop_id.is_some()
    && id != WORKSPACE_RECORD
    && (*selection == AppSelection::Remove || !filtered.is_empty()))
  .then_some(())
  {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid workspace app",
      ));
    }
  }

  match at.parent() {
    Some(folder) => std::fs::create_dir_all(folder)?,
    None => {}
  }

  let lock = console_core_atomic_writes::open_lock_file(&at.with_extension("lock"))?;
  lock.try_lock().map_err(io::Error::from)?;

  let mut workspaces = load_workspaces(at)?;

  match selection {
    AppSelection::Include => {
      for existing in workspaces.values_mut() {
        existing.remove(id);
      }

      workspaces
        .entry(workspace.clone())
        .or_default()
        .insert(id.clone(), filtered);
    }
    AppSelection::Remove => {
      workspaces.entry(workspace.clone()).or_default().remove(id);
    }
  }

  let Ok(saved) = serialize(&workspaces);
  console_core_atomic_writes::whole(at, saved.as_bytes()).map_err(io::Error::other)
}

fn serialize(workspaces: &Workspaces) -> Result<String, Never> {
  let mut saved = String::new();

  for (workspace, apps) in workspaces {
    for (id, classes) in apps {
      match workspace.as_str() {
        NAME => {}
        _ => {
          saved.push_str(WORKSPACE_RECORD);
          saved.push('\t');
          saved.push_str(workspace);
          saved.push('\t');
        }
      }

      saved.push_str(id);

      for class in classes {
        saved.push('\t');
        saved.push_str(class);
      }

      saved.push('\n');
    }
  }

  Ok(saved)
}

pub fn reload() -> Result<DispatchResult, Never> {
  crate::reload_configuration()
}

pub fn toggle() -> Result<DispatchResult, Never> {
  toggle_workspace(NAME)
}

pub fn toggle_named(name: &str) -> Result<DispatchResult, Never> {
  toggle_workspace(name)
}

fn toggle_workspace(name: &str) -> Result<DispatchResult, Never> {
  let mut workspaces = match configured() {
    Ok(saved) => saved,
    Err(fault) => return Ok(DispatchResult::Failure(fault.to_string())),
  };
  let Ok(dispatcher) = toggle_action(name);
  let selected = match workspaces.remove(name) {
    Some(apps) => apps,
    None => {
      return match (name == NAME).then_some(()) {
        Some(()) => request(
          Request::Script,
          &format!("if hl.get_workspace('special:{NAME}') then hl.dispatch({dispatcher}) end"),
        ),
        None => request(Request::Dispatch, &dispatcher),
      };
    }
  };
  let snapshot = ask_together(FocusedSpecialWorkspace, Clients);
  let (visible, windows) = match snapshot {
    Ok(observed) => observed,
    Err(fault) => return Ok(DispatchResult::Failure(fault.to_string())),
  };
  let target = format!("special:{name}");

  match (visible.as_deref() == Some(target.as_str())).then_some(()) {
    Some(()) => request(Request::Dispatch, &dispatcher),
    None => {
      let Ok(roots) = console_core_places::applications();
      let Ok(effects) = opening_apps(&target, &selected, &windows, &roots);
      let Ok(script) = opening_script(name, &effects);
      request(Request::Script, &script)
    }
  }
}

fn toggle_action(name: &str) -> Result<String, Never> {
  let Ok(named) = lua::quote(name);

  Ok(format!("hl.dsp.workspace.toggle_special({named})"))
}

#[derive(Clone, Copy)]
struct FocusedSpecialWorkspace;

impl Question for FocusedSpecialWorkspace {
  const ASKED: Query = Query::Monitors;
  type Reply = Option<String>;

  fn said(value: serde_json::Value) -> Result<Self::Reply, HyprctlError> {
    let monitors = value
      .as_array()
      .ok_or(HyprctlError::MissingField(Query::Monitors, "a list"));
    let listed = monitors?;
    let focused = listed
      .iter()
      .find(|monitor| monitor.get("focused").and_then(serde_json::Value::as_bool) == Some(true));

    Ok(
      focused
        .and_then(|monitor| monitor.get("specialWorkspace"))
        .and_then(|workspace| workspace.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned),
    )
  }
}

#[derive(Debug, PartialEq, Eq)]
enum AppEffect {
  Move(String),
  Launch(PathBuf),
}

#[cfg_attr(
  dylint_lib = "explicit028_no_search_in_a_loop",
  expect(
    explicit028_no_search_in_a_loop,
    reason = "matches each of the selected desktop entries against windows in one bounded compositor reply"
  )
)]
fn opening_apps(
  target: &str,
  apps: &Applications,
  windows: &[Window],
  roots: &[PathBuf],
) -> Result<Vec<AppEffect>, Never> {
  let Ok(entries) = console_applications::entry::files(roots);
  let mut effects = Vec::new();

  for entry in entries {
    let id = match entry.file_name().and_then(std::ffi::OsStr::to_str) {
      Some(id) => id,
      None => continue,
    };
    let classes = match apps.get(id) {
      Some(matches) => matches,
      None => continue,
    };
    let mut matching = windows
      .iter()
      .filter(|window| classes.contains(&window.first_class));

    match matching
      .clone()
      .any(|window| window.workspace_named == target || window.pinned == Pinned::Yes)
      .then_some(())
    {
      Some(()) => continue,
      None => {}
    }

    effects.push(
      match matching.find(|window| window.workspace != crate::NO_WORKSPACE) {
        Some(window) => AppEffect::Move(window.address.clone()),
        None => AppEffect::Launch(entry),
      },
    );
  }

  Ok(effects)
}

fn opening_script(name: &str, effects: &[AppEffect]) -> Result<String, Never> {
  let Ok(quoted_target) = lua::quote(&format!("special:{name}"));
  let Ok(dispatcher) = toggle_action(name);
  let mut script = format!(
    "local visible = hl.get_active_special_workspace()\n\
     local closing = visible and visible.name == {quoted_target}\n\
     hl.dispatch({dispatcher})\n\
     if closing then return end\n"
  );

  for effect in effects {
    match effect {
      AppEffect::Move(address) => {
        let Ok(selector) = lua::quote(&format!("address:{address}"));
        script.push_str(&format!(
          "if hl.get_window({selector}) then hl.dispatch(hl.dsp.window.move({{ window = {selector}, workspace = {quoted_target}, follow = false }})) end\n"
        ));
      }
      AppEffect::Launch(entry) => {
        let Ok(launch) = console_applications::launch::entry(entry);
        let words: Option<Vec<_>> = std::iter::once(launch.get_program())
          .chain(launch.get_args())
          .map(std::ffi::OsStr::to_str)
          .collect();

        match words {
          Some(arguments) => {
            let Ok(command) = console_core_external_programs::shell::join(arguments);
            let Ok(quoted) = lua::quote(&command);
            script.push_str(&format!(
              "hl.exec_cmd({quoted}, {{ workspace = {quoted_target} }})\n"
            ));
          }
          None => console_core_logging::warn!(
            "Quick Workspace app has a non-UTF-8 path: {}",
            entry.display()
          ),
        }
      }
    }
  }

  Ok(script)
}

fn configured() -> io::Result<Workspaces> {
  let Ok(at) = path();

  match at {
    Some(file) => load_workspaces(&file),
    None => Ok(Workspaces::new()),
  }
}

pub fn rules() -> io::Result<String> {
  let saved = configured()?;
  let Ok(script) = workspace_rules(&saved);

  Ok(script)
}

fn workspace_rules(saved: &Workspaces) -> Result<String, Never> {
  let mut script = String::new();

  for (name, apps) in saved {
    let mut classes = BTreeSet::new();

    for class in apps.values().flatten() {
      let Ok(pattern) = literal_pattern(class);
      classes.insert(pattern);
    }

    let patterns = classes.into_iter().collect::<Vec<_>>().join("|");
    let Ok(pattern) = lua::quote(&patterns);
    let Ok(target) = lua::quote(&format!("special:{name}"));
    let Ok(label) = lua::quote(&format!("open apps in special:{name}"));
    script.push_str(&format!(
      "hl.window_rule({{ name = {label}, match = {{ initial_class = {pattern} }}, workspace = {target} }})\n"
    ));
  }

  Ok(script)
}

fn literal_pattern(class: &str) -> Result<String, Never> {
  let mut escaped = String::new();

  for letter in class.chars() {
    match letter {
      '\\' | '.' | '^' | '$' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}' => {
        escaped.push('\\')
      }
      _ => {}
    }

    escaped.push(letter);
  }

  Ok(escaped)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn named_workspaces_keep_separate_apps_and_preserve_the_original_configuration()
  -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("quick-workspaces")?;
    let at = folder.join("quick-workspace");
    console_core_atomic_writes::whole(&at, b"console-music.desktop\tmusic\n")?;
    let mut notes = Assignment {
      workspace: "Research notes".to_owned(),
      id: "notes.desktop".to_owned(),
      classes: vec!["notes".to_owned(), "Console Notes".to_owned()],
      selection: AppSelection::Include,
    };
    let browser = Assignment {
      id: "browser.desktop".to_owned(),
      classes: vec!["browser".to_owned()],
      ..notes.clone()
    };
    save_assignment(&at, &notes)?;
    save_assignment(&at, &browser)?;
    save_assignment(&at, &browser)?;

    let loaded = load_workspaces(&at)?;
    assert_eq!(
      loaded.get("Research notes"),
      Some(&Applications::from([
        (notes.id.clone(), notes.classes.clone()),
        (browser.id.clone(), browser.classes.clone()),
      ]))
    );
    let original = load(&at)?;
    assert_eq!(
      original,
      Applications::from([(MUSIC.to_owned(), vec!["music".to_owned()])])
    );

    let lock = console_core_atomic_writes::open_lock_file(&at.with_extension("lock"))?;
    lock.try_lock()?;
    notes.workspace = "Writing".to_owned();
    let busy = save_assignment(&at, &notes);
    assert!(matches!(busy, Err(fault) if fault.kind() == io::ErrorKind::WouldBlock));
    let unchanged = load_workspaces(&at)?;
    assert_eq!(unchanged, loaded);
    drop(lock);

    save_assignment(&at, &notes)?;
    let changed = load_workspaces(&at)?;
    assert_eq!(
      changed.get("Research notes"),
      Some(&Applications::from([(browser.id, browser.classes)]))
    );
    assert_eq!(
      changed.get("Writing"),
      Some(&Applications::from([(
        notes.id.clone(),
        notes.classes.clone()
      )]))
    );
    notes.selection = AppSelection::Remove;
    save_assignment(&at, &notes)?;
    let remaining = load_workspaces(&at)?;
    assert!(!remaining.contains_key("Writing"));
    let legacy = load(&at)?;
    assert_eq!(legacy, original);
    Ok(())
  }

  #[test]
  fn opening_brings_in_running_apps_and_starts_missing_apps_in_a_partly_filled_workspace()
  -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("opening-quick-workspace")?;
    let installed = [
      folder.join("browser.desktop"),
      folder.join("notes.desktop"),
      folder.join("music.desktop"),
      folder.join("pinned.desktop"),
    ];

    for path in &installed {
      console_core_atomic_writes::whole(path, b"")?;
    }

    let selected = Applications::from(
      ["browser", "notes", "music", "pinned"]
        .map(|class| (format!("{class}.desktop"), vec![class.to_owned()])),
    );
    let target = "special:Research notes";
    let windows = crate::said_of(
      Clients,
      serde_json::json!([
        {"address":"0x1", "initialClass":"notes", "workspace":{"id":-99,"name":target}},
        {"address":"0x2", "initialClass":"music", "workspace":{"id":2,"name":"2"}},
        {"address":"0x3", "initialClass":"pinned", "pinned":true, "workspace":{"id":1,"name":"1"}},
      ]),
    )?;
    let Ok(effects) = opening_apps(target, &selected, &windows, std::slice::from_ref(&folder));
    assert_eq!(
      effects,
      [
        AppEffect::Launch(folder.join("browser.desktop")),
        AppEffect::Move("0x2".to_owned()),
      ]
    );
    let Ok(script) = opening_script("Research notes", &effects);
    assert!(script.contains("if closing then return end"));
    assert!(script.contains("address:0x2"));
    assert!(script.contains("browser.desktop"));
    assert!(!script.contains("notes.desktop"));

    Ok(())
  }
}

pub fn music(command: &str) -> Result<DispatchResult, Never> {
  let Ok(command_2) = console_core_external_programs::shell::join([command]);
  let Ok(command_3) = lua::quote(&command_2);

  request(
    Request::Script,
    &format!(
      r#"
local target = 'special:{NAME}'
local music = hl.get_window('class:^music$')
if not music then
    hl.exec_cmd({command}, {{ workspace = target }})
    return
end

local visible = hl.get_active_special_workspace()
local focused = hl.get_active_window()
if visible and visible.name == target and focused and focused.address == music.address then
    hl.dispatch(hl.dsp.workspace.toggle_special('{NAME}'))
    return
end

if music.workspace and music.workspace.name ~= target then
    hl.dispatch(hl.dsp.window.move({{ window = music, workspace = target, follow = false }}))
end
if not visible or visible.name ~= target then
    hl.dispatch(hl.dsp.workspace.toggle_special('{NAME}'))
end
hl.dispatch(hl.dsp.focus({{ window = music }}))
"#,
      command = command_3
    ),
  )
}
