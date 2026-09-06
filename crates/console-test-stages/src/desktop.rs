//! The device's own desktop, nested on this machine, and looked at.
//!
//! What this can answer that nothing else can is what colour the screen is. A
//! service being active proves nothing about whether it is doing its job: the
//! wallpaper on the device did not paint for days because hyprpaper read a
//! config format it no longer understood, painted nothing, and reported
//! success. Nothing was in a failed state. The screen was the wrong colour.
//!
//! It presses with the pointer. A finger needs /dev/uinput, and uinput is this
//! machine's -- a touchscreen made here puts its finger on the laptop, not
//! inside the picture, because the nested compositor is a client of this one
//! and hears only what this one forwards. The pointer has a way in that a
//! finger does not: `console-point` asks the nested compositor for a pointer of
//! its own, over the socket, and that pointer is inside the picture. So
//! everything the pointer does -- hovering, clicking, the wheel -- can be
//! pressed here, and a finger is still the device's tier.
//!
//! Which is not a small thing to have gained. Before it, the only pressing
//! anywhere was on somebody's actual handheld: a check for what the pointer
//! does could be written only for a machine that has to be plugged in, awake
//! and reachable, so mostly it was not written at all.

use std::path::PathBuf;
use std::process::Command;

use console_external_programs::Program;
use console_never::Never;
use console_number_conversion::{Float, fitted};

use crate::picture::{Picture, where_};

pub const PATIENCE: u64 = 180;

const DRAWN: f64 = 6.0;

const HAND: f64 = 0.6;

const BETWEEN: f64 = 0.5;

const AFTER: f64 = 1.5;

fn pressing_hand() -> Result<(), String> {
    let Ok(nesting) = nesting_program();
    let beside = nesting.parent().map(|at| at.join("console-point"));

    match beside {
        Some(at) if at.is_file() => Ok(()),
        Some(at) => Err(format!(
            "{} is not there, so nothing would be pressed: cargo build -p console-virtual-pointer",
            at.display()
        )),
        None => Err("console-point is not beside console-desktop".to_string()),
    }
}

fn nesting_program() -> Result<PathBuf, Never> {
    let beside = match std::env::current_exe() {
        Ok(at) => at.parent().map(|at| at.join("console-desktop")),
        Err(fault) => {
            eprintln!("console-test-stages: where this program is: {fault}");

            None
        }
    };

    Ok(beside.filter(|at| at.exists()).unwrap_or_else(|| PathBuf::from("console-desktop")))
}

pub struct Desktop {
    open_these: Vec<String>,
    press_these: Vec<String>,
    here: PathBuf,
    taken: Option<Picture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Installed {
    Yes,
    No,
}

impl Default for Desktop {
    fn default() -> Self {
        let Ok(desktop) = Desktop::new();

        desktop
    }
}

impl Desktop {
    pub fn new() -> Result<Self, Never> {
        let here = std::env::temp_dir().join(format!("console-desktop-{}", std::process::id()));

        Ok(Desktop { open_these: Vec::new(), press_these: Vec::new(), here, taken: None })
    }

    pub fn fresh(&mut self) -> Result<(), Never> {
        self.open_these.clear();
        self.press_these.clear();
        self.taken = None;

        Ok(())
    }

    pub fn open(&mut self, command: &str) -> Result<(), String> {
        match self.taken.is_some() {
            true => {
                return Err("the picture has already been taken; open before looking".to_string());
            }
            false => {},
        }

        self.open_these.push(command.to_string());
        Ok(())
    }

    pub fn point(&mut self, at: (u32, u32)) -> Result<(), String> {
        self.pointing(None, at, "")
    }

    pub fn click(&mut self, at: (u32, u32)) -> Result<(), String> {
        self.pointing(None, at, " --click")
    }

    pub fn scroll(&mut self, at: (u32, u32), notches: i32) -> Result<(), String> {
        self.pointing(None, at, &format!(" --scroll {notches}"))
    }

    pub fn point_in(&mut self, namespace: &str, at: (u32, u32)) -> Result<(), String> {
        self.pointing(Some(namespace), at, "")
    }

    pub fn click_in(&mut self, namespace: &str, at: (u32, u32)) -> Result<(), String> {
        self.pointing(Some(namespace), at, " --click")
    }

    pub fn scroll_in(
        &mut self,
        namespace: &str,
        at: (u32, u32),
        notches: i32,
    ) -> Result<(), String> {
        self.pointing(Some(namespace), at, &format!(" --scroll {notches}"))
    }

    fn pointing(
        &mut self,
        inside: Option<&str>,
        at: (u32, u32),
        doing: &str,
    ) -> Result<(), String> {
        let within = match inside {
            Some(namespace) => format!("--in {namespace} "),
            None => String::new(),
        };

        self.press(format!("console-point {within}{} {}{doing}", at.0, at.1))
    }

    fn press(&mut self, command: String) -> Result<(), String> {
        match self.taken {
            Some(_) => Err("the picture has already been taken; press before looking".to_string()),
            None => {
                self.press_these.push(command);

                Ok(())
            },
        }
    }

    fn pressing(&self) -> Result<Option<(String, f64)>, Never> {
        let Some((first, rest)) = self.press_these.split_first() else { return Ok(None) };

        let mut script = format!("sleep {DRAWN}; {first}");

        for command in rest {
            script.push_str(&format!("; sleep {BETWEEN}; {command}"));
        }

        let Ok(pressed) = fitted::<usize, u64>(self.press_these.len());
        let Ok(many) = pressed.float();

        let waited = DRAWN + many * (HAND + BETWEEN) + AFTER;

        Ok(Some((script, waited)))
    }

    fn picture(&mut self) -> Result<&Picture, String> {
        match self.taken.is_none() {
            true => {
                std::fs::create_dir_all(&self.here).map_err(|fault| fault.to_string())?;
                let shot = self.here.join("screen.png");
                let Ok(program) = nesting_program();
                let mut nesting = Command::new(program);
                nesting.arg("shot").arg(&shot);

                for command in &self.open_these {
                    nesting.args(["--open", command]);
                }

                let Ok(pressing) = self.pressing();

                match pressing {
                    Some((script, waited)) => {
                        pressing_hand()?;

                        nesting.args(["--open", &script]);
                        nesting.args(["--settle", &format!("{waited:.1}")]);
                    }
                    None => {},
                }

                let said = nesting.output().map_err(|fault| fault.to_string())?;

                match shot.exists() {
                    true => {},
                    false => {
                        let why = String::from_utf8_lossy(&said.stderr);
                        let last =
                            why.trim().lines().next_back().unwrap_or_default().to_string();
                        return Err(format!("the nested desktop took no picture: {last}"));
                    }
                }

                let picture = Picture::read(&shot)?;

                self.taken = Some(picture);
            }
            false => {},
        }

        self.taken.as_ref().ok_or_else(|| "the nested desktop took a picture and then had none".to_string())
    }

    pub fn installed(&self, program: &str) -> Result<Installed, Never> {
        let Ok(quoted) = crate::device::quoted(program);
        let Ok(mut asking) = Program::Sh.command();

        let found = asking
            .args(["-c", &format!("command -v {quoted}")])
            .output()
            .is_ok_and(|done| done.status.success());

        Ok(match found {
            true => Installed::Yes,
            false => Installed::No,
        })
    }

    pub fn colour(&mut self, across: f64, down: f64) -> Result<String, String> {
        let screen = crate::screen()?;
        let picture = self.picture()?;

        where_(picture, across, down, &screen)
    }

    pub fn patch(&mut self, across: f64, down: f64) -> Result<String, String> {
        let picture = self.picture()?;
        let Ok(average) = picture.average(across, down, crate::picture::PATCH);

        Ok(average)
    }

    pub fn background(&mut self) -> Result<String, String> {
        let picture = self.picture()?;
        let Ok(commonest) = picture.commonest();

        Ok(commonest)
    }

    pub fn close(&mut self) -> Result<(), Never> {
        let _ = std::fs::remove_dir_all(&self.here);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new() -> Desktop {
        let Ok(desktop) = Desktop::new();

        desktop
    }

    #[test]
    fn nothing_can_be_opened_once_the_picture_has_been_taken() {
        let mut desktop = new();
        assert!(desktop.open("alacritty").is_ok());
        desktop.taken = None;
        assert!(desktop.open("console-buttons --menu").is_ok());
        assert_eq!(desktop.open_these.len(), 2);
    }

    #[test]
    fn a_fresh_desktop_has_nothing_open_and_nothing_looked_at() {
        let mut desktop = new();
        desktop.open("alacritty").expect("something to open");
        desktop.point((10, 20)).expect("somewhere to point");
        desktop.fresh();
        assert!(desktop.open_these.is_empty());
        assert!(desktop.press_these.is_empty());
    }

    #[test]
    fn nothing_pressed_is_a_session_that_is_not_kept_open_for_it() {
        let Ok(pressing) = new().pressing();

        assert_eq!(pressing, None);
    }

    #[test]
    fn the_presses_are_one_script_in_the_order_they_were_asked_for() {
        let mut desktop = new();
        desktop.point((10, 20)).expect("somewhere to point");
        desktop.click((30, 40)).expect("somewhere to click");
        desktop.scroll((30, 40), -2).expect("somewhere to scroll");

        let Ok(pressing) = desktop.pressing();
        let (script, waited) = pressing.expect("a script");

        assert_eq!(
            script,
            format!(
                "sleep {DRAWN}; console-point 10 20; sleep {BETWEEN}; console-point 30 40 --click; \
                 sleep {BETWEEN}; console-point 30 40 --scroll -2"
            )
        );
        assert!(waited > DRAWN + AFTER, "the session goes before the last press: {waited}");
    }
}
