//! What this desktop can do, and what each of those is bound to.
//!
//! One table, and now it really is one. What a button meant used to be split
//! between an InputPlumber profile, this daemon, and a third place for the one
//! button neither of them knew about; then between two profiles, because a
//! button meant one thing on the desktop and another with a chooser up.
//! Nothing could be asked "what does X do" and answer.
//!
//! So the profile stopped saying what a button means and started saying only
//! what it is -- `console_gamepad::routing` -- and everything a press comes to is
//! decided here: the job, when it applies, and what has to be held down with
//! it. The daemon reads it, the setup screen writes to it, and the guide reads
//! it out loud. There is no second copy.
//!
//! What is in the table is the default and nothing more. Somebody who moves a
//! job onto another button writes that in `~/.config/console/buttons.toml`,
//! and only what they moved is in there: a machine nobody has touched has an
//! empty file and the whole of its answer here.

use evdev::{KeyCode, RelativeAxisCode};

use console_never::Never;
use console_onscreen::Said;
use console_gamepad::jobs::{ALONE, Binding, Held, Jobs, Layer};

use crate::doing::{Carry, Doing, Out};
use crate::mode::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum What {
    Menu,
    Dictate,
    PutAway,
    Screenshot,
    Settings,
    Brighter,
    Dimmer,
    Louder,
    Quieter,
    GameMode,
    Browser,
    Guide,
    Keyboard,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Job {
    pub slug: &'static str,
    pub what: What,
    pub when: When,
    pub bound: &'static [(Layer, &'static str)],
}

const ON: Layer = ALONE;
const L2: Layer = Layer { l2: true, r2: false };

pub const JOBS: [Job; 36] = [
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
    Job {
        slug: "keyboard",
        what: What::Keyboard,
        when: When::Anywhere,
        bound: &[(ON, "x"), (ON, "keyboard")],
    },
    Job {
        slug: "screenshot",
        what: What::Screenshot,
        when: When::Anywhere,
        bound: &[(L2, "right-paddle-bottom")],
    },
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
    Job { slug: "back", what: What::Back, when: When::Anywhere, bound: &[(ON, "b")] },
    Job { slug: "up", what: What::Up, when: When::Anywhere, bound: &[(ON, "dpad-up")] },
    Job { slug: "down", what: What::Down, when: When::Anywhere, bound: &[(ON, "dpad-down")] },
    Job { slug: "left", what: What::Left, when: When::Anywhere, bound: &[(ON, "dpad-left")] },
    Job { slug: "right", what: What::Right, when: When::Anywhere, bound: &[(ON, "dpad-right")] },
    Job {
        slug: "click",
        what: What::Click,
        when: When::OnTheDesktop,
        bound: &[(ON, "a"), (ON, "r3")],
    },
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
    Job {
        slug: "home-choose",
        what: What::Tell(Said::Pressed),
        when: When::StandingOnASquare,
        bound: &[(ON, "a"), (ON, "r3")],
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
    Job {
        slug: "choose",
        what: What::Choose,
        when: When::WithAChooserUp,
        bound: &[(ON, "a"), (ON, "r3")],
    },
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

pub fn job(slug: &str) -> Result<Option<&'static Job>, Never> {
    Ok(JOBS.iter().find(|job| job.slug == slug))
}

impl What {
    pub fn says(self) -> Result<&'static str, Never> {
        Ok(match self {
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
            What::Brighter => started(&["/usr/local/bin/console-brightness", "up"]),
            What::Dimmer => started(&["/usr/local/bin/console-brightness", "down"]),
            What::Louder => started(&["/usr/local/bin/console-volume", "up"]),
            What::Quieter => started(&["/usr/local/bin/console-volume", "down"]),
            What::GameMode => started(&["game-mode"]),
            What::Browser => started(&["/usr/local/bin/console-browser"]),
            What::Guide => started(&["/usr/local/bin/console-buttons", "--menu"]),
            What::Keyboard => started(&["keyboard-toggle"]),
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

fn moved(step: i32, carrying: Carry) -> Result<Option<Doing>, Never> {
    let Ok(moved) = Doing::workspace(&format!("{step:+}"), carrying);

    Ok(Some(moved))
}

pub fn sends() -> Result<Vec<KeyCode>, Never> {
    let mut every: Vec<KeyCode> = JOBS
        .iter()
        .filter_map(|job| {
            let Ok(does) = job.what.does(Press::Down);

            match does {
                Some(Doing::Frame(frame)) => frame.first().map(|out| KeyCode(out.code)),
                Some(Doing::Run(_) | Doing::Tell(_)) | None => None,
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
        Ok(Table {
            bound: JOBS
                .iter()
                .map(|job| {
                    let Ok(given) = said.bound(job.slug);

                    let bound = match given {
                        Some(moved) => moved.to_vec(),
                        None => job
                            .bound
                            .iter()
                            .map(|(layer, button)| {
                                let Ok(held) = Binding::held(*layer, (*button).to_string());

                                held
                            })
                            .collect(),
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

    pub fn what(&self, button: &str, layer: Layer, mode: Mode) -> Result<Option<&'static Job>, Never> {
        let Ok(matching) = self.matching(button, layer, mode);

        match matching {
            Some(job) => return Ok(Some(job)),
            None => {},
        }

        let Ok(held) = layer.held();

        match held {
            Held::Down => self.matching(button, ALONE, mode),
            Held::Up => Ok(None),
        }
    }

    fn matching(&self, button: &str, layer: Layer, mode: Mode) -> Result<Option<&'static Job>, Never> {
        let mut found: Option<&'static Job> = None;
        let Ok(every) = self.every();

        for (job, bound) in every {
            let Ok(suits) = job.when.suits(mode);

            match suits {
                Suits::Elsewhere => continue,
                Suits::InFront => {},
            }

            match bound.iter().any(|one| one.layer == layer && one.button == button) {
                true => {},
                false => continue,
            }

            found = match found {
                Some(already) if already.when.beats(job.when) == Ok(Suits::InFront) => Some(already),
                Some(_) | None => Some(job),
            };
        }

        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use console_gamepad::jobs::Played;
    use super::*;
    use std::collections::BTreeSet;
    use console_gamepad::vocabulary::button_name;

    fn ok<T>(answer: Result<T, console_never::Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn ours() -> Table {
        ok(Table::ours())
    }

    fn none() -> Jobs {
        ok(Jobs::none())
    }

    #[test]
    fn nothing_is_written_down_duplicates() {
        let mut seen = BTreeSet::new();
        let twice: Vec<&str> =
            JOBS.iter().map(|job| job.slug).filter(|slug| !seen.insert(*slug)).collect();

        assert!(twice.is_empty(), "two jobs are called {twice:?}");
    }

    #[test]
    fn nothing_is_bound_twice_in_one_place() {
        for mode in [Mode::Desktop, Mode::Tabs, Mode::Home] {
            let mut every: Vec<String> = Vec::new();
            for job in JOBS.iter().filter(|job| job.when.suits(mode) == Ok(Suits::InFront)) {
                for (layer, button) in job.bound {
                    every.push(format!("{layer:?} {button} {}", ok(job.when.rank())));
                }
            }
            let mut seen = BTreeSet::new();
            let twice: Vec<String> =
                every.into_iter().filter(|on| !seen.insert(on.clone())).collect();

            assert!(twice.is_empty(), "two jobs on one button in {mode:?}: {twice:?}");
        }
    }

    #[test]
    fn every_default_is_on_a_button_this_desktop_can_route() {
        for job in JOBS {
            for (_, button) in job.bound {
                let named = button_name(button).unwrap_or_else(|_| panic!("{button}"));
                assert!(
                    ok(console_gamepad::routing::arrives(named)).is_some(),
                    "{} is on {button}, which arrives nowhere",
                    job.slug
                );
            }
        }
    }

    #[test]
    fn every_job_says_what_it_is() {
        for job in JOBS {
            assert!(!ok(job.what.says()).is_empty(), "{} says nothing", job.slug);
            assert!(job.slug.chars().all(|letter| letter.is_ascii_lowercase() || letter == '-'));
        }
    }

    #[test]
    fn a_button_with_a_second_job_does_that_one_while_l2_is_held() {
        let table = ours();
        assert_eq!(ok(table.what("dpad-up", ON, Mode::Desktop)).map(|job| job.what), Some(What::Up));
        assert_eq!(ok(table.what("dpad-up", L2, Mode::Desktop)).map(|job| job.what), Some(What::Louder));
    }

    #[test]
    fn a_button_with_no_second_job_keeps_doing_its_first_one() {
        let table = ours();
        assert_eq!(ok(table.what("left-paddle-top", L2, Mode::Desktop)).map(|job| job.what), Some(What::Menu));
    }

    #[test]
    fn both_triggers_is_not_either_of_them() {
        let table = ours();
        let both = ok(Layer::of(Held::Down, Held::Down));
        assert_eq!(ok(table.what("dpad-up", both, Mode::Desktop)).map(|job| job.what), Some(What::Up));
    }

    #[test]
    fn a_button_can_mean_one_thing_on_the_desktop_and_another_in_a_chooser() {
        let table = ours();
        assert_eq!(ok(table.what("a", ON, Mode::Desktop)).map(|job| job.what), Some(What::Click));
        assert_eq!(ok(table.what("a", ON, Mode::Tabs)).map(|job| job.what), Some(What::Choose));
        assert_eq!(ok(table.what("r1", ON, Mode::Desktop)).map(|job| job.what), Some(What::Workspace(1)));
        assert_eq!(ok(table.what("r1", ON, Mode::Tabs)).map(|job| job.what), Some(What::Tab(1)));
    }

    #[test]
    fn leaving_for_steam_is_not_something_to_do_by_brushing_a_button() {
        let table = ours();
        assert!(ok(table.what("legion-left", ON, Mode::Desktop)).is_some());
        assert_eq!(table.what("legion-left", ON, Mode::Tabs), Ok(None));
        assert_eq!(table.what("view", ON, Mode::Tabs), Ok(None));
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
        let job = ok(table.what("x", ON, Mode::Desktop)).expect("x");

        assert_eq!(job.what, What::Keyboard);
        assert_eq!(job.what.does(Press::Down), Ok(Some(ok(Doing::run(&["keyboard-toggle"])))));
        assert_eq!(ok(table.what("keyboard", ON, Mode::Desktop)).map(|job| job.what), Some(What::Keyboard));
    }

    #[test]
    fn what_somebody_moved_is_where_they_moved_it() {
        let said = Jobs::read("[jobs]\nscreenshot = \"r2 + a\"\n").expect("a table");
        let table = ok(Table::of(&said));
        let r2 = ok(Layer::of(Held::Up, Held::Down));
        assert_eq!(ok(table.what("a", r2, Mode::Desktop)).map(|job| job.what), Some(What::Screenshot));
        assert_ne!(
            ok(table.what("right-paddle-bottom", L2, Mode::Desktop)).map(|job| job.what),
            Some(What::Screenshot),
        );
        assert_eq!(ok(table.what("dpad-up", L2, Mode::Desktop)).map(|job| job.what), Some(What::Louder));
    }

    #[test]
    fn a_job_left_with_no_button_is_on_no_button() {
        let said = Jobs::read("[jobs]\nmenu = \"\"\n").expect("a table");
        let table = ok(Table::of(&said));
        assert_eq!(table.what("left-paddle-top", ON, Mode::Desktop), Ok(None));
        assert_eq!(ok(table.bindings("menu")).len(), 1);
        assert_eq!(ok(ok(table.bindings("menu"))[0].played()), Played::ByNothing);
    }

    #[test]
    fn a_job_this_desktop_does_not_have_is_left_alone() {
        let said = Jobs::read("[jobs]\nteleport = \"a\"\n").expect("a table");
        let table = ok(Table::of(&said));
        assert_eq!(ok(table.what("a", ON, Mode::Desktop)).map(|job| job.what), Some(What::Click));
    }

    #[test]
    fn the_home_screen_takes_the_d_pad_and_leaves_the_rest() {
        let table = ok(Table::of(&none()));
        let what = |button, mode| ok(table.what(button, ON, mode)).map(|job| job.what);

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

    #[test]
    fn a_is_the_pointers_button_until_the_home_screen_is_awake() {
        let table = ok(Table::of(&none()));
        let what = |button, mode| ok(table.what(button, ON, mode)).map(|job| job.what);

        assert_eq!(what("a", Mode::Desktop), Some(What::Click));
        assert_eq!(what("a", Mode::Home), Some(What::Click), "asleep, it is the pointer's");
        assert_eq!(what("a", Mode::Standing), Some(What::Tell(Said::Pressed)));

        assert_eq!(what("y", Mode::Desktop), Some(What::MoreOptions));
        assert_eq!(what("y", Mode::Home), Some(What::MoreOptions));
        assert_eq!(what("y", Mode::Standing), Some(What::Tell(Said::More)));

        assert_eq!(what("b", Mode::Home), Some(What::Back), "asleep, B is out of things");
        assert_eq!(what("b", Mode::Standing), Some(What::Tell(Said::Back)));
    }

    #[test]
    fn the_d_pad_stays_the_home_screens_once_it_is_awake() {
        let table = ok(Table::of(&none()));
        let what = |button, mode| ok(table.what(button, ON, mode)).map(|job| job.what);

        assert_eq!(what("dpad-up", Mode::Standing), Some(What::Tell(Said::Up)));
        assert_eq!(what("r1", Mode::Standing), Some(What::Workspace(1)));
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
