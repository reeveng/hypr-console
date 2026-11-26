use std::ffi::OsStr;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::ExitCode;

use console_compositor::{DispatchResult, DisplayPower, quick_workspace, session, theme_borders};
use console_core_never::Never;

fn main() -> ExitCode {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let command = arguments
    .split_first()
    .and_then(|(action, words)| action.to_str().map(|name| (name, words)));
  let power = match command {
    Some(("start", [config])) => {
      let Ok(mut start) = session::start_command(Path::new(config));
      console_core_logging::warn!("console-compositor: {}", start.exec());

      return ExitCode::FAILURE;
    }
    Some(("quick-workspace", [])) => {
      let Ok(outcome) = quick_workspace::toggle();
      let Ok(status) = report(outcome);

      return status;
    }
    Some((quick_workspace::TOGGLE_NAMED, [name])) => {
      let target = match name.to_str() {
        Some(target) => target,
        None => {
          console_core_logging::warn!("console-compositor: the workspace name must be UTF-8");
          return ExitCode::from(2);
        }
      };
      let Ok(outcome) = quick_workspace::toggle_named(target);
      let Ok(status) = report(outcome);

      return status;
    }
    Some(("quick-workspace-rules", [])) => {
      let rules = quick_workspace::rules();

      return match rules {
        Ok(script) => {
          print!("{script}");
          ExitCode::SUCCESS
        }
        Err(fault) => {
          console_core_logging::warn!(
            "console-compositor: reading Quick Workspace apps: {fault}"
          );
          ExitCode::FAILURE
        }
      };
    }
    Some(("reload", [])) => {
      let Ok(outcome) = console_compositor::reload_configuration();
      let Ok(status) = report(outcome);

      return status;
    }
    Some(("theme-borders", [])) => {
      let Ok(status) = print_theme_borders();

      return status;
    }
    Some(("quick-music", [library, panel])) => {
      let Ok(status) = music(library, panel);

      return status;
    }
    Some(("display-on", [])) => DisplayPower::On,
    Some(("display-off", [])) => DisplayPower::Off,
    Some(_) | None => {
      console_core_logging::warn!(
        "usage: console-compositor start CONFIG | display-on | display-off | reload | theme-borders | quick-workspace | special-workspace NAME | quick-workspace-rules | quick-music LIBRARY PANEL"
      );

      return ExitCode::from(2);
    }
  };
  let Ok(outcome) = power.set();
  let Ok(status) = report(outcome);

  status
}

#[must_use]
fn print_theme_borders() -> Result<ExitCode, Never> {
  Ok(match theme_borders::Colors::current() {
    Ok(colors) => {
      let Ok(pair) = colors.hex_pair();
      println!("{pair}");
      ExitCode::SUCCESS
    }
    Err(fault) => {
      console_core_logging::warn!("console-compositor: reading theme borders: {fault}");
      ExitCode::FAILURE
    }
  })
}

#[must_use]
fn report(outcome: DispatchResult) -> Result<ExitCode, Never> {
  Ok(match outcome {
    DispatchResult::Success => ExitCode::SUCCESS,
    DispatchResult::Failure(why) => {
      console_core_logging::warn!("console-compositor: {why}");
      ExitCode::FAILURE
    }
  })
}

#[must_use]
fn music(library: &OsStr, panel: &OsStr) -> Result<ExitCode, Never> {
  let Ok(path) = quick_workspace::path();

  match (path, library.to_str()) {
    (Some(path_2), Some(command)) => match quick_workspace::load(&path_2) {
      Ok(apps) => match apps.get(quick_workspace::MUSIC) {
        Some(_) => {
          let Ok(outcome) = quick_workspace::music(command);
          return report(outcome);
        }
        None => {}
      },
      Err(fault) => {
        console_core_logging::warn!("could not read Quick Workspace settings: {fault}")
      }
    },
    (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {}
  }

  let fault = std::process::Command::new(panel).exec();
  console_core_logging::warn!("could not open the Music panel: {fault}");
  Ok(ExitCode::FAILURE)
}
