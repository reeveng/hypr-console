//! What this desktop can do, and what each of those is bound to.
//!
//! One table, and now it really is one. What a button meant used to be split
//! between an InputPlumber profile, this daemon, and a third place for the one
//! button neither of them knew about; then between two profiles, because a
//! button meant one thing on the desktop and another with a chooser up.
//! Nothing could be asked "what does X do" and answer.
//!
//! So the profile stopped saying what a button means and started saying only
//! what it is -- `console_pad::routing` -- and everything a press comes to is
//! decided here: the job, when it applies, and what has to be held down with
//! it. The daemon reads it, the setup screen writes to it, and the guide reads
//! it out loud. There is no second copy.
//!
//! What is in the table is the default and nothing more. Somebody who moves a
//! job onto another button writes that in `~/.config/console/buttons.toml`,
//! and only what they moved is in there: a machine nobody has touched has an
//! empty file and the whole of its answer here.

use evdev::KeyCode;

use console_pad::jobs::{ALONE, Binding, Jobs, Layer};

use crate::doing::{Doing, Out};
use crate::mode::Mode;

/// What a job is, said as the job and not as the program that does it.
///
/// The guide reads these. A device with different hardware maps its own
/// buttons onto them. Nothing here names a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    /// The menu.
    Menu,
    /// Take what is said and type it.
    Dictate,
    /// Put away whatever is up.
    PutAway,
    Screenshot,
    Settings,
    Brighter,
    Dimmer,
    Louder,
    Quieter,
    /// Leave the desktop for Steam.
    GameMode,
    Browser,
    /// The guide to what every button does.
    Guide,
    /// The on-screen keyboard, up and down.
    ///
    /// This desktop's own now. It used to be the one job nothing here carried
    /// out: X passed through to the pad as `North`, and the on-screen
    /// keyboard's own fork read the pad and toggled itself on it, so what X
    /// did was two programs happening to have one device open. Under the
    /// router X arrives as a key that fork cannot see, and the toggle is a
    /// signal like any other -- which is what `osk` was always sending.
    Keyboard,
    /// The place before or after this one.
    Workspace(i32),
    /// The same, with the window brought along.
    Carry(i32),
    /// The pointer's own button, and the one that asks for more.
    Click,
    MoreOptions,
    /// Back, and out of whatever is up.
    Back,
    /// The four ways a list is walked.
    Up,
    Down,
    Left,
    Right,
    /// Take the row the highlight is on.
    Choose,
    /// What else can be done with the row the highlight is on.
    More,
    /// The tab to one side or the other.
    Tab(i32),
}

/// When a job applies.
///
/// The one thing the old pair of profiles said that a single profile cannot.
/// A is a click on the desktop and takes the highlighted row with a chooser
/// up; the shoulders move between workspaces on the desktop and between tabs
/// in a chooser. That difference used to be two files that had to be swapped
/// on the way in and out of every menu, which destroyed the pad and built
/// another every time -- the fault half the comments in this crate are about.
/// It is a column now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    /// Wherever you are. Most of them: a button on the back of the machine
    /// means one thing everywhere, which is the promise the profiles made in
    /// their own comments and could only half keep.
    Anywhere,
    OnTheDesktop,
    WithAChooserUp,
}

impl When {
    /// Whether this is one of the jobs in front of you now.
    pub fn suits(self, mode: Mode) -> bool {
        match self {
            When::Anywhere => true,
            When::OnTheDesktop => mode == Mode::Desktop,
            When::WithAChooserUp => mode == Mode::Tabs,
        }
    }

    /// Whether this is more particular than that, where both suit.
    ///
    /// A job for the desktop beats a job for anywhere on the same button. That
    /// is not a rule anything in the table needs today -- nothing is bound
    /// twice -- and it is the answer to what a person's own file can ask for,
    /// which is a job put on a button something general already has.
    fn beats(self, other: Self) -> bool {
        self != When::Anywhere && other == When::Anywhere
    }
}

/// One job: what it is, when it applies, and where this desktop puts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Job {
    /// What it is called in the file somebody writes.
    pub slug: &'static str,
    pub what: What,
    pub when: When,
    /// Where this desktop puts it, until somebody says otherwise.
    ///
    /// More than one only where two buttons genuinely do one job: the button
    /// with a keyboard drawn on it does what X does, and always has.
    pub bound: &'static [(Layer, &'static str)],
}

/// Nothing held, said the way the table says it.
const ON: Layer = ALONE;
/// The left trigger held, which is where the second thing a button does lives.
const L2: Layer = Layer { l2: true, r2: false };

/// Everything this desktop does, and what does it.
///
/// One job, one row. Two rows may name one button where they cannot both
/// apply -- A is a click on the desktop and takes the row in a chooser -- and
/// `nothing_is_bound_twice_in_one_place` is what holds that to what it says.
pub const JOBS: [Job; 28] = [
    // The back of the machine, and the front's three named buttons. These mean
    // the same thing wherever you are: a paddle that meant one thing on the
    // desktop and another with a menu up would mean the wrong one for the beat
    // between the screen changing and the pad hearing about it, and a thumb is
    // quicker than that beat.
    Job { slug: "menu", what: What::Menu, when: When::Anywhere, bound: &[(ON, "left-paddle-top")] },
    Job {
        slug: "dictate",
        what: What::Dictate,
        when: When::Anywhere,
        bound: &[(ON, "left-paddle-bottom")],
    },
    Job {
        slug: "put-away",
        what: What::PutAway,
        when: When::Anywhere,
        bound: &[(ON, "right-paddle-top")],
    },
    Job {
        slug: "settings",
        what: What::Settings,
        when: When::Anywhere,
        bound: &[(ON, "legion-right")],
    },
    Job { slug: "guide", what: What::Guide, when: When::Anywhere, bound: &[(ON, "menu")] },
    // Two buttons, one job, and the only row here with two. The button with a
    // keyboard drawn on it does what X does; a machine where the button named
    // after the keyboard was the one that could not open it would be a machine
    // that had stopped making sense.
    Job {
        slug: "keyboard",
        what: What::Keyboard,
        when: When::Anywhere,
        bound: &[(ON, "x"), (ON, "keyboard")],
    },
    // The second thing a button does, which is the whole of what a layer is
    // for. The front of the device is for what you reach for without thinking,
    // and how bright the screen is is not that -- nor is a screenshot, which
    // the paddle behind it taught the hard way: a button a hand rests on is a
    // button that goes off while nobody is asking for anything.
    Job {
        slug: "screenshot",
        what: What::Screenshot,
        when: When::Anywhere,
        bound: &[(L2, "right-paddle-bottom")],
    },
    Job {
        slug: "brighter",
        what: What::Brighter,
        when: When::Anywhere,
        bound: &[(L2, "dpad-right")],
    },
    Job { slug: "dimmer", what: What::Dimmer, when: When::Anywhere, bound: &[(L2, "dpad-left")] },
    Job { slug: "louder", what: What::Louder, when: When::Anywhere, bound: &[(L2, "dpad-up")] },
    Job { slug: "quieter", what: What::Quieter, when: When::Anywhere, bound: &[(L2, "dpad-down")] },
    // The keys a list is walked with. These are the daemon's now: the profile
    // used to turn the d-pad into arrows and B into Escape, and a profile that
    // does that is a profile deciding what a button means.
    Job { slug: "back", what: What::Back, when: When::Anywhere, bound: &[(ON, "b")] },
    Job { slug: "up", what: What::Up, when: When::Anywhere, bound: &[(ON, "dpad-up")] },
    Job { slug: "down", what: What::Down, when: When::Anywhere, bound: &[(ON, "dpad-down")] },
    Job { slug: "left", what: What::Left, when: When::Anywhere, bound: &[(ON, "dpad-left")] },
    Job { slug: "right", what: What::Right, when: When::Anywhere, bound: &[(ON, "dpad-right")] },
    // The desktop, where there is a pointer to click with and somewhere to go.
    Job { slug: "click", what: What::Click, when: When::OnTheDesktop, bound: &[(ON, "a")] },
    Job {
        slug: "more-options",
        what: What::MoreOptions,
        when: When::OnTheDesktop,
        bound: &[(ON, "y")],
    },
    Job {
        slug: "game-mode",
        what: What::GameMode,
        when: When::OnTheDesktop,
        bound: &[(ON, "legion-left")],
    },
    Job {
        slug: "browser",
        what: What::Browser,
        when: When::OnTheDesktop,
        bound: &[(ON, "view")],
    },
    Job {
        slug: "workspace-next",
        what: What::Workspace(1),
        when: When::OnTheDesktop,
        bound: &[(ON, "r1")],
    },
    Job {
        slug: "workspace-previous",
        what: What::Workspace(-1),
        when: When::OnTheDesktop,
        bound: &[(ON, "l1")],
    },
    Job {
        slug: "carry-next",
        what: What::Carry(1),
        when: When::OnTheDesktop,
        bound: &[(L2, "r1")],
    },
    Job {
        slug: "carry-previous",
        what: What::Carry(-1),
        when: When::OnTheDesktop,
        bound: &[(L2, "l1")],
    },
    // A chooser, which is driven by the highlight rather than by the pointer.
    // A takes the row it is on rather than clicking whatever the pointer
    // happens to be over, and the shoulders carry the panel between its tabs
    // rather than carrying you between workspaces.
    Job { slug: "choose", what: What::Choose, when: When::WithAChooserUp, bound: &[(ON, "a")] },
    Job { slug: "more", what: What::More, when: When::WithAChooserUp, bound: &[(ON, "y")] },
    Job {
        slug: "tab-right",
        what: What::Tab(1),
        when: When::WithAChooserUp,
        bound: &[(ON, "r1")],
    },
    Job {
        slug: "tab-left",
        what: What::Tab(-1),
        when: When::WithAChooserUp,
        bound: &[(ON, "l1")],
    },
];

/// The job that goes by that name, if one does.
pub fn job(slug: &str) -> Option<&'static Job> {
    JOBS.iter().find(|job| job.slug == slug)
}

impl What {
    /// What it is called, which is what the guide says and what a row on the
    /// setup screen says.
    pub fn says(self) -> &'static str {
        match self {
            What::Menu => "the menu",
            What::Dictate => "take what is said and type it",
            What::PutAway => "put away whatever is up",
            What::Screenshot => "a screenshot",
            What::Settings => "the settings",
            What::Brighter => "screen brighter",
            What::Dimmer => "screen dimmer",
            What::Louder => "louder",
            What::Quieter => "quieter",
            What::GameMode => "leave for Steam",
            What::Browser => "the browser",
            What::Guide => "what every button does",
            What::Keyboard => "show or hide the keyboard",
            What::Workspace(-1) => "the place before this one",
            What::Workspace(_) => "the place after this one",
            What::Carry(-1) => "carry the window to the place before",
            What::Carry(_) => "carry the window to the place after",
            What::Click => "click",
            What::MoreOptions => "right click, more options",
            What::Back => "back, and out of what is up",
            What::Up => "move up",
            What::Down => "move down",
            What::Left => "move left",
            What::Right => "move right",
            What::Choose => "choose the row you are on",
            What::More => "what else can be done with a row",
            What::Tab(-1) => "the tab to the left",
            What::Tab(_) => "the tab to the right",
        }
    }

    /// What is done about it, on the way down or on the way back up.
    ///
    /// Two kinds, and the difference is whether the press is a press or a
    /// hold. Starting something happens once, when the button goes down.
    /// Sending a key or a mouse button follows the button exactly: held is
    /// held, which is what makes a drag a drag and what lets the compositor
    /// repeat an arrow while a thumb stays on the d-pad.
    pub fn does(self, down: bool) -> Option<Doing> {
        match self {
            What::Click => Some(pressed(KeyCode::BTN_LEFT, down)),
            What::MoreOptions => Some(pressed(KeyCode::BTN_RIGHT, down)),
            What::Back => Some(pressed(KeyCode::KEY_ESC, down)),
            What::Up => Some(pressed(KeyCode::KEY_UP, down)),
            What::Down => Some(pressed(KeyCode::KEY_DOWN, down)),
            What::Left => Some(pressed(KeyCode::KEY_LEFT, down)),
            What::Right => Some(pressed(KeyCode::KEY_RIGHT, down)),
            What::Choose => Some(pressed(KeyCode::KEY_ENTER, down)),
            What::More => Some(pressed(KeyCode::KEY_F18, down)),
            What::Tab(-1) => Some(pressed(KeyCode::KEY_PAGEUP, down)),
            What::Tab(_) => Some(pressed(KeyCode::KEY_PAGEDOWN, down)),
            _ if !down => None,
            What::Menu => Some(Doing::run(&["launcher", "--keep"])),
            What::Dictate => Some(Doing::run(&["dictate"])),
            What::PutAway => Some(Doing::run(&["put-away"])),
            What::Screenshot => Some(Doing::run(&["/usr/local/bin/console-screenshot"])),
            What::Settings => Some(Doing::run(&["settings-panel"])),
            What::Brighter => Some(Doing::run(&["/usr/local/bin/console-brightness", "up"])),
            What::Dimmer => Some(Doing::run(&["/usr/local/bin/console-brightness", "down"])),
            What::Louder => Some(Doing::run(&["/usr/local/bin/console-volume", "up"])),
            What::Quieter => Some(Doing::run(&["/usr/local/bin/console-volume", "down"])),
            What::GameMode => Some(Doing::run(&["game-mode"])),
            What::Browser => Some(Doing::run(&["/usr/local/bin/console-browser"])),
            What::Guide => Some(Doing::run(&["/usr/local/bin/console-buttons", "--menu"])),
            What::Keyboard => Some(Doing::run(&["osk"])),
            What::Workspace(step) => Some(Doing::workspace(&format!("{step:+}"), false)),
            What::Carry(step) => Some(Doing::workspace(&format!("{step:+}"), true)),
        }
    }
}

/// One key or one mouse button, following the button that asked for it.
fn pressed(code: KeyCode, down: bool) -> Doing {
    Doing::Frame(vec![Out::key(code.0, i32::from(down))])
}

/// Every key and mouse button this desktop can send.
///
/// What the daemon's own device has to be built with. A device that does not
/// claim a key cannot send it: the press goes nowhere, silently, and the
/// button reads as dead. So the list is read out of the table rather than
/// written beside it -- a job given a new key is a job that works.
pub fn sends() -> Vec<KeyCode> {
    let mut every: Vec<KeyCode> = JOBS
        .iter()
        .filter_map(|job| match job.what.does(true) {
            Some(Doing::Frame(frame)) => frame.first().map(|out| KeyCode(out.code)),
            _ => None,
        })
        .collect();
    every.sort_unstable_by_key(|key| key.0);
    every.dedup();
    every
}

/// Every job, and what plays it on this machine.
///
/// The defaults with somebody's own answers over the top. Built once and asked
/// many times: what a press comes to is this question, and reading a file for
/// every press would be reading a file for every press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    bound: Vec<(&'static str, Vec<Binding>)>,
}

/// A table nobody has said anything about, which is this desktop's own.
///
/// Not an empty one. A daemon holding an empty table is a machine where every
/// button does nothing, and the way to get there is for a file to be missing --
/// which is the ordinary state of a machine nobody has touched.
impl Default for Table {
    fn default() -> Self {
        Table::ours()
    }
}

impl Table {
    /// The table as this desktop ships it.
    pub fn ours() -> Self {
        Table::of(&Jobs::none())
    }

    /// The same, with what somebody said about their own machine over it.
    ///
    /// A job named in the file that this desktop has never heard of is left
    /// alone rather than argued with. It is what a file written by a newer
    /// version of this desktop looks like to an older one, and the answer to
    /// that is to carry on doing the other twenty-seven jobs.
    pub fn of(said: &Jobs) -> Self {
        Table {
            bound: JOBS
                .iter()
                .map(|job| {
                    let bound = match said.bound(job.slug) {
                        Some(moved) => moved.to_vec(),
                        None => job
                            .bound
                            .iter()
                            .map(|(layer, button)| Binding::held(*layer, (*button).to_string()))
                            .collect(),
                    };
                    (job.slug, bound)
                })
                .collect(),
        }
    }

    /// What plays this job here.
    pub fn bindings(&self, slug: &str) -> &[Binding] {
        self.bound
            .iter()
            .find(|(named, _)| *named == slug)
            .map_or(&[], |(_, bound)| bound.as_slice())
    }

    /// Every job and what plays it, which is what the setup screen draws and
    /// what a move is worked out against.
    pub fn every(&self) -> impl Iterator<Item = (&'static Job, &[Binding])> {
        self.bound.iter().filter_map(|(slug, bound)| {
            job(slug).map(|job| (job, bound.as_slice()))
        })
    }

    /// What a press comes to: the button, what is held with it, and what is in
    /// front of you.
    ///
    /// The layer is matched exactly first, and then the button on its own.
    /// Exactly, because both triggers held is a layer of its own and not
    /// either of them; then on its own, because holding a trigger is not a
    /// second machine -- a button with no second job goes on doing its first
    /// one while L2 is down, which is what makes the menu paddle the menu
    /// paddle wherever your fingers happen to be.
    pub fn what(&self, button: &str, layer: Layer, mode: Mode) -> Option<&'static Job> {
        self.matching(button, layer, mode)
            .or_else(|| layer.held().then(|| self.matching(button, ALONE, mode)).flatten())
    }

    fn matching(&self, button: &str, layer: Layer, mode: Mode) -> Option<&'static Job> {
        let mut found: Option<&'static Job> = None;
        for (job, bound) in self.every() {
            if !job.when.suits(mode) {
                continue;
            }
            if !bound.iter().any(|one| one.layer == layer && one.button == button) {
                continue;
            }
            found = match found {
                Some(already) if already.when.beats(job.when) => Some(already),
                _ => Some(job),
            };
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_pad::vocabulary::button_name;

    fn ours() -> Table {
        Table::ours()
    }

    /// One job, one row. A job written down twice is a job whose two rows
    /// disagree the first time one of them is changed.
    #[test]
    fn nothing_is_written_down_twice() {
        let mut every: Vec<&str> = JOBS.iter().map(|job| job.slug).collect();
        let many = every.len();
        every.sort_unstable();
        every.dedup();
        assert_eq!(every.len(), many);
    }

    /// And no two jobs are on one button in one place. Two would mean a press
    /// this desktop has two answers for, and which of them happened would be
    /// whichever was written down first.
    #[test]
    fn nothing_is_bound_twice_in_one_place() {
        for mode in [Mode::Desktop, Mode::Tabs] {
            let mut every: Vec<String> = Vec::new();
            for job in JOBS.iter().filter(|job| job.when.suits(mode)) {
                for (layer, button) in job.bound {
                    every.push(format!("{layer:?} {button}"));
                }
            }
            let many = every.len();
            every.sort();
            every.dedup();
            assert_eq!(every.len(), many, "two jobs on one button in {mode:?}");
        }
    }

    /// Every button named here is a button this machine has a word for, and so
    /// one the profile can route. A default on a button nothing routes is a job
    /// nobody can reach out of the box.
    #[test]
    fn every_default_is_on_a_button_this_desktop_can_route() {
        for job in JOBS {
            for (_, button) in job.bound {
                let named = button_name(button).unwrap_or_else(|_| panic!("{button}"));
                assert!(
                    console_pad::routing::arrives(named).is_some(),
                    "{} is on {button}, which arrives nowhere",
                    job.slug
                );
            }
        }
    }

    /// Every job says what it is. A job with no name is a row the guide cannot
    /// draw and a row the setup screen cannot ask about.
    #[test]
    fn every_job_says_what_it_is() {
        for job in JOBS {
            assert!(!job.what.says().is_empty(), "{} says nothing", job.slug);
            assert!(job.slug.chars().all(|letter| letter.is_ascii_lowercase() || letter == '-'));
        }
    }

    #[test]
    fn a_button_with_a_second_job_does_that_one_while_l2_is_held() {
        let table = ours();
        assert_eq!(table.what("dpad-up", ON, Mode::Desktop).map(|job| job.what), Some(What::Up));
        assert_eq!(table.what("dpad-up", L2, Mode::Desktop).map(|job| job.what), Some(What::Louder));
    }

    /// Holding a trigger is not a second machine.
    #[test]
    fn a_button_with_no_second_job_keeps_doing_its_first_one() {
        let table = ours();
        assert_eq!(table.what("left-paddle-top", L2, Mode::Desktop).map(|job| job.what), Some(What::Menu));
    }

    /// Both triggers is a layer of its own, and falls back to the button on
    /// its own rather than to either trigger's layer.
    #[test]
    fn both_triggers_is_not_either_of_them() {
        let table = ours();
        let both = Layer::of(true, true);
        assert_eq!(table.what("dpad-up", both, Mode::Desktop).map(|job| job.what), Some(What::Up));
    }

    /// The difference the two profiles used to be.
    #[test]
    fn a_button_can_mean_one_thing_on_the_desktop_and_another_in_a_chooser() {
        let table = ours();
        assert_eq!(table.what("a", ON, Mode::Desktop).map(|job| job.what), Some(What::Click));
        assert_eq!(table.what("a", ON, Mode::Tabs).map(|job| job.what), Some(What::Choose));
        assert_eq!(table.what("r1", ON, Mode::Desktop).map(|job| job.what), Some(What::Workspace(1)));
        assert_eq!(table.what("r1", ON, Mode::Tabs).map(|job| job.what), Some(What::Tab(1)));
    }

    /// And the two the chooser deliberately silences: they are the desktop's,
    /// so with a chooser up they are nobody's.
    #[test]
    fn leaving_for_steam_is_not_something_to_do_by_brushing_a_button() {
        let table = ours();
        assert!(table.what("legion-left", ON, Mode::Desktop).is_some());
        assert_eq!(table.what("legion-left", ON, Mode::Tabs), None);
        assert_eq!(table.what("view", ON, Mode::Tabs), None);
    }

    /// Everything any job can send is something the daemon's own device has
    /// to claim, or the press goes nowhere and the button reads as dead.
    #[test]
    fn what_it_can_send_is_read_out_of_the_table() {
        let sends = sends();
        for wanted in [KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::KEY_ESC, KeyCode::KEY_UP] {
            assert!(sends.contains(&wanted), "{wanted:?} is bound and cannot be sent");
        }
        // The wheel and the pointer are not in here: they are movement rather
        // than presses, and `stick-scroll` builds those axes itself.
        assert!(sends.len() >= 11, "{sends:?}");
    }

    /// A key follows the button that asked for it, so a held button is a held
    /// key and the compositor repeats it.
    #[test]
    fn a_key_is_held_for_as_long_as_the_button_is() {
        assert_eq!(What::Up.does(true), Some(Doing::Frame(vec![Out::key(KeyCode::KEY_UP.0, 1)])));
        assert_eq!(What::Up.does(false), Some(Doing::Frame(vec![Out::key(KeyCode::KEY_UP.0, 0)])));
        assert_eq!(What::Click.does(true), Some(Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 1)])));
    }

    /// Something that starts a program happens once, on the way down. Twice
    /// would be two menus for one press.
    #[test]
    fn something_that_starts_a_program_happens_once() {
        assert_eq!(What::Menu.does(true), Some(Doing::run(&["launcher", "--keep"])));
        assert_eq!(What::Menu.does(false), None);
    }

    #[test]
    fn the_shoulders_move_you_and_carry_the_window_while_l2_is_held() {
        assert_eq!(What::Workspace(1).does(true), Some(Doing::workspace("+1", false)));
        assert_eq!(What::Carry(-1).does(true), Some(Doing::workspace("-1", true)));
    }

    /// The keyboard is this desktop's own now. It was the one job nothing here
    /// carried out, because the on-screen keyboard's fork read the pad itself;
    /// under the router that fork never sees the button.
    #[test]
    fn the_keyboard_is_ours() {
        let table = ours();
        let job = table.what("x", ON, Mode::Desktop).expect("x");
        assert_eq!(job.what, What::Keyboard);
        assert_eq!(job.what.does(true), Some(Doing::run(&["osk"])));
        // And the button with a keyboard drawn on it does the same thing.
        assert_eq!(table.what("keyboard", ON, Mode::Desktop).map(|job| job.what), Some(What::Keyboard));
    }

    /// What somebody says about their own machine wins, and only about the job
    /// they said it about.
    #[test]
    fn what_somebody_moved_is_where_they_moved_it() {
        let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let table = Table::of(&said);
        let r2 = Layer::of(false, true);
        assert_eq!(table.what("a", r2, Mode::Desktop).map(|job| job.what), Some(What::Screenshot));
        assert_eq!(table.what("right-paddle-bottom", L2, Mode::Desktop), None);
        assert_eq!(table.what("dpad-up", L2, Mode::Desktop).map(|job| job.what), Some(What::Louder));
    }

    /// A job somebody took the button off is a job with no button, and not a
    /// job back where it started.
    #[test]
    fn a_job_left_with_no_button_is_on_no_button() {
        let said = Jobs::read("[jobs]\nmenu = \"\"\n").expect("a table");
        let table = Table::of(&said);
        assert_eq!(table.what("left-paddle-top", ON, Mode::Desktop), None);
        assert_eq!(table.bindings("menu").len(), 1);
        assert!(!table.bindings("menu")[0].played());
    }

    /// A file from a newer desktop is not a reason to stop doing the jobs this
    /// one has.
    #[test]
    fn a_job_this_desktop_does_not_have_is_left_alone() {
        let said = Jobs::read("[jobs]\nteleport = \"a\"\n").expect("a table");
        let table = Table::of(&said);
        assert_eq!(table.what("a", ON, Mode::Desktop).map(|job| job.what), Some(What::Click));
    }
}
