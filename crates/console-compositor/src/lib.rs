//! What the compositor is asked, and what its answers mean.
//!
//! The program is `console_core_external_programs::Program::Hyprctl`, and that
//! was the only part of this anyone owned. What came back was read wherever
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
//! is not an answer anyone else should be given. So asking is one function
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
//! Each question is its own type, and what it is answered with is that type's
//! own: `ask(Layers)` is a list of layers. It used to be one `Answer` enum for
//! every question, so a caller that had asked for the layers matched every
//! other kind of answer to refuse it -- a dozen crates writing an arm for
//! something that could not happen, some quietly reading it as an empty desktop
//! and some as a fault, and none of them able to be told apart from a real
//! refusal. The only way to fail now is the compositor not answering.
//!
//! What is not here is which surfaces belong to the system, which namespace the home
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
//! So a keyword is `hl.config` or `hl.device` through [`Request::Script`], a
//! dispatcher is the `hl.dsp.*` object [`Request::Dispatch`] already wraps,
//! and the two verbs that are hyprctl's own rather than the config's --
//! `switchxkblayout` among them -- are called as themselves, with no verb in
//! front. [`DispatchResult::Failure`] now knows that sentence by heart,
//! because the one thing worse than a compositor that will not take something
//! is one that says so where no one is looking.
//!
//! Which is also why [`onto`] is here rather than written out wherever a
//! workspace is asked for. The daemon, the two test stages and the bar each
//! spelled that dispatcher themselves, and the bar spelled it in hyprctl's own
//! words: `dispatch workspace 2` reaches a lua config as
//! `hl.dispatch(workspace 2)`, which is a syntax error, so a tap on a
//! workspace along the bar did nothing at all and said so nowhere.
//!
//! `stirred` is the same crate's other half: what the compositor says when
//! no one asked it anything. Four crates were reading those lines by the words
//! Hyprland spells them with, and two of the four lists had already drifted
//! apart, which is what a literal in two places does when no one has decided
//! twice.

use std::fmt;
use std::process::Command;

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_u32};
use console_core_words::Words;

pub mod events;
pub mod socket;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Query {
    #[words(about = "what is on the screen")]
    Layers,
    #[words(about = "the workspace in front")]
    ActiveWorkspace,
    #[words(about = "the workspaces there are")]
    Workspaces,
    #[words(about = "what the screens are")]
    Monitors,
    #[words(about = "every screen it has")]
    EveryMonitor,
    #[words(about = "the windows that are open")]
    Clients,
    #[words(about = "what is plugged in")]
    Devices,
    #[words(about = "the keys it is holding")]
    Binds,
}

impl Query {
    pub fn words(self) -> Result<&'static [&'static str], Never> {
        Ok(match self {
            Query::Layers => &["layers", "-j"],
            Query::ActiveWorkspace => &["activeworkspace", "-j"],
            Query::Workspaces => &["workspaces", "-j"],
            Query::Monitors => &["monitors", "-j"],
            Query::EveryMonitor => &["monitors", "all", "-j"],
            Query::Clients => &["clients", "-j"],
            Query::Devices => &["devices", "-j"],
            Query::Binds => &["binds", "-j"],
        })
    }

    pub fn line(self) -> Result<String, Never> {
        let Ok(words) = self.words();
        let asked: Vec<&str> = words.iter().copied().filter(|word| *word != JSON).collect();

        Ok(format!("j/{}", asked.join(" ")))
    }
}

const JSON: &str = "-j";

#[derive(Debug)]
pub enum HyprctlError {
    Spawn(Query, std::io::Error),
    Socket(Query, socket::SocketError),
    Failed(Query, String),
    Parse(Query, serde_json::Error),
    Json(serde_json::Error),
    MissingField(Query, &'static str),
}

impl fmt::Display for HyprctlError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HyprctlError::Spawn(query, fault) => {
                let Ok(about) = query.about();

                write!(to, "asking hyprctl {about}: {fault}")
            }
            HyprctlError::Socket(query, fault) => {
                let Ok(about) = query.about();

                write!(to, "asking the compositor {about}: {fault}")
            }
            HyprctlError::Failed(query, value) => {
                let Ok(about) = query.about();

                write!(to, "hyprctl would not say {about}: {value}")
            }
            HyprctlError::Parse(query, fault) => {
                let Ok(about) = query.about();

                write!(to, "reading hyprctl's answer about {about}: {fault}")
            }
            HyprctlError::Json(fault) => write!(to, "reading what hyprctl said: {fault}"),
            HyprctlError::MissingField(query, field) => {
                let Ok(about) = query.about();

                write!(to, "hyprctl's answer about {about} missing field {field}")
            }
        }
    }
}

impl std::error::Error for HyprctlError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    pub id: i64,
    pub named: String,
    pub windows: Option<i64>,
}

pub trait Question: Copy {
    const ASKED: Query;

    type Reply;

    fn said(value: serde_json::Value) -> Result<Self::Reply, HyprctlError>;
}

macro_rules! questions {
    ($($question:ident => $said:ty, $parse:ident;)*) => {
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct $question;

            impl Question for $question {
                const ASKED: Query = Query::$question;

                type Reply = $said;

                fn said(value: serde_json::Value) -> Result<$said, HyprctlError> {
                    $parse(value)
                }
            }
        )*
    };
}

questions! {
    Layers => Vec<Layer>, parse_layers;
    ActiveWorkspace => Option<Workspace>, parse_active_workspace;
    Workspaces => Vec<Workspace>, parse_workspaces;
    Monitors => Vec<Monitor>, parse_monitors;
    EveryMonitor => Vec<Monitor>, parse_monitors;
    Clients => Vec<Window>, parse_clients;
    Devices => Vec<Keyboard>, parse_devices;
    Binds => Vec<BoundKey>, parse_binds;
}

pub fn ask<Q: Question>(question: Q) -> Result<Q::Reply, HyprctlError> {
    let at = socket::socket(socket::Socket::Requests)
        .map_err(|why| HyprctlError::Socket(Q::ASKED, socket::SocketError::Unplaced(why)))?;

    ask_at(&at, question)
}

pub fn ask_at<Q: Question>(at: &std::path::Path, _question: Q) -> Result<Q::Reply, HyprctlError> {
    let Ok(line) = Q::ASKED.line();
    let printed = socket::send_at(at, &line).map_err(|fault| HyprctlError::Socket(Q::ASKED, fault))?;
    let value: serde_json::Value =
        serde_json::from_slice(&printed).map_err(|fault| HyprctlError::Parse(Q::ASKED, fault))?;

    Q::said(value)
}

pub fn ask_with<Q: Question>(mut command: Command, _question: Q) -> Result<Q::Reply, HyprctlError> {
    let Ok(words) = Q::ASKED.words();

    let output = command
        .args(words)
        .output()
        .map_err(|fault| HyprctlError::Spawn(Q::ASKED, fault))?;

    let printed = match output.status.success() {
        true => output.stdout,
        false => {
            return Err(HyprctlError::Failed(
                Q::ASKED,
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }
    };

    let value: serde_json::Value =
        serde_json::from_slice(&printed).map_err(|fault| HyprctlError::Parse(Q::ASKED, fault))?;

    Q::said(value)
}

pub fn read<Q: Question>(_question: Q, text: &str) -> Result<Q::Reply, HyprctlError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(HyprctlError::Json)?;

    Q::said(value)
}

pub fn said_of<Q: Question>(_question: Q, value: serde_json::Value) -> Result<Q::Reply, HyprctlError> {
    Q::said(value)
}

pub fn read_value(text: &str) -> Result<serde_json::Value, HyprctlError> {
    serde_json::from_str(text).map_err(HyprctlError::Json)
}

fn parse_layers(value: serde_json::Value) -> Result<Vec<Layer>, HyprctlError> {
    let screens = match value.as_object() {
        Some(screens) => screens,
        None => return Err(HyprctlError::MissingField(Query::Layers, "a screen")),
    };

    let layers = screens
        .values()
        .filter_map(|screen| screen.get("levels").and_then(serde_json::Value::as_object))
        .flat_map(serde_json::Map::values)
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(|one| {
            let Ok(layer) = parse_layer(one);

            layer
        })
        .collect();

    Ok(layers)
}

fn parse_layer(value: &serde_json::Value) -> Result<Option<Layer>, Never> {
    let namespace = match value.get("namespace").and_then(serde_json::Value::as_str) {
        Some(namespace) => namespace.to_string(),
        None => return Ok(None),
    };

    let address = value.get("address").and_then(serde_json::Value::as_str).map(str::to_string);
    let x = value.get("x").and_then(serde_json::Value::as_i64);
    let y = value.get("y").and_then(serde_json::Value::as_i64);
    let width = value.get("w").and_then(serde_json::Value::as_i64);
    let height = value.get("h").and_then(serde_json::Value::as_i64);

    Ok(Some(Layer { namespace, address, x, y, width, height }))
}

fn parse_active_workspace(value: serde_json::Value) -> Result<Option<Workspace>, HyprctlError> {
    match value.get("id").and_then(serde_json::Value::as_i64) {
        Some(_named_below) => {},
        None => return Err(HyprctlError::MissingField(Query::ActiveWorkspace, "id")),
    }

    let Ok(workspace) = parse_workspace(&value);

    Ok(workspace)
}

fn parse_workspaces(value: serde_json::Value) -> Result<Vec<Workspace>, HyprctlError> {
    let listed = match value.as_array() {
        Some(listed) => listed,
        None => return Err(HyprctlError::MissingField(Query::Workspaces, "a list")),
    };

    let mut found: Vec<Workspace> = listed
        .iter()
        .filter_map(|one| {
            let Ok(workspace) = parse_workspace(one);

            workspace
        })
        .collect();

    found.sort_by_key(|one| one.id);

    Ok(found)
}

fn parse_workspace(value: &serde_json::Value) -> Result<Option<Workspace>, Never> {
    let id = match value.get("id").and_then(serde_json::Value::as_i64) {
        Some(id) => id,
        None => return Ok(None),
    };

    let windows = value.get("windows").and_then(serde_json::Value::as_i64);

    let named = match value.get("name").and_then(serde_json::Value::as_str) {
        Some(named) => named.to_string(),
        None => id.to_string(),
    };

    Ok(Some(Workspace { id, named, windows }))
}

fn parse_monitors(value: serde_json::Value) -> Result<Vec<Monitor>, HyprctlError> {
    let listed = match value.as_array() {
        Some(listed) => listed,
        None => return Err(HyprctlError::MissingField(Query::Monitors, "a list")),
    };

    let mut monitors = Vec::new();

    for monitor in listed {
        let named = match monitor.get("name").and_then(serde_json::Value::as_str) {
            Some(named) => named.to_string(),
            None => return Err(HyprctlError::MissingField(Query::Monitors, "name")),
        };

        let wide = monitor.get("width").and_then(serde_json::Value::as_i64);
        let tall = monitor.get("height").and_then(serde_json::Value::as_i64);
        let scale = monitor.get("scale").and_then(serde_json::Value::as_f64);
        let refresh = monitor.get("refreshRate").and_then(serde_json::Value::as_f64);
        let transform = monitor.get("transform").and_then(serde_json::Value::as_i64);

        monitors.push(Monitor {
            named,
            size: wide.zip(tall),
            scale,
            refresh,
            transform,
        });
    }

    Ok(monitors)
}

fn parse_clients(value: serde_json::Value) -> Result<Vec<Window>, HyprctlError> {
    let listed = match value.as_array() {
        Some(listed) => listed,
        None => return Err(HyprctlError::MissingField(Query::Clients, "a list")),
    };

    Ok(listed
        .iter()
        .filter_map(|one| {
            let Ok(window) = parse_window(one);

            window
        })
        .collect())
}

fn parse_window(window: &serde_json::Value) -> Result<Option<Window>, Never> {
    let address = match window.get("address").and_then(serde_json::Value::as_str) {
        Some(address) => address.to_string(),
        None => return Ok(None),
    };

    let Ok(title) = string_or_empty(window, "title");
    let Ok(first_class) = string_or_empty(window, "initialClass");
    let Ok(first_title) = string_or_empty(window, "initialTitle");

    let workspace = window.get("workspace");
    let workspace_id = match workspace.and_then(|one| one.get("id")).and_then(serde_json::Value::as_i64) {
        Some(id) => id,
        None => NO_WORKSPACE,
    };

    let workspace_named = match workspace
        .and_then(|one| one.get("name"))
        .and_then(serde_json::Value::as_str)
    {
        Some(named) => named.to_string(),
        None => workspace_id.to_string(),
    };

    let monitor = window.get("monitor").and_then(serde_json::Value::as_i64);

    let floating = match window.get("floating").and_then(serde_json::Value::as_bool) {
        Some(true) => Floating::Yes,
        Some(false) | None => Floating::No,
    };

    let pinned = match window.get("pinned").and_then(serde_json::Value::as_bool) {
        Some(true) => Pinned::Yes,
        Some(false) | None => Pinned::No,
    };

    let filling = match window.get("fullscreen").and_then(serde_json::Value::as_i64) {
        Some(MAXIMIZED) => Filling::Maximized,
        Some(SCREEN) => Filling::Screen,
        Some(_nothing_hyprland_names_for) => Filling::None,
        None => Filling::None,
    };

    let Ok(at) = parse_pair(window, "at");
    let Ok(size) = parse_pair(window, "size");

    let at = match at {
        Some(at) => at,
        None => ORIGIN,
    };

    let size = match size {
        Some(size) => size,
        None => ORIGIN,
    };

    let pid = match window.get("pid").and_then(serde_json::Value::as_i64) {
        Some(pid) => pid,
        None => NO_PROCESS,
    };

    Ok(Some(Window {
        address,
        title,
        first_class,
        first_title,
        workspace: workspace_id,
        workspace_named,
        monitor,
        floating,
        pinned,
        filling,
        at,
        size,
        pid,
    }))
}

fn string_or_empty(window: &serde_json::Value, name: &str) -> Result<String, Never> {
    Ok(match window.get(name).and_then(serde_json::Value::as_str) {
        Some(value) => value.to_string(),
        None => String::new(),
    })
}

const NO_WORKSPACE: i64 = 0;

const NO_PROCESS: i64 = 0;

const ORIGIN: (i64, i64) = (0, 0);

const MAXIMIZED: i64 = 1;

const SCREEN: i64 = 2;

fn parse_pair(window: &serde_json::Value, name: &str) -> Result<Option<(i64, i64)>, Never> {
    let pair = match window.get(name).and_then(serde_json::Value::as_array) {
        Some(pair) => pair,
        None => return Ok(None),
    };

    let across = match pair.first().and_then(serde_json::Value::as_i64) {
        Some(across) => across,
        None => return Ok(None),
    };

    let down = match pair.get(1).and_then(serde_json::Value::as_i64) {
        Some(down) => down,
        None => return Ok(None),
    };

    Ok(Some((across, down)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub namespace: String,
    pub address: Option<String>,
    pub x: Option<i64>,
    pub y: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

fn parse_devices(value: serde_json::Value) -> Result<Vec<Keyboard>, HyprctlError> {
    let listed = match value.get("keyboards").and_then(serde_json::Value::as_array) {
        Some(listed) => listed.clone(),
        None => return Ok(Vec::new()),
    };

    let mut keyboards = Vec::new();

    for one in &listed {
        let name = match one.get("name").and_then(serde_json::Value::as_str) {
            Some(name) => name.to_string(),
            None => return Err(HyprctlError::MissingField(Query::Devices, "name")),
        };

        let layouts = match one.get("layout").and_then(serde_json::Value::as_str) {
            Some(value) => value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect(),
            None => Vec::new(),
        };

        let wearing = one
            .get("active_keymap")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);

        let leading = match one.get("main").and_then(serde_json::Value::as_bool) {
            Some(true) => Leading::Yes,
            Some(false) | None => Leading::No,
        };

        keyboards.push(Keyboard { name, layouts, wearing, leading });
    }

    Ok(keyboards)
}

fn parse_binds(value: serde_json::Value) -> Result<Vec<BoundKey>, HyprctlError> {
    let listed = match value.as_array() {
        Some(listed) => listed,
        None => return Err(HyprctlError::MissingField(Query::Binds, "a list")),
    };

    let mut binds = Vec::new();

    for one in listed {
        let held = match one.get("modmask").and_then(serde_json::Value::as_u64) {
            Some(held) => held,
            None => return Err(HyprctlError::MissingField(Query::Binds, "modmask")),
        };

        let named = match one.get("key").and_then(serde_json::Value::as_str) {
            Some(named) => named.to_string(),
            None => return Err(HyprctlError::MissingField(Query::Binds, "key")),
        };

        let about = match one.get("description").and_then(serde_json::Value::as_str) {
            Some(about) => about.to_string(),
            None => return Err(HyprctlError::MissingField(Query::Binds, "description")),
        };

        binds.push(BoundKey { held, named, about });
    }

    Ok(binds)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Request {
    #[words(word = "dispatch")]
    Dispatch,
    #[words(word = "eval")]
    Script,
}

const SWITCH: &str = "switchxkblayout";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leading {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyboard {
    pub name: String,
    pub layouts: Vec<String>,
    pub wearing: Option<String>,
    pub leading: Leading,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundKey {
    pub held: u64,
    pub named: String,
    pub about: String,
}

pub fn switch_layout(name: &str, which: u32) -> Result<DispatchResult, Never> {
    dispatch(&format!("/{SWITCH} {name} {which}"))
}

pub fn switch_layout_with(mut command: Command, name: &str, which: u32) -> Result<DispatchResult, Never> {
    Ok(match command.args([SWITCH, name, &which.to_string()]).output() {
        Ok(output) => {
            let Ok(result) = result_of(&output);

            result
        }
        Err(fault) => DispatchResult::Failure(format!("no hyprctl to run: {fault}")),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchResult {
    Success,
    Failure(String),
}

const KB_LAYOUT: &str = "kb_layout";

const BY_THE_PARSER: &str = "non-legacy parsers";

const COMPLAINED: &str = "error";

fn result_of(output: &std::process::Output) -> Result<DispatchResult, Never> {
    let printed = String::from_utf8_lossy(&output.stdout);

    Ok(match output.status.success() {
        true => {
            let Ok(result) = parse_dispatch_result(&printed);

            result
        }
        false => DispatchResult::Failure(format!(
            "{}{}",
            printed.trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    })
}

fn parse_dispatch_result(printed: &str) -> Result<DispatchResult, Never> {
    let read = printed.to_lowercase();

    Ok(match !read.contains(COMPLAINED) && !read.contains(BY_THE_PARSER) {
        true => DispatchResult::Success,
        false => DispatchResult::Failure(printed.trim().to_string()),
    })
}

fn dispatch(line: &str) -> Result<DispatchResult, Never> {
    let at = match socket::socket(socket::Socket::Requests) {
        Ok(at) => at,
        Err(why) => return Ok(DispatchResult::Failure(why.to_string())),
    };

    dispatch_at(&at, line)
}

pub fn dispatch_at(at: &std::path::Path, line: &str) -> Result<DispatchResult, Never> {
    Ok(match socket::send_at(at, line) {
        Ok(printed) => {
            let Ok(result) = parse_dispatch_result(&String::from_utf8_lossy(&printed));

            result
        }
        Err(fault) => DispatchResult::Failure(fault.to_string()),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layouts<'a>(pub &'a str);

pub fn set_layouts(name: &str, layouts: Layouts<'_>) -> Result<DispatchResult, Never> {
    let Ok(lua) = device_layouts(name, layouts);

    request(Request::Script, &lua)
}

pub fn set_layouts_with(command: Command, name: &str, layouts: Layouts<'_>) -> Result<DispatchResult, Never> {
    let Ok(lua) = device_layouts(name, layouts);

    request_with(command, Request::Script, &lua)
}

fn device_layouts(name: &str, layouts: Layouts<'_>) -> Result<String, Never> {
    let Ok(quoted_name) = quote(name);
    let Ok(offered) = quote(layouts.0);

    Ok(format!("hl.device({{ name = {quoted_name}, {KB_LAYOUT} = {offered} }})"))
}

pub const CLOSE_WINDOW: &str = "hl.dsp.window.close()";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrying {
    Window,
    None,
}

pub fn onto(where_: &str, carrying: Carrying) -> Result<String, Never> {
    let dispatcher = match carrying {
        Carrying::Window => "hl.dsp.window.move",
        Carrying::None => "hl.dsp.focus",
    };
    let Ok(target) = quote(where_);

    Ok(format!("{dispatcher}({{workspace = {target}}})"))
}

pub fn quote(text: &str) -> Result<String, Never> {
    Ok(format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")))
}

pub fn request(request: Request, lua: &str) -> Result<DispatchResult, Never> {
    let Ok(line) = request.line(lua);

    dispatch(&line)
}

impl Request {
    pub fn line(self, lua: &str) -> Result<String, Never> {
        let Ok(word) = self.word();

        Ok(format!("/{word} {lua}"))
    }
}

pub fn request_with(mut command: Command, request: Request, lua: &str) -> Result<DispatchResult, Never> {
    let Ok(word) = request.word();

    Ok(match command.args([word, lua]).output() {
        Ok(output) => {
            let Ok(result) = result_of(&output);

            result
        }
        Err(fault) => DispatchResult::Failure(format!("no hyprctl to run: {fault}")),
    })
}

pub fn batch_arguments(commands: &[Vec<String>]) -> Result<Vec<String>, Never> {
    let Ok(hyprctl) = Program::Hyprctl.name();
    let Ok(dispatch) = Request::Dispatch.word();

    Ok(commands
        .iter()
        .filter(|arguments| arguments.first().is_some_and(|program| program.ends_with(hyprctl)))
        .filter(|arguments| arguments.get(1).is_some_and(|word| word == dispatch))
        .filter_map(|arguments| arguments.last().cloned())
        .collect())
}

pub fn clients(
    clients: &serde_json::Value,
) -> Result<impl Iterator<Item = &serde_json::Value>, Never> {
    Ok(clients.as_array().into_iter().flatten())
}

pub fn windows(workspace: &serde_json::Value) -> Result<Option<i64>, Never> {
    Ok(workspace.get("windows").and_then(serde_json::Value::as_i64))
}

pub fn workspace(workspace: &serde_json::Value) -> Result<Option<&str>, Never> {
    Ok(workspace.get("name").and_then(serde_json::Value::as_str))
}

pub fn front_of(monitor: &serde_json::Value) -> Result<Option<Workspace>, Never> {
    match monitor.get("activeWorkspace") {
        Some(value) => {
            let Ok(workspace) = parse_workspace(value);

            Ok(workspace)
        },
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visible {
    Yes,
    No,
}

impl Layer {
    pub fn visibility(&self) -> Result<Visible, Never> {
        Ok(match self.height {
            Some(tall) => match tall > 0 {
                true => Visible::Yes,
                false => Visible::No,
            },
            None => Visible::Yes,
        })
    }

    pub fn corner(&self) -> Result<Option<Frame>, Never> {
        let placed = match (self.x, self.y) {
            (Some(across), Some(down)) => (across, down),
            (Some(_across), None) => return Ok(None),
            (None, Some(_down)) => return Ok(None),
            (None, None) => return Ok(None),
        };

        let sized = match (self.width, self.height) {
            (Some(wide), Some(tall)) => (wide, tall),
            (Some(_wide), None) => return Ok(None),
            (None, Some(_tall)) => return Ok(None),
            (None, None) => return Ok(None),
        };

        Ok(Some(Frame { x: placed.0, y: placed.1, width: sized.0, height: sized.1 }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub named: String,
    pub size: Option<(i64, i64)>,
    pub scale: Option<f64>,
    pub refresh: Option<f64>,
    pub transform: Option<i64>,
}

impl Monitor {
    pub fn logical(&self) -> Result<Option<Size<u32>>, Never> {
        let ((wide, tall), scale) = match self.size.zip(self.scale) {
            Some(both) => both,
            None => return Ok(None),
        };

        let (wide, tall) = match self.transform {
            Some(transform) => match transform & 1 == 1 {
                true => (tall, wide),
                false => (wide, tall),
            },
            None => (wide, tall),
        };

        let Ok(wide) = wide.float();
        let Ok(tall) = tall.float();

        let Ok(across) = toward_zero_u32(wide / scale.max(f64::EPSILON));
        let Ok(down) = toward_zero_u32(tall / scale.max(f64::EPSILON));

        Ok(Some(Size { width: across, height: down }))
    }
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
    None,
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

const SAID_NOTHING: &str = "";

const SAID_NO_NUMBER: i64 = 0;

fn word_in(of: &serde_json::Value, name: &str) -> Result<String, Never> {
    Ok(match of.get(name).and_then(serde_json::Value::as_str) {
        Some(value) => value.to_string(),
        None => SAID_NOTHING.to_string(),
    })
}

fn number_field(of: &serde_json::Value, name: &str) -> Result<i64, Never> {
    Ok(match of.get(name).and_then(serde_json::Value::as_i64) {
        Some(number) => number,
        None => SAID_NO_NUMBER,
    })
}

fn pair(window: &serde_json::Value, name: &str) -> Result<(i64, i64), Never> {
    let value = window.get(name);

    let across = value.and_then(|both| both.get(0)).and_then(serde_json::Value::as_i64);
    let down = value.and_then(|both| both.get(1)).and_then(serde_json::Value::as_i64);

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

        let string_in = |name: &str| {
            let Ok(value) = word_in(window, name);

            value
        };

        let address = match word("address") {
            Some(address) => address,
            None => continue,
        };

        let workspace = window.get("workspace");
        let Ok(at) = pair(window, "at");
        let Ok(size) = pair(window, "size");

        open.push(Window {
            address: address.to_string(),
            title: string_in("title"),
            first_class: string_in("initialClass"),
            first_title: string_in("initialTitle"),
            workspace: match workspace {
                Some(workspace) => {
                    let Ok(id) = number_field(workspace, "id");

                    id
                }
                None => SAID_NO_NUMBER,
            },
            workspace_named: match workspace {
                Some(workspace) => {
                    let Ok(named) = word_in(workspace, "name");

                    named
                }
                None => SAID_NOTHING.to_string(),
            },
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
                Some(value) => match value {
                    1 => Filling::Maximized,
                    2 => Filling::Screen,
                    _filling_nothing => Filling::None,
                },
                None => Filling::None,
            },
            at,
            size,
            pid: {
                let Ok(pid) = number_field(window, "pid");

                pid
            },
        });
    }

    Ok(open)
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "HYPRLAND_INSTANCE_SIGNATURE names the compositor this session is talking to, and this crate is what talking to it means. Three crates read it before this existed and two of them disagreed about what an empty one was"
    )
)]
pub fn instance() -> Result<Option<String>, Never> {
    Ok(match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        Ok(value) => match value.is_empty() {
            true => None,
            false => Some(value),
        },
        Err(_unset) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    const TWO_SCREENS: &str = r#"[
        {"name":"eDP-1","width":2560,"height":1600,"scale":2.50,
         "refreshRate":143.97,"transform":1},
        {"name":"HEADLESS-2","width":1600,"height":2560,"scale":1.00}]"#;

    const TWO_BOARDS: &str = r#"{"keyboards":[
        {"name":"hl-virtual-keyboard-console-keyboard","layout":"us",
         "active_keymap":"English (US)","main":false},
        {"name":"lab31---keyboard","layout":"us,th",
         "active_keymap":"Thai","main":true}]}"#;

    #[test]
    fn the_board_someone_is_typing_on_is_the_one_the_compositor_leads_with() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_BOARDS)?;
        let keyboards = parse_devices(value)?;

        let leading: Vec<&str> = keyboards
            .iter()
            .filter(|keyboard| keyboard.leading == Leading::Yes)
            .map(|keyboard| keyboard.name.as_str())
            .collect();

        assert_eq!(leading, ["lab31---keyboard"], "{keyboards:?}");
        assert_eq!(keyboards.first().map(|keyboard| keyboard.leading), Some(Leading::No));

        Ok(())
    }

    #[test]
    fn a_screen_is_named_measured_and_scaled() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_SCREENS)?;
        let Ok(monitors) = monitors(&value);

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

        Ok(())
    }

    #[test]
    fn every_screen_is_answered_in_the_order_the_compositor_gave_them() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_SCREENS)?;
        let Ok(monitors) = monitors(&value);
        let named: Vec<&str> = monitors.iter().map(|monitor| monitor.named.as_str()).collect();

        assert_eq!(named, ["eDP-1", "HEADLESS-2"]);

        Ok(())
    }

    #[test]
    fn a_screen_that_says_nothing_about_a_turn_is_not_a_screen_standing_upright() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_SCREENS)?;
        let Ok(monitors) = monitors(&value);

        assert_eq!(monitors.get(1).and_then(|monitor| monitor.transform), None);

        Ok(())
    }

    #[test]
    fn a_screen_that_says_no_size_is_a_screen_without_one_rather_than_one_of_zero() -> Result<(), Box<dyn Error>> {
        let value = read_value(r#"[{"name":"HEADLESS-2"}]"#)?;
        let Ok(monitors) = monitors(&value);

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

        Ok(())
    }

    #[test]
    fn a_turned_screen_is_as_wide_as_it_looks_rather_than_as_wide_as_its_mode() -> Result<(), Box<dyn Error>> {
        let value = read_value(
            r#"[{"name":"HEADLESS-1","width":1600,"height":2560,"scale":2.5,"transform":1}]"#,
        )?;

        let Ok(monitors) = monitors(&value);
        let turned = monitors.first().ok_or("expected a screen")?;
        let Ok(logical) = turned.logical();

        assert_eq!(logical, Some(Size { width: 1024, height: 640 }));

        Ok(())
    }

    #[test]
    fn a_screen_standing_upright_is_its_mode_over_its_scale() -> Result<(), Box<dyn Error>> {
        let value = read_value(
            r#"[{"name":"HEADLESS-2","width":2560,"height":1600,"scale":2.0,"transform":0}]"#,
        )?;

        let Ok(monitors) = monitors(&value);
        let upright = monitors.first().ok_or("expected a screen")?;
        let Ok(logical) = upright.logical();

        assert_eq!(logical, Some(Size { width: 1280, height: 800 }));

        Ok(())
    }

    #[test]
    fn a_screen_that_will_not_say_how_big_it_is_says_nothing_rather_than_zero_by_zero() -> Result<(), Box<dyn Error>> {
        let value = read_value(r#"[{"name":"HEADLESS-2"}]"#)?;
        let Ok(monitors) = monitors(&value);
        let unmeasured = monitors.first().ok_or("expected a screen")?;

        assert_eq!(unmeasured.logical(), Ok(None));

        Ok(())
    }

    #[test]
    fn a_screen_with_no_name_is_no_screen_anyone_could_ask_after() -> Result<(), Box<dyn Error>> {
        let value = read_value(r#"[{"width":2560,"height":1600}]"#)?;
        let Ok(monitors) = monitors(&value);

        assert_eq!(monitors, []);

        Ok(())
    }

    #[test]
    fn nothing_at_all_is_no_screens_rather_than_a_reading_of_the_wrong_thing() -> Result<(), Box<dyn Error>> {
        let value = read_value(r#"{"eDP-1":{"levels":{}}}"#)?;
        let Ok(monitors) = monitors(&value);

        assert_eq!(monitors, []);

        Ok(())
    }

    const IN_FRONT: &str = r#"{"id":3,"name":"3","windows":2,"lastwindowtitle":"a folder"}"#;

    #[test]
    fn the_workspace_in_front_says_its_name_and_what_it_holds() -> Result<(), Box<dyn Error>> {
        let value = read_value(IN_FRONT)?;

        assert_eq!(windows(&value), Ok(Some(2)));
        assert_eq!(workspace(&value), Ok(Some("3")));

        Ok(())
    }

    #[test]
    fn a_workspace_that_counts_nothing_is_not_a_workspace_holding_nothing() -> Result<(), Box<dyn Error>> {
        let value = read_value(r#"{"id":3,"name":"3"}"#)?;

        assert_eq!(windows(&value), Ok(None));

        let value = read_value(r#"{"id":3,"name":"3","windows":0}"#)?;

        assert_eq!(windows(&value), Ok(Some(0)));

        Ok(())
    }

    fn one_layer(text: &str) -> Result<Layer, Box<dyn Error>> {
        let value = read_value(text)?;
        let Ok(layer) = parse_layer(&value);

        Ok(match layer {
            Some(layer) => layer,
            None => Layer { namespace: String::new(), address: None, x: None, y: None, width: None, height: None },
        })
    }

    #[test]
    fn a_surface_stands_where_the_compositor_says_it_does() -> Result<(), Box<dyn Error>> {
        let layer = one_layer(r#"{"namespace":"launcher","x":260,"y":140,"w":1400,"h":900}"#)?;

        assert_eq!(
            layer.corner(),
            Ok(Some(Frame { x: 260, y: 140, width: 1400, height: 900 }))
        );

        Ok(())
    }

    #[test]
    fn a_corner_half_said_is_no_corner_rather_than_a_corner_with_zero_in_it() -> Result<(), Box<dyn Error>> {
        let layer = one_layer(r#"{"namespace":"launcher","y":140,"w":1400,"h":900}"#)?;

        assert_eq!(layer.corner(), Ok(None));

        Ok(())
    }

    #[test]
    fn a_surface_the_compositor_gave_no_height_for_is_drawn_rather_than_gone() -> Result<(), Box<dyn Error>> {
        let unsaid = one_layer(r#"{"namespace":"launcher"}"#)?;
        let flat = one_layer(r#"{"namespace":"launcher","h":0}"#)?;
        let tall = one_layer(r#"{"namespace":"launcher","h":900}"#)?;

        assert_eq!(unsaid.visibility(), Ok(Visible::Yes));
        assert_eq!(flat.visibility(), Ok(Visible::No));
        assert_eq!(tall.visibility(), Ok(Visible::Yes));

        Ok(())
    }

    const ON_THE_SCREEN: &str = r#"{"eDP-1":{"levels":{
        "0":[{"namespace":"awww-daemon","address":"0x1","h":1600}],
        "2":[{"namespace":"console-bar","address":"0x2","h":40}],
        "3":[{"namespace":"launcher","address":"0x3","h":0}]}}}"#;

    fn layers_of(text: &str) -> Result<Vec<Layer>, Box<dyn Error>> {
        let value = read_value(text)?;
        let layers = said_of(Layers, value)?;

        Ok(layers)
    }

    #[test]
    fn every_surface_is_walked_whichever_level_it_is_drawn_on() -> Result<(), Box<dyn Error>> {
        let up = layers_of(ON_THE_SCREEN)?;
        let named: Vec<&str> = up.iter().map(|layer| layer.namespace.as_str()).collect();

        assert_eq!(named, ["awww-daemon", "console-bar", "launcher"]);

        Ok(())
    }

    #[test]
    fn a_surface_no_one_can_see_is_still_a_surface_the_walk_hands_over() -> Result<(), Box<dyn Error>> {
        let layers = layers_of(ON_THE_SCREEN)?;

        let hidden: Vec<String> =
            layers.into_iter().filter_map(|layer| layer.address).collect();

        assert_eq!(hidden, ["0x1", "0x2", "0x3"]);

        Ok(())
    }

    #[test]
    fn the_monitors_answer_walked_as_layers_is_nothing_rather_than_a_guess() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_SCREENS)?;
        let walked = match said_of(Layers, value) {
            Ok(_answer) => "an answer",
            Err(HyprctlError::MissingField(_, _)) => "a field it did not have",
            Err(
                HyprctlError::Spawn(_, _)
                | HyprctlError::Socket(_, _)
                | HyprctlError::Failed(_, _)
                | HyprctlError::Parse(_, _)
                | HyprctlError::Json(_),
            ) => "some other fault",
        };

        assert_eq!(walked, "a field it did not have");

        Ok(())
    }

    const OPEN: &str = r#"[
        {"address":"0xa","workspace":{"id":3,"name":"3"}},
        {"address":"0xb","workspace":{"id":4,"name":"4"}}]"#;

    #[test]
    fn a_window_says_where_it_is_and_what_it_is_called() -> Result<(), Box<dyn Error>> {
        let value = read_value(OPEN)?;
        let open = said_of(Clients, value)?;
        let first = open.first();

        assert_eq!(first.map(|window| window.address.as_str()), Some("0xa"));
        assert_eq!(first.map(|window| window.workspace_named.as_str()), Some("3"));

        Ok(())
    }

    #[test]
    fn no_windows_open_is_an_answer_and_not_a_failure() -> Result<(), Box<dyn Error>> {
        let value = read_value("[]")?;
        let Ok(clients) = clients(&value);

        assert_eq!(clients.count(), 0);

        Ok(())
    }

    #[test]
    fn a_question_is_spelled_the_way_hyprctl_spells_it() {
        assert_eq!(Query::Layers.words(), Ok(["layers", "-j"].as_slice()));
        assert_eq!(Query::EveryMonitor.words(), Ok(["monitors", "all", "-j"].as_slice()));
    }

    #[test]
    fn what_would_not_be_read_says_so_rather_than_answering() {
        let value = match read_value("hyprland is not running") {
            Ok(_answered) => "it was read as an answer",
            Err(_not_an_answer) => "it was not read",
        };

        assert_eq!(value, "it was not read", "an answer no one can read is not an empty desktop");
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
    fn a_window_is_read_by_what_it_was_called_when_it_opened() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_WINDOWS)?;
        let Ok(open) = windows_open(&value);
        let first = open.first().map(|window| window.first_class.as_str());

        assert_eq!(first, Some("foot"));

        Ok(())
    }

    #[test]
    fn what_a_window_is_doing_is_a_name_rather_than_a_flag() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_WINDOWS)?;
        let Ok(open) = windows_open(&value);
        let doing = open
            .iter()
            .map(|window| (window.floating, window.pinned, window.filling))
            .collect::<Vec<_>>();

        assert_eq!(
            doing,
            [
                (Floating::No, Pinned::No, Filling::None),
                (Floating::Yes, Pinned::Yes, Filling::Screen),
            ]
        );

        Ok(())
    }

    #[test]
    fn a_workspace_is_kept_by_its_number_and_by_its_name() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_WINDOWS)?;
        let Ok(open) = windows_open(&value);
        let sky = open.get(1).map(|window| (window.workspace, window.workspace_named.as_str()));

        assert_eq!(sky, Some((-99, "special:sky")));

        Ok(())
    }

    #[test]
    fn a_window_on_no_screen_says_so_rather_than_answering_the_first_one() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_WINDOWS)?;
        let Ok(open) = windows_open(&value);
        let screens = open.iter().map(|window| window.monitor).collect::<Vec<_>>();

        assert_eq!(screens, [Some(0), None]);

        Ok(())
    }

    #[test]
    fn a_window_with_no_address_is_not_a_window_this_can_be_told_about() -> Result<(), Box<dyn Error>> {
        let value = read_value(r#"[{"title":"untitled"},{"address":"0x3"}]"#)?;
        let Ok(open) = windows_open(&value);
        let addresses = open.iter().map(|window| window.address.as_str()).collect::<Vec<_>>();

        assert_eq!(addresses, ["0x3"]);

        Ok(())
    }

    #[test]
    fn where_a_window_is_and_how_big_it_is_come_back_as_pairs() -> Result<(), Box<dyn Error>> {
        let value = read_value(TWO_WINDOWS)?;
        let Ok(open) = windows_open(&value);
        let first = open.first().map(|window| (window.at, window.size));

        assert_eq!(first, Some(((10, 20), (800, 600))));

        Ok(())
    }

    fn workspaces_of(value: serde_json::Value) -> Result<Vec<Workspace>, Box<dyn Error>> {
        let found = said_of(Workspaces, value)?;

        Ok(found)
    }

    #[test]
    fn the_workspaces_come_back_in_the_order_a_bar_draws_them() -> Result<(), Box<dyn Error>> {
        let value = serde_json::json!([
            {"id": 3, "name": "3", "windows": 1},
            {"id": 1, "name": "1", "windows": 2},
            {"id": 2, "name": "2", "windows": 0}
        ]);
        let found = workspaces_of(value)?;
        let numbered: Vec<i64> = found.iter().map(|one| one.id).collect();

        assert_eq!(numbered, [1, 2, 3]);

        Ok(())
    }

    #[test]
    fn a_workspace_with_a_name_of_its_own_keeps_it() -> Result<(), Box<dyn Error>> {
        let found = workspaces_of(serde_json::json!([{"id": -98, "name": "special:magic"}]))?;

        assert_eq!(
            found.first().map(|one| one.named.as_str()),
            Some("special:magic"),
            "{found:?}"
        );

        Ok(())
    }

    #[test]
    fn a_workspace_nothing_numbered_is_not_a_workspace() -> Result<(), Box<dyn Error>> {
        let found = workspaces_of(serde_json::json!([{"name": "3"}, {"id": 4, "name": "4"}]))?;

        assert_eq!(found.len(), 1, "{found:?}");

        Ok(())
    }

    #[test]
    fn the_workspace_in_front_is_read_the_same_way_one_in_the_list_is() {
        let value = serde_json::json!({"id": 2, "name": "2", "windows": 1});
        let front = parse_workspace(&value);

        assert_eq!(front, Ok(Some(Workspace { id: 2, named: "2".to_string(), windows: Some(1) })));
    }

    #[test]
    fn a_compositor_that_answered_nothing_says_so_rather_than_naming_a_workspace() {
        let none = said_of(Workspaces, serde_json::json!({}));
        let no_one = parse_workspace(&serde_json::json!({}));

        assert!(matches!(none, Err(HyprctlError::MissingField(Query::Workspaces, "a list"))));
        assert_eq!(no_one, Ok(None));
    }

    type Failure = Box<dyn std::error::Error>;

    type Received = std::thread::JoinHandle<Result<String, std::io::Error>>;

    fn fake_socket(answer: &'static str) -> Result<(std::path::PathBuf, Received), Failure> {
        use std::io::{BufRead, Write};

        let folder = console_core_temporary_directories::fresh(&format!("compositor-{}", answer.len()))?;
        let at = folder.join("compositor.sock");
        let listener = std::os::unix::net::UnixListener::bind(&at)?;
        let heard = std::thread::spawn(move || {
            let (mut asking, _) = listener.accept()?;
            let asked = {
                let mut reading = std::io::BufReader::new(&asking);
                let waiting = reading.fill_buf()?;

                String::from_utf8_lossy(waiting).to_string()
            };

            asking.write_all(answer.as_bytes())?;

            Ok(asked)
        });

        Ok((at, heard))
    }

    fn what_was_asked(heard: Received) -> Result<String, Failure> {
        let asked = heard.join().map_err(|_| "the listener stopped before it heard anything")?;
        let asked = asked?;

        Ok(asked)
    }

    #[test]
    fn a_question_is_asked_on_the_socket_the_way_hyprctl_writes_it() -> Result<(), Failure> {
        let (at, heard) = fake_socket(r#"{"id": 2, "name": "2", "windows": 1}"#)?;
        let answer = ask_at(&at, ActiveWorkspace);
        let asked = what_was_asked(heard)?;
        let _ = std::fs::remove_file(&at);

        assert_eq!(asked, "j/activeworkspace");
        assert!(
            matches!(answer, Ok(Some(Workspace { id: 2, .. }))),
            "{answer:?}"
        );

        Ok(())
    }

    #[test]
    fn every_question_has_the_line_hyprctl_would_have_sent() {
        assert_eq!(Query::Layers.line(), Ok("j/layers".to_string()));
        assert_eq!(Query::EveryMonitor.line(), Ok("j/monitors all".to_string()));
    }

    #[test]
    fn a_dispatch_is_told_on_the_socket_and_a_refusal_is_a_failure() -> Result<(), Failure> {
        let Ok(line) = Request::Dispatch.line(CLOSE_WINDOW);
        let (at, heard) = fake_socket("ok")?;
        let Ok(taken) = dispatch_at(&at, &line);
        let asked = what_was_asked(heard)?;
        let _ = std::fs::remove_file(&at);

        assert_eq!(asked, "/dispatch hl.dsp.window.close()");
        assert_eq!(taken, DispatchResult::Success);

        let (at, heard) = fake_socket("error: keyword can't work with non-legacy parsers")?;
        let Ok(refused) = dispatch_at(&at, "/keyword general:gaps_in 0");
        let _ = heard.join();
        let _ = std::fs::remove_file(&at);

        assert!(matches!(refused, DispatchResult::Failure(_)), "{refused:?}");

        Ok(())
    }

    #[test]
    fn nobody_listening_is_a_failure_rather_than_an_answer() -> Result<(), Failure> {
        let folder = console_core_temporary_directories::fresh("compositor-nobody")?;
        let at = folder.join("compositor-nobody.sock");
        let asked = ask_at(&at, Layers);

        assert!(matches!(asked, Err(HyprctlError::Socket(Query::Layers, socket::SocketError::Unreachable(..)))), "{asked:?}");

        Ok(())
    }
}
