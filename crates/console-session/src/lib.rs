//! Which session has the screen, and how the machine gets to the other one.
//!
//! Two sessions live on this device. The desktop is Hyprland and everything in
//! this repository; Game Mode is Steam, and the machine leaves for it entirely
//! rather than running it in a window. The left Legion button goes there, and
//! the same button held comes back.
//!
//! What is here is the deciding. Whether a press does anything at all, and in
//! what order the steps happen when it does. Neither of those needed a machine
//! to be worked out and neither of them could be asked without one, because
//! both lived in a shell script.
//!
//! `run_each` also times what it runs, because this is the longest wait on the
//! device. A switch usually does not get to write its line: the session going
//! down takes the program writing it with it, somewhere inside
//! `steamos-session-select`. What always lands is `session`/`starting`, which
//! is measured in the session that is coming up, and that is the half somebody
//! is actually sitting there watching.

pub mod reaching;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Session {
    Desktop,
    Game,
}

pub const GAME_TARGET: &str = "gamescope-session.target";

pub const SWITCHER: &str = "/usr/local/bin/steamos-session-select";

impl Session {
    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Session::Desktop => "plasma",
            Session::Game => "gamescope",
        })
    }

    pub fn going(self) -> Result<&'static str, Never> {
        Ok(match self {
            Session::Desktop => "back to the desktop",
            Session::Game => "to game mode",
        })
    }

    pub fn buttons(self) -> Result<Option<&'static str>, Never> {
        Ok(match self {
            Session::Desktop => None,
            Session::Game => Some("game"),
        })
    }
}

pub fn worth_going(now: Session, to: Session) -> Result<Going, Never> {
    Ok(match now != to {
        true => Going::Worth,
        false => Going::Already,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Going {
    Worth,
    Already,
}

pub fn steps(now: Session, to: Session) -> Result<Vec<Vec<String>>, Never> {
    let Ok(worth) = worth_going(now, to);

    match worth {
        Going::Already => return Ok(Vec::new()),
        Going::Worth => {}
    }

    let mut steps = Vec::new();

    let Ok(buttons) = to.buttons();

    match buttons {
        Some(profile) => steps.push(vec!["controller-profile".to_string(), profile.to_string()]),
        None => {}
    }

    let Ok(word) = to.word();

    steps.push(vec![SWITCHER.to_string(), word.to_string()]);

    Ok(steps)
}

pub const HANDED_OVER: [&str; 5] = [
    "WAYLAND_DISPLAY",
    "HYPRLAND_INSTANCE_SIGNATURE",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_RUNTIME_DIR",
];

pub const TARGET: &str = "console.target";

pub fn starting() -> Result<Vec<Vec<String>>, Never> {
    let Ok(mut handing) = Program::Systemctl.argv(&["--user", "import-environment"]);

    handing.extend(HANDED_OVER.iter().map(|name| (*name).to_string()));

    Ok(vec![
        handing,
        vec![SIZE.to_string(), "apply".to_string()],
        vec![
            "systemctl".to_string(),
            "--user".to_string(),
            "restart".to_string(),
            "--no-block".to_string(),
            TARGET.to_string(),
        ],
    ])
}

const SIZE: &str = "/usr/local/bin/console-scale";

use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;

pub fn here(target: &str) -> Result<Session, Never> {
    let Ok(mut systemctl) = Program::Systemctl.command();

    let asked = systemctl.args(["--user", "is-active", "--quiet", target]).status();

    Ok(match asked.map(|how| how.success()) {
        Ok(true) => Session::Game,
        Ok(false) => Session::Desktop,
        Err(_) => Session::Desktop,
    })
}

pub fn run_each(what: &str, steps: &[Vec<String>]) -> Result<(), Never> {
    let Ok(mut waiting) = console_response_times::Waiting::on("session", what);

    for argv in steps {
        let Some((program, rest)) = argv.split_first() else { continue };

        let mut starting = Command::new(program);
        starting.args(rest);
        let Ok(()) = console_response_times::not_a_press(&mut starting);

        match starting.status() {
            Ok(how) if how.success() => {
                let Ok(plainly) = plainly(program);
                let Ok(()) = waiting.mark(&plainly);
            }
            Ok(how) => {
                eprintln!("{program} said {how}");
                return Ok(());
            }
            Err(why) => {
                eprintln!("no {program} to run: {why}");
                return Ok(());
            }
        }
    }

    let Ok(()) = waiting.done();
    let Ok(()) = console_response_times::settled();

    Ok(())
}

pub fn plainly(program: &str) -> Result<String, Never> {
    Ok(match program.rsplit_once('/') {
        Some((_, name)) => name.to_string(),
        None => program.to_string(),
    })
}

pub fn run(now: Session, to: Session) -> Result<(), Never> {
    let Ok(going) = to.going();
    let Ok(steps) = steps(now, to);
    let Ok(()) = run_each(going, &steps);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_is_written_down_under_the_name_of_the_program_and_not_its_path() {
        let Ok(path) = plainly("/usr/local/bin/steamos-session-select");
        let Ok(name) = plainly("systemctl");

        assert_eq!(path, "steamos-session-select");
        assert_eq!(name, "systemctl");
    }

    #[test]
    fn each_way_across_says_which_way_it_was_going() {
        let Ok(back) = Session::Desktop.going();
        let Ok(away) = Session::Game.going();

        assert_ne!(back, away);
    }

    #[test]
    fn neither_way_to_a_session_acts_when_the_machine_is_already_in_it() {
        let Ok(in_game) = worth_going(Session::Game, Session::Game);
        let Ok(on_the_desktop) = worth_going(Session::Desktop, Session::Desktop);
        let Ok(game_steps) = steps(Session::Game, Session::Game);
        let Ok(desktop_steps) = steps(Session::Desktop, Session::Desktop);

        assert_eq!(in_game, Going::Already);
        assert_eq!(on_the_desktop, Going::Already);
        assert_eq!(game_steps, Vec::<Vec<String>>::new());
        assert_eq!(desktop_steps, Vec::<Vec<String>>::new());
    }

    #[test]
    fn the_pad_is_a_gamepad_again_before_steam_is_asked_for() {
        let Ok(steps) = steps(Session::Desktop, Session::Game);

        assert_eq!(steps[0], ["controller-profile", "game"]);
        assert_eq!(steps[1], [SWITCHER, "gamescope"]);
    }

    #[test]
    fn coming_back_does_not_reach_for_the_controller() {
        let Ok(steps) = steps(Session::Game, Session::Desktop);

        assert_eq!(steps, vec![vec![SWITCHER, "plasma"]]);
    }

    fn the_desktop() -> (usize, Vec<String>) {
        let Ok(starting) = starting();

        starting
            .into_iter()
            .enumerate()
            .find(|(_, step)| step.contains(&TARGET.to_string()))
            .expect("nothing in the session starts the desktop")
    }

    #[test]
    fn the_compositor_is_handed_over_before_anything_is_started() {
        let Ok(starting) = starting();

        assert_eq!(starting[0][2], "import-environment");
        assert!(starting[0].contains(&"HYPRLAND_INSTANCE_SIGNATURE".to_string()));
        assert!(the_desktop().0 > 0, "the desktop starts before it is told where it is");
    }

    #[test]
    fn the_desktop_is_restarted_and_never_merely_started() {
        let (_, step) = the_desktop();
        assert!(step.contains(&"restart".to_string()));
        assert!(!step.contains(&"start".to_string()));
    }

    #[test]
    fn the_screen_is_put_back_to_its_size_before_the_desktop_is_started() {
        let Ok(starting) = starting();
        let at = starting
            .iter()
            .position(|step| step[0] == SIZE)
            .expect("nothing puts the screen back to the size it was left at");
        assert_eq!(starting[at][1], "apply");
        assert!(at < the_desktop().0, "the desktop is drawn before the screen is the right size");
    }
}
