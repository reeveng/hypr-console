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

use evdev::{KeyCode, RelativeAxisCode};

use console_door::Said;
use console_pad::jobs::{ALONE, Binding, Held, Jobs, Layer};

use crate::doing::{Carry, Doing, Out};
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
    /// signal like any other -- which is what `keyboard-toggle` was always sending.
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
    /// The home screen, told what the pad did rather than typed at.
    ///
    /// It is the only surface on the desktop that is spoken to this way, and
    /// `console_door::homeward` is where the reason is written down: it is
    /// drawn under everything and never in front, so the only way it could
    /// hold the keyboard was to ask for it exclusively -- which Hyprland reads
    /// as a lock screen and answers by handing it every touch on the screen,
    /// including all of the bar's.
    Tell(Said),
    /// A on the home screen, which is both halves of the press.
    ///
    /// A press and a hold are the same press until one of them has gone on
    /// long enough, and the home screen is what does that reckoning -- the
    /// same reckoning it makes of a finger held on a square, so the pad and
    /// the screen agree without either being told about the other.
    Choosing,
    /// Turn the wheel down a notch, which on a page is the way a page is read.
    ScrollDown,
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
    /// The home screen: the desktop with the apps drawn on it and nothing over
    /// them. Two buttons mean something there that they cannot mean over a
    /// bare wallpaper -- A opens what the d-pad is standing on, Y is what else
    /// can be done with it -- and everything else the desktop does, it does.
    OnTheHomeScreen,
    /// The home screen with a highlight up.
    ///
    /// The d-pad belongs to the home screen from the moment it is drawn, and
    /// the first press of it is what raises the highlight. A and Y only join
    /// it once there is a highlight for them to be about: until then A is the
    /// pointer's button, so a thumb on the touchpad can press the bar, a
    /// notification, or anything else the pointer is over.
    StandingOnASquare,
    WithAChooserUp,
}

/// Which way a button is travelling.
///
/// Every job that sends a key sends it twice -- once going down and once
/// coming back up -- and the ones that run a program act on the way down only.
/// So this is the difference between the two halves of a press and not a
/// state anything holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The button going in.
    Down,
    /// The same button coming back out.
    Up,
}

/// Whether a job is one of the ones in front of you.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suits {
    /// It applies to what is on the screen now.
    InFront,
    /// It belongs to some other mode, and this press is not for it.
    Elsewhere,
}

/// Whether a job goes on acting while the button is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeats {
    /// It steps again for as long as the button is in, which is what a scale
    /// wants and what a menu does not.
    WhileHeld,
    /// It happens once for each press.
    Once,
}

impl When {
    /// Whether this is one of the jobs in front of you now.
    ///
    /// The home screen is the desktop with something drawn on it, so every job
    /// written for the desktop is one of its jobs too: the shoulders still
    /// change workspace, the left Legion button still leaves for Steam, and
    /// the button with an eye on it still opens the browser. Two of them it
    /// takes for itself, and those say so.
    pub fn suits(self, mode: Mode) -> Suits {
        let suits = match self {
            When::Anywhere => true,
            When::OnTheDesktop => matches!(mode, Mode::Desktop | Mode::Home | Mode::Standing),
            When::OnTheHomeScreen => matches!(mode, Mode::Home | Mode::Standing),
            When::StandingOnASquare => mode == Mode::Standing,
            When::WithAChooserUp => mode == Mode::Tabs,
        };

        match suits {
            true => Suits::InFront,
            false => Suits::Elsewhere,
        }
    }

    /// How particular this is, where more than one job suits.
    ///
    /// Each step is a place inside the one before it. Anywhere is the least
    /// particular and loses to everything; the desktop and a chooser are each
    /// about one place; the home screen is a place inside the desktop, and
    /// standing on a square is a place inside that -- which is what lets A be
    /// the pointer's button on a home screen that is asleep and the square's
    /// once a highlight is up, without either of them knowing about the
    /// other.
    fn rank(self) -> u8 {
        match self {
            When::Anywhere => 0,
            When::OnTheDesktop | When::WithAChooserUp => 1,
            When::OnTheHomeScreen => 2,
            When::StandingOnASquare => 3,
        }
    }

    /// Whether this is more particular than that, where both suit.
    ///
    /// A job for the desktop beats a job for anywhere on the same button, and
    /// a job for the home screen beats the desktop's. That is what lets A be a
    /// click on a bare desktop and the press that opens what it is standing on
    /// when the home screen is drawn over it, out of two rows rather than out
    /// of a condition inside one.
    fn beats(self, other: Self) -> Suits {
        match self.rank() > other.rank() {
            true => Suits::InFront,
            false => Suits::Elsewhere,
        }
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
pub const JOBS: [Job; 36] = [
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
    // The one paddle nothing was on, and the one job that wants a finger
    // already resting on the machine. A page is read downwards, so down is
    // what the bare press does; the same button held with L2 is still the
    // screenshot. This is a job for a bare paddle by the rule above rather
    // than in spite of it: a page that moved says so the instant it happens,
    // and the way to undo it is the stick, which is already under the thumb.
    Job {
        slug: "scroll-down",
        what: What::ScrollDown,
        when: When::Anywhere,
        bound: &[(ON, "right-paddle-bottom")],
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
    // The home screen's two. A is a click on a bare desktop, because the
    // stick is a pointer there and there is nothing else for it to mean; with
    // the apps drawn on the screen it opens the one being stood on. Y is the
    // same shape: more options, which on an app is what else can be done with
    // it and on an empty square is the offer to put something there.
    // The d-pad belongs to the home screen from the moment it is drawn. These
    // are what wake it: the first press raises the highlight where it was
    // rather than moving it, because what is under a highlight has to be seen
    // before it can be meant.
    Job {
        slug: "home-up",
        what: What::Tell(Said::Up),
        when: When::OnTheHomeScreen,
        bound: &[(ON, "dpad-up")],
    },
    Job {
        slug: "home-down",
        what: What::Tell(Said::Down),
        when: When::OnTheHomeScreen,
        bound: &[(ON, "dpad-down")],
    },
    Job {
        slug: "home-left",
        what: What::Tell(Said::Left),
        when: When::OnTheHomeScreen,
        bound: &[(ON, "dpad-left")],
    },
    Job {
        slug: "home-right",
        what: What::Tell(Said::Right),
        when: When::OnTheHomeScreen,
        bound: &[(ON, "dpad-right")],
    },
    // And these three are the highlight's, so they are the home screen's only
    // once there is one. Asleep, A is the pointer's button and Y is its other
    // one, which is what lets a thumb on the touchpad press the bar.
    Job {
        slug: "home-choose",
        what: What::Choosing,
        when: When::StandingOnASquare,
        bound: &[(ON, "a")],
    },
    Job {
        slug: "home-more",
        what: What::Tell(Said::More),
        when: When::StandingOnASquare,
        bound: &[(ON, "y")],
    },
    Job {
        slug: "home-back",
        what: What::Tell(Said::Back),
        when: When::StandingOnASquare,
        bound: &[(ON, "b")],
    },
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
            What::ScrollDown => "scroll the page down",
            What::Choose => "choose the row you are on",
            What::More => "what else can be done with a row",
            What::Choosing => "open the square you are standing on",
            What::Tell(Said::Up) => "move up the home screen",
            What::Tell(Said::Down) => "move down the home screen",
            What::Tell(Said::Left) => "move left along the home screen",
            What::Tell(Said::Right) => "move right along the home screen",
            What::Tell(Said::More) => "what else can be done with this square",
            What::Tell(Said::Back) => "put down what you are holding, and the highlight away",
            What::Tell(Said::Pressing | Said::Released) => "open the square you are standing on",
            // No button says this one and none should. It is what the settings
            // tab says to the home screen when the grid has been changed, and
            // it is here because the door carries one word that is not a press
            // and this match is over every word the door has.
            What::Tell(Said::Again) => "read the home screen's own settings again",
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
    /// Whether holding this one down should go on doing it.
    ///
    /// The four steps along a scale, and nothing else. A job that sends a key
    /// already repeats without being asked -- the key is held down for as long
    /// as the button is and the compositor is what repeats it -- so a list
    /// walked with the d-pad has always gone on walking. A job that runs a
    /// program fires once when the button goes down, which is right for the
    /// menu and a screenshot and wrong for the volume: five percent a press is
    /// twenty presses from silent to loud, and the thumb is already on the
    /// button.
    ///
    /// Said as a property of the job rather than of the button, so that a
    /// scale moved onto another button keeps this and a button given the menu
    /// does not inherit it.
    pub fn repeats(self) -> Repeats {
        let goes_on = matches!(
            self,
            What::Brighter
                | What::Dimmer
                | What::Louder
                | What::Quieter
                | What::ScrollDown
                // The home screen is told rather than typed at, and a word is
                // said once however long the thumb stays on the button. So the
                // walking that a held arrow key got from the compositor has to
                // be asked for here instead, or the d-pad moves one square a
                // press and a pane takes fifteen of them to cross.
                | What::Tell(Said::Up | Said::Down | Said::Left | Said::Right)
        );

        match goes_on {
            true => Repeats::WhileHeld,
            false => Repeats::Once,
        }
    }

    pub fn does(self, down: Press) -> Option<Doing> {
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
            // Both halves, and the only job here that says anything on the way
            // back up. The home screen is told when A went in and when it came
            // out, and works out from the two of them whether that was a press
            // or a hold.
            What::Choosing => Some(Doing::Tell(match down {
                Press::Down => Said::Pressing,
                Press::Up => Said::Released,
            })),
            _ if down == Press::Up => None,
            What::Tell(said) => Some(Doing::Tell(said)),
            // A wheel notch, and not Page Down. Page Down is a key a page may
            // do anything with -- a video player takes it, a text field moves
            // the caret with it -- and the promise this button makes is the
            // one the stick already makes: the page moves the way a wheel
            // moves it, in the same units, wherever a wheel works at all.
            What::ScrollDown => Some(Doing::Frame(vec![Out::rel(RelativeAxisCode::REL_WHEEL.0, -1)])),
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
            What::Keyboard => Some(Doing::run(&["keyboard-toggle"])),
            What::Workspace(step) => Some(Doing::workspace(&format!("{step:+}"), Carry::Nothing)),
            What::Carry(step) => Some(Doing::workspace(&format!("{step:+}"), Carry::Window)),
        }
    }
}

/// One key or one mouse button, following the button that asked for it.
fn pressed(code: KeyCode, down: Press) -> Doing {
    let value = match down {
        Press::Down => 1,
        Press::Up => 0,
    };

    Doing::Frame(vec![Out::key(code.0, value)])
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
        .filter_map(|job| match job.what.does(Press::Down) {
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
            .or_else(|| match layer.held() {
                Held::Down => self.matching(button, ALONE, mode),
                Held::Up => None,
            })
    }

    fn matching(&self, button: &str, layer: Layer, mode: Mode) -> Option<&'static Job> {
        let mut found: Option<&'static Job> = None;

        for (job, bound) in self.every() {
            if job.when.suits(mode) == Suits::Elsewhere {
                continue;
            }

            if !bound.iter().any(|one| one.layer == layer && one.button == button) {
                continue;
            }

            found = match found {
                Some(already) if already.when.beats(job.when) == Suits::InFront => Some(already),
                _ => Some(job),
            };
        }

        found
    }
}

#[cfg(test)]
mod tests {
    use console_pad::jobs::Played;
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
        for mode in [Mode::Desktop, Mode::Tabs, Mode::Home] {
            let mut every: Vec<String> = Vec::new();
            for job in JOBS.iter().filter(|job| job.when.suits(mode) == Suits::InFront) {
                for (layer, button) in job.bound {
                    // Which of the two the home screen takes for itself is the
                    // one written for the home screen, and that is settled by
                    // how particular each is rather than by which was found
                    // first. So the pair is one button here, and a second job
                    // of the same particularity on it is the tie this is
                    // about.
                    every.push(format!("{layer:?} {button} {}", job.when.rank()));
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
        let both = Layer::of(Held::Down, Held::Down);
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
        assert_eq!(What::Up.does(Press::Down), Some(Doing::Frame(vec![Out::key(KeyCode::KEY_UP.0, 1)])));
        assert_eq!(What::Up.does(Press::Up), Some(Doing::Frame(vec![Out::key(KeyCode::KEY_UP.0, 0)])));
        assert_eq!(What::Click.does(Press::Down), Some(Doing::Frame(vec![Out::key(KeyCode::BTN_LEFT.0, 1)])));
    }

    /// Something that starts a program happens once, on the way down. Twice
    /// would be two menus for one press.
    #[test]
    fn something_that_starts_a_program_happens_once() {
        assert_eq!(What::Menu.does(Press::Down), Some(Doing::run(&["launcher", "--keep"])));
        assert_eq!(What::Menu.does(Press::Up), None);
    }

    #[test]
    fn the_shoulders_move_you_and_carry_the_window_while_l2_is_held() {
        assert_eq!(What::Workspace(1).does(Press::Down), Some(Doing::workspace("+1", Carry::Nothing)));
        assert_eq!(What::Carry(-1).does(Press::Down), Some(Doing::workspace("-1", Carry::Window)));
    }

    /// The keyboard is this desktop's own now. It was the one job nothing here
    /// carried out, because the on-screen keyboard's fork read the pad itself;
    /// under the router that fork never sees the button.
    #[test]
    fn the_keyboard_is_ours() {
        let table = ours();
        let job = table.what("x", ON, Mode::Desktop).expect("x");
        assert_eq!(job.what, What::Keyboard);
        assert_eq!(job.what.does(Press::Down), Some(Doing::run(&["keyboard-toggle"])));
        // And the button with a keyboard drawn on it does the same thing.
        assert_eq!(table.what("keyboard", ON, Mode::Desktop).map(|job| job.what), Some(What::Keyboard));
    }

    /// What somebody says about their own machine wins, and only about the job
    /// they said it about.
    #[test]
    fn what_somebody_moved_is_where_they_moved_it() {
        let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let table = Table::of(&said);
        let r2 = Layer::of(Held::Up, Held::Down);
        assert_eq!(table.what("a", r2, Mode::Desktop).map(|job| job.what), Some(What::Screenshot));
        // The button it came off is not empty: L2 falls through to what the
        // button does bare, which is the scroll. What has to be true is that
        // the screenshot is not there any more, and asserting `None` here
        // asserted that the paddle had nothing on it at all, which is a fact
        // about the rest of the table rather than about the move.
        assert_ne!(
            table.what("right-paddle-bottom", L2, Mode::Desktop).map(|job| job.what),
            Some(What::Screenshot),
        );
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
        assert_eq!(table.bindings("menu")[0].played(), Played::ByNothing);
    }

    /// A file from a newer desktop is not a reason to stop doing the jobs this
    /// one has.
    #[test]
    fn a_job_this_desktop_does_not_have_is_left_alone() {
        let said = Jobs::read("[jobs]\nteleport = \"a\"\n").expect("a table");
        let table = Table::of(&said);
        assert_eq!(table.what("a", ON, Mode::Desktop).map(|job| job.what), Some(What::Click));
    }

    /// The d-pad is the home screen's from the moment it is drawn, and it is
    /// told rather than typed at -- the whole of why is in
    /// `console_door::homeward`. Everything else the desktop does, an asleep
    /// home screen goes on doing.
    #[test]
    fn the_home_screen_takes_the_d_pad_and_leaves_the_rest() {
        let table = Table::of(&Jobs::none());
        let what = |button, mode| table.what(button, ON, mode).map(|job| job.what);

        assert_eq!(what("dpad-up", Mode::Desktop), Some(What::Up), "an arrow key, anywhere else");
        assert_eq!(what("dpad-up", Mode::Home), Some(What::Tell(Said::Up)));
        assert_eq!(what("dpad-down", Mode::Home), Some(What::Tell(Said::Down)));
        assert_eq!(what("dpad-left", Mode::Home), Some(What::Tell(Said::Left)));
        assert_eq!(what("dpad-right", Mode::Home), Some(What::Tell(Said::Right)));

        assert_eq!(what("r1", Mode::Home), Some(What::Workspace(1)));
        assert_eq!(what("l1", Mode::Home), Some(What::Workspace(-1)));
        assert_eq!(what("legion-left", Mode::Home), Some(What::GameMode));
        assert_eq!(what("view", Mode::Home), Some(What::Browser));
        assert_eq!(what("left-paddle-top", Mode::Home), Some(What::Menu));
    }

    /// A is the pointer's button until there is a highlight for it to be
    /// about.
    ///
    /// This is the fault this arrangement is for. The home screen held A from
    /// the moment it was drawn, which is every minute the machine is on, so a
    /// thumb that moved the pointer onto the bar and pressed A opened whatever
    /// the home screen was standing on -- the bar could not be pressed at all,
    /// and it looked like the bar was broken.
    #[test]
    fn a_is_the_pointers_button_until_the_home_screen_is_awake() {
        let table = Table::of(&Jobs::none());
        let what = |button, mode| table.what(button, ON, mode).map(|job| job.what);

        assert_eq!(what("a", Mode::Desktop), Some(What::Click));
        assert_eq!(what("a", Mode::Home), Some(What::Click), "asleep, it is the pointer's");
        assert_eq!(what("a", Mode::Standing), Some(What::Choosing));

        assert_eq!(what("y", Mode::Desktop), Some(What::MoreOptions));
        assert_eq!(what("y", Mode::Home), Some(What::MoreOptions));
        assert_eq!(what("y", Mode::Standing), Some(What::Tell(Said::More)));

        assert_eq!(what("b", Mode::Home), Some(What::Back), "asleep, B is out of things");
        assert_eq!(what("b", Mode::Standing), Some(What::Tell(Said::Back)));
    }

    /// Awake, the d-pad is still the home screen's: standing on a square is a
    /// place inside the home screen and not somewhere else.
    #[test]
    fn the_d_pad_stays_the_home_screens_once_it_is_awake() {
        let table = Table::of(&Jobs::none());
        let what = |button, mode| table.what(button, ON, mode).map(|job| job.what);

        assert_eq!(what("dpad-up", Mode::Standing), Some(What::Tell(Said::Up)));
        assert_eq!(what("r1", Mode::Standing), Some(What::Workspace(1)));
    }

    /// A held d-pad walks the home screen, the way a held arrow key walks
    /// everything else.
    ///
    /// Not the same mechanism, and that is the point. A key repeats because
    /// the compositor repeats a key that is held; a word is said once, so the
    /// walking has to be asked for. A job that forgot to would move one square
    /// however long the thumb stayed on it.
    #[test]
    fn holding_the_d_pad_on_the_home_screen_goes_on_walking_it() {
        for said in [Said::Up, Said::Down, Said::Left, Said::Right] {
            assert_eq!(What::Tell(said).repeats(), Repeats::WhileHeld, "{said:?}");
        }

        assert_eq!(What::Tell(Said::More).repeats(), Repeats::Once);
        assert_eq!(What::Choosing.repeats(), Repeats::Once);
    }

    /// A says something on the way down and something else on the way up, so
    /// the home screen can tell a press from a hold the way it tells a tap
    /// from a finger held on a square.
    #[test]
    fn a_on_the_home_screen_is_said_going_in_and_coming_back_out() {
        assert_eq!(What::Choosing.does(Press::Down), Some(Doing::Tell(Said::Pressing)));
        assert_eq!(What::Choosing.does(Press::Up), Some(Doing::Tell(Said::Released)));

        // Everything else is said once, on the way in. A second word coming
        // back out would be a square opened twice.
        assert_eq!(What::Tell(Said::Up).does(Press::Down), Some(Doing::Tell(Said::Up)));
        assert_eq!(What::Tell(Said::Up).does(Press::Up), None);
    }

    /// Nothing the home screen is told is a key, and nothing it is told needs
    /// the daemon's keyboard to claim a code for it.
    #[test]
    fn a_word_to_the_home_screen_is_not_a_key() {
        for said in console_door::homeward::EVERY {
            assert!(
                matches!(What::Tell(said).does(Press::Down), Some(Doing::Tell(_)) | None),
                "{said:?} came out as something other than a word"
            );
        }
    }
}
