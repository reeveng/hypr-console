//! What arrived, and what to do about it.
//!
//! Three devices are read: the pad InputPlumber publishes, the keyboard it
//! publishes beside it, and the controller's own touchpad. This is all of the
//! deciding, and none of the opening.

use evdev::{AbsoluteAxisCode, EventType, KeyCode};

use console_pad::jobs::Layer;
use console_pad::routing::{self, Hat};
use console_pad::vocabulary::spoken_for;

use crate::buttons;
use crate::doing::Doing;
use crate::means::{Job, Press, Repeats, Table};
use crate::touch::Axis;
use crate::mode::Mode;
use crate::scroll::{Wheel, pushed};
use crate::touch::Finger;

/// How far a trigger must be pulled to count as held.
pub const CARRY_HELD: f64 = 0.5;

/// How long a step job is held before it starts repeating.
///
/// Long enough that one press is one step and never two. A thumb that means a
/// single notch is off the button well inside this; a thumb that means "keep
/// going" is still on it.
pub const STEP_AFTER: f64 = 0.400;

/// The gap between the first repeats, and the shortest it gathers to.
///
/// It accelerates because the distance somebody wants is not known when they
/// start: a nudge is one or two steps and a reach for silent is twenty, and a
/// rate that suits either one is wrong for the other. Starting slow keeps the
/// first repeats countable, and gathering means the far end arrives without
/// the thumb having to stay down all day.
pub const STEP_FIRST: f64 = 0.180;
pub const STEP_FASTEST: f64 = 0.080;

/// What each repeat takes off the gap before the next one.
///
/// Not so steep that a scale runs away from a thumb that meant three steps.
const STEP_GATHER: f64 = 0.85;

/// How many buttons can be down at once before this stops remembering them.
///
/// Ten, which is more fingers than anybody has. What is remembered is which
/// job took each press, so that the release goes to the same job: a button
/// pressed on the desktop and let go with a menu open would otherwise put a
/// key down as one thing and lift it as another, and the one that matters is
/// the mouse button -- a click that goes down and never comes up is a pointer
/// dragging everything it touches until the machine is restarted.
pub const AT_ONCE: usize = 10;

/// Which device something arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum From {
    Pad,
    Keys,
    Touch,
}

/// The ranges the pad reports over, read off it when it is found.
///
/// Held rather than asked for every event: the pad goes away whenever a
/// profile is switched, and arithmetic that has to ask a device that is not
/// there is arithmetic that stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ranges {
    pub stick: i32,
    pub trigger: (i32, i32),
}

/// Whether a trigger is far enough in to count as held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Past the point where it reads as a press.
    Held,
    /// Not yet, however far it has travelled.
    Loose,
}

impl Default for Ranges {
    fn default() -> Self {
        Ranges { stick: 1, trigger: (0, 1) }
    }
}

/// A step job being held down, and when the next step is due.
///
/// The job is the one that took the press and not the one the table would
/// answer now, for the same reason `holding` keeps it: what a button means can
/// change under a thumb that has not moved, and a scale that changed direction
/// halfway through a hold would be a scale nobody could aim.
#[derive(Debug, Clone, PartialEq)]
struct Stepping {
    button: &'static str,
    job: &'static Job,
    /// Seconds until the next step goes out.
    until: f64,
    /// The gap being waited out, which shortens as the thumb stays on.
    gap: f64,
}

/// Everything the daemon is holding between one event and the next.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Controller {
    /// What is in front of you, which is what the buttons are for.
    ///
    /// Read off the compositor rather than remembered, and set from outside
    /// by whatever is watching the screen. Default is the desktop, which is
    /// where this daemon starts and what it falls back to when the compositor
    /// cannot be asked.
    pub mode: Mode,
    /// Which triggers are pulled far enough to be a layer.
    pub layer: Layer,
    pub wheel: Wheel,
    pub finger: Finger,
    pub ranges: Ranges,
    /// What each thing this desktop does is bound to on this machine.
    ///
    /// Handed in rather than read here, for the same reason nothing else in
    /// this crate opens a file: what a press comes to has to be a question
    /// that can be asked twice and answered the same way.
    pub table: Table,
    /// Which job took the press of each button that is down.
    holding: Vec<(&'static str, &'static Job)>,
    /// The step job with a thumb still on it, where there is one.
    stepping: Option<Stepping>,
    /// Where the d-pad's two axes are standing.
    hat: (i32, i32),
    stick: (f64, f64),
}

impl Controller {
    /// The pad has been found, and reports over these ranges.
    pub fn reading(&mut self, ranges: Ranges) {
        self.ranges = ranges;
    }

    /// What is in front of you has changed.
    pub fn now_in(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// The pad has gone, which a profile switch does every time.
    ///
    /// Reading from nothing is what used to end this process, and it took the
    /// workspace buttons with it.
    pub fn pad_went(&mut self) -> Vec<Doing> {
        self.stick = (0.0, 0.0);
        self.hat = (0, 0);
        self.layer = Layer::default();
        self.let_go()
    }

    /// One event, and what it comes to.
    ///
    /// The time it arrived is handed in rather than read, because whether a
    /// touch was a tap is arithmetic and arithmetic has one right answer.
    pub fn saw(
        &mut self,
        from: From,
        kind: EventType,
        code: u16,
        value: i32,
        now: f64,
    ) -> Vec<Doing> {
        match from {
            From::Pad => self.on_pad(kind, code, value),
            From::Keys => self.on_keys(kind, code, value),
            From::Touch => self.on_touch(kind, code, value, now),
        }
    }

    fn on_pad(&mut self, kind: EventType, code: u16, value: i32) -> Vec<Doing> {
        match kind {
            EventType::ABSOLUTE => self.on_axis(code, value),
            EventType::KEY => match routing::button_of_pad(code) {
                Some(button) => self.pressed(button, value),
                None => self.on_trigger_button(code, value),
            },
            _ => Vec::new(),
        }
    }

    /// A trigger reported as a button, which some pads do and this one does
    /// not.
    ///
    /// Kept because it costs a line and because the pad this reads is not the
    /// hardware: it is whatever InputPlumber publishes, and what that reports
    /// a pulled trigger as is its business rather than ours.
    fn on_trigger_button(&mut self, code: u16, value: i32) -> Vec<Doing> {
        if code == KeyCode::BTN_TL2.0 {
            self.layer.l2 = value == 1;
        } else if code == KeyCode::BTN_TR2.0 {
            self.layer.r2 = value == 1;
        }

        Vec::new()
    }

    fn on_axis(&mut self, code: u16, value: i32) -> Vec<Doing> {
        // The two triggers, which are the two layers. How far each is pulled
        // is the whole of what makes a chord a chord.
        if code == AbsoluteAxisCode::ABS_Z.0 || code == AbsoluteAxisCode::ABS_RZ.0 {
            let held = self.pulled(value) == Trigger::Held;

            match code == AbsoluteAxisCode::ABS_Z.0 {
                true => self.layer.l2 = held,
                false => self.layer.r2 = held,
            }

            return Vec::new();
        }

        if routing::is_hat(code) == Hat::Axis {
            return self.on_hat(code, value);
        }

        if code == AbsoluteAxisCode::ABS_RX.0 {
            self.stick.0 = pushed(value, self.ranges.stick);
        } else if code == AbsoluteAxisCode::ABS_RY.0 {
            self.stick.1 = pushed(value, self.ranges.stick);
        }

        Vec::new()
    }

    /// How far a trigger is pulled, as held or not held.
    ///
    /// Both triggers are read against the range the pad reported for the left
    /// one. They are the same two ends on this hardware, and a pad that
    /// reported two different ranges for its two triggers would be a pad worth
    /// asking about rather than one worth guessing at.
    fn pulled(&self, value: i32) -> Trigger {
        let (low, high) = self.ranges.trigger;
        let span = f64::from((high - low).max(1));

        match f64::from(value - low) / span > CARRY_HELD {
            true => Trigger::Held,
            false => Trigger::Loose,
        }
    }

    /// The d-pad, which arrives as a hat: two axes with three positions each.
    ///
    /// Rolling from one end to the other without passing through the middle is
    /// one event, so what was down is let go of before what is down now is
    /// taken. A thumb rolling around a d-pad does exactly that.
    fn on_hat(&mut self, code: u16, value: i32) -> Vec<Doing> {
        let was = match code == AbsoluteAxisCode::ABS_HAT0X.0 {
            true => std::mem::replace(&mut self.hat.0, value),
            false => std::mem::replace(&mut self.hat.1, value),
        };

        if was == value {
            return Vec::new();
        }

        let mut done = Vec::new();

        if let Some(button) = routing::button_of_hat(code, was) {
            done.extend(self.pressed(button, 0));
        }

        if let Some(button) = routing::button_of_hat(code, value) {
            done.extend(self.pressed(button, 1));
        }

        done
    }

    fn on_keys(&mut self, kind: EventType, code: u16, value: i32) -> Vec<Doing> {
        if kind != EventType::KEY {
            return Vec::new();
        }

        match routing::button_of_key(code) {
            Some(button) => self.pressed(button, value),
            None => Vec::new(),
        }
    }

    /// One button, down or up, and what it comes to.
    ///
    /// The job is looked up when the button goes down and remembered until it
    /// comes back up. Not looked up twice, because the answer can change while
    /// a thumb is still on the button: a menu opens under it, or the other
    /// hand lets go of a trigger. Looked up twice, A pressed on the desktop
    /// and released with a chooser up would put the mouse button down and lift
    /// Enter, and the mouse button would stay down for ever.
    ///
    /// A repeat -- the kernel's, value 2 -- is not a press. Whatever this
    /// sends is held for as long as the button is, and the compositor is what
    /// repeats a held key.
    fn pressed(&mut self, button: &'static str, value: i32) -> Vec<Doing> {
        let button = spoken_for(button);

        match value {
            1 => {
                if self.holding.iter().any(|(down, _)| *down == button) {
                    return Vec::new();
                }

                let Some(job) = buttons::job_for(&self.table, self.mode, button, self.layer) else {
                    return Vec::new();
                };

                if self.holding.len() < AT_ONCE {
                    self.holding.push((button, job));

                    // Only what is remembered is repeated. A press this is too
                    // full to hold is one no release will be matched against,
                    // and a step nothing can stop is worse than a step missed.
                    if job.what.repeats() == Repeats::WhileHeld {
                        self.stepping = Some(Stepping {
                            button,
                            job,
                            until: STEP_AFTER,
                            gap: STEP_FIRST,
                        });
                    }
                }

                buttons::acted(job, Press::Down).into_iter().collect()
            }
            0 => {
                let Some(at) = self.holding.iter().position(|(down, _)| *down == button) else {
                    return Vec::new();
                };

                let (_, job) = self.holding.remove(at);

                if self.stepping.as_ref().is_some_and(|held| held.button == button) {
                    self.stepping = None;
                }

                buttons::acted(job, Press::Up).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    /// Everything still down, let go of.
    ///
    /// What is sent when a button goes down is held until it comes up, and the
    /// device it was coming from can be taken away in between -- which a
    /// profile switch does every time. Without this, the last thing pressed
    /// before a switch is held down for ever by a daemon that will never see
    /// its release.
    fn let_go(&mut self) -> Vec<Doing> {
        self.stepping = None;
        let held = std::mem::take(&mut self.holding);
        held.into_iter().filter_map(|(_, job)| buttons::acted(job, Press::Up)).collect()
    }

    fn on_touch(&mut self, kind: EventType, code: u16, value: i32, now: f64) -> Vec<Doing> {
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

                self.finger.at(along, value);
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// A moment has passed. The stick is where it was, so the wheel turns,
    /// and a step job under a thumb takes its next step.
    pub fn tick(&mut self, seconds: f64) -> Vec<Doing> {
        let mut done = self.stepped(seconds);
        let notches = self.wheel.turned(self.stick.0, self.stick.1, seconds);

        if !notches.is_empty() {
            done.push(Doing::Frame(notches));
        }

        done
    }

    /// A scale with a thumb held on it, stepped again.
    ///
    /// One step a tick at most. The gap gathers towards `STEP_FASTEST` and
    /// stops there rather than going on shortening, because a scale that ends
    /// up stepping faster than it can be watched is one that overshoots every
    /// time.
    fn stepped(&mut self, seconds: f64) -> Vec<Doing> {
        let Some(held) = &mut self.stepping else { return Vec::new() };

        held.until -= seconds;

        if held.until > 0.0 {
            return Vec::new();
        }

        held.gap = (held.gap * STEP_GATHER).max(STEP_FASTEST);
        held.until = held.gap;
        let job = held.job;
        buttons::acted(job, Press::Down).into_iter().collect()
    }

    /// How long to wait before reading again.
    ///
    /// A finger on the pad is read at the pad's own pace. Anything slower
    /// arrives as a series of jumps, which reads as a slow machine.
    pub fn poll(&self) -> f64 {
        match self.finger.down {
            true => crate::touch::POLL,
            false => POLL,
        }
    }
}

/// Seconds between ticks when nothing is under a thumb.
pub const POLL: f64 = 0.02;

#[cfg(test)]
mod tests {
    use crate::doing::Carry;
    use super::*;
    use crate::doing::Out;
    use evdev::RelativeAxisCode;

    /// The pad's real ranges, as the capture records them.
    fn ranges() -> Ranges {
        Ranges { stick: 32767, trigger: (0, 1023) }
    }

    fn controller() -> Controller {
        let mut held = Controller::default();
        held.reading(ranges());
        held
    }

    fn pressed(held: &mut Controller, from: From, code: KeyCode) -> Vec<Doing> {
        let down = held.saw(from, EventType::KEY, code.0, 1, 1000.0);
        held.saw(from, EventType::KEY, code.0, 0, 1000.0);
        down
    }

    /// L2 and the d-pad right, which is the screen getting brighter.
    fn brighter(held: &mut Controller) -> Vec<Doing> {
        held.saw(From::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1, 1000.0);
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0X.0, 1, 1000.0)
    }

    /// Ticks at the rate the daemon really polls at, and counts what came out.
    fn ticked(held: &mut Controller, seconds: f64) -> usize {
        let mut steps = 0;
        let mut left = seconds;
        while left > 0.0 {
            steps += held.tick(POLL).len();
            left -= POLL;
        }
        steps
    }

    /// A thumb left on a scale goes on moving it.
    ///
    /// Five percent a press is twenty presses from silent to loud, and the
    /// thumb is already on the button.
    #[test]
    fn a_scale_held_down_goes_on_stepping() {
        let mut held = controller();
        let step = Doing::run(&["/usr/local/bin/console-brightness", "up"]);
        assert_eq!(brighter(&mut held), std::slice::from_ref(&step), "the press itself");
        assert_eq!(ticked(&mut held, STEP_AFTER - 0.1), 0, "before the delay is up");
        assert!(ticked(&mut held, 0.4) > 0, "after it");
    }

    /// And one press is one step. A thumb that meant a single notch is off
    /// the button well inside the delay, and nothing follows it.
    #[test]
    fn one_press_of_a_scale_is_one_step() {
        let mut held = controller();
        assert_eq!(brighter(&mut held).len(), 1);
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0X.0, 0, 1000.0);
        assert_eq!(ticked(&mut held, 3.0), 0, "a press that was let go went on stepping");
    }

    /// The menu is not a scale. Holding it opens one menu.
    #[test]
    fn a_job_that_is_not_a_scale_does_not_repeat_when_it_is_held() {
        let mut held = controller();
        let down = held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 1, 1000.0);
        assert_eq!(down, [Doing::run(&["launcher", "--keep"])]);
        assert_eq!(ticked(&mut held, 3.0), 0, "the menu opened again on its own");
    }

    /// A pad taken away under a held scale stops it. The release is never
    /// going to arrive, and a scale nothing can stop runs to one end.
    #[test]
    fn a_pad_that_went_away_stops_a_scale() {
        let mut held = controller();
        assert_eq!(brighter(&mut held).len(), 1);
        held.pad_went();
        assert_eq!(ticked(&mut held, 3.0), 0);
    }

    /// It gathers, and then it stops gathering.
    ///
    /// A nudge is one or two steps and a reach for silent is twenty, so the
    /// rate cannot suit both without changing. It must not go on changing:
    /// a scale stepping faster than it can be watched overshoots every time.
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
        // Settled, rather than identical: the gap does not divide the poll, so
        // a step that falls either side of a second's edge counts in one of
        // them and not the other.
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
            held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 1, 1000.0),
            [Doing::run(&["launcher", "--keep"])]
        );
        assert!(held.saw(From::Keys, EventType::KEY, KeyCode::KEY_F13.0, 0, 1000.0).is_empty());
    }

    /// A key follows the button, both ways. Anything else is a key that is
    /// held down by a daemon rather than by a thumb.
    #[test]
    fn a_button_that_sends_a_key_sends_it_down_and_up() {
        let mut held = controller();
        let down = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, 1000.0);
        assert_eq!(down, [Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 1)])]);
        let up = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0, 1000.0);
        assert_eq!(up, [Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 0)])]);
    }

    /// The d-pad is a hat, and a thumb rolling from one end to the other in
    /// one event lets go of where it was before it takes where it is.
    #[test]
    fn the_dpad_arrives_as_a_hat_and_is_read_as_four_buttons() {
        let mut held = controller();
        let up = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, -1, 1000.0);
        assert_eq!(up, [Doing::Frame(vec![Out::key(KeyCode::KEY_UP.0, 1)])]);
        let over = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 1, 1000.0);
        assert_eq!(
            over,
            [
                Doing::Frame(vec![Out::key(KeyCode::KEY_UP.0, 0)]),
                Doing::Frame(vec![Out::key(KeyCode::KEY_DOWN.0, 1)]),
            ]
        );
        let middle = held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_HAT0Y.0, 0, 1000.0);
        assert_eq!(middle, [Doing::Frame(vec![Out::key(KeyCode::KEY_DOWN.0, 0)])]);
    }

    /// The release goes to the job that took the press, whatever has happened
    /// since. The one that matters is the mouse button: a click that goes down
    /// and never comes up drags everything it touches.
    #[test]
    fn a_button_is_let_go_of_by_the_job_that_took_it() {
        let mut held = controller();
        held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, 1000.0);
        held.now_in(Mode::Tabs);
        let up = held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 0, 1000.0);
        assert_eq!(up, [Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 0)])], "not Enter");
    }

    /// And a pad taken away under a held button lets go of it, because the
    /// release is never going to arrive.
    #[test]
    fn a_pad_that_went_away_lets_go_of_what_was_held() {
        let mut held = controller();
        held.saw(From::Pad, EventType::KEY, KeyCode::BTN_SOUTH.0, 1, 1000.0);
        assert_eq!(held.pad_went(), [Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 0)])]);
        assert!(held.pad_went().is_empty(), "and only the once");
    }

    #[test]
    fn the_shoulders_carry_the_window_while_l2_is_held() {
        let mut held = controller();
        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), [Doing::workspace("+1", Carry::Nothing)]);
        held.saw(From::Pad, EventType::KEY, KeyCode::BTN_TL2.0, 1, 1000.0);
        assert!(held.layer.l2);
        assert_eq!(pressed(&mut held, From::Pad, KeyCode::BTN_TR), [Doing::workspace("+1", Carry::Window)]);
    }

    /// L2 is an axis before it is a button, and how far it is pulled is what
    /// says whether it is being held.
    #[test]
    fn pulling_l2_past_halfway_is_holding_it() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 400, 1000.0);
        assert!(!held.layer.l2, "not far enough");
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Z.0, 900, 1000.0);
        assert!(held.layer.l2);
        // And the right trigger is the other layer, read the same way.
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RZ.0, 900, 1000.0);
        assert!(held.layer.r2);
    }

    #[test]
    fn the_right_stick_turns_the_wheel_and_the_left_one_does_not() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Y.0, -32767, 1000.0);
        assert!(held.tick(1.0).is_empty(), "the left stick is not a wheel");
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, 1000.0);
        let turned = held.tick(1.0);

        // A second at full deflection is `MAX_HZ` notches. Named rather than
        // written out: a number here is one that goes stale the first time
        // somebody changes how fast the stick scrolls, and it did.
        let wanted = console_number::toward_zero_usize(crate::scroll::MAX_HZ);

        assert!(matches!(turned.as_slice(), [Doing::Frame(notches)] if notches.len() == wanted));
    }

    /// The pad goes away whenever a profile is switched. The stick has to stop
    /// where it stood, or the wheel turns forever on a device that is gone.
    #[test]
    fn a_pad_that_went_away_stops_the_wheel() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RY.0, -32767, 1000.0);
        held.pad_went();
        assert!(held.tick(1.0).is_empty());
    }

    #[test]
    fn a_tap_on_the_touchpad_is_a_click() {
        let mut held = controller();
        held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, 1000.0);
        let clicked = held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 0, 1000.05);
        assert_eq!(clicked.len(), 2, "down and up");
    }

    #[test]
    fn a_finger_on_the_pad_is_read_at_the_pads_own_pace() {
        let mut held = controller();
        assert_eq!(held.poll(), POLL);
        held.saw(From::Touch, EventType::KEY, KeyCode::BTN_TOUCH.0, 1, 1000.0);
        assert_eq!(held.poll(), crate::touch::POLL);
    }

    /// Everything the daemon publishes goes out of its own device, so a wheel
    /// notch and a pointer move are the same kind of thing.
    #[test]
    fn what_comes_out_is_movement_on_one_device() {
        let mut held = controller();
        held.saw(From::Pad, EventType::ABSOLUTE, AbsoluteAxisCode::ABS_RX.0, 32767, 1000.0);
        let turned = held.tick(1.0);
        let Some(Doing::Frame(notches)) = turned.first() else {
            panic!("a frame of notches");
        };
        assert!(notches.contains(&Out::rel(RelativeAxisCode::REL_HWHEEL.0, 1)));
    }
}
