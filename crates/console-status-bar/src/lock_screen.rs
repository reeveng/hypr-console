//! The bar over a screen nobody is in yet: the way in, and the lock.
//!
//! **What stands on it is what needs no session.** Everything the desktop's
//! bar opens -- the launcher, the keyboard, a workspace, a tab of the settings
//! -- is a program in somebody's session, which is the one thing neither of
//! these screens has. So the left is empty, the clock keeps the middle, and the
//! right keeps what belongs to the machine rather than to anybody: the sound
//! and the battery where they stand on the desktop, and after them the way to
//! put the machine to sleep or stop it, which the desktop keeps in its settings
//! and which is most of what a person at a locked machine wants that is not to
//! come in.
//!
//! **A tap opens a row under the bar.** On the desktop the sound opens its tab.
//! Here there is nothing to open a tab in, so a slot that can be pressed hangs
//! a row from the right end of the bar instead: quieter, silent and louder for
//! the sound, Sleep, Restart and Shut Down for the power. One row is down at a
//! time, and the slot it hangs from is lit while it is, the way a slot on the
//! desktop is lit while its tab is in front.
//!
//! **The sound says its number.** The desktop's bar wears a glyph and no
//! percentage, because the rocker says the figure as it changes. Nothing comes
//! up over a lock to say it, so here the figure stands beside the glyph, the
//! way the battery's does.
//!
//! **The pad reaches it from the top of the ring.** Up from a dot with nothing
//! above it is the first slot that can be pressed. Left and right walk along,
//! A opens a row and steps into it or does what the slot in it says, and B steps
//! back out: from the row to the slot it hangs from, from the bar to the ring.
//! Down from the bar is the ring again. A row the pad opened is down while the
//! pad is in it; a row a finger opened goes at the next touch anywhere else,
//! which the screen says as [`LockScreenBarEvent::Dismissed`]. While the pad is
//! on the bar the slot under it is lit, and the screen drawing the ring leaves
//! its cursor off, because two cursors is a screen that does not say where a
//! press goes.
//!
//! Nothing here reads the machine or runs anything. What the machine said
//! arrives as a request, and what a press wants done leaves as an effect.

use std::collections::BTreeMap;
use std::process::Command;

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_internal_programs::InternalProgram;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_state_machine::{Machine, Queue};
use console_core_walking::where_it_is;
use console_core_words::Words;
use console_input_event_devices::presses::ButtonPress;

use crate::measuring::{every, sized};
use crate::reading::{Reading, SILENT, StatusItem, Tone};
use crate::showing::{BarAction, Face, Filling, Fitting, Layout, Lit, Rendered, Slot, Span, Wearing, along, beneath};

pub const POWER: &str = "\u{f0425}";

pub const QUIETER: &str = "\u{f075e}";

pub const LOUDER: &str = "\u{f075d}";

pub const READ: [StatusItem; 2] = [StatusItem::Sound, StatusItem::Battery];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Sound,
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Volume {
    #[words(word = "down")]
    Down,
    #[words(word = "mute")]
    Mute,
    #[words(word = "up")]
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Power {
    #[words(says = "Sleep", systemctl = "suspend")]
    Sleep,
    #[words(says = "Restart", systemctl = "reboot")]
    Restart,
    #[words(says = "Shut Down", systemctl = "poweroff")]
    ShutDown,
}

impl Volume {
    pub fn command(self) -> Result<Command, Never> {
        let Ok(mut command) = InternalProgram::Volume.command();
        let Ok(word) = self.word();

        command.arg(word);

        Ok(command)
    }
}

impl Power {
    pub fn command(self) -> Result<Command, Never> {
        let Ok(mut command) = Program::Systemctl.command();
        let Ok(word) = self.systemctl();

        command.arg(word);

        Ok(command)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LockScreenBar {
    pub readings: BTreeMap<StatusItem, Reading>,
    pub clock: String,
    pub menu: Option<Menu>,
    pub focus: Option<BarAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockScreenBarEvent {
    Read(StatusItem, Option<Reading>),
    Ticked(String),
    Tapped(BarAction),
    Pressed(ButtonPress),
    Entered,
    Dismissed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockScreenBarEffect {
    Volume(Volume),
    Power(Power),
    Read(StatusItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Within {
    Bar,
    Row(Menu),
    Nowhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Toward {
    Start,
    End,
}

type Effects = Queue<LockScreenBarEffect>;

fn choices(menu: Menu) -> Result<Vec<(BarAction, Span)>, Never> {
    let icon = |text: &str| Span { text: text.to_string(), face: Face::Icon };
    let words = |power: Power| {
        let Ok(says) = power.says();

        (BarAction::Power(power), Span { text: says.to_string(), face: Face::Reading })
    };

    Ok(match menu {
        Menu::Sound => vec![
            (BarAction::Volume(Volume::Down), icon(QUIETER)),
            (BarAction::Volume(Volume::Mute), icon(SILENT)),
            (BarAction::Volume(Volume::Up), icon(LOUDER)),
        ],
        Menu::Power => vec![words(Power::Sleep), words(Power::Restart), words(Power::ShutDown)],
    })
}

fn row_of(menu: Menu) -> Result<Vec<BarAction>, Never> {
    let Ok(choices) = choices(menu);

    Ok(choices.into_iter().map(|(action, _)| action).collect())
}

fn opens(item: StatusItem) -> Result<Option<Menu>, Never> {
    Ok(match item {
        StatusItem::Sound => Some(Menu::Sound),
        StatusItem::Battery | StatusItem::Bluetooth | StatusItem::Network => None,
    })
}

fn stepped(line: &[BarAction], from: BarAction, toward: Toward) -> Result<BarAction, Never> {
    let Ok(at) = where_it_is(line, &from);
    let next = at.and_then(|at| match toward {
        Toward::Start => at.checked_sub(1),
        Toward::End => at.checked_add(1),
    });
    let found = next.and_then(|next| {
        let Ok(next) = index(next);

        line.get(next).copied()
    });

    Ok(match found {
        Some(found) => found,
        None => from,
    })
}

impl LockScreenBar {
    fn pressable(&self) -> Result<Vec<BarAction>, Never> {
        let mut line = Vec::new();

        for item in READ {
            let Ok(menu) = opens(item);

            match (self.readings.contains_key(&item), menu) {
                (true, Some(menu)) => line.push(BarAction::Menu(menu)),
                (true, None) | (false, _) => {}
            }
        }

        line.push(BarAction::Menu(Menu::Power));

        Ok(line)
    }

    fn within(&self, focus: BarAction) -> Result<Within, Never> {
        let Ok(bar) = self.pressable();
        let Ok(row) = match self.menu {
            Some(menu) => row_of(menu),
            None => Ok(Vec::new()),
        };

        Ok(match (bar.contains(&focus), row.contains(&focus), self.menu) {
            (true, _, _) => Within::Bar,
            (false, true, Some(menu)) => Within::Row(menu),
            (false, true, None) | (false, false, _) => Within::Nowhere,
        })
    }

    fn line(&self, within: Within) -> Result<Vec<BarAction>, Never> {
        match within {
            Within::Bar => self.pressable(),
            Within::Row(menu) => row_of(menu),
            Within::Nowhere => Ok(Vec::new()),
        }
    }

    fn entered(self) -> Result<LockScreenBar, Never> {
        let Ok(line) = self.pressable();
        let focus = line.first().copied();

        Ok(LockScreenBar { focus, menu: None, ..self })
    }

    fn left(self) -> Result<LockScreenBar, Never> {
        Ok(LockScreenBar { focus: None, menu: None, ..self })
    }

    fn done(self, action: BarAction, effects: &mut Effects) -> Result<LockScreenBar, Never> {
        match action {
            BarAction::Volume(volume) => {
                let Ok(()) = effects.offer(LockScreenBarEffect::Volume(volume));
                let Ok(()) = effects.offer(LockScreenBarEffect::Read(StatusItem::Sound));

                Ok(self)
            }
            BarAction::Power(power) => {
                let Ok(()) = effects.offer(LockScreenBarEffect::Power(power));

                self.left()
            }
            BarAction::Menu(_)
            | BarAction::Calendar
            | BarAction::Launcher
            | BarAction::Keyboard
            | BarAction::Workspace(_)
            | BarAction::Settings(_)
            | BarAction::Music
            | BarAction::Notifications => Ok(self),
        }
    }

    fn tapped(self, action: BarAction, effects: &mut Effects) -> Result<LockScreenBar, Never> {
        match action {
            BarAction::Menu(menu) => {
                let menu = match self.menu == Some(menu) {
                    true => None,
                    false => Some(menu),
                };

                Ok(LockScreenBar { menu, focus: None, ..self })
            }
            BarAction::Volume(_)
            | BarAction::Power(_)
            | BarAction::Calendar
            | BarAction::Launcher
            | BarAction::Keyboard
            | BarAction::Workspace(_)
            | BarAction::Settings(_)
            | BarAction::Music
            | BarAction::Notifications => self.done(action, effects),
        }
    }

    fn chosen(self, focus: BarAction, effects: &mut Effects) -> Result<LockScreenBar, Never> {
        match focus {
            BarAction::Menu(menu) => {
                let Ok(row) = row_of(menu);
                let focus = row.first().copied();

                Ok(LockScreenBar { menu: Some(menu), focus, ..self })
            }
            BarAction::Volume(_)
            | BarAction::Power(_)
            | BarAction::Calendar
            | BarAction::Launcher
            | BarAction::Keyboard
            | BarAction::Workspace(_)
            | BarAction::Settings(_)
            | BarAction::Music
            | BarAction::Notifications => self.done(focus, effects),
        }
    }

    fn pressed(self, focus: BarAction, press: ButtonPress, effects: &mut Effects) -> Result<LockScreenBar, Never> {
        let Ok(within) = self.within(focus);
        let Ok(line) = self.line(within);
        let walked = |toward: Toward| {
            let Ok(focus) = stepped(&line, focus, toward);

            focus
        };

        match (within, press) {
            (Within::Nowhere, _) => self.entered(),
            (Within::Bar | Within::Row(_), ButtonPress::Left) => Ok(LockScreenBar { focus: Some(walked(Toward::Start)), ..self }),
            (Within::Bar | Within::Row(_), ButtonPress::Right) => Ok(LockScreenBar { focus: Some(walked(Toward::End)), ..self }),
            (Within::Bar, ButtonPress::Up) | (Within::Row(_), ButtonPress::Down) => Ok(self),
            (Within::Bar, ButtonPress::Down | ButtonPress::Back) => self.left(),
            (Within::Row(menu), ButtonPress::Up | ButtonPress::Back) => {
                Ok(LockScreenBar { focus: Some(BarAction::Menu(menu)), menu: None, ..self })
            }
            (Within::Bar | Within::Row(_), ButtonPress::Choose) => self.chosen(focus, effects),
        }
    }

    fn lit(&self, action: BarAction) -> Result<Lit, Never> {
        let focused = self.focus == Some(action);
        let hanging = self.menu.map(BarAction::Menu) == Some(action);

        Ok(match focused || hanging {
            true => Lit::Yes,
            false => Lit::No,
        })
    }

    pub fn layout(&self) -> Result<Layout, Never> {
        let middle = vec![Slot {
            spans: vec![Span { text: self.clock.clone(), face: Face::Clock }],
            tone: Tone::Plain,
            lit: Lit::No,
            action: None,
        }];
        let mut right = Vec::new();

        for item in READ {
            let Ok(menu) = opens(item);
            let action = menu.map(BarAction::Menu);
            let lit = match action {
                Some(action) => {
                    let Ok(lit) = self.lit(action);

                    lit
                }
                None => Lit::No,
            };

            match self.readings.get(&item) {
                Some(reading) => {
                    let mut spans = vec![Span { text: reading.icon.clone(), face: Face::Icon }];

                    spans.extend(reading.beside.iter().map(|beside| Span { text: beside.clone(), face: Face::Small }));
                    right.push(Slot { spans, tone: reading.tone, lit, action });
                }
                None => {}
            }
        }

        let power = BarAction::Menu(Menu::Power);
        let Ok(lit) = self.lit(power);

        right.push(Slot {
            spans: vec![Span { text: POWER.to_string(), face: Face::Icon }],
            tone: Tone::Pressed,
            lit,
            action: Some(power),
        });

        Ok(Layout { left: Vec::new(), middle, right, filling: Filling::None })
    }

    pub fn row(&self) -> Result<Vec<Slot>, Never> {
        let menu = match self.menu {
            Some(menu) => menu,
            None => return Ok(Vec::new()),
        };
        let Ok(choices) = choices(menu);

        Ok(choices
            .into_iter()
            .map(|(action, span)| {
                let Ok(lit) = self.lit(action);

                Slot { spans: vec![span], tone: Tone::Plain, lit, action: Some(action) }
            })
            .collect())
    }

    pub fn pictured(&self, wearing: &Wearing, room: Size<u32>) -> Result<Rendered, Never> {
        let Ok(fitting) = Fitting::of_em();
        let Ok(layout) = self.layout();
        let Ok(bar) = sized(&layout, fitting);
        let Ok(row) = self.row();
        let Ok(row) = every(&row, fitting);
        let Ok(Rendered { mut shapes, room: drawn, mut touching }) = along(&bar, wearing, room);
        let Ok(under) = beneath(&row, wearing, room);

        shapes.extend(under.shapes);
        touching.extend(under.touching);

        Ok(Rendered { shapes, room: Size { width: drawn.width, height: drawn.height.saturating_add(under.room.height) }, touching })
    }
}

impl Machine for LockScreenBar {
    type Input = ();
    type State = LockScreenBar;
    type Request = LockScreenBarEvent;
    type Effect = LockScreenBarEffect;

    fn initialize(_input: &(), _previous: Option<LockScreenBar>, _effects: &mut Effects) -> Result<LockScreenBar, Never> {
        Ok(LockScreenBar::default())
    }

    fn handle(bar: LockScreenBar, event: LockScreenBarEvent, effects: &mut Effects) -> Result<LockScreenBar, Never> {
        match event {
            LockScreenBarEvent::Read(item, Some(reading)) => {
                let mut readings = bar.readings;
                let _was = readings.insert(item, reading);

                Ok(LockScreenBar { readings, ..bar })
            }
            LockScreenBarEvent::Read(item, None) => {
                let mut readings = bar.readings;
                let _was = readings.remove(&item);

                Ok(LockScreenBar { readings, ..bar })
            }
            LockScreenBarEvent::Ticked(clock) => Ok(LockScreenBar { clock, ..bar }),
            LockScreenBarEvent::Tapped(action) => bar.tapped(action, effects),
            LockScreenBarEvent::Pressed(press) => match bar.focus {
                Some(focus) => bar.pressed(focus, press, effects),
                None => Ok(bar),
            },
            LockScreenBarEvent::Entered => bar.entered(),
            LockScreenBarEvent::Dismissed => bar.left(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_state_machine::{Transcript, run_from};

    fn says((icon, beside): (&str, &str)) -> Result<Reading, Never> {
        Ok(Reading { icon: icon.to_string(), beside: Some(beside.to_string()), tone: Tone::Plain })
    }

    fn standing() -> Result<LockScreenBar, Never> {
        let Ok(sound) = says(("\u{f0580}", "50%\u{2007}"));
        let Ok(battery) = says(("\u{f007e}", "64%\u{2007}"));

        Ok(LockScreenBar {
            readings: [(StatusItem::Sound, sound), (StatusItem::Battery, battery)].into_iter().collect(),
            clock: String::from("14:30"),
            menu: None,
            focus: None,
        })
    }

    fn after(events: &[LockScreenBarEvent]) -> Transcript<LockScreenBar> {
        let Ok(bar) = standing();

        run_from::<LockScreenBar>(bar, events)
    }

    fn pressed(presses: &[ButtonPress]) -> Result<Vec<LockScreenBarEvent>, Never> {
        Ok(std::iter::once(LockScreenBarEvent::Entered).chain(presses.iter().map(|press| LockScreenBarEvent::Pressed(*press))).collect())
    }

    fn does(slots: &[Slot]) -> Result<Vec<Option<BarAction>>, Never> {
        Ok(slots.iter().map(|slot| slot.action).collect())
    }

    fn lit(slots: &[Slot]) -> Result<Vec<Lit>, Never> {
        Ok(slots.iter().map(|slot| slot.lit).collect())
    }

    #[test]
    fn the_clock_keeps_the_middle_and_what_is_the_machine_s_keeps_the_right() {
        let Ok(bar) = standing();
        let Ok(layout) = bar.layout();
        let Ok(middle) = does(&layout.middle);
        let Ok(right) = does(&layout.right);
        let said: Vec<&str> = layout.right.iter().filter_map(|slot| slot.spans.get(1)).map(|span| span.text.as_str()).collect();

        assert!(layout.left.is_empty(), "the way in has nothing on the left to open");
        assert_eq!(middle, [None]);
        assert_eq!(right, [Some(BarAction::Menu(Menu::Sound)), None, Some(BarAction::Menu(Menu::Power))]);
        assert_eq!(said, ["50%\u{2007}", "64%\u{2007}"], "the sound and the battery do not say their numbers");
    }

    #[test]
    fn a_sound_nobody_answered_for_is_left_off_and_the_pad_starts_past_it() {
        let Ok(trace) = after(&[LockScreenBarEvent::Read(StatusItem::Sound, None), LockScreenBarEvent::Entered]);
        let Ok(layout) = trace.state.layout();
        let Ok(right) = does(&layout.right);

        assert_eq!(right, [None, Some(BarAction::Menu(Menu::Power))]);
        assert_eq!(trace.state.focus, Some(BarAction::Menu(Menu::Power)));
    }

    #[test]
    fn a_tap_hangs_a_row_from_the_slot_and_a_second_tap_takes_it_away() {
        let opener = BarAction::Menu(Menu::Power);
        let Ok(once) = after(&[LockScreenBarEvent::Tapped(opener)]);
        let Ok(twice) = after(&[LockScreenBarEvent::Tapped(opener), LockScreenBarEvent::Tapped(opener)]);
        let Ok(row) = once.state.row();
        let Ok(row) = does(&row);
        let Ok(layout) = once.state.layout();
        let Ok(right) = lit(&layout.right);
        let Ok(gone) = twice.state.row();

        assert_eq!(
            row,
            [Some(BarAction::Power(Power::Sleep)), Some(BarAction::Power(Power::Restart)), Some(BarAction::Power(Power::ShutDown))]
        );
        assert_eq!(right, [Lit::No, Lit::No, Lit::Yes], "the slot the row hangs from is not the lit one");
        assert!(gone.is_empty());
    }

    #[test]
    fn one_row_is_down_at_a_time() {
        let Ok(trace) = after(&[LockScreenBarEvent::Tapped(BarAction::Menu(Menu::Power)), LockScreenBarEvent::Tapped(BarAction::Menu(Menu::Sound))]);
        let Ok(row) = trace.state.row();
        let Ok(row) = does(&row);

        assert_eq!(row, [Some(BarAction::Volume(Volume::Down)), Some(BarAction::Volume(Volume::Mute)), Some(BarAction::Volume(Volume::Up))]);
    }

    #[test]
    fn sleep_asks_for_sleep_and_puts_the_row_away() {
        let Ok(trace) = after(&[LockScreenBarEvent::Tapped(BarAction::Menu(Menu::Power)), LockScreenBarEvent::Tapped(BarAction::Power(Power::Sleep))]);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, [LockScreenBarEffect::Power(Power::Sleep)]);
        assert_eq!(trace.state.menu, None);
    }

    #[test]
    fn a_step_of_the_volume_asks_for_the_sound_again_and_leaves_the_row_down() {
        let Ok(trace) = after(&[LockScreenBarEvent::Tapped(BarAction::Menu(Menu::Sound)), LockScreenBarEvent::Tapped(BarAction::Volume(Volume::Up))]);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, [LockScreenBarEffect::Volume(Volume::Up), LockScreenBarEffect::Read(StatusItem::Sound)]);
        assert_eq!(trace.state.menu, Some(Menu::Sound));
    }

    #[test]
    fn a_touch_anywhere_else_takes_a_row_away() {
        let Ok(trace) = after(&[LockScreenBarEvent::Tapped(BarAction::Menu(Menu::Sound)), LockScreenBarEvent::Dismissed]);

        assert_eq!(trace.state.menu, None);
        assert_eq!(trace.state.focus, None);
    }

    #[test]
    fn the_pad_walks_along_the_bar_and_stops_at_its_end() {
        let Ok(events) = pressed(&[ButtonPress::Right, ButtonPress::Right, ButtonPress::Right]);
        let Ok(entered) = after(&[LockScreenBarEvent::Entered]);
        let Ok(walked) = after(&events);
        let Ok(layout) = walked.state.layout();
        let Ok(right) = lit(&layout.right);

        assert_eq!(entered.state.focus, Some(BarAction::Menu(Menu::Sound)));
        assert_eq!(walked.state.focus, Some(BarAction::Menu(Menu::Power)));
        assert_eq!(right, [Lit::No, Lit::No, Lit::Yes], "the slot under the pad is not the lit one");
    }

    #[test]
    fn a_opens_the_row_and_steps_into_it_and_b_steps_back_out_one_level_at_a_time() {
        let Ok(into) = pressed(&[ButtonPress::Right, ButtonPress::Choose, ButtonPress::Right]);
        let Ok(back) = pressed(&[ButtonPress::Right, ButtonPress::Choose, ButtonPress::Right, ButtonPress::Back]);
        let Ok(out) = pressed(&[ButtonPress::Right, ButtonPress::Choose, ButtonPress::Right, ButtonPress::Back, ButtonPress::Back]);
        let Ok(into) = after(&into);
        let Ok(back) = after(&back);
        let Ok(out) = after(&out);

        assert_eq!((into.state.menu, into.state.focus), (Some(Menu::Power), Some(BarAction::Power(Power::Restart))));
        assert_eq!((back.state.menu, back.state.focus), (None, Some(BarAction::Menu(Menu::Power))));
        assert_eq!((out.state.menu, out.state.focus), (None, None));
    }

    #[test]
    fn a_on_a_choice_in_the_row_does_it() {
        let Ok(events) = pressed(&[ButtonPress::Choose, ButtonPress::Right, ButtonPress::Right, ButtonPress::Choose]);
        let Ok(trace) = after(&events);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, [LockScreenBarEffect::Volume(Volume::Up), LockScreenBarEffect::Read(StatusItem::Sound)]);
    }

    #[test]
    fn down_from_the_bar_is_the_ring_again_and_up_from_it_goes_nowhere() {
        let Ok(down) = pressed(&[ButtonPress::Down]);
        let Ok(up) = pressed(&[ButtonPress::Up]);
        let Ok(down) = after(&down);
        let Ok(up) = after(&up);

        assert_eq!(down.state.focus, None);
        assert_eq!(up.state.focus, Some(BarAction::Menu(Menu::Sound)));
    }

    #[test]
    fn a_press_with_the_pad_on_the_ring_is_not_the_bar_s() {
        let Ok(trace) = after(&[LockScreenBarEvent::Pressed(ButtonPress::Choose)]);
        let Ok(effects) = trace.effects();
        let Ok(untouched) = standing();

        assert_eq!(trace.state, untouched);
        assert!(effects.is_empty());
    }

    #[test]
    fn each_way_of_stopping_the_machine_is_the_systemctl_command_for_it() {
        let asked = |power: Power| {
            let Ok(command) = power.command();

            command.get_args().map(|word| word.to_string_lossy().into_owned()).collect::<Vec<String>>()
        };

        assert_eq!(asked(Power::Sleep), ["suspend"]);
        assert_eq!(asked(Power::Restart), ["reboot"]);
        assert_eq!(asked(Power::ShutDown), ["poweroff"]);
    }
}
