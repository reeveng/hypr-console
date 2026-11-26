//! What the bar is holding, and the slots it makes of it.
//!
//! One process draws the whole bar, so what used to be seven programs each
//! printing a line of JSON is seven readings in one place. The reading itself
//! is `reading`, `notifications` and the music player's own; what is here is the
//! other half of every one of those programs -- which icon, which tone, whether
//! the thing this opens is already on the screen, and what a tap on it does.
//! Every one of them had said all four in a stylesheet class, an `on-click`
//! line and an icon passed on a command line.
//!
//! **Nothing here runs anything.** What the machine answered arrives as
//! [`BarState`] and this turns it into slots, which is the half worth pressing: a
//! reading that is lit when its own tab is in front, a workspace lit when it is
//! the one you are on, a battery that says two things where the others say one.
//!
//! The slot after the workspaces is one past the last of them, which is the
//! only way a thumb has of reaching a workspace that is not there yet: the
//! compositor makes one the moment it is asked to go to it. It used to be the
//! first number no one was on, which put the new one in whatever gap a closed
//! window had left rather than under the + that was tapped.
//!
//! A numbered workspace has the same ID in its label, shortcut, and click.
//! Asking the machine is the program's, because it is the half that needs a
//! machine.

use console_compositor::{Workspace, WorkspaceKind};
use console_core_never::Never;
use console_core_state_machine::{Machine, Transition};

use crate::panels::{Panel, PanelEffect, PanelEvent, Panels};
use crate::preferences::{Item, Placement, Preferences, Visibility};
use crate::reading::{Reading, StatusItem, Tone};
use crate::settings::SettingsTab;
use crate::showing::{BarAction, Face, Filling, Layout, Lit, Slot, Span};

pub const LAUNCHER: &str = "\u{f003b}";

pub const KEYBOARD: &str = "\u{f030c}";

const CLIPBOARD: &str = "\u{f014e}";

pub const MUSIC: &str = "\u{f075a}";

pub const PAUSED: &str = "\u{f03e4}";

pub const ANOTHER: &str = "+";

pub const ALONG: [StatusItem; 4] = [
  StatusItem::Sound,
  StatusItem::Bluetooth,
  StatusItem::Network,
  StatusItem::Battery,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarState {
  pub readings: Vec<(StatusItem, Reading)>,
  pub bell: Reading,
  pub music: Reading,
  pub clock: String,
  pub workspaces: Vec<Workspace>,
  pub front: Option<i64>,
  pub panels: Panels,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum FocusUpdate {
  #[default]
  Unchanged,
  NoWorkspace,
  Workspace(Workspace),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressEffect {
  Panel(PanelEffect),
  Action(BarAction),
}

pub fn music(paused: Paused, playing: Playing) -> Result<Reading, Never> {
  match (playing, paused) {
    (Playing::None, _) => Reading::new(MUSIC, Tone::Secondary),
    (Playing::Some, Paused::Yes) => Reading::new(PAUSED, Tone::Plain),
    (Playing::Some, Paused::No) => Reading::new(MUSIC, Tone::Plain),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paused {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playing {
  Some,
  None,
}

fn icon(icon: &str, tone: Tone, lit: Lit, action: BarAction) -> Result<Slot, Never> {
  Ok(Slot {
    spans: vec![Span {
      text: icon.to_string(),
      face: Face::Icon,
    }],
    tone,
    lit,
    action: Some(action),
  })
}

fn reading(reading: &Reading, lit: Lit, action: BarAction) -> Result<Slot, Never> {
  let mut spans = vec![Span {
    text: reading.icon.clone(),
    face: Face::Icon,
  }];

  match &reading.beside {
    Some(beside) => spans.push(Span {
      text: beside.clone(),
      face: Face::Small,
    }),
    None => {}
  }

  Ok(Slot {
    spans,
    tone: reading.tone,
    lit,
    action: Some(action),
  })
}

impl BarState {
  pub fn observed_workspaces(
    &mut self,
    workspaces: Option<Vec<Workspace>>,
    front: FocusUpdate,
  ) -> Result<(), Never> {
    match workspaces {
      Some(observed) => self.workspaces = observed,
      None => {}
    }

    match front {
      FocusUpdate::Workspace(workspace) => {
        let Ok(kind) = WorkspaceKind::of(&workspace.named);

        match kind {
          WorkspaceKind::Special => return Ok(()),
          WorkspaceKind::Regular => {}
        }

        self.front = Some(workspace.id);

        match self
          .workspaces
          .binary_search_by_key(&workspace.id, |one| one.id)
        {
          Ok(_already_listed) => {}
          Err(_not_listed) => {
            self.workspaces.push(workspace);
            self.workspaces.sort_by_key(|one| one.id);
          }
        }
      }
      FocusUpdate::NoWorkspace => self.front = None,
      FocusUpdate::Unchanged => {}
    }

    Ok(())
  }

  pub fn told(&mut self, event: PanelEvent) -> Result<Vec<PanelEffect>, Never> {
    let panels = std::mem::take(&mut self.panels);
    let Ok(Transition { state, effects }) = Panels::transition(panels, event);

    self.panels = state;

    Ok(effects)
  }

  pub fn pressed(&mut self, action: BarAction) -> Result<Vec<PressEffect>, Never> {
    let Ok(panel) = Panel::of(action);

    match panel {
      Some(panel_2) => {
        let Ok(effects) = self.told(PanelEvent::Pressed(panel_2));

        Ok(effects.into_iter().map(PressEffect::Panel).collect())
      }
      None => {
        let Ok(()) = match action {
          BarAction::Workspace(id) => {
            self.front = Some(id);

            Ok(())
          }
          BarAction::Launcher
          | BarAction::Keyboard
          | BarAction::Clipboard
          | BarAction::Music
          | BarAction::Notifications
          | BarAction::Calendar
          | BarAction::Settings(_)
          | BarAction::Applet(_)
          | BarAction::Menu(_)
          | BarAction::Volume(_)
          | BarAction::Power(_) => Ok::<(), Never>(()),
        };

        Ok(vec![PressEffect::Action(action)])
      }
    }
  }

  pub fn next(&self) -> Result<i64, Never> {
    let last = self
      .workspaces
      .iter()
      .map(|workspace| workspace.id)
      .filter(|id| *id > 0)
      .max();

    Ok(match last {
      Some(last_2) => last_2.saturating_add(1),
      None => 1,
    })
  }

  pub fn layout(&self, filling: Filling) -> Result<Layout, Never> {
    self.layout_with_clock(filling, &self.clock, Face::Clock)
  }

  fn layout_with_clock(
    &self,
    filling: Filling,
    clock: &str,
    face: Face,
  ) -> Result<Layout, Never> {
    let Ok(launcher) = self.panels.lit(Panel::Launcher);
    let Ok(keyboard) = self.panels.lit(Panel::Keyboard);
    let Ok(clipboard) = self.panels.lit(Panel::Clipboard);
    let Ok(music) = self.panels.lit(Panel::Music);
    let Ok(notifications) = self.panels.lit(Panel::Notifications);

    let Ok(door) = icon(LAUNCHER, Tone::Pressed, launcher, BarAction::Launcher);
    let Ok(board) = icon(KEYBOARD, Tone::Pressed, keyboard, BarAction::Keyboard);
    let Ok(history) = icon(CLIPBOARD, Tone::Pressed, clipboard, BarAction::Clipboard);
    let mut left = vec![door, board, history];

    for workspace in &self.workspaces {
      let Ok(kind) = WorkspaceKind::of(&workspace.named);

      match kind {
        WorkspaceKind::Special => continue,
        WorkspaceKind::Regular => {}
      }

      let named = match ::core::primitive::bool::then_some(workspace.id > 0, ()) {
        ::core::option::Option::Some(()) => workspace.id.to_string(),
        ::core::option::Option::None => workspace.named.clone(),
      };

      let lit = match ::core::primitive::bool::then_some(self.front == Some(workspace.id), ()) {
        ::core::option::Option::Some(()) => Lit::Yes,
        ::core::option::Option::None => Lit::No,
      };

      left.push(Slot {
        spans: vec![Span {
          text: named,
          face: Face::Reading,
        }],
        tone: Tone::Secondary,
        lit,
        action: Some(BarAction::Workspace(workspace.id)),
      });
    }

    let Ok(next) = self.next();

    left.push(Slot {
      spans: vec![Span {
        text: ANOTHER.to_string(),
        face: Face::Reading,
      }],
      tone: Tone::Secondary,
      lit: Lit::No,
      action: Some(BarAction::Workspace(next)),
    });

    let Ok(calendar) = self.panels.lit(Panel::Calendar);

    let Ok(clock_spans) = clock_spans(clock, face);
    let middle = vec![Slot {
      spans: clock_spans,
      tone: Tone::Plain,
      lit: calendar,
      action: Some(BarAction::Calendar),
    }];

    let Ok(song) = reading(&self.music, music, BarAction::Music);
    let mut right = vec![song];

    for item in ALONG {
      let found = self.readings.iter().find(|(named, _)| *named == item);

      match found {
        Some((_named, found_2)) => {
          let Ok(tab) = item.settings_tab();
          let Ok(lit) = self.panels.lit(Panel::Settings(tab));
          let Ok(slot) = reading(found_2, lit, BarAction::Settings(tab));

          right.push(slot);
        }
        None => {}
      }
    }

    let Ok(bell) = reading(&self.bell, notifications, BarAction::Notifications);

    right.push(bell);

    Ok(Layout {
      left,
      middle,
      right,
      filling,
    })
  }

  pub fn configured(&self, preferences: &Preferences, filling: Filling) -> Result<Layout, Never> {
    let (clock, face) = match preferences.placement {
      Placement::Top | Placement::Bottom => (self.clock.as_str(), Face::Clock),
      Placement::Left | Placement::Right => {
        let Ok(time) = crate::clock::time_only(&self.clock);

        (time, Face::SideClock)
      }
    };
    let Ok(mut layout) = self.layout_with_clock(filling, clock, face);

    for group in [&mut layout.left, &mut layout.middle, &mut layout.right] {
      group.retain(|slot| match slot.action {
        Some(action) => {
          let Ok(item) = item(action);

          item.is_none_or(|one| preferences.visibility(one) == Ok(Visibility::Shown))
        }
        None => true,
      });
    }

    let Ok(progress) = preferences.visibility(Item::UpdateProgress);

    match progress {
      Visibility::Hidden => layout.filling = Filling::None,
      Visibility::Shown => {}
    }

    Ok(layout)
  }

  pub fn settings_applets(&self, preferences: &Preferences) -> Result<Vec<Slot>, Never> {
    Ok(
      SettingsTab::EVERY
        .into_iter()
        .filter(|tab| {
          tab.indicator() == Ok(None)
            && preferences.settings_visibility(*tab) == Ok(Visibility::Shown)
        })
        .map(|tab| {
          let Ok(lit) = self.panels.lit(Panel::Settings(tab));
          let Ok(icon) = tab.icon();
          let Ok(glyph) = icon.glyph();

          Slot {
            spans: vec![Span {
              text: glyph.to_owned(),
              face: Face::Icon,
            }],
            tone: Tone::Plain,
            lit,
            action: Some(BarAction::Settings(tab)),
          }
        })
        .collect(),
    )
  }
}

fn clock_spans(clock: &str, face: Face) -> Result<Vec<Span>, Never> {
  let (time, suffix) = match (face, clock.split_once(' ')) {
    (Face::SideClock, Some((time, suffix))) => (time, Some(suffix)),
    (Face::Icon | Face::Clock | Face::Reading | Face::Small, _) | (Face::SideClock, None) => {
      (clock, None)
    }
  };

  Ok(
    std::iter::once(time)
      .chain(suffix)
      .map(|part| Span {
        text: part.to_owned(),
        face,
      })
      .collect(),
  )
}

fn item(action: BarAction) -> Result<Option<Item>, Never> {
  Ok(match action {
    BarAction::Launcher => Some(Item::Launcher),
    BarAction::Keyboard => Some(Item::Keyboard),
    BarAction::Clipboard => Some(Item::Clipboard),
    BarAction::Workspace(_) => Some(Item::Workspaces),
    BarAction::Calendar => Some(Item::Clock),
    BarAction::Music => Some(Item::Music),
    BarAction::Settings(tab) => return tab.indicator(),
    BarAction::Notifications => Some(Item::Notifications),
    BarAction::Applet(_) | BarAction::Menu(_) | BarAction::Volume(_) | BarAction::Power(_) => {
      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn showing(up: Vec<Panel>) -> Result<Panels, Never> {
    let Ok(Transition { state, effects: _ }) =
      Panels::transition(Panels::default(), PanelEvent::Shown(up));

    Ok(state)
  }

  fn says(icon: &str, tone: Tone) -> Result<Reading, Never> {
    Ok(Reading {
      icon: String::from(icon),
      beside: None,
      tone,
    })
  }

  fn sample_state() -> Result<BarState, Never> {
    let readings = ALONG
      .iter()
      .map(|item| {
        let Ok(reading) = says("x", Tone::Plain);

        (*item, reading)
      })
      .collect();
    let Ok(bell) = says("\u{f009c}", Tone::Secondary);
    let Ok(music) = says(MUSIC, Tone::Secondary);

    Ok(BarState {
      readings,
      bell,
      music,
      clock: String::from("14:30"),
      workspaces: vec![
        Workspace {
          id: 1,
          named: String::from("1"),
          windows: Some(0),
        },
        Workspace {
          id: 2,
          named: String::from("2"),
          windows: Some(0),
        },
      ],
      front: Some(2),
      panels: Panels::default(),
    })
  }

  fn does(slots: &[Slot]) -> Result<Vec<Option<BarAction>>, Never> {
    Ok(slots.iter().map(|slot| slot.action).collect())
  }

  #[test]
  fn the_launcher_keyboard_and_clipboard_are_before_the_workspaces() {
    let Ok(state) = sample_state();
    let Ok(layout) = state.layout(Filling::None);
    let Ok(left) = does(&layout.left);

    assert_eq!(
      left,
      [
        Some(BarAction::Launcher),
        Some(BarAction::Keyboard),
        Some(BarAction::Clipboard),
        Some(BarAction::Workspace(1)),
        Some(BarAction::Workspace(2)),
        Some(BarAction::Workspace(3))
      ]
    );
  }

  #[test]
  fn the_slot_after_the_workspaces_is_one_past_the_last_of_them() {
    let next = |ids: &[i64]| {
      let Ok(state) = sample_state();
      let held = BarState {
        workspaces: ids
          .iter()
          .map(|id| Workspace {
            id: *id,
            named: id.to_string(),
            windows: Some(0),
          })
          .collect(),
        ..state
      };
      let Ok(next) = held.next();

      next
    };

    assert_eq!(next(&[1, 2]), 3);
    assert_eq!(next(&[2, 3]), 4);
    assert_eq!(next(&[1, 3]), 4);
    assert_eq!(next(&[-98, 1]), 2);
    assert_eq!(next(&[-98]), 1);
    assert_eq!(next(&[]), 1);
  }

  #[test]
  fn an_empty_workspace_can_light_before_the_full_list_arrives() {
    let Ok(mut state) = sample_state();
    let Ok(()) = state.observed_workspaces(
      None,
      FocusUpdate::Workspace(Workspace {
        id: 9,
        named: "9".to_owned(),
        windows: Some(0),
      }),
    );
    let Ok(layout) = state.layout(Filling::None);
    let lit = layout
      .left
      .iter()
      .find(|slot| slot.action == Some(BarAction::Workspace(9)));

    assert_eq!(lit.map(|slot| slot.lit), Some(Lit::Yes));
  }

  #[test]
  fn workspace_labels_keep_their_ids_across_gaps_and_skip_special_workspaces() {
    let Ok(state) = sample_state();
    let held = BarState {
      workspaces: [
        (-1337, "Project"),
        (-99, "special"),
        (-98, "special:console-quick"),
        (2, "2"),
        (3, "3"),
        (9, "9"),
      ]
      .iter()
      .map(|(id, name)| Workspace {
        id: *id,
        named: (*name).to_owned(),
        windows: Some(1),
      })
      .collect(),
      ..state
    };
    let Ok(layout) = held.layout(Filling::None);
    let named: Vec<_> = layout
      .left
      .iter()
      .filter_map(|slot| match (slot.action, slot.spans.first()) {
        (Some(BarAction::Workspace(id)), Some(span)) => Some((span.text.as_str(), id)),
        _ => None,
      })
      .collect();

    assert_eq!(
      named,
      [
        ("Project", -1337),
        ("2", 2),
        ("3", 3),
        ("9", 9),
        (ANOTHER, 10)
      ]
    );
  }

  #[test]
  fn opening_a_special_workspace_keeps_the_underlying_workspace_lit() {
    let Ok(mut state) = sample_state();
    let Ok(()) = state.observed_workspaces(
      None,
      FocusUpdate::Workspace(Workspace {
        id: -98,
        named: "special:console-quick".to_owned(),
        windows: Some(1),
      }),
    );
    let Ok(layout) = state.layout(Filling::None);

    assert_eq!(state.front, Some(2));
    assert!(
      layout
        .left
        .iter()
        .all(|slot| slot.action != Some(BarAction::Workspace(-98)))
    );
  }

  #[test]
  fn the_readings_stand_in_one_order_and_the_bell_is_last() {
    let Ok(state) = sample_state();
    let Ok(layout) = state.layout(Filling::None);
    let Ok(right) = does(&layout.right);

    assert_eq!(
      right,
      [
        Some(BarAction::Music),
        Some(BarAction::Settings(SettingsTab::Sound)),
        Some(BarAction::Settings(SettingsTab::Bluetooth)),
        Some(BarAction::Settings(SettingsTab::Wifi)),
        Some(BarAction::Settings(SettingsTab::Battery)),
        Some(BarAction::Notifications)
      ]
    );
  }

  #[test]
  fn the_workspace_you_are_on_is_the_lit_one_and_the_rest_are_quiet() {
    let Ok(state) = sample_state();
    let Ok(layout) = state.layout(Filling::None);
    let workspaces: Vec<Lit> = layout
      .left
      .iter()
      .filter(|slot| matches!(slot.action, Some(BarAction::Workspace(_))))
      .map(|slot| slot.lit)
      .collect();

    assert_eq!(workspaces, [Lit::No, Lit::Yes, Lit::No]);
  }

  #[test]
  fn a_press_is_lit_before_the_compositor_has_said_anything() {
    let Ok(mut state) = sample_state();
    let Ok(switching) = state.pressed(BarAction::Workspace(3));
    let Ok(opening) = state.pressed(BarAction::Settings(SettingsTab::Battery));

    assert_eq!(state.front, Some(3));
    assert_eq!(
      state.panels.lit(Panel::Settings(SettingsTab::Battery)),
      Ok(Lit::Yes)
    );
    assert_eq!(switching, [PressEffect::Action(BarAction::Workspace(3))]);
    assert_eq!(
      opening,
      [PressEffect::Panel(PanelEffect::Start(Panel::Settings(
        SettingsTab::Battery
      )))]
    );
  }

  #[test]
  fn a_door_says_whether_a_tap_will_open_or_close_what_it_opens() {
    let Ok(state) = sample_state();
    let Ok(closed) = state.layout(Filling::None);
    let Ok(panels) = showing(vec![Panel::Launcher]);
    let launcher_up = BarState { panels, ..state };
    let Ok(open) = launcher_up.layout(Filling::None);
    let first = |layout: &Layout| layout.left.first().map(|slot| slot.lit);

    assert_eq!(first(&closed), Some(Lit::No));
    assert_eq!(first(&open), Some(Lit::Yes));
  }

  #[test]
  fn the_clipboard_icon_opens_history_and_can_be_hidden_in_settings() {
    let Ok(mut state) = sample_state();
    let mut preferences = Preferences::default();
    let clipboard = |state_2: &BarState, preferences_2: &Preferences| {
      let Ok(layout) = state_2.configured(preferences_2, Filling::None);
      layout
        .left
        .iter()
        .find(|slot| slot.action == Some(BarAction::Clipboard))
        .map(|slot| slot.lit)
    };

    assert_eq!(clipboard(&state, &preferences), Some(Lit::No));
    assert_eq!(
      state.pressed(BarAction::Clipboard),
      Ok(vec![PressEffect::Panel(PanelEffect::Start(
        Panel::Clipboard
      ))])
    );
    assert_eq!(clipboard(&state, &preferences), Some(Lit::Yes));
    let Ok(()) = preferences.toggle(Item::Clipboard);
    assert_eq!(clipboard(&state, &preferences), None);
  }

  #[test]
  fn a_reading_is_lit_only_while_its_own_tab_is_the_one_in_front() {
    let lit_on = |up: Vec<Panel>| {
      let Ok(state) = sample_state();
      let Ok(panels) = showing(up);
      let held = BarState { panels, ..state };
      let Ok(layout) = held.layout(Filling::None);

      layout
        .right
        .iter()
        .filter_map(|slot| match slot.action {
          Some(BarAction::Settings(item)) => Some((item, slot.lit)),
          Some(_) | None => None,
        })
        .collect::<Vec<_>>()
    };

    assert!(lit_on(vec![]).iter().all(|(_, lit)| *lit == Lit::No));
    assert_eq!(
      lit_on(vec![Panel::Settings(SettingsTab::Sound)])
        .into_iter()
        .filter(|(_, lit_2)| *lit_2 == Lit::Yes)
        .map(|(item, _)| item)
        .collect::<Vec<_>>(),
      [SettingsTab::Sound]
    );
  }

  #[test]
  fn each_opt_in_settings_applet_opens_and_lights_its_own_tab() {
    let Ok(mut state) = sample_state();
    let mut preferences = Preferences::default();
    let Ok(applets) = state.settings_applets(&preferences);
    assert!(applets.is_empty());

    for tab in SettingsTab::EVERY
      .into_iter()
      .filter(|tab| tab.indicator() == Ok(None))
    {
      let Ok(()) = preferences.toggle_settings(tab);
      let action = BarAction::Settings(tab);
      let Ok(opening) = state.pressed(action);
      let Ok(open_applets) = state.settings_applets(&preferences);
      let Ok(icon) = tab.icon();
      let Ok(glyph) = icon.glyph();

      assert_eq!(
        opening,
        [PressEffect::Panel(PanelEffect::Start(Panel::Settings(tab)))]
      );
      assert_eq!(
        open_applets,
        [Slot {
          spans: vec![Span {
            text: glyph.to_owned(),
            face: Face::Icon,
          }],
          tone: Tone::Plain,
          lit: Lit::Yes,
          action: Some(action),
        }],
      );

      let Ok(_ended) = state.told(PanelEvent::Ended(Panel::Settings(tab)));
      let Ok(_closed) = state.told(PanelEvent::Shown(Vec::new()));
      let Ok(()) = preferences.toggle_settings(tab);
    }

    let Ok(closed) = state.settings_applets(&preferences);
    assert!(closed.is_empty());
  }

  #[test]
  fn a_reading_with_something_beside_it_makes_a_slot_that_says_two_things() {
    let Ok(state) = sample_state();
    let charge = Reading {
      icon: String::from("\u{f0079}"),
      beside: Some(String::from("64%")),
      tone: Tone::Plain,
    };
    let held = BarState {
      readings: vec![(StatusItem::Battery, charge)],
      ..state
    };
    let Ok(layout) = held.layout(Filling::None);
    let faces: Vec<Vec<Face>> = layout
      .right
      .iter()
      .filter(|slot| slot.action == Some(BarAction::Settings(SettingsTab::Battery)))
      .map(|slot| slot.spans.iter().map(|span| span.face).collect())
      .collect();

    assert_eq!(faces, [vec![Face::Icon, Face::Small]]);
  }

  #[test]
  fn a_reading_nothing_answered_for_is_left_off_rather_than_drawn_blank() {
    let Ok(state) = sample_state();
    let held = BarState {
      readings: Vec::new(),
      ..state
    };
    let Ok(layout) = held.layout(Filling::None);
    let Ok(right) = does(&layout.right);

    assert_eq!(
      right,
      [Some(BarAction::Music), Some(BarAction::Notifications)]
    );
  }

  #[test]
  fn the_clock_is_the_only_thing_in_the_middle_and_it_opens_the_calendar() {
    let Ok(state) = sample_state();
    let Ok(layout) = state.layout(Filling::None);
    let Ok(middle) = does(&layout.middle);

    assert_eq!(middle, [Some(BarAction::Calendar)]);
  }

  #[test]
  fn the_clock_is_lit_while_the_calendar_it_opens_is_up() {
    let Ok(state) = sample_state();
    let Ok(closed) = state.layout(Filling::None);
    let Ok(panels) = showing(vec![Panel::Calendar]);
    let up = BarState { panels, ..state };
    let Ok(open) = up.layout(Filling::None);
    let lit = |layout: &Layout| {
      layout
        .middle
        .iter()
        .map(|slot| slot.lit)
        .collect::<Vec<_>>()
    };

    assert_eq!(lit(&closed), [Lit::No]);
    assert_eq!(lit(&open), [Lit::Yes]);
  }

  #[test]
  fn nothing_playing_is_the_note_rather_than_no_note_at_all() {
    let Ok(nothing) = music(Paused::No, Playing::None);
    let Ok(paused) = music(Paused::Yes, Playing::Some);
    let Ok(going) = music(Paused::No, Playing::Some);

    assert_eq!(nothing.icon, MUSIC);
    assert_eq!(nothing.tone, Tone::Secondary);
    assert_ne!(paused.icon, going.icon);
    assert_eq!(going.tone, Tone::Plain);
  }
}
