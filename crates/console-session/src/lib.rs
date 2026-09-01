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

/// One of the two sessions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Session {
    Desktop,
    Game,
}

/// The unit that is active while Game Mode has the screen.
pub const GAME_TARGET: &str = "gamescope-session.target";

/// What the session switcher is called, and the word each session answers to.
///
/// SteamOS's own script, carried unchanged. The names are its, not ours.
pub const SWITCHER: &str = "/usr/local/bin/steamos-session-select";

impl Session {
    /// The word `steamos-session-select` knows this session by.
    pub fn word(self) -> &'static str {
        match self {
            Session::Desktop => "plasma",
            Session::Game => "gamescope",
        }
    }

    /// What the controller is set to on the way here.
    ///
    /// Only on the way to Game Mode. Steam and the games under it need the pad
    /// itself rather than every button routed to the desktop's daemon.
    ///
    /// Nothing is set on the way back: loading the router is the first thing
    /// the desktop's own controller daemon does when its target starts, and
    /// doing it here as well would take the pad away from Steam on the way out
    /// for no reason.
    pub fn buttons(self) -> Option<&'static str> {
        match self {
            Session::Desktop => None,
            Session::Game => Some("game"),
        }
    }
}

/// Whether going somewhere is anything at all.
///
/// Nothing to do when the machine is already there, and both directions keep
/// the rule. That is not a hypothetical: the desktop's own controller daemon
/// has been seen running through a whole Game Mode session, and it answers the
/// left Legion button by asking for Game Mode. Without this, one press over
/// there asks Steam to shut down, waits ten seconds for it, and then fails to
/// leave a compositor that is not running.
///
/// It is also what makes a button held a moment too long harmless. The session
/// takes a while to go, the hold is acted on once, and a second call arriving
/// from anywhere else finds the first one has already happened and stops.
pub fn worth_going(now: Session, to: Session) -> bool {
    now != to
}

/// The steps, in the order they happen, or nothing if the machine is there.
pub fn steps(now: Session, to: Session) -> Vec<Vec<String>> {
    if !worth_going(now, to) {
        return Vec::new();
    }
    let mut steps = Vec::new();
    if let Some(profile) = to.buttons() {
        steps.push(vec!["controller-profile".to_string(), profile.to_string()]);
    }
    steps.push(vec![SWITCHER.to_string(), to.word().to_string()]);
    steps
}

/// What the session hands to systemd before the desktop is started.
///
/// Everything that makes up this desktop is a user service rather than
/// something the compositor launches and forgets, so one place decides what
/// runs, it is the same after every reboot, and anything that dies comes back
/// by itself. What those services cannot work out for themselves is where the
/// compositor is, which is what these say.
pub const HANDED_OVER: [&str; 5] = [
    "WAYLAND_DISPLAY",
    "HYPRLAND_INSTANCE_SIGNATURE",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "XDG_RUNTIME_DIR",
];

/// The desktop's own target, which the compositor starts.
pub const TARGET: &str = "console.target";

/// Bringing the desktop up, in the order it happens.
///
/// Restarted rather than started. These services outlive the compositor, and
/// what they were told about it on the way in is a socket a new one does not
/// answer on: a desktop that came back would be driven by services still
/// talking to the one before it.
pub fn starting() -> Vec<Vec<String>> {
    let mut handing = vec!["systemctl".to_string(), "--user".to_string(), "import-environment".to_string()];
    handing.extend(HANDED_OVER.iter().map(|name| (*name).to_string()));
    vec![
        handing,
        vec![
            "systemctl".to_string(),
            "--user".to_string(),
            "restart".to_string(),
            "--no-block".to_string(),
            TARGET.to_string(),
        ],
    ]
}

// ---------------------------------------------------------------- the doing

use std::process::Command;

/// Which session has the screen, asked of systemd.
///
/// The one target that is active only over there. Anything else -- and a
/// machine where systemd cannot be asked at all -- is taken as the desktop,
/// because the desktop is where this is running from.
pub fn here(target: &str) -> Session {
    let asked = Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", target])
        .status();
    match asked.map(|how| how.success()) {
        Ok(true) => Session::Game,
        _ => Session::Desktop,
    }
}

/// Do them, in order, stopping at the first that will not run.
pub fn run_each(steps: &[Vec<String>]) {
    for argv in steps {
        let Some((program, rest)) = argv.split_first() else { continue };
        match Command::new(program).args(rest).status() {
            Ok(how) if how.success() => (),
            Ok(how) => {
                eprintln!("{program} said {how}");
                return;
            }
            Err(why) => {
                eprintln!("no {program} to run: {why}");
                return;
            }
        }
    }
}

/// Go from one session to the other, if that is anything at all.
pub fn run(now: Session, to: Session) {
    run_each(&steps(now, to));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Neither way to a session does anything when the machine is already in
    /// it. This was two shell scripts asserting the same rule in opposite
    /// directions, checked by a test that read both of them for a string.
    #[test]
    fn neither_way_to_a_session_acts_when_the_machine_is_already_in_it() {
        assert!(!worth_going(Session::Game, Session::Game));
        assert!(!worth_going(Session::Desktop, Session::Desktop));
        assert_eq!(steps(Session::Game, Session::Game), Vec::<Vec<String>>::new());
        assert_eq!(steps(Session::Desktop, Session::Desktop), Vec::<Vec<String>>::new());
    }

    /// The controller goes back to being a gamepad before Steam has the
    /// screen, not after: a game that arrives to a pad still pretending to be
    /// a mouse is a game nobody can press A in.
    #[test]
    fn the_pad_is_a_gamepad_again_before_steam_is_asked_for() {
        let steps = steps(Session::Desktop, Session::Game);
        assert_eq!(steps[0], ["controller-profile", "game"]);
        assert_eq!(steps[1], [SWITCHER, "gamescope"]);
    }

    /// Nothing is set on the way back. The desktop's controller daemon does it
    /// when its target starts, and doing it here as well would take the pad
    /// away from Steam on the way out for no reason.
    #[test]
    fn coming_back_does_not_reach_for_the_controller() {
        assert_eq!(steps(Session::Game, Session::Desktop), vec![vec![SWITCHER, "plasma"]]);
    }

    /// The services are told where the compositor is before they are started,
    /// or they come up talking to nothing.
    #[test]
    fn the_compositor_is_handed_over_before_anything_is_started() {
        let starting = starting();
        assert_eq!(starting[0][2], "import-environment");
        assert!(starting[0].contains(&"HYPRLAND_INSTANCE_SIGNATURE".to_string()));
        assert!(starting[1].contains(&"restart".to_string()));
    }

    /// Started rather than restarted, the desktop that came back would be
    /// driven by services still talking to the compositor before it.
    #[test]
    fn the_desktop_is_restarted_and_never_merely_started() {
        assert!(starting()[1].contains(&"restart".to_string()));
        assert!(!starting()[1].contains(&"start".to_string()));
    }
}
