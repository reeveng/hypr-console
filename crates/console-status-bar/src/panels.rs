//! Whether each panel the bar opens is open, as one machine.
//!
//! The bar held one `Open` and three things wrote it: a press flipped a slot,
//! the compositor's answer replaced the lot, and one promise -- the last press,
//! and only the last -- was laid back over it until the compositor agreed or
//! the program the press started ended. A second press before the first panel
//! came up dropped the first promise, so the compositor's next answer put the
//! first slot out under the thumb that had pressed it, and lit it again a
//! moment later when the surface arrived.
//!
//! Here every panel has a phase of its own. A press moves it to `Opening` or
//! `Closing` and asks for its program; what the compositor says moves it on
//! when it agrees. An opening waits until
//! the program it started ends, because a panel's program lives exactly as
//! long as its surface: ended, it either drew and went or never drew, and the
//! compositor's next word is taken as it stands. The old picker stays dark
//! while a close or replacement is pending. A later observation restores the
//! indicator when the attempt ended and the old picker is still visible.
//! The keyboard is the one whose program does not: it asks the keyboard that is
//! always running and leaves, so its ending says nothing and only the
//! compositor does. Most slots give immediate feedback from their phase.
//! The clock indicates when the calendar is visible, so it follows the
//! latest compositor observation through both opening and dismissal.
//!
//! Every other panel is a picker, and there is one picker at a time: the one
//! on the screen goes and the new one takes its place. So a press that opens
//! one puts every other picker into `Closing` at once, which is what keeps a
//! settings tab tapped from lighting beside the tab it is about to replace.

use std::collections::{BTreeMap, BTreeSet};

use console_compositor::Layer;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_onscreen::Up;

use crate::settings::SettingsTab;
use crate::showing::{BarAction, Lit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Panel {
  Launcher,
  Keyboard,
  Clipboard,
  Music,
  Notifications,
  Calendar,
  Settings(SettingsTab),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requested {
  Running,
  Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
  Closed,
  Opening(Requested),
  Open,
  Closing(Closing),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closing {
  ByPress(Requested),
  ForPicker(Panel),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelEvent {
  Pressed(Panel),
  Shown(Vec<Panel>),
  Ended(Panel),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEffect {
  Start(Panel),
}

impl EffectDependencies for PanelEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Start(_) => &[Service::Panels],
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
  AsAPicker,
  ByTheKeyboard,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Panels {
  phases: BTreeMap<Panel, Phase>,
  visible: BTreeSet<Panel>,
}

impl Panel {
  pub fn of(action: BarAction) -> Result<Option<Panel>, Never> {
    Ok(match action {
      BarAction::Launcher => Some(Panel::Launcher),
      BarAction::Keyboard => Some(Panel::Keyboard),
      BarAction::Clipboard => Some(Panel::Clipboard),
      BarAction::Music => Some(Panel::Music),
      BarAction::Notifications => Some(Panel::Notifications),
      BarAction::Calendar => Some(Panel::Calendar),
      BarAction::Settings(item) => Some(Panel::Settings(item)),
      BarAction::Workspace(_)
      | BarAction::Applet(_)
      | BarAction::Menu(_)
      | BarAction::Volume(_)
      | BarAction::Power(_) => None,
    })
  }

  pub fn action(self) -> Result<BarAction, Never> {
    Ok(match self {
      Panel::Launcher => BarAction::Launcher,
      Panel::Keyboard => BarAction::Keyboard,
      Panel::Clipboard => BarAction::Clipboard,
      Panel::Music => BarAction::Music,
      Panel::Notifications => BarAction::Notifications,
      Panel::Calendar => BarAction::Calendar,
      Panel::Settings(item) => BarAction::Settings(item),
    })
  }

  fn drawn(self) -> Result<Presentation, Never> {
    Ok(match self {
      Panel::Keyboard => Presentation::ByTheKeyboard,
      Panel::Launcher
      | Panel::Clipboard
      | Panel::Music
      | Panel::Notifications
      | Panel::Calendar
      | Panel::Settings(_) => Presentation::AsAPicker,
    })
  }
}

impl Panels {
  pub fn phase(&self, panel: Panel) -> Result<Phase, Never> {
    Ok(match self.phases.get(&panel) {
      Some(phase) => *phase,
      None => Phase::Closed,
    })
  }

  pub fn lit(&self, panel: Panel) -> Result<Lit, Never> {
    let lit = match panel {
      Panel::Calendar => self.visible.contains(&panel).then_some(()),
      Panel::Launcher
      | Panel::Keyboard
      | Panel::Clipboard
      | Panel::Music
      | Panel::Notifications
      | Panel::Settings(_) => {
        let Ok(phase) = self.phase(panel);

        match phase {
          Phase::Opening(_) | Phase::Open => Some(()),
          Phase::Closed | Phase::Closing(_) => None,
        }
      }
    };

    Ok(match lit {
      Some(()) => Lit::Yes,
      None => Lit::No,
    })
  }

  fn with(mut self, panel: Panel, phase: Phase) -> Result<Self, Never> {
    match phase {
      Phase::Closed => {
        let _was = self.phases.remove(&panel);
      }
      Phase::Opening(_) | Phase::Open | Phase::Closing(_) => {
        let _was = self.phases.insert(panel, phase);
      }
    }

    Ok(self)
  }

  fn displaced_by(self, coming: Panel) -> Result<Self, Never> {
    let Ok(drawn) = coming.drawn();

    match drawn {
      Presentation::ByTheKeyboard => Ok(self),
      Presentation::AsAPicker => Ok(Panels {
        phases: self
          .phases
          .into_iter()
          .map(|(panel, phase)| {
            let Ok(drawn_2) = panel.drawn();
            let Ok(next) = displaced(phase, drawn_2, coming);

            (panel, next)
          })
          .collect(),
        visible: self.visible,
      }),
    }
  }

  fn shown(self, up: &[Panel]) -> Result<Self, Never> {
    let up_2: BTreeSet<Panel> = up.iter().copied().collect();
    let arrived = up_2
      .iter()
      .filter(|panel| !self.phases.contains_key(panel))
      .map(|panel| (*panel, Phase::Closed));
    let phases = self
      .phases
      .iter()
      .map(|(panel, phase)| (*panel, *phase))
      .chain(arrived)
      .filter_map(|(panel, phase)| {
        let standing = match ::core::primitive::bool::then_some(up_2.contains(&panel), ()) {
          ::core::option::Option::Some(()) => Up::OnScreen,
          ::core::option::Option::None => Up::NotThere,
        };
        let Ok(seen) = self.seen(panel, phase, standing, &up_2);

        match seen {
          Phase::Closed => None,
          Phase::Opening(_) | Phase::Open | Phase::Closing(_) => Some((panel, seen)),
        }
      })
      .collect();

    Ok(Panels {
      phases,
      visible: up_2,
    })
  }

  fn seen(
    &self,
    panel: Panel,
    phase: Phase,
    standing: Up,
    up: &BTreeSet<Panel>,
  ) -> Result<Phase, Never> {
    let Ok(drawn) = panel.drawn();

    Ok(match (phase, standing, drawn) {
      (Phase::Opening(Requested::Running), Up::NotThere, _)
      | (
        Phase::Closing(Closing::ByPress(Requested::Running)),
        Up::OnScreen,
        Presentation::AsAPicker,
      ) => phase,
      (Phase::Closing(Closing::ForPicker(replacing)), Up::OnScreen, _) => {
        let Ok(replacement) = self.phase(replacing);

        match (replacement, up.contains(&replacing).then_some(())) {
          (Phase::Opening(Requested::Running), _) | (_, Some(())) => phase,
          (_, None) => Phase::Open,
        }
      }
      (_, Up::OnScreen, _) => Phase::Open,
      (_, Up::NotThere, _) => Phase::Closed,
    })
  }
}

fn pressed(phase: Phase) -> Result<Phase, Never> {
  Ok(match phase {
    Phase::Closed | Phase::Closing(_) => Phase::Opening(Requested::Running),
    Phase::Open | Phase::Opening(_) => Phase::Closing(Closing::ByPress(Requested::Running)),
  })
}

fn displaced(phase: Phase, drawn: Presentation, coming: Panel) -> Result<Phase, Never> {
  Ok(match (phase, drawn) {
    (
      Phase::Open | Phase::Opening(_) | Phase::Closing(Closing::ForPicker(_)),
      Presentation::AsAPicker,
    ) => Phase::Closing(Closing::ForPicker(coming)),
    (Phase::Closed | Phase::Closing(Closing::ByPress(_)), Presentation::AsAPicker)
    | (_, Presentation::ByTheKeyboard) => phase,
  })
}

fn ended(phase: Phase) -> Result<Phase, Never> {
  Ok(match phase {
    Phase::Opening(_) => Phase::Opening(Requested::Ended),
    Phase::Closing(Closing::ByPress(_)) => Phase::Closing(Closing::ByPress(Requested::Ended)),
    Phase::Closed | Phase::Open | Phase::Closing(Closing::ForPicker(_)) => phase,
  })
}

impl Machine for Panels {
  type Input = ();
  type State = Panels;
  type Request = PanelEvent;
  type Effect = PanelEffect;

  fn initialize(
    _input: &(),
    _previous: Option<Panels>,
    _effects: &mut Queue<PanelEffect>,
  ) -> Result<Panels, Never> {
    Ok(Panels::default())
  }

  fn handle(
    panels: Panels,
    event: PanelEvent,
    effects: &mut Queue<PanelEffect>,
  ) -> Result<Panels, Never> {
    match event {
      PanelEvent::Pressed(panel) => {
        let Ok(phase) = panels.phase(panel);
        let Ok(next) = pressed(phase);
        let Ok(()) = effects.offer(PanelEffect::Start(panel));
        let Ok(cleared) = match next {
          Phase::Opening(_) => panels.displaced_by(panel),
          Phase::Closed | Phase::Open | Phase::Closing(_) => Ok(panels),
        };

        cleared.with(panel, next)
      }
      PanelEvent::Ended(panel) => {
        let Ok(phase) = panels.phase(panel);
        let Ok(drawn) = panel.drawn();
        let Ok(next) = match drawn {
          Presentation::AsAPicker => ended(phase),
          Presentation::ByTheKeyboard => Ok(phase),
        };

        panels.with(panel, next)
      }
      PanelEvent::Shown(up) => panels.shown(&up),
    }
  }
}

pub fn on_screen(layers: &[Layer], tab: Option<&str>) -> Result<Vec<Panel>, Never> {
  let up = |namespace: &str| {
    let Ok(up) = console_onscreen::up(layers, namespace);

    up
  };
  let Ok(launcher) = InternalProgram::Launcher.name();
  let Ok(clipboard) = InternalProgram::Clipboard.name();
  let Ok(music) = InternalProgram::MusicPanel.name();
  let Ok(notifications) = InternalProgram::NotificationsPanel.name();
  let Ok(calendar) = InternalProgram::CalendarPanel.name();
  let Ok(settings) = InternalProgram::SettingsPanel.name();
  let named = [
    (Panel::Launcher, up(launcher)),
    (Panel::Keyboard, up(console_onscreen::KEYBOARD)),
    (Panel::Clipboard, up(clipboard)),
    (Panel::Music, up(music)),
    (Panel::Notifications, up(notifications)),
    (Panel::Calendar, up(calendar)),
  ];
  let settings_tab = match (up(settings), tab) {
    (Up::OnScreen, Some(front)) => SettingsTab::EVERY
      .into_iter()
      .find(|tab_2| {
        let Ok(title) = tab_2.title();

        title == front
      })
      .map(Panel::Settings),
    (Up::OnScreen, None) | (Up::NotThere, _) => None,
  };

  Ok(
    named
      .into_iter()
      .filter_map(|(panel, standing)| match standing {
        Up::OnScreen => Some(panel),
        Up::NotThere => None,
      })
      .chain(settings_tab)
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_state_machine::run_from;

  fn after(events: &[PanelEvent]) -> Result<Panels, Never> {
    let Ok(trace) = run_from::<Panels>(Panels::default(), events);

    Ok(trace.state)
  }

  #[test]
  fn a_second_press_before_the_first_panel_is_up_keeps_both_lit() {
    let Ok(panels) = after(&[
      PanelEvent::Pressed(Panel::Launcher),
      PanelEvent::Pressed(Panel::Keyboard),
      PanelEvent::Shown(vec![]),
    ]);

    assert_eq!(panels.lit(Panel::Keyboard), Ok(Lit::Yes));
    assert_eq!(
      panels.lit(Panel::Launcher),
      Ok(Lit::Yes),
      "the first press went dark under the thumb that pressed it"
    );
  }

  #[test]
  fn a_press_asks_for_its_panel_whichever_way_it_goes() {
    let Ok(trace) = run_from::<Panels>(
      Panels::default(),
      &[
        PanelEvent::Pressed(Panel::Calendar),
        PanelEvent::Pressed(Panel::Calendar),
      ],
    );
    assert_eq!(
      trace.effects(),
      Ok(vec![
        PanelEffect::Start(Panel::Calendar),
        PanelEffect::Start(Panel::Calendar)
      ])
    );
    assert_eq!(
      trace.state.lit(Panel::Calendar),
      Ok(Lit::No),
      "the compositor has never shown the calendar"
    );
  }

  #[test]
  fn the_clock_follows_the_calendar_through_open_and_close_requests() {
    let calendar = Panel::Calendar;
    let Ok(opening) = after(&[PanelEvent::Pressed(calendar), PanelEvent::Shown(vec![])]);
    assert_eq!(
      opening.phase(calendar),
      Ok(Phase::Opening(Requested::Running))
    );
    assert_eq!(opening.lit(calendar), Ok(Lit::No));

    let mut panels = opening;

    for (event, lit) in [
      (PanelEvent::Shown(vec![calendar]), Lit::Yes),
      (PanelEvent::Pressed(calendar), Lit::Yes),
      (PanelEvent::Ended(calendar), Lit::Yes),
      (PanelEvent::Shown(vec![]), Lit::No),
    ] {
      let Ok(next) = Panels::transition(panels, event);
      panels = next.state;
      assert_eq!(panels.lit(calendar), Ok(lit));
    }
  }

  #[test]
  fn a_panel_whose_program_ended_without_it_is_let_go() {
    let Ok(panels) = after(&[
      PanelEvent::Pressed(Panel::Music),
      PanelEvent::Ended(Panel::Music),
      PanelEvent::Shown(vec![]),
    ]);

    assert_eq!(panels.phase(Panel::Music), Ok(Phase::Closed));
  }

  #[test]
  fn the_keyboard_waits_for_the_screen_because_its_program_only_asks() {
    let Ok(panels) = after(&[
      PanelEvent::Pressed(Panel::Keyboard),
      PanelEvent::Ended(Panel::Keyboard),
      PanelEvent::Shown(vec![]),
    ]);

    assert_eq!(panels.lit(Panel::Keyboard), Ok(Lit::Yes));

    let Ok(up) = after(&[
      PanelEvent::Pressed(Panel::Keyboard),
      PanelEvent::Ended(Panel::Keyboard),
      PanelEvent::Shown(vec![Panel::Keyboard]),
    ]);

    assert_eq!(up.phase(Panel::Keyboard), Ok(Phase::Open));
  }

  #[test]
  fn a_panel_dims_on_press_and_closes_when_it_leaves_the_screen() {
    let Ok(panels) = after(&[
      PanelEvent::Shown(vec![Panel::Notifications]),
      PanelEvent::Pressed(Panel::Notifications),
    ]);

    assert_eq!(panels.lit(Panel::Notifications), Ok(Lit::No));

    let Ok(gone) = after(&[
      PanelEvent::Shown(vec![Panel::Notifications]),
      PanelEvent::Pressed(Panel::Notifications),
      PanelEvent::Shown(vec![]),
    ]);

    assert_eq!(gone.phase(Panel::Notifications), Ok(Phase::Closed));
  }

  #[test]
  fn a_launcher_that_is_still_visible_after_a_close_attempt_lights_again() {
    let Ok(panels) = after(&[
      PanelEvent::Shown(vec![Panel::Launcher]),
      PanelEvent::Pressed(Panel::Launcher),
      PanelEvent::Ended(Panel::Launcher),
      PanelEvent::Shown(vec![Panel::Launcher]),
    ]);

    assert_eq!(panels.phase(Panel::Launcher), Ok(Phase::Open));
  }

  #[test]
  fn a_launcher_close_stays_dark_while_the_compositor_still_lists_it() {
    let Ok(pending) = after(&[
      PanelEvent::Shown(vec![Panel::Launcher]),
      PanelEvent::Pressed(Panel::Launcher),
      PanelEvent::Shown(vec![Panel::Launcher]),
    ]);
    assert_eq!(pending.lit(Panel::Launcher), Ok(Lit::No));
  }

  #[test]
  fn a_picker_replacement_lights_only_the_incoming_slot_during_handoff() {
    let settings = Panel::Settings(SettingsTab::Display);
    let Ok(during) = after(&[
      PanelEvent::Shown(vec![settings]),
      PanelEvent::Pressed(Panel::Launcher),
      PanelEvent::Shown(vec![settings]),
      PanelEvent::Shown(vec![settings, Panel::Launcher]),
    ]);

    assert_eq!(during.lit(settings), Ok(Lit::No));
    assert_eq!(during.lit(Panel::Launcher), Ok(Lit::Yes));
  }

  #[test]
  fn a_settings_panel_that_stays_visible_after_a_failed_replacement_lights_again() {
    let settings = Panel::Settings(SettingsTab::Display);
    let Ok(panels) = after(&[
      PanelEvent::Shown(vec![settings]),
      PanelEvent::Pressed(Panel::Launcher),
      PanelEvent::Ended(Panel::Launcher),
      PanelEvent::Shown(vec![settings]),
    ]);

    assert_eq!(panels.lit(settings), Ok(Lit::Yes));
    assert_eq!(panels.lit(Panel::Launcher), Ok(Lit::No));
  }

  #[test]
  fn a_tab_pressed_puts_out_the_tab_it_replaces_at_once() {
    let sound = Panel::Settings(SettingsTab::Sound);
    let display = Panel::Settings(SettingsTab::Display);
    let Ok(panels) = after(&[PanelEvent::Shown(vec![sound]), PanelEvent::Pressed(display)]);

    assert_eq!(panels.lit(display), Ok(Lit::Yes));
    assert_eq!(panels.lit(sound), Ok(Lit::No));

    let Ok(switched) = after(&[
      PanelEvent::Shown(vec![sound]),
      PanelEvent::Pressed(display),
      PanelEvent::Shown(vec![display]),
    ]);

    assert_eq!(switched.phase(display), Ok(Phase::Open));
    assert_eq!(switched.phase(sound), Ok(Phase::Closed));
  }

  #[test]
  fn a_picker_opened_leaves_the_keyboard_alone() {
    let Ok(panels) = after(&[
      PanelEvent::Shown(vec![Panel::Keyboard]),
      PanelEvent::Pressed(Panel::Launcher),
    ]);

    assert_eq!(panels.lit(Panel::Keyboard), Ok(Lit::Yes));
  }

  #[test]
  fn a_settings_slot_is_on_the_screen_only_while_its_own_tab_is_in_front() {
    let Ok(settings) = InternalProgram::SettingsPanel.name();

    let layers = vec![Layer {
      namespace: settings.to_string(),
      address: None,
      x: Some(0),
      y: Some(0),
      width: Some(640),
      height: Some(480),
    }];
    for tab in SettingsTab::EVERY {
      let Ok(title) = tab.title();
      assert_eq!(
        on_screen(&layers, Some(&title)),
        Ok(vec![Panel::Settings(tab)])
      );
    }

    assert_eq!(on_screen(&layers, None), Ok(vec![]));
    assert_eq!(on_screen(&[], Some("Display")), Ok(vec![]));
  }
}
