//! What the bar is holding, and the slots it makes of it.
//!
//! One process draws the whole bar, so what used to be seven programs each
//! printing a line of JSON is seven readings in one place. The reading itself
//! is `reading`, `notices` and the music player's own; what is here is the
//! other half of every one of those programs -- which icon, which tone, whether
//! the thing this opens is already on the screen, and what a tap on it does.
//! Every one of them had said all four in a stylesheet class, an `on-click`
//! line and an icon passed on a command line.
//!
//! **Nothing here runs anything.** What the machine answered arrives as
//! [`Held`] and this turns it into slots, which is the half worth pressing: a
//! reading that is lit when its own tab is in front, a workspace lit when it is
//! the one you are on, a battery that says two things where the others say one.
//! Asking the machine is the program's, because it is the half that needs a
//! machine.

use console_compositor::Workspace;
use console_core_never::Never;
use console_onscreen::Up;

use crate::reading::{Says, Tone, What};
use crate::showing::{Does, Face, Filling, Lit, Said, Saying, Slot};

pub const LAUNCHER: &str = "\u{f003b}";

pub const KEYBOARD: &str = "\u{f030c}";

pub const MUSIC: &str = "\u{f075a}";

pub const PAUSED: &str = "\u{f03e4}";

pub const ALONG: [What; 4] = [What::Sound, What::Bluetooth, What::Network, What::Battery];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    pub launcher: Up,
    pub keyboard: Up,
    pub music: Up,
    pub notices: Up,
    pub settings: Up,
    pub tab: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub readings: Vec<(What, Says)>,
    pub bell: Says,
    pub music: Says,
    pub clock: String,
    pub workspaces: Vec<Workspace>,
    pub front: Option<i64>,
    pub open: Open,
}

pub fn music(paused: Paused, playing: Playing) -> Result<Says, Never> {
    match (playing, paused) {
        (Playing::Nothing, _) => Says::new(MUSIC, Tone::Quiet),
        (Playing::Something, Paused::Yes) => Says::new(PAUSED, Tone::Plain),
        (Playing::Something, Paused::No) => Says::new(MUSIC, Tone::Plain),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paused {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playing {
    Something,
    Nothing,
}

fn lit(up: Up) -> Result<Lit, Never> {
    Ok(match up {
        Up::OnScreen => Lit::Yes,
        Up::NotThere => Lit::No,
    })
}

fn icon(said: &str, tone: Tone, lit: Lit, does: Does) -> Result<Slot, Never> {
    Ok(Slot {
        said: vec![Said { said: said.to_string(), face: Face::Icon }],
        tone,
        lit,
        does: Some(does),
    })
}

fn reading(says: &Says, lit: Lit, does: Does) -> Result<Slot, Never> {
    let mut said = vec![Said { said: says.icon.clone(), face: Face::Icon }];

    match &says.beside {
        Some(beside) => said.push(Said { said: beside.clone(), face: Face::Small }),
        None => {},
    }

    Ok(Slot { said, tone: says.tone, lit, does: Some(does) })
}

impl Open {
    fn on(&self, what: What) -> Result<Lit, Never> {
        let Ok(mine) = what.tab();

        let front = match self.settings {
            Up::NotThere => false,
            Up::OnScreen => self.tab.as_deref() == Some(mine),
        };

        Ok(match front {
            true => Lit::Yes,
            false => Lit::No,
        })
    }
}

impl Held {
    pub fn saying(&self, filling: Filling) -> Result<Saying, Never> {
        let Ok(launcher) = lit(self.open.launcher);
        let Ok(keyboard) = lit(self.open.keyboard);
        let Ok(music) = lit(self.open.music);
        let Ok(notices) = lit(self.open.notices);

        let Ok(door) = icon(LAUNCHER, Tone::Pressed, launcher, Does::Launcher);
        let Ok(board) = icon(KEYBOARD, Tone::Pressed, keyboard, Does::Keyboard);
        let mut left = vec![door, board];

        for workspace in &self.workspaces {
            let lit = match self.front == Some(workspace.id) {
                true => Lit::Yes,
                false => Lit::No,
            };

            left.push(Slot {
                said: vec![Said { said: workspace.named.clone(), face: Face::Reading }],
                tone: Tone::Quiet,
                lit,
                does: Some(Does::Workspace(workspace.id)),
            });
        }

        let middle = vec![Slot {
            said: vec![Said { said: self.clock.clone(), face: Face::Clock }],
            tone: Tone::Plain,
            lit: Lit::No,
            does: None,
        }];

        let Ok(song) = reading(&self.music, music, Does::Music);
        let mut right = vec![song];

        for what in ALONG {
            let says = self.readings.iter().find(|(named, _)| *named == what);

            match says {
                Some((_named, says)) => {
                    let Ok(lit) = self.open.on(what);
                    let Ok(slot) = reading(says, lit, Does::Settings(what));

                    right.push(slot);
                }
                None => {},
            }
        }

        let Ok(bell) = reading(&self.bell, notices, Does::Notices);

        right.push(bell);

        Ok(Saying { left, middle, right, filling })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shut() -> Open {
        Open {
            launcher: Up::NotThere,
            keyboard: Up::NotThere,
            music: Up::NotThere,
            notices: Up::NotThere,
            settings: Up::NotThere,
            tab: None,
        }
    }

    fn says(icon: &str, tone: Tone) -> Says {
        Says { icon: icon.to_string(), beside: None, tone }
    }

    fn held() -> Held {
        Held {
            readings: ALONG.iter().map(|what| (*what, says("x", Tone::Plain))).collect(),
            bell: says("\u{f009c}", Tone::Quiet),
            music: says(MUSIC, Tone::Quiet),
            clock: "14:30".to_string(),
            workspaces: vec![
                Workspace { id: 1, named: "1".to_string() },
                Workspace { id: 2, named: "2".to_string() },
            ],
            front: Some(2),
            open: shut(),
        }
    }

    fn said(held: &Held) -> Saying {
        let Ok(saying) = held.saying(Filling::Nothing);

        saying
    }

    fn does(slots: &[Slot]) -> Vec<Option<Does>> {
        slots.iter().map(|slot| slot.does).collect()
    }

    #[test]
    fn the_two_doors_are_the_first_things_a_thumb_reaches_on_the_left() {
        let saying = said(&held());

        assert_eq!(
            does(&saying.left),
            [
                Some(Does::Launcher),
                Some(Does::Keyboard),
                Some(Does::Workspace(1)),
                Some(Does::Workspace(2))
            ]
        );
    }

    #[test]
    fn the_readings_stand_in_one_order_and_the_bell_is_last() {
        let saying = said(&held());

        assert_eq!(
            does(&saying.right),
            [
                Some(Does::Music),
                Some(Does::Settings(What::Sound)),
                Some(Does::Settings(What::Bluetooth)),
                Some(Does::Settings(What::Network)),
                Some(Does::Settings(What::Battery)),
                Some(Does::Notices)
            ]
        );
    }

    #[test]
    fn the_workspace_you_are_on_is_the_lit_one_and_the_rest_are_quiet() {
        let saying = said(&held());
        let workspaces: Vec<Lit> = saying
            .left
            .iter()
            .filter(|slot| matches!(slot.does, Some(Does::Workspace(_))))
            .map(|slot| slot.lit)
            .collect();

        assert_eq!(workspaces, [Lit::No, Lit::Yes]);
    }

    #[test]
    fn a_door_says_whether_a_tap_will_open_or_close_what_it_opens() {
        let closed = said(&held());
        let open = said(&Held { open: Open { launcher: Up::OnScreen, ..shut() }, ..held() });
        let first = |saying: &Saying| saying.left.first().map(|slot| slot.lit);

        assert_eq!(first(&closed), Some(Lit::No));
        assert_eq!(first(&open), Some(Lit::Yes));
    }

    #[test]
    fn a_reading_is_lit_only_while_its_own_tab_is_the_one_in_front() {
        let lit = |settings, tab: Option<&str>| {
            let held = Held {
                open: Open { settings, tab: tab.map(str::to_string), ..shut() },
                ..held()
            };
            let saying = said(&held);

            saying
                .right
                .iter()
                .filter_map(|slot| match slot.does {
                    Some(Does::Settings(what)) => Some((what, slot.lit)),
                    Some(_) | None => None,
                })
                .collect::<Vec<_>>()
        };

        assert!(lit(Up::NotThere, Some("Sound")).iter().all(|(_, lit)| *lit == Lit::No));
        assert_eq!(
            lit(Up::OnScreen, Some("Sound"))
                .into_iter()
                .filter(|(_, lit)| *lit == Lit::Yes)
                .map(|(what, _)| what)
                .collect::<Vec<_>>(),
            [What::Sound]
        );
        assert!(lit(Up::OnScreen, None).iter().all(|(_, lit)| *lit == Lit::No));
    }

    #[test]
    fn a_reading_with_something_beside_it_makes_a_slot_that_says_two_things() {
        let charge = Says {
            icon: "\u{f0079}".to_string(),
            beside: Some("64%".to_string()),
            tone: Tone::Plain,
        };
        let held = Held {
            readings: vec![(What::Battery, charge)],
            ..held()
        };
        let saying = said(&held);
        let faces: Vec<Vec<Face>> = saying
            .right
            .iter()
            .filter(|slot| slot.does == Some(Does::Settings(What::Battery)))
            .map(|slot| slot.said.iter().map(|said| said.face).collect())
            .collect();

        assert_eq!(faces, [vec![Face::Icon, Face::Small]]);
    }

    #[test]
    fn a_reading_nothing_answered_for_is_left_off_rather_than_drawn_blank() {
        let held = Held { readings: Vec::new(), ..held() };
        let saying = said(&held);

        assert_eq!(does(&saying.right), [Some(Does::Music), Some(Does::Notices)]);
    }

    #[test]
    fn the_clock_is_the_only_thing_in_the_middle_and_it_opens_nothing() {
        let saying = said(&held());

        assert_eq!(does(&saying.middle), [None]);
    }

    #[test]
    fn nothing_playing_is_the_note_rather_than_no_note_at_all() {
        let Ok(nothing) = music(Paused::No, Playing::Nothing);
        let Ok(paused) = music(Paused::Yes, Playing::Something);
        let Ok(going) = music(Paused::No, Playing::Something);

        assert_eq!(nothing.icon, MUSIC);
        assert_eq!(nothing.tone, Tone::Quiet);
        assert_ne!(paused.icon, going.icon);
        assert_eq!(going.tone, Tone::Plain);
    }
}
