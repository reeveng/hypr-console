//! The settings, drawn.
//!
//! What is here is the reading of the machine. What each tab holds once it has
//! been read is `crate::rows`, where it can be asked without a
//! machine to ask.
//!
//! Anything that takes a moment, connecting above all, is done off to one side
//! so the panel keeps answering the buttons while it happens.


use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::sync::Arc;

pub const WHO: &str = "settings-panel";

pub fn door(argv: &[String]) -> Result<Door, Never> {
    let tab = argv.first().map(String::as_str).unwrap_or_default();

    Door::closing(&format!("settings {tab}"))
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
    let tab = argv.first().cloned();
    let opening = Settings::opening(&Argv::default());
    let Ok(looking) = actor::supervise(move || Looking(opening.state));
    let held = looking.addr.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }));
    let Ok(card) = card.opening_at(tab.as_deref());

    card.shutting(Box::new(move || looking.shutdown()))
}

use console_default_applications::battery;
use console_panel::actor::{self, Addr, Answer};
use console_panel::page::{Does, Level, Page, Rows, Showing};
use console_panel::running::{said, say};
use console_panel::before;
use console_panel::card::{Card, Door};
use crate::defaults::{self, Application};
use crate::level::stepped;
use crate::rows::{
    Chosen, battery_rows, bluetooth_rows, dictation_rows, dictation_says, engine_says,
    notifications_rows, screen_rows, search_rows, sound_rows, system_rows, tabs, wifi_rows,
};
use crate::wallpaper::{Found, Offered, wallpaper_rows};
use crate::warm::{self, Warmth};
use crate::{bluetooth, screen, size, sound, wifi};
use console_home_screen::shape::{self, Shape};
use console_program_contract::{Argv, Doing, Program as _, Turn, Word};
use crate::choosing::{Closes, Heard, Its, Onto, Settings, closes};
use console_wallpaper::choose::{Set, Wanted};
use console_wallpaper::place;

fn pactl(argv: &[&str]) -> Result<String, Never> {
    said(Program::Pactl, argv)
}

fn of_kind(kind: &str) -> Result<Vec<sound::Thing>, Never> {
    let Ok(said) = pactl(&["-f", "json", "list", &format!("{kind}s")]);

    sound::read(&said)
}

fn words(argv: &[&str]) -> Result<Vec<String>, Never> {
    Ok(argv.iter().map(|word| (*word).to_string()).collect())
}

fn hush(index: i64, kind: &'static str) -> Result<Does, Never> {
    Does::and_stay(move |_| {
        let Ok(_) = pactl(&[&format!("set-{kind}-mute"), &index.to_string(), "toggle"]);
    })
}

fn turn_to(index: i64, kind: &'static str) -> Result<Level, Never> {
    Ok(Arc::new(move |step| {
        let Ok(of_kind) = of_kind(kind);

        let Ok(one) = sound::one(&of_kind, index);

        let Some(thing) = one.cloned() else {
            return;
        };

        let Ok(level) = thing.level();
        let Ok(going) = stepped(level, step);

        let Ok(_) = pactl(&[
            &format!("set-{kind}-volume"),
            &index.to_string(),
            &format!("{going}%"),
        ]);
    }))
}

const SINKS: &str = "sinks";
const SPEAKERS: &str = "default sink";

fn asked(note: &str, program: Program, argv: &[&str]) -> Result<String, Never> {
    before::said(note, program, argv)
}

fn kept(note: &str) -> Result<String, Never> {
    before::last(note)
}

fn pactl_kept(note: &str, argv: &[&str]) -> Result<String, Never> {
    asked(note, Program::Pactl, argv)
}

fn sound_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(sinks) = pactl_kept(SINKS, &["-f", "json", "list", "sinks"]);
    let Ok(playing) = of_kind("sink-input");
    let Ok(speakers) = pactl_kept(SPEAKERS, &["get-default-sink"]);

    let Ok(sinks) = sound::read(&sinks);

    sound_rows(&sinks, &playing, &speakers, hush, turn_to)
}

fn sound_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(sinks) = kept(SINKS);
    let Ok(speakers) = kept(SPEAKERS);

    let Ok(sinks) = sound::read(&sinks);

    sound_rows(&sinks, &[], &speakers, hush, turn_to)
}

fn brightness() -> Result<i32, Never> {
    let now = screen::now()?;

    let points = match now {
        Some(now) => screen::as_points(now)?,
        None => 0,
    };

    fitted(points)
}

fn dim() -> Result<Level, Never> {
    Ok(Arc::new(|step| {
        let way = match step > 0 {
            true => screen::Way::Up,
            false => screen::Way::Down,
        };

        let Ok(now) = screen::now();

        match now {
            Some(now) => {
                let Ok(going) = screen::stepped(now, way);

                let _ = screen::set(going);
            }
            None => {},
        }
    }))
}

fn guard(step: battery::Step) -> Result<Level, Never> {
    Ok(Arc::new(move |way| {
        let Ok(levels) = battery::Levels::here();
        let Ok(at) = levels.at(step);
        let Ok(going) = stepped(at, way);
        let Ok(_) = levels.set(step, going);
    }))
}

const PROFILE: &str = "power profile";

fn battery_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(profile) = asked(PROFILE, Program::Powerprofilesctl, &["get"]);

    let Ok(levels) = battery::Levels::here();

    battery_rows(Some(&profile), levels, guard)
}

fn warmth() -> Result<Warmth, Never> {
    let Ok(home) = std::env::var("HOME") else { return Ok(Warmth::Following) };

    let Ok(at) = warm::at(&home);

    let Ok(said) = std::fs::read_to_string(at) else {
        return Ok(Warmth::Following);
    };

    Warmth::read(&said)
}

fn battery_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(profile) = kept(PROFILE);

    let Ok(levels) = battery::Levels::here();

    battery_rows(Some(&profile), levels, guard)
}

const SCREENS: &str = "screens";

fn size_tab(held: &Held) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(brightness) = brightness();
    let Ok(warmth) = warmth();
    let Ok(screens) = asked(SCREENS, Program::Hyprctl, &["monitors", "-j"]);
    let Ok(home) = home_rows(held);
    let Ok(standing) = size::standing(&screens);

    let Ok(dim) = dim();

    screen_rows(Some(brightness), dim, warmth, standing, home)
}

fn home_at() -> Result<Option<std::path::PathBuf>, Never> {
    Ok(match std::env::var("HOME") {
        Ok(home) => {
            let Ok(at) = shape::at(std::path::Path::new(&home));

            Some(at)
        }
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => None,
    })
}

fn home_shape() -> Result<Shape, Never> {
    let Ok(Some(at)) = home_at() else { return Ok(Shape::USUAL) };

    Ok(match std::fs::read_to_string(&at) {
        Ok(said) => {
            let Ok(shape) = Shape::read(&said);

            shape
        }
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Shape::USUAL,
        Err(fault) => {
            eprintln!("settings-panel: {}: {fault}", at.display());

            Shape::USUAL
        },
    })
}

fn home_set(shape: Shape) -> Result<(), Never> {
    let Ok(Some(at)) = home_at() else {
        eprintln!("settings-panel: this machine will not say whose home to write the grid in");

        return Ok(());
    };

    match at.parent().map(std::fs::create_dir_all) {
        Some(Err(fault)) => {
            eprintln!("settings-panel: {}: {fault}", at.display());

            return Ok(());
        }
        Some(Ok(())) | None => {},
    }

    let Ok(written) = shape.written();

    match std::fs::write(&at, written) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("settings-panel: {}: {fault}", at.display());

            return Ok(());
        }
    }

    match console_panel::door::telling(console_panel::door::Said::Again) {
        Ok(()) => {},
        Err(fault) => eprintln!("settings-panel: the home screen was not told: {fault}"),
    }

    Ok(())
}

fn home_rows(held: &Held) -> Result<Vec<console_panel::page::Row>, Never> {
    let across = held.clone();
    let down = held.clone();
    let sized = held.clone();
    let Ok(shape) = home_shape();

    crate::rows::home_rows(
        shape,
        Arc::new(move |step| {
            let Ok(shape) = home_shape();
            let Ok(()) = grid(&across, Heard::Across { shape, step });
        }),
        Arc::new(move |step| {
            let Ok(shape) = home_shape();
            let Ok(()) = grid(&down, Heard::Down { shape, step });
        }),
        Arc::new(move |step| {
            let Ok(shape) = home_shape();
            let Ok(()) = grid(&sized, Heard::Sized { shape, step });
        }),
    )
}

fn grid(held: &Held, heard: Heard) -> Result<(), Never> {
    let doings = match held.ask(|answer| Msg::Heard(heard, answer)) {
        Ok(doings) => doings,
        Err(_) => {
            eprintln!("settings-panel: the panel's own state has gone, so the grid did not move");

            Vec::new()
        }
    };

    for shape in doings.iter().filter_map(|doing| {
        let Ok(shape) = shaped(doing);

        shape
    }) {
        let Ok(()) = home_set(*shape);
    }

    Ok(())
}

fn shaped(doing: &Doing<Its>) -> Result<Option<&shape::Shape>, Never> {
    Ok(match doing {
        Doing::Its(Its::Home(shape)) => Some(shape),

        Doing::Its(Its::Replace(_))
        | Doing::Ask(_)
        | Doing::Watch(_)
        | Doing::AskWhoever(_)
        | Doing::Start(_)
        | Doing::Listen(_)
        | Doing::Deafen(_)
        | Doing::Write(_)
        | Doing::Say(_)
        | Doing::Print(_)
        | Doing::Stop(_) => None,
    })
}

fn size_meanwhile(held: &Held) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(brightness) = brightness();
    let Ok(warmth) = warmth();
    let Ok(screens) = kept(SCREENS);
    let Ok(home) = home_rows(held);
    let Ok(standing) = size::standing(&screens);

    let Ok(dim) = dim();

    screen_rows(Some(brightness), dim, warmth, standing, home)
}

fn join(network: wifi::Network, known: wifi::Known) -> Result<Does, Never> {
    Does::and_stay(move |showing| {
        let name = network.name.clone();

        match known {
            wifi::Known::Yes => {
                let Ok(nmcli) = Program::Nmcli.name();
                let Ok(words) = words(&[nmcli, "connection", "up", "id", &name]);

                showing.later(words);

                return;
            }
            wifi::Known::No => {},
        }

        match network.locked {
            true => {},
            false => {
                let Ok(nmcli) = Program::Nmcli.name();
                let Ok(words) = words(&[nmcli, "device", "wifi", "connect", &name]);

                showing.later(words);

                return;
            }
        }

        let asking = name.clone();
        showing.ask(
            &format!("The password for {name}"),
            Arc::new(move |showing, word| {
                let Ok(words) = words(&[
                    "nmcli", "device", "wifi", "connect", &asking, "password", word,
                ]);

                showing.later(words);
            }),
        );
    })
}

const QUIET: &str = "makoctl mode";

fn notifications_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(mode) = asked(QUIET, Program::Makoctl, &["mode"]);

    let Ok(held_back) = console_notifications::reading::held_back(&mode);

    notifications_rows(held_back)
}

fn notifications_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(mode) = kept(QUIET);

    let Ok(held_back) = console_notifications::reading::held_back(&mode);

    notifications_rows(held_back)
}

const WIFI_RADIO: &str = "wifi radio";
const KNOWN: &str = "wifi known";
const IN_RANGE: &str = "wifi in range";

fn wifi_at(
    radio: &str,
    known: &str,
    in_range: &str,
) -> Result<Vec<console_panel::page::Row>, Never> {
    let on = wifi::on(radio)?;
    let networks = wifi::networks(in_range)?;
    let saved = wifi::saved(known)?;

    wifi_rows(on, networks, &saved, join)
}

fn wifi_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(radio) = asked(WIFI_RADIO, Program::Nmcli, &["radio", "wifi"]);
    let Ok(known) =
        asked(KNOWN, Program::Nmcli, &["-t", "-f", "NAME,TYPE", "connection", "show"]);
    let Ok(in_range) = asked(
        IN_RANGE,
        Program::Nmcli,
        &["-t", "-f", "ACTIVE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"],
    );

    wifi_at(&radio, &known, &in_range)
}

fn wifi_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(radio) = kept(WIFI_RADIO);
    let Ok(known) = kept(KNOWN);
    let Ok(in_range) = kept(IN_RANGE);

    wifi_at(&radio, &known, &in_range)
}

fn look_again(showing: &dyn Showing) -> Result<(), Never> {
    let Ok(nmcli) = Program::Nmcli.name();
    let Ok(words) = words(&[nmcli, "device", "wifi", "rescan"]);

    showing.later(words);

    Ok(())
}

const BLUETOOTH_RADIO: &str = "bluetooth radio";
const INTRODUCED: &str = "bluetooth devices";

fn about(address: &str) -> Result<String, Never> {
    Ok(format!("bluetooth {address}"))
}

fn bluetooth_at(
    radio: &str,
    introduced: &str,
    ask: impl Fn(&str) -> String,
) -> Result<Vec<console_panel::page::Row>, Never> {
    let mut devices = Vec::new();

    let known = bluetooth::devices(introduced)?;

    for device in known {
        let joined = bluetooth::joined(&ask(&device.address))?;

        devices.push((device, joined));
    }

    let on = bluetooth::on(radio)?;

    bluetooth_rows(on, devices)
}

fn bluetooth_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(radio) = asked(BLUETOOTH_RADIO, Program::Bluetoothctl, &["show"]);
    let Ok(introduced) = asked(INTRODUCED, Program::Bluetoothctl, &["devices"]);

    bluetooth_at(&radio, &introduced, |address| {
        let Ok(about) = about(address);
        let Ok(said) = asked(&about, Program::Bluetoothctl, &["info", address]);

        said
    })
}

fn bluetooth_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(radio) = kept(BLUETOOTH_RADIO);
    let Ok(introduced) = kept(INTRODUCED);

    bluetooth_at(&radio, &introduced, |address| {
        let Ok(about) = about(address);
        let Ok(said) = kept(&about);

        said
    })
}

const UP: &str = "wallpaper";

fn on_the_screen() -> Result<String, Never> {
    let Ok(said) = asked(UP, Program::Awww, &["query"]);

    place::showing(&said)
}

fn was_on_the_screen() -> Result<String, Never> {
    let Ok(said) = kept(UP);

    place::showing(&said)
}

fn write_down(wanted: &Wanted) -> Result<(), String> {
    let Ok(asked) = place::asked();

    let at = asked.ok_or("This machine will not say whose home to write it in.")?;

    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?,
        None => {},
    }

    let Ok(written) = wanted.written();

    std::fs::write(&at, written)
        .map_err(|fault| format!("{} could not be written: {fault}", at.display()))
}

fn ask_for(showing: &dyn Showing, wanted: &Wanted, going_on: &str) -> Result<(), Never> {
    match write_down(wanted) {
        Ok(()) => {
            let Ok(words) = words(&["console-sky", "--now"]);

            showing.note(going_on);
            showing.later(words);
        }
        Err(why) => {
            let Ok(()) = say("wallpaper-choice", "Couldn't change the wallpaper", &why);
        }
    }

    Ok(())
}

fn offered() -> Result<Vec<Offered>, Never> {
    let Ok(table_at) = place::table();

    let held = std::fs::read_to_string(table_at);

    let table = match held.as_deref() {
        Ok(said) => {
            let Ok(set) = Set::read(said);

            set
        }
        Err(_) => None,
    };
    let written_down = |name: &str| {
        let set = table.as_ref()?;

        set.pictures
            .iter()
            .find(|picture| picture.name == name)
            .map(|picture| Offered {
                name: picture.name.clone(),
                says: picture.says.clone(),
                by: picture.by.clone(),
            })
    };
    let Ok(every) = place::every();

    Ok(every
        .into_iter()
        .map(|name| match written_down(&name) {
            Some(offered) => offered,
            None => {
                let Ok(offered) = Offered::of(&name);

                offered
            },
        })
        .collect())
}

fn dropped() -> Result<usize, Never> {
    let Ok(dropped) = place::dropped();

    Ok(dropped
        .and_then(|at| {
            let Ok(found) = std::fs::read_dir(at) else { return None };

            Some(found)
        })
        .map(|found| {
            found
                .flatten()
                .filter(|entry| entry.path().is_file())
                .count()
        })
        .unwrap_or(0))
}

fn wallpaper_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(up) = on_the_screen();

    wallpaper_at(&up)
}

fn wallpaper_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(up) = was_on_the_screen();

    wallpaper_at(&up)
}

fn named(pictures: &[Offered], name: &str) -> Result<String, Never> {
    Ok(pictures
        .iter()
        .find(|picture| picture.name == name)
        .map(|picture| picture.says.clone())
        .unwrap_or_else(|| {
            let Ok(offered) = Offered::of(name);

            offered.says
        }))
}

fn wallpaper_at(up: &str) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(asked) = Wanted::asked();

    let up = up.to_string();
    let Ok(pictures) = offered();
    let Ok(waiting) = dropped();
    let found = Found {
        pictures: &pictures,
        following: asked.follow,
        up: &up,
        dropped: waiting,
    };
    let Ok(taking) = Does::and_stay(move |showing| {
        let Ok(words) = words(&["sky-press", "--dropped"]);

        showing.note(&match waiting {
            1 => "The picture is being taken up, which takes about a minute".to_string(),
            many => format!("The {many} pictures are being taken up, about a minute each"),
        });
        showing.later(words);
    });
    let Ok(finding) = Does::run(&["files-panel", "Pictures"]);

    wallpaper_rows(
        &found,
        |following| {
            let picture = up.clone();
            let Ok(said) = named(&pictures, &up);
            let going_on = match (following, up.is_empty()) {
                (true, _) => "The wallpaper is following the weather again".to_string(),
                (false, true) => "The wallpaper is staying where it is".to_string(),
                (false, false) => format!("The wallpaper is staying on {said}"),
            };
            let Ok(does) = Does::and_stay(move |showing| {
                let Ok(()) = ask_for(
                    showing,
                    &Wanted {
                        follow: following,
                        picture: picture.clone(),
                    },
                    &going_on,
                );

                showing.refresh();
            });

            does
        },
        |name| {
            let picture = name.to_string();
            let Ok(said) = named(&pictures, name);
            let going_on = format!("{said} is going up");
            let Ok(does) = Does::and_stay(move |showing| {
                let Ok(()) = ask_for(
                    showing,
                    &Wanted {
                        follow: false,
                        picture: picture.clone(),
                    },
                    &going_on,
                );

                showing.refresh();
            });

            does
        },
        taking,
        finding,
    )
}

fn applications() -> Result<Vec<Application>, Never> {
    let hers = match std::env::var("HOME") {
        Ok(home) => format!("{home}/.local/share/applications"),

        Err(_) => String::new(),
    };

    let looking = [
        hers,
        "/usr/local/share/applications".to_string(),
        "/usr/share/applications".to_string(),
    ];
    let mut found: Vec<Application> = Vec::new();

    for at in looking {
        let Ok(holding) = std::fs::read_dir(&at) else {
            continue;
        };

        for entry in holding.flatten() {
            let path = entry.path();

            match path.extension().is_none_or(|kind| kind != "desktop") {
                true => continue,
                false => {},
            }

            let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            match found.iter().any(|held| held.id == id) {
                true => continue,
                false => {},
            }

            let Ok(held) = std::fs::read_to_string(&path) else { continue };

            let named = defaults::application(id, &held)?;

            match named {
                Some(application) => found.push(application),
                None => {},
            }
        }
    }

    Ok(found)
}

fn opening(mime: &str) -> Result<String, Never> {
    said(Program::XdgMime, &["query", "default", mime])
}

fn use_it(
    looking: &Held,
    kind: &defaults::Kind,
    application: &Application,
) -> Result<Does, Never> {
    let Ok(every) = kind.every();

    let every: Vec<String> = every.map(str::to_string).collect();
    let scheme = kind.mime.starts_with("x-scheme-handler/");
    let id = application.id.clone();
    let looking = looking.clone();

    Does::and_stay(move |showing| {
        for mime in &every {
            let Ok(_) = said(Program::XdgMime, &["default", &id, mime]);
        }

        match scheme {
            true => {
                let Ok(_) = said(Program::XdgSettings, &["set", "default-web-browser", &id]);
            }
            false => {},
        }

        let Ok(()) = went_up(&looking, showing);
    })
}

enum Msg {
    Heard(Heard, Answer<Vec<Doing<Its>>>),
    At(Answer<Onto>),
}

struct Looking(Onto);

impl actor::Machine for Looking {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Heard(heard, answer) => {
                let Turn { now, doings } = Settings::heard(&self.0, &Word::Its(heard));
                let _ = answer.say(doings);

                Looking(now)
            }
            Msg::At(answer) => {
                let _ = answer.say(self.0);

                self
            },
        }
    }
}

type Held = Addr<Msg>;

fn looking_at(held: &Held) -> Result<Onto, Never> {
    Ok(match held.ask(Msg::At) {
        Ok(onto) => onto,
        Err(_) => Onto::Settings,
    })
}

fn press(held: &Held, heard: Heard, showing: &dyn Showing) -> Result<(), Never> {
    let doings = match held.ask(|answer| Msg::Heard(heard, answer)) {
        Ok(doings) => doings,
        Err(_) => {
            eprintln!("settings-panel: the panel's own state has gone, so the press did nothing");

            Vec::new()
        }
    };

    for doing in doings {
        match doing {
            Doing::Its(Its::Replace(row)) => showing.replace(row),
            Doing::Its(Its::Home(shape)) => {
                let Ok(()) = home_set(shape);
            }

            Doing::Ask(_)
            | Doing::Watch(_)
            | Doing::AskWhoever(_)
            | Doing::Start(_)
            | Doing::Listen(_)
            | Doing::Deafen(_)
            | Doing::Write(_)
            | Doing::Say(_)
            | Doing::Print(_)
            | Doing::Stop(_) => {},
        }
    }

    Ok(())
}

fn went_up(held: &Held, showing: &dyn Showing) -> Result<(), Never> {
    press(held, Heard::Back, showing)
}

fn back_up(held: &Held) -> Result<Chosen, Never> {
    let held = held.clone();

    Ok(Arc::new(move |showing: &dyn Showing| {
        let Ok(()) = went_up(&held, showing);
    }))
}

fn open(held: &Held, onto: Onto) -> Result<Does, Never> {
    let held = held.clone();

    Does::and_stay(move |showing| {
        let Ok(()) = press(&held, Heard::Opened(onto), showing);
    })
}

fn defaults_tab(looking: &Held) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(onto) = looking_at(looking);

    match onto {
        Onto::Settings => setting_rows(looking),
        Onto::Search => {
            let Ok(back) = back_up(looking);

            let Ok(chosen) = console_default_applications::engines::chosen();

            search_rows(&chosen, back)
        }
        Onto::Dictation => {
            let Ok(back) = back_up(looking);

            let Ok(chosen) = console_input_dictation::languages::chosen();

            dictation_rows(&chosen, back)
        }
        Onto::Kind(at) => {
            let leaving = looking.clone();
            let chosen = looking.clone();

            let Some(kind) = defaults::KINDS.get(at) else { return Ok(Vec::new()) };

            let Ok(applications) = applications();

            defaults::choice_rows(
                kind,
                &applications,
                &opening,
                move |showing| {
                    let Ok(()) = went_up(&leaving, showing);
                },
                move |kind, application| use_it(&chosen, kind, application),
            )
        }
    }
}

fn defaults_meanwhile(looking: &Held) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(onto) = looking_at(looking);

    match onto {
        Onto::Settings => {
            let Ok(search) = search_row(looking);
            let Ok(dictation) = dictation_row(looking);
            let Ok(naming) = console_panel::page::Row::naming("On this device", "");
            let Ok(buttons) = buttons_row();
            let Ok(kinds) = defaults::meanwhile_rows(|at| open(looking, Onto::Kind(at)));
            let mut rows = vec![search, dictation, naming, buttons];

            rows.extend(kinds);

            Ok(rows)
        }
        Onto::Search | Onto::Dictation | Onto::Kind(_) => Ok(Vec::new()),
    }
}

fn search_row(looking: &Held) -> Result<console_panel::page::Row, Never> {
    let Ok(engine) = console_default_applications::engines::chosen();
    let Ok(says) = engine_says(&engine);
    let Ok(opens) = open(looking, Onto::Search);
    let Ok(row) = console_panel::page::Row::new("Search", &says, opens);

    row.opening()
}

fn dictation_row(looking: &Held) -> Result<console_panel::page::Row, Never> {
    let Ok(language) = console_input_dictation::languages::chosen();
    let Ok(says) = dictation_says(&language);
    let Ok(opens) = open(looking, Onto::Dictation);
    let Ok(row) = console_panel::page::Row::new("Dictation", &says, opens);

    row.opening()
}

fn buttons_row() -> Result<console_panel::page::Row, Never> {
    let Ok(runs) = Does::run(&["/usr/local/bin/layout-panel"]);
    let Ok(row) = console_panel::page::Row::new("Buttons", "", runs);

    row.opening()
}

fn setting_rows(looking: &Held) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(search) = search_row(looking);
    let Ok(dictation) = dictation_row(looking);
    let Ok(naming) = console_panel::page::Row::naming("On this device", "");
    let Ok(buttons) = buttons_row();
    let Ok(applications) = applications();
    let Ok(kinds) = defaults::defaults_rows(&applications, &opening, |at| {
        open(looking, Onto::Kind(at))
    });
    let mut rows = vec![search, dictation, naming, buttons];

    rows.extend(kinds);

    Ok(rows)
}

fn tab(
    of: impl Fn() -> Result<Vec<console_panel::page::Row>, Never> + Send + Sync + 'static,
) -> Result<Rows, Never> {
    Rows::asked(move || {
        let Ok(rows) = of();

        rows
    })
}

fn pages(looking: &Held) -> Result<Vec<Page>, Never> {
    let drawing = looking.clone();
    let backing = looking.clone();
    let waiting = looking.clone();
    let sizing = looking.clone();
    let drawing_size = looking.clone();

    let Ok([sound, bluetooth, wifi, battery, notifications, size, wallpaper, configuration, system]) =
        tabs();

    let Ok(asked) = tab(sound_tab);
    let Ok(page) = Page::new(&sound, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(rows) = sound_meanwhile();

        rows
    });
    let Ok(stdbuf) = Program::Stdbuf.name();
    let Ok(pactl) = Program::Pactl.name();
    let Ok(watch) = console_panel::page::Watch::on(&[stdbuf, "-oL", pactl, "subscribe"], "on sink");
    let Ok(sound) = page.watching(watch);

    let Ok(asked) = tab(bluetooth_tab);
    let Ok(page) = Page::new(&bluetooth, asked);
    let Ok(bluetooth) = page.meanwhile(move || {
        let Ok(rows) = bluetooth_meanwhile();

        rows
    });

    let Ok(asked) = tab(wifi_tab);
    let Ok(page) = Page::new(&wifi, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(rows) = wifi_meanwhile();

        rows
    });
    let Ok(wifi) = page.on_arriving(|showing| {
        let Ok(()) = look_again(showing);
    });

    let Ok(asked) = tab(battery_tab);
    let Ok(page) = Page::new(&battery, asked);
    let Ok(battery) = page.meanwhile(move || {
        let Ok(rows) = battery_meanwhile();

        rows
    });

    let Ok(asked) = tab(notifications_tab);
    let Ok(page) = Page::new(&notifications, asked);
    let Ok(notifications) = page.meanwhile(move || {
        let Ok(rows) = notifications_meanwhile();

        rows
    });

    let Ok(asked) = tab(move || size_tab(&drawing_size));
    let Ok(page) = Page::new(&size, asked);
    let Ok(size) = page.meanwhile(move || {
        let Ok(rows) = size_meanwhile(&sizing);

        rows
    });

    let Ok(asked) = tab(wallpaper_tab);
    let Ok(page) = Page::new(&wallpaper, asked);
    let Ok(wallpaper) = page.meanwhile(move || {
        let Ok(rows) = wallpaper_meanwhile();

        rows
    });

    let Ok(asked) = tab(move || defaults_tab(&drawing));
    let Ok(page) = Page::new(&configuration, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(rows) = defaults_meanwhile(&waiting);

        rows
    });
    let Ok(configuration) = page.on_back(move |showing| {
        let Ok(was) = looking_at(&backing);
        let Ok(()) = press(&backing, Heard::Back, showing);

        let Ok(closes) = closes(was);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });

    let Ok(asked) = tab(system_rows);
    let Ok(system) = Page::new(&system, asked);

    Ok(vec![
        sound,
        bluetooth,
        wifi,
        battery,
        notifications,
        size,
        wallpaper,
        configuration,
        system,
    ])
}

