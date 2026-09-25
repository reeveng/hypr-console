//! The settings, drawn.
//!
//! What is here is the reading of the machine. What each tab holds once it has
//! been read is `crate::rows`, where it can be asked without a
//! machine to ask.
//!
//! Anything that takes a moment, connecting above all, is done off to one side
//! so the panel keeps answering the buttons while it happens.


use console_books::appearance::Appearance;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_internal_programs::InternalProgram;
use console_core_number_conversion::fitted;
use std::sync::Arc;

pub const WHO: &str = "settings-panel";

pub fn door(arguments: &[String]) -> Result<Door, Never> {
    Door::closing_at("settings", arguments.first().map(String::as_str))
}

pub fn card(arguments: &[String]) -> Result<Card, Never> {
    let initial = Settings::init(&Arguments::default());
    let Ok(card) = Card::supervised(move || Looking(initial.state.clone()), pages);

    card.opening_at(arguments.first().map(String::as_str))
}

use console_battery as battery;
use console_panel::actor::{self, Address, Answer};
use console_panel::page::{Aside, Handler, Level, Page, Rows, Showing};
use console_panel::running::{Notification, said, say};
use console_panel::before;
use console_panel::card::{Card, Door};
use crate::defaults::{self, Application};
use crate::level::{Step, stepped};
use console_sound_effects::SoundEffects;
use crate::rows::{Chosen, Languages, Opens, alphabet_rows, battery_rows, bluetooth_rows, called_row, clock_rows, dictation_rows, dictation_says, engine_says, language_rows, meeting_rows, notifications_rows, place_rows, region_rows, screen_rows, search_rows, sound_rows, tabs, language_picker_rows, security_rows, sound_effects, wifi_rows, zone_rows};
use crate::languages::{self, Names, Language};
use crate::{hours, named};
use console_core_atomic_writes as writes;
use console_input_alphabets as alphabets;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use crate::network_code::{self, Sharing};
use crate::wallpaper::{Found, Offered, wallpaper_rows};
use crate::learned::{self, Following};
use crate::light;
use crate::warm::{self, NightShift};
use crate::{bluetooth, screen, size, sound, turning, wifi};
use console_home_screen::shape::{self, Shape};
use console_program_contract::{Arguments, Effect, Program as _, Update, Event};
use crate::choosing::{Closes, Deeper, SettingsEvent, SettingsEffect, Meeting, Destination, Settings, Under, closes, under};
use console_wallpaper::choose::{Set, Wanted};
use console_wallpaper::place;

const NONE_AT_ALL: u32 = 0;


fn pactl(arguments: &[&str]) -> Result<String, Never> {
    said(Program::Pactl, arguments)
}

fn of_kind(kind: &str) -> Result<Vec<sound::Thing>, Never> {
    let Ok(said) = pactl(&["-f", "json", "list", &format!("{kind}s")]);

    sound::read(&said)
}

fn words(arguments: &[&str]) -> Result<Vec<String>, Never> {
    Ok(arguments.iter().map(|word| (*word).to_string()).collect())
}

fn hush(index: i64, kind: &'static str) -> Result<Handler, Never> {
    Handler::and_stay(move |_| {
        let Ok(_) = pactl(&[&format!("set-{kind}-mute"), &index.to_string(), "toggle"]);
    })
}

fn turn_to(index: i64, kind: &'static str) -> Result<Level, Never> {
    Ok(Arc::new(move |step| {
        let Ok(of_kind) = of_kind(kind);

        let Ok(one) = sound::one(&of_kind, index);

        let thing = match one.cloned() {
            Some(thing) => thing,
            None => return,
        };

        let Ok(level) = thing.level();
        let Ok(going) = stepped(level, Step(step));

        let Ok(_) = pactl(&[
            &format!("set-{kind}-volume"),
            &index.to_string(),
            &format!("{going}%"),
        ]);
    }))
}

const SINKS: &str = "sinks";
const SPEAKERS: &str = "default sink";

fn asked(note: &str, program: Program, arguments: &[&str]) -> Result<String, Never> {
    before::said(note, program, arguments)
}

fn kept(note: &str) -> Result<String, Never> {
    before::last(note)
}

fn pactl_kept(note: &str, arguments: &[&str]) -> Result<String, Never> {
    asked(note, Program::Pactl, arguments)
}

fn sound_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(sinks) = pactl_kept(SINKS, &["-f", "json", "list", "sinks"]);
    let Ok(playing) = of_kind("sink-input");
    let Ok(speakers) = pactl_kept(SPEAKERS, &["get-default-sink"]);

    let Ok(sinks) = sound::read(&sinks);
    let Ok(mut rows) = sound_rows(&sinks, &playing, &speakers, hush, turn_to);
    let Ok(effects) = SoundEffects::chosen();
    let Ok(switch) = sound_effects(effects);

    rows.push(switch);

    Ok(rows)
}

fn sound_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(sinks) = kept(SINKS);
    let Ok(speakers) = kept(SPEAKERS);

    let Ok(sinks) = sound::read(&sinks);
    let Ok(mut rows) = sound_rows(&sinks, &[], &speakers, hush, turn_to);
    let Ok(effects) = SoundEffects::chosen();
    let Ok(switch) = sound_effects(effects);

    rows.push(switch);

    Ok(rows)
}

fn brightness() -> Result<i32, Never> {
    let Ok(panel) = screen::here();

    let points = match panel {
        Some(panel) => {
            let Ok(now) = panel.now();

            match now {
                Some(now) => panel.as_points(now)?,
                None => 0,
            }
        }
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

        let Ok(panel) = screen::here();

        match panel {
            Some(panel) => {
                let Ok(now) = panel.now();

                match now {
                    Some(now) => {
                        let Ok(going) = panel.stepped(now, way);

                        let _ = panel.set(going);
                    }
                    None => {},
                }
            }
            None => {},
        }
    }))
}

fn guard(step: battery::Step) -> Result<Level, Never> {
    Ok(Arc::new(move |way| {
        let Ok(levels) = battery::Levels::here();
        let Ok(at) = levels.at(step);
        let Ok(going) = stepped(at, Step(way));
        let Ok(_) = levels.set(step, going);
    }))
}

const PROFILE: &str = "power profile";

fn battery_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(profile) = asked(PROFILE, Program::Powerprofilesctl, &["get"]);

    let Ok(levels) = battery::Levels::here();

    battery_rows(Some(&profile), levels, guard)
}

fn warmth() -> Result<NightShift, Never> {
    let Ok(said) = console_core_places::home();

    let home = match said {
        Some(home) => home,
        None => return Ok(NightShift::Scheduled),
    };

    let Ok(standing) = warm::standing(&home);

    match standing {
        warm::Standing::Loaded(warmth) => Ok(warmth),
        warm::Standing::Invalid(fault) => {
            let Ok(at) = warm::at(&home);

            eprintln!(
                "settings-panel: {} says whether the screen follows the clock, and it will not \
                 be read: {fault}. The row is drawn as though it were following, which is what a \
                 machine no one has told does.",
                at.display()
            );

            Ok(NightShift::Scheduled)
        }
    }
}

fn battery_meanwhile() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(profile) = kept(PROFILE);

    let Ok(levels) = battery::Levels::here();

    battery_rows(Some(&profile), levels, guard)
}

const SCREENS: &str = "screens";

fn standing_at(said: &str) -> Result<Option<size::Size>, Never> {
    let Ok(monitors) = monitors_in(said);

    let monitors = match monitors {
        Some(monitors) => monitors,
        None => return Ok(None),
    };

    size::standing(&monitors)
}

fn monitors_in(said: &str) -> Result<Option<Vec<console_compositor::Monitor>>, Never> {
    Ok(match console_compositor::read(console_compositor::Query::Monitors, said) {
        Ok(console_compositor::Answer::Monitors(monitors)) => Some(monitors),
        Ok(_not_what_was_asked) => None,
        Err(_the_compositor_said_nothing) => None,
    })
}

fn turned_at(said: &str) -> Result<Option<turning::Turn>, Never> {
    let Ok(monitors) = monitors_in(said);

    let monitors = match monitors {
        Some(monitors) => monitors,
        None => return Ok(None),
    };
    let Ok(shown) = console_screen::shown(&monitors);

    match shown {
        Some(screen) => turning::standing(&screen),
        None => Ok(None),
    }
}

fn room() -> Result<Option<Following>, Never> {
    let Ok(found) = light::here();

    match found {
        Some(_a_machine_with_eyes) => {},
        None => return Ok(None),
    }

    let Ok(home) = console_core_places::home();

    let home = match home {
        Some(home) => home,
        None => return Ok(None),
    };

    let Ok(following) = learned::following(&home);

    Ok(Some(following))
}

fn size_tab(held: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(brightness) = brightness();
    let Ok(warmth) = warmth();
    let Ok(words) = console_compositor::Query::Monitors.words();
    let Ok(screens) = asked(SCREENS, Program::Hyprctl, words);
    let Ok(home) = home_rows(held);
    let Ok(standing) = standing_at(&screens);
    let Ok(turned) = turned_at(&screens);

    let Ok(dim) = dim();

    let Ok(room) = room();

    screen_rows(Some(brightness), dim, room, warmth, standing, turned, home)
}

fn home_at() -> Result<Option<std::path::PathBuf>, Never> {
    let Ok(said) = console_core_places::home();

    Ok(match said {
        Some(home) => {
            let Ok(at) = shape::at(&home);

            Some(at)
        }
        None => None,
    })
}

fn home_shape() -> Result<Shape, Never> {
    let Ok(home) = home_at();

    let at = match home {
        Some(at) => at,
        None => return Ok(Shape::USUAL),
    };

    let Ok(held) = console_core_atomic_writes::read(&at);

    Ok(match held {
        console_core_atomic_writes::Stored::Text(said) => {
            let Ok(shape) = Shape::read(&said);

            shape
        }
        console_core_atomic_writes::Stored::Absent => Shape::USUAL,
        console_core_atomic_writes::Stored::Failed(fault) => {
            eprintln!("settings-panel: {}: {fault}", at.display());

            Shape::USUAL
        },
    })
}

fn home_set(shape: Shape) -> Result<(), Never> {
    let Ok(home) = home_at();

    let at = match home {
        Some(at) => at,
        None => {
            eprintln!("settings-panel: this machine will not say whose home to write the grid in");

            return Ok(());
        }
    };

    match at.parent().map(std::fs::create_dir_all) {
        Some(Err(fault)) => {
            eprintln!("settings-panel: {}: {fault}", at.display());

            return Ok(());
        }
        Some(Ok(())) | None => {},
    }

    let Ok(written) = shape.written();

    match console_core_atomic_writes::whole(&at, written.as_bytes()) {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("settings-panel: {}: {fault}", at.display());

            return Ok(());
        }
    }

    match console_panel::door::telling(console_panel::door::PadInput::Again) {
        Ok(()) => {},
        Err(fault) => eprintln!("settings-panel: the home screen was not told: {fault}"),
    }

    Ok(())
}

fn home_rows(held: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let across = held.clone();
    let down = held.clone();
    let sized = held.clone();
    let Ok(shape) = home_shape();

    crate::rows::home_rows(
        shape,
        Arc::new(move |step| {
            let Ok(shape) = home_shape();
            let Ok(()) = grid(&across, SettingsEvent::Across { shape, step });
        }),
        Arc::new(move |step| {
            let Ok(shape) = home_shape();
            let Ok(()) = grid(&down, SettingsEvent::Down { shape, step });
        }),
        Arc::new(move |step| {
            let Ok(shape) = home_shape();
            let Ok(()) = grid(&sized, SettingsEvent::Sized { shape, step });
        }),
    )
}

fn grid(held: &ActorAddress, heard: SettingsEvent) -> Result<(), Never> {
    let effects = match held.ask(|answer| Message::Event(heard, answer)) {
        Ok(effects) => effects,
        Err(_the_actor_has_gone) => {
            eprintln!("settings-panel: the panel's own state is missing, so the grid did not move");

            Vec::new()
        }
    };

    for shape in effects.iter().filter_map(|effect| {
        let Ok(shape) = shaped(effect);

        shape
    }) {
        let Ok(()) = home_set(*shape);
    }

    Ok(())
}

fn shaped(effect: &Effect<SettingsEffect>) -> Result<Option<&shape::Shape>, Never> {
    Ok(match effect {
        Effect::Custom(SettingsEffect::HomeScreen(shape)) => Some(shape),

        Effect::Custom(SettingsEffect::Replace(_))
        | Effect::Run(_)
        | Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_) => None,
    })
}

fn size_meanwhile(held: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(brightness) = brightness();
    let Ok(warmth) = warmth();
    let Ok(screens) = kept(SCREENS);
    let Ok(home) = home_rows(held);
    let Ok(standing) = standing_at(&screens);
    let Ok(turned) = turned_at(&screens);

    let Ok(dim) = dim();

    let Ok(room) = room();

    screen_rows(Some(brightness), dim, room, warmth, standing, turned, home)
}

fn join(network: wifi::Network, known: wifi::Known) -> Result<Handler, Never> {
    Handler::and_stay(move |showing| {
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
            &format!("Password for {name}"),
            Arc::new(move |showing, word| {
                let Ok(words) = words(&[
                    "nmcli", "device", "wifi", "connect", &asking, "password", word,
                ]);

                showing.later(words);
            }),
        );
    })
}

fn notifications_tab() -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(held) = console_notifications::serving::held();

    notifications_rows(held.do_not_disturb)
}

const WIFI_RADIO: &str = "wifi radio";
const KNOWN: &str = "wifi known";
const IN_RANGE: &str = "wifi in range";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Wifi<'a> {
    radio: &'a str,
    known: &'a str,
    in_range: &'a str,
}

fn wifi_at(said: Wifi<'_>, sharing: &Sharing) -> Result<Vec<console_panel::page::Row>, Never> {
    let on = wifi::on(said.radio)?;
    let networks = wifi::networks(said.in_range)?;
    let saved = wifi::saved(said.known)?;

    wifi_rows(on, networks, &saved, join, |network| network_code::offered(sharing, network))
}

fn wifi_tab(sharing: &Sharing) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(shown) = network_code::shared(sharing);

    match shown {
        Some(network) => return network_code::rows(&network),
        None => {},
    }

    let Ok(radio) = asked(WIFI_RADIO, Program::Nmcli, &["radio", "wifi"]);
    let Ok(known) = asked(KNOWN, Program::Nmcli, &wifi::KNOWN);
    let Ok(in_range) = asked(
        IN_RANGE,
        Program::Nmcli,
        &["-t", "-f", wifi::FIELDS, "device", "wifi", "list"],
    );

    wifi_at(Wifi { radio: &radio, known: &known, in_range: &in_range }, sharing)
}

fn wifi_meanwhile(sharing: &Sharing) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(shown) = network_code::shared(sharing);

    match shown {
        Some(_) => return Ok(Vec::new()),
        None => {},
    }

    let Ok(radio) = kept(WIFI_RADIO);
    let Ok(known) = kept(KNOWN);
    let Ok(in_range) = kept(IN_RANGE);

    wifi_at(Wifi { radio: &radio, known: &known, in_range: &in_range }, sharing)
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

fn meeting(looking: &ActorAddress) -> Result<Opens, Never> {
    let held = looking.clone();

    Ok(Arc::new(move |met: &bluetooth::Met, at: u32, showing: &dyn Showing| {
        let meeting = Meeting { address: met.device.address.clone(), at };
        let Ok(()) = press(&held, SettingsEvent::Opened(Destination::Meeting(meeting)), showing);
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bluetooth<'a> {
    radio: &'a str,
    introduced: &'a str,
}

fn bluetooth_at(
    looking: &ActorAddress,
    said: Bluetooth<'_>,
    ask: impl Fn(&str) -> String,
) -> Result<Vec<console_panel::page::Row>, Never> {
    let mut met = Vec::new();

    let devices = bluetooth::devices(said.introduced)?;

    for device in devices {
        let said = ask(&device.address);
        let joined = bluetooth::joined(&said)?;
        let known = bluetooth::known(&said)?;
        let heard = bluetooth::heard(&said)?;

        met.push(bluetooth::Met { device, known, joined, heard });
    }

    let on = bluetooth::on(said.radio)?;
    let looking_now = bluetooth::looking(said.radio)?;
    let Ok(onto) = looking_at(looking);

    let standing = match onto {
        Destination::Meeting(meeting) => {
            met.iter().find(|met| met.device.address == meeting.address).cloned()
        }
        Destination::Settings
        | Destination::Search
        | Destination::Dictation
        | Destination::Kind(_)
        | Destination::Languages
        | Destination::Language(_)
        | Destination::Alphabets
        | Destination::Zones
        | Destination::Zone(_)
        | Destination::Clock => None,
    };

    match standing {
        Some(standing) => {
            let Ok(back) = back_up(looking);

            meeting_rows(&standing, back)
        }
        None => {
            let Ok(opens) = meeting(looking);

            bluetooth_rows(on, looking_now, met, opens)
        }
    }
}

fn bluetooth_tab(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(radio) = asked(BLUETOOTH_RADIO, Program::Bluetoothctl, &["show"]);
    let Ok(introduced) = asked(INTRODUCED, Program::Bluetoothctl, &["devices"]);

    bluetooth_at(looking, Bluetooth { radio: &radio, introduced: &introduced }, |address| {
        let Ok(about) = about(address);
        let Ok(said) = asked(&about, Program::Bluetoothctl, &["info", address]);

        said
    })
}

fn bluetooth_meanwhile(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(radio) = kept(BLUETOOTH_RADIO);
    let Ok(introduced) = kept(INTRODUCED);

    bluetooth_at(looking, Bluetooth { radio: &radio, introduced: &introduced }, |address| {
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

#[derive(Debug)]
enum Unchosen {
    NoOnes,
    Holding(std::path::PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
}

impl std::fmt::Display for Unchosen {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unchosen::NoOnes => {
                write!(to, "This machine will not say whose home to write it in.")
            }
            Unchosen::Holding(at, fault) => {
                write!(to, "{} could not be made: {fault}", at.display())
            }
            Unchosen::Writing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unchosen {}

fn write_down(wanted: &Wanted) -> Result<(), Unchosen> {
    let Ok(asked) = place::asked();

    let at = asked.ok_or(Unchosen::NoOnes)?;

    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unchosen::Holding(holding.to_path_buf(), fault))?,
        None => {},
    }

    let Ok(written) = wanted.written();

    console_core_atomic_writes::whole(&at, written.as_bytes()).map_err(Unchosen::Writing)
}

fn ask_for(showing: &dyn Showing, wanted: &Wanted, going_on: &str) -> Result<(), Never> {
    match write_down(wanted) {
        Ok(()) => {
            let Ok(words) = words(&["console-wallpaper", "--now"]);

            showing.note(going_on);
            showing.later(words);
        }
        Err(why) => {
            let Ok(()) = say("wallpaper-choice", Notification {
                summary: "Couldn't change the wallpaper",
                body: &why.to_string(),
            });
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
        Err(_unreadable) => None,
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

fn dropped() -> Result<u32, Never> {
    let Ok(dropped) = place::dropped();

    match dropped
        .and_then(|at| match std::fs::read_dir(at) {
            Ok(found) => Some(found),
            Err(_unreadable) => None,
        })
        .map(|found| found.flatten().filter(|entry| entry.path().is_file()).count())
    {
        Some(many) => fitted(many),
        None => Ok(NONE_AT_ALL),
    }
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
    let found = pictures.iter().find(|picture| picture.name == name).map(|one| one.says.clone());

    Ok(match found {
        Some(says) => says,
        None => {
            let Ok(offered) = Offered::of(name);

            offered.says
        }
    })
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
    let Ok(taking) = Handler::and_stay(move |showing| {
        let Ok(words) = words(&["wallpaper-render", "--dropped"]);

        showing.note(&match waiting {
            1 => "Adding the picture, about a minute".to_string(),
            many => format!("Adding {many} pictures, a minute each"),
        });
        showing.later(words);
    });
    let Ok(finding) = Handler::run(&["files", "Pictures"]);

    wallpaper_rows(
        &found,
        |following| {
            let picture = up.clone();
            let Ok(said) = named(&pictures, &up);
            let going_on = match (following, up.is_empty()) {
                (true, _) => "Wallpaper follows the weather".to_string(),
                (false, true) => "Wallpaper stays as it is".to_string(),
                (false, false) => format!("Wallpaper stays on {said}"),
            };
            let Ok(does) = Handler::and_stay(move |showing| {
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
            let Ok(does) = Handler::and_stay(move |showing| {
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
    let looking = console_core_places::applications()?;

    let files = console_applications::entry::files(&looking)?;

    files.iter().try_fold(Vec::new(), |mut found: Vec<Application>, path| {
        let application = application_at(path)?;

        match application {
            Some(application) => found.push(application),
            None => {},
        }

        Ok(found)
    })
}

fn application_at(path: &std::path::Path) -> Result<Option<Application>, Never> {
    let id = match path.file_name().and_then(|name| name.to_str()) {
        Some(id) => id,
        None => return Ok(None),
    };

    let held = match std::fs::read_to_string(path) {
        Ok(held) => held,
        Err(_unreadable) => return Ok(None),
    };

    defaults::application(id, defaults::DesktopFilePath(&held))
}

fn opening(mime: &str) -> Result<String, Never> {
    said(Program::XdgMime, &["query", "default", mime])
}

fn use_it(
    looking: &ActorAddress,
    kind: &defaults::Kind,
    application: &Application,
) -> Result<Handler, Never> {
    let Ok(every) = kind.every();

    let every: Vec<String> = every.map(str::to_string).collect();
    let scheme = kind.mime.starts_with("x-scheme-handler/");
    let id = application.id.clone();
    let looking = looking.clone();

    Handler::and_stay(move |showing| {
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

enum Message {
    Event(SettingsEvent, Answer<Vec<Effect<SettingsEffect>>>),
    At(Answer<Destination>),
}

struct Looking(Destination);

impl actor::Machine for Looking {
    type Message = Message;

    fn step(self, message: Message) -> Self {
        match message {
            Message::Event(heard, answer) => {
                let Update { state, effects } = Settings::update(&self.0, &Event::Custom(heard));
                let _ = answer.say(effects);

                Looking(state)
            }
            Message::At(answer) => {
                let _ = answer.say(self.0.clone());

                self
            },
        }
    }
}

type ActorAddress = Address<Message>;

fn looking_at(held: &ActorAddress) -> Result<Destination, Never> {
    Ok(match held.ask(Message::At) {
        Ok(onto) => onto,
        Err(_the_actor_has_gone) => Destination::Settings,
    })
}

fn press(held: &ActorAddress, heard: SettingsEvent, showing: &dyn Showing) -> Result<(), Never> {
    let effects = match held.ask(|answer| Message::Event(heard, answer)) {
        Ok(effects) => effects,
        Err(_the_actor_has_gone) => {
            eprintln!("settings-panel: the panel's own state is missing, so the press did nothing");

            Vec::new()
        }
    };

    for effect in effects {
        match effect {
            Effect::Custom(SettingsEffect::Replace(row)) => {
                let Ok(row) = console_core_number_conversion::fitted(row);

                showing.replace(row)
            }
            Effect::Custom(SettingsEffect::HomeScreen(shape)) => {
                let Ok(()) = home_set(shape);
            }

            Effect::Run(_)
            | Effect::Stream(_)
            | Effect::Prompt(_)
            | Effect::Spawn(_)
            | Effect::Subscribe(_)
            | Effect::Unsubscribe(_)
            | Effect::Write(_)
            | Effect::Notify(_)
            | Effect::Print(_)
            | Effect::Stop(_) => {},
        }
    }

    Ok(())
}

fn went_up(held: &ActorAddress, showing: &dyn Showing) -> Result<(), Never> {
    press(held, SettingsEvent::Back, showing)
}

fn back_up(held: &ActorAddress) -> Result<Chosen, Never> {
    let held = held.clone();

    Ok(Arc::new(move |showing: &dyn Showing| {
        let Ok(()) = went_up(&held, showing);
    }))
}

fn open(held: &ActorAddress, onto: Destination) -> Result<Handler, Never> {
    let held = held.clone();

    Handler::and_stay(move |showing| {
        let Ok(()) = press(&held, SettingsEvent::Opened(onto.clone()), showing);
    })
}

fn defaults_tab(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(onto) = looking_at(looking);
    let Ok(under) = under(&onto);

    match under {
        Under::Configuration => {},
        Under::Language => return setting_rows(looking),
    }

    match onto {
        Destination::Settings => setting_rows(looking),
        Destination::Search => {
            let Ok(back) = back_up(looking);

            let Ok(chosen) = console_default_applications::engines::chosen();

            search_rows(&chosen, back)
        }
        Destination::Zones => {
            let Ok(back) = back_up(looking);
            let Ok(zones) = zones();
            let Ok(regions) = hours::regions(&zones);
            let Ok(here) = here();

            region_rows(&regions, here.as_deref(), back, |at, region| {
                open(looking, Destination::Zone(Deeper { name: region.to_string(), at }))
            })
        }
        Destination::Zone(deeper) => {
            let Ok(back) = back_up(looking);
            let Ok(zones) = zones();
            let Ok(places) = hours::places(&zones, &deeper.name);
            let Ok(here) = here();

            zone_rows(&deeper.name, &places, here.as_deref(), back)
        }
        Destination::Clock => {
            let Ok(back) = back_up(looking);
            let Ok(now) = console_default_applications::clock::clock();

            clock_rows(now, back)
        }
        Destination::Meeting(_)
        | Destination::Dictation
        | Destination::Languages
        | Destination::Language(_)
        | Destination::Alphabets => setting_rows(looking),

        Destination::Kind(at) => {
            let leaving = looking.clone();
            let chosen = looking.clone();

            let Ok(at) = console_core_number_conversion::index(at);

            let kind = match defaults::KINDS.get(at) {
                Some(kind) => kind,
                None => return Ok(Vec::new()),
            };

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

fn defaults_meanwhile(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(onto) = looking_at(looking);

    match onto {
        Destination::Settings => {
            let Ok(search) = search_row(looking);
            let Ok(where_) = where_row(looking);
            let Ok(clock) = clock_row(looking);
            let Ok(called) = named_row();
            let Ok(naming) = console_panel::page::Row::naming("On this device", Aside(""));
            let Ok(buttons) = buttons_row();
            let Ok(kinds) = defaults::meanwhile_rows(|at| open(looking, Destination::Kind(at)));
            let mut rows = vec![search, where_, clock, called, naming, buttons];

            rows.extend(kinds);

            Ok(rows)
        }
        Destination::Search
        | Destination::Dictation
        | Destination::Kind(_)
        | Destination::Meeting(_)
        | Destination::Languages
        | Destination::Language(_)
        | Destination::Alphabets
        | Destination::Zones
        | Destination::Zone(_)
        | Destination::Clock => Ok(Vec::new()),
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Description<'a>(&'a str);

fn run(at: &str, what: Description<'_>) -> Result<String, Never> {
    let Ok(held) = writes::read(Path::new(at));

    Ok(match held {
        writes::Stored::Text(said) => said,
        writes::Stored::Absent => String::new(),
        writes::Stored::Failed(fault) => {
            eprintln!(
                "settings-panel: {at} is {}, and it will not be read: {fault}. The list is \
                 drawn empty, which is a machine that says it can do nothing rather than one \
                 that quietly offers half.",
                what.0
            );

            String::new()
        }
    })
}

fn names() -> Result<&'static Names, Never> {
    #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        allow(
            explicit044_no_ambient_value,
            reason = "the names glibc gives the languages it ships, which are part of the installed system and do not change while a panel is up; the panel is an actor whose state is the row someone is looking at, and a table of language names is not that"
        )
    )]
    static ASKED: OnceLock<Names> = OnceLock::new();

    Ok(ASKED.get_or_init(|| {
        let Ok(names) = Names::here();

        names
    }))
}

fn spoken() -> Result<&'static Vec<Language>, Never> {
    #[cfg_attr(
        dylint_lib = "explicit044_no_ambient_value",
        allow(
            explicit044_no_ambient_value,
            reason = "what glibc can be asked to make, read off the installed system and unchanged while a panel is up; which of them has been made is the half that changes, and `generated` reads that again at every draw"
        )
    )]
    static ASKED: OnceLock<Vec<Language>> = OnceLock::new();

    Ok(ASKED.get_or_init(|| {
        let Ok(said) = run(languages::SUPPORTED, Description("every language glibc can make"));
        let Ok(supported) = languages::supported(&said);
        let Ok(names) = names();
        let Ok(spoken) = languages::languages(&supported, names);

        spoken
    }))
}

fn generated() -> Result<Vec<String>, Never> {
    let Ok(said) = said(Program::Locale, &["-a"]);

    languages::generated(&said)
}

fn lang() -> Result<Option<String>, Never> {
    let Ok(said) = said(Program::Localectl, &["status"]);

    languages::chosen(&said)
}

const ZONES: &str = "zones";

fn zones() -> Result<Vec<String>, Never> {
    let Ok(said) = asked(ZONES, Program::Timedatectl, &["list-timezones"]);

    hours::zones(&said)
}

fn here() -> Result<Option<String>, Never> {
    let Ok(said) = said(Program::Timedatectl, &["show", "--property=Timezone", "--value"]);

    hours::chosen(&said)
}

fn called() -> Result<String, Never> {
    let Ok(said) = said(Program::Hostnamectl, &["--static"]);

    named::read(&said)
}

fn language_top(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(lang) = lang();
    let Ok(spoken) = spoken();
    let Ok(names) = names();
    let Ok(standing) = languages::standing(spoken, lang.as_deref());

    let says = match &standing {
        Some(locale) => {
            let Ok(says) = names.says(locale);

            says
        }
        None => match lang {
            Some(lang) => lang,
            None => String::new(),
        },
    };

    let Ok(chosen) = alphabets::chosen();
    let Ok(types) = alphabets::said(&chosen);

    let Ok(heard) = console_input_dictation::languages::chosen();
    let Ok(listens) = dictation_says(&heard);

    let Ok(words) = open(looking, Destination::Languages);
    let Ok(keyboard) = open(looking, Destination::Alphabets);
    let Ok(dictation) = open(looking, Destination::Dictation);

    language_rows(
        Languages { says: &says, types: &types, listens: &listens },
        [words, keyboard, dictation],
    )
}

fn language_tab(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(onto) = looking_at(looking);
    let Ok(under) = under(&onto);

    match under {
        Under::Language => {},
        Under::Configuration => return language_top(looking),
    }

    match onto {
        Destination::Languages => {
            let Ok(back) = back_up(looking);
            let Ok(spoken) = spoken();
            let Ok(lang) = lang();
            let Ok(standing) = languages::standing(spoken, lang.as_deref());

            language_picker_rows(spoken, standing.as_ref(), back, |at, language| {
                open(looking, Destination::Language(Deeper { name: language.to_string(), at }))
            })
        }
        Destination::Language(deeper) => {
            let Ok(back) = back_up(looking);
            let Ok(spoken) = spoken();
            let Ok(names) = names();
            let Ok(lang) = lang();
            let Ok(standing) = languages::standing(spoken, lang.as_deref());
            let Ok(generated) = generated();

            let language = match spoken.iter().find(|language| language.language == deeper.name) {
                Some(language) => language,
                None => return language_top(looking),
            };

            place_rows(language, names, &generated, standing.as_ref(), back)
        }
        Destination::Alphabets => {
            let Ok(back) = back_up(looking);
            let Ok(chosen) = alphabets::chosen();

            alphabet_rows(&chosen, back)
        }
        Destination::Dictation => {
            let Ok(back) = back_up(looking);
            let Ok(chosen) = console_input_dictation::languages::chosen();

            dictation_rows(&chosen, back)
        }
        Destination::Settings
        | Destination::Search
        | Destination::Kind(_)
        | Destination::Meeting(_)
        | Destination::Zones
        | Destination::Zone(_)
        | Destination::Clock => language_top(looking),
    }
}

fn where_row(looking: &ActorAddress) -> Result<console_panel::page::Row, Never> {
    let Ok(here) = here();
    let Ok(says) = crate::rows::where_you_are();

    let zone = match here {
        Some(zone) => zone.replace('_', " "),
        None => String::new(),
    };

    let Ok(opens) = open(looking, Destination::Zones);
    let Ok(row) = console_panel::page::Row::new(&says, Aside(&zone), opens);

    row.opening()
}

fn clock_row(looking: &ActorAddress) -> Result<console_panel::page::Row, Never> {
    let Ok(now) = console_default_applications::clock::clock();
    let Ok(reads) = now.says();
    let Ok(says) = crate::rows::the_clock();
    let Ok(opens) = open(looking, Destination::Clock);
    let Ok(row) = console_panel::page::Row::new(&says, Aside(reads), opens);

    row.opening()
}

fn named_row() -> Result<console_panel::page::Row, Never> {
    let Ok(name) = called();

    called_row(&name)
}

fn search_row(looking: &ActorAddress) -> Result<console_panel::page::Row, Never> {
    let Ok(engine) = console_default_applications::engines::chosen();
    let Ok(says) = engine_says(&engine);
    let Ok(opens) = open(looking, Destination::Search);
    let Ok(row) = console_panel::page::Row::new("Search Engine", Aside(&says), opens);

    row.opening()
}

fn buttons_row() -> Result<console_panel::page::Row, Never> {
    let Ok(mapping) = InternalProgram::MappingPanel.path();
    let Ok(runs) = Handler::run(&[mapping]);
    let Ok(row) = console_panel::page::Row::new("Buttons", Aside(""), runs);

    row.opening()
}

fn setting_rows(looking: &ActorAddress) -> Result<Vec<console_panel::page::Row>, Never> {
    let Ok(search) = search_row(looking);
    let Ok(where_) = where_row(looking);
    let Ok(clock) = clock_row(looking);
    let Ok(called) = named_row();
    let Ok(naming) = console_panel::page::Row::naming("On this device", Aside(""));
    let Ok(buttons) = buttons_row();
    let Ok(game) = crate::rows::game_row();
    let Ok(applications) = applications();
    let Ok(kinds) = defaults::defaults_rows(&applications, &opening, |at| {
        open(looking, Destination::Kind(at))
    });
    let mut rows = vec![search, where_, clock, called, naming, buttons, game];

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

fn pages(looking: &ActorAddress) -> Result<Vec<Page>, Never> {
    let Ok(
        [
            sound,
            bluetooth,
            wifi,
            battery,
            notifications,
            size,
            wallpaper,
            language,
            configuration,
            books,
            security,
        ],
    ) = tabs();

    let Ok(sound) = sound_page(&sound);
    let Ok(bluetooth) = bluetooth_page(&bluetooth, looking);
    let Ok(wifi) = wifi_page(&wifi);
    let Ok(battery) = battery_page(&battery);

    let Ok(asked) = tab(notifications_tab);
    let Ok(notifications) = Page::new(&notifications, asked);

    let Ok(size) = size_page(&size, looking);
    let Ok(wallpaper) = wallpaper_page(&wallpaper);
    let Ok(language) = language_page(&language, looking);
    let Ok(configuration) = configuration_page(&configuration, looking);

    let Ok(asked) = tab(|| {
        let Ok(now) = Appearance::chosen();

        crate::rows::books_rows(now)
    });
    let Ok(books) = Page::new(&books, asked);

    let Ok(asked) = tab(security_rows);
    let Ok(security) = Page::new(&security, asked);

    Ok(vec![
        sound,
        bluetooth,
        wifi,
        battery,
        notifications,
        size,
        wallpaper,
        language,
        configuration,
        books,
        security,
    ])
}

fn sound_page(title: &str) -> Result<Page, Never> {
    let Ok(asked) = tab(sound_tab);
    let Ok(page) = Page::new(title, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(rows) = sound_meanwhile();

        rows
    });

    page.listening(console_program_contract::Topic::Sound, console_events::again::sound)
}

fn bluetooth_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
    let drawing_bluetooth = looking.clone();
    let waiting_bluetooth = looking.clone();
    let backing_bluetooth = looking.clone();

    let Ok(asked) = tab(move || bluetooth_tab(&drawing_bluetooth));
    let Ok(page) = Page::new(title, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(rows) = bluetooth_meanwhile(&waiting_bluetooth);

        rows
    });
    let Ok(words) = crate::rows::looking_words();
    let Ok(watch) = console_panel::page::Watch::anything(&words);
    let Ok(page) = page.watching(watch);

    page.on_back(move |showing| {
        let Ok(was) = looking_at(&backing_bluetooth);

        match was {
            Destination::Meeting(_) => {
                let Ok(()) = press(&backing_bluetooth, SettingsEvent::Back, showing);

                false
            }
            Destination::Settings
            | Destination::Search
            | Destination::Dictation
            | Destination::Kind(_)
            | Destination::Languages
            | Destination::Language(_)
            | Destination::Alphabets
            | Destination::Zones
            | Destination::Zone(_)
            | Destination::Clock => true,
        }
    })
}

fn wifi_page(title: &str) -> Result<Page, Never> {
    let sharing: Sharing = Arc::new(Mutex::new(None));
    let drawing = Arc::clone(&sharing);
    let waiting = Arc::clone(&sharing);
    let Ok(asked) = tab(move || wifi_tab(&drawing));
    let Ok(page) = Page::new(title, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(rows) = wifi_meanwhile(&waiting);

        rows
    });
    let Ok(page) = page.on_back(move |showing| {
        let Ok(was) = network_code::put_away(&sharing);

        match was {
            Some(_) => {
                showing.refresh();

                false
            },
            None => true,
        }
    });

    page.on_arriving(|showing| {
        let Ok(()) = look_again(showing);
    })
}

fn battery_page(title: &str) -> Result<Page, Never> {
    let Ok(asked) = tab(battery_tab);
    let Ok(page) = Page::new(title, asked);

    page.meanwhile(move || {
        let Ok(rows) = battery_meanwhile();

        rows
    })
}

fn size_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
    let sizing = looking.clone();
    let drawing_size = looking.clone();

    let Ok(asked) = tab(move || size_tab(&drawing_size));
    let Ok(page) = Page::new(title, asked);

    page.meanwhile(move || {
        let Ok(rows) = size_meanwhile(&sizing);

        rows
    })
}

fn wallpaper_page(title: &str) -> Result<Page, Never> {
    let Ok(asked) = tab(wallpaper_tab);
    let Ok(page) = Page::new(title, asked);

    page.meanwhile(move || {
        let Ok(rows) = wallpaper_meanwhile();

        rows
    })
}

fn backed(page: Page, looking: &ActorAddress) -> Result<Page, Never> {
    let backing = looking.clone();

    page.on_back(move |showing| {
        let Ok(was) = looking_at(&backing);
        let Ok(()) = press(&backing, SettingsEvent::Back, showing);

        let Ok(closes) = closes(&was);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    })
}

fn language_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
    let drawing_language = looking.clone();

    let Ok(asked) = tab(move || language_tab(&drawing_language));
    let Ok(page) = Page::new(title, asked);

    backed(page, looking)
}

fn configuration_page(title: &str, looking: &ActorAddress) -> Result<Page, Never> {
    let drawing = looking.clone();
    let waiting = looking.clone();

    let Ok(asked) = tab(move || defaults_tab(&drawing));
    let Ok(page) = Page::new(title, asked);
    let Ok(page) = page.meanwhile(move || {
        let Ok(rows) = defaults_meanwhile(&waiting);

        rows
    });

    backed(page, looking)
}

