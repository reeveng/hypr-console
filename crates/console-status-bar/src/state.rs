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
//! A workspace is shown by where it stands and not by the compositor's number
//! for it. The compositor never reuses a gap on its own -- close the window on
//! 4 of five and what is left is 1, 2, 3 and 5, and a + tapped on an empty
//! desktop is a 2 with nothing before it -- so its numbers say what happened
//! rather than what there is. What a person counts is the row, and the row
//! always starts at one.
//! Asking the machine is the program's, because it is the half that needs a
//! machine.

use console_compositor::Workspace;
use console_core_never::Never;
use console_onscreen::Up;

use crate::reading::{Reading, Tone, StatusItem};
use crate::showing::{BarAction, Face, Filling, Lit, Span, Layout, Slot};

pub const LAUNCHER: &str = "\u{f003b}";

pub const KEYBOARD: &str = "\u{f030c}";

pub const MUSIC: &str = "\u{f075a}";

pub const PAUSED: &str = "\u{f03e4}";

pub const ANOTHER: &str = "+";

pub const ALONG: [StatusItem; 4] = [StatusItem::Sound, StatusItem::Bluetooth, StatusItem::Network, StatusItem::Battery];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    pub launcher: Up,
    pub keyboard: Up,
    pub music: Up,
    pub notifications: Up,
    pub calendar: Up,
    pub settings: Up,
    pub tab: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarState {
    pub readings: Vec<(StatusItem, Reading)>,
    pub bell: Reading,
    pub music: Reading,
    pub clock: String,
    pub workspaces: Vec<Workspace>,
    pub front: Option<i64>,
    pub open: Open,
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

fn lit(up: Up) -> Result<Lit, Never> {
    Ok(match up {
        Up::OnScreen => Lit::Yes,
        Up::NotThere => Lit::No,
    })
}

fn icon(icon: &str, tone: Tone, lit: Lit, action: BarAction) -> Result<Slot, Never> {
    Ok(Slot {
        spans: vec![Span { text: icon.to_string(), face: Face::Icon }],
        tone,
        lit,
        action: Some(action),
    })
}

fn reading(reading: &Reading, lit: Lit, action: BarAction) -> Result<Slot, Never> {
    let mut spans = vec![Span { text: reading.icon.clone(), face: Face::Icon }];

    match &reading.beside {
        Some(beside) => spans.push(Span { text: beside.clone(), face: Face::Small }),
        None => {},
    }

    Ok(Slot { spans, tone: reading.tone, lit, action: Some(action) })
}

impl Open {
    fn on(&self, item: StatusItem) -> Result<Lit, Never> {
        let Ok(mine) = item.tab();

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Promise {
    Fulfilled,
    Waiting,
}

impl Open {
    pub fn promised(&self, action: BarAction, pressed: &Open) -> Result<(Open, Promise), Never> {
        let mut held = self.clone();

        let agrees = match action {
            BarAction::Launcher => self.launcher == pressed.launcher,
            BarAction::Keyboard => self.keyboard == pressed.keyboard,
            BarAction::Music => self.music == pressed.music,
            BarAction::Notifications => self.notifications == pressed.notifications,
            BarAction::Calendar => self.calendar == pressed.calendar,
            BarAction::Settings(_) => self.settings == pressed.settings && self.tab == pressed.tab,
            BarAction::Workspace(_) => true,
        };

        match agrees {
            true => return Ok((held, Promise::Fulfilled)),
            false => {},
        }

        match action {
            BarAction::Launcher => held.launcher = pressed.launcher,
            BarAction::Keyboard => held.keyboard = pressed.keyboard,
            BarAction::Music => held.music = pressed.music,
            BarAction::Notifications => held.notifications = pressed.notifications,
            BarAction::Calendar => held.calendar = pressed.calendar,
            BarAction::Settings(_) => {
                held.settings = pressed.settings;
                held.tab = pressed.tab.clone();
            }
            BarAction::Workspace(_) => {},
        }

        Ok((held, Promise::Waiting))
    }
}

fn flipped(up: &mut Up) -> Result<(), Never> {
    *up = match up {
        Up::OnScreen => Up::NotThere,
        Up::NotThere => Up::OnScreen,
    };

    Ok(())
}

impl BarState {
    pub fn pressed(&mut self, action: BarAction) -> Result<(), Never> {
        match action {
            BarAction::Workspace(id) => self.front = Some(id),
            BarAction::Launcher => {
                let Ok(()) = flipped(&mut self.open.launcher);
            }
            BarAction::Keyboard => {
                let Ok(()) = flipped(&mut self.open.keyboard);
            }
            BarAction::Music => {
                let Ok(()) = flipped(&mut self.open.music);
            }
            BarAction::Notifications => {
                let Ok(()) = flipped(&mut self.open.notifications);
            }
            BarAction::Calendar => {
                let Ok(()) = flipped(&mut self.open.calendar);
            }
            BarAction::Settings(item) => {
                let Ok(mine) = item.tab();

                self.open.settings = Up::OnScreen;
                self.open.tab = Some(mine.to_string());
            }
        }

        Ok(())
    }

    pub fn next(&self) -> Result<i64, Never> {
        let last = self.workspaces.iter().map(|workspace| workspace.id).filter(|id| *id > 0).max();

        Ok(match last {
            Some(last) => last.saturating_add(1),
            None => 1,
        })
    }

    pub fn layout(&self, filling: Filling) -> Result<Layout, Never> {
        let Ok(launcher) = lit(self.open.launcher);
        let Ok(keyboard) = lit(self.open.keyboard);
        let Ok(music) = lit(self.open.music);
        let Ok(notifications) = lit(self.open.notifications);

        let Ok(door) = icon(LAUNCHER, Tone::Pressed, launcher, BarAction::Launcher);
        let Ok(board) = icon(KEYBOARD, Tone::Pressed, keyboard, BarAction::Keyboard);
        let mut left = vec![door, board];
        let mut counted: u32 = 0;

        for workspace in &self.workspaces {
            let named = match workspace.id > 0 {
                true => {
                    counted = counted.saturating_add(1);

                    counted.to_string()
                }
                false => workspace.named.clone(),
            };

            let lit = match self.front == Some(workspace.id) {
                true => Lit::Yes,
                false => Lit::No,
            };

            left.push(Slot {
                spans: vec![Span { text: named, face: Face::Reading }],
                tone: Tone::Secondary,
                lit,
                action: Some(BarAction::Workspace(workspace.id)),
            });
        }

        let Ok(next) = self.next();

        left.push(Slot {
            spans: vec![Span { text: ANOTHER.to_string(), face: Face::Reading }],
            tone: Tone::Secondary,
            lit: Lit::No,
            action: Some(BarAction::Workspace(next)),
        });

        let Ok(calendar) = lit(self.open.calendar);

        let middle = vec![Slot {
            spans: vec![Span { text: self.clock.clone(), face: Face::Clock }],
            tone: Tone::Plain,
            lit: calendar,
            action: Some(BarAction::Calendar),
        }];

        let Ok(song) = reading(&self.music, music, BarAction::Music);
        let mut right = vec![song];

        for item in ALONG {
            let found = self.readings.iter().find(|(named, _)| *named == item);

            match found {
                Some((_named, found)) => {
                    let Ok(lit) = self.open.on(item);
                    let Ok(slot) = reading(found, lit, BarAction::Settings(item));

                    right.push(slot);
                }
                None => {},
            }
        }

        let Ok(bell) = reading(&self.bell, notifications, BarAction::Notifications);

        right.push(bell);

        Ok(Layout { left, middle, right, filling })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shut() -> Result<Open, Never> {
        Ok(Open {
            launcher: Up::NotThere,
            keyboard: Up::NotThere,
            music: Up::NotThere,
            notifications: Up::NotThere,
            calendar: Up::NotThere,
            settings: Up::NotThere,
            tab: None,
        })
    }

    fn says(icon: &str, tone: Tone) -> Result<Reading, Never> {
        Ok(Reading { icon: String::from(icon), beside: None, tone })
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
        let Ok(open) = shut();

        Ok(BarState {
            readings,
            bell,
            music,
            clock: String::from("14:30"),
            workspaces: vec![
                Workspace { id: 1, named: String::from("1"), windows: Some(0) },
                Workspace { id: 2, named: String::from("2"), windows: Some(0) },
            ],
            front: Some(2),
            open,
        })
    }

    fn does(slots: &[Slot]) -> Result<Vec<Option<BarAction>>, Never> {
        Ok(slots.iter().map(|slot| slot.action).collect())
    }

    #[test]
    fn the_two_doors_are_the_first_things_a_thumb_reaches_on_the_left() {
        let Ok(state) = sample_state();
        let Ok(layout) = state.layout(Filling::None);
        let Ok(left) = does(&layout.left);

        assert_eq!(
            left,
            [
                Some(BarAction::Launcher),
                Some(BarAction::Keyboard),
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
                    .map(|id| Workspace { id: *id, named: id.to_string(), windows: Some(0) })
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
    fn the_workspaces_are_counted_from_one_whatever_the_compositor_numbered_them() {
        let Ok(state) = sample_state();
        let held = BarState {
            workspaces: [-98, 2, 3, 5]
                .iter()
                .map(|id| Workspace { id: *id, named: format!("was {id}"), windows: Some(1) })
                .collect(),
            ..state
        };
        let Ok(layout) = held.layout(Filling::None);
        let named: Vec<&str> = layout
            .left
            .iter()
            .filter(|slot| matches!(slot.action, Some(BarAction::Workspace(_))))
            .filter_map(|slot| slot.spans.first())
            .map(|span| span.text.as_str())
            .collect();

        assert_eq!(named, ["was -98", "1", "2", "3", ANOTHER]);
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
                Some(BarAction::Settings(StatusItem::Sound)),
                Some(BarAction::Settings(StatusItem::Bluetooth)),
                Some(BarAction::Settings(StatusItem::Network)),
                Some(BarAction::Settings(StatusItem::Battery)),
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
        let Ok(()) = state.pressed(BarAction::Workspace(3));
        let Ok(()) = state.pressed(BarAction::Calendar);
        let Ok(()) = state.pressed(BarAction::Settings(StatusItem::Battery));

        assert_eq!(state.front, Some(3));
        assert_eq!(state.open.calendar, Up::OnScreen);
        assert_eq!(state.open.settings, Up::OnScreen);

        let Ok(()) = state.pressed(BarAction::Calendar);

        assert_eq!(state.open.calendar, Up::NotThere, "a second press puts it away");
    }

    #[test]
    fn a_press_stays_lit_through_the_gap_between_one_panel_going_and_the_next_coming() {
        let Ok(shut) = shut();
        let settings_up = Open { settings: Up::OnScreen, tab: Some(String::from("sound")), ..shut.clone() };
        let pressed = Open { calendar: Up::OnScreen, ..settings_up.clone() };

        let Ok((gap, waiting)) = shut.promised(BarAction::Calendar, &pressed);

        assert_eq!(gap.calendar, Up::OnScreen, "nothing is up yet and the calendar is still coming");
        assert_eq!(gap.settings, Up::NotThere, "what went is let go at once");
        assert_eq!(waiting, Promise::Waiting);

        let arrived = Open { calendar: Up::OnScreen, ..shut };
        let Ok((after, kept)) = arrived.promised(BarAction::Calendar, &pressed);

        assert_eq!(after, arrived);
        assert_eq!(kept, Promise::Fulfilled);
    }

    #[test]
    fn a_tab_pressed_stays_in_front_while_the_old_one_is_still_on_the_screen() {
        let Ok(shut) = shut();
        let pressed = Open { settings: Up::OnScreen, tab: Some(String::from("wifi")), ..shut.clone() };
        let before = Open { settings: Up::OnScreen, tab: Some(String::from("sound")), ..shut };

        let Ok((held, waiting)) = before.promised(BarAction::Settings(StatusItem::Network), &pressed);

        assert_eq!(held.tab.as_deref(), Some("wifi"));
        assert_eq!(waiting, Promise::Waiting);
    }

    #[test]
    fn a_door_says_whether_a_tap_will_open_or_close_what_it_opens() {
        let Ok(state) = sample_state();
        let Ok(shut) = shut();
        let Ok(closed) = state.layout(Filling::None);
        let launcher_up = BarState { open: Open { launcher: Up::OnScreen, ..shut }, ..state };
        let Ok(open) = launcher_up.layout(Filling::None);
        let first = |layout: &Layout| layout.left.first().map(|slot| slot.lit);

        assert_eq!(first(&closed), Some(Lit::No));
        assert_eq!(first(&open), Some(Lit::Yes));
    }

    #[test]
    fn a_reading_is_lit_only_while_its_own_tab_is_the_one_in_front() {
        let lit = |settings, tab: Option<&str>| {
            let Ok(state) = sample_state();
            let Ok(shut) = shut();
            let held = BarState {
                open: Open { settings, tab: tab.map(String::from), ..shut },
                ..state
            };
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

        assert!(lit(Up::NotThere, Some("Sound")).iter().all(|(_, lit)| *lit == Lit::No));
        assert_eq!(
            lit(Up::OnScreen, Some("Sound"))
                .into_iter()
                .filter(|(_, lit)| *lit == Lit::Yes)
                .map(|(item, _)| item)
                .collect::<Vec<_>>(),
            [StatusItem::Sound]
        );
        assert!(lit(Up::OnScreen, None).iter().all(|(_, lit)| *lit == Lit::No));
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
            .filter(|slot| slot.action == Some(BarAction::Settings(StatusItem::Battery)))
            .map(|slot| slot.spans.iter().map(|span| span.face).collect())
            .collect();

        assert_eq!(faces, [vec![Face::Icon, Face::Small]]);
    }

    #[test]
    fn a_reading_nothing_answered_for_is_left_off_rather_than_drawn_blank() {
        let Ok(state) = sample_state();
        let held = BarState { readings: Vec::new(), ..state };
        let Ok(layout) = held.layout(Filling::None);
        let Ok(right) = does(&layout.right);

        assert_eq!(right, [Some(BarAction::Music), Some(BarAction::Notifications)]);
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
        let Ok(shut) = shut();
        let Ok(closed) = state.layout(Filling::None);
        let up = BarState { open: Open { calendar: Up::OnScreen, ..shut }, ..state };
        let Ok(open) = up.layout(Filling::None);
        let lit = |layout: &Layout| layout.middle.iter().map(|slot| slot.lit).collect::<Vec<_>>();

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
