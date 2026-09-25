//! What this desktop can do, and what each of those is bound to.  One table,
//! and now it really is one. What a button meant used to be split between an
//! InputPlumber profile, this daemon, and a third place for the one button
//! neither of them knew about; then between two profiles, because a button
//! meant one thing on the desktop and another with a picker up. Nothing could
//! be asked "what does X do" and answer.  So the profile stopped saying what a
//! button means and started saying only what it is --
//! `console_input_gamepad::routing` -- and everything a press comes to is
//! decided here: the job, when it applies, and what has to be held down with
//! it. The daemon reads it, the setup screen writes to it, and the guide reads
//! it out loud. There is no second copy.  What is in the table is the default
//! and nothing more. Someone who moves a job onto another button writes that
//! in `~/.config/console/buttons.toml`, and only what they moved is in there: a
//! machine no one has touched has an empty file and the whole of its answer
//! here.
//!
//! ## Two inputs, one table
//!
//! A keyboard was the last thing outside it. `hyprland.lua` used to carry
//! thirty binds under a comment saying they were "the effects the pad already
//! names, reached by someone whose hands are on keys" -- which is this table's
//! argument, written in a file this table could not see. So a job is bound on
//! the pad, on a keyboard, or on both, and every one of them is a row here.
//!
//! Which program carries a press out is worked out from the input and is never
//! written down: the daemon matches what arrives off the pad, and a keyboard
//! binding is handed to the compositor as a bind, because the compositor is
//! what owns modifier state and what can swallow a chord before it reaches the
//! window someone is typing in. Two consequences worth knowing. A job reached
//! from a keyboard has to be one that *runs* something -- `Effect::Run` becomes
//! `exec`, and a job that sends a key or tells the home screen has no keyboard
//! binding, because a keyboard already has those keys. And a keyboard binding
//! is not held to `when`: the compositor has no idea what is on the screen, so
//! a key bound to something the pad only does on the desktop will do it with a
//! panel up as well. Both of those were true of the lua binds this replaces.
//!
//! Which key a job sits on is settled by the laptop this tree is written on,
//! because that is the other keyboard the same person types at. Super and a
//! letter opens something, Super and Ctrl and a letter opens a page of the
//! settings, and the letters are the ones already worn in over there -- B the
//! browser, F the files, M the music, W closes the window, Return a terminal. A
//! key that means one thing on the desk and another in the hand is a key that
//! gets pressed wrong once and is never trusted again, and agreeing costs only
//! a few letters this desktop would otherwise have picked for itself. The
//! window jobs are the four an 8.8-inch screen where every window fills it
//! leaves any meaning -- close it, fill the screen, go to the next one, carry
//! it along -- and the rest of a tiling desktop's alphabet stays unbound rather
//! than bound to something that cannot happen here.
//!
//! ## HJKL is where the hand already is
//!
//! Those four jobs are on the arrows and on HJKL both, which is four jobs
//! reached two ways rather than eight jobs. Someone who moves around a file
//! that way moves around a screen that way and the row is under their fingers
//! already; the arrows stay because a hand that never learned vim reaches for
//! them, and Shift with H and L carries the window the way Shift with the
//! arrows already does.
//!
//! That reserves four letters, and it is why the keyboard is on Super and T
//! rather than Super and K, where it was. T is what a keyboard is for, and a
//! job that would take one of the four gets another letter instead.
//!
//! ## A digit says which place, and only a keyboard has one
//!
//! A place is reached by walking -- R1 and L1 in the hand, Super and Tab on a
//! desk -- because a window opens on the next empty workspace and the places
//! are only ever whatever is open. Super and a digit is the other question:
//! not the place after this one, the third place. It is a keyboard's alone,
//! because the pad has no row of ten of anything, and it is the one thing in
//! this table that names a place by a number rather than by where it is from
//! here. A number nothing is open on is an empty place, which on this desktop
//! is the home screen, so a digit is always somewhere to go and never an
//! error. Zero is the tenth, where the row of keys puts it. Shift carries the
//! window along, which is the same sentence Shift already says beside the
//! arrows.
//!
//! The power key is in here too, and it is the one row whose being in a table
//! is a loss as well as a gain. It was the first line of the lua for a reason:
//! a config that fails to load abandons every line after the failure, and a
//! screen that is off and will not come back is this device at its worst --
//! the machine running, every button working, and none of them able to be seen
//! to. `docs/button-contract.md` carries the rest of that argument and what it
//! now rests on instead.

use console_input_event_devices::{KeyCode, RelativeAxisCode};

use console_core_never::Never;
use console_core_internal_programs::InternalProgram;
use console_onscreen::PadInput;
use console_input_bindings::bound::{Binding, Fits, Input};
use console_input_bindings::moved::Tasks;

use crate::effect::{Effect, Payload, Output};
use crate::mode::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Menu,
    Dictate,
    PutAway,
    Screenshot,
    Settings,
    SettingsAt(&'static str),
    Brighter,
    Dimmer,
    Louder,
    Quieter,
    Mute,
    Wake,
    GameMode,
    Browser,
    ButtonGuide,
    Keyboard,
    Language(i32),
    Terminal,
    Files,
    Music,
    Downloads,
    Notifications,
    CloseWindow,
    Fullscreen,
    Focus(&'static str),
    Workspace(i32),
    Place(&'static str),
    Payload(i32),
    CarryTo(&'static str),
    Click,
    MoreOptions,
    Back,
    Up,
    Down,
    Left,
    Right,
    Tell(PadInput),
    ScrollDown,
    Choose,
    More,
    Tab(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Anywhere,
    OnTheDesktop,
    OnTheHomeScreen,
    StandingOnASquare,
    WithAPickerUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonPress {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    InFront,
    Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    WhileHeld,
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockBehavior {
    EvenThen,
    Unlocked,
}

impl Context {
    pub fn applicability(self, mode: Mode) -> Result<Applicability, Never> {
        let fit = match self {
            Context::Anywhere => true,
            Context::OnTheDesktop => matches!(mode, Mode::Desktop | Mode::HomeScreen | Mode::Standing),
            Context::OnTheHomeScreen => matches!(mode, Mode::HomeScreen | Mode::Standing),
            Context::StandingOnASquare => mode == Mode::Standing,
            Context::WithAPickerUp => mode == Mode::Tabs,
        };

        Ok(match fit {
            true => Applicability::InFront,
            false => Applicability::Elsewhere,
        })
    }

    fn rank(self) -> Result<u8, Never> {
        Ok(match self {
            Context::Anywhere => 0,
            Context::OnTheDesktop | Context::WithAPickerUp => 1,
            Context::OnTheHomeScreen => 2,
            Context::StandingOnASquare => 3,
        })
    }

    fn beats(self, other: Self) -> Result<Applicability, Never> {
        let Ok(mine) = self.rank();
        let Ok(theirs) = other.rank();

        Ok(match mine > theirs {
            true => Applicability::InFront,
            false => Applicability::Elsewhere,
        })
    }
}

pub type Bound = (Input, &'static [&'static str], &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Task {
    pub slug: &'static str,
    pub action: Action,
    pub context: Context,
    pub bound: &'static [Bound],
}

const PAD: Input = Input::Pad;
const KEYS: Input = Input::Keyboard;

const ALONE: &[&str] = &[];
const L2: &[&str] = &["l2"];
const SUPER: &[&str] = &["super"];
const SUPER_CONTROL: &[&str] = &["super", "ctrl"];
const SUPER_SHIFT: &[&str] = &["super", "shift"];

pub const JOBS: [Task; 76] = [
    Task {
        slug: "menu",
        action: Action::Menu,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "left-paddle-top"), (KEYS, SUPER, "space")],
    },
    Task {
        slug: "dictate",
        action: Action::Dictate,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "left-paddle-bottom"), (KEYS, SUPER, "v")],
    },
    Task {
        slug: "put-away",
        action: Action::PutAway,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "right-paddle-top")],
    },
    Task {
        slug: "settings",
        action: Action::Settings,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "legion-right"), (KEYS, SUPER, "i")],
    },
    Task {
        slug: "guide",
        action: Action::ButtonGuide,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "menu"), (KEYS, SUPER, "slash")],
    },
    Task {
        slug: "keyboard",
        action: Action::Keyboard,
        context: Context::Anywhere,
        bound: &[
            (PAD, ALONE, "x"),
            (PAD, ALONE, "keyboard"),
            (KEYS, SUPER, "t"),
            (KEYS, ALONE, "calculator"),
        ],
    },
    Task {
        slug: "language-next",
        action: Action::Language(1),
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER_SHIFT, "space")],
    },
    Task {
        slug: "language-previous",
        action: Action::Language(-1),
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER_CONTROL, "space")],
    },
    Task {
        slug: "screenshot",
        action: Action::Screenshot,
        context: Context::Anywhere,
        bound: &[(PAD, L2, "right-paddle-bottom"), (KEYS, SUPER, "s"), (KEYS, ALONE, "print")],
    },
    Task {
        slug: "scroll-down",
        action: Action::ScrollDown,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "right-paddle-bottom")],
    },
    Task {
        slug: "brighter",
        action: Action::Brighter,
        context: Context::Anywhere,
        bound: &[(PAD, L2, "dpad-right"), (KEYS, ALONE, "brightness-up")],
    },
    Task {
        slug: "dimmer",
        action: Action::Dimmer,
        context: Context::Anywhere,
        bound: &[(PAD, L2, "dpad-left"), (KEYS, ALONE, "brightness-down")],
    },
    Task {
        slug: "louder",
        action: Action::Louder,
        context: Context::Anywhere,
        bound: &[(PAD, L2, "dpad-up"), (KEYS, ALONE, "volume-up")],
    },
    Task {
        slug: "quieter",
        action: Action::Quieter,
        context: Context::Anywhere,
        bound: &[(PAD, L2, "dpad-down"), (KEYS, ALONE, "volume-down")],
    },
    Task {
        slug: "mute",
        action: Action::Mute,
        context: Context::Anywhere,
        bound: &[(KEYS, ALONE, "mute")],
    },
    Task {
        slug: "wake",
        action: Action::Wake,
        context: Context::Anywhere,
        bound: &[(KEYS, ALONE, "power")],
    },
    Task {
        slug: "terminal",
        action: Action::Terminal,
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER, "enter")],
    },
    Task {
        slug: "files",
        action: Action::Files,
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER, "f")],
    },
    Task {
        slug: "music",
        action: Action::Music,
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER, "m")],
    },
    Task {
        slug: "downloads",
        action: Action::Downloads,
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER, "d")],
    },
    Task {
        slug: "notifications",
        action: Action::Notifications,
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER, "comma")],
    },
    Task {
        slug: "settings-sound",
        action: Action::SettingsAt("Sound"),
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER_CONTROL, "a")],
    },
    Task {
        slug: "settings-bluetooth",
        action: Action::SettingsAt("Bluetooth"),
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER_CONTROL, "b")],
    },
    Task {
        slug: "settings-wifi",
        action: Action::SettingsAt("Wi-Fi"),
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER_CONTROL, "w")],
    },
    Task {
        slug: "settings-screen",
        action: Action::SettingsAt("Display"),
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER_CONTROL, "d")],
    },
    Task {
        slug: "settings-power",
        action: Action::SettingsAt("Battery"),
        context: Context::Anywhere,
        bound: &[(KEYS, SUPER_CONTROL, "p")],
    },
    Task { slug: "back", action: Action::Back, context: Context::Anywhere, bound: &[(PAD, ALONE, "b")] },
    Task { slug: "up", action: Action::Up, context: Context::Anywhere, bound: &[(PAD, ALONE, "dpad-up")] },
    Task {
        slug: "down",
        action: Action::Down,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "dpad-down")],
    },
    Task {
        slug: "left",
        action: Action::Left,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "dpad-left")],
    },
    Task {
        slug: "right",
        action: Action::Right,
        context: Context::Anywhere,
        bound: &[(PAD, ALONE, "dpad-right")],
    },
    Task {
        slug: "click",
        action: Action::Click,
        context: Context::OnTheDesktop,
        bound: &[(PAD, ALONE, "a"), (PAD, ALONE, "r3")],
    },
    Task {
        slug: "more-options",
        action: Action::MoreOptions,
        context: Context::OnTheDesktop,
        bound: &[(PAD, ALONE, "y")],
    },
    Task {
        slug: "game-mode",
        action: Action::GameMode,
        context: Context::OnTheDesktop,
        bound: &[(PAD, ALONE, "legion-left"), (KEYS, SUPER, "g")],
    },
    Task {
        slug: "browser",
        action: Action::Browser,
        context: Context::OnTheDesktop,
        bound: &[(PAD, ALONE, "view"), (KEYS, SUPER, "b")],
    },
    Task {
        slug: "close-window",
        action: Action::CloseWindow,
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "w")],
    },
    Task {
        slug: "fullscreen",
        action: Action::Fullscreen,
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "f")],
    },
    Task {
        slug: "focus-left",
        action: Action::Focus("left"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "left"), (KEYS, SUPER, "h")],
    },
    Task {
        slug: "focus-right",
        action: Action::Focus("right"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "right"), (KEYS, SUPER, "l")],
    },
    Task {
        slug: "focus-up",
        action: Action::Focus("up"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "up"), (KEYS, SUPER, "k")],
    },
    Task {
        slug: "focus-down",
        action: Action::Focus("down"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "down"), (KEYS, SUPER, "j")],
    },
    Task {
        slug: "workspace-next",
        action: Action::Workspace(1),
        context: Context::OnTheDesktop,
        bound: &[(PAD, ALONE, "r1"), (KEYS, SUPER, "tab")],
    },
    Task {
        slug: "workspace-previous",
        action: Action::Workspace(-1),
        context: Context::OnTheDesktop,
        bound: &[(PAD, ALONE, "l1"), (KEYS, SUPER_SHIFT, "tab")],
    },
    Task {
        slug: "place-one",
        action: Action::Place("1"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "1")],
    },
    Task {
        slug: "place-two",
        action: Action::Place("2"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "2")],
    },
    Task {
        slug: "place-three",
        action: Action::Place("3"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "3")],
    },
    Task {
        slug: "place-four",
        action: Action::Place("4"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "4")],
    },
    Task {
        slug: "place-five",
        action: Action::Place("5"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "5")],
    },
    Task {
        slug: "place-six",
        action: Action::Place("6"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "6")],
    },
    Task {
        slug: "place-seven",
        action: Action::Place("7"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "7")],
    },
    Task {
        slug: "place-eight",
        action: Action::Place("8"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "8")],
    },
    Task {
        slug: "place-nine",
        action: Action::Place("9"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "9")],
    },
    Task {
        slug: "place-ten",
        action: Action::Place("10"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER, "0")],
    },
    Task {
        slug: "carry-next",
        action: Action::Payload(1),
        context: Context::OnTheDesktop,
        bound: &[(PAD, L2, "r1"), (KEYS, SUPER_SHIFT, "right"), (KEYS, SUPER_SHIFT, "l")],
    },
    Task {
        slug: "carry-previous",
        action: Action::Payload(-1),
        context: Context::OnTheDesktop,
        bound: &[(PAD, L2, "l1"), (KEYS, SUPER_SHIFT, "left"), (KEYS, SUPER_SHIFT, "h")],
    },
    Task {
        slug: "carry-to-one",
        action: Action::CarryTo("1"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "1")],
    },
    Task {
        slug: "carry-to-two",
        action: Action::CarryTo("2"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "2")],
    },
    Task {
        slug: "carry-to-three",
        action: Action::CarryTo("3"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "3")],
    },
    Task {
        slug: "carry-to-four",
        action: Action::CarryTo("4"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "4")],
    },
    Task {
        slug: "carry-to-five",
        action: Action::CarryTo("5"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "5")],
    },
    Task {
        slug: "carry-to-six",
        action: Action::CarryTo("6"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "6")],
    },
    Task {
        slug: "carry-to-seven",
        action: Action::CarryTo("7"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "7")],
    },
    Task {
        slug: "carry-to-eight",
        action: Action::CarryTo("8"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "8")],
    },
    Task {
        slug: "carry-to-nine",
        action: Action::CarryTo("9"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "9")],
    },
    Task {
        slug: "carry-to-ten",
        action: Action::CarryTo("10"),
        context: Context::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "0")],
    },
    Task {
        slug: "home-up",
        action: Action::Tell(PadInput::Up),
        context: Context::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-up")],
    },
    Task {
        slug: "home-down",
        action: Action::Tell(PadInput::Down),
        context: Context::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-down")],
    },
    Task {
        slug: "home-left",
        action: Action::Tell(PadInput::Left),
        context: Context::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-left")],
    },
    Task {
        slug: "home-right",
        action: Action::Tell(PadInput::Right),
        context: Context::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-right")],
    },
    Task {
        slug: "home-choose",
        action: Action::Tell(PadInput::Pressed),
        context: Context::StandingOnASquare,
        bound: &[(PAD, ALONE, "a"), (PAD, ALONE, "r3")],
    },
    Task {
        slug: "home-more",
        action: Action::Tell(PadInput::More),
        context: Context::StandingOnASquare,
        bound: &[(PAD, ALONE, "y")],
    },
    Task {
        slug: "home-back",
        action: Action::Tell(PadInput::Back),
        context: Context::StandingOnASquare,
        bound: &[(PAD, ALONE, "b")],
    },
    Task {
        slug: "choose",
        action: Action::Choose,
        context: Context::WithAPickerUp,
        bound: &[(PAD, ALONE, "a"), (PAD, ALONE, "r3")],
    },
    Task {
        slug: "more",
        action: Action::More,
        context: Context::WithAPickerUp,
        bound: &[(PAD, ALONE, "y")],
    },
    Task {
        slug: "tab-right",
        action: Action::Tab(1),
        context: Context::WithAPickerUp,
        bound: &[(PAD, ALONE, "r1")],
    },
    Task {
        slug: "tab-left",
        action: Action::Tab(-1),
        context: Context::WithAPickerUp,
        bound: &[(PAD, ALONE, "l1")],
    },
];

pub fn job(slug: &str) -> Result<Option<&'static Task>, Never> {
    let Ok(mut every) = every();

    Ok(every.find(|job| job.slug == slug))
}

pub fn every() -> Result<impl Iterator<Item = &'static Task>, Never> {
    Ok(JOBS.iter())
}

impl Action {
    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            Action::Menu => "open the menu",
            Action::Dictate => "start dictation",
            Action::PutAway => "close the panel or the window",
            Action::Screenshot => "take a screenshot",
            Action::Settings => "open Settings",
            Action::SettingsAt("Sound") => "open Sound settings",
            Action::SettingsAt("Bluetooth") => "open Bluetooth settings",
            Action::SettingsAt("Wi-Fi") => "open Wi-Fi settings",
            Action::SettingsAt("Display") => "open Display settings",
            Action::SettingsAt(_) => "open Battery settings",
            Action::Brighter => "increase brightness",
            Action::Dimmer => "decrease brightness",
            Action::Louder => "volume up",
            Action::Quieter => "volume down",
            Action::Mute => "mute and unmute",
            Action::Wake => "wake the screen",
            Action::GameMode => "switch to Steam Game Mode",
            Action::Browser => "open the browser",
            Action::ButtonGuide => "show the button guide",
            Action::Keyboard => "show or hide the keyboard",
            Action::Language(-1) => "previous keyboard",
            Action::Language(_) => "next keyboard",
            Action::Terminal => "open Terminal",
            Action::Files => "open Files",
            Action::Music => "open Music",
            Action::Downloads => "open Downloads",
            Action::Notifications => "show notifications",
            Action::CloseWindow => "close the window",
            Action::Fullscreen => "full screen on or off",
            Action::Focus("left") => "switch to the window on the left",
            Action::Focus("right") => "switch to the window on the right",
            Action::Focus("up") => "switch to the window above",
            Action::Focus(_) => "switch to the window below",
            Action::Workspace(-1) => "previous desktop",
            Action::Workspace(_) => "next desktop",
            Action::Place("1") => "go to desktop 1",
            Action::Place("2") => "go to desktop 2",
            Action::Place("3") => "go to desktop 3",
            Action::Place("4") => "go to desktop 4",
            Action::Place("5") => "go to desktop 5",
            Action::Place("6") => "go to desktop 6",
            Action::Place("7") => "go to desktop 7",
            Action::Place("8") => "go to desktop 8",
            Action::Place("9") => "go to desktop 9",
            Action::Place(_) => "go to desktop 10",
            Action::Payload(-1) => "move the window to the previous desktop",
            Action::Payload(_) => "move the window to the next desktop",
            Action::CarryTo("1") => "move the window to desktop 1",
            Action::CarryTo("2") => "move the window to desktop 2",
            Action::CarryTo("3") => "move the window to desktop 3",
            Action::CarryTo("4") => "move the window to desktop 4",
            Action::CarryTo("5") => "move the window to desktop 5",
            Action::CarryTo("6") => "move the window to desktop 6",
            Action::CarryTo("7") => "move the window to desktop 7",
            Action::CarryTo("8") => "move the window to desktop 8",
            Action::CarryTo("9") => "move the window to desktop 9",
            Action::CarryTo(_) => "move the window to desktop 10",
            Action::Click => "click",
            Action::MoreOptions => "secondary click",
            Action::Back => "go back",
            Action::Up => "move up",
            Action::Down => "move down",
            Action::Left => "move left",
            Action::Right => "move right",
            Action::ScrollDown => "scroll down",
            Action::Choose => "select",
            Action::More => "more options",
            Action::Tell(PadInput::Up) => "move up on the Home Screen",
            Action::Tell(PadInput::Down) => "move down on the Home Screen",
            Action::Tell(PadInput::Left) => "move left on the Home Screen",
            Action::Tell(PadInput::Right) => "move right on the Home Screen",
            Action::Tell(PadInput::More) => "options for this app",
            Action::Tell(PadInput::Back) => "cancel",
            Action::Tell(PadInput::Pressed) => "open the app",
            Action::Tell(PadInput::Again) => "reload the Home Screen layout",
            Action::Tell(PadInput::Payload) => "pick up the app to move it",
            Action::Tell(PadInput::Off) => "remove the app from the Home Screen",
            Action::Tab(-1) => "previous tab",
            Action::Tab(_) => "next tab",
        })
    }

    pub fn repeats(self) -> Result<RepeatMode, Never> {
        let goes_on = matches!(
            self,
            Action::Brighter
                | Action::Dimmer
                | Action::Louder
                | Action::Quieter
                | Action::ScrollDown
                | Action::Tell(PadInput::Up | PadInput::Down | PadInput::Left | PadInput::Right)
        );

        Ok(match goes_on {
            true => RepeatMode::WhileHeld,
            false => RepeatMode::Once,
        })
    }

    pub fn locked(self) -> Result<LockBehavior, Never> {
        let in_the_dark = matches!(
            self,
            Action::Brighter | Action::Dimmer | Action::Louder | Action::Quieter | Action::Mute | Action::Wake
        );

        Ok(match in_the_dark {
            true => LockBehavior::EvenThen,
            false => LockBehavior::Unlocked,
        })
    }

    pub fn does(self, down: ButtonPress) -> Result<Option<Effect>, Never> {
        match self {
            Action::Click => pressed(KeyCode::BTN_LEFT, down),
            Action::MoreOptions => pressed(KeyCode::BTN_RIGHT, down),
            Action::Back => pressed(KeyCode::KEY_ESC, down),
            Action::Up => pressed(KeyCode::KEY_UP, down),
            Action::Down => pressed(KeyCode::KEY_DOWN, down),
            Action::Left => pressed(KeyCode::KEY_LEFT, down),
            Action::Right => pressed(KeyCode::KEY_RIGHT, down),
            Action::Choose => pressed(KeyCode::KEY_ENTER, down),
            Action::More => pressed(KeyCode::KEY_F18, down),
            Action::Tab(-1) => pressed(KeyCode::KEY_PAGEUP, down),
            Action::Tab(_) => pressed(KeyCode::KEY_PAGEDOWN, down),
            Action::Tell(_)
            | Action::ScrollDown
            | Action::Menu
            | Action::Dictate
            | Action::PutAway
            | Action::Screenshot
            | Action::Settings
            | Action::SettingsAt(_)
            | Action::Brighter
            | Action::Dimmer
            | Action::Louder
            | Action::Quieter
            | Action::Mute
            | Action::Wake
            | Action::GameMode
            | Action::Browser
            | Action::ButtonGuide
            | Action::Keyboard
            | Action::Language(_)
            | Action::Terminal
            | Action::Files
            | Action::Music
            | Action::Downloads
            | Action::Notifications
            | Action::CloseWindow
            | Action::Fullscreen
            | Action::Focus(_)
            | Action::Workspace(_)
            | Action::Place(_)
            | Action::Payload(_)
            | Action::CarryTo(_) => match down {
                ButtonPress::Up => Ok(None),
                ButtonPress::Down => self.once(),
            },
        }
    }

    fn once(self) -> Result<Option<Effect>, Never> {
        match self {
            Action::Click
            | Action::MoreOptions
            | Action::Back
            | Action::Up
            | Action::Down
            | Action::Left
            | Action::Right
            | Action::Choose
            | Action::More
            | Action::Tab(_) => Ok(None),
            Action::Tell(said) => Ok(Some(Effect::Tell(said))),
            Action::ScrollDown => scrolled(),
            Action::Menu => run_by_name(InternalProgram::Launcher, &["--keep"]),
            Action::Dictate => run_by_name(InternalProgram::Dictate, &[]),
            Action::PutAway => run_by_name(InternalProgram::PutAway, &[]),
            Action::Screenshot => run_by_path(InternalProgram::Screenshot, &[]),
            Action::Settings => run_by_name(InternalProgram::SettingsPanel, &[]),
            Action::SettingsAt(tab) => run_by_name(InternalProgram::SettingsPanel, &[tab]),
            Action::Brighter => run_by_path(InternalProgram::Brightness, &["up"]),
            Action::Dimmer => run_by_path(InternalProgram::Brightness, &["down"]),
            Action::Louder => run_by_path(InternalProgram::Volume, &["up"]),
            Action::Quieter => run_by_path(InternalProgram::Volume, &["down"]),
            Action::Mute => run_by_path(InternalProgram::Volume, &["mute"]),
            Action::Wake => run_by_path(InternalProgram::Brightness, &["undim"]),
            Action::GameMode => run_by_name(InternalProgram::SessionGame, &[]),
            Action::Browser => run_by_path(InternalProgram::Browser, &[]),
            Action::ButtonGuide => run_by_path(InternalProgram::MappingPanel, &[]),
            Action::Keyboard => run_by_name(InternalProgram::KeyboardToggle, &[]),
            Action::Language(-1) => {
                started(&[console_input_language::NAMED, console_input_language::BACK])
            }
            Action::Language(_) => started(&[console_input_language::NAMED]),
            Action::Terminal => terminal(),
            Action::Files => run_by_path(InternalProgram::Files, &[]),
            Action::Music => run_by_path(InternalProgram::MusicPanel, &[]),
            Action::Downloads => run_by_path(InternalProgram::Downloads, &[]),
            Action::Notifications => run_by_path(InternalProgram::NotificationsPanel, &[]),
            Action::CloseWindow => {
                let closes = Effect::dispatch(console_compositor::CLOSE_WINDOW)?;

                Ok(Some(closes))
            }
            Action::Fullscreen => {
                let fills = Effect::dispatch("hl.dsp.window.fullscreen()")?;

                Ok(Some(fills))
            }
            Action::Focus(way) => {
                let Ok(focus) = Effect::dispatch(&format!("hl.dsp.focus({{direction = \"{way}\"}})"));

                Ok(Some(focus))
            }
            Action::Workspace(step) => moved(step, Payload::None),
            Action::Place(number) => at(number, Payload::None),
            Action::Payload(step) => moved(step, Payload::Window),
            Action::CarryTo(number) => at(number, Payload::Window),
        }
    }
}

fn pressed(code: KeyCode, down: ButtonPress) -> Result<Option<Effect>, Never> {
    let value = match down {
        ButtonPress::Down => 1,
        ButtonPress::Up => 0,
    };
    let Ok(out) = Output::key(code.0, value);

    Ok(Some(Effect::Frame(vec![out])))
}

fn scrolled() -> Result<Option<Effect>, Never> {
    let Ok(out) = Output::relative(RelativeAxisCode::REL_WHEEL.0, -1);

    Ok(Some(Effect::Frame(vec![out])))
}

fn started(arguments: &[&str]) -> Result<Option<Effect>, Never> {
    let Ok(run) = Effect::run(arguments);

    Ok(Some(run))
}

fn run_by_name(program: InternalProgram, arguments: &[&str]) -> Result<Option<Effect>, Never> {
    let Ok(named) = program.name();

    let words: Vec<&str> = std::iter::once(named).chain(arguments.iter().copied()).collect();

    started(&words)
}

fn run_by_path(program: InternalProgram, arguments: &[&str]) -> Result<Option<Effect>, Never> {
    let Ok(at) = program.path();

    let words: Vec<&str> = std::iter::once(at).chain(arguments.iter().copied()).collect();

    started(&words)
}

fn terminal() -> Result<Option<Effect>, Never> {
    let Ok(alacritty) = console_core_external_programs::Program::Alacritty.name();

    started(&[alacritty])
}

fn moved(step: i32, carrying: Payload) -> Result<Option<Effect>, Never> {
    let Ok(moved) = Effect::workspace(&format!("{step:+}"), carrying);

    Ok(Some(moved))
}

fn at(number: &str, carrying: Payload) -> Result<Option<Effect>, Never> {
    let Ok(gone) = Effect::workspace(number, carrying);

    Ok(Some(gone))
}

pub fn sends() -> Result<Vec<KeyCode>, Never> {
    let Ok(jobs) = every();

    let mut every: Vec<KeyCode> = jobs
        .filter_map(|job| {
            let Ok(does) = job.action.does(ButtonPress::Down);

            match does {
                Some(Effect::Frame(frame)) => frame.first().map(|out| KeyCode(out.code)),
                Some(Effect::Run(_) | Effect::Tell(_) | Effect::Using(_) | Effect::Reconnected(_)) | None => None,
            }
        })
        .collect();

    every.sort_unstable_by_key(|key| key.0);
    every.dedup();

    Ok(every)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    bound: Vec<(&'static str, Vec<Binding>)>,
}

impl Default for Table {
    fn default() -> Self {
        let Ok(ours) = Table::ours();

        ours
    }
}

impl Table {
    pub fn ours() -> Result<Self, Never> {
        let Ok(none) = Tasks::none();

        Table::of(&none)
    }

    pub fn of(said: &Tasks) -> Result<Self, Never> {
        let Ok(jobs) = every();

        Ok(Table {
            bound: jobs
                .map(|job| {
                    let Ok(given) = said.bound(job.slug);

                    let bound = match given {
                        Some(moved) => moved.to_vec(),
                        None => {
                            let Ok(ours) = ours(job);

                            ours
                        }
                    };

                    (job.slug, bound)
                })
                .collect(),
        })
    }

    pub fn bindings(&self, slug: &str) -> Result<&[Binding], Never> {
        Ok(self
            .bound
            .iter()
            .find(|(named, _)| *named == slug)
            .map_or(&[], |(_, bound)| bound.as_slice()))
    }

    pub fn every(&self) -> Result<impl Iterator<Item = (&'static Task, &[Binding])>, Never> {
        Ok(self.bound.iter().filter_map(|(slug, bound)| {
            let Ok(job) = job(slug);

            job.map(|job| (job, bound.as_slice()))
        }))
    }

    pub fn what(
        &self,
        on: Input,
        held: &[&str],
        pressed: &str,
        mode: Mode,
    ) -> Result<Option<&'static Task>, Never> {
        let mut found: Option<(&'static Task, u32)> = None;
        let Ok(every) = self.every();

        for (job, bound) in every {
            let Ok(applicability) = job.context.applicability(mode);

            match applicability {
                Applicability::Elsewhere => continue,
                Applicability::InFront => {},
            }

            let Ok(deepest) = deepest(bound, on, held, pressed);

            let depth = match deepest {
                Some(depth) => depth,
                None => continue,
            };

            found = match found {
                Some((already, was)) => match was > depth {
                    true => Some((already, was)),
                    false => match was == depth {
                        true => {
                            let Ok(beats) = already.context.beats(job.context);

                            match beats {
                                Applicability::InFront => Some((already, was)),
                                Applicability::Elsewhere => Some((job, depth)),
                            }
                        }
                        false => Some((job, depth)),
                    },
                },
                None => Some((job, depth)),
            };
        }

        Ok(found.map(|(job, _)| job))
    }
}

pub fn ours(job: &Task) -> Result<Vec<Binding>, Never> {
    Ok(job
        .bound
        .iter()
        .map(|(on, held, pressed)| {
            let Ok(binding) = Binding::holding(*on, held, pressed);

            binding
        })
        .collect())
}

fn deepest(
    bound: &[Binding],
    on: Input,
    held: &[&str],
    pressed: &str,
) -> Result<Option<u32>, Never> {
    Ok(bound
        .iter()
        .filter(|one| {
            let Ok(fits) = one.fits(on, held, pressed);

            fits == Fits::Yes
        })
        .map(|one| {
            let Ok(depth) = one.depth();

            depth
        })
        .max())
}

#[cfg(test)]
mod tests {
    use console_input_bindings::bound::Played;
    use super::*;
    use std::collections::BTreeSet;
    use console_input_gamepad::vocabulary::button_name;

    fn ok<T>(answer: Result<T, console_core_never::Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn ours() -> Table {
        ok(Table::ours())
    }

    fn none() -> Tasks {
        ok(Tasks::none())
    }

    fn what(table: &Table, pressed: &str, held: &[&str], mode: Mode) -> Option<Action> {
        let Ok(found) = table.what(Input::Pad, held, pressed, mode);

        found.map(|job| job.action)
    }

    fn typed(table: &Table, pressed: &str, held: &[&str]) -> Option<Action> {
        let Ok(found) = table.what(Input::Keyboard, held, pressed, Mode::Desktop);

        found.map(|job| job.action)
    }

    #[test]
    fn nothing_is_written_down_twice() {
        let mut seen = BTreeSet::new();
        let twice: Vec<&str> =
            ok(every()).map(|job| job.slug).filter(|slug| !seen.insert(*slug)).collect();

        assert!(twice.is_empty(), "two jobs are called {twice:?}");
    }

    #[test]
    fn nothing_is_bound_twice_in_one_place() {
        for mode in [Mode::Desktop, Mode::Tabs, Mode::HomeScreen] {
            let mut places: Vec<String> = Vec::new();

            for job in ok(every()).filter(|job| job.context.applicability(mode) == Ok(Applicability::InFront)) {
                for (on, held, pressed) in job.bound {
                    let Ok(word) = on.word();

                    places.push(format!("{word} {held:?} {pressed} {}", ok(job.context.rank())));
                }
            }

            let mut seen = BTreeSet::new();
            let twice: Vec<String> =
                places.into_iter().filter(|on| !seen.insert(on.clone())).collect();

            assert!(twice.is_empty(), "two jobs in one place in {mode:?}: {twice:?}");
        }
    }

    #[test]
    fn every_default_is_on_something_this_desktop_can_read() {
        for job in ok(every()) {
            for (on, held, pressed) in job.bound {
                let Ok(binding) = Binding::holding(*on, held, pressed);
                let read = console_input_bindings::bound::Binding::read(&binding.to_string());

                assert!(read.is_ok(), "{}: {binding} ({read:?})", job.slug);
            }
        }
    }

    #[test]
    fn every_button_a_default_names_is_one_this_desktop_routes() {
        for job in ok(every()) {
            for (on, _, pressed) in job.bound {
                match on {
                    Input::Keyboard => continue,
                    Input::Pad => {},
                }

                let named = button_name(pressed).unwrap_or_else(|_| panic!("{pressed}"));

                assert!(
                    ok(console_input_gamepad::routing::arrives(named)).is_some(),
                    "{} is on {pressed}, which arrives nowhere",
                    job.slug
                );
            }
        }
    }

    #[test]
    fn a_job_reached_from_a_keyboard_is_a_job_that_runs_something() {
        for job in ok(every()) {
            let keys = job.bound.iter().any(|(on, _, _)| *on == Input::Keyboard);

            match keys {
                true => {},
                false => continue,
            }

            let Ok(does) = job.action.does(ButtonPress::Down);

            assert!(
                matches!(does, Some(Effect::Run(_))),
                "{} is on a key and does not run anything",
                job.slug
            );
        }
    }

    #[test]
    fn every_job_says_what_it_is() {
        for job in ok(every()) {
            assert!(!ok(job.action.says()).is_empty(), "{} says nothing", job.slug);
            assert!(job.slug.chars().all(|letter| letter.is_ascii_lowercase() || letter == '-'));
        }
    }

    #[test]
    fn a_button_with_a_second_job_does_that_one_while_l2_is_held() {
        let table = ours();

        assert_eq!(what(&table, "dpad-up", &[], Mode::Desktop), Some(Action::Up));
        assert_eq!(what(&table, "dpad-up", &["l2"], Mode::Desktop), Some(Action::Louder));
    }

    #[test]
    fn a_button_with_no_second_job_keeps_doing_its_first_one() {
        let table = ours();

        assert_eq!(what(&table, "left-paddle-top", &["l2"], Mode::Desktop), Some(Action::Menu));
    }

    #[test]
    fn both_triggers_is_not_either_of_them() {
        let table = ours();

        assert_eq!(what(&table, "dpad-up", &["l2", "r2"], Mode::Desktop), Some(Action::Louder));
    }

    #[test]
    fn a_button_can_mean_one_thing_on_the_desktop_and_another_in_a_picker() {
        let table = ours();

        assert_eq!(what(&table, "a", &[], Mode::Desktop), Some(Action::Click));
        assert_eq!(what(&table, "a", &[], Mode::Tabs), Some(Action::Choose));
        assert_eq!(what(&table, "r1", &[], Mode::Desktop), Some(Action::Workspace(1)));
        assert_eq!(what(&table, "r1", &[], Mode::Tabs), Some(Action::Tab(1)));
    }

    #[test]
    fn leaving_for_steam_is_not_something_to_do_by_brushing_a_button() {
        let table = ours();

        assert!(what(&table, "legion-left", &[], Mode::Desktop).is_some());
        assert_eq!(what(&table, "legion-left", &[], Mode::Tabs), None);
        assert_eq!(what(&table, "view", &[], Mode::Tabs), None);
    }

    #[test]
    fn a_key_is_not_a_button_and_does_not_answer_for_one() {
        let table = ours();

        assert_eq!(typed(&table, "i", &["super"]), Some(Action::Settings));
        assert_eq!(typed(&table, "i", &[]), None, "the modifier is part of the place");
        assert_eq!(what(&table, "i", &["super"], Mode::Desktop), None);
    }

    #[test]
    fn a_digit_names_a_place_by_its_number_and_zero_is_the_tenth() {
        let table = ours();

        assert_eq!(typed(&table, "3", &["super"]), Some(Action::Place("3")));
        assert_eq!(typed(&table, "0", &["super"]), Some(Action::Place("10")));
        assert_eq!(what(&table, "3", &["super"], Mode::Desktop), None, "a pad has no digits");

        let Ok(third) = Effect::workspace("3", Payload::None);

        assert_eq!(Action::Place("3").does(ButtonPress::Down), Ok(Some(third)));
        assert_eq!(ok(Action::Place("10").says()), "go to desktop 10");

        let Ok(carried) = Effect::workspace("3", Payload::Window);

        assert_eq!(typed(&table, "3", &["super", "shift"]), Some(Action::CarryTo("3")));
        assert_eq!(Action::CarryTo("3").does(ButtonPress::Down), Ok(Some(carried)));
    }

    #[test]
    fn the_keys_the_lua_used_to_hold_are_in_the_table() {
        let table = ours();

        assert_eq!(typed(&table, "enter", &["super"]), Some(Action::Terminal));
        assert_eq!(typed(&table, "f", &["super"]), Some(Action::Files));
        assert_eq!(typed(&table, "w", &["super"]), Some(Action::CloseWindow));
        assert_eq!(typed(&table, "f", &["super", "shift"]), Some(Action::Fullscreen));
        assert_eq!(typed(&table, "left", &["super"]), Some(Action::Focus("left")));
        assert_eq!(typed(&table, "left", &["super", "shift"]), Some(Action::Payload(-1)));
        assert_eq!(typed(&table, "a", &["super", "ctrl"]), Some(Action::SettingsAt("Sound")));
        assert_eq!(typed(&table, "print", &[]), Some(Action::Screenshot));
        assert_eq!(typed(&table, "volume-up", &[]), Some(Action::Louder));
    }

    #[test]
    fn the_level_keys_answer_with_the_screen_locked_and_nothing_else_does() {
        assert_eq!(Action::Louder.locked(), Ok(LockBehavior::EvenThen));
        assert_eq!(Action::Wake.locked(), Ok(LockBehavior::EvenThen));
        assert_eq!(Action::Terminal.locked(), Ok(LockBehavior::Unlocked));
        assert_eq!(Action::Settings.locked(), Ok(LockBehavior::Unlocked));
    }

    #[test]
    fn what_it_can_send_is_read_out_of_the_table() {
        let sends = ok(sends());

        for wanted in [KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::KEY_ESC, KeyCode::KEY_UP] {
            assert!(sends.contains(&wanted), "{wanted:?} is bound and cannot be sent");
        }

        assert!(sends.len() >= 11, "{sends:?}");
    }

    #[test]
    fn a_key_is_held_for_as_long_as_the_button_is() {
        let down = ok(Output::key(KeyCode::KEY_UP.0, 1));
        let up = ok(Output::key(KeyCode::KEY_UP.0, 0));
        let clicked = ok(Output::key(KeyCode::BTN_LEFT.0, 1));

        assert_eq!(Action::Up.does(ButtonPress::Down), Ok(Some(Effect::Frame(vec![down]))));
        assert_eq!(Action::Up.does(ButtonPress::Up), Ok(Some(Effect::Frame(vec![up]))));
        assert_eq!(Action::Click.does(ButtonPress::Down), Ok(Some(Effect::Frame(vec![clicked]))));
    }

    #[test]
    fn something_that_starts_a_program_happens_once() {
        assert_eq!(Action::Menu.does(ButtonPress::Down), Ok(Some(ok(Effect::run(&["launcher", "--keep"])))));
        assert_eq!(Action::Menu.does(ButtonPress::Up), Ok(None));
    }

    #[test]
    fn the_shoulders_move_you_and_carry_the_window_while_l2_is_held() {
        let moved = ok(Effect::workspace("+1", Payload::None));
        let carried = ok(Effect::workspace("-1", Payload::Window));

        assert_eq!(Action::Workspace(1).does(ButtonPress::Down), Ok(Some(moved)));
        assert_eq!(Action::Payload(-1).does(ButtonPress::Down), Ok(Some(carried)));
    }

    #[test]
    fn the_keyboard_is_ours() {
        let table = ours();
        let job = ok(table.what(Input::Pad, &[], "x", Mode::Desktop)).expect("x");

        assert_eq!(job.action, Action::Keyboard);
        assert_eq!(job.action.does(ButtonPress::Down), Ok(Some(ok(Effect::run(&["keyboard-toggle"])))));
        assert_eq!(what(&table, "keyboard", &[], Mode::Desktop), Some(Action::Keyboard));
    }

    #[test]
    fn what_someone_moved_is_where_they_moved_it() {
        let said = Tasks::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let table = ok(Table::of(&said));

        assert_eq!(what(&table, "a", &["r2"], Mode::Desktop), Some(Action::Screenshot));
        assert_ne!(
            what(&table, "right-paddle-bottom", &["l2"], Mode::Desktop),
            Some(Action::Screenshot)
        );
        assert_eq!(what(&table, "dpad-up", &["l2"], Mode::Desktop), Some(Action::Louder));
    }

    #[test]
    fn a_job_moved_onto_a_chord_of_two_buttons_is_read_off_the_chord() {
        let said = Tasks::read("[jobs]\nmenu = \"left-paddle-bottom + right-paddle-top\"\n")
            .expect("a table");
        let table = ok(Table::of(&said));

        assert_eq!(
            what(&table, "right-paddle-top", &["left-paddle-bottom"], Mode::Desktop),
            Some(Action::Menu)
        );
        assert_eq!(
            what(&table, "right-paddle-top", &[], Mode::Desktop),
            Some(Action::PutAway),
            "the button on its own goes on doing what it did"
        );
    }

    #[test]
    fn a_job_left_with_no_button_is_on_no_button() {
        let said = Tasks::read("[jobs]\nmenu = \"\"\n").expect("a table");
        let table = ok(Table::of(&said));

        assert_eq!(what(&table, "left-paddle-top", &[], Mode::Desktop), None);
        assert_eq!(ok(table.bindings("menu")).len(), 1);
        assert_eq!(
            ok(table.bindings("menu")).first().map(|one| one.played()),
            Some(Ok(Played::ByNothing))
        );
    }

    #[test]
    fn a_job_this_desktop_does_not_have_is_left_alone() {
        let said = Tasks::read("[jobs]\nteleport = \"a\"\n").expect("a table");
        let table = ok(Table::of(&said));

        assert_eq!(what(&table, "a", &[], Mode::Desktop), Some(Action::Click));
    }

    #[test]
    fn the_home_screen_takes_the_d_pad_and_leaves_the_rest() {
        let table = ok(Table::of(&none()));

        assert_eq!(what(&table, "dpad-up", &[], Mode::Desktop), Some(Action::Up));
        assert_eq!(what(&table, "dpad-up", &[], Mode::HomeScreen), Some(Action::Tell(PadInput::Up)));
        assert_eq!(what(&table, "dpad-down", &[], Mode::HomeScreen), Some(Action::Tell(PadInput::Down)));
        assert_eq!(what(&table, "dpad-left", &[], Mode::HomeScreen), Some(Action::Tell(PadInput::Left)));
        assert_eq!(what(&table, "dpad-right", &[], Mode::HomeScreen), Some(Action::Tell(PadInput::Right)));

        assert_eq!(what(&table, "r1", &[], Mode::HomeScreen), Some(Action::Workspace(1)));
        assert_eq!(what(&table, "l1", &[], Mode::HomeScreen), Some(Action::Workspace(-1)));
        assert_eq!(what(&table, "legion-left", &[], Mode::HomeScreen), Some(Action::GameMode));
        assert_eq!(what(&table, "view", &[], Mode::HomeScreen), Some(Action::Browser));
        assert_eq!(what(&table, "left-paddle-top", &[], Mode::HomeScreen), Some(Action::Menu));
    }

    #[test]
    fn a_is_the_pointers_button_until_the_home_screen_is_awake() {
        let table = ok(Table::of(&none()));

        assert_eq!(what(&table, "a", &[], Mode::Desktop), Some(Action::Click));
        assert_eq!(what(&table, "a", &[], Mode::HomeScreen), Some(Action::Click));
        assert_eq!(what(&table, "a", &[], Mode::Standing), Some(Action::Tell(PadInput::Pressed)));

        assert_eq!(what(&table, "y", &[], Mode::Desktop), Some(Action::MoreOptions));
        assert_eq!(what(&table, "y", &[], Mode::HomeScreen), Some(Action::MoreOptions));
        assert_eq!(what(&table, "y", &[], Mode::Standing), Some(Action::Tell(PadInput::More)));

        assert_eq!(what(&table, "b", &[], Mode::HomeScreen), Some(Action::Back));
        assert_eq!(what(&table, "b", &[], Mode::Standing), Some(Action::Tell(PadInput::Back)));
    }

    #[test]
    fn the_d_pad_stays_the_home_screens_once_it_is_awake() {
        let table = ok(Table::of(&none()));

        assert_eq!(what(&table, "dpad-up", &[], Mode::Standing), Some(Action::Tell(PadInput::Up)));
        assert_eq!(what(&table, "r1", &[], Mode::Standing), Some(Action::Workspace(1)));
    }

    #[test]
    fn holding_the_d_pad_on_the_home_screen_goes_on_walking_it() {
        for said in [PadInput::Up, PadInput::Down, PadInput::Left, PadInput::Right] {
            assert_eq!(Action::Tell(said).repeats(), Ok(RepeatMode::WhileHeld), "{said:?}");
        }

        assert_eq!(Action::Tell(PadInput::More).repeats(), Ok(RepeatMode::Once));
        assert_eq!(Action::Tell(PadInput::Pressed).repeats(), Ok(RepeatMode::Once));
    }

    #[test]
    fn what_the_home_screen_is_told_is_said_once() {
        for said in [PadInput::Pressed, PadInput::Up, PadInput::More, PadInput::Back] {
            assert_eq!(Action::Tell(said).does(ButtonPress::Down), Ok(Some(Effect::Tell(said))), "{said:?}");
            assert_eq!(Action::Tell(said).does(ButtonPress::Up), Ok(None), "{said:?}");
        }
    }

    #[test]
    fn a_word_to_the_home_screen_is_not_a_key() {
        for said in console_onscreen::homeward::EVERY {
            assert!(
                matches!(Action::Tell(said).does(ButtonPress::Down), Ok(Some(Effect::Tell(_)) | None)),
                "{said:?} came out as something other than a word"
            );
        }
    }
}
