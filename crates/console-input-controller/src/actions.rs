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
//! to. The `button-contract.md` note in Orbit carries the rest of that argument
//! and what it now rests on instead.

use std::borrow::Cow;

use console_input_event_devices::{KeyCode, RelativeAxisCode};

use console_core_internal_programs::{
  InternalProgram, LAUNCHER_KEEP, OVERVIEW_SHOW, SELECT_SCREENSHOT_AREA,
};
use console_core_never::Never;
use console_input_bindings::bound::{Binding, Fits, Input};
use console_input_bindings::moved::Tasks;
use console_onscreen::PadInput;

use crate::effect::{CompositorAction, Direction, Effect, Output, Payload, WorkspaceTarget};
use crate::mode::Mode;
use console_compositor::{TileChange, TileColumn, TileEdge};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
  Menu,
  Dictate,
  PutAway,
  Screenshot,
  ScreenshotArea,
  Settings,
  SettingsAt(&'static str),
  Brighter,
  Dimmer,
  Louder,
  Quieter,
  Mute,
  MicMute,
  PowerProfile,
  Media(Media),
  Touchpad,
  Wake,
  GameMode,
  Browser,
  ButtonGuide,
  Keyboard,
  Language(i32),
  Terminal,
  Files,
  Music,
  QuickWorkspace,
  Downloads,
  Notifications,
  Clipboard,
  TaskManager,
  FocusBar,
  FocusPopups,
  CloseWindow,
  Fullscreen,
  Focus(Direction),
  Tile(TileChange),
  Workspace(i32),
  Place(i64),
  Payload(i32),
  CarryTo(i64),
  Overview,
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
pub enum Media {
  PlayPause,
  Next,
  Previous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
  Anywhere,
  OnTheDesktop,
  OnTheHomeScreen,
  StandingOnASquare,
  WithAPanelOrApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonPress {
  Down,
  Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
  Available,
  Unavailable,
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
  pub fn availability(self, mode: Mode) -> Result<Availability, Never> {
    let fit = match self {
      Context::Anywhere => true,
      Context::OnTheDesktop => matches!(
        mode,
        Mode::Desktop | Mode::App | Mode::HomeScreen | Mode::Standing
      ),
      Context::OnTheHomeScreen => matches!(mode, Mode::HomeScreen | Mode::Standing),
      Context::StandingOnASquare => mode == Mode::Standing,
      Context::WithAPanelOrApp => matches!(mode, Mode::Tabs | Mode::App),
    };

    Ok(match ::core::primitive::bool::then_some(fit, ()) {
      ::core::option::Option::Some(()) => Availability::Available,
      ::core::option::Option::None => Availability::Unavailable,
    })
  }

  fn rank(self) -> Result<u8, Never> {
    Ok(match self {
      Context::Anywhere => 0,
      Context::OnTheDesktop => 1,
      Context::OnTheHomeScreen | Context::WithAPanelOrApp => 2,
      Context::StandingOnASquare => 3,
    })
  }

  fn beats(self, other: Self) -> Result<Availability, Never> {
    let Ok(mine) = self.rank();
    let Ok(theirs) = other.rank();

    Ok(
      match ::core::primitive::bool::then_some(mine > theirs, ()) {
        ::core::option::Option::Some(()) => Availability::Available,
        ::core::option::Option::None => Availability::Unavailable,
      },
    )
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
const L2_R2: &[&str] = &["l2", "r2"];
const SUPER: &[&str] = &["super"];
const SUPER_CONTROL: &[&str] = &["super", "ctrl"];
const SUPER_SHIFT: &[&str] = &["super", "shift"];
const SUPER_ALT: &[&str] = &["super", "alt"];
const SUPER_ALT_SHIFT: &[&str] = &["super", "alt", "shift"];

pub const JOBS: [Task; 103] = [
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
    slug: "clipboard-history",
    action: Action::Clipboard,
    context: Context::Anywhere,
    bound: &[(PAD, L2, "left-paddle-bottom"), (KEYS, SUPER_SHIFT, "v")],
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
    bound: &[(PAD, L2, "right-paddle-bottom"), (KEYS, SUPER, "s")],
  },
  Task {
    slug: "screenshot-area",
    action: Action::ScreenshotArea,
    context: Context::Anywhere,
    bound: &[
      (PAD, L2, "right-paddle-top"),
      (KEYS, SUPER_SHIFT, "s"),
      (KEYS, ALONE, "print"),
      (KEYS, ALONE, "selective-screenshot"),
    ],
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
    slug: "mic-mute",
    action: Action::MicMute,
    context: Context::Anywhere,
    bound: &[(KEYS, ALONE, "mic-mute")],
  },
  Task {
    slug: "power-profile",
    action: Action::PowerProfile,
    context: Context::Anywhere,
    bound: &[(KEYS, ALONE, "performance"), (KEYS, SUPER, "f8")],
  },
  Task {
    slug: "play-pause",
    action: Action::Media(Media::PlayPause),
    context: Context::Anywhere,
    bound: &[(KEYS, ALONE, "play-pause")],
  },
  Task {
    slug: "next-track",
    action: Action::Media(Media::Next),
    context: Context::Anywhere,
    bound: &[(KEYS, ALONE, "next-track")],
  },
  Task {
    slug: "previous-track",
    action: Action::Media(Media::Previous),
    context: Context::Anywhere,
    bound: &[(KEYS, ALONE, "previous-track")],
  },
  Task {
    slug: "toggle-touchpad",
    action: Action::Touchpad,
    context: Context::Anywhere,
    bound: &[(KEYS, ALONE, "touchpad-toggle")],
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
    slug: "quick-workspace",
    action: Action::QuickWorkspace,
    context: Context::Anywhere,
    bound: &[(PAD, L2, "y"), (KEYS, SUPER_SHIFT, "m")],
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
    slug: "focus-bar",
    action: Action::FocusBar,
    context: Context::Anywhere,
    bound: &[(PAD, L2, "menu"), (KEYS, SUPER, "period")],
  },
  Task {
    slug: "focus-popups",
    action: Action::FocusPopups,
    context: Context::Anywhere,
    bound: &[(PAD, L2, "x"), (KEYS, SUPER_SHIFT, "comma")],
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
    bound: &[(KEYS, SUPER_CONTROL, "d"), (KEYS, ALONE, "display-switch")],
  },
  Task {
    slug: "settings-power",
    action: Action::SettingsAt("Battery"),
    context: Context::Anywhere,
    bound: &[(KEYS, SUPER_CONTROL, "p")],
  },
  Task {
    slug: "back",
    action: Action::Back,
    context: Context::Anywhere,
    bound: &[(PAD, ALONE, "b")],
  },
  Task {
    slug: "up",
    action: Action::Up,
    context: Context::Anywhere,
    bound: &[(PAD, ALONE, "dpad-up")],
  },
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
    action: Action::Focus(Direction::Left),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "left"), (KEYS, SUPER, "h")],
  },
  Task {
    slug: "focus-right",
    action: Action::Focus(Direction::Right),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "right"), (KEYS, SUPER, "l")],
  },
  Task {
    slug: "focus-up",
    action: Action::Focus(Direction::Up),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "up"), (KEYS, SUPER, "k")],
  },
  Task {
    slug: "focus-down",
    action: Action::Focus(Direction::Down),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "down"), (KEYS, SUPER, "j")],
  },
  Task {
    slug: "swap-window-left",
    action: Action::Tile(TileChange::Swap(Direction::Left)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT, "h")],
  },
  Task {
    slug: "resize-window-left",
    action: Action::Tile(TileChange::Resize(Direction::Left)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_CONTROL, "h")],
  },
  Task {
    slug: "swap-window-down",
    action: Action::Tile(TileChange::Swap(Direction::Down)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT, "j")],
  },
  Task {
    slug: "resize-window-down",
    action: Action::Tile(TileChange::Resize(Direction::Down)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_CONTROL, "j")],
  },
  Task {
    slug: "swap-window-up",
    action: Action::Tile(TileChange::Swap(Direction::Up)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT, "k")],
  },
  Task {
    slug: "resize-window-up",
    action: Action::Tile(TileChange::Resize(Direction::Up)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_CONTROL, "k")],
  },
  Task {
    slug: "swap-window-right",
    action: Action::Tile(TileChange::Swap(Direction::Right)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT, "l")],
  },
  Task {
    slug: "resize-window-right",
    action: Action::Tile(TileChange::Resize(Direction::Right)),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_CONTROL, "l")],
  },
  Task {
    slug: "tile-above-left",
    action: Action::Tile(TileChange::Place {
      column: TileColumn::Left,
      edge: TileEdge::Above,
    }),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT_SHIFT, "u")],
  },
  Task {
    slug: "tile-above-right",
    action: Action::Tile(TileChange::Place {
      column: TileColumn::Right,
      edge: TileEdge::Above,
    }),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT_SHIFT, "i")],
  },
  Task {
    slug: "tile-below-left",
    action: Action::Tile(TileChange::Place {
      column: TileColumn::Left,
      edge: TileEdge::Below,
    }),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT_SHIFT, "j")],
  },
  Task {
    slug: "tile-below-right",
    action: Action::Tile(TileChange::Place {
      column: TileColumn::Right,
      edge: TileEdge::Below,
    }),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT_SHIFT, "k")],
  },
  Task {
    slug: "move-window-to-root",
    action: Action::Tile(TileChange::MoveToRoot),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT_SHIFT, "space")],
  },
  Task {
    slug: "toggle-tile-split",
    action: Action::Tile(TileChange::ToggleSplit),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_ALT, "space")],
  },
  Task {
    slug: "workspace-next",
    action: Action::Workspace(1),
    context: Context::OnTheDesktop,
    bound: &[(PAD, ALONE, "r1"), (PAD, L2, "r1"), (KEYS, SUPER, "tab")],
  },
  Task {
    slug: "workspace-previous",
    action: Action::Workspace(-1),
    context: Context::OnTheDesktop,
    bound: &[
      (PAD, ALONE, "l1"),
      (PAD, L2, "l1"),
      (KEYS, SUPER_SHIFT, "tab"),
    ],
  },
  Task {
    slug: "place-one",
    action: Action::Place(1),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "1")],
  },
  Task {
    slug: "place-two",
    action: Action::Place(2),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "2")],
  },
  Task {
    slug: "place-three",
    action: Action::Place(3),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "3")],
  },
  Task {
    slug: "place-four",
    action: Action::Place(4),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "4")],
  },
  Task {
    slug: "place-five",
    action: Action::Place(5),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "5")],
  },
  Task {
    slug: "place-six",
    action: Action::Place(6),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "6")],
  },
  Task {
    slug: "place-seven",
    action: Action::Place(7),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "7")],
  },
  Task {
    slug: "place-eight",
    action: Action::Place(8),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "8")],
  },
  Task {
    slug: "place-nine",
    action: Action::Place(9),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "9")],
  },
  Task {
    slug: "place-ten",
    action: Action::Place(10),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER, "0")],
  },
  Task {
    slug: "carry-next",
    action: Action::Payload(1),
    context: Context::OnTheDesktop,
    bound: &[
      (PAD, L2_R2, "r1"),
      (KEYS, SUPER_SHIFT, "right"),
      (KEYS, SUPER_SHIFT, "l"),
    ],
  },
  Task {
    slug: "carry-previous",
    action: Action::Payload(-1),
    context: Context::OnTheDesktop,
    bound: &[
      (PAD, L2_R2, "l1"),
      (KEYS, SUPER_SHIFT, "left"),
      (KEYS, SUPER_SHIFT, "h"),
    ],
  },
  Task {
    slug: "overview",
    action: Action::Overview,
    context: Context::OnTheDesktop,
    bound: &[(PAD, L2, "view"), (KEYS, SUPER, "o")],
  },
  Task {
    slug: "task-manager",
    action: Action::TaskManager,
    context: Context::Anywhere,
    bound: &[(KEYS, SUPER, "escape")],
  },
  Task {
    slug: "carry-to-one",
    action: Action::CarryTo(1),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "1")],
  },
  Task {
    slug: "carry-to-two",
    action: Action::CarryTo(2),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "2")],
  },
  Task {
    slug: "carry-to-three",
    action: Action::CarryTo(3),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "3")],
  },
  Task {
    slug: "carry-to-four",
    action: Action::CarryTo(4),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "4")],
  },
  Task {
    slug: "carry-to-five",
    action: Action::CarryTo(5),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "5")],
  },
  Task {
    slug: "carry-to-six",
    action: Action::CarryTo(6),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "6")],
  },
  Task {
    slug: "carry-to-seven",
    action: Action::CarryTo(7),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "7")],
  },
  Task {
    slug: "carry-to-eight",
    action: Action::CarryTo(8),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "8")],
  },
  Task {
    slug: "carry-to-nine",
    action: Action::CarryTo(9),
    context: Context::OnTheDesktop,
    bound: &[(KEYS, SUPER_SHIFT, "9")],
  },
  Task {
    slug: "carry-to-ten",
    action: Action::CarryTo(10),
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
    context: Context::WithAPanelOrApp,
    bound: &[(PAD, ALONE, "a"), (PAD, ALONE, "r3")],
  },
  Task {
    slug: "more",
    action: Action::More,
    context: Context::WithAPanelOrApp,
    bound: &[(PAD, ALONE, "y")],
  },
  Task {
    slug: "tab-right",
    action: Action::Tab(1),
    context: Context::WithAPanelOrApp,
    bound: &[(PAD, ALONE, "r1")],
  },
  Task {
    slug: "tab-left",
    action: Action::Tab(-1),
    context: Context::WithAPanelOrApp,
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
      Action::ScreenshotArea => "take a screenshot of an area",
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
      Action::MicMute => "mute and unmute the microphone",
      Action::PowerProfile => "cycle power modes",
      Action::Media(Media::PlayPause) => "play or pause media",
      Action::Media(Media::Next) => "play the next track",
      Action::Media(Media::Previous) => "play the previous track",
      Action::Touchpad => "enable or disable the touchpad",
      Action::Wake => "wake the screen",
      Action::GameMode => "switch to Steam Game Mode",
      Action::Browser => "open the browser",
      Action::Overview => "show every desktop",
      Action::ButtonGuide => "show the button guide",
      Action::Keyboard => "show or hide the keyboard",
      Action::Language(-1) => "previous keyboard",
      Action::Language(_) => "next keyboard",
      Action::Terminal => "open Terminal",
      Action::Files => "open Files",
      Action::Music => "open Music",
      Action::QuickWorkspace => "show or hide Quick Workspace",
      Action::Downloads => "open Downloads",
      Action::Notifications => "show notifications",
      Action::Clipboard => "open clipboard history",
      Action::TaskManager => "open Task Manager",
      Action::FocusBar => "focus the desktop bar",
      Action::FocusPopups => "focus notification popups",
      Action::CloseWindow => "close the window",
      Action::Fullscreen => "full screen on or off",
      Action::Focus(Direction::Left) => "switch to the window on the left",
      Action::Focus(Direction::Right) => "switch to the window on the right",
      Action::Focus(Direction::Up) => "switch to the window above",
      Action::Focus(Direction::Down) => "switch to the window below",
      Action::Tile(TileChange::Swap(Direction::Left)) => "swap with the window on the left",
      Action::Tile(TileChange::Swap(Direction::Right)) => "swap with the window on the right",
      Action::Tile(TileChange::Swap(Direction::Up)) => "swap with the window above",
      Action::Tile(TileChange::Swap(Direction::Down)) => "swap with the window below",
      Action::Tile(TileChange::Resize(Direction::Left)) => "make the window narrower",
      Action::Tile(TileChange::Resize(Direction::Right)) => "make the window wider",
      Action::Tile(TileChange::Resize(Direction::Up)) => "make the window shorter",
      Action::Tile(TileChange::Resize(Direction::Down)) => "make the window taller",
      Action::Tile(TileChange::Place {
        column: TileColumn::Left,
        edge: TileEdge::Above,
      }) => "move the window to the top of the left stack",
      Action::Tile(TileChange::Place {
        column: TileColumn::Left,
        edge: TileEdge::Below,
      }) => "move the window to the bottom of the left stack",
      Action::Tile(TileChange::Place {
        column: TileColumn::Right,
        edge: TileEdge::Above,
      }) => "move the window to the top of the right stack",
      Action::Tile(TileChange::Place {
        column: TileColumn::Right,
        edge: TileEdge::Below,
      }) => "move the window to the bottom of the right stack",
      Action::Tile(TileChange::MoveToRoot) => "move the window out of its stack",
      Action::Tile(TileChange::ToggleSplit) => "switch between side-by-side and stacked tiles",
      Action::Workspace(-1) => "previous desktop",
      Action::Workspace(_) => "next desktop",
      Action::Place(1) => "go to desktop 1",
      Action::Place(2) => "go to desktop 2",
      Action::Place(3) => "go to desktop 3",
      Action::Place(4) => "go to desktop 4",
      Action::Place(5) => "go to desktop 5",
      Action::Place(6) => "go to desktop 6",
      Action::Place(7) => "go to desktop 7",
      Action::Place(8) => "go to desktop 8",
      Action::Place(9) => "go to desktop 9",
      Action::Place(_) => "go to desktop 10",
      Action::Payload(-1) => "move the window to the previous desktop",
      Action::Payload(_) => "move the window to the next desktop",
      Action::CarryTo(1) => "move the window to desktop 1",
      Action::CarryTo(2) => "move the window to desktop 2",
      Action::CarryTo(3) => "move the window to desktop 3",
      Action::CarryTo(4) => "move the window to desktop 4",
      Action::CarryTo(5) => "move the window to desktop 5",
      Action::CarryTo(6) => "move the window to desktop 6",
      Action::CarryTo(7) => "move the window to desktop 7",
      Action::CarryTo(8) => "move the window to desktop 8",
      Action::CarryTo(9) => "move the window to desktop 9",
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
        | Action::Tile(TileChange::Resize(_))
        | Action::Tell(PadInput::Up | PadInput::Down | PadInput::Left | PadInput::Right)
    );

    Ok(match ::core::primitive::bool::then_some(goes_on, ()) {
      ::core::option::Option::Some(()) => RepeatMode::WhileHeld,
      ::core::option::Option::None => RepeatMode::Once,
    })
  }

  pub fn locked(self) -> Result<LockBehavior, Never> {
    let in_the_dark = matches!(
      self,
      Action::Brighter
        | Action::Dimmer
        | Action::Louder
        | Action::Quieter
        | Action::Mute
        | Action::MicMute
        | Action::Media(_)
        | Action::Touchpad
        | Action::Wake
    );

    Ok(match ::core::primitive::bool::then_some(in_the_dark, ()) {
      ::core::option::Option::Some(()) => LockBehavior::EvenThen,
      ::core::option::Option::None => LockBehavior::Unlocked,
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
      | Action::ScreenshotArea
      | Action::Settings
      | Action::SettingsAt(_)
      | Action::Brighter
      | Action::Dimmer
      | Action::Louder
      | Action::Quieter
      | Action::Mute
      | Action::MicMute
      | Action::PowerProfile
      | Action::Media(_)
      | Action::Touchpad
      | Action::Wake
      | Action::GameMode
      | Action::Browser
      | Action::ButtonGuide
      | Action::Keyboard
      | Action::Language(_)
      | Action::Terminal
      | Action::Files
      | Action::Music
      | Action::QuickWorkspace
      | Action::Downloads
      | Action::Notifications
      | Action::Clipboard
      | Action::TaskManager
      | Action::FocusBar
      | Action::FocusPopups
      | Action::CloseWindow
      | Action::Fullscreen
      | Action::Focus(_)
      | Action::Tile(_)
      | Action::Workspace(_)
      | Action::Place(_)
      | Action::Payload(_)
      | Action::CarryTo(_)
      | Action::Overview => match down {
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
      Action::Menu => run_by_name(InternalProgram::Launcher, &[LAUNCHER_KEEP.spelling]),
      Action::Dictate => run_by_name(InternalProgram::Dictate, &[]),
      Action::PutAway => run_by_name(InternalProgram::PutAway, &[]),
      Action::Screenshot => run_by_path(InternalProgram::Screenshot, &[]),
      Action::ScreenshotArea => run_by_path(
        InternalProgram::ControlCenter,
        &[SELECT_SCREENSHOT_AREA.spelling],
      ),
      Action::Settings => run_by_name(InternalProgram::SettingsPanel, &[]),
      Action::SettingsAt(tab) => run_by_name(InternalProgram::SettingsPanel, &[tab]),
      Action::Brighter => run_by_path(InternalProgram::Brightness, &["up"]),
      Action::Dimmer => run_by_path(InternalProgram::Brightness, &["down"]),
      Action::Louder => run_by_path(InternalProgram::Volume, &["up"]),
      Action::Quieter => run_by_path(InternalProgram::Volume, &["down"]),
      Action::Mute => run_by_path(InternalProgram::Volume, &["mute"]),
      Action::MicMute => run_by_path(InternalProgram::Volume, &["mic-mute"]),
      Action::PowerProfile => run_by_path(InternalProgram::Hotkey, &["power-profile"]),
      Action::Media(Media::PlayPause) => run_by_path(InternalProgram::Hotkey, &["play-pause"]),
      Action::Media(Media::Next) => run_by_path(InternalProgram::Hotkey, &["next-track"]),
      Action::Media(Media::Previous) => run_by_path(InternalProgram::Hotkey, &["previous-track"]),
      Action::Touchpad => run_by_path(InternalProgram::Hotkey, &["toggle-touchpad"]),
      Action::Wake => run_by_path(InternalProgram::Brightness, &["undim"]),
      Action::GameMode => run_by_name(InternalProgram::SessionGame, &[]),
      Action::Browser => run_by_path(InternalProgram::Browser, &[]),
      Action::Overview => run_by_name(InternalProgram::Overview, &[OVERVIEW_SHOW.spelling]),
      Action::ButtonGuide => run_by_path(InternalProgram::MappingPanel, &[]),
      Action::Keyboard => run_by_name(InternalProgram::KeyboardToggle, &[]),
      Action::Language(-1) => run_by_name(
        InternalProgram::LanguageSwitch,
        &[console_input_language::BACK.spelling],
      ),
      Action::Language(_) => run_by_name(InternalProgram::LanguageSwitch, &[]),
      Action::Terminal => terminal(),
      Action::Files => run_by_path(InternalProgram::Files, &[]),
      Action::Music => {
        let Ok(library) = InternalProgram::Music.path();
        let Ok(panel) = InternalProgram::MusicPanel.path();
        run_by_path(
          InternalProgram::Compositor,
          &["quick-music", library, panel],
        )
      }
      Action::QuickWorkspace => run_by_path(InternalProgram::Compositor, &["quick-workspace"]),
      Action::Downloads => run_by_path(InternalProgram::Downloads, &[]),
      Action::Notifications => run_by_path(InternalProgram::NotificationsPanel, &[]),
      Action::Clipboard => run_by_path(InternalProgram::Clipboard, &[]),
      Action::TaskManager => run_by_path(InternalProgram::TaskManager, &[]),
      Action::FocusBar => run_by_path(InternalProgram::Bar, &[console_onscreen::focus::FLAG]),
      Action::FocusPopups => {
        run_by_path(InternalProgram::Notify, &[console_onscreen::focus::FLAG])
      }
      Action::CloseWindow => Ok(Some(Effect::Dispatch(CompositorAction::CloseWindow))),
      Action::Fullscreen => Ok(Some(Effect::Dispatch(CompositorAction::ToggleFullscreen))),
      Action::Focus(way) => Ok(Some(Effect::Dispatch(CompositorAction::Focus(way)))),
      Action::Tile(change) => Ok(Some(Effect::Dispatch(CompositorAction::Tile(change)))),
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

  let words: Vec<&str> = std::iter::once(named)
    .chain(arguments.iter().copied())
    .collect();

  started(&words)
}

fn run_by_path(program: InternalProgram, arguments: &[&str]) -> Result<Option<Effect>, Never> {
  let Ok(at) = program.path();

  let words: Vec<&str> = std::iter::once(at)
    .chain(arguments.iter().copied())
    .collect();

  started(&words)
}

fn terminal() -> Result<Option<Effect>, Never> {
  let Ok(alacritty) = console_core_external_programs::Program::Alacritty.name();

  started(&[alacritty])
}

fn moved(step: i32, carrying: Payload) -> Result<Option<Effect>, Never> {
  let Ok(moved) = Effect::workspace(WorkspaceTarget::Relative(step), carrying);

  Ok(Some(moved))
}

fn at(number: i64, carrying: Payload) -> Result<Option<Effect>, Never> {
  let Ok(gone) = Effect::workspace(WorkspaceTarget::Number(number), carrying);

  Ok(Some(gone))
}

pub fn sends() -> Result<Vec<KeyCode>, Never> {
  let Ok(jobs) = every();

  let mut every: Vec<KeyCode> = jobs
    .filter_map(|job| {
      let Ok(does) = job.action.does(ButtonPress::Down);

      match does {
        Some(Effect::Frame(frame)) => frame.first().map(|out| KeyCode(out.code)),
        Some(
          Effect::Run(_)
          | Effect::Dispatch(_)
          | Effect::Tell(_)
          | Effect::Using(_)
          | Effect::Reconnected(_),
        )
        | None => None,
      }
    })
    .collect();

  every.sort_unstable_by_key(|key| key.0);
  every.dedup();

  Ok(every)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappedAction {
  Builtin(Action),
  SpecialWorkspace(String),
}

impl MappedAction {
  pub fn says(&self) -> Result<Cow<'static, str>, Never> {
    Ok(match self {
      Self::Builtin(action) => {
        let Ok(says) = action.says();
        Cow::Borrowed(says)
      }
      Self::SpecialWorkspace(name) => {
        Cow::Owned(format!("show or hide special workspace {name}"))
      }
    })
  }

  pub fn locked(&self) -> Result<LockBehavior, Never> {
    match self {
      Self::Builtin(action) => action.locked(),
      Self::SpecialWorkspace(_) => Ok(LockBehavior::Unlocked),
    }
  }

  pub fn repeats(&self) -> Result<RepeatMode, Never> {
    match self {
      Self::Builtin(action) => action.repeats(),
      Self::SpecialWorkspace(_) => Ok(RepeatMode::Once),
    }
  }

  pub fn does(&self, press: ButtonPress) -> Result<Option<Effect>, Never> {
    match (self, press) {
      (Self::Builtin(action), _) => action.does(press),
      (Self::SpecialWorkspace(_), ButtonPress::Up) => Ok(None),
      (Self::SpecialWorkspace(name), ButtonPress::Down) => run_by_path(
        InternalProgram::Compositor,
        &[console_compositor::quick_workspace::TOGGLE_NAMED, name],
      ),
    }
  }
}

const SPECIAL_WORKSPACE_PREFIX: &str = "special-workspace-";

fn special_name(name: &str) -> Result<Option<&str>, Never> {
  Ok(bool::then_some(
    !name.is_empty()
      && name != console_compositor::quick_workspace::NAME
      && !name.chars().any(char::is_control),
    name,
  ))
}

pub fn special_workspace_job(name: &str) -> Result<Option<String>, Never> {
  let trimmed = name.trim();
  let workspace = match trimmed.strip_prefix("special:") {
    Some(workspace) => workspace,
    None => trimmed,
  };
  let Ok(valid) = special_name(workspace);

  Ok(valid.map(|named| format!("{SPECIAL_WORKSPACE_PREFIX}{named}")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedTask {
  pub slug: Cow<'static, str>,
  pub action: MappedAction,
  pub context: Context,
  defaults: &'static [Bound],
}

impl MappedTask {
  pub fn defaults(&self) -> Result<Vec<Binding>, Never> {
    bindings_of(self.defaults)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
  bound: Vec<(MappedTask, Vec<Binding>)>,
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

    let builtin = jobs.map(|job| {
      let Ok(given) = said.bound(job.slug);

      let bound = match given {
        Some(moved) => moved.to_vec(),
        None => {
          let Ok(ours) = ours(job);

          ours
        }
      };

      (
        MappedTask {
          slug: Cow::Borrowed(job.slug),
          action: MappedAction::Builtin(job.action),
          context: job.context,
          defaults: job.bound,
        },
        bound,
      )
    });
    let special = said.moved.iter().filter_map(|(slug, bindings)| {
      let suffix = slug.strip_prefix(SPECIAL_WORKSPACE_PREFIX);
      let name = suffix?;
      let Ok(valid) = special_name(name);
      let workspace = valid?;

      Some((
        MappedTask {
          slug: Cow::Owned(slug.clone()),
          action: MappedAction::SpecialWorkspace(workspace.to_owned()),
          context: Context::Anywhere,
          defaults: &[],
        },
        bindings.clone(),
      ))
    });

    Ok(Table {
      bound: builtin.chain(special).collect(),
    })
  }

  pub fn bindings(&self, slug: &str) -> Result<&[Binding], Never> {
    Ok(
      self
        .bound
        .iter()
        .find(|(job, _)| job.slug == slug)
        .map_or(&[], |(_, bound)| bound.as_slice()),
    )
  }

  pub fn every(&self) -> Result<impl Iterator<Item = (&MappedTask, &[Binding])>, Never> {
    Ok(
      self
        .bound
        .iter()
        .map(|(job, bound)| (job, bound.as_slice())),
    )
  }

  pub fn what(
    &self,
    on: Input,
    held: &[&str],
    pressed: &str,
    mode: Mode,
  ) -> Result<Option<&MappedTask>, Never> {
    let mut found: Option<(&MappedTask, u32)> = None;
    let Ok(every) = self.every();

    for (job, bound) in every {
      let Ok(applicability) = job.context.availability(mode);

      match applicability {
        Availability::Unavailable => continue,
        Availability::Available => {}
      }

      let Ok(deepest) = deepest(bound, on, held, pressed);

      let depth = match deepest {
        Some(depth) => depth,
        None => continue,
      };

      let keep = match found {
        Some((already, was)) => {
          let Ok(beats) = already.context.beats(job.context);

          was > depth || (was == depth && beats == Availability::Available)
        }
        None => false,
      };

      match ::core::primitive::bool::then_some(keep, ()) {
        ::core::option::Option::Some(()) => continue,
        ::core::option::Option::None => {}
      }

      found = Some((job, depth));
    }

    Ok(found.map(|(job, _)| job))
  }
}

pub fn ours(job: &Task) -> Result<Vec<Binding>, Never> {
  bindings_of(job.bound)
}

fn bindings_of(bound: &[Bound]) -> Result<Vec<Binding>, Never> {
  Ok(
    bound
      .iter()
      .map(|(on, held, pressed)| {
        let Ok(binding) = Binding::chord(*on, held, pressed);

        binding
      })
      .collect(),
  )
}

fn deepest(
  bound: &[Binding],
  on: Input,
  held: &[&str],
  pressed: &str,
) -> Result<Option<u32>, Never> {
  Ok(
    bound
      .iter()
      .filter(|one| {
        let Ok(fits) = one.fits(on, held, pressed);

        fits == Fits::Yes
      })
      .map(|one| {
        let Ok(depth) = one.depth();

        depth
      })
      .max(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_input_bindings::bound::Played;
  use console_input_gamepad::vocabulary::button_name;
  use std::collections::BTreeSet;

  type Failure = Box<dyn std::error::Error>;

  fn what(
    table: &Table,
    pressed: &str,
    held: &[&str],
    mode: Mode,
  ) -> Result<Option<Action>, Never> {
    let Ok(found) = table.what(Input::Pad, held, pressed, mode);

    Ok(found.and_then(|job| match job.action {
      MappedAction::Builtin(action) => Some(action),
      MappedAction::SpecialWorkspace(_) => None,
    }))
  }

  fn typed(table: &Table, pressed: &str, held: &[&str]) -> Result<Option<Action>, Never> {
    let Ok(found) = table.what(Input::Keyboard, held, pressed, Mode::Desktop);

    Ok(found.and_then(|job| match job.action {
      MappedAction::Builtin(action) => Some(action),
      MappedAction::SpecialWorkspace(_) => None,
    }))
  }

  fn stated(written: &str) -> Result<Table, Failure> {
    let said = Tasks::read(written)?;
    let Ok(table) = Table::of(&said);

    Ok(table)
  }

  fn unmoved() -> Result<Table, Never> {
    let Ok(none) = Tasks::none();

    Table::of(&none)
  }

  #[test]
  fn nothing_is_written_down_twice() {
    let Ok(every) = every();
    let mut seen = BTreeSet::new();
    let mut twice: Vec<&str> = Vec::new();

    for job in every {
      match ::core::primitive::bool::then_some(seen.insert(job.slug), ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => twice.push(job.slug),
      }
    }

    assert!(twice.is_empty(), "two jobs are called {twice:?}");
  }

  #[test]
  fn nothing_is_bound_twice_in_one_place() {
    for mode in [Mode::Desktop, Mode::Tabs, Mode::App, Mode::HomeScreen] {
      let mut places: Vec<String> = Vec::new();

      let Ok(every) = every();

      for job in every.filter(|job| job.context.availability(mode) == Ok(Availability::Available))
      {
        let Ok(rank) = job.context.rank();

        for (on, held, pressed) in job.bound {
          let Ok(word) = on.word();

          places.push(format!("{word} {held:?} {pressed} {rank}"));
        }
      }

      let mut seen = BTreeSet::new();
      let mut twice: Vec<String> = Vec::new();

      for place in places {
        match ::core::primitive::bool::then_some(seen.contains(&place), ()) {
          ::core::option::Option::Some(()) => twice.push(place),
          ::core::option::Option::None => {
            seen.insert(place);
          }
        }
      }

      assert!(
        twice.is_empty(),
        "two jobs in one place in {mode:?}: {twice:?}"
      );
    }
  }

  #[test]
  fn every_default_is_on_something_this_desktop_can_read() -> Result<(), Failure> {
    let Ok(every) = every();

    for job in every {
      for (on, held, pressed) in job.bound {
        let Ok(binding) = Binding::chord(*on, held, pressed);

        console_input_bindings::bound::Binding::read(&binding.to_string())?;
      }
    }

    Ok(())
  }

  #[test]
  fn every_button_a_default_names_is_one_this_desktop_routes() -> Result<(), Failure> {
    let Ok(every) = every();

    for job in every {
      for (_, _, pressed) in job.bound.iter().filter(|(on, _, _)| *on == Input::Pad) {
        let named = button_name(pressed)?;

        assert_ne!(
          console_input_gamepad::routing::arrives(named),
          Ok(None),
          "{} is on {pressed}, which arrives nowhere",
          job.slug
        );
      }
    }

    Ok(())
  }

  #[test]
  fn every_keyboard_job_can_be_bound_by_the_compositor() {
    let Ok(every) = every();

    let on_a_key = every
      .flat_map(|job| job.bound.iter().map(move |(on, _, _)| (job, on)))
      .filter(|(_, on)| **on == Input::Keyboard)
      .map(|(job, _)| job);

    for job in on_a_key {
      let Ok(does) = job.action.does(ButtonPress::Down);

      assert!(
        matches!(does, Some(Effect::Run(_) | Effect::Dispatch(_))),
        "{} is on a key but has no command or compositor action",
        job.slug
      );
    }
  }

  #[test]
  fn every_job_says_what_it_is() {
    let Ok(every) = every();

    for job in every {
      assert_ne!(job.action.says(), Ok(""), "{} says nothing", job.slug);
      assert!(
        job
          .slug
          .chars()
          .all(|letter| letter.is_ascii_lowercase() || letter == '-')
      );
    }
  }

  #[test]
  fn a_button_with_a_second_job_does_that_one_while_l2_is_held() {
    let Ok(table) = Table::ours();

    assert_eq!(
      what(&table, "dpad-up", &[], Mode::Desktop),
      Ok(Some(Action::Up))
    );
    assert_eq!(
      what(&table, "dpad-up", &["l2"], Mode::Desktop),
      Ok(Some(Action::Louder))
    );
  }

  #[test]
  fn a_button_with_no_second_job_keeps_doing_its_first_one() {
    let Ok(table) = Table::ours();

    assert_eq!(
      what(&table, "left-paddle-top", &["l2"], Mode::Desktop),
      Ok(Some(Action::Menu))
    );
  }

  #[test]
  fn both_triggers_is_not_either_of_them() {
    let Ok(table) = Table::ours();

    assert_eq!(
      what(&table, "dpad-up", &["l2", "r2"], Mode::Desktop),
      Ok(Some(Action::Louder))
    );
  }

  #[test]
  fn a_button_can_mean_one_thing_on_the_desktop_and_another_in_a_picker() {
    let Ok(table) = Table::ours();

    assert_eq!(
      what(&table, "a", &[], Mode::Desktop),
      Ok(Some(Action::Click))
    );
    assert_eq!(what(&table, "a", &[], Mode::Tabs), Ok(Some(Action::Choose)));
    assert_eq!(
      what(&table, "r1", &[], Mode::Desktop),
      Ok(Some(Action::Workspace(1)))
    );
    assert_eq!(
      what(&table, "r1", &[], Mode::Tabs),
      Ok(Some(Action::Tab(1)))
    );
  }

  #[test]
  fn in_one_of_our_apps_a_press_means_what_it_means_in_a_panel_and_l2_reaches_the_workspaces() {
    let Ok(table) = Table::ours();

    assert_eq!(what(&table, "a", &[], Mode::App), Ok(Some(Action::Choose)));
    assert_eq!(what(&table, "y", &[], Mode::App), Ok(Some(Action::More)));
    assert_eq!(what(&table, "r1", &[], Mode::App), Ok(Some(Action::Tab(1))));
    assert_eq!(
      what(&table, "l1", &[], Mode::App),
      Ok(Some(Action::Tab(-1)))
    );
    assert_eq!(
      what(&table, "r1", &["l2"], Mode::App),
      Ok(Some(Action::Workspace(1)))
    );
    assert_eq!(
      what(&table, "l1", &["l2"], Mode::App),
      Ok(Some(Action::Workspace(-1)))
    );
    assert_eq!(
      what(&table, "r1", &["l2", "r2"], Mode::App),
      Ok(Some(Action::Payload(1)))
    );
  }

  #[test]
  fn l2_reaches_the_workspaces_from_anywhere_but_a_picker_and_l2_with_r2_carries_the_window() {
    let Ok(table) = Table::ours();

    for mode in [Mode::Desktop, Mode::App, Mode::HomeScreen, Mode::Standing] {
      assert_eq!(
        what(&table, "r1", &["l2"], mode),
        Ok(Some(Action::Workspace(1))),
        "{mode:?}"
      );
      assert_eq!(
        what(&table, "l1", &["l2"], mode),
        Ok(Some(Action::Workspace(-1))),
        "{mode:?}"
      );
    }

    assert_eq!(
      what(&table, "r1", &["l2"], Mode::Tabs),
      Ok(Some(Action::Tab(1))),
      "a picker keeps the desktop where it is"
    );

    assert_eq!(
      what(&table, "r1", &["l2", "r2"], Mode::Desktop),
      Ok(Some(Action::Payload(1)))
    );
    assert_eq!(
      what(&table, "l1", &["l2", "r2"], Mode::Desktop),
      Ok(Some(Action::Payload(-1)))
    );
    assert_eq!(
      what(&table, "r1", &[], Mode::Desktop),
      Ok(Some(Action::Workspace(1))),
      "a bare desktop has no tabs"
    );
  }

  #[test]
  fn leaving_for_steam_is_not_something_to_do_by_brushing_a_button() {
    let Ok(table) = Table::ours();

    assert_ne!(what(&table, "legion-left", &[], Mode::Desktop), Ok(None));
    assert_eq!(what(&table, "legion-left", &[], Mode::Tabs), Ok(None));
    assert_eq!(what(&table, "view", &[], Mode::Tabs), Ok(None));
  }

  #[test]
  fn a_key_is_not_a_button_and_does_not_answer_for_one() {
    let Ok(table) = Table::ours();

    assert_eq!(typed(&table, "i", &["super"]), Ok(Some(Action::Settings)));
    assert_eq!(
      typed(&table, "i", &[]),
      Ok(None),
      "the modifier is part of the place"
    );
    assert_eq!(what(&table, "i", &["super"], Mode::Desktop), Ok(None));
  }

  #[test]
  fn a_digit_names_a_place_by_its_number_and_zero_is_the_tenth() {
    let Ok(table) = Table::ours();

    assert_eq!(typed(&table, "3", &["super"]), Ok(Some(Action::Place(3))));
    assert_eq!(typed(&table, "0", &["super"]), Ok(Some(Action::Place(10))));
    assert_eq!(
      what(&table, "3", &["super"], Mode::Desktop),
      Ok(None),
      "a pad has no digits"
    );

    let Ok(third) = Effect::workspace(WorkspaceTarget::Number(3), Payload::None);

    assert_eq!(Action::Place(3).does(ButtonPress::Down), Ok(Some(third)));
    assert_eq!(Action::Place(10).says(), Ok("go to desktop 10"));

    let Ok(carried) = Effect::workspace(WorkspaceTarget::Number(3), Payload::Window);

    assert_eq!(
      typed(&table, "3", &["super", "shift"]),
      Ok(Some(Action::CarryTo(3)))
    );
    assert_eq!(
      Action::CarryTo(3).does(ButtonPress::Down),
      Ok(Some(carried))
    );
  }

  #[test]
  fn window_actions_emit_typed_dispatches_on_press() {
    for (action, expected) in [
      (Action::CloseWindow, CompositorAction::CloseWindow),
      (Action::Fullscreen, CompositorAction::ToggleFullscreen),
      (
        Action::Focus(Direction::Left),
        CompositorAction::Focus(Direction::Left),
      ),
      (
        Action::Focus(Direction::Right),
        CompositorAction::Focus(Direction::Right),
      ),
      (
        Action::Focus(Direction::Up),
        CompositorAction::Focus(Direction::Up),
      ),
      (
        Action::Focus(Direction::Down),
        CompositorAction::Focus(Direction::Down),
      ),
    ] {
      assert_eq!(
        action.does(ButtonPress::Down),
        Ok(Some(Effect::Dispatch(expected)))
      );
      assert_eq!(action.does(ButtonPress::Up), Ok(None));
    }
  }

  #[test]
  fn the_keys_the_lua_used_to_hold_are_in_the_table() {
    let Ok(table) = Table::ours();

    assert_eq!(
      typed(&table, "enter", &["super"]),
      Ok(Some(Action::Terminal))
    );
    assert_eq!(typed(&table, "f", &["super"]), Ok(Some(Action::Files)));
    assert_eq!(
      typed(&table, "w", &["super"]),
      Ok(Some(Action::CloseWindow))
    );
    assert_eq!(
      typed(&table, "f", &["super", "shift"]),
      Ok(Some(Action::Fullscreen))
    );
    assert_eq!(
      typed(&table, "left", &["super"]),
      Ok(Some(Action::Focus(Direction::Left)))
    );
    assert_eq!(
      typed(&table, "left", &["super", "shift"]),
      Ok(Some(Action::Payload(-1)))
    );
    assert_eq!(
      typed(&table, "a", &["super", "ctrl"]),
      Ok(Some(Action::SettingsAt("Sound")))
    );
    assert_eq!(
      typed(&table, "print", &[]),
      Ok(Some(Action::ScreenshotArea))
    );
    assert_eq!(
      typed(&table, "s", SUPER_SHIFT),
      Ok(Some(Action::ScreenshotArea))
    );
    assert_eq!(typed(&table, "volume-up", &[]), Ok(Some(Action::Louder)));
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
    let Ok(sends) = sends();

    for wanted in [
      KeyCode::BTN_LEFT,
      KeyCode::BTN_RIGHT,
      KeyCode::KEY_ESC,
      KeyCode::KEY_UP,
    ] {
      assert!(
        sends.contains(&wanted),
        "{wanted:?} is bound and cannot be sent"
      );
    }

    assert!(sends.len() >= 11, "{sends:?}");
  }

  #[test]
  fn a_key_is_held_for_as_long_as_the_button_is() {
    let Ok(down) = Output::key(KeyCode::KEY_UP.0, 1);
    let Ok(up) = Output::key(KeyCode::KEY_UP.0, 0);
    let Ok(clicked) = Output::key(KeyCode::BTN_LEFT.0, 1);

    assert_eq!(
      Action::Up.does(ButtonPress::Down),
      Ok(Some(Effect::Frame(vec![down])))
    );
    assert_eq!(
      Action::Up.does(ButtonPress::Up),
      Ok(Some(Effect::Frame(vec![up])))
    );
    assert_eq!(
      Action::Click.does(ButtonPress::Down),
      Ok(Some(Effect::Frame(vec![clicked])))
    );
  }

  #[test]
  fn something_that_starts_a_program_happens_once() {
    let Ok(started) = Effect::run(&["launcher", LAUNCHER_KEEP.spelling]);

    assert_eq!(Action::Menu.does(ButtonPress::Down), Ok(Some(started)));
    assert_eq!(Action::Menu.does(ButtonPress::Up), Ok(None));
  }

  #[test]
  fn the_shoulders_move_you_and_carry_the_window_while_l2_is_held() {
    let Ok(moved) = Effect::workspace(WorkspaceTarget::Relative(1), Payload::None);
    let Ok(carried) = Effect::workspace(WorkspaceTarget::Relative(-1), Payload::Window);

    assert_eq!(
      Action::Workspace(1).does(ButtonPress::Down),
      Ok(Some(moved))
    );
    assert_eq!(
      Action::Payload(-1).does(ButtonPress::Down),
      Ok(Some(carried))
    );
  }

  #[test]
  fn l2_with_view_shows_every_desktop_and_view_alone_is_still_the_browser() {
    let Ok(table) = Table::ours();
    let Ok(shown) = Effect::run(&["console-overview", OVERVIEW_SHOW.spelling]);

    assert_eq!(
      what(&table, "view", &["l2"], Mode::Desktop),
      Ok(Some(Action::Overview))
    );
    assert_eq!(
      what(&table, "view", &[], Mode::Desktop),
      Ok(Some(Action::Browser))
    );
    assert_eq!(Action::Overview.does(ButtonPress::Down), Ok(Some(shown)));
  }

  #[test]
  fn the_keyboard_is_ours() {
    let Ok(table) = Table::ours();
    let Ok(toggled) = Effect::run(&["keyboard-toggle"]);

    assert_eq!(
      what(&table, "x", &[], Mode::Desktop),
      Ok(Some(Action::Keyboard))
    );
    assert_eq!(Action::Keyboard.does(ButtonPress::Down), Ok(Some(toggled)));
    assert_eq!(
      what(&table, "keyboard", &[], Mode::Desktop),
      Ok(Some(Action::Keyboard))
    );
  }

  #[test]
  fn layer_focus_chords_work_without_replacing_the_unmodified_buttons() {
    let Ok(table) = Table::ours();

    for mode in [Mode::Desktop, Mode::HomeScreen, Mode::Tabs] {
      assert_eq!(
        what(&table, "menu", &["l2"], mode),
        Ok(Some(Action::FocusBar))
      );
      assert_eq!(
        what(&table, "x", &["l2"], mode),
        Ok(Some(Action::FocusPopups))
      );
      assert_eq!(
        what(&table, "menu", &[], mode),
        Ok(Some(Action::ButtonGuide))
      );
      assert_eq!(what(&table, "x", &[], mode), Ok(Some(Action::Keyboard)));
    }

    assert_eq!(typed(&table, "period", SUPER), Ok(Some(Action::FocusBar)));
    assert_eq!(
      typed(&table, "comma", SUPER_SHIFT),
      Ok(Some(Action::FocusPopups))
    );
    assert_eq!(
      typed(&table, "comma", SUPER),
      Ok(Some(Action::Notifications))
    );
  }

  #[test]
  fn what_someone_moved_is_where_they_moved_it() -> Result<(), Failure> {
    let table = stated("[jobs]\nscreenshot = \"r2 + a\"\n")?;

    assert_eq!(
      what(&table, "a", &["r2"], Mode::Desktop),
      Ok(Some(Action::Screenshot))
    );
    assert_ne!(
      what(&table, "right-paddle-bottom", &["l2"], Mode::Desktop),
      Ok(Some(Action::Screenshot))
    );
    assert_eq!(
      what(&table, "dpad-up", &["l2"], Mode::Desktop),
      Ok(Some(Action::Louder))
    );

    Ok(())
  }

  #[test]
  fn a_job_moved_onto_a_chord_of_two_buttons_is_read_off_the_chord() -> Result<(), Failure> {
    let table = stated("[jobs]\nmenu = \"left-paddle-bottom + right-paddle-top\"\n")?;

    assert_eq!(
      what(
        &table,
        "right-paddle-top",
        &["left-paddle-bottom"],
        Mode::Desktop
      ),
      Ok(Some(Action::Menu))
    );
    assert_eq!(
      what(&table, "right-paddle-top", &[], Mode::Desktop),
      Ok(Some(Action::PutAway)),
      "the button on its own goes on doing what it did"
    );

    Ok(())
  }

  #[test]
  fn a_job_left_with_no_button_is_on_no_button() -> Result<(), Failure> {
    let table = stated("[jobs]\nmenu = \"\"\n")?;

    assert_eq!(
      what(&table, "left-paddle-top", &[], Mode::Desktop),
      Ok(None)
    );
    let Ok(menu) = table.bindings("menu");
    let played: Vec<_> = menu.iter().map(|one| one.played()).collect();

    assert_eq!(played, [Ok(Played::ByNothing)]);

    Ok(())
  }

  #[test]
  fn a_job_this_desktop_does_not_have_is_left_alone() -> Result<(), Failure> {
    let table = stated("[jobs]\nteleport = \"a\"\n")?;

    assert_eq!(
      what(&table, "a", &[], Mode::Desktop),
      Ok(Some(Action::Click))
    );

    Ok(())
  }

  #[test]
  fn the_home_screen_takes_the_d_pad_and_leaves_the_rest() {
    let Ok(table) = unmoved();

    assert_eq!(
      what(&table, "dpad-up", &[], Mode::Desktop),
      Ok(Some(Action::Up))
    );
    assert_eq!(
      what(&table, "dpad-up", &[], Mode::HomeScreen),
      Ok(Some(Action::Tell(PadInput::Up)))
    );
    assert_eq!(
      what(&table, "dpad-down", &[], Mode::HomeScreen),
      Ok(Some(Action::Tell(PadInput::Down)))
    );
    assert_eq!(
      what(&table, "dpad-left", &[], Mode::HomeScreen),
      Ok(Some(Action::Tell(PadInput::Left)))
    );
    assert_eq!(
      what(&table, "dpad-right", &[], Mode::HomeScreen),
      Ok(Some(Action::Tell(PadInput::Right)))
    );

    assert_eq!(
      what(&table, "r1", &[], Mode::HomeScreen),
      Ok(Some(Action::Workspace(1)))
    );
    assert_eq!(
      what(&table, "l1", &[], Mode::HomeScreen),
      Ok(Some(Action::Workspace(-1)))
    );
    assert_eq!(
      what(&table, "legion-left", &[], Mode::HomeScreen),
      Ok(Some(Action::GameMode))
    );
    assert_eq!(
      what(&table, "view", &[], Mode::HomeScreen),
      Ok(Some(Action::Browser))
    );
    assert_eq!(
      what(&table, "left-paddle-top", &[], Mode::HomeScreen),
      Ok(Some(Action::Menu))
    );
  }

  #[test]
  fn a_is_the_pointers_button_until_the_home_screen_is_awake() {
    let Ok(table) = unmoved();

    assert_eq!(
      what(&table, "a", &[], Mode::Desktop),
      Ok(Some(Action::Click))
    );
    assert_eq!(
      what(&table, "a", &[], Mode::HomeScreen),
      Ok(Some(Action::Click))
    );
    assert_eq!(
      what(&table, "a", &[], Mode::Standing),
      Ok(Some(Action::Tell(PadInput::Pressed)))
    );

    assert_eq!(
      what(&table, "y", &[], Mode::Desktop),
      Ok(Some(Action::MoreOptions))
    );
    assert_eq!(
      what(&table, "y", &[], Mode::HomeScreen),
      Ok(Some(Action::MoreOptions))
    );
    assert_eq!(
      what(&table, "y", &[], Mode::Standing),
      Ok(Some(Action::Tell(PadInput::More)))
    );

    assert_eq!(
      what(&table, "b", &[], Mode::HomeScreen),
      Ok(Some(Action::Back))
    );
    assert_eq!(
      what(&table, "b", &[], Mode::Standing),
      Ok(Some(Action::Tell(PadInput::Back)))
    );
  }

  #[test]
  fn the_d_pad_stays_the_home_screens_once_it_is_awake() {
    let Ok(table) = unmoved();

    assert_eq!(
      what(&table, "dpad-up", &[], Mode::Standing),
      Ok(Some(Action::Tell(PadInput::Up)))
    );
    assert_eq!(
      what(&table, "r1", &[], Mode::Standing),
      Ok(Some(Action::Workspace(1)))
    );
  }

  #[test]
  fn holding_the_d_pad_on_the_home_screen_goes_on_walking_it() {
    for said in [
      PadInput::Up,
      PadInput::Down,
      PadInput::Left,
      PadInput::Right,
    ] {
      assert_eq!(
        Action::Tell(said).repeats(),
        Ok(RepeatMode::WhileHeld),
        "{said:?}"
      );
    }

    assert_eq!(Action::Tell(PadInput::More).repeats(), Ok(RepeatMode::Once));
    assert_eq!(
      Action::Tell(PadInput::Pressed).repeats(),
      Ok(RepeatMode::Once)
    );
  }

  #[test]
  fn what_the_home_screen_is_told_is_said_once() {
    for said in [
      PadInput::Pressed,
      PadInput::Up,
      PadInput::More,
      PadInput::Back,
    ] {
      assert_eq!(
        Action::Tell(said).does(ButtonPress::Down),
        Ok(Some(Effect::Tell(said))),
        "{said:?}"
      );
      assert_eq!(
        Action::Tell(said).does(ButtonPress::Up),
        Ok(None),
        "{said:?}"
      );
    }
  }

  #[test]
  fn a_word_to_the_home_screen_is_not_a_key() {
    for said in console_onscreen::homeward::EVERY {
      assert!(
        matches!(
          Action::Tell(said).does(ButtonPress::Down),
          Ok(Some(Effect::Tell(_)) | None)
        ),
        "{said:?} came out as something other than a word"
      );
    }
  }
}
