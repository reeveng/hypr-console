use std::fmt;

use console_core_localization::text;
use console_core_never::Never;
use console_input_alphabets::{self as alphabets, Alphabet};
use console_input_language as language;
use console_panel::page::{Aside, Handler, NOW, Row, Showing};

use crate::choosing::KeyboardMenu;
use crate::rows::Chosen;
use crate::words::Word;

pub fn rows(
  menu: KeyboardMenu,
  installed: &[&'static Alphabet],
  active: &Alphabet,
  back: impl Chosen,
  open: impl Fn(KeyboardMenu, &dyn Showing) + Clone + Send + Sync + 'static,
) -> Result<Vec<Row>, Never> {
  let (parent, title) = match menu {
    KeyboardMenu::Closed => (Word::Language, Word::Keyboards),
    KeyboardMenu::Open => (Word::Keyboards, Word::Keyboards),
    KeyboardMenu::Install => (Word::Keyboards, Word::InstallKeyboardLayout),
    KeyboardMenu::Remove => (Word::Keyboards, Word::RemoveKeyboardLayout),
  };
  let Ok(parent_2) = text(&parent);
  let Ok(title_2) = text(&title);
  let Ok(way_back) = Row::back(&parent_2, back);
  let Ok(heading) = Row::naming(title_2.trim_end_matches('…'), Aside(""));
  let mut rows = vec![way_back, heading];

  match menu {
    KeyboardMenu::Closed | KeyboardMenu::Open => {
      let (next, arrow) = match menu {
        KeyboardMenu::Closed => (KeyboardMenu::Open, "▾"),
        KeyboardMenu::Open | KeyboardMenu::Install | KeyboardMenu::Remove => {
          (KeyboardMenu::Closed, "▴")
        }
      };
      let Ok(chooses) = opening(next, open.clone());
      let Ok(layout) = text(&Word::KeyboardLayout);
      let Ok(dropdown) = Row::new(&layout, Aside(&format!("{} {arrow}", active.says)), chooses);
      rows.push(dropdown);

      match menu {
        KeyboardMenu::Open => {
          let Ok(available) = installed_rows(installed, active, open);
          rows.extend(available);
        }
        KeyboardMenu::Closed | KeyboardMenu::Install | KeyboardMenu::Remove => {}
      }
    }
    KeyboardMenu::Install | KeyboardMenu::Remove => {
      for alphabet in &alphabets::EVERY {
        let present = installed.iter().any(|other| other.key == alphabet.key);
        let action = match (menu, present, alphabet.key == alphabets::LATIN) {
          (KeyboardMenu::Install, false, _) => install,
          (KeyboardMenu::Remove, true, false) => remove,
          _unavailable => continue,
        };
        let close = open.clone();
        let Ok(handler) = Handler::and_stay(move |showing| {
          let Ok(()) = finish(action(alphabet), showing, &close);
        });
        let Ok(row) = Row::new(alphabet.says, Aside(""), handler);
        rows.push(row);
      }
    }
  }

  Ok(rows)
}

fn installed_rows(
  installed: &[&'static Alphabet],
  active: &Alphabet,
  open: impl Fn(KeyboardMenu, &dyn Showing) + Clone + Send + Sync + 'static,
) -> Result<Vec<Row>, Never> {
  let mut rows = Vec::new();

  for alphabet in installed.iter().copied() {
    let close = open.clone();
    let mark = match bool::then_some(alphabet.key == active.key, ()) {
      Some(()) => NOW,
      None => "",
    };
    let Ok(choose) = Handler::and_stay(move |showing| {
      let selected = language::select(alphabet).map_err(LayoutFault::Select);
      let Ok(()) = finish(selected, showing, &close);
    });
    let Ok(row) = Row::new(alphabet.says, Aside(mark), choose);
    rows.push(row);
  }

  match bool::then_some(installed.len() < alphabets::EVERY.len(), ()) {
    Some(()) => {
      let Ok(row) = link(
        Word::InstallKeyboardLayout,
        KeyboardMenu::Install,
        open.clone(),
      );
      rows.push(row);
    }
    None => {}
  }

  match {
    bool::then_some(
      installed
        .iter()
        .any(|alphabet| alphabet.key != alphabets::LATIN),
      (),
    )
  } {
    Some(()) => {
      let Ok(row) = link(Word::RemoveKeyboardLayout, KeyboardMenu::Remove, open);
      rows.push(row);
    }
    None => {}
  }

  Ok(rows)
}

fn link(
  label: Word,
  menu: KeyboardMenu,
  open: impl Fn(KeyboardMenu, &dyn Showing) + Send + Sync + 'static,
) -> Result<Row, Never> {
  let Ok(label_2) = text(&label);
  let Ok(handler) = opening(menu, open);
  let Ok(row) = Row::new(&label_2, Aside(""), handler);

  row.opening()
}

fn opening(
  menu: KeyboardMenu,
  open: impl Fn(KeyboardMenu, &dyn Showing) + Send + Sync + 'static,
) -> Result<Handler, Never> {
  Handler::and_stay(move |showing| open(menu, showing))
}

enum LayoutFault {
  Select(language::SelectError),
  Save,
}

impl fmt::Display for LayoutFault {
  fn fmt(&self, destination: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Select(fault) => fmt::Display::fmt(fault, destination),
      Self::Save => destination.write_str("Could not save installed keyboard layouts"),
    }
  }
}

fn finish(
  result: Result<(), LayoutFault>,
  showing: &dyn Showing,
  open: &impl Fn(KeyboardMenu, &dyn Showing),
) -> Result<(), Never> {
  match result {
    Ok(()) => open(KeyboardMenu::Closed, showing),
    Err(fault) => {
      console_core_logging::warn!("could not change the keyboard layout: {fault}");
      showing.note(&fault.to_string());
      showing.refresh();
    }
  }

  Ok(())
}

fn install(alphabet: &Alphabet) -> Result<(), LayoutFault> {
  save_installation(alphabet, Installation::Present)?;

  language::select(alphabet).map_err(LayoutFault::Select)
}

fn remove(alphabet: &Alphabet) -> Result<(), LayoutFault> {
  let Ok(active) = language::active();
  let selected = match bool::then_some(active.key == alphabet.key, ()) {
    Some(()) => {
      let Ok(latin) = alphabets::latin();

      latin
    }
    None => active,
  };

  language::select(selected).map_err(LayoutFault::Select)?;
  save_installation(alphabet, Installation::Absent)?;
  let Ok(()) = language::reload_screen();

  Ok(())
}

enum Installation {
  Present,
  Absent,
}

fn save_installation(alphabet: &Alphabet, installation: Installation) -> Result<(), LayoutFault> {
  let install = matches!(installation, Installation::Present);
  let Ok(installed) = alphabets::current();
  let present = installed.iter().any(|other| other.key == alphabet.key);

  match bool::then_some(present == install, ()) {
    Some(()) => {
      return Ok(());
    }
    None => {}
  }

  let Ok(keys) = alphabets::toggle(&installed, alphabet.key);
  let Ok(()) = alphabets::choose(&keys);
  let Ok(updated) = alphabets::current();

  match bool::then_some(
    updated.iter().any(|other| other.key == alphabet.key) == install,
    (),
  ) {
    Some(()) => Ok(()),
    None => Err(LayoutFault::Save),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_dropdown_separates_installed_choices_from_installable_layouts() {
    let Ok(installed) = alphabets::read("latin,thai");
    let Ok(latin) = alphabets::latin();
    let rendered = |menu| {
      let Ok(rows) = rows(menu, &installed, latin, |_| {}, |_, _| {});

      rows
        .into_iter()
        .skip(2)
        .map(|row| (row.says, row.aside))
        .collect::<Vec<_>>()
    };

    assert_eq!(
      rendered(KeyboardMenu::Closed),
      [(String::from("Keyboard Layout"), String::from("Latin ▾"))]
    );
    let open = rendered(KeyboardMenu::Open);
    assert!(open.contains(&(String::from("Latin"), String::from(NOW))));
    assert!(open.contains(&(String::from("Thai"), String::from(""))));
    assert!(open.iter().any(|(label, _)| label == "Install Layout…"));
    assert!(!open.iter().any(|(label, _)| label == "Greek"));
    assert!(
      rendered(KeyboardMenu::Install)
        .iter()
        .any(|(label, _)| label == "Greek")
    );
    assert_eq!(
      rendered(KeyboardMenu::Remove),
      [(String::from("Thai"), String::from(""))]
    );
  }
}
