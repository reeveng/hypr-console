//! Preferences and shortcuts for the workspace that opens over the current one.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_places::Base;

use crate::{DispatchResult, Request, dispatch, lua, request};

pub const MUSIC: &str = "console-music.desktop";
const NAME: &str = "console-quick";

pub type Applications = BTreeMap<String, Vec<String>>;

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
  let saved = match std::fs::read_to_string(at) {
    Ok(saved) => saved,
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(BTreeMap::new()),
      None => return Err(fault),
    },
  };

  Ok(
    saved
      .lines()
      .filter_map(|line| {
        let (id, classes) = line.split_once('\t')?;
        let classes_2: Vec<_> = classes
          .split('\t')
          .filter(|class| !class.is_empty())
          .map(str::to_owned)
          .collect();

        (!id.is_empty() && !classes_2.is_empty()).then(|| (id.to_owned(), classes_2))
      })
      .collect(),
  )
}

pub fn reload() -> Result<DispatchResult, Never> {
  dispatch("/reload")
}

pub fn toggle() -> Result<DispatchResult, Never> {
  request(
    Request::Script,
    &format!(
      "if hl.get_workspace('special:{NAME}') then \
         hl.dispatch(hl.dsp.workspace.toggle_special('{NAME}')) \
       end"
    ),
  )
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
