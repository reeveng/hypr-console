use std::path::{Path, PathBuf};

use console_core_controls::Switch;
use console_core_never::Never;
use console_panel::icons::Icon;
use console_panel::page::{Aside, Handler, NOW, Picture, Row, Showing};

use console_status_bar::applets;
use console_status_bar::preferences::{self, Item, Placement, Preferences, Visibility};
use console_status_bar::settings::SettingsTab;

pub fn rows() -> Result<Vec<Row>, Never> {
  let Ok(at) = preferences::path();

  let path = match at {
    Some(path) => path,
    None => {
      let Ok(row) = Row::text("Status bar settings unavailable", Aside(""));
      return Ok(vec![row]);
    }
  };

  let Ok(folders) = applets::folders();
  let Ok(applications) = console_core_places::applications();

  rows_at(&path, &folders, &applications)
}

fn rows_at(at: &Path, folders: &[PathBuf], applications: &[PathBuf]) -> Result<Vec<Row>, Never> {
  let preferences = match Preferences::load(at) {
    Ok(preferences) => preferences,
    Err(error) => {
      let Ok(row) = Row::text(
        "Could not read status bar settings",
        Aside(&error.to_string()),
      );
      return Ok(vec![row]);
    }
  };
  let Ok(position) = Row::naming("Position", Aside(""));
  let mut rows = vec![position];

  for placement in Placement::EVERY {
    let Ok(row) = position_row(at, preferences.placement, placement);
    rows.push(row);
  }

  let Ok(items) = Row::naming("Built-in items", Aside(""));
  rows.push(items);

  for item in Item::STANDALONE {
    let Ok(title) = item.name();
    let Ok(icon) = item.icon();
    let Ok(shown) = preferences.visibility(item);
    let Ok(row) = visibility(at, title, icon, shown, move |preferences_2| {
      let Ok(()) = preferences_2.toggle(item);
    });
    rows.push(row);
  }

  let Ok(settings) = Row::naming("Settings applets", Aside(""));
  rows.push(settings);

  for tab in SettingsTab::EVERY {
    let Ok(title) = tab.title();
    let Ok(icon) = tab.icon();
    let Ok(shown) = preferences.settings_visibility(tab);
    let Ok(row) = visibility(at, &title, icon, shown, move |preferences_3| {
      let Ok(()) = preferences_3.toggle_settings(tab);
    });
    rows.push(row);
  }

  let Ok(heading) = Row::naming("Installed applets", Aside(""));
  rows.push(heading);
  let Ok(applets) = applets::discover(folders, applications);

  match applets.as_slice() {
    [] => {
      let Ok(row) = Row::text("No applets installed", Aside(""));
      rows.push(row);
    }
    [_, ..] => {}
  }

  for applet in applets {
    let Ok(enabled) = preferences.applet_visibility(&applet.id);
    let id = applet.id;
    let Ok(row) = visibility(
      at,
      &applet.name,
      applet.settings_icon,
      enabled,
      move |preferences_4| {
        let Ok(()) = preferences_4.toggle_applet(&id);
      },
    );
    rows.push(row);
  }

  Ok(rows)
}

#[cfg(test)]
mod tests {
  use console_panel::page::{ActionOutcome, Control, Nowhere};

  use super::*;

  fn choose(row: &Row) -> Result<(), Box<dyn std::error::Error>> {
    match &row.does {
      Some(Handler::Call(action)) => assert_eq!(action(&Nowhere), ActionOutcome::Stay),
      Some(Handler::Run(_)) | None => {
        let fault = std::io::Error::other(format!("missing action for {}", row.says));
        return Err(Box::new(fault));
      }
    }

    Ok(())
  }

  #[test]
  fn bar_settings_save_the_position_and_both_kinds_of_switch()
  -> Result<(), Box<dyn std::error::Error>> {
    let folder = console_core_temporary_directories::fresh("settings-status-bar")?;
    let configuration = folder.join("config");
    std::fs::create_dir(&configuration)?;
    let at = configuration.join("status-bar.json");
    console_core_atomic_writes::whole(&at, br#"{"hidden":["battery"]}"#)?;
    let applet_at = folder.join("example.json");
    console_core_atomic_writes::whole(&applet_at, br#"{"name":"Example applet"}"#)?;
    let Ok(rows) = rows_at(&at, std::slice::from_ref(&folder), &[]);
    let row = |name: &str| {
      rows
        .iter()
        .find(|row| row.says == name)
        .ok_or("missing setting")
    };
    let top = row("Top")?;
    let bottom = row("Bottom")?;
    assert_eq!(top.aside, NOW);
    assert_eq!(bottom.aside, "");
    let clock = row("Clock")?;
    let applet = row("Example applet")?;
    assert_eq!(clock.control, Some(Control::Switch(Switch::On)));
    assert_eq!(applet.control, Some(Control::Switch(Switch::Off)));
    assert_eq!(applet.picture, Picture::Named(Icon::Applet));

    assert!(
      rows
        .iter()
        .all(|entry| entry.does.is_none() || matches!(entry.picture, Picture::Named(_)))
    );

    for tab in SettingsTab::EVERY {
      let Ok(title) = tab.title();
      let expected = match ::core::primitive::bool::then_some(
        { matches!(tab.indicator(), Ok(Some(_indicator))) && tab != SettingsTab::Battery },
        (),
      ) {
        ::core::option::Option::Some(()) => Switch::On,
        ::core::option::Option::None => Switch::Off,
      };
      let setting = row(&title)?;
      assert_eq!(setting.control, Some(Control::Switch(expected)), "{tab:?}");
    }

    for edge in [
      Placement::Left,
      Placement::Right,
      Placement::Top,
      Placement::Bottom,
    ] {
      let Ok(name) = edge.name();
      let option = row(name)?;
      choose(option)?;
      let load = Preferences::load(&at)?;
      assert_eq!(load.placement, edge);
    }

    let display = row("Display")?;

    for switch in [clock, applet, display] {
      choose(switch)?;
    }

    let saved = Preferences::load(&at)?;
    assert_eq!(saved.placement, Placement::Bottom);
    assert_eq!(saved.visibility(Item::Clock), Ok(Visibility::Hidden));
    assert_eq!(saved.applet_visibility("example"), Ok(Visibility::Shown));
    assert_eq!(
      saved.settings_visibility(SettingsTab::Display),
      Ok(Visibility::Shown)
    );
    assert_eq!(
      saved.settings_visibility(SettingsTab::Battery),
      Ok(Visibility::Hidden)
    );

    Ok(())
  }

  #[test]
  fn obs_has_an_off_switch_only_when_its_desktop_entry_exists()
  -> Result<(), Box<dyn std::error::Error>> {
    let root = console_core_temporary_directories::fresh("settings-obs-applet")?;
    let at = root.join("status-bar.json");
    let applications = [root.join("applications")];
    let [application_folder] = &applications;
    let folders = std::slice::from_ref(&root);
    console_core_atomic_writes::whole(
      &root.join("obs.json"),
      include_bytes!("../../../files/usr/local/share/console/bar-applets/obs.json"),
    )?;
    let Ok(absent) = rows_at(&at, folders, &applications);
    assert!(absent.iter().all(|row| row.says != "OBS stream status"));

    std::fs::create_dir(application_folder)?;
    console_core_atomic_writes::whole(
      &application_folder.join("com.obsproject.Studio.desktop"),
      b"[Desktop Entry]\nName=OBS Studio\n",
    )?;
    let Ok(installed) = rows_at(&at, folders, &applications);
    let setting = installed
      .iter()
      .find(|row| row.says == "OBS stream status")
      .ok_or("missing OBS switch")?;
    assert_eq!(setting.control, Some(Control::Switch(Switch::Off)));
    assert_eq!(setting.picture, Picture::Named(Icon::Film));

    Ok(())
  }
}

fn position_row(at: &Path, current: Placement, placement: Placement) -> Result<Row, Never> {
  let at_2 = at.to_path_buf();
  let aside = match (current == placement).then_some(()) {
    Some(()) => NOW,
    None => "",
  };
  let Ok(name) = placement.name();
  let Ok(icon) = placement.icon();
  let Ok(choose) = Handler::and_stay(move |showing| {
    let Ok(()) = save(&at_2, showing, |preferences| {
      preferences.placement = placement
    });
  });

  let Ok(row) = Row::new(name, Aside(aside), choose);

  row.picturing(Picture::Named(icon))
}

fn visibility(
  at: &Path,
  name: &str,
  icon: Icon,
  visible: Visibility,
  change: impl Fn(&mut Preferences) + Send + Sync + 'static,
) -> Result<Row, Never> {
  let at_2 = PathBuf::from(at);
  let Ok(handler) = Handler::and_stay(move |showing| {
    let Ok(()) = save(&at_2, showing, &change);
  });
  let Ok(row) = Row::new(name, Aside(""), handler);
  let Ok(pictured) = row.picturing(Picture::Named(icon));
  let state = match visible {
    Visibility::Shown => Switch::On,
    Visibility::Hidden => Switch::Off,
  };

  pictured.switched(state)
}

fn save(
  at: &Path,
  showing: &dyn Showing,
  change: impl FnOnce(&mut Preferences),
) -> Result<(), Never> {
  match Preferences::update(at, change) {
    Ok(()) => showing.refresh(),
    Err(error) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Settings"),
        "Could not save status bar settings",
        Some(&error.to_string()),
      );
    }
  }

  Ok(())
}
