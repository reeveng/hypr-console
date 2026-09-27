//! What arrived, and what to do about it.
//!
//! Three devices are read: the pad InputPlumber publishes, the keyboard it
//! publishes beside it, and the controller's own touchpad. This is all of the
//! deciding, and none of the opening.

use console_input_event_devices::{AbsoluteAxisCode, EventType, KeyCode};

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_words::Words;
use console_input_gamepad::axis::Range;
use console_input_gamepad::routing::{self, Hat};
use console_input_gamepad::vocabulary::{spoken_for, trigger_of_axis, trigger_of_button, trigger_spoken};

use console_input_bindings::bound::Input;

use crate::buttons;
use crate::effect::Effect;
use crate::actions::{Task, ButtonPress, RepeatMode, Table};
use crate::touch::Axis;
use crate::mode::{InputHandling, Mode};
use crate::scroll::{Stick, Wheel, pushed};
use crate::touch::Touch;

pub const CARRY_HELD: f64 = 0.5;

pub const STEP_AFTER: f64 = 0.400;

pub const STEP_FIRST: f64 = 0.180;
pub const STEP_FASTEST: f64 = 0.080;

const STEP_GATHER: f64 = 0.85;

pub const AT_ONCE: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Words)]
pub enum From {
    #[words(said = "pad", called = "CONSOLE_PAD")]
    Pad,
    #[words(said = "keyboard", called = "CONSOLE_KEYS")]
    Keys,
    #[words(said = "touchpad", called = "CONSOLE_TOUCHPAD")]
    Touch,
    #[words(said = "keyboard someone plugged in", called = "CONSOLE_TYPING")]
    Typing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wants {
    One,
    Every,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Meant {
    Some,
    None,
}

impl From {
    #[cfg_attr(
        dylint_lib = "explicit026_env_read_once",
        allow(
            explicit026_env_read_once,
            reason = "a variable names the device to read instead of finding one, and which variable is on the variant. Two binaries read these before this existed and the second one only knew about the pad"
        )
    )]
    pub fn path_from_environment(self) -> Result<Option<String>, Never> {
        let Ok(called) = self.called();

        Ok(match std::env::var(called) {
            Ok(said) => match said.is_empty() {
                true => None,
                false => Some(said),
            },
            Err(std::env::VarError::NotPresent) => None,
            Err(fault) => {
                eprintln!("{called}: {fault}; finding that device instead");

                None
            }
        })
    }

    pub fn wants(self) -> Result<Wants, Never> {
        Ok(match self {
            From::Pad | From::Keys | From::Touch => Wants::One,
            From::Typing => Wants::Every,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ranges {
    pub stick: Range,
    pub trigger: (i32, i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Pressed,
    Loose,
}

pub fn pulled(value: i32, (low, high): (i32, i32)) -> Result<Trigger, Never> {
    let span = f64::from(high.saturating_sub(low).max(1));

    Ok(match f64::from(value.saturating_sub(low)) / span > CARRY_HELD {
        true => Trigger::Pressed,
        false => Trigger::Loose,
    })
}

fn pulled_by(value: i32) -> Result<Trigger, Never> {
    Ok(match value == 1 {
        true => Trigger::Pressed,
        false => Trigger::Loose,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulled {
    pub l2: Trigger,
    pub r2: Trigger,
}

impl Default for Pulled {
    fn default() -> Self {
        Pulled { l2: Trigger::Loose, r2: Trigger::Loose }
    }
}

impl Pulled {
    pub fn held_names(self) -> Result<Vec<&'static str>, Never> {
        let mut said = Vec::new();

        match self.l2 {
            Trigger::Pressed => said.push("l2"),
            Trigger::Loose => {},
        }

        match self.r2 {
            Trigger::Pressed => said.push("r2"),
            Trigger::Loose => {},
        }

        Ok(said)
    }

    fn hold(&mut self, named: Option<&str>, pulled: Trigger) -> Result<(), Never> {
        let Ok(spoken) = named.map_or(Ok(None), trigger_spoken);

        match spoken {
            Some("l2") => self.l2 = pulled,
            Some("r2") => self.r2 = pulled,
            Some(_) | None => {},
        }

        Ok(())
    }
}

impl Default for Ranges {
    fn default() -> Self {
        Ranges { stick: Range { low: -1, high: 1 }, trigger: (0, 1) }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Stepping {
    button: &'static str,
    job: &'static Task,
    until: f64,
    gap: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controller {
    pub mode: Mode,
    pub pulled: Pulled,
    pub wheel: Wheel,
    pub finger: Touch,
    pub ranges: Ranges,
    pub table: Table,
    using: Option<Input>,
    holding: Vec<(&'static str, &'static Task)>,
    pressed_buttons: Vec<&'static str>,
    stepping: Option<Stepping>,
    hat: (i32, i32),
    stick: (f64, f64),
}

impl Controller {
    pub fn reading(&mut self, ranges: Ranges) -> Result<(), Never> {
        self.ranges = ranges;

        Ok(())
    }

    pub fn now_in(&mut self, mode: Mode) -> Result<Vec<Effect>, Never> {
        let was = std::mem::replace(&mut self.mode, mode);
        let Ok(before) = was.input_handling();
        let Ok(now) = mode.input_handling();

        match (before, now) {
            (InputHandling::Enabled, InputHandling::Disabled) => self.let_go(),
            (InputHandling::Enabled, InputHandling::Enabled)
            | (InputHandling::Disabled, InputHandling::Disabled)
            | (InputHandling::Disabled, InputHandling::Enabled) => Ok(Vec::new()),
        }
    }

    pub fn pad_went(&mut self) -> Result<Vec<Effect>, Never> {
        self.stick = (0.0, 0.0);
        self.hat = (0, 0);
        self.pulled = Pulled::default();
        self.pressed_buttons.clear();

        self.let_go()
    }

    pub fn saw(
        &mut self,
        from: From,
        kind: EventType,
        code: u16,
        value: i32,
        now: f64,
    ) -> Result<Vec<Effect>, Never> {
        match from {
            From::Pad => self.on_pad(kind, code, value),
            From::Keys => self.on_keys(kind, code, value),
            From::Touch => self.on_touch(kind, code, value, now),
            From::Typing => self.on_typing(kind, value),
        }
    }

    pub fn using(&mut self, on: Input) -> Result<(), Never> {
        self.using = Some(on);

        Ok(())
    }

    fn on_typing(&mut self, kind: EventType, value: i32) -> Result<Vec<Effect>, Never> {
        match (kind, value) {
            (EventType::KEY, 1) => self.now_using(Input::Keyboard),
            (_, _) => Ok(Vec::new()),
        }
    }

    fn now_using(&mut self, on: Input) -> Result<Vec<Effect>, Never> {
        match self.using {
            Some(was) => match was == on {
                true => return Ok(Vec::new()),
                false => {},
            },
            None => {},
        }

        self.using = Some(on);

        Ok(vec![Effect::Using(on)])
    }

    fn on_pad(&mut self, kind: EventType, code: u16, value: i32) -> Result<Vec<Effect>, Never> {
        let Ok(meant) = self.classify(kind, code, value);

        let Ok(mut done) = match meant {
            Meant::Some => self.now_using(Input::Pad),
            Meant::None => Ok(Vec::new()),
        };

        let Ok(rest) = match kind {
            EventType::ABSOLUTE => self.on_axis(code, value),
            EventType::KEY => {
                let Ok(pad) = routing::button_of_pad(code);

                match pad {
                    Some(button) => self.pressed(button, value),
                    None => self.on_trigger_button(code, value),
                }
            },
            _ => Ok(Vec::new()),
        };

        done.extend(rest);

        Ok(done)
    }

    fn classify(&self, kind: EventType, code: u16, value: i32) -> Result<Meant, Never> {
        Ok(match kind {
            EventType::KEY => match value {
                1 => Meant::Some,
                _ => Meant::None,
            },
            EventType::ABSOLUTE => {
                let Ok(moved) = self.moved(code, value);

                moved
            }
            _ => Meant::None,
        })
    }

    fn moved(&self, code: u16, value: i32) -> Result<Meant, Never> {
        let Ok(hat) = routing::is_hat(code);

        match hat {
            Hat::Axis => {
                return Ok(match value {
                    0 => Meant::None,
                    _ => Meant::Some,
                });
            }
            Hat::NotAnAxis => {},
        }

        let Ok(trigger) = trigger_of_axis(code);

        match trigger {
            Some(_) => {
                let Ok(pulled) = pulled(value, self.ranges.trigger);

                return Ok(match pulled {
                    Trigger::Pressed => Meant::Some,
                    Trigger::Loose => Meant::None,
                });
            }
            None => {},
        }

        let Ok(pushed) = pushed(Stick { value, range: self.ranges.stick });

        Ok(match pushed.abs() > 0.0 {
            true => Meant::Some,
            false => Meant::None,
        })
    }

    fn on_trigger_button(&mut self, code: u16, value: i32) -> Result<Vec<Effect>, Never> {
        let Ok(pulled) = pulled_by(value);

        let Ok(named) = trigger_of_button(code);
        let Ok(()) = self.pulled.hold(named, pulled);

        Ok(Vec::new())
    }

    fn on_axis(&mut self, code: u16, value: i32) -> Result<Vec<Effect>, Never> {
        let Ok(trigger) = trigger_of_axis(code);

        match trigger {
            Some(named) => {
                let Ok(pulled) = pulled(value, self.ranges.trigger);
                let Ok(()) = self.pulled.hold(Some(named), pulled);

                return Ok(Vec::new());
            }
            None => {},
        }

        let Ok(hat) = routing::is_hat(code);

        match hat {
            Hat::Axis => return self.on_hat(code, value),
            Hat::NotAnAxis => {},
        }

        let Ok(pushed) = pushed(Stick { value, range: self.ranges.stick });

        match (code == AbsoluteAxisCode::ABS_RX.0, code == AbsoluteAxisCode::ABS_RY.0) {
            (true, _) => self.stick.0 = pushed,
            (false, true) => self.stick.1 = pushed,
            (false, false) => {},
        }

        Ok(Vec::new())
    }

    fn on_hat(&mut self, code: u16, value: i32) -> Result<Vec<Effect>, Never> {
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

    fn on_keys(&mut self, kind: EventType, code: u16, value: i32) -> Result<Vec<Effect>, Never> {
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

    fn pressed(&mut self, button: &'static str, value: i32) -> Result<Vec<Effect>, Never> {
        let Ok(button) = spoken_for(button);

        match value {
            1 => {
                match self.holding.iter().any(|(down, _)| *down == button) {
                    true => return Ok(Vec::new()),
                    false => {},
                }

                let Ok(held) = self.held_buttons();
                let Ok(found) = buttons::job_for(&self.table, self.mode, &held, button);

                self.pressed_buttons.push(button);

                let job = match found {
                    Some(job) => job,
                    None => return Ok(Vec::new()),
                };

                let Ok(holding) = console_core_number_conversion::fitted::<_, u32>(self.holding.len());

                match holding < AT_ONCE {
                    true => {
                        self.holding.push((button, job));

                        let Ok(repeats) = job.action.repeats();

                        match repeats {
                            RepeatMode::WhileHeld => {
                                self.stepping = Some(Stepping {
                                    button,
                                    job,
                                    until: STEP_AFTER,
                                    gap: STEP_FIRST,
                                });
                            }
                            RepeatMode::Once => {},
                        }
                    }
                    false => {},
                }

                let Ok(acted) = buttons::acted(job, ButtonPress::Down);

                Ok(acted.into_iter().collect())
            }
            0 => {
                self.pressed_buttons.retain(|held| *held != button);

                let (_, job) = match self.holding.iter().position(|(down, _)| *down == button) {
                    Some(at) => self.holding.remove(at),
                    None => return Ok(Vec::new()),
                };

                match self.stepping.as_ref().is_some_and(|held| held.button == button) {
                    true => self.stepping = None,
                    false => {},
                }

                let Ok(acted) = buttons::acted(job, ButtonPress::Up);

                Ok(acted.into_iter().collect())
            }
            _ => Ok(Vec::new()),
        }
    }

    fn held_buttons(&self) -> Result<Vec<&'static str>, Never> {
        let Ok(mut held) = self.pulled.held_names();

        held.extend(self.pressed_buttons.iter().copied());

        Ok(held)
    }

    fn let_go(&mut self) -> Result<Vec<Effect>, Never> {
        self.stepping = None;

        let held = std::mem::take(&mut self.holding);

        Ok(held
            .into_iter()
            .filter_map(|(_, job)| {
                let Ok(acted) = buttons::acted(job, ButtonPress::Up);

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
    ) -> Result<Vec<Effect>, Never> {
        const TOUCHED: u16 = KeyCode::BTN_TOUCH.0;
        const PRESSED: u16 = KeyCode::BTN_0.0;
        const ACROSS: u16 = AbsoluteAxisCode::ABS_X.0;
        const DOWN: u16 = AbsoluteAxisCode::ABS_Y.0;

        match (kind, code) {
            (EventType::KEY, TOUCHED) => {
                let down = match value == 1 {
                    true => ButtonPress::Down,
                    false => ButtonPress::Up,
                };

                self.finger.handle_press(down, now)
            }
            (EventType::KEY, PRESSED) => self.finger.pressed(value),
            (EventType::ABSOLUTE, ACROSS) => {
                let Ok(()) = self.finger.at(Axis::Sideways, value);

                Ok(Vec::new())
            }
            (EventType::ABSOLUTE, DOWN) => {
                let Ok(()) = self.finger.at(Axis::Down, value);

                Ok(Vec::new())
            }
            _ => Ok(Vec::new()),
        }
    }

    pub fn tick(&mut self, seconds: f64) -> Result<Vec<Effect>, Never> {
        let Ok(mut done) = self.tick_repeat(seconds);
        let by = Point { x: self.stick.0, y: self.stick.1 };
        let Ok(notches) = self.wheel.scroll(by, seconds);

        match notches.is_empty() {
            true => {},
            false => done.push(Effect::Frame(notches)),
        }

        Ok(done)
    }

    fn tick_repeat(&mut self, seconds: f64) -> Result<Vec<Effect>, Never> {
        let held = match &mut self.stepping {
            Some(held) => held,
            None => return Ok(Vec::new()),
        };

        held.until -= seconds;

        match held.until > 0.0 {
            true => return Ok(Vec::new()),
            false => {},
        }

        held.gap = (held.gap * STEP_GATHER).max(STEP_FASTEST);
        held.until = held.gap;

        let job = held.job;
        let Ok(acted) = buttons::acted(job, ButtonPress::Down);

        Ok(acted.into_iter().collect())
    }

    pub fn wake(&self) -> Result<Wake, Never> {
        let finger = match self.finger.in_contact {
            true => Wake::Within(crate::touch::POLL),
            false => Wake::OnInput,
        };

        let stepping = match self.stepping {
            Some(_) => Wake::Within(POLL),
            None => Wake::OnInput,
        };

        let stick = match self.stick.0.abs() > 0.0 || self.stick.1.abs() > 0.0 {
            true => Wake::Within(POLL),
            false => Wake::OnInput,
        };

        let Ok(sooner) = finger.sooner(stepping);

        sooner.sooner(stick)
    }
}

pub const POLL: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wake {
    OnInput,
    Within(f64),
}

impl Wake {
    pub fn sooner(self, other: Wake) -> Result<Wake, Never> {
        Ok(match (self, other) {
            (Wake::OnInput, Wake::OnInput) => Wake::OnInput,
            (Wake::Within(seconds), Wake::OnInput) | (Wake::OnInput, Wake::Within(seconds)) => {
                Wake::Within(seconds)
            }
            (Wake::Within(one), Wake::Within(other)) => Wake::Within(one.min(other)),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::effect::Payload;
    use super::*;
    use crate::effect::Output;
    use console_core_number_conversion::{fitted, toward_zero_u32};
    use console_input_event_devices::RelativeAxisCode;

    const RANGES: Ranges = Ranges { stick: Range { low: -32767, high: 32767 }, trigger: (0, 1023) };
    const THEN: f64 = 1000.0;

    fn fresh() -> Result<Controller, Never> {
        let mut held = Controller::default();
        let Ok(()) = held.reading(RANGES);

        Ok(held)
    }

    fn controller() -> Result<Controller, Never> {
        let Ok(mut held) = fresh();
        let Ok(()) = held.using(Input::Pad);

        Ok(held)
    }

    fn pressed(held: &mut Controller, from: From, code: KeyCode) -> Result<Vec<Effect>, Never> {
        let Ok(down) = held.saw(from, EventType::KEY, code.0, 1, THEN);
        let Ok(_) = held.saw(from, EventType::KEY, code.0, 0, THEN);

        Ok(down)
    }

    fn brighter(held: &mut Controller) -> Result<Vec<Effect>, Never> {
        let Ok(_) = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1, THEN);

        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0X.0, 1, THEN)
    }

    fn ticked(held: &mut Controller, seconds: f64) -> Result<u32, Never> {
        let mut steps: u32 = 0;

        for _left in std::iter::successors(Some(seconds), |left| Some(left - POLL)).take_while(|left| *left > 0.0) {
            let Ok(turned) = held.tick(POLL);
            let Ok(these) = fitted::<_, u32>(turned.len());

            steps = steps.saturating_add(these);
        }

        Ok(steps)
    }

    fn key(code: KeyCode, value: i32) -> Result<Effect, Never> {
        let Ok(output) = Output::key(code.0, value);

        Ok(Effect::Frame(vec![output]))
    }

    fn launcher() -> Result<Effect, Never> {
        Effect::run(&["launcher", "--keep"])
    }

    #[test]
    fn a_scale_held_down_goes_on_stepping() {
        let Ok(mut held) = controller();
        let Ok(brightness) = console_core_internal_programs::InternalProgram::Brightness.path();
        let Ok(step) = Effect::run(&[brightness, "up"]);

        assert_eq!(brighter(&mut held), Ok(vec![step]), "the press itself");
        assert_eq!(ticked(&mut held, STEP_AFTER - 0.1), Ok(0), "before the delay is up");

        let Ok(after) = ticked(&mut held, 0.4);

        assert!(after > 0, "after it");
    }

    #[test]
    fn one_press_of_a_scale_is_one_step() {
        let Ok(mut held) = controller();
        let Ok(stepped) = brighter(&mut held);

        assert_eq!(stepped.len(), 1);

        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0X.0, 0, THEN);

        assert_eq!(ticked(&mut held, 3.0), Ok(0), "a press that was let go went on stepping");
    }

    #[test]
    fn a_job_that_is_not_a_scale_does_not_repeat_when_it_is_held() {
        let Ok(mut held) = controller();
        let Ok(launched) = launcher();

        assert_eq!(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 1, THEN), Ok(vec![launched]));
        assert_eq!(ticked(&mut held, 3.0), Ok(0), "the menu opened again on its own");
    }

    #[test]
    fn a_pad_that_went_away_stops_a_scale() {
        let Ok(mut held) = controller();
        let Ok(stepped) = brighter(&mut held);

        assert_eq!(stepped.len(), 1);

        let Ok(_) = held.pad_went();

        assert_eq!(ticked(&mut held, 3.0), Ok(0));
    }

    #[test]
    fn a_held_scale_gathers_pace_and_then_holds_it() {
        let Ok(mut held) = controller();
        let Ok(_) = brighter(&mut held);
        let Ok(_) = ticked(&mut held, STEP_AFTER);
        let Ok(first) = ticked(&mut held, 1.0);
        let Ok(later) = ticked(&mut held, 1.0);

        assert!(later > first, "it did not gather: {first} then {later}");

        let Ok(most) = toward_zero_u32((1.0 / STEP_FASTEST).ceil());

        assert!(later <= most, "{later} steps in a second is past {most}");

        let Ok(settled) = ticked(&mut held, 1.0);

        assert!(
            later.abs_diff(settled) <= 1,
            "it went on gathering past the floor: {later} then {settled}",
        );
    }

    #[test]
    fn a_button_that_starts_something_acts_when_it_goes_down_and_not_when_it_comes_up() {
        let Ok(mut held) = controller();
        let Ok(launched) = launcher();

        assert_eq!(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 1, THEN), Ok(vec![launched]));
        assert_eq!(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 0, THEN), Ok(Vec::new()));
    }

    #[test]
    fn a_button_that_sends_a_key_sends_it_down_and_up() {
        let Ok(mut held) = controller();
        let Ok(clicked) = key(KeyCode::BTN_LEFT, 1);
        let Ok(released) = key(KeyCode::BTN_LEFT, 0);

        assert_eq!(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, THEN), Ok(vec![clicked]));
        assert_eq!(held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0, THEN), Ok(vec![released]));
    }

    #[test]
    fn the_dpad_arrives_as_a_hat_and_is_read_as_four_buttons() {
        let Ok(mut held) = controller();
        let Ok(up) = key(KeyCode::KEY_UP, 1);
        let Ok(up_released) = key(KeyCode::KEY_UP, 0);
        let Ok(down) = key(KeyCode::KEY_DOWN, 1);
        let Ok(down_released) = key(KeyCode::KEY_DOWN, 0);

        assert_eq!(held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, -1, THEN), Ok(vec![up]));
        assert_eq!(
            held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 1, THEN),
            Ok(vec![up_released, down])
        );
        assert_eq!(
            held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 0, THEN),
            Ok(vec![down_released])
        );
    }

    #[test]
    fn standing_down_lets_go_of_a_button_whose_release_will_never_arrive() {
        let Ok(mut held) = controller();
        let Ok(down) = held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F22.0, 1, THEN);

        assert!(!down.is_empty(), "X does nothing at all, so this test is asking nothing");

        let Ok(_) = held.now_in(Mode::Keyboard);
        let Ok(again) = held.now_in(Mode::Desktop);

        assert!(again.is_empty(), "coming back is not a release of its own");
        assert_eq!(
            held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F22.0, 1, 2000.0),
            Ok(down),
            "X was still held from last time, so the press that puts the keyboard back does nothing"
        );
    }

    #[test]
    fn a_button_held_across_a_menu_opening_stays_held() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, THEN);

        assert_eq!(held.now_in(Mode::Tabs), Ok(Vec::new()), "a picker opening let go of A");
    }

    #[test]
    fn a_button_is_let_go_of_by_the_job_that_took_it() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, THEN);
        let Ok(_) = held.now_in(Mode::Tabs);
        let Ok(released) = key(KeyCode::BTN_LEFT, 0);

        assert_eq!(
            held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0, THEN),
            Ok(vec![released]),
            "not Enter"
        );
    }

    #[test]
    fn a_pad_that_went_away_lets_go_of_what_was_held() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, THEN);
        let Ok(released) = key(KeyCode::BTN_LEFT, 0);

        assert_eq!(held.pad_went(), Ok(vec![released]));
        assert_eq!(held.pad_went(), Ok(Vec::new()), "and only the once");
    }

    #[test]
    fn the_shoulders_carry_the_window_while_l2_is_held() {
        let Ok(mut held) = controller();
        let Ok(moved) = Effect::workspace("+1", Payload::None);
        let Ok(carried) = Effect::workspace("+1", Payload::Window);

        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), Ok(vec![moved]));

        let Ok(_) = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1, THEN);

        assert_eq!(held.pulled.l2, Trigger::Pressed);
        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), Ok(vec![carried]));
    }

    #[test]
    fn pulling_l2_past_halfway_is_holding_it() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 400, THEN);

        assert_eq!(held.pulled.l2, Trigger::Loose, "not far enough");

        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 900, THEN);

        assert_eq!(held.pulled.l2, Trigger::Pressed);

        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RZ.0, 900, THEN);

        assert_eq!(held.pulled.r2, Trigger::Pressed);
    }

    #[test]
    fn the_right_stick_turns_the_wheel_and_the_left_one_does_not() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Y.0, -32767, THEN);

        assert_eq!(held.tick(1.0), Ok(Vec::new()), "the left stick is not a wheel");

        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, THEN);
        let Ok(turned) = held.tick(1.0);
        let Ok(wanted) = toward_zero_u32(crate::scroll::MAX_HZ);
        let notches = match turned.as_slice() {
            [Effect::Frame(notches)] => {
                let Ok(counted) = fitted::<_, u32>(notches.len());

                Some(counted)
            },
            _ => None,
        };

        assert_eq!(notches, Some(wanted), "one frame of notches, at the most the wheel turns: {turned:?}");
    }

    #[test]
    fn a_pad_that_went_away_stops_the_wheel() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, THEN);
        let Ok(_) = held.pad_went();

        assert_eq!(held.tick(1.0), Ok(Vec::new()));
    }

    #[test]
    fn a_tap_on_the_touchpad_is_a_click() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, THEN);
        let Ok(clicked) = held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 0, 1000.05);

        assert_eq!(clicked.len(), 2, "down and up");
    }

    #[test]
    fn a_finger_on_the_pad_is_read_at_the_pads_own_pace() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, THEN);

        assert_eq!(held.wake(), Ok(Wake::Within(crate::touch::POLL)));

        let Ok(_) = held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 0, 1000.5);

        assert_eq!(held.wake(), Ok(Wake::OnInput), "a finger lifted is nothing to look at");
    }

    #[test]
    fn a_controller_nobody_is_holding_waits_for_a_press() {
        let Ok(mut held) = controller();

        assert_eq!(held.wake(), Ok(Wake::OnInput));

        let Ok(_) = pressed(&mut held, From::Pad, KeyCode::BTN_SOUTH);
        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 12, THEN);

        assert_eq!(held.wake(), Ok(Wake::OnInput), "a press let go and a stick resting are not held");
    }

    #[test]
    fn a_stick_pushed_is_looked_at_until_it_is_let_go() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, THEN);

        assert_eq!(held.wake(), Ok(Wake::Within(POLL)));

        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, 0, 1000.5);

        assert_eq!(held.wake(), Ok(Wake::OnInput));
    }

    #[test]
    fn a_button_that_repeats_is_looked_at_while_it_is_held() {
        let Ok(mut held) = controller();
        let Ok(_) = brighter(&mut held);

        assert_eq!(held.wake(), Ok(Wake::Within(POLL)));

        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0X.0, 0, 1000.5);

        assert_eq!(held.wake(), Ok(Wake::OnInput));
    }

    #[test]
    fn the_sooner_of_two_wakes_is_the_one_kept() {
        assert_eq!(Wake::OnInput.sooner(Wake::OnInput), Ok(Wake::OnInput));
        assert_eq!(Wake::OnInput.sooner(Wake::Within(1.0)), Ok(Wake::Within(1.0)));
        assert_eq!(Wake::Within(1.0).sooner(Wake::OnInput), Ok(Wake::Within(1.0)));
        assert_eq!(Wake::Within(1.0).sooner(Wake::Within(POLL)), Ok(Wake::Within(POLL)));
    }

    #[test]
    fn a_key_on_a_keyboard_someone_plugged_in_says_which_input_is_being_used() {
        let Ok(mut held) = controller();

        assert_eq!(
            held.saw(From::Typing, EventType::KEY, KeyCode::KEY_I.0, 1, THEN),
            Ok(vec![Effect::Using(Input::Keyboard)])
        );
        assert_eq!(
            held.saw(From::Typing, EventType::KEY, KeyCode::KEY_J.0, 1, THEN),
            Ok(Vec::new()),
            "the input it was already on is not said twice"
        );

        let Ok(back) = pressed(&mut held, From::Pad, KeyCode::BTN_SOUTH);

        assert_eq!(back.first(), Some(&Effect::Using(Input::Pad)), "the last press wins");
    }

    #[test]
    fn a_key_on_a_keyboard_someone_plugged_in_does_nothing_else_at_all() {
        let Ok(mut held) = controller();
        let Ok(typed) = held.saw(From::Typing, EventType::KEY, KeyCode::KEY_I.0, 1, THEN);

        assert!(
            !typed.iter().any(|what| matches!(what, Effect::Run(_) | Effect::Frame(_))),
            "the compositor carries a key, and the daemon acting on one too is it happening twice"
        );
        assert_eq!(
            held.saw(From::Typing, EventType::KEY, KeyCode::KEY_I.0, 0, THEN),
            Ok(Vec::new()),
            "a key let go says nothing"
        );
    }

    #[test]
    fn a_pad_that_is_merely_being_held_has_not_said_anything() {
        let Ok(mut held) = fresh();
        let Ok(()) = held.using(Input::Keyboard);

        assert_eq!(
            held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 12, THEN),
            Ok(Vec::new()),
            "a stick resting on its center is no one using anything"
        );
        assert_eq!(
            held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 3, THEN),
            Ok(Vec::new()),
            "a trigger a few units off zero is a finger lying on it"
        );

        let Ok(moved) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 32767, THEN);

        assert_eq!(moved.first(), Some(&Effect::Using(Input::Pad)), "a stick pushed is meant");
    }

    #[test]
    fn nothing_has_been_pressed_yet_is_not_the_same_as_the_pad() {
        let Ok(mut held) = fresh();

        assert_eq!(
            held.saw(From::Typing, EventType::KEY, KeyCode::KEY_I.0, 1, THEN),
            Ok(vec![Effect::Using(Input::Keyboard)]),
            "a daemon that was told nothing says what the first press was"
        );
    }

    #[test]
    fn what_comes_out_is_movement_on_one_device() {
        let Ok(mut held) = controller();
        let Ok(_) = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 32767, THEN);
        let Ok(turned) = held.tick(1.0);
        let Ok(notch) = Output::relative(RelativeAxisCode::REL_HWHEEL.0, 1);
        let notches = match turned.first() {
            Some(Effect::Frame(notches)) => notches.as_slice(),
            Some(Effect::Run(_) | Effect::Tell(_) | Effect::Using(_) | Effect::Reconnected(_)) | None => &[],
        };

        assert!(notches.contains(&notch), "a frame of notches, and one of them sideways: {turned:?}");
    }
}
