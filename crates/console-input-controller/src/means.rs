//! What this desktop can do, and what each of those is bound to.  One table,
//! and now it really is one. What a button meant used to be split between an
//! InputPlumber profile, this daemon, and a third place for the one button
//! neither of them knew about; then between two profiles, because a button
//! meant one thing on the desktop and another with a chooser up. Nothing could
//! be asked "what does X do" and answer.  So the profile stopped saying what a
//! button means and started saying only what it is --
//! `console_input_gamepad::routing` -- and everything a press comes to is
//! decided here: the job, when it applies, and what has to be held down with
//! it. The daemon reads it, the setup screen writes to it, and the guide reads
//! it out loud. There is no second copy.  What is in the table is the default
//! and nothing more. Somebody who moves a job onto another button writes that
//! in `~/.config/console/buttons.toml`, and only what they moved is in there: a
//! machine nobody has touched has an empty file and the whole of its answer
//! here.
//!
//! ## Two inputs, one table
//!
//! A keyboard was the last thing outside it. `hyprland.lua` used to carry
//! thirty binds under a comment saying they were "the doings the pad already
//! names, reached by somebody whose hands are on keys" -- which is this table's
//! argument, written in a file this table could not see. So a job is bound on
//! the pad, on a keyboard, or on both, and every one of them is a row here.
//!
//! Which program carries a press out is worked out from the input and is never
//! written down: the daemon matches what arrives off the pad, and a keyboard
//! binding is handed to the compositor as a bind, because the compositor is
//! what owns modifier state and what can swallow a chord before it reaches the
//! window somebody is typing in. Two consequences worth knowing. A job reached
//! from a keyboard has to be one that *runs* something -- `Doing::Run` becomes
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
//! The power key is in here too, and it is the one row whose being in a table
//! is a loss as well as a gain. It was the first line of the lua for a reason:
//! a config that fails to load abandons every line after the failure, and a
//! screen that is off and will not come back is this device at its worst --
//! the machine running, every button working, and none of them able to be seen
//! to. `docs/button-contract.md` carries the rest of that argument and what it
//! now rests on instead.

use evdev::{KeyCode, RelativeAxisCode};

use console_core_never::Never;
use console_onscreen::Said;
use console_input_bindings::bound::{Binding, Fits, Input};
use console_input_bindings::moved::Jobs;

use crate::doing::{Carry, Doing, Out};
use crate::mode::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
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
    Guide,
    Keyboard,
    SwitchLanguage,
    Terminal,
    Files,
    Music,
    Downloads,
    Notices,
    CloseWindow,
    Fullscreen,
    Focus(&'static str),
    Workspace(i32),
    Carry(i32),
    Click,
    MoreOptions,
    Back,
    Up,
    Down,
    Left,
    Right,
    Tell(Said),
    ScrollDown,
    Choose,
    More,
    Tab(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Anywhere,
    OnTheDesktop,
    OnTheHomeScreen,
    StandingOnASquare,
    WithAChooserUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suits {
    InFront,
    Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeats {
    WhileHeld,
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locked {
    EvenThen,
    Awake,
}

impl When {
    pub fn suits(self, mode: Mode) -> Result<Suits, Never> {
        let suits = match self {
            When::Anywhere => true,
            When::OnTheDesktop => matches!(mode, Mode::Desktop | Mode::Home | Mode::Standing),
            When::OnTheHomeScreen => matches!(mode, Mode::Home | Mode::Standing),
            When::StandingOnASquare => mode == Mode::Standing,
            When::WithAChooserUp => mode == Mode::Tabs,
        };

        Ok(match suits {
            true => Suits::InFront,
            false => Suits::Elsewhere,
        })
    }

    fn rank(self) -> Result<u8, Never> {
        Ok(match self {
            When::Anywhere => 0,
            When::OnTheDesktop | When::WithAChooserUp => 1,
            When::OnTheHomeScreen => 2,
            When::StandingOnASquare => 3,
        })
    }

    fn beats(self, other: Self) -> Result<Suits, Never> {
        let Ok(mine) = self.rank();
        let Ok(theirs) = other.rank();

        Ok(match mine > theirs {
            true => Suits::InFront,
            false => Suits::Elsewhere,
        })
    }
}

pub type Bound = (Input, &'static [&'static str], &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Job {
    pub slug: &'static str,
    pub what: What,
    pub when: When,
    pub bound: &'static [Bound],
}

const PAD: Input = Input::Pad;
const KEYS: Input = Input::Keyboard;

const ALONE: &[&str] = &[];
const L2: &[&str] = &["l2"];
const SUPER: &[&str] = &["super"];
const SUPER_CTRL: &[&str] = &["super", "ctrl"];
const SUPER_SHIFT: &[&str] = &["super", "shift"];

pub const JOBS: [Job; 55] = [
    Job {
        slug: "menu",
        what: What::Menu,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "left-paddle-top"), (KEYS, SUPER, "space")],
    },
    Job {
        slug: "dictate",
        what: What::Dictate,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "left-paddle-bottom"), (KEYS, SUPER, "v")],
    },
    Job {
        slug: "put-away",
        what: What::PutAway,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "right-paddle-top")],
    },
    Job {
        slug: "settings",
        what: What::Settings,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "legion-right"), (KEYS, SUPER, "i")],
    },
    Job {
        slug: "guide",
        what: What::Guide,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "menu"), (KEYS, SUPER, "slash")],
    },
    Job {
        slug: "keyboard",
        what: What::Keyboard,
        when: When::Anywhere,
        bound: &[
            (PAD, ALONE, "x"),
            (PAD, ALONE, "keyboard"),
            (KEYS, SUPER, "k"),
            (KEYS, ALONE, "calculator"),
        ],
    },
    Job {
        slug: "switch-language",
        what: What::SwitchLanguage,
        when: When::Anywhere,
        bound: &[(KEYS, SUPER_SHIFT, "space")],
    },
    Job {
        slug: "screenshot",
        what: What::Screenshot,
        when: When::Anywhere,
        bound: &[(PAD, L2, "right-paddle-bottom"), (KEYS, SUPER, "s"), (KEYS, ALONE, "print")],
    },
    Job {
        slug: "scroll-down",
        what: What::ScrollDown,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "right-paddle-bottom")],
    },
    Job {
        slug: "brighter",
        what: What::Brighter,
        when: When::Anywhere,
        bound: &[(PAD, L2, "dpad-right"), (KEYS, ALONE, "brightness-up")],
    },
    Job {
        slug: "dimmer",
        what: What::Dimmer,
        when: When::Anywhere,
        bound: &[(PAD, L2, "dpad-left"), (KEYS, ALONE, "brightness-down")],
    },
    Job {
        slug: "louder",
        what: What::Louder,
        when: When::Anywhere,
        bound: &[(PAD, L2, "dpad-up"), (KEYS, ALONE, "volume-up")],
    },
    Job {
        slug: "quieter",
        what: What::Quieter,
        when: When::Anywhere,
        bound: &[(PAD, L2, "dpad-down"), (KEYS, ALONE, "volume-down")],
    },
    Job {
        slug: "mute",
        what: What::Mute,
        when: When::Anywhere,
        bound: &[(KEYS, ALONE, "mute")],
    },
    Job {
        slug: "wake",
        what: What::Wake,
        when: When::Anywhere,
        bound: &[(KEYS, ALONE, "power")],
    },
    Job {
        slug: "terminal",
        what: What::Terminal,
        when: When::Anywhere,
        bound: &[(KEYS, SUPER, "enter")],
    },
    Job {
        slug: "files",
        what: What::Files,
        when: When::Anywhere,
        bound: &[(KEYS, SUPER, "f")],
    },
    Job {
        slug: "music",
        what: What::Music,
        when: When::Anywhere,
        bound: &[(KEYS, SUPER, "m")],
    },
    Job {
        slug: "downloads",
        what: What::Downloads,
        when: When::Anywhere,
        bound: &[(KEYS, SUPER, "d")],
    },
    Job {
        slug: "notices",
        what: What::Notices,
        when: When::Anywhere,
        bound: &[(KEYS, SUPER, "comma")],
    },
    Job {
        slug: "settings-sound",
        what: What::SettingsAt("Sound"),
        when: When::Anywhere,
        bound: &[(KEYS, SUPER_CTRL, "a")],
    },
    Job {
        slug: "settings-bluetooth",
        what: What::SettingsAt("Bluetooth"),
        when: When::Anywhere,
        bound: &[(KEYS, SUPER_CTRL, "b")],
    },
    Job {
        slug: "settings-wifi",
        what: What::SettingsAt("Wi-Fi"),
        when: When::Anywhere,
        bound: &[(KEYS, SUPER_CTRL, "w")],
    },
    Job {
        slug: "settings-screen",
        what: What::SettingsAt("Screen"),
        when: When::Anywhere,
        bound: &[(KEYS, SUPER_CTRL, "d")],
    },
    Job {
        slug: "settings-power",
        what: What::SettingsAt("Power"),
        when: When::Anywhere,
        bound: &[(KEYS, SUPER_CTRL, "p")],
    },
    Job { slug: "back", what: What::Back, when: When::Anywhere, bound: &[(PAD, ALONE, "b")] },
    Job { slug: "up", what: What::Up, when: When::Anywhere, bound: &[(PAD, ALONE, "dpad-up")] },
    Job {
        slug: "down",
        what: What::Down,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "dpad-down")],
    },
    Job {
        slug: "left",
        what: What::Left,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "dpad-left")],
    },
    Job {
        slug: "right",
        what: What::Right,
        when: When::Anywhere,
        bound: &[(PAD, ALONE, "dpad-right")],
    },
    Job {
        slug: "click",
        what: What::Click,
        when: When::OnTheDesktop,
        bound: &[(PAD, ALONE, "a"), (PAD, ALONE, "r3")],
    },
    Job {
        slug: "more-options",
        what: What::MoreOptions,
        when: When::OnTheDesktop,
        bound: &[(PAD, ALONE, "y")],
    },
    Job {
        slug: "game-mode",
        what: What::GameMode,
        when: When::OnTheDesktop,
        bound: &[(PAD, ALONE, "legion-left"), (KEYS, SUPER, "g")],
    },
    Job {
        slug: "browser",
        what: What::Browser,
        when: When::OnTheDesktop,
        bound: &[(PAD, ALONE, "view"), (KEYS, SUPER, "b")],
    },
    Job {
        slug: "close-window",
        what: What::CloseWindow,
        when: When::OnTheDesktop,
        bound: &[(KEYS, SUPER, "w")],
    },
    Job {
        slug: "fullscreen",
        what: What::Fullscreen,
        when: When::OnTheDesktop,
        bound: &[(KEYS, SUPER_SHIFT, "f")],
    },
    Job {
        slug: "focus-left",
        what: What::Focus("left"),
        when: When::OnTheDesktop,
        bound: &[(KEYS, SUPER, "left")],
    },
    Job {
        slug: "focus-right",
        what: What::Focus("right"),
        when: When::OnTheDesktop,
        bound: &[(KEYS, SUPER, "right")],
    },
    Job {
        slug: "focus-up",
        what: What::Focus("up"),
        when: When::OnTheDesktop,
        bound: &[(KEYS, SUPER, "up")],
    },
    Job {
        slug: "focus-down",
        what: What::Focus("down"),
        when: When::OnTheDesktop,
        bound: &[(KEYS, SUPER, "down")],
    },
    Job {
        slug: "workspace-next",
        what: What::Workspace(1),
        when: When::OnTheDesktop,
        bound: &[(PAD, ALONE, "r1"), (KEYS, SUPER, "tab")],
    },
    Job {
        slug: "workspace-previous",
        what: What::Workspace(-1),
        when: When::OnTheDesktop,
        bound: &[(PAD, ALONE, "l1"), (KEYS, SUPER_SHIFT, "tab")],
    },
    Job {
        slug: "carry-next",
        what: What::Carry(1),
        when: When::OnTheDesktop,
        bound: &[(PAD, L2, "r1"), (KEYS, SUPER_SHIFT, "right")],
    },
    Job {
        slug: "carry-previous",
        what: What::Carry(-1),
        when: When::OnTheDesktop,
        bound: &[(PAD, L2, "l1"), (KEYS, SUPER_SHIFT, "left")],
    },
    Job {
        slug: "home-up",
        what: What::Tell(Said::Up),
        when: When::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-up")],
    },
    Job {
        slug: "home-down",
        what: What::Tell(Said::Down),
        when: When::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-down")],
    },
    Job {
        slug: "home-left",
        what: What::Tell(Said::Left),
        when: When::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-left")],
    },
    Job {
        slug: "home-right",
        what: What::Tell(Said::Right),
        when: When::OnTheHomeScreen,
        bound: &[(PAD, ALONE, "dpad-right")],
    },
    Job {
        slug: "home-choose",
        what: What::Tell(Said::Pressed),
        when: When::StandingOnASquare,
        bound: &[(PAD, ALONE, "a"), (PAD, ALONE, "r3")],
    },
    Job {
        slug: "home-more",
        what: What::Tell(Said::More),
        when: When::StandingOnASquare,
        bound: &[(PAD, ALONE, "y")],
    },
    Job {
        slug: "home-back",
        what: What::Tell(Said::Back),
        when: When::StandingOnASquare,
        bound: &[(PAD, ALONE, "b")],
    },
    Job {
        slug: "choose",
        what: What::Choose,
        when: When::WithAChooserUp,
        bound: &[(PAD, ALONE, "a"), (PAD, ALONE, "r3")],
    },
    Job {
        slug: "more",
        what: What::More,
        when: When::WithAChooserUp,
        bound: &[(PAD, ALONE, "y")],
    },
    Job {
        slug: "tab-right",
        what: What::Tab(1),
        when: When::WithAChooserUp,
        bound: &[(PAD, ALONE, "r1")],
    },
    Job {
        slug: "tab-left",
        what: What::Tab(-1),
        when: When::WithAChooserUp,
        bound: &[(PAD, ALONE, "l1")],
    },
];

pub fn job(slug: &str) -> Result<Option<&'static Job>, Never> {
    let Ok(mut every) = every();

    Ok(every.find(|job| job.slug == slug))
}

pub fn every() -> Result<impl Iterator<Item = &'static Job>, Never> {
    Ok(JOBS.iter())
}

impl What {
    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
            What::Menu => "the menu",
            What::Dictate => "take what is said and type it",
            What::PutAway => "put away whatever is up",
            What::Screenshot => "a screenshot",
            What::Settings => "the settings",
            What::SettingsAt("Sound") => "the sound settings",
            What::SettingsAt("Bluetooth") => "the bluetooth settings",
            What::SettingsAt("Wi-Fi") => "the wi-fi settings",
            What::SettingsAt("Screen") => "the screen settings",
            What::SettingsAt(_) => "the power settings",
            What::Brighter => "screen brighter",
            What::Dimmer => "screen dimmer",
            What::Louder => "louder",
            What::Quieter => "quieter",
            What::Mute => "silence, and sound again",
            What::Wake => "wake the screen",
            What::GameMode => "leave for Steam",
            What::Browser => "the browser",
            What::Guide => "what every button does",
            What::Keyboard => "show or hide the keyboard",
            What::SwitchLanguage => "the next alphabet",
            What::Terminal => "a terminal",
            What::Files => "the files",
            What::Music => "the music",
            What::Downloads => "what has been downloaded",
            What::Notices => "what this machine has said",
            What::CloseWindow => "close the window in front",
            What::Fullscreen => "fill the screen with this window",
            What::Focus("left") => "the window to the left",
            What::Focus("right") => "the window to the right",
            What::Focus("up") => "the window above",
            What::Focus(_) => "the window below",
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
            What::Tell(Said::Up) => "move up the home screen",
            What::Tell(Said::Down) => "move down the home screen",
            What::Tell(Said::Left) => "move left along the home screen",
            What::Tell(Said::Right) => "move right along the home screen",
            What::Tell(Said::More) => "what else can be done with this square",
            What::Tell(Said::Back) => "put down what you are holding, and the highlight away",
            What::Tell(Said::Pressed) => "open the square you are standing on",
            What::Tell(Said::Again) => "read the home screen's own settings again",
            What::Tell(Said::Carry) => "pick up the square you are standing on",
            What::Tell(Said::Off) => "take the square you are standing on off the home screen",
            What::Tab(-1) => "the tab to the left",
            What::Tab(_) => "the tab to the right",
        })
    }

    pub fn repeats(self) -> Result<Repeats, Never> {
        let goes_on = matches!(
            self,
            What::Brighter
                | What::Dimmer
                | What::Louder
                | What::Quieter
                | What::ScrollDown
                | What::Tell(Said::Up | Said::Down | Said::Left | Said::Right)
        );

        Ok(match goes_on {
            true => Repeats::WhileHeld,
            false => Repeats::Once,
        })
    }

    pub fn locked(self) -> Result<Locked, Never> {
        let in_the_dark = matches!(
            self,
            What::Brighter | What::Dimmer | What::Louder | What::Quieter | What::Mute | What::Wake
        );

        Ok(match in_the_dark {
            true => Locked::EvenThen,
            false => Locked::Awake,
        })
    }

    pub fn does(self, down: Press) -> Result<Option<Doing>, Never> {
        match self {
            What::Click => pressed(KeyCode::BTN_LEFT, down),
            What::MoreOptions => pressed(KeyCode::BTN_RIGHT, down),
            What::Back => pressed(KeyCode::KEY_ESC, down),
            What::Up => pressed(KeyCode::KEY_UP, down),
            What::Down => pressed(KeyCode::KEY_DOWN, down),
            What::Left => pressed(KeyCode::KEY_LEFT, down),
            What::Right => pressed(KeyCode::KEY_RIGHT, down),
            What::Choose => pressed(KeyCode::KEY_ENTER, down),
            What::More => pressed(KeyCode::KEY_F18, down),
            What::Tab(-1) => pressed(KeyCode::KEY_PAGEUP, down),
            What::Tab(_) => pressed(KeyCode::KEY_PAGEDOWN, down),
            _ if down == Press::Up => Ok(None),
            What::Tell(said) => Ok(Some(Doing::Tell(said))),
            What::ScrollDown => scrolled(),
            What::Menu => started(&["launcher", "--keep"]),
            What::Dictate => started(&["dictate"]),
            What::PutAway => started(&["put-away"]),
            What::Screenshot => started(&["/usr/local/bin/console-screenshot"]),
            What::Settings => started(&["settings-panel"]),
            What::SettingsAt(tab) => started(&["settings-panel", tab]),
            What::Brighter => started(&["/usr/local/bin/console-brightness", "up"]),
            What::Dimmer => started(&["/usr/local/bin/console-brightness", "down"]),
            What::Louder => started(&["/usr/local/bin/console-volume", "up"]),
            What::Quieter => started(&["/usr/local/bin/console-volume", "down"]),
            What::Mute => started(&["/usr/local/bin/console-volume", "mute"]),
            What::Wake => started(&["/usr/local/bin/console-brightness", "undim"]),
            What::GameMode => started(&["game-mode"]),
            What::Browser => started(&["/usr/local/bin/console-browser"]),
            What::Guide => started(&["/usr/local/bin/console-buttons", "--menu"]),
            What::Keyboard => started(&["keyboard-toggle"]),
            What::SwitchLanguage => started(&["switch-language"]),
            What::Terminal => terminal(),
            What::Files => started(&["/usr/local/bin/files-panel"]),
            What::Music => started(&["/usr/local/bin/music-panel"]),
            What::Downloads => started(&["/usr/local/bin/download-panel"]),
            What::Notices => started(&["/usr/local/bin/notices-panel"]),
            What::CloseWindow => {
                let closes = Doing::dispatch("hl.dsp.window.close()")?;

                Ok(Some(closes))
            }
            What::Fullscreen => {
                let fills = Doing::dispatch("hl.dsp.window.fullscreen()")?;

                Ok(Some(fills))
            }
            What::Focus(way) => {
                let Ok(focus) = Doing::dispatch(&format!("hl.dsp.focus({{direction = \"{way}\"}})"));

                Ok(Some(focus))
            }
            What::Workspace(step) => moved(step, Carry::Nothing),
            What::Carry(step) => moved(step, Carry::Window),
        }
    }
}

fn pressed(code: KeyCode, down: Press) -> Result<Option<Doing>, Never> {
    let value = match down {
        Press::Down => 1,
        Press::Up => 0,
    };
    let Ok(out) = Out::key(code.0, value);

    Ok(Some(Doing::Frame(vec![out])))
}

fn scrolled() -> Result<Option<Doing>, Never> {
    let Ok(out) = Out::rel(RelativeAxisCode::REL_WHEEL.0, -1);

    Ok(Some(Doing::Frame(vec![out])))
}

fn started(argv: &[&str]) -> Result<Option<Doing>, Never> {
    let Ok(run) = Doing::run(argv);

    Ok(Some(run))
}

fn terminal() -> Result<Option<Doing>, Never> {
    let Ok(alacritty) = console_core_external_programs::Program::Alacritty.name();

    started(&[alacritty])
}

fn moved(step: i32, carrying: Carry) -> Result<Option<Doing>, Never> {
    let Ok(moved) = Doing::workspace(&format!("{step:+}"), carrying);

    Ok(Some(moved))
}

pub fn sends() -> Result<Vec<KeyCode>, Never> {
    let Ok(jobs) = every();

    let mut every: Vec<KeyCode> = jobs
        .filter_map(|job| {
            let Ok(does) = job.what.does(Press::Down);

            match does {
                Some(Doing::Frame(frame)) => frame.first().map(|out| KeyCode(out.code)),
                Some(Doing::Run(_) | Doing::Tell(_) | Doing::Using(_)) | None => None,
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
        let Ok(none) = Jobs::none();

        Table::of(&none)
    }

    pub fn of(said: &Jobs) -> Result<Self, Never> {
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

    pub fn every(&self) -> Result<impl Iterator<Item = (&'static Job, &[Binding])>, Never> {
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
    ) -> Result<Option<&'static Job>, Never> {
        let mut found: Option<(&'static Job, usize)> = None;
        let Ok(every) = self.every();

        for (job, bound) in every {
            let Ok(suits) = job.when.suits(mode);

            match suits {
                Suits::Elsewhere => continue,
                Suits::InFront => {},
            }

            let Ok(deepest) = deepest(bound, on, held, pressed);

            let depth = match deepest {
                Some(depth) => depth,
                None => continue,
            };

            found = match found {
                Some((already, was)) if was > depth => Some((already, was)),
                Some((already, was))
                    if was == depth && already.when.beats(job.when) == Ok(Suits::InFront) =>
                {
                    Some((already, was))
                }
                Some(_) | None => Some((job, depth)),
            };
        }

        Ok(found.map(|(job, _)| job))
    }
}

pub fn ours(job: &Job) -> Result<Vec<Binding>, Never> {
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
) -> Result<Option<usize>, Never> {
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

    fn none() -> Jobs {
        ok(Jobs::none())
    }

    fn what(table: &Table, pressed: &str, held: &[&str], mode: Mode) -> Option<What> {
        let Ok(found) = table.what(Input::Pad, held, pressed, mode);

        found.map(|job| job.what)
    }

    fn typed(table: &Table, pressed: &str, held: &[&str]) -> Option<What> {
        let Ok(found) = table.what(Input::Keyboard, held, pressed, Mode::Desktop);

        found.map(|job| job.what)
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
        for mode in [Mode::Desktop, Mode::Tabs, Mode::Home] {
            let mut places: Vec<String> = Vec::new();

            for job in ok(every()).filter(|job| job.when.suits(mode) == Ok(Suits::InFront)) {
                for (on, held, pressed) in job.bound {
                    let Ok(word) = on.word();

                    places.push(format!("{word} {held:?} {pressed} {}", ok(job.when.rank())));
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

            let Ok(does) = job.what.does(Press::Down);

            assert!(
                matches!(does, Some(Doing::Run(_))),
                "{} is on a key and does not run anything",
                job.slug
            );
        }
    }

    #[test]
    fn every_job_says_what_it_is() {
        for job in ok(every()) {
            assert!(!ok(job.what.says()).is_empty(), "{} says nothing", job.slug);
            assert!(job.slug.chars().all(|letter| letter.is_ascii_lowercase() || letter == '-'));
        }
    }

    #[test]
    fn a_button_with_a_second_job_does_that_one_while_l2_is_held() {
        let table = ours();

        assert_eq!(what(&table, "dpad-up", &[], Mode::Desktop), Some(What::Up));
        assert_eq!(what(&table, "dpad-up", &["l2"], Mode::Desktop), Some(What::Louder));
    }

    #[test]
    fn a_button_with_no_second_job_keeps_doing_its_first_one() {
        let table = ours();

        assert_eq!(what(&table, "left-paddle-top", &["l2"], Mode::Desktop), Some(What::Menu));
    }

    #[test]
    fn both_triggers_is_not_either_of_them() {
        let table = ours();

        assert_eq!(what(&table, "dpad-up", &["l2", "r2"], Mode::Desktop), Some(What::Louder));
    }

    #[test]
    fn a_button_can_mean_one_thing_on_the_desktop_and_another_in_a_chooser() {
        let table = ours();

        assert_eq!(what(&table, "a", &[], Mode::Desktop), Some(What::Click));
        assert_eq!(what(&table, "a", &[], Mode::Tabs), Some(What::Choose));
        assert_eq!(what(&table, "r1", &[], Mode::Desktop), Some(What::Workspace(1)));
        assert_eq!(what(&table, "r1", &[], Mode::Tabs), Some(What::Tab(1)));
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

        assert_eq!(typed(&table, "i", &["super"]), Some(What::Settings));
        assert_eq!(typed(&table, "i", &[]), None, "the modifier is part of the place");
        assert_eq!(what(&table, "i", &["super"], Mode::Desktop), None);
    }

    #[test]
    fn the_keys_the_lua_used_to_hold_are_in_the_table() {
        let table = ours();

        assert_eq!(typed(&table, "enter", &["super"]), Some(What::Terminal));
        assert_eq!(typed(&table, "f", &["super"]), Some(What::Files));
        assert_eq!(typed(&table, "w", &["super"]), Some(What::CloseWindow));
        assert_eq!(typed(&table, "f", &["super", "shift"]), Some(What::Fullscreen));
        assert_eq!(typed(&table, "left", &["super"]), Some(What::Focus("left")));
        assert_eq!(typed(&table, "left", &["super", "shift"]), Some(What::Carry(-1)));
        assert_eq!(typed(&table, "a", &["super", "ctrl"]), Some(What::SettingsAt("Sound")));
        assert_eq!(typed(&table, "print", &[]), Some(What::Screenshot));
        assert_eq!(typed(&table, "volume-up", &[]), Some(What::Louder));
    }

    #[test]
    fn the_level_keys_answer_with_the_screen_locked_and_nothing_else_does() {
        assert_eq!(What::Louder.locked(), Ok(Locked::EvenThen));
        assert_eq!(What::Wake.locked(), Ok(Locked::EvenThen));
        assert_eq!(What::Terminal.locked(), Ok(Locked::Awake));
        assert_eq!(What::Settings.locked(), Ok(Locked::Awake));
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
        let down = ok(Out::key(KeyCode::KEY_UP.0, 1));
        let up = ok(Out::key(KeyCode::KEY_UP.0, 0));
        let clicked = ok(Out::key(KeyCode::BTN_LEFT.0, 1));

        assert_eq!(What::Up.does(Press::Down), Ok(Some(Doing::Frame(vec![down]))));
        assert_eq!(What::Up.does(Press::Up), Ok(Some(Doing::Frame(vec![up]))));
        assert_eq!(What::Click.does(Press::Down), Ok(Some(Doing::Frame(vec![clicked]))));
    }

    #[test]
    fn something_that_starts_a_program_happens_once() {
        assert_eq!(What::Menu.does(Press::Down), Ok(Some(ok(Doing::run(&["launcher", "--keep"])))));
        assert_eq!(What::Menu.does(Press::Up), Ok(None));
    }

    #[test]
    fn the_shoulders_move_you_and_carry_the_window_while_l2_is_held() {
        let moved = ok(Doing::workspace("+1", Carry::Nothing));
        let carried = ok(Doing::workspace("-1", Carry::Window));

        assert_eq!(What::Workspace(1).does(Press::Down), Ok(Some(moved)));
        assert_eq!(What::Carry(-1).does(Press::Down), Ok(Some(carried)));
    }

    #[test]
    fn the_keyboard_is_ours() {
        let table = ours();
        let job = ok(table.what(Input::Pad, &[], "x", Mode::Desktop)).expect("x");

        assert_eq!(job.what, What::Keyboard);
        assert_eq!(job.what.does(Press::Down), Ok(Some(ok(Doing::run(&["keyboard-toggle"])))));
        assert_eq!(what(&table, "keyboard", &[], Mode::Desktop), Some(What::Keyboard));
    }

    #[test]
    fn what_somebody_moved_is_where_they_moved_it() {
        let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let table = ok(Table::of(&said));

        assert_eq!(what(&table, "a", &["r2"], Mode::Desktop), Some(What::Screenshot));
        assert_ne!(
            what(&table, "right-paddle-bottom", &["l2"], Mode::Desktop),
            Some(What::Screenshot)
        );
        assert_eq!(what(&table, "dpad-up", &["l2"], Mode::Desktop), Some(What::Louder));
    }

    #[test]
    fn a_job_moved_onto_a_chord_of_two_buttons_is_read_off_the_chord() {
        let said = Jobs::read("[jobs]\nmenu = \"left-paddle-bottom + right-paddle-top\"\n")
            .expect("a table");
        let table = ok(Table::of(&said));

        assert_eq!(
            what(&table, "right-paddle-top", &["left-paddle-bottom"], Mode::Desktop),
            Some(What::Menu)
        );
        assert_eq!(
            what(&table, "right-paddle-top", &[], Mode::Desktop),
            Some(What::PutAway),
            "the button on its own goes on doing what it did"
        );
    }

    #[test]
    fn a_job_left_with_no_button_is_on_no_button() {
        let said = Jobs::read("[jobs]\nmenu = \"\"\n").expect("a table");
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
        let said = Jobs::read("[jobs]\nteleport = \"a\"\n").expect("a table");
        let table = ok(Table::of(&said));

        assert_eq!(what(&table, "a", &[], Mode::Desktop), Some(What::Click));
    }

    #[test]
    fn the_home_screen_takes_the_d_pad_and_leaves_the_rest() {
        let table = ok(Table::of(&none()));

        assert_eq!(what(&table, "dpad-up", &[], Mode::Desktop), Some(What::Up));
        assert_eq!(what(&table, "dpad-up", &[], Mode::Home), Some(What::Tell(Said::Up)));
        assert_eq!(what(&table, "dpad-down", &[], Mode::Home), Some(What::Tell(Said::Down)));
        assert_eq!(what(&table, "dpad-left", &[], Mode::Home), Some(What::Tell(Said::Left)));
        assert_eq!(what(&table, "dpad-right", &[], Mode::Home), Some(What::Tell(Said::Right)));

        assert_eq!(what(&table, "r1", &[], Mode::Home), Some(What::Workspace(1)));
        assert_eq!(what(&table, "l1", &[], Mode::Home), Some(What::Workspace(-1)));
        assert_eq!(what(&table, "legion-left", &[], Mode::Home), Some(What::GameMode));
        assert_eq!(what(&table, "view", &[], Mode::Home), Some(What::Browser));
        assert_eq!(what(&table, "left-paddle-top", &[], Mode::Home), Some(What::Menu));
    }

    #[test]
    fn a_is_the_pointers_button_until_the_home_screen_is_awake() {
        let table = ok(Table::of(&none()));

        assert_eq!(what(&table, "a", &[], Mode::Desktop), Some(What::Click));
        assert_eq!(what(&table, "a", &[], Mode::Home), Some(What::Click));
        assert_eq!(what(&table, "a", &[], Mode::Standing), Some(What::Tell(Said::Pressed)));

        assert_eq!(what(&table, "y", &[], Mode::Desktop), Some(What::MoreOptions));
        assert_eq!(what(&table, "y", &[], Mode::Home), Some(What::MoreOptions));
        assert_eq!(what(&table, "y", &[], Mode::Standing), Some(What::Tell(Said::More)));

        assert_eq!(what(&table, "b", &[], Mode::Home), Some(What::Back));
        assert_eq!(what(&table, "b", &[], Mode::Standing), Some(What::Tell(Said::Back)));
    }

    #[test]
    fn the_d_pad_stays_the_home_screens_once_it_is_awake() {
        let table = ok(Table::of(&none()));

        assert_eq!(what(&table, "dpad-up", &[], Mode::Standing), Some(What::Tell(Said::Up)));
        assert_eq!(what(&table, "r1", &[], Mode::Standing), Some(What::Workspace(1)));
    }

    #[test]
    fn holding_the_d_pad_on_the_home_screen_goes_on_walking_it() {
        for said in [Said::Up, Said::Down, Said::Left, Said::Right] {
            assert_eq!(What::Tell(said).repeats(), Ok(Repeats::WhileHeld), "{said:?}");
        }

        assert_eq!(What::Tell(Said::More).repeats(), Ok(Repeats::Once));
        assert_eq!(What::Tell(Said::Pressed).repeats(), Ok(Repeats::Once));
    }

    #[test]
    fn what_the_home_screen_is_told_is_said_once() {
        for said in [Said::Pressed, Said::Up, Said::More, Said::Back] {
            assert_eq!(What::Tell(said).does(Press::Down), Ok(Some(Doing::Tell(said))), "{said:?}");
            assert_eq!(What::Tell(said).does(Press::Up), Ok(None), "{said:?}");
        }
    }

    #[test]
    fn a_word_to_the_home_screen_is_not_a_key() {
        for said in console_onscreen::homeward::EVERY {
            assert!(
                matches!(What::Tell(said).does(Press::Down), Ok(Some(Doing::Tell(_)) | None)),
                "{said:?} came out as something other than a word"
            );
        }
    }
}
