//! What the compositor is asked, and what its answers mean.
//!
//! The program is `console_core_external_programs::Program::Hyprctl`, and that
//! was the only part of this anybody owned. What came back was read wherever
//! it landed: the home screen and the wallpaper each counted the windows on
//! the workspace in front, the nested desktop read the monitors wherever it
//! needed them, and the settings panel went looking for `"scale"` in the text
//! with `find` and took the digits after the colon -- a reading that takes the
//! first scale the answer happens to spell, whatever it belongs to.
//!
//! They did not agree about failure either. A compositor that would not answer
//! was an empty string in one place, which reads on as a desktop with no
//! screens and nothing on it; a line on stderr in another; the covered-up
//! answer in a third, which is the right one where the wallpaper is asking and
//! is not an answer anybody else should be given. So asking is one function
//! here and it says how it failed, and what to do about a compositor that has
//! gone quiet is left to the crate that knows what it was going to do with the
//! answer.
//!
//! The questions keep hyprctl's own words -- `layers`, `activeworkspace`,
//! `monitors`, `clients` -- because they are hyprctl's, and a question spelled
//! some other way here would be a second name for a thing that already has
//! one. The answers are handed back in this desktop's words, which is what the
//! crate is for.
//!
//! What is not here is which surfaces are furniture, which namespace the home
//! screen draws under, and what a window on the workspace in front means for
//! the wallpaper. This crate walks the shape of an answer;
//! `console-onscreen` spells this desktop's words over the walk.
//!
//! ## What a lua config changes about hyprctl
//!
//! This desktop's compositor is configured in `hyprland.lua`, and that is not
//! a spelling of the same file: it swaps the parser, and two of hyprctl's
//! verbs go with it. `hyprctl keyword` answers *keyword can't work with
//! non-legacy parsers* and **exits zero**, so a caller reading the status
//! reads a setting that was never made; `hyprctl dispatch` wraps whatever it
//! is handed in `hl.dispatch(...)`, so a dispatcher named in hyprctl's own
//! words is lua that does not parse. Both were written here against the
//! legacy parser and both were quiet about it -- the binds this desktop hands
//! over were refused for as long as the lua config has existed, and nothing
//! said so, because a refusal that exits zero and never says *error* is
//! indistinguishable from being taken.
//!
//! So a keyword is `hl.config` or `hl.device` through [`Told::Eval`], a
//! dispatcher is the `hl.dsp.*` object [`Told::Dispatch`] already wraps, and
//! the two verbs that are hyprctl's own rather than the config's --
//! `switchxkblayout` among them -- are called as themselves, with no verb in
//! front. [`Done::Refused`] now knows that sentence by heart, because the one
//! thing worse than a compositor that will not take something is one that
//! says so where nobody is looking.
//!
//! `stirred` is the same crate's other half: what the compositor says when
//! nobody asked it anything. Four crates were reading those lines by the words
//! Hyprland spells them with, and two of the four lists had already drifted
//! apart, which is what a literal in two places does when nobody has decided
//! twice.

use std::process::Command;

use console_core_external_programs::Program;
use console_core_never::Never;

pub mod stirred;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    Layers,
    ActiveWorkspace,
    Monitors,
    EveryMonitor,
    Clients,
    Devices,
    Binds,
}

impl Asked {
    pub fn words(self) -> Result<&'static [&'static str], Never> {
        Ok(match self {
            Asked::Layers => &["layers", "-j"],
            Asked::ActiveWorkspace => &["activeworkspace", "-j"],
            Asked::Monitors => &["monitors", "-j"],
            Asked::EveryMonitor => &["monitors", "all", "-j"],
            Asked::Clients => &["clients", "-j"],
            Asked::Devices => &["devices", "-j"],
            Asked::Binds => &["binds", "-j"],
        })
    }

    pub fn about(self) -> Result<&'static str, Never> {
        Ok(match self {
            Asked::Layers => "what is on the screen",
            Asked::ActiveWorkspace => "the workspace in front",
            Asked::Monitors => "what the screens are",
            Asked::EveryMonitor => "every screen it has",
            Asked::Clients => "the windows that are open",
            Asked::Devices => "what is plugged in",
            Asked::Binds => "the keys it is holding",
        })
    }
}

pub fn asked(question: Asked) -> Result<serde_json::Value, String> {
    let Ok(asking) = Program::Hyprctl.command();

    answered(asking, question)
}

pub fn answered(mut asking: Command, question: Asked) -> Result<serde_json::Value, String> {
    let Ok(words) = question.words();
    let Ok(about) = question.about();

    let said = asking
        .args(words)
        .output()
        .map_err(|fault| format!("asking hyprctl {about}: {fault}"))?;

    let printed = match said.status.success() {
        true => said.stdout,
        false => {
            return Err(format!(
                "hyprctl would not say {about}: {}",
                String::from_utf8_lossy(&said.stderr).trim()
            ));
        }
    };

    serde_json::from_slice(&printed)
        .map_err(|fault| format!("reading hyprctl's answer about {about}: {fault}"))
}

pub fn read(said: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(said).map_err(|fault| format!("reading what hyprctl said: {fault}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Told {
    Dispatch,
    Eval,
}

impl Told {
    pub fn word(self) -> Result<&'static str, Never> {
        Ok(match self {
            Told::Dispatch => "dispatch",
            Told::Eval => "eval",
        })
    }
}

const SWITCH: &str = "switchxkblayout";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyboard {
    pub name: String,
    pub layouts: Vec<String>,
    pub wearing: Option<String>,
}

pub fn keyboards(devices: &serde_json::Value) -> Result<Vec<Keyboard>, Never> {
    let listed = match devices.get("keyboards").and_then(serde_json::Value::as_array) {
        Some(listed) => listed,
        None => return Ok(Vec::new()),
    };

    Ok(listed
        .iter()
        .filter_map(|one| {
            let name = one.get("name").and_then(serde_json::Value::as_str)?;
            let layouts = one
                .get("layout")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|said| !said.is_empty())
                .map(str::to_string)
                .collect();
            let wearing = one
                .get("active_keymap")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);

            Some(Keyboard { name: name.to_string(), layouts, wearing })
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind {
    pub held: u64,
    pub named: String,
    pub about: String,
}

pub fn binds(said: &serde_json::Value) -> Result<Vec<Bind>, Never> {
    Ok(said
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|one| {
            let held = one.get("modmask").and_then(serde_json::Value::as_u64)?;
            let named = one.get("key").and_then(serde_json::Value::as_str).unwrap_or_default();
            let about =
                one.get("description").and_then(serde_json::Value::as_str).unwrap_or_default();

            Some(Bind {
                held,
                named: named.to_string(),
                about: about.to_string(),
            })
        })
        .collect())
}

pub fn wears(name: &str, which: usize) -> Result<Done, Never> {
    let Ok(telling) = Program::Hyprctl.command();

    switching(telling, name, which)
}

pub fn switching(mut telling: Command, name: &str, which: usize) -> Result<Done, Never> {
    Ok(match telling.args([SWITCH, name, &which.to_string()]).output() {
        Ok(said) => {
            let Ok(done) = took(&said);

            done
        }
        Err(fault) => Done::Refused(format!("no hyprctl to run: {fault}")),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    Taken,
    Refused(String),
}

const KB_LAYOUT: &str = "kb_layout";

const BY_THE_PARSER: &str = "non-legacy parsers";

const COMPLAINED: &str = "error";

fn took(said: &std::process::Output) -> Result<Done, Never> {
    let printed = String::from_utf8_lossy(&said.stdout);
    let read = printed.to_lowercase();

    Ok(
        match said.status.success()
            && !read.contains(COMPLAINED)
            && !read.contains(BY_THE_PARSER)
        {
            true => Done::Taken,
            false => Done::Refused(format!(
                "{}{}",
                printed.trim(),
                String::from_utf8_lossy(&said.stderr).trim()
            )),
        },
    )
}

pub fn offers(name: &str, layouts: &str) -> Result<Done, Never> {
    let Ok(telling) = Program::Hyprctl.command();

    laying_out(telling, name, layouts)
}

pub fn laying_out(telling: Command, name: &str, layouts: &str) -> Result<Done, Never> {
    let Ok(said) = quoted(name);
    let Ok(offered) = quoted(layouts);

    doing(
        telling,
        Told::Eval,
        &format!("hl.device({{ name = {said}, {KB_LAYOUT} = {offered} }})"),
    )
}

pub fn quoted(said: &str) -> Result<String, Never> {
    Ok(format!("\"{}\"", said.replace('\\', "\\\\").replace('"', "\\\"")))
}

pub fn told(what: Told, lua: &str) -> Result<Done, Never> {
    let Ok(telling) = Program::Hyprctl.command();

    doing(telling, what, lua)
}

pub fn doing(mut telling: Command, what: Told, lua: &str) -> Result<Done, Never> {
    let Ok(word) = what.word();

    Ok(match telling.args([word, lua]).output() {
        Ok(said) => {
            let Ok(done) = took(&said);

            done
        }
        Err(fault) => Done::Refused(format!("no hyprctl to run: {fault}")),
    })
}

pub fn surfaces(
    layers: &serde_json::Value,
) -> Result<impl Iterator<Item = &serde_json::Value>, Never> {
    Ok(layers
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(_, screen)| {
            let levels = screen.get("levels")?;

            levels.as_object()
        })
        .flatten()
        .filter_map(|(_, level)| level.as_array())
        .flatten())
}

pub fn namespace(surface: &serde_json::Value) -> Result<Option<&str>, Never> {
    Ok(surface.get("namespace").and_then(serde_json::Value::as_str))
}

pub fn tall(surface: &serde_json::Value) -> Result<Option<i64>, Never> {
    Ok(surface.get("h").and_then(serde_json::Value::as_i64))
}

pub fn address(what: &serde_json::Value) -> Result<Option<&str>, Never> {
    Ok(what.get("address").and_then(serde_json::Value::as_str))
}

pub fn clients(
    clients: &serde_json::Value,
) -> Result<impl Iterator<Item = &serde_json::Value>, Never> {
    Ok(clients.as_array().into_iter().flatten())
}

pub fn workspace_of(client: &serde_json::Value) -> Result<Option<&str>, Never> {
    Ok(client
        .get("workspace")
        .and_then(|workspace| workspace.get("name"))
        .and_then(serde_json::Value::as_str))
}

pub fn windows(workspace: &serde_json::Value) -> Result<Option<i64>, Never> {
    Ok(workspace.get("windows").and_then(serde_json::Value::as_i64))
}

pub fn workspace(workspace: &serde_json::Value) -> Result<Option<&str>, Never> {
    Ok(workspace.get("name").and_then(serde_json::Value::as_str))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Corner {
    pub across: i64,
    pub down: i64,
    pub wide: i64,
    pub tall: i64,
}

pub fn corner(surface: &serde_json::Value) -> Result<Option<Corner>, Never> {
    let said = |name: &str| surface.get(name).and_then(serde_json::Value::as_i64);

    let across = match said("x") {
        Some(across) => across,
        None => return Ok(None),
    };

    let down = match said("y") {
        Some(down) => down,
        None => return Ok(None),
    };

    let wide = match said("w") {
        Some(wide) => wide,
        None => return Ok(None),
    };

    let tall = match said("h") {
        Some(tall) => tall,
        None => return Ok(None),
    };

    Ok(Some(Corner { across, down, wide, tall }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub named: String,
    pub size: Option<(i64, i64)>,
    pub scale: Option<f64>,
    pub refresh: Option<f64>,
    pub transform: Option<i64>,
}

pub fn monitors(monitors: &serde_json::Value) -> Result<Vec<Monitor>, Never> {
    Ok(monitors
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|monitor| {
            let named = monitor.get("name").and_then(serde_json::Value::as_str)?;
            let wide = monitor.get("width").and_then(serde_json::Value::as_i64);
            let tall = monitor.get("height").and_then(serde_json::Value::as_i64);

            Some(Monitor {
                named: named.to_string(),
                size: wide.zip(tall),
                scale: monitor.get("scale").and_then(serde_json::Value::as_f64),
                refresh: monitor.get("refreshRate").and_then(serde_json::Value::as_f64),
                transform: monitor.get("transform").and_then(serde_json::Value::as_i64),
            })
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Floating {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pinned {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filling {
    Nothing,
    Maximized,
    Screen,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub address: String,
    pub title: String,
    pub first_class: String,
    pub first_title: String,
    pub workspace: i64,
    pub workspace_named: String,
    pub monitor: Option<i64>,
    pub floating: Floating,
    pub pinned: Pinned,
    pub filling: Filling,
    pub at: (i64, i64),
    pub size: (i64, i64),
    pub pid: i64,
}

fn pair(window: &serde_json::Value, name: &str) -> Result<(i64, i64), Never> {
    let said = window.get(name);

    let across = said.and_then(|both| both.get(0)).and_then(serde_json::Value::as_i64);
    let down = said.and_then(|both| both.get(1)).and_then(serde_json::Value::as_i64);

    Ok(match (across, down) {
        (Some(across), Some(down)) => (across, down),
        (None, _) | (_, None) => (0, 0),
    })
}

pub fn windows_open(clients: &serde_json::Value) -> Result<Vec<Window>, Never> {
    let mut open = Vec::new();

    for window in clients.as_array().into_iter().flatten() {
        let word = |name: &str| window.get(name).and_then(serde_json::Value::as_str);
        let whether = |name: &str| window.get(name).and_then(serde_json::Value::as_bool);

        let address = match word("address") {
            Some(address) => address,
            None => continue,
        };

        let workspace = window.get("workspace");
        let Ok(at) = pair(window, "at");
        let Ok(size) = pair(window, "size");

        open.push(Window {
            address: address.to_string(),
            title: word("title").unwrap_or_default().to_string(),
            first_class: word("initialClass").unwrap_or_default().to_string(),
            first_title: word("initialTitle").unwrap_or_default().to_string(),
            workspace: workspace
                .and_then(|workspace| workspace.get("id"))
                .and_then(serde_json::Value::as_i64)
                .unwrap_or_default(),
            workspace_named: workspace
                .and_then(|workspace| workspace.get("name"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            monitor: window.get("monitor").and_then(serde_json::Value::as_i64),
            floating: match whether("floating") {
                Some(true) => Floating::Yes,
                Some(false) | None => Floating::No,
            },
            pinned: match whether("pinned") {
                Some(true) => Pinned::Yes,
                Some(false) | None => Pinned::No,
            },
            filling: match window.get("fullscreen").and_then(serde_json::Value::as_i64) {
                Some(said) => match said {
                    1 => Filling::Maximized,
                    2 => Filling::Screen,
                    _filling_nothing => Filling::Nothing,
                },
                None => Filling::Nothing,
            },
            at,
            size,
            pid: window.get("pid").and_then(serde_json::Value::as_i64).unwrap_or_default(),
        });
    }

    Ok(open)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str) -> serde_json::Value {
        read(text).expect("what hyprctl said")
    }

    const TWO_SCREENS: &str = r#"[
        {"name":"eDP-1","width":2560,"height":1600,"scale":2.50,
         "refreshRate":143.97,"transform":1},
        {"name":"HEADLESS-2","width":1600,"height":2560,"scale":1.00}]"#;

    #[test]
    fn a_screen_is_named_measured_and_scaled() {
        let Ok(monitors) = monitors(&said(TWO_SCREENS));

        assert_eq!(
            monitors.first(),
            Some(&Monitor {
                named: "eDP-1".to_string(),
                size: Some((2560, 1600)),
                scale: Some(2.5),
                refresh: Some(143.97),
                transform: Some(1),
            })
        );
    }

    #[test]
    fn every_screen_is_answered_in_the_order_the_compositor_gave_them() {
        let Ok(monitors) = monitors(&said(TWO_SCREENS));
        let named: Vec<&str> = monitors.iter().map(|monitor| monitor.named.as_str()).collect();

        assert_eq!(named, ["eDP-1", "HEADLESS-2"]);
    }

    #[test]
    fn a_screen_that_says_nothing_about_a_turn_is_not_a_screen_standing_upright() {
        let Ok(monitors) = monitors(&said(TWO_SCREENS));

        assert_eq!(monitors.get(1).and_then(|monitor| monitor.transform), None);
    }

    #[test]
    fn a_screen_that_says_no_size_is_a_screen_without_one_rather_than_one_of_nought() {
        let Ok(monitors) = monitors(&said(r#"[{"name":"HEADLESS-2"}]"#));

        assert_eq!(
            monitors.first(),
            Some(&Monitor {
                named: "HEADLESS-2".to_string(),
                size: None,
                scale: None,
                refresh: None,
                transform: None,
            })
        );
    }

    #[test]
    fn a_screen_with_no_name_is_no_screen_anybody_could_ask_after() {
        let Ok(monitors) = monitors(&said(r#"[{"width":2560,"height":1600}]"#));

        assert_eq!(monitors, []);
    }

    #[test]
    fn nothing_at_all_is_no_screens_rather_than_a_reading_of_the_wrong_thing() {
        let Ok(monitors) = monitors(&said(r#"{"eDP-1":{"levels":{}}}"#));

        assert_eq!(monitors, []);
    }

    const IN_FRONT: &str = r#"{"id":3,"name":"3","windows":2,"lastwindowtitle":"a folder"}"#;

    #[test]
    fn the_workspace_in_front_says_its_name_and_what_it_holds() {
        assert_eq!(windows(&said(IN_FRONT)), Ok(Some(2)));
        assert_eq!(workspace(&said(IN_FRONT)), Ok(Some("3")));
    }

    #[test]
    fn a_workspace_that_counts_nothing_is_not_a_workspace_holding_nothing() {
        assert_eq!(windows(&said(r#"{"id":3,"name":"3"}"#)), Ok(None));
        assert_eq!(windows(&said(r#"{"id":3,"name":"3","windows":0}"#)), Ok(Some(0)));
    }

    #[test]
    fn a_surface_stands_where_the_compositor_says_it_does() {
        let said = said(r#"{"namespace":"launcher","x":260,"y":140,"w":1400,"h":900}"#);

        assert_eq!(
            corner(&said),
            Ok(Some(Corner { across: 260, down: 140, wide: 1400, tall: 900 }))
        );
    }

    #[test]
    fn a_corner_half_said_is_no_corner_rather_than_a_corner_with_nought_in_it() {
        let said = said(r#"{"namespace":"launcher","y":140,"w":1400,"h":900}"#);

        assert_eq!(corner(&said), Ok(None));
    }

    const ON_THE_SCREEN: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","address":"0x1","h":1600}],
        "2":[{"namespace":"waybar","address":"0x2","h":38}],
        "3":[{"namespace":"launcher","address":"0x3","h":0}]}}}"#;

    #[test]
    fn every_surface_is_walked_whichever_level_it_is_drawn_on() {
        let said = said(ON_THE_SCREEN);
        let Ok(surfaces) = surfaces(&said);
        let named: Vec<&str> = surfaces
            .filter_map(|surface| {
                let Ok(named) = namespace(surface);

                named
            })
            .collect();

        assert_eq!(named, ["awww-daemon", "waybar", "launcher"]);
    }

    #[test]
    fn a_surface_nobody_can_see_is_still_a_surface_the_walk_hands_over() {
        let said = said(ON_THE_SCREEN);
        let Ok(surfaces) = surfaces(&said);
        let hidden = surfaces
            .filter_map(|surface| {
                let Ok(at) = address(surface);

                at
            })
            .collect::<Vec<&str>>();

        assert_eq!(hidden, ["0x1", "0x2", "0x3"]);
    }

    #[test]
    fn the_monitors_answer_walked_as_layers_is_nothing_rather_than_a_guess() {
        let said = said(TWO_SCREENS);
        let Ok(surfaces) = surfaces(&said);

        assert_eq!(surfaces.count(), 0);
    }

    const OPEN: &str = r#"[
        {"address":"0xa","workspace":{"id":3,"name":"3"}},
        {"address":"0xb","workspace":{"id":4,"name":"4"}}]"#;

    #[test]
    fn a_window_says_where_it_is_and_what_it_is_called() {
        let said = said(OPEN);
        let Ok(mut clients) = clients(&said);
        let first = clients.next();

        assert_eq!(first.map(address), Some(Ok(Some("0xa"))));
        assert_eq!(first.map(workspace_of), Some(Ok(Some("3"))));
    }

    #[test]
    fn no_windows_open_is_an_answer_and_not_a_failure() {
        let said = said("[]");
        let Ok(clients) = clients(&said);

        assert_eq!(clients.count(), 0);
    }

    #[test]
    fn a_question_is_spelled_the_way_hyprctl_spells_it() {
        assert_eq!(Asked::Layers.words(), Ok(&["layers", "-j"][..]));
        assert_eq!(Asked::EveryMonitor.words(), Ok(&["monitors", "all", "-j"][..]));
    }

    #[test]
    fn what_would_not_be_read_says_so_rather_than_answering() {
        let said = match read("hyprland is not running") {
            Ok(_answered) => "it was read as an answer",
            Err(_why) => "it was not read",
        };

        assert_eq!(said, "it was not read", "an answer nobody can read is not an empty desktop");
    }

    const TWO_WINDOWS: &str = r#"[
        {"address":"0x1","title":"a shell","initialClass":"foot",
         "initialTitle":"foot","workspace":{"id":2,"name":"2"},"monitor":0,
         "floating":false,"pinned":false,"fullscreen":0,
         "at":[10,20],"size":[800,600],"pid":4242},
        {"address":"0x2","title":"pictures","initialClass":"imv",
         "initialTitle":"imv","workspace":{"id":-99,"name":"special:sky"},
         "floating":true,"pinned":true,"fullscreen":2,
         "at":[0,0],"size":[1024,640],"pid":4243}]"#;

    #[test]
    fn a_window_is_read_by_what_it_was_called_when_it_opened() {
        let Ok(open) = windows_open(&said(TWO_WINDOWS));
        let first = open.first().map(|window| window.first_class.as_str());

        assert_eq!(first, Some("foot"));
    }

    #[test]
    fn what_a_window_is_doing_is_a_name_rather_than_a_flag() {
        let Ok(open) = windows_open(&said(TWO_WINDOWS));
        let doing = open
            .iter()
            .map(|window| (window.floating, window.pinned, window.filling))
            .collect::<Vec<_>>();

        assert_eq!(
            doing,
            [
                (Floating::No, Pinned::No, Filling::Nothing),
                (Floating::Yes, Pinned::Yes, Filling::Screen),
            ]
        );
    }

    #[test]
    fn a_workspace_is_kept_by_its_number_and_by_its_name() {
        let Ok(open) = windows_open(&said(TWO_WINDOWS));
        let sky = open.get(1).map(|window| (window.workspace, window.workspace_named.as_str()));

        assert_eq!(sky, Some((-99, "special:sky")));
    }

    #[test]
    fn a_window_on_no_screen_says_so_rather_than_answering_the_first_one() {
        let Ok(open) = windows_open(&said(TWO_WINDOWS));
        let screens = open.iter().map(|window| window.monitor).collect::<Vec<_>>();

        assert_eq!(screens, [Some(0), None]);
    }

    #[test]
    fn a_window_with_no_address_is_not_a_window_this_can_be_told_about() {
        let Ok(open) = windows_open(&said(r#"[{"title":"nameless"},{"address":"0x3"}]"#));
        let addresses = open.iter().map(|window| window.address.as_str()).collect::<Vec<_>>();

        assert_eq!(addresses, ["0x3"]);
    }

    #[test]
    fn where_a_window_is_and_how_big_it_is_come_back_as_pairs() {
        let Ok(open) = windows_open(&said(TWO_WINDOWS));
        let first = open.first().map(|window| (window.at, window.size));

        assert_eq!(first, Some(((10, 20), (800, 600))));
    }
}
