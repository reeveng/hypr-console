//! The device's own desktop, nested on this machine, and looked at.
//!
//! What this can answer that nothing else can is what colour the screen is. A
//! service being active proves nothing about whether it is doing its job: the
//! wallpaper on the device did not paint for days because hyprpaper read a
//! config format it no longer understood, painted nothing, and reported
//! success. Nothing was in a failed state. The screen was the wrong colour.
//!
//! It cannot press anything. That needs an input device, which needs
//! /dev/uinput, which is the other tier.

use std::path::PathBuf;
use std::process::Command;

use crate::picture::{Picture, where_};

/// How long the nested desktop is given to come up and be photographed.
pub const PATIENCE: u64 = 180;

/// The program that runs the device's desktop nested on this machine.
///
/// Beside whatever is running, because that is where cargo puts the workspace's
/// programs and a check is one of them. On a machine where it was installed
/// instead, the name on its own is enough.
fn nesting_program() -> PathBuf {
    // A program that cannot say where it is falls back to the bare name, which
    // is the answer on a machine where this was installed rather than built.
    // Worth a line: it is also how a run in a tree comes to use an installed
    // desktop instead of the one just compiled beside it.
    let beside = match std::env::current_exe() {
        Ok(at) => at.parent().map(|at| at.join("console-desktop")),
        Err(fault) => {
            eprintln!("console-stage: where this program is: {fault}");

            None
        }
    };

    beside.filter(|at| at.exists()).unwrap_or_else(|| PathBuf::from("console-desktop"))
}

pub struct Desktop {
    /// What is to be running when the picture is taken.
    open_these: Vec<String>,
    here: PathBuf,
    taken: Option<Picture>,
}

/// Whether a program is on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    /// It is on the path and can be run.
    Yes,
    /// It is not, so whatever wanted it has to say so rather than fail oddly.
    No,
}

impl Default for Desktop {
    fn default() -> Self {
        Desktop::new()
    }
}

impl Desktop {
    pub fn new() -> Self {
        let here = std::env::temp_dir().join(format!("console-desktop-{}", std::process::id()));
        Desktop { open_these: Vec::new(), here, taken: None }
    }

    /// Forget the picture and what was asked for; another check is next.
    pub fn fresh(&mut self) {
        self.open_these.clear();
        self.taken = None;
    }

    /// Have these running when the picture is taken.
    ///
    /// One picture answers every question asked of it, so this has to be said
    /// before anything is looked at. Said afterwards it would quietly be a
    /// statement about a screen that never had them on it, so it refuses.
    pub fn open(&mut self, command: &str) -> Result<(), String> {
        if self.taken.is_some() {
            return Err("the picture has already been taken; open before looking".to_string());
        }

        self.open_these.push(command.to_string());
        Ok(())
    }

    /// One session, one picture, and every question asked of that.
    fn picture(&mut self) -> Result<&Picture, String> {
        if self.taken.is_none() {
            std::fs::create_dir_all(&self.here).map_err(|fault| fault.to_string())?;
            let shot = self.here.join("screen.png");
            let mut nesting = Command::new(nesting_program());
            nesting.arg("shot").arg(&shot);

            for command in &self.open_these {
                nesting.args(["--open", command]);
            }

            let said = nesting.output().map_err(|fault| fault.to_string())?;

            if !shot.exists() {
                let why = String::from_utf8_lossy(&said.stderr);
                let last = why.trim().lines().next_back().unwrap_or_default().to_string();
                return Err(format!("the nested desktop took no picture: {last}"));
            }

            self.taken = Some(Picture::read(&shot)?);
        }

        // Set just above, on the one road that reaches here without one. Said
        // rather than unwrapped, because "it was set a moment ago" is a claim
        // about code that goes on being true until somebody edits the road.
        self.taken.as_ref().ok_or_else(|| "the nested desktop took a picture and then had none".to_string())
    }

    pub fn installed(&self, program: &str) -> Installed {
        let found = Command::new("sh")
            .args(["-c", &format!("command -v {}", crate::device::quoted(program))])
            .output()
            .is_ok_and(|done| done.status.success());

        match found {
            true => Installed::Yes,
            false => Installed::No,
        }
    }

    pub fn colour(&mut self, across: f64, down: f64) -> Result<String, String> {
        let screen = console_screen::Screen::read(
            &std::fs::read_to_string(crate::root().join(console_screen::CONFIG))
                .map_err(|fault| fault.to_string())?,
        )?;
        where_(self.picture()?, across, down, &screen)
    }

    pub fn patch(&mut self, across: f64, down: f64) -> Result<String, String> {
        Ok(self.picture()?.average(across, down, crate::picture::PATCH))
    }

    pub fn background(&mut self) -> Result<String, String> {
        Ok(self.picture()?.commonest())
    }

    pub fn close(&mut self) {
        let _ = std::fs::remove_dir_all(&self.here);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Said afterwards it would quietly be a statement about a screen that
    /// never had them on it.
    #[test]
    fn nothing_can_be_opened_once_the_picture_has_been_taken() {
        let mut desktop = Desktop::new();
        assert!(desktop.open("alacritty").is_ok());
        desktop.taken = None;
        assert!(desktop.open("console-buttons --menu").is_ok());
        assert_eq!(desktop.open_these.len(), 2);
    }

    #[test]
    fn a_fresh_desktop_has_nothing_open_and_nothing_looked_at() {
        let mut desktop = Desktop::new();
        desktop.open("alacritty").expect("something to open");
        desktop.fresh();
        assert!(desktop.open_these.is_empty());
    }
}
