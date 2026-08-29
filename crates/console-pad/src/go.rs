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
//!     pad untouched. That is what an empty profile means, and keyboard.yaml,
//!     which has no mappings at all and is documented as passing everything
//!     through, is the case that says so.
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

use evdev::{EventType, KeyCode};

use crate::devices::{Devices, Sink};
use crate::profile::{Profile, Target};
use crate::vocabulary;

/// How long a press is held before it is let go.
pub const PRESS_SECONDS: f64 = 0.02;

/// The role each target device is published as.
fn role_of(target_device: &str) -> Option<&'static str> {
    match target_device {
        "keyboard" => Some("keyboard"),
        "mouse" => Some("mouse"),
        "xbox-elite" => Some("pad"),
        _ => None,
    }
}

/// Waiting, which is the one thing a test wants to hold still.
pub trait Clock {
    fn wait(&mut self, seconds: f64);
}

/// The wall clock, for a machine somebody is watching.
pub struct Passing;

impl Clock for Passing {
    fn wait(&mut self, seconds: f64) {
        std::thread::sleep(Duration::from_secs_f64(seconds.max(0.0)));
    }
}

/// A clock that does not move, and remembers what it was asked for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Held {
    pub waited: Vec<f64>,
}

impl Clock for Held {
    fn wait(&mut self, seconds: f64) {
        self.waited.push(seconds);
    }
}

/// The front of the machine, and the devices behind it.
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

    // ------------------------------------------------------------- profiles

    /// What `controller-profile <name>` does, without the bus.
    pub fn load_profile(&mut self, name: &str) -> Result<(), String> {
        if !self.profiles.contains_key(name) {
            let every: Vec<&str> = self.profiles.keys().map(String::as_str).collect();
            return Err(format!("no profile called {name:?}; there is {}", every.join(", ")));
        }
        self.profile = name.to_string();
        Ok(())
    }

    pub fn profile_name(&self) -> &str {
        &self.profile
    }

    pub fn profile(&self) -> &Profile {
        &self.profiles[&self.profile]
    }

    /// What is being held down, in the words they were pressed by.
    pub fn holding(&self) -> Vec<&str> {
        self.held.iter().map(String::as_str).collect()
    }

    // -------------------------------------------------------------- buttons

    /// Press and keep pressing.
    pub fn down(&mut self, spoken: &str) -> Result<(), String> {
        self.held.insert(spoken.to_string());
        self.button(spoken, 1)
    }

    /// Let go.
    pub fn up(&mut self, spoken: &str) -> Result<(), String> {
        self.held.remove(spoken);
        self.button(spoken, 0)
    }

    pub fn press(&mut self, spoken: &str) -> Result<(), String> {
        self.down(spoken)?;
        self.clock.wait(PRESS_SECONDS);
        self.up(spoken)
    }

    /// Held until `release` or `release_all`. Reads better in a scenario.
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
        // A trigger is an axis, and holding one is pulling it all the way.
        // Saying "hold l2" is what a person means, so it is what it does.
        if vocabulary::is_trigger(spoken) {
            return self.trigger(spoken, if value == 0 { 0.0 } else { 1.0 });
        }
        let name = vocabulary::button_name(spoken)?;
        let targets: Vec<Target> =
            self.profile().targets_of(spoken)?.into_iter().cloned().collect();
        match targets.is_empty() {
            true => self.passthrough(name, value),
            false => targets.iter().try_for_each(|target| self.send(target, value)),
        }
    }

    /// No mapping: the press reaches the pad as itself, if there is a pad.
    fn passthrough(&mut self, name: &str, value: i32) -> Result<(), String> {
        let code = vocabulary::gamepad_code(name);
        match code {
            Some(code) if self.profile().publishes("xbox-elite") => {
                self.emit_key("pad", code, value)
            }
            _ => Ok(()),
        }
    }

    fn send(&mut self, target: &Target, value: i32) -> Result<(), String> {
        let role = match role_of(target.kind.needs()) {
            Some(role) if self.profile().publishes(target.kind.needs()) => role,
            _ => return Ok(()),
        };
        match (target.code(), self.devices.has(role)) {
            (Some(code), true) => self.emit_key(role, code, value),
            _ => Ok(()),
        }
    }

    fn emit_key(&mut self, role: &str, code: KeyCode, value: i32) -> Result<(), String> {
        self.devices.emit(role, EventType::KEY, code.0, value, true);
        Ok(())
    }

    // --------------------------------------------------------------- sticks

    /// Push a stick, each axis from -1 to 1. Up the screen is negative y.
    pub fn stick(&mut self, which: &str, x: f64, y: f64) -> Result<(), String> {
        let name = vocabulary::axis_named(which);
        let codes = vocabulary::axis_codes(name)
            .ok_or_else(|| format!("no stick called {which:?}"))?;
        if !self.profile().publishes("xbox-elite") {
            return Ok(());
        }
        for (code, amount) in [(codes.0, x), (codes.1, y)] {
            let at = self.devices.absolute("pad", code.0, amount)?;
            self.devices.emit("pad", EventType::ABSOLUTE, code.0, at, false);
        }
        self.devices.syn("pad");
        Ok(())
    }

    pub fn centre(&mut self, which: &str) -> Result<(), String> {
        self.stick(which, 0.0, 0.0)
    }

    /// Pull a trigger, from 0 to 1.
    pub fn trigger(&mut self, which: &str, amount: f64) -> Result<(), String> {
        let name = vocabulary::trigger_named(which);
        let code = vocabulary::trigger_code(name)
            .ok_or_else(|| format!("no trigger called {which:?}"))?;
        if !self.profile().publishes("xbox-elite") {
            return Ok(());
        }
        let at = self.devices.along("pad", code.0, amount)?;
        self.devices.emit("pad", EventType::ABSOLUTE, code.0, at, true);
        Ok(())
    }

    // -------------------------------------------------------------- touchpad

    pub fn touch_down(&mut self, x: i32, y: i32) {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_TOUCH.0, 1, false);
        self.touch_at(x, y);
    }

    pub fn touch_move(&mut self, x: i32, y: i32) {
        self.touch_at(x, y);
    }

    pub fn touch_up(&mut self) {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_TOUCH.0, 0, true);
    }

    /// The pad pressed in, which is a button of its own, not a tap.
    pub fn touch_click(&mut self, value: i32) {
        self.devices.emit("touchpad", EventType::KEY, KeyCode::BTN_0.0, value, true);
    }

    pub fn tap(&mut self, x: i32, y: i32) {
        self.touch_down(x, y);
        self.touch_up();
    }

    /// A finger from one place to another, in as many reports.
    pub fn drag(&mut self, from: (i32, i32), to: (i32, i32), steps: i32, seconds: f64) {
        self.touch_down(from.0, from.1);
        for step in 1..=steps {
            self.touch_move(
                from.0 + (to.0 - from.0) * step / steps,
                from.1 + (to.1 - from.1) * step / steps,
            );
            if seconds > 0.0 {
                self.clock.wait(seconds / f64::from(steps));
            }
        }
        self.touch_up();
    }

    fn touch_at(&mut self, x: i32, y: i32) {
        self.devices.emit("touchpad", EventType::ABSOLUTE, 0, x, false);
        self.devices.emit("touchpad", EventType::ABSOLUTE, 1, y, false);
        self.devices.syn("touchpad");
    }

    // ------------------------------------------------------------------- raw

    /// Straight onto a device, with no profile in the way.
    pub fn raw(&mut self, role: &str, kind: EventType, code: u16, value: i32) {
        self.devices.emit(role, kind, code, value, true);
    }

    pub fn wait(&mut self, seconds: f64) {
        self.clock.wait(seconds);
    }

    pub fn close(&mut self) {
        self.devices.close();
    }
}

/// Where a stick is centred, and where a touch lands when nothing says.
pub const MIDDLE: i32 = 512;
