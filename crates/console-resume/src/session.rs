//! What was open, written down, and put back.
//!
//! Two files per session, because they answer two different questions. `exec.conf`
//! is the list of commands that start the programs again, one line each, with the
//! rules the window wants in front of it. `clients.json` is what each window was
//! like once it was there -- which workspace, where, how big, floating or pinned
//! or filling the screen -- and it is read as windows appear, because a program
//! that has just been started has no window yet to move.
//!
//! ## A desktop with nothing on it is not a session
//!
//! Logging out closes every window a second or two before the compositor stops,
//! and a saver watching that sees an empty desktop and writes it down. What was
//! on the screen all day is then gone, and nothing looks wrong: the file is
//! there, it is valid, it says nothing was open. So a save of nothing is refused
//! and the session already on disk stands. Throwing one away is `delete`, which
//! is a thing somebody asks for.
//!
//! A window closing is the same fault a minute earlier. It is written down, but
//! only after [`LOSS_SETTLES_AFTER`], because the first thing a log-out looks
//! like is one window closing.
//!
//! ## Windows are matched by what they were called when they opened
//!
//! A title is what a program is showing right now. A terminal rewrites its own
//! the moment a prompt appears, and by the time it is on the screen it resembles
//! nothing on disk. What a window was called when it first mapped does not move,
//! so that is what is compared -- and because two terminals from the same
//! program are then identical in every field that survives a restart, a saved
//! window is claimed by the first real one that matches it and cannot be claimed
//! twice. Without that, the second terminal to open is adjusted onto the first
//! one's workspace and both land in the same place.
//!
//! ## Closing a window is asked of the compositor
//!
//! The fork this came from cleared the screen by sending each window's process a
//! signal. That is one program deciding to end another one on a desktop where the
//! compositor owns windows, and it takes a whole program down when one of its
//! windows was on the screen. `hl.dsp.window.close` is the same request made to
//! the thing that can refuse it, and it is what the close button asks for.
//!
//! ## A special workspace is known by its name and not by its number
//!
//! Hyprland numbers them from -99 downwards, one further down for every one
//! that exists, so the second special workspace anybody makes is -98 and a
//! program that knows only -99 writes that number down as though it were a
//! workspace somebody could ask for. `workspace -98 silent` names nothing, and
//! the window comes back wherever a rule the compositor cannot read leaves it.
//! What survives is the name -- `special:sky` is what it was called and what
//! asks for it back -- so the number decides nothing here. The two places that
//! ask spell the answer differently: a rule prefix takes the name as a word and
//! a dispatcher takes it quoted, which is why they are one question and two
//! sentences.
//!
//! ## Where a window goes back is the layout's, unless it floats
//!
//! A tiled window has no place of its own. The layout puts it where the windows
//! around it leave room, and pixels asked for over that are refused or held
//! until the next window disagrees with them. What decides where a tiled window
//! lands is the order the programs are started in, and that order is what
//! `exec.conf` is. So a corner and a size are asked only of a window that
//! floats -- and of a floating window both are asked, because a window that
//! comes back the wrong size is the same fault as one that comes back in the
//! wrong place, and the rule prefix meant to have settled it is applied as the
//! window maps rather than after the program has decided what it wanted.
//!
//! ## A program with no window is not in a session
//!
//! What is written down is what the compositor can be asked about, and a
//! program that sits in the tray or waits on a socket has no window to be asked
//! about. That is not a gap here: what this device runs without a window is
//! what `desktop.conf` starts under `[services]`, and it is running again
//! before anybody has logged in. A session is the windows.
//!
//! ## `Saved` spells booleans because JSON does
//!
//! Everywhere else in this desktop what a window is doing is a name --
//! [`console_compositor::Floating`] and its siblings. `Saved` is not that; it is
//! the shape of a file, and the file is JSON, whose vocabulary for these is
//! `true` and `false`. So the names are turned into the file's words on the way
//! out and read back into names on the way in, and no caller sees a bare boolean.

use std::collections::HashSet;
use std::fs::{File, create_dir_all, remove_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use console_compositor::stirred::Stirred;
use console_compositor::{Asked, Done, Filling, Floating, Pinned, Window};
use console_core_atomic_writes::Held;
use console_core_never::Never;
use console_events::listening::Heard;
use console_program_contract::Topic;
use console_waiting::{Patience, Seen, Waited};

use crate::starting::what_starts_it;

const EXEC_NAME: &str = "exec.conf";
const CLIENTS_NAME: &str = "clients.json";

const TICK: Duration = Duration::from_secs(1);

const LOSS_SETTLES_AFTER: Duration = Duration::from_secs(15);

const CLOSING_SETTLES: Duration = Duration::from_secs(10);

const SPECIAL: &str = "special";

const SPECIAL_NAMED: &str = "special:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Really {
    Truly,
    Simulated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restoring {
    MovingWhatIsOpen,
    StartingItAgain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duplicates {
    OnePerProgram,
    OnePerWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PutBack {
    Windows,
    NothingSaved(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Worth {
    Saving,
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Differs {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Workspace {
    Special,
    Numbered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placing {
    ByPixels,
    ByTheLayout,
}

pub struct Sessions {
    pub at: PathBuf,
    pub adjusting_for: Duration,
    pub really: Really,
    pub restoring: Restoring,
    pub duplicates: Duplicates,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Saved {
    pub address: String,
    pub title: String,
    pub first_class: String,
    pub first_title: String,
    pub workspace: i64,
    pub workspace_named: String,
    pub monitor: Option<i64>,
    pub floating: bool,
    pub pinned: bool,
    pub fullscreen: i64,
    pub at: (i64, i64),
    pub size: (i64, i64),
    pub pid: i64,
}

fn open_windows() -> Result<Vec<Window>, String> {
    let said = console_compositor::asked(Asked::Clients)?;

    let Ok(open) = console_compositor::windows_open(&said);

    Ok(open)
}

fn kept(window: &Window) -> Result<Saved, Never> {
    Ok(Saved {
        address: window.address.clone(),
        title: window.title.clone(),
        first_class: window.first_class.clone(),
        first_title: window.first_title.clone(),
        workspace: window.workspace,
        workspace_named: window.workspace_named.clone(),
        monitor: window.monitor,
        floating: window.floating == Floating::Yes,
        pinned: window.pinned == Pinned::Yes,
        fullscreen: match window.filling {
            Filling::Nothing => 0,
            Filling::Maximized => 1,
            Filling::Screen => 2,
        },
        at: window.at,
        size: window.size,
        pid: window.pid,
    })
}

fn as_window(saved: &Saved) -> Result<Window, Never> {
    Ok(Window {
        address: saved.address.clone(),
        title: saved.title.clone(),
        first_class: saved.first_class.clone(),
        first_title: saved.first_title.clone(),
        workspace: saved.workspace,
        workspace_named: saved.workspace_named.clone(),
        monitor: saved.monitor,
        floating: match saved.floating {
            true => Floating::Yes,
            false => Floating::No,
        },
        pinned: match saved.pinned {
            true => Pinned::Yes,
            false => Pinned::No,
        },
        filling: match saved.fullscreen {
            1 => Filling::Maximized,
            2 => Filling::Screen,
            _filling_nothing => Filling::Nothing,
        },
        at: saved.at,
        size: saved.size,
        pid: saved.pid,
    })
}

#[derive(Default)]
struct Changes {
    gained: bool,
    lost: Option<Instant>,
}

impl Changes {
    fn gain(&mut self) -> Result<(), Never> {
        self.gained = true;

        Ok(())
    }

    fn lose(&mut self) -> Result<(), Never> {
        self.lost = Some(Instant::now());

        Ok(())
    }

    fn worth_saving(&self, saved: Instant, interval: Duration) -> Result<Worth, Never> {
        let settled = self.lost.is_some_and(|at| at.elapsed() >= LOSS_SETTLES_AFTER);

        Ok(match self.gained || settled || saved.elapsed() >= interval {
            true => Worth::Saving,
            false => Worth::Waiting,
        })
    }
}

type Noted = Arc<Mutex<Changes>>;

fn held(changes: &Noted) -> Result<MutexGuard<'_, Changes>, Never> {
    Ok(match changes.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    })
}

fn note(changes: &Noted, stirred: &Stirred) -> Result<(), Never> {
    let change = match stirred {
        Stirred::WindowOpened(_)
        | Stirred::WindowRenamed(_)
        | Stirred::WindowMoved
        | Stirred::WindowFloated
        | Stirred::WindowPinned
        | Stirred::WindowFilled => Changes::gain,
        Stirred::WindowClosed(_) => Changes::lose,
        Stirred::LayerOpened
        | Stirred::LayerClosed
        | Stirred::WorkspaceChanged
        | Stirred::ScreenFocused
        | Stirred::ConfigReloaded
        | Stirred::Nothing => return Ok(()),
    };

    let Ok(mut held) = held(changes);

    change(&mut held)
}

fn under(at: &Path, name: &str) -> Result<PathBuf, Never> {
    Ok(match name.is_empty() {
        true => at.to_path_buf(),
        false => at.join(name),
    })
}

fn which_workspace(window: &Window) -> Result<Workspace, Never> {
    let named = &window.workspace_named;

    Ok(match named == SPECIAL || named.starts_with(SPECIAL_NAMED) {
        true => Workspace::Special,
        false => Workspace::Numbered,
    })
}

fn workspace_selector(saved: &Window) -> Result<String, Never> {
    let Ok(which) = which_workspace(saved);

    match which {
        Workspace::Special => crate::lua::quote(&saved.workspace_named),
        Workspace::Numbered => crate::lua::quote(&saved.workspace.to_string()),
    }
}

fn placing(saved: &Window) -> Result<Placing, Never> {
    Ok(match (saved.floating, saved.filling) {
        (Floating::Yes, Filling::Nothing) => Placing::ByPixels,
        (Floating::Yes, Filling::Maximized | Filling::Screen)
        | (Floating::No, Filling::Nothing | Filling::Maximized | Filling::Screen) => {
            Placing::ByTheLayout
        },
    })
}

fn is_same_client(saved: &Window, real: &Window) -> Result<Differs, Never> {
    let alike = saved.first_class == real.first_class && saved.first_title == real.first_title;

    Ok(match alike {
        true => Differs::No,
        false => Differs::Yes,
    })
}

fn differs<T: PartialEq>(
    real: &Window,
    saved: &Window,
    of: fn(&Window) -> T,
) -> Result<Differs, Never> {
    Ok(match of(real) == of(saved) {
        true => Differs::No,
        false => Differs::Yes,
    })
}

fn asked_of_the_compositor(lua: &str, about: &str, window: &str) -> Result<(), Never> {
    let Ok(done) = crate::lua::dispatch(lua);

    match done {
        Done::Taken => {},
        Done::Refused(why) => eprintln!("{window}: {about}: {why}"),
    }

    Ok(())
}

fn put<T: PartialEq>(
    real: &Window,
    saved: &Window,
    of: fn(&Window) -> T,
    really: Really,
    lua: &str,
    about: &str,
) -> Result<(), Never> {
    let Ok(differs) = differs(real, saved, of);

    match differs {
        Differs::No => return Ok(()),
        Differs::Yes => {},
    }

    println!("{}: {about}", real.title);

    match really {
        Really::Simulated => return Ok(()),
        Really::Truly => {},
    }

    asked_of_the_compositor(lua, about, &real.title)
}

fn fill(real: &Window, saved: &Window, really: Really, named: &str) -> Result<(), Never> {
    let Ok(differs) = differs(real, saved, |window| window.filling);

    match differs {
        Differs::No => return Ok(()),
        Differs::Yes => {},
    }

    let mode = match saved.filling {
        Filling::Maximized => "maximized",
        Filling::Screen | Filling::Nothing => "fullscreen",
    };

    println!("{}: filling the screen", real.title);

    match really {
        Really::Simulated => return Ok(()),
        Really::Truly => {},
    }

    let Ok(()) = asked_of_the_compositor(
        &format!("hl.dsp.focus({{ window = {named} }})"),
        "taking the focus first",
        &real.title,
    );

    asked_of_the_compositor(
        &format!(
            "hl.dsp.window.fullscreen({{ window = {named}, mode = \"{mode}\", action = \"toggle\" }})"
        ),
        "filling the screen",
        &real.title,
    )
}

fn adjust(real: &Window, saved: &Window, really: Really) -> Result<(), Never> {
    let Ok(named) = crate::lua::window(&real.address);
    let Ok(workspace) = workspace_selector(saved);

    let Ok(()) = put(
        real,
        saved,
        |window| window.workspace,
        really,
        &format!(
            "hl.dsp.window.move({{ window = {named}, workspace = {workspace}, follow = false }})"
        ),
        "onto the workspace it was on",
    );

    let Ok(screen) = crate::lua::quote(&saved.monitor.unwrap_or_default().to_string());

    let Ok(()) = put(
        real,
        saved,
        |window| window.monitor.unwrap_or_default(),
        really,
        &format!("hl.dsp.workspace.move({{ workspace = {workspace}, monitor = {screen} }})"),
        "onto the screen it was on",
    );

    let Ok(()) = put(
        real,
        saved,
        |window| window.floating,
        really,
        &format!("hl.dsp.window.float({{ window = {named}, action = \"toggle\" }})"),
        "floating the way it was",
    );

    let Ok(()) = put(
        real,
        saved,
        |window| window.pinned,
        really,
        &format!("hl.dsp.window.pin({{ window = {named}, action = \"toggle\" }})"),
        "pinned the way it was",
    );

    let Ok(()) = fill(real, saved, really, &named);

    let Ok(placing) = placing(saved);

    match placing {
        Placing::ByTheLayout => return Ok(()),
        Placing::ByPixels => {},
    }

    let Ok(()) = put(
        real,
        saved,
        |window| window.size,
        really,
        &format!(
            "hl.dsp.window.resize({{ window = {named}, x = {}, y = {}, relative = false }})",
            saved.size.0, saved.size.1
        ),
        "the size it was",
    );

    put(
        real,
        saved,
        |window| window.at,
        really,
        &format!(
            "hl.dsp.window.move({{ window = {named}, x = {}, y = {}, relative = false }})",
            saved.at.0, saved.at.1
        ),
        "back where it was",
    )
}

fn window_opened(
    address: &str,
    saved: &[Window],
    claimed: &Mutex<HashSet<usize>>,
    really: Really,
) -> Result<(), Never> {
    let open = match open_windows() {
        Ok(open) => open,
        Err(_the_compositor_said_nothing) => return Ok(()),
    };

    let real = match open.into_iter().find(|window| window.address == address) {
        Some(real) => real,
        None => return Ok(()),
    };

    let mut claimed = match claimed.lock() {
        Ok(claimed) => claimed,
        Err(poisoned) => poisoned.into_inner(),
    };

    let found = saved.iter().enumerate().find(|(index, one)| {
        let Ok(differs) = is_same_client(one, &real);

        !claimed.contains(index) && differs == Differs::No
    });

    let (index, one) = match found {
        Some(found) => found,
        None => return Ok(()),
    };

    claimed.insert(index);
    drop(claimed);

    adjust(&real, one, really)
}

fn saved_windows(at: &Path) -> Result<Vec<Window>, String> {
    let at = at.join(CLIENTS_NAME);

    let Ok(held) = console_core_atomic_writes::read(&at);

    let said = match held {
        Held::Said(said) => said,
        Held::Nothing => return Ok(Vec::new()),
        Held::Unreadable(fault) => return Err(format!("{}: reading it: {fault}", at.display())),
    };

    let saved: Vec<Saved> = serde_json::from_str(&said)
        .map_err(|fault| format!("{}: reading it: {fault}", at.display()))?;

    Ok(saved
        .iter()
        .map(|one| {
            let Ok(window) = as_window(one);

            window
        })
        .collect())
}

fn what_to_start(at: &Path) -> Result<Vec<String>, String> {
    let at = at.join(EXEC_NAME);

    let Ok(held) = console_core_atomic_writes::read(&at);

    let said = match held {
        Held::Said(said) => said,
        Held::Nothing => return Ok(Vec::new()),
        Held::Unreadable(fault) => return Err(format!("{}: reading it: {fault}", at.display())),
    };

    Ok(said.lines().filter(|line| !line.trim().is_empty()).map(str::to_string).collect())
}

fn start_programs(lines: &[String], really: Really) -> Result<(), Never> {
    for line in lines {
        let Ok((rules, command)) = crate::lua::split_exec_line(line);

        println!("starting {command}");

        match really {
            Really::Simulated => {},
            Really::Truly => {
                let Ok(_started) = crate::lua::exec(command, &rules);
            },
        }
    }

    Ok(())
}

impl Sessions {
    pub fn save(&self, name: &str) -> Result<(), String> {
        let Ok(at) = under(&self.at, name);

        let open = open_windows()?;

        match open.first() {
            Some(_something_is_open) => {},
            None => {
                println!("nothing is open, so the session already saved stands");

                return Ok(());
            },
        }

        create_dir_all(&at).map_err(|fault| format!("{}: making it: {fault}", at.display()))?;

        let mut lines = File::create(at.join(EXEC_NAME))
            .map_err(|fault| format!("{}: making it: {fault}", at.display()))?;

        let mut pids: Vec<i64> = Vec::new();
        let mut written: Vec<Saved> = Vec::new();

        let Ok(known) = crate::starting::from_desktop_files();

        for window in open.iter().rev() {
            let Ok(keeping) = kept(window);

            written.push(keeping);

            let again = match self.duplicates {
                Duplicates::OnePerWindow => false,
                Duplicates::OnePerProgram => pids.contains(&window.pid),
            };

            match again {
                true => continue,
                false => {},
            }

            let command = match what_starts_it(window, &known) {
                Ok(command) => command,
                Err(_nothing_says_what_started_it) => continue,
            };

            pids.push(window.pid);

            let Ok(rules) = self.rules_for(window);

            writeln!(lines, "[{rules}] {command}")
                .map_err(|fault| format!("{}: writing it: {fault}", at.display()))?;
        }

        let json = File::create(at.join(CLIENTS_NAME))
            .map_err(|fault| format!("{}: making it: {fault}", at.display()))?;

        serde_json::to_writer(&json, &written)
            .map_err(|fault| format!("{}: writing it: {fault}", at.display()))?;

        Ok(())
    }

    fn rules_for(&self, window: &Window) -> Result<String, Never> {
        let Ok(which) = which_workspace(window);

        let workspace = match which {
            Workspace::Special => format!("workspace {} silent", window.workspace_named),
            Workspace::Numbered => format!("workspace {} silent", window.workspace),
        };

        let floating = match window.floating {
            Floating::Yes => Some("float".to_string()),
            Floating::No => None,
        };

        let pinned = match window.pinned {
            Pinned::Yes => Some("pin".to_string()),
            Pinned::No => None,
        };

        let filling = match window.filling {
            Filling::Nothing => 0,
            Filling::Maximized => 1,
            Filling::Screen => 2,
        };

        Ok([
            Some(format!("monitor {}", window.monitor.unwrap_or_default())),
            Some(workspace),
            floating,
            Some(format!("move {} {}", window.at.0, window.at.1)),
            Some(format!("size {} {}", window.size.0, window.size.1)),
            pinned,
            Some(format!("fullscreenstate {filling}")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<String>>()
        .join(";"))
    }

    pub fn clear(&self) -> Result<(), String> {
        match self.really {
            Really::Simulated => return Ok(()),
            Really::Truly => {},
        }

        let open = open_windows()?;

        for window in open {
            let Ok(named) = crate::lua::window(&window.address);

            let Ok(()) = asked_of_the_compositor(
                &format!("hl.dsp.window.close({{ window = {named} }})"),
                "closing it",
                &window.title,
            );
        }

        let Ok(patience) = Patience::asking_every(CLOSING_SETTLES, TICK);

        let Ok(waited) = console_waiting::until(patience, || {
            Ok(match open_windows() {
                Ok(open) => match open.first() {
                    Some(_still_open) => Seen::NotYet,
                    None => Seen::Yes,
                },
                Err(_the_compositor_said_nothing) => Seen::Yes,
            })
        });

        match waited {
            Waited::Happened => Ok(()),
            Waited::RanOut => {
                Err("something would not close, so what was saved is not put back".to_string())
            },
        }
    }

    pub fn load(&self, name: &str) -> Result<PutBack, String> {
        let Ok(at) = under(&self.at, name);

        let saved = saved_windows(&at)?;
        let starting = what_to_start(&at)?;

        match self.restoring {
            Restoring::MovingWhatIsOpen => {},
            Restoring::StartingItAgain => match starting.first() {
                None => return Ok(PutBack::NothingSaved(at)),
                Some(_there_is_something_to_put_back) => {
                    self.clear()?;

                    let Ok(()) = start_programs(&starting, self.really);
                },
            },
        }

        let began = Instant::now();
        let adjusting_for = self.adjusting_for;
        let really = self.really;

        std::thread::spawn(move || {
            let claimed = Mutex::new(HashSet::new());

            let Ok(listening) = console_events::listening::listen(&[Topic::Compositor]);
            let Ok(heard) = listening.heard();

            for said in heard {
                match began.elapsed() >= adjusting_for {
                    true => break,
                    false => {},
                }

                let line = match said {
                    Heard::GotIn => continue,
                    Heard::Said(changed) => changed.said,
                };

                let Ok(stirred) = console_compositor::stirred::read(&line);

                let address = match stirred {
                    Stirred::WindowOpened(address) | Stirred::WindowRenamed(address) => address,
                    Stirred::WindowClosed(_)
                    | Stirred::WindowMoved
                    | Stirred::WindowFloated
                    | Stirred::WindowPinned
                    | Stirred::WindowFilled
                    | Stirred::LayerOpened
                    | Stirred::LayerClosed
                    | Stirred::WorkspaceChanged
                    | Stirred::ScreenFocused
                    | Stirred::ConfigReloaded
                    | Stirred::Nothing => continue,
                };

                let Ok(()) = window_opened(&address, &saved, &claimed, really);
            }
        });

        let Ok(patience) = Patience::asking_every(self.adjusting_for, TICK);

        let Ok(_it_is_over_when_the_time_is) =
            console_waiting::until(patience, || Ok(Seen::NotYet));

        Ok(PutBack::Windows)
    }

    pub fn watch(&self, name: &str, interval: Duration) -> Result<(), String> {
        let changes: Noted = Arc::new(Mutex::new(Changes::default()));
        let noticing = Arc::clone(&changes);

        std::thread::spawn(move || {
            let Ok(listening) = console_events::listening::listen(&[Topic::Compositor]);
            let Ok(heard) = listening.heard();

            for said in heard {
                let line = match said {
                    Heard::GotIn => continue,
                    Heard::Said(changed) => changed.said,
                };

                let Ok(stirred) = console_compositor::stirred::read(&line);
                let Ok(()) = note(&noticing, &stirred);
            }
        });

        let mut saved = Instant::now();

        let Ok(patience) = Patience::asking_every(interval, TICK);

        loop {
            let Ok(_either_way_it_is_time) = console_waiting::until(patience, || {
                let Ok(now) = held(&changes);
                let Ok(worth) = now.worth_saving(saved, interval);

                Ok(match worth {
                    Worth::Saving => Seen::Yes,
                    Worth::Waiting => Seen::NotYet,
                })
            });

            let Ok(mut now) = held(&changes);

            *now = Changes::default();

            drop(now);

            self.save(name)?;

            saved = Instant::now();
        }
    }

    pub fn list(&self) -> Result<Vec<String>, Never> {
        let read = match std::fs::read_dir(&self.at) {
            Ok(read) => read,
            Err(_there_are_none) => return Ok(Vec::new()),
        };

        Ok(read
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect())
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        let Ok(at) = under(&self.at, name);

        remove_dir_all(&at).map_err(|fault| format!("{}: removing it: {fault}", at.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(class: &str, title: &str) -> Window {
        Window {
            address: "0x1".to_string(),
            title: title.to_string(),
            first_class: class.to_string(),
            first_title: title.to_string(),
            workspace: 2,
            workspace_named: "2".to_string(),
            monitor: Some(0),
            floating: Floating::No,
            pinned: Pinned::No,
            filling: Filling::Nothing,
            at: (2, 2),
            size: (800, 600),
            pid: 42,
        }
    }

    fn worth(changes: &Noted) -> Result<Worth, Never> {
        let Ok(now) = held(changes);

        now.worth_saving(Instant::now(), Duration::from_secs(600))
    }

    fn on_special(named: &str, number: i64) -> Window {
        let mut special = window("imv", "pictures");
        special.workspace = number;
        special.workspace_named = named.to_string();

        special
    }

    fn sessions(at: &str) -> Sessions {
        Sessions {
            at: PathBuf::from(at),
            adjusting_for: Duration::from_secs(1),
            really: Really::Simulated,
            restoring: Restoring::StartingItAgain,
            duplicates: Duplicates::OnePerProgram,
        }
    }

    #[test]
    fn what_a_window_was_doing_survives_the_trip_through_the_file() {
        let mut was = window("foot", "a shell");
        was.floating = Floating::Yes;
        was.pinned = Pinned::Yes;
        was.filling = Filling::Screen;

        let Ok(saved) = kept(&was);
        let Ok(again) = as_window(&saved);

        assert_eq!(again, was);
    }

    #[test]
    fn a_special_workspace_goes_by_the_name_no_number_would_carry() {
        assert_eq!(
            workspace_selector(&on_special("special:sky", -99)),
            Ok(r#""special:sky""#.to_string())
        );
        assert_eq!(workspace_selector(&window("foot", "a shell")), Ok(r#""2""#.to_string()));
    }

    #[test]
    fn the_second_special_workspace_is_not_the_first_one() {
        let second = on_special("special:magic", -98);

        let Ok(rules) = sessions("/nowhere").rules_for(&second);

        assert_eq!(workspace_selector(&second), Ok(r#""special:magic""#.to_string()));
        assert!(rules.contains("workspace special:magic silent"), "{rules}");
        assert!(
            !rules.contains("-98"),
            "a number below -99 is a workspace nobody can ask for: {rules}"
        );
    }

    #[test]
    fn a_workspace_of_somebodys_own_called_specials_is_not_a_special_one() {
        let mut named = window("foot", "a shell");
        named.workspace = 4;
        named.workspace_named = "specials".to_string();

        assert_eq!(which_workspace(&named), Ok(Workspace::Numbered));
        assert_eq!(workspace_selector(&named), Ok(r#""4""#.to_string()));
    }

    #[test]
    fn a_window_is_the_same_one_by_what_it_was_called_rather_than_what_it_shows() {
        let saved = window("foot", "foot");
        let mut now = window("foot", "foot");
        now.title = "half a build".to_string();

        assert_eq!(is_same_client(&saved, &now), Ok(Differs::No));
        assert_eq!(is_same_client(&saved, &window("librewolf", "foot")), Ok(Differs::Yes));
    }

    #[test]
    fn a_special_workspace_is_asked_for_by_the_word_the_rules_take() {
        let Ok(rules) = sessions("/nowhere").rules_for(&on_special("special:sky", -99));

        assert!(rules.contains("workspace special:sky silent"), "{rules}");
    }

    #[test]
    fn the_special_workspace_nobody_named_is_still_a_special_one() {
        let Ok(rules) = sessions("/nowhere").rules_for(&on_special("special", -99));

        assert_eq!(which_workspace(&on_special("special", -99)), Ok(Workspace::Special));
        assert!(rules.contains("workspace special silent"), "{rules}");
    }

    #[test]
    fn a_tiled_window_is_left_where_the_layout_puts_it() {
        assert_eq!(placing(&window("foot", "a shell")), Ok(Placing::ByTheLayout));
    }

    #[test]
    fn a_floating_window_is_put_back_by_the_corner_and_the_size_it_had() {
        let mut floats = window("imv", "pictures");
        floats.floating = Floating::Yes;

        assert_eq!(placing(&floats), Ok(Placing::ByPixels));
    }

    #[test]
    fn a_window_filling_the_screen_is_not_shifted_about_underneath_it() {
        let mut filling = window("imv", "pictures");
        filling.floating = Floating::Yes;
        filling.filling = Filling::Screen;

        assert_eq!(placing(&filling), Ok(Placing::ByTheLayout));
    }

    #[test]
    fn a_rule_a_window_does_not_want_is_left_out_rather_than_left_empty() {
        let Ok(plain) = sessions("/nowhere").rules_for(&window("foot", "a shell"));

        assert!(!plain.contains("float"), "{plain}");
        assert!(!plain.contains(";;"), "an empty rule is a rule nobody wrote: {plain}");
    }

    #[test]
    fn a_window_that_floats_and_is_pinned_says_both() {
        let mut both = window("imv", "pictures");
        both.floating = Floating::Yes;
        both.pinned = Pinned::Yes;

        let Ok(rules) = sessions("/nowhere").rules_for(&both);

        assert!(rules.contains(";float;"), "{rules}");
        assert!(rules.contains(";pin;"), "{rules}");
    }

    #[test]
    fn a_session_with_no_name_is_the_directory_the_sessions_live_in() {
        let at = PathBuf::from("/home/somebody/.local/share/console/resume");

        assert_eq!(under(&at, ""), Ok(at.clone()));
        assert_eq!(under(&at, "yesterday"), Ok(at.join("yesterday")));
    }

    #[test]
    fn a_change_to_a_window_is_worth_saving_and_the_screen_moving_is_not() {
        let changes: Noted = Arc::new(Mutex::new(Changes::default()));

        let Ok(()) = note(&changes, &Stirred::WorkspaceChanged);

        assert_eq!(
            worth(&changes),
            Ok(Worth::Waiting)
        );

        let Ok(()) = note(&changes, &Stirred::WindowMoved);

        assert_eq!(
            worth(&changes),
            Ok(Worth::Saving)
        );
    }

    #[test]
    fn a_window_that_has_just_closed_is_not_written_down_yet() {
        let changes: Noted = Arc::new(Mutex::new(Changes::default()));

        let Ok(()) = note(&changes, &Stirred::WindowClosed("0x1".to_string()));

        assert_eq!(
            worth(&changes),
            Ok(Worth::Waiting),
            "the first thing logging out looks like is one window closing"
        );
    }

    #[test]
    fn listing_where_no_session_has_ever_been_saved_is_no_sessions_rather_than_a_fault() {
        assert_eq!(sessions("/nowhere/at/all").list(), Ok(Vec::new()));
    }
}
