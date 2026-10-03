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
//! when it agrees and is waited past when it does not. An opening waits until
//! the program it started ends, because a panel's program lives exactly as
//! long as its surface: ended, it either drew and went or never drew, and the
//! compositor's next word is taken as it stands. A closing waits for the
//! compositor alone. The program that puts a panel away ends once the screen is
//! let go, but the compositor may still be listing the surface when it does,
//! and taking that answer is the slot lighting again under the thumb that put
//! it out.
//! The keyboard is the one whose program does not: it asks the keyboard that is
//! always running and leaves, so its ending says nothing and only the
//! compositor does. A slot is lit while its panel is `Opening` or `Open`.
//!
//! Every other panel is a picker, and there is one picker at a time: the one
//! on the screen goes and the new one takes its place. So a press that opens
//! one puts every other picker into `Closing` at once, which is what keeps a
//! settings tab tapped from lighting beside the tab it is about to replace.

use std::collections::{BTreeMap, BTreeSet};

use console_compositor::Layer;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue};
use console_onscreen::Up;

use crate::reading::StatusItem;
use crate::showing::{BarAction, Lit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Panel {
    Launcher,
    Keyboard,
    Music,
    Notifications,
    Calendar,
    Settings(StatusItem),
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
    Closing,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presentation {
    AsAPicker,
    ByTheKeyboard,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Panels {
    phases: BTreeMap<Panel, Phase>,
}

impl Panel {
    pub fn of(action: BarAction) -> Result<Option<Panel>, Never> {
        Ok(match action {
            BarAction::Launcher => Some(Panel::Launcher),
            BarAction::Keyboard => Some(Panel::Keyboard),
            BarAction::Music => Some(Panel::Music),
            BarAction::Notifications => Some(Panel::Notifications),
            BarAction::Calendar => Some(Panel::Calendar),
            BarAction::Settings(item) => Some(Panel::Settings(item)),
            BarAction::Workspace(_) | BarAction::Menu(_) | BarAction::Volume(_) | BarAction::Power(_) => None,
        })
    }

    pub fn action(self) -> Result<BarAction, Never> {
        Ok(match self {
            Panel::Launcher => BarAction::Launcher,
            Panel::Keyboard => BarAction::Keyboard,
            Panel::Music => BarAction::Music,
            Panel::Notifications => BarAction::Notifications,
            Panel::Calendar => BarAction::Calendar,
            Panel::Settings(item) => BarAction::Settings(item),
        })
    }

    fn drawn(self) -> Result<Presentation, Never> {
        Ok(match self {
            Panel::Keyboard => Presentation::ByTheKeyboard,
            Panel::Launcher | Panel::Music | Panel::Notifications | Panel::Calendar | Panel::Settings(_) => {
                Presentation::AsAPicker
            }
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
        let Ok(phase) = self.phase(panel);

        Ok(match phase {
            Phase::Opening(_) | Phase::Open => Lit::Yes,
            Phase::Closed | Phase::Closing => Lit::No,
        })
    }

    fn with(mut self, panel: Panel, phase: Phase) -> Result<Self, Never> {
        match phase {
            Phase::Closed => {
                let _was = self.phases.remove(&panel);
            }
            Phase::Opening(_) | Phase::Open | Phase::Closing => {
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
                        let Ok(drawn) = panel.drawn();
                        let Ok(next) = displaced(phase, drawn);

                        (panel, next)
                    })
                    .collect(),
            }),
        }
    }

    fn shown(self, up: &[Panel]) -> Result<Self, Never> {
        let up: BTreeSet<Panel> = up.iter().copied().collect();
        let arrived = up
            .iter()
            .filter(|panel| !self.phases.contains_key(panel))
            .map(|panel| (*panel, Phase::Closed));
        let phases = self
            .phases
            .iter()
            .map(|(panel, phase)| (*panel, *phase))
            .chain(arrived)
            .filter_map(|(panel, phase)| {
                let standing = match up.contains(&panel) {
                    true => Up::OnScreen,
                    false => Up::NotThere,
                };
                let Ok(seen) = seen(phase, standing);

                match seen {
                    Phase::Closed => None,
                    Phase::Opening(_) | Phase::Open | Phase::Closing => Some((panel, seen)),
                }
            })
            .collect();

        Ok(Panels { phases })
    }
}

fn pressed(phase: Phase) -> Result<Phase, Never> {
    Ok(match phase {
        Phase::Closed | Phase::Closing => Phase::Opening(Requested::Running),
        Phase::Open | Phase::Opening(_) => Phase::Closing,
    })
}

fn displaced(phase: Phase, drawn: Presentation) -> Result<Phase, Never> {
    Ok(match (phase, drawn) {
        (Phase::Open | Phase::Opening(_), Presentation::AsAPicker) => Phase::Closing,
        (Phase::Closed | Phase::Closing, Presentation::AsAPicker)
        | (Phase::Closed | Phase::Open | Phase::Opening(_) | Phase::Closing, Presentation::ByTheKeyboard) => phase,
    })
}

fn ended(phase: Phase) -> Result<Phase, Never> {
    Ok(match phase {
        Phase::Opening(_) => Phase::Opening(Requested::Ended),
        Phase::Closed | Phase::Open | Phase::Closing => phase,
    })
}

fn seen(phase: Phase, standing: Up) -> Result<Phase, Never> {
    Ok(match (phase, standing) {
        (Phase::Opening(Requested::Running), Up::NotThere) | (Phase::Closing, Up::OnScreen) => phase,
        (Phase::Closed | Phase::Open | Phase::Opening(_), Up::OnScreen) => Phase::Open,
        (Phase::Closed | Phase::Open | Phase::Closing | Phase::Opening(Requested::Ended), Up::NotThere) => Phase::Closed,
    })
}

impl Machine for Panels {
    type Input = ();
    type State = Panels;
    type Request = PanelEvent;
    type Effect = PanelEffect;

    fn initialize(_input: &(), _previous: Option<Panels>, _effects: &mut Queue<PanelEffect>) -> Result<Panels, Never> {
        Ok(Panels { phases: BTreeMap::new() })
    }

    fn handle(panels: Panels, event: PanelEvent, effects: &mut Queue<PanelEffect>) -> Result<Panels, Never> {
        match event {
            PanelEvent::Pressed(panel) => {
                let Ok(phase) = panels.phase(panel);
                let Ok(next) = pressed(phase);
                let Ok(()) = effects.offer(PanelEffect::Start(panel));
                let Ok(cleared) = match next {
                    Phase::Opening(_) => panels.displaced_by(panel),
                    Phase::Closed | Phase::Open | Phase::Closing => Ok(panels),
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
    let Ok(music) = InternalProgram::MusicPanel.name();
    let Ok(notifications) = InternalProgram::NotificationsPanel.name();
    let Ok(calendar) = InternalProgram::CalendarPanel.name();
    let Ok(settings) = InternalProgram::SettingsPanel.name();
    let named = [
        (Panel::Launcher, up(launcher)),
        (Panel::Keyboard, up(console_onscreen::KEYBOARD)),
        (Panel::Music, up(music)),
        (Panel::Notifications, up(notifications)),
        (Panel::Calendar, up(calendar)),
    ];
    let settings_up = up(settings);
    let tabs = crate::state::ALONG.iter().map(|item| {
        let Ok(mine) = item.tab();
        let front = match (settings_up, tab == Some(mine)) {
            (Up::OnScreen, true) => Up::OnScreen,
            (Up::OnScreen, false) | (Up::NotThere, _) => Up::NotThere,
        };

        (Panel::Settings(*item), front)
    });

    Ok(named
        .into_iter()
        .chain(tabs)
        .filter_map(|(panel, standing)| match standing {
            Up::OnScreen => Some(panel),
            Up::NotThere => None,
        })
        .collect())
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
        assert_eq!(panels.lit(Panel::Launcher), Ok(Lit::Yes), "the first press went dark under the thumb that pressed it");
    }

    #[test]
    fn a_press_asks_for_its_panel_whichever_way_it_goes() {
        let Ok(trace) = run_from::<Panels>(Panels::default(), &[
            PanelEvent::Pressed(Panel::Calendar),
            PanelEvent::Pressed(Panel::Calendar),
        ]);
        assert_eq!(trace.effects(), Ok(vec![PanelEffect::Start(Panel::Calendar), PanelEffect::Start(Panel::Calendar)]));
        assert_eq!(trace.state.lit(Panel::Calendar), Ok(Lit::No), "a second press puts it away");
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
    fn a_panel_put_away_stays_dark_while_the_screen_still_shows_it() {
        let Ok(panels) = after(&[
            PanelEvent::Shown(vec![Panel::Notifications]),
            PanelEvent::Pressed(Panel::Notifications),
            PanelEvent::Shown(vec![Panel::Notifications]),
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
    fn a_panel_put_away_is_not_lit_again_by_a_compositor_that_is_still_listing_it() {
        let Ok(panels) = after(&[
            PanelEvent::Shown(vec![Panel::Calendar]),
            PanelEvent::Pressed(Panel::Calendar),
            PanelEvent::Ended(Panel::Calendar),
            PanelEvent::Shown(vec![Panel::Calendar]),
        ]);

        assert_eq!(panels.lit(Panel::Calendar), Ok(Lit::No));
    }

    #[test]
    fn a_tab_pressed_puts_out_the_tab_it_replaces_at_once() {
        let sound = Panel::Settings(StatusItem::Sound);
        let wifi = Panel::Settings(StatusItem::Network);
        let Ok(panels) = after(&[
            PanelEvent::Shown(vec![sound]),
            PanelEvent::Pressed(wifi),
            PanelEvent::Shown(vec![sound]),
        ]);

        assert_eq!(panels.lit(wifi), Ok(Lit::Yes));
        assert_eq!(panels.lit(sound), Ok(Lit::No));

        let Ok(switched) = after(&[
            PanelEvent::Shown(vec![sound]),
            PanelEvent::Pressed(wifi),
            PanelEvent::Shown(vec![wifi]),
        ]);

        assert_eq!(switched.phase(wifi), Ok(Phase::Open));
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
        let Ok(sound) = StatusItem::Sound.tab();

        assert_eq!(on_screen(&layers, Some(sound)), Ok(vec![Panel::Settings(StatusItem::Sound)]));
        assert_eq!(on_screen(&layers, None), Ok(vec![]));
        assert_eq!(on_screen(&[], Some(sound)), Ok(vec![]));
    }
}
