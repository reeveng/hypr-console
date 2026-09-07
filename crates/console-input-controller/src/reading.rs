//! What arrived, and what to do about it.
//!
//! Three devices are read: the pad InputPlumber publishes, the keyboard it
//! publishes beside it, and the controller's own touchpad. This is all of the
//! deciding, and none of the opening.

use evdev::{AbsoluteAxisCode, EventType, KeyCode};

use console_input_gamepad::jobs::Layer;
use console_core_never::Never;
use console_input_gamepad::routing::{self, Hat};
use console_input_gamepad::vocabulary::spoken_for;

use crate::buttons;
use crate::doing::Doing;
use crate::means::{Job, Press, Repeats, Table};
use crate::touch::Axis;
use crate::mode::{Acts, Mode};
use crate::scroll::{Wheel, pushed};
use crate::touch::Finger;

pub const CARRY_HELD: f64 = 0.5;

pub const STEP_AFTER: f64 = 0.400;

pub const STEP_FIRST: f64 = 0.180;
pub const STEP_FASTEST: f64 = 0.080;

const STEP_GATHER: f64 = 0.85;

pub const AT_ONCE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum From {
    Pad,
    Keys,
    Touch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ranges {
    pub stick: i32,
    pub trigger: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Held,
    Loose,
}

impl Default for Ranges {
    fn default() -> Self {
        Ranges { stick: 1, trigger: (0, 1) }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Stepping {
    button: &'static str,
    job: &'static Job,
    until: f64,
    gap: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controller {
    pub mode: Mode,
    pub layer: Layer,
    pub wheel: Wheel,
    pub finger: Finger,
    pub ranges: Ranges,
    pub table: Table,
    holding: Vec<(&'static str, &'static Job)>,
    stepping: Option<Stepping>,
    hat: (i32, i32),
    stick: (f64, f64),
}

impl Controller {
    pub fn reading(&mut self, ranges: Ranges) -> Result<(), Never> {
        self.ranges = ranges;

        Ok(())
    }

    pub fn now_in(&mut self, mode: Mode) -> Result<Vec<Doing>, Never> {
        let was = std::mem::replace(&mut self.mode, mode);
        let Ok(before) = was.acts();
        let Ok(now) = mode.acts();

        match (before, now) {
            (Acts::OnPresses, Acts::NotReading) => self.let_go(),
            (Acts::OnPresses, Acts::OnPresses)
            | (Acts::NotReading, Acts::NotReading)
            | (Acts::NotReading, Acts::OnPresses) => Ok(Vec::new()),
        }
    }

    pub fn pad_went(&mut self) -> Result<Vec<Doing>, Never> {
        self.stick = (0.0, 0.0);
        self.hat = (0, 0);
        self.layer = Layer::default();

        self.let_go()
    }

    pub fn saw(
        &mut self,
        from: From,
        kind: EventType,
        code: u16,
        value: i32,
        now: f64,
    ) -> Result<Vec<Doing>, Never> {
        match from {
            From::Pad => self.on_pad(kind, code, value),
            From::Keys => self.on_keys(kind, code, value),
            From::Touch => self.on_touch(kind, code, value, now),
        }
    }

    fn on_pad(&mut self, kind: EventType, code: u16, value: i32) -> Result<Vec<Doing>, Never> {
        match kind {
            EventType::ABSOLUTE => self.on_axis(code, value),
            EventType::KEY => {
                let Ok(pad) = routing::button_of_pad(code);

                match pad {
                    Some(button) => self.pressed(button, value),
                    None => self.on_trigger_button(code, value),
                }
            },
            _ => Ok(Vec::new()),
        }
    }

    fn on_trigger_button(&mut self, code: u16, value: i32) -> Result<Vec<Doing>, Never> {
        match (code == KeyCode::BTN_TL2.0, code == KeyCode::BTN_TR2.0) {
            (true, _) => self.layer.l2 = value == 1,
            (false, true) => self.layer.r2 = value == 1,
            (false, false) => {},
        }

        Ok(Vec::new())
    }

    fn on_axis(&mut self, code: u16, value: i32) -> Result<Vec<Doing>, Never> {
        match code == AbsoluteAxisCode::ABS_Z.0 || code == AbsoluteAxisCode::ABS_RZ.0 {
            true => {
                let Ok(pulled) = self.pulled(value);
                let held = pulled == Trigger::Held;

                match code == AbsoluteAxisCode::ABS_Z.0 {
                    true => self.layer.l2 = held,
                    false => self.layer.r2 = held,
                }

                return Ok(Vec::new());
            }
            false => {},
        }

        let Ok(hat) = routing::is_hat(code);

        match hat {
            Hat::Axis => return self.on_hat(code, value),
            Hat::NotAnAxis => {},
        }

        let Ok(pushed) = pushed(value, self.ranges.stick);

        match (code == AbsoluteAxisCode::ABS_RX.0, code == AbsoluteAxisCode::ABS_RY.0) {
            (true, _) => self.stick.0 = pushed,
            (false, true) => self.stick.1 = pushed,
            (false, false) => {},
        }

        Ok(Vec::new())
    }

    fn pulled(&self, value: i32) -> Result<Trigger, Never> {
        let (low, high) = self.ranges.trigger;
        let span = f64::from(high.saturating_sub(low).max(1));

        Ok(match f64::from(value.saturating_sub(low)) / span > CARRY_HELD {
            true => Trigger::Held,
            false => Trigger::Loose,
        })
    }

    fn on_hat(&mut self, code: u16, value: i32) -> Result<Vec<Doing>, Never> {
        let was = match code == AbsoluteAxisCode::ABS_HAT0X.0 {
            true => std::mem::replace(&mut self.hat.0, value),
            false => std::mem::replace(&mut self.hat.1, value),
        };

        match was == value {
            true => return Ok(Vec::new()),
            false => {},
        }

        let mut done = Vec::new();

        let Ok(left) = routing::button_of_hat(code, was);

        match left {
            Some(button) => {
                let Ok(pressed) = self.pressed(button, 0);

                done.extend(pressed);
            },
            None => {},
        }

        let Ok(reached) = routing::button_of_hat(code, value);

        match reached {
            Some(button) => {
                let Ok(pressed) = self.pressed(button, 1);

                done.extend(pressed);
            },
            None => {},
        }

        Ok(done)
    }

    fn on_keys(&mut self, kind: EventType, code: u16, value: i32) -> Result<Vec<Doing>, Never> {
        match kind == EventType::KEY {
            true => {},
            false => return Ok(Vec::new()),
        }

        let Ok(key) = routing::button_of_key(code);

        match key {
            Some(button) => self.pressed(button, value),
            None => Ok(Vec::new()),
        }
    }

    fn pressed(&mut self, button: &'static str, value: i32) -> Result<Vec<Doing>, Never> {
        let Ok(button) = spoken_for(button);

        match value {
            1 => {
                match self.holding.iter().any(|(down, _)| *down == button) {
                    true => return Ok(Vec::new()),
                    false => {},
                }

                let Ok(found) = buttons::job_for(&self.table, self.mode, button, self.layer);

                let Some(job) = found else {
                    return Ok(Vec::new());
                };

                match self.holding.len() < AT_ONCE {
                    true => {
                        self.holding.push((button, job));

                        let Ok(repeats) = job.what.repeats();

                        match repeats {
                            Repeats::WhileHeld => {
                                self.stepping = Some(Stepping {
                                    button,
                                    job,
                                    until: STEP_AFTER,
                                    gap: STEP_FIRST,
                                });
                            }
                            Repeats::Once => {},
                        }
                    }
                    false => {},
                }

                let Ok(acted) = buttons::acted(job, Press::Down);

                Ok(acted.into_iter().collect())
            }
            0 => {
                let Some(at) = self.holding.iter().position(|(down, _)| *down == button) else {
                    return Ok(Vec::new());
                };

                let (_, job) = self.holding.remove(at);

                match self.stepping.as_ref().is_some_and(|held| held.button == button) {
                    true => self.stepping = None,
                    false => {},
                }

                let Ok(acted) = buttons::acted(job, Press::Up);

                Ok(acted.into_iter().collect())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn let_go(&mut self) -> Result<Vec<Doing>, Never> {
        self.stepping = None;

        let held = std::mem::take(&mut self.holding);

        Ok(held
            .into_iter()
            .filter_map(|(_, job)| {
                let Ok(acted) = buttons::acted(job, Press::Up);

                acted
            })
            .collect())
    }

    fn on_touch(
        &mut self,
        kind: EventType,
        code: u16,
        value: i32,
        now: f64,
    ) -> Result<Vec<Doing>, Never> {
        match (kind, code) {
            (EventType::KEY, code) if code == KeyCode::BTN_TOUCH.0 => {
                let down = match value == 1 {
                    true => Press::Down,
                    false => Press::Up,
                };

                self.finger.touched(down, now)
            }
            (EventType::KEY, code) if code == KeyCode::BTN_0.0 => self.finger.pressed(value),
            (EventType::ABSOLUTE, code)
                if code == AbsoluteAxisCode::ABS_X.0 || code == AbsoluteAxisCode::ABS_Y.0 =>
            {
                let along = match code == AbsoluteAxisCode::ABS_X.0 {
                    true => Axis::Sideways,
                    false => Axis::Down,
                };

                let Ok(()) = self.finger.at(along, value);

                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    pub fn tick(&mut self, seconds: f64) -> Result<Vec<Doing>, Never> {
        let Ok(mut done) = self.stepped(seconds);
        let Ok(notches) = self.wheel.turned(self.stick.0, self.stick.1, seconds);

        match notches.is_empty() {
            true => {},
            false => done.push(Doing::Frame(notches)),
        }

        Ok(done)
    }

    fn stepped(&mut self, seconds: f64) -> Result<Vec<Doing>, Never> {
        let Some(held) = &mut self.stepping else { return Ok(Vec::new()) };

        held.until -= seconds;

        match held.until > 0.0 {
            true => return Ok(Vec::new()),
            false => {},
        }

        held.gap = (held.gap * STEP_GATHER).max(STEP_FASTEST);
        held.until = held.gap;

        let job = held.job;
        let Ok(acted) = buttons::acted(job, Press::Down);

        Ok(acted.into_iter().collect())
    }

    pub fn poll(&self) -> Result<f64, Never> {
        Ok(match self.finger.down {
            true => crate::touch::POLL,
            false => POLL,
        })
    }
}

pub const POLL: f64 = 0.02;

#[cfg(test)]
mod tests {
    use crate::doing::Carry;
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }
    use crate::doing::Out;
    use evdev::RelativeAxisCode;

    fn ranges() -> Ranges {
        Ranges { stick: 32767, trigger: (0, 1023) }
    }

    fn controller() -> Controller {
        let mut held = Controller::default();
        ok(held.reading(ranges()));
        held
    }

    fn pressed(held: &mut Controller, from: From, code: KeyCode) -> Vec<Doing> {
        let down = ok(held.saw(from, EventType::KEY, code.0, 1, 1000.0));
        ok(held.saw(from, EventType::KEY, code.0, 0, 1000.0));
        down
    }

    fn brighter(held: &mut Controller) -> Vec<Doing> {
        ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1, 1000.0));
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0X.0, 1, 1000.0))
    }

    fn ticked(held: &mut Controller, seconds: f64) -> usize {
        let mut steps = 0;
        let mut left = seconds;
        while left > 0.0 {
            steps += ok(held.tick(POLL)).len();
            left -= POLL;
        }
        steps
    }

    #[test]
    fn a_scale_held_down_goes_on_stepping() {
        let mut held = controller();
        let step = ok(Doing::run(&["/usr/local/bin/console-brightness", "up"]));
        assert_eq!(brighter(&mut held), std::slice::from_ref(&step), "the press itself");
        assert_eq!(ticked(&mut held, STEP_AFTER - 0.1), 0, "before the delay is up");
        assert!(ticked(&mut held, 0.4) > 0, "after it");
    }

    #[test]
    fn one_press_of_a_scale_is_one_step() {
        let mut held = controller();
        assert_eq!(brighter(&mut held).len(), 1);
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0X.0, 0, 1000.0));
        assert_eq!(ticked(&mut held, 3.0), 0, "a press that was let go went on stepping");
    }

    #[test]
    fn a_job_that_is_not_a_scale_does_not_repeat_when_it_is_held() {
        let mut held = controller();
        let down = ok(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 1, 1000.0));
        assert_eq!(down, [ok(Doing::run(&["launcher", "--keep"]))]);
        assert_eq!(ticked(&mut held, 3.0), 0, "the menu opened again on its own");
    }

    #[test]
    fn a_pad_that_went_away_stops_a_scale() {
        let mut held = controller();
        assert_eq!(brighter(&mut held).len(), 1);
        ok(held.pad_went());
        assert_eq!(ticked(&mut held, 3.0), 0);
    }

    #[test]
    fn a_held_scale_gathers_pace_and_then_holds_it() {
        let mut held = controller();
        brighter(&mut held);
        ticked(&mut held, STEP_AFTER);
        let first = ticked(&mut held, 1.0);
        let later = ticked(&mut held, 1.0);
        assert!(later > first, "it did not gather: {first} then {later}");
        let most = (1.0 / STEP_FASTEST).ceil() as usize;
        assert!(later <= most, "{later} steps in a second is past {most}");
        let settled = ticked(&mut held, 1.0);
        assert!(
            later.abs_diff(settled) <= 1,
            "it went on gathering past the floor: {later} then {settled}",
        );
    }

    #[test]
    fn a_button_that_starts_something_acts_when_it_goes_down_and_not_when_it_comes_up() {
        let mut held = controller();
        assert_eq!(
            ok(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 1, 1000.0)),
            [ok(Doing::run(&["launcher", "--keep"]))]
        );
        assert!(ok(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 0, 1000.0)).is_empty());
    }

    #[test]
    fn a_button_that_sends_a_key_sends_it_down_and_up() {
        let mut held = controller();
        let down = ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, 1000.0));
        assert_eq!(down, [Doing::Frame(vec![ok(Out::key(KeyCode::BTN_LEFT.0, 1))])]);
        let up = ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0, 1000.0));
        assert_eq!(up, [Doing::Frame(vec![ok(Out::key(KeyCode::BTN_LEFT.0, 0))])]);
    }

    #[test]
    fn the_dpad_arrives_as_a_hat_and_is_read_as_four_buttons() {
        let mut held = controller();
        let up = ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, -1, 1000.0));
        assert_eq!(up, [Doing::Frame(vec![ok(Out::key(KeyCode::KEY_UP.0, 1))])]);
        let over = ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 1, 1000.0));
        assert_eq!(
            over,
            [
                Doing::Frame(vec![ok(Out::key(KeyCode::KEY_UP.0, 0))]),
                Doing::Frame(vec![ok(Out::key(KeyCode::KEY_DOWN.0, 1))]),
            ]
        );
        let middle = ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 0, 1000.0));
        assert_eq!(middle, [Doing::Frame(vec![ok(Out::key(KeyCode::KEY_DOWN.0, 0))])]);
    }

    #[test]
    fn standing_down_lets_go_of_a_button_whose_release_will_never_arrive() {
        let mut held = controller();
        let down = ok(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F22.0, 1, 1000.0));

        assert!(!down.is_empty(), "X does nothing at all, so this test is asking nothing");

        let _ = ok(held.now_in(Mode::Keyboard));
        let again = ok(held.now_in(Mode::Desktop));

        assert!(again.is_empty(), "coming back is not a release of its own");
        assert_eq!(
            ok(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F22.0, 1, 2000.0)),
            down,
            "X was still held from last time, so the press that puts the keyboard back does nothing"
        );
    }

    #[test]
    fn a_button_held_across_a_menu_opening_stays_held() {
        let mut held = controller();
        ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, 1000.0));

        assert!(ok(held.now_in(Mode::Tabs)).is_empty(), "a chooser opening let go of A");
    }

    #[test]
    fn a_button_is_let_go_of_by_the_job_that_took_it() {
        let mut held = controller();
        ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, 1000.0));
        let _ = ok(held.now_in(Mode::Tabs));
        let up = ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0, 1000.0));
        assert_eq!(up, [Doing::Frame(vec![ok(Out::key(KeyCode::BTN_LEFT.0, 0))])], "not Enter");
    }

    #[test]
    fn a_pad_that_went_away_lets_go_of_what_was_held() {
        let mut held = controller();
        ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, 1000.0));
        assert_eq!(ok(held.pad_went()), [Doing::Frame(vec![ok(Out::key(KeyCode::BTN_LEFT.0, 0))])]);
        assert!(ok(held.pad_went()).is_empty(), "and only the once");
    }

    #[test]
    fn the_shoulders_carry_the_window_while_l2_is_held() {
        let mut held = controller();
        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), [ok(Doing::workspace("+1", Carry::Nothing))]);
        ok(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1, 1000.0));
        assert!(held.layer.l2);
        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), [ok(Doing::workspace("+1", Carry::Window))]);
    }

    #[test]
    fn pulling_l2_past_halfway_is_holding_it() {
        let mut held = controller();
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 400, 1000.0));
        assert!(!held.layer.l2, "not far enough");
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 900, 1000.0));
        assert!(held.layer.l2);
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RZ.0, 900, 1000.0));
        assert!(held.layer.r2);
    }

    #[test]
    fn the_right_stick_turns_the_wheel_and_the_left_one_does_not() {
        let mut held = controller();
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Y.0, -32767, 1000.0));
        assert!(ok(held.tick(1.0)).is_empty(), "the left stick is not a wheel");
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, 1000.0));
        let turned = ok(held.tick(1.0));

        let Ok(wanted) = console_core_number_conversion::toward_zero_usize(crate::scroll::MAX_HZ);

        assert!(matches!(turned.as_slice(), [Doing::Frame(notches)] if notches.len() == wanted));
    }

    #[test]
    fn a_pad_that_went_away_stops_the_wheel() {
        let mut held = controller();
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, 1000.0));
        ok(held.pad_went());
        assert!(ok(held.tick(1.0)).is_empty());
    }

    #[test]
    fn a_tap_on_the_touchpad_is_a_click() {
        let mut held = controller();
        ok(held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, 1000.0));
        let clicked = ok(held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 0, 1000.05));
        assert_eq!(clicked.len(), 2, "down and up");
    }

    #[test]
    fn a_finger_on_the_pad_is_read_at_the_pads_own_pace() {
        let mut held = controller();
        assert_eq!(ok(held.poll()), POLL);
        ok(held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, 1000.0));
        assert_eq!(ok(held.poll()), crate::touch::POLL);
    }

    #[test]
    fn what_comes_out_is_movement_on_one_device() {
        let mut held = controller();
        ok(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 32767, 1000.0));
        let turned = ok(held.tick(1.0));
        let Some(Doing::Frame(notches)) = turned.first() else {
            panic!("a frame of notches");
        };
        assert!(notches.contains(&ok(Out::rel(RelativeAxisCode::REL_HWHEEL.0, 1))));
    }
}
