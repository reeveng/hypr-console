//! A Legion Go you can press.
//!
//! What a press turns into is decided the same way the real machine decides
//! it, by the profile that is loaded, so this is a test of the profile as much
//! as of whatever is reading the other end. Loading a different profile changes
//! what the same press means, exactly as `controller-profile` does on the
//! device.
//!
//! Two things here are a model of InputPlumber rather than a recording of it:
//!
//!   * A button with no mapping in the loaded profile is passed through to the
//!     pad untouched. That is what an empty profile means, and game.yaml, which
//!     has no mappings at all and is documented as passing everything through,
//!     is the case that says so.
//!   * An event can only reach a device the profile lists in `target_devices`.
//!     InputPlumber builds the targets a profile names and destroys the rest,
//!     so a mapping that sends a pad button from a profile with no pad in it
//!     sends it nowhere.
//!
//! The touchpad is not in this loop at all. InputPlumber cannot translate it
//! and the compositor makes it absolute, so on the device it is left alone and
//! read directly. It is left alone here too.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use console_core_geometry::Point;
use console_core_never::Never;
use evdev::{EventType, KeyCode};

use crate::Unpressed;
use crate::devices::{Devices, Has, Report, Sink};
use crate::profile::{Kind, Profile, Target};
use crate::vocabulary::{self, Names};

const NO_DISTANCE: i32 = 0;


pub const PRESS_SECONDS: f64 = 0.02;

fn role_of(target_device: &str) -> Result<Option<&'static str>, Never> {
    Ok(match target_device {
        "keyboard" => Some("keyboard"),
        "mouse" => Some("mouse"),
        "xbox-elite" => Some("pad"),
        _ => None,
    })
}

pub trait Clock {
    fn wait(&mut self, seconds: f64);
}

pub struct Passing;

impl Clock for Passing {
    fn wait(&mut self, seconds: f64) {
        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "the emulator is playing back a capture, and how long the person held the button is part of what is being played; `Held` is the same trait without a clock, which is what the tests press"
            )
        )]
        std::thread::sleep(Duration::from_secs_f64(seconds.max(0.0)));
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Held {
    pub waited: Vec<f64>,
}

impl Clock for Held {
    fn wait(&mut self, seconds: f64) {
        self.waited.push(seconds);
    }
}

pub struct LegionGo<S: Sink, C: Clock> {
    pub profiles: BTreeMap<String, Profile>,
    pub devices: Devices<S>,
    pub clock: C,
    profile: String,
    held: BTreeSet<String>,
    nothing: Profile,
}

impl<S: Sink, C: Clock> LegionGo<S, C> {
    pub fn new(
        profiles: BTreeMap<String, Profile>,
        devices: Devices<S>,
        clock: C,
        profile: &str,
    ) -> Result<Self, Unpressed> {
        let mut go = LegionGo {
            profiles,
            devices,
            clock,
            profile: String::new(),
            held: BTreeSet::new(),
            nothing: Profile::default(),
        };
        go.load_profile(profile)?;
        Ok(go)
    }

    pub fn load_profile(&mut self, name: &str) -> Result<(), Unpressed> {
        match self.profiles.contains_key(name) {
            true => {},
            false => {
                let every: Vec<String> = self.profiles.keys().cloned().collect();

                return Err(Unpressed::NoSuchProfile(name.to_string(), every));
            }
        }

        self.profile = name.to_string();
        Ok(())
    }

    pub fn profile_name(&self) -> Result<&str, Never> {
        Ok(&self.profile)
    }

    pub fn profile(&self) -> Result<&Profile, Never> {
        Ok(match self.profiles.get(&self.profile) {
            Some(profile) => profile,
            None => &self.nothing,
        })
    }

    pub fn holding(&self) -> Result<Vec<&str>, Never> {
        Ok(self.held.iter().map(String::as_str).collect())
    }

    pub fn down(&mut self, spoken: &str) -> Result<(), Unpressed> {
        self.held.insert(spoken.to_string());
        self.button(spoken, 1)
    }

    pub fn up(&mut self, spoken: &str) -> Result<(), Unpressed> {
        self.held.remove(spoken);
        self.button(spoken, 0)
    }

    pub fn press(&mut self, spoken: &str) -> Result<(), Unpressed> {
        self.down(spoken)?;
        self.clock.wait(PRESS_SECONDS);
        self.up(spoken)
    }

    pub fn hold(&mut self, spoken: &str) -> Result<(), Unpressed> {
        self.down(spoken)
    }

    pub fn release(&mut self, spoken: &str) -> Result<(), Unpressed> {
        self.up(spoken)
    }

    pub fn release_all(&mut self) -> Result<(), Unpressed> {
        #[cfg_attr(
            dylint_lib = "explicit027_no_needless_collection",
            allow(
                explicit027_no_needless_collection,
                reason = "`up` takes `&mut self` and `self.held` is what would be walked, so the list is what ends the borrow before the first release changes it"
            )
        )]
        let held: Vec<String> = self.held.iter().cloned().collect();

        held.iter().try_for_each(|spoken| self.up(spoken))
    }

    fn button(&mut self, spoken: &str, value: i32) -> Result<(), Unpressed> {
        let Ok(names) = vocabulary::is_trigger(spoken);

        match names {
            Names::ATrigger => {
                let pulled = match value {
                    0 => 0.0,
                    _ => 1.0,
                };

                return self.trigger(spoken, pulled);
            }
            Names::AButton => {},
        }

        let name = vocabulary::button_name(spoken)?;

        let Ok(profile) = self.profile();

        let aimed = profile.targets_of(spoken)?;
        let targets: Vec<Target> = aimed.into_iter().cloned().collect();

        match targets.is_empty() {
            true => self.passthrough(name, value),
            false => targets.iter().try_for_each(|target| self.send(target, value)),
        }
    }

    fn passthrough(&mut self, name: &str, value: i32) -> Result<(), Unpressed> {
        let Ok(profile) = self.profile();

        let Ok(publishes) = profile.publishes("xbox-elite");

        match publishes {
            Has::Yes => self.on_the_pad(name, value),
            Has::No => Ok(()),
        }
    }

    fn send(&mut self, target: &Target, value: i32) -> Result<(), Unpressed> {
        let Ok(needs) = target.kind.needs();

        let Ok(named) = role_of(needs);

        let Ok(profile) = self.profile();

        let Ok(publishes) = profile.publishes(needs);

        let role = match (named, publishes) {
            (Some(role), Has::Yes) => role,
            (Some(_), Has::No) | (None, _) => return Ok(()),
        };

        let Ok(has) = self.devices.has(role);

        match target.kind {
            Kind::GamepadButton => match has {
                Has::Yes => self.on_the_pad(&target.name, value),
                Has::No => Ok(()),
            },
            Kind::Key | Kind::MouseButton => match has {
                Has::No => Ok(()),
                Has::Yes => {
                    let code = target.code()?;

                    self.emit_key(role, code, value)
                }
            },
            Kind::MouseMotion | Kind::GamepadAxis | Kind::GamepadTrigger => Ok(()),
        }
    }

    fn on_the_pad(&mut self, name: &str, value: i32) -> Result<(), Unpressed> {
        let Ok(hat) = vocabulary::hat_code(name);

        match hat {
            Some((axis, end)) => {
                let at = match value {
                    0 => 0,
                    _ => end,
                };

                let Ok(()) = self.devices.emit("pad", EventType::ABSOLUTE, axis.0, at, Report::Now);

                return Ok(());
            }
            None => {},
        }

        let Ok(code) = vocabulary::gamepad_code(name);

        match code {
            Some(code) => self.emit_key("pad", code, value),
            None => Ok(()),
        }
    }

    fn emit_key(&mut self, role: &str, code: KeyCode, value: i32) -> Result<(), Unpressed> {
        let Ok(()) = self.devices.emit(role, EventType::KEY, code.0, value, Report::Now);

        Ok(())
    }

    pub fn stick(&mut self, which: &str, to: Point<f64>) -> Result<(), Unpressed> {
        let Ok(name) = vocabulary::axis_named(which);

        let Ok(found) = vocabulary::axis_codes(name);

        let codes = found.ok_or_else(|| Unpressed::NoStick(which.to_string()))?;

        let Ok(profile) = self.profile();

        let Ok(publishes) = profile.publishes("xbox-elite");

        match publishes {
            Has::No => return Ok(()),
            Has::Yes => {},
        }

        for (code, amount) in [(codes.0, to.across), (codes.1, to.down)] {
            let at = self.devices.absolute("pad", code.0, amount)?;

            let Ok(()) = self.devices.emit("pad", EventType::ABSOLUTE, code.0, at, Report::Later);
        }

        let Ok(()) = self.devices.syn("pad");

        Ok(())
    }

    pub fn centre(&mut self, which: &str) -> Result<(), Unpressed> {
        self.stick(which, Point { across: 0.0, down: 0.0 })
    }

    pub fn trigger(&mut self, which: &str, amount: f64) -> Result<(), Unpressed> {
        let Ok(name) = vocabulary::trigger_named(which);

        let Ok(found) = vocabulary::trigger_code(name);

        let code = found.ok_or_else(|| Unpressed::NoTrigger(which.to_string()))?;

        let Ok(profile) = self.profile();

        let Ok(publishes) = profile.publishes("xbox-elite");

        match publishes {
            Has::No => return Ok(()),
            Has::Yes => {},
        }

        let at = self.devices.along("pad", code.0, amount)?;

        let Ok(()) = self.devices.emit("pad", EventType::ABSOLUTE, code.0, at, Report::Now);

        Ok(())
    }

    pub fn touch_down(&mut self, at: Point<i32>) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_TOUCH.0, 1, Report::Later)?;

        self.touch_at(at)
    }

    pub fn touch_move(&mut self, at: Point<i32>) -> Result<(), Never> {
        self.touch_at(at)
    }

    pub fn touch_up(&mut self) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_TOUCH.0, 0, Report::Now)
    }

    pub fn touch_click(&mut self, value: i32) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_0.0, value, Report::Now)
    }

    pub fn tap(&mut self, at: Point<i32>) -> Result<(), Never> {
        self.touch_down(at)?;

        self.touch_up()
    }

    pub fn drag(
        &mut self,
        from: Point<i32>,
        to: Point<i32>,
        steps: i32,
        seconds: f64,
    ) -> Result<(), Never> {
        self.touch_down(from)?;

        let part = |from: i32, to: i32, step: i32| {
            let across = to.saturating_sub(from).saturating_mul(step);

            let part = match across.checked_div(steps) {
                Some(part) => part,
                None => NO_DISTANCE,
            };

            from.saturating_add(part)
        };

        for step in 1..=steps {
            self.touch_move(Point {
                across: part(from.across, to.across, step),
                down: part(from.down, to.down, step),
            })?;

            match seconds > 0.0 {
                true => self.clock.wait(seconds / f64::from(steps)),
                false => {},
            }
        }

        self.touch_up()
    }

    fn touch_at(&mut self, at: Point<i32>) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::ABSOLUTE, 0, at.across, Report::Later)?;

        self.devices.emit("touchpad", EventType::ABSOLUTE, 1, at.down, Report::Later)?;

        self.devices.syn("touchpad")
    }

    pub fn raw(
        &mut self,
        role: &str,
        kind: EventType,
        code: u16,
        value: i32,
    ) -> Result<(), Never> {
        self.devices.emit(role, kind, code, value, Report::Now)
    }

    pub fn wait(&mut self, seconds: f64) -> Result<(), Never> {
        self.clock.wait(seconds);

        Ok(())
    }

    pub fn close(&mut self) -> Result<(), Never> {
        self.devices.close()
    }
}

pub const MIDDLE: i32 = 512;
