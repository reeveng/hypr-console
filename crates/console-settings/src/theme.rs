use console_compositor::{DispatchResult, theme_borders};
use console_core_color::Rgba;
use console_core_color::palette::{self, ColorRole, Overrides, Theme, ThemeSettings};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_notifications::saying::Content as Notification;
use console_panel::page::{Aside, Handler, Initial, NOW, Row, Showing, Subject};
use console_panel::tasks::Task;
use console_program_contract::Command;

fn update(
  showing: &dyn Showing,
  restart: &Task,
  note: &str,
  change: impl FnOnce(&mut ThemeSettings) -> Result<(), Never>,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(previous) = ThemeSettings::current();
    let mut settings = previous.clone();
    let Ok(()) = change(&mut settings);

    match { ::core::primitive::bool::then_some(settings == previous, ()) } {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    match settings.save() {
      Ok(()) => {
        let Ok(()) = apply_window_borders(note);
        showing.refresh();
        showing.run(restart.clone());
      }
      Err(fault) => {
        let Ok(()) = console_notifications::saying::report(
          console_notifications::Application("Settings"),
          "color-theme",
          Notification {
            summary: "Couldn't change the theme",
            body: &fault.to_string(),
          },
        );
      }
    }
  };

  Ok(())
}

fn apply_window_borders(note: &str) -> Result<(), Never> {
  let borders = match theme_borders::Colors::current() {
    Ok(borders) => borders,
    Err(fault) => {
      console_core_logging::warn!("settings-panel: reading theme borders: {fault}");
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Settings"),
        "Theme saved",
        Some("Could not load the window border colors."),
      );
      return Ok(());
    }
  };
  let Ok(result) = borders.apply();

  match result {
    DispatchResult::Success => {
      let Ok(()) =
        console_notifications::notify(console_notifications::Application("Settings"), note, None);
    }
    DispatchResult::Failure(fault) => {
      console_core_logging::warn!("settings-panel: applying theme borders: {fault}");
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Settings"),
        "Theme saved",
        Some("Window borders will update when Console reloads."),
      );
    }
  }

  Ok(())
}

fn choose_base(
  showing: &dyn Showing,
  theme: Theme,
  restart: &Task,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(name) = theme.name();
    let Ok(()) = update(
      showing,
      restart,
      &format!("{name} base theme applied"),
      |settings| {
        settings.base = theme;
        Ok(())
      },
    );
  };

  Ok(())
}

fn color_input(entered: &str) -> Result<Option<Rgba>, console_core_color::ParseError> {
  let entered_2 = entered.trim();

  match { ::core::primitive::bool::then_some(entered_2.is_empty(), ()) } {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  Rgba::of(entered_2).map(Some)
}

enum ColorSource {
  Base,
  Override,
}

fn color_row(
  role: ColorRole,
  displayed: Option<&str>,
  source: ColorSource,
  restart: Task,
) -> Result<Row, Never> {
  let code = displayed.map(|code| format!("#{}", code.to_ascii_uppercase()));
  let aside = match (code.as_deref(), source) {
    (Some(code_2), ColorSource::Override) => format!("{code_2} · Custom"),
    (Some(code_3), ColorSource::Base) => code_3.to_string(),
    (None, _) => String::new(),
  };
  let Ok(label) = role.label();
  let Ok(edit) = Handler::and_stay(move |showing| {
    let restart_2 = restart.clone();
    let Ok(respond) = crate::rows::answer(move |showing_2, entered| match color_input(entered) {
      Ok(color) => {
        let Ok(explicit_value) = update(
          showing_2,
          &restart_2,
          &format!("{label} color updated"),
          |settings| settings.set_color(role, color),
        );
        explicit_value
      }
      Err(_invalid_color) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Settings"),
          "Enter six hex digits or leave blank to reset",
          None,
        );
      }
    });
    let initial = match code.as_deref() {
      Some(code_4) => code_4,
      None => "",
    };
    showing.ask_aloud(
      &format!("{label} (hex, blank for base color)"),
      Initial(initial),
      respond,
    );
  });

  Row::new(label, Aside(&aside), edit)
}

pub fn rows() -> Result<Vec<Row>, Never> {
  let Ok(current) = ThemeSettings::current();
  let Ok(restart) = Command::external(
    Program::Systemctl,
    &[
      "--user",
      "--no-block",
      "try-restart",
      "console-home.service",
      "console-input-keyboard.service",
      "console-notify.service",
    ],
  );
  let Ok(restart_2) = Task::of(restart);
  let Ok(heading) = Row::naming("Base theme", Aside(""));
  let mut rows = vec![heading];

  for theme in Theme::EVERY {
    let aside = match { ::core::primitive::bool::then_some(theme == current.base, ()) } {
      ::core::option::Option::Some(()) => NOW,
      ::core::option::Option::None => "",
    };
    let restart_3 = restart_2.clone();
    let Ok(choose) = Handler::and_stay(move |showing| {
      let Ok(explicit_value) = choose_base(showing, theme, &restart_3);
      explicit_value
    });
    let Ok(name) = theme.name();
    let Ok(row) = Row::new(name, Aside(aside), choose);
    rows.push(row);
  }

  let base_restart = restart_2.clone();
  let Ok(edit) = Handler::and_stay(move |showing| {
    let Ok(current_2) = ThemeSettings::current();
    let Ok(custom) = current_2.base.custom_color();
    let color = match custom {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => ::std::string::String::new(),
    };
    let restart_4 = base_restart.clone();
    let Ok(respond) =
      crate::rows::answer(move |showing_2, entered| match Theme::custom(entered) {
        Ok(theme) => {
          let Ok(explicit_value) = choose_base(showing_2, theme, &restart_4);
          explicit_value
        }
        Err(_invalid_base) => {
          let Ok(()) = console_notifications::notify(
            console_notifications::Application("Settings"),
            "Enter a six-digit hex color, e.g. #6AABCF",
            None,
          );
        }
      });
    showing.ask_aloud("Base theme color (hex)", Initial(&color), respond);
  });
  let aside = match current.base {
    Theme::Custom(_) => NOW,
    Theme::Blossom | Theme::Ocean | Theme::Forest | Theme::Slate => "",
  };
  let Ok(custom) = Row::new("Custom base color", Aside(aside), edit);
  rows.push(custom);

  let Ok(heading_2) = Row::naming("Individual colors", Aside(""));
  rows.push(heading_2);
  let colors = match palette::spent_with(&current) {
    Ok(colors) => colors,
    Err(fault) => {
      console_core_logging::warn!("settings-panel: loading theme colors: {fault}");
      Default::default()
    }
  };

  for role in ColorRole::EVERY {
    let Ok(key) = role.key();
    let code = colors.get(key).map(String::as_str);
    let Ok(overridden) = current.color(role);
    let source = match overridden {
      Some(_) => ColorSource::Override,
      None => ColorSource::Base,
    };
    let Ok(row) = color_row(role, code, source, restart_2.clone());
    rows.push(row);
  }

  let Ok(overrides) = current.overrides();

  match overrides {
    Overrides::Active => {
      let Ok(reset) = Handler::and_stay(move |showing| {
        let restart_5 = restart_2.clone();
        showing.sure(
          "Reset all individual colors?",
          Subject(""),
          &["Reset colors"],
          Box::new(move |showing_3, _| {
            let Ok(()) = update(
              showing_3,
              &restart_5,
              "Base theme colors restored",
              ThemeSettings::reset_colors,
            );
          }),
        );
      });
      let Ok(row) = Row::new("Reset individual colors", Aside(""), reset);
      rows.push(row);
    }
    Overrides::None => {}
  }

  Ok(rows)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_color_field_accepts_hex_and_uses_a_blank_to_reset()
  -> Result<(), console_core_color::ParseError> {
    let mixed_case = Rgba::of("fedcba")?;
    assert_eq!(color_input(" #fEDcBa "), Ok(Some(mixed_case)));
    assert_eq!(color_input("   "), Ok(None));
    assert!(matches!(
      color_input("#rgb"),
      Err(console_core_color::ParseError::Length(_))
    ));

    Ok(())
  }
}
