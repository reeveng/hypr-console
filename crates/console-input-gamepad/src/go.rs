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
use std::sync::OnceLock;
use std::time::Duration;

use console_core_never::Never;
use evdev::{EventType, KeyCode};

use crate::devices::{Devices, Has, Report, Sink};
use crate::profile::{Kind, Profile, Target};
use crate::vocabulary::{self, Names};

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
}

impl<S: Sink, C: Clock> LegionGo<S, C> {
    pub fn new(
        profiles: BTreeMap<String, Profile>,
        devices: Devices<S>,
        clock: C,
        profile: &str,
    ) -> Result<Self, String> {
        let mut go =
            LegionGo { profiles, devices, clock, profile: String::new(), held: BTreeSet::new() };
        go.load_profile(profile)?;
        Ok(go)
    }

    pub fn load_profile(&mut self, name: &str) -> Result<(), String> {
        match self.profiles.contains_key(name) {
            true => {},
            false => {
                let every: Vec<&str> = self.profiles.keys().map(String::as_str).collect();
                return Err(format!("no profile called {name:?}; there is {}", every.join(", ")));
            }
        }

        self.profile = name.to_string();
        Ok(())
    }

    pub fn profile_name(&self) -> Result<&str, Never> {
        Ok(&self.profile)
    }

    pub fn profile(&self) -> Result<&Profile, Never> {
        static NOTHING: OnceLock<Profile> = OnceLock::new();

        Ok(match self.profiles.get(&self.profile) {
            Some(profile) => profile,
            None => NOTHING.get_or_init(Profile::default),
        })
    }

    pub fn holding(&self) -> Result<Vec<&str>, Never> {
        Ok(self.held.iter().map(String::as_str).collect())
    }

    pub fn down(&mut self, spoken: &str) -> Result<(), String> {
        self.held.insert(spoken.to_string());
        self.button(spoken, 1)
    }

    pub fn up(&mut self, spoken: &str) -> Result<(), String> {
        self.held.remove(spoken);
        self.button(spoken, 0)
    }

    pub fn press(&mut self, spoken: &str) -> Result<(), String> {
        self.down(spoken)?;
        self.clock.wait(PRESS_SECONDS);
        self.up(spoken)
    }

    pub fn hold(&mut self, spoken: &str) -> Result<(), String> {
        self.down(spoken)
    }

    pub fn release(&mut self, spoken: &str) -> Result<(), String> {
        self.up(spoken)
    }

    pub fn release_all(&mut self) -> Result<(), String> {
        let held: Vec<String> = self.held.iter().cloned().collect();
        held.iter().try_for_each(|spoken| self.up(spoken))
    }

    fn button(&mut self, spoken: &str, value: i32) -> Result<(), String> {
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

    fn passthrough(&mut self, name: &str, value: i32) -> Result<(), String> {
        let Ok(profile) = self.profile();

        let Ok(publishes) = profile.publishes("xbox-elite");

        match publishes {
            Has::Yes => self.on_the_pad(name, value),
            Has::No => Ok(()),
        }
    }

    fn send(&mut self, target: &Target, value: i32) -> Result<(), String> {
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

    fn on_the_pad(&mut self, name: &str, value: i32) -> Result<(), String> {
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

    fn emit_key(&mut self, role: &str, code: KeyCode, value: i32) -> Result<(), String> {
        let Ok(()) = self.devices.emit(role, EventType::KEY, code.0, value, Report::Now);

        Ok(())
    }

    pub fn stick(&mut self, which: &str, x: f64, y: f64) -> Result<(), String> {
        let Ok(name) = vocabulary::axis_named(which);

        let Ok(found) = vocabulary::axis_codes(name);

        let codes = found.ok_or_else(|| format!("no stick called {which:?}"))?;

        let Ok(profile) = self.profile();

        let Ok(publishes) = profile.publishes("xbox-elite");

        match publishes {
            Has::No => return Ok(()),
            Has::Yes => {},
        }

        for (code, amount) in [(codes.0, x), (codes.1, y)] {
            let at = self.devices.absolute("pad", code.0, amount)?;

            let Ok(()) = self.devices.emit("pad", EventType::ABSOLUTE, code.0, at, Report::Later);
        }

        let Ok(()) = self.devices.syn("pad");

        Ok(())
    }

    pub fn centre(&mut self, which: &str) -> Result<(), String> {
        self.stick(which, 0.0, 0.0)
    }

    pub fn trigger(&mut self, which: &str, amount: f64) -> Result<(), String> {
        let Ok(name) = vocabulary::trigger_named(which);

        let Ok(found) = vocabulary::trigger_code(name);

        let code = found.ok_or_else(|| format!("no trigger called {which:?}"))?;

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

    pub fn touch_down(&mut self, x: i32, y: i32) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_TOUCH.0, 1, Report::Later)?;

        self.touch_at(x, y)
    }

    pub fn touch_move(&mut self, x: i32, y: i32) -> Result<(), Never> {
        self.touch_at(x, y)
    }

    pub fn touch_up(&mut self) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_TOUCH.0, 0, Report::Now)
    }

    pub fn touch_click(&mut self, value: i32) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_0.0, value, Report::Now)
    }

    pub fn tap(&mut self, x: i32, y: i32) -> Result<(), Never> {
        self.touch_down(x, y)?;

        self.touch_up()
    }

    pub fn drag(
        &mut self,
        from: (i32, i32),
        to: (i32, i32),
        steps: i32,
        seconds: f64,
    ) -> Result<(), Never> {
        self.touch_down(from.0, from.1)?;

        for step in 1..=steps {
            self.touch_move(
                from.0.saturating_add(
                    to.0.saturating_sub(from.0).saturating_mul(step).checked_div(steps).unwrap_or(0),
                ),
                from.1.saturating_add(
                    to.1.saturating_sub(from.1).saturating_mul(step).checked_div(steps).unwrap_or(0),
                ),
            )?;

            match seconds > 0.0 {
                true => self.clock.wait(seconds / f64::from(steps)),
                false => {},
            }
        }

        self.touch_up()
    }

    fn touch_at(&mut self, x: i32, y: i32) -> Result<(), Never> {
        self.devices.emit("touchpad", EventType::ABSOLUTE, 0, x, Report::Later)?;

        self.devices.emit("touchpad", EventType::ABSOLUTE, 1, y, Report::Later)?;

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
