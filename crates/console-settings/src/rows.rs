//! What each tab holds, as a function of what the machine said.
//!
//! Reading the machine is one thing and knowing what to draw from it is
//! another. Everything here is the second, so the shape of every tab can be
//! asked without a machine to ask.

use std::sync::Arc;

use console_default_applications::{battery, engines};
use console_core_external_programs::Program;
use console_home_screen::shape::Shape;
use console_core_never::Never;
use console_core_localization::say;

use crate::words::Word;
use console_notifications::reading::{QUIET, Quiet};
use console_panel::page::{Does, Level, NOW, Row, Showing, YET};
use crate::introducing;
use console_input_dictation::languages;

use console_input_alphabets::{self as alphabets, Alphabet};

use console_default_applications::clock::{self, Clock};

use crate::hours::{self, Place};
use crate::level::{CELLS, Muted, bar, volume};
use crate::named::{self, Allowed};
use crate::tongues::{Locale, Made, Names, Tongue, made};
use crate::size::{EVERY, Size};
use crate::warm::Warmth;
use crate::{bluetooth, sound, wifi};

pub fn switch(says: &str, argv: &[&str]) -> Result<Row, Never> {
    let argv: Vec<String> = argv.iter().map(|word| (*word).to_string()).collect();
    let Ok(later) = Does::and_stay(move |showing| showing.later(argv.clone()));

    Row::new(says, "", later)
}

pub fn sound_rows(
    sinks: &[sound::Thing],
    playing: &[sound::Thing],
    default: &str,
    hush: impl Fn(i64, &'static str) -> Result<Does, Never>,
    turn: impl Fn(i64, &'static str) -> Result<Level, Never>,
) -> Result<Vec<Row>, Never> {
    let mut rows = Vec::new();

    let Ok(found) = sound::speakers(sinks, default);

    match found {
        Some(speakers) => {
            let muted = match speakers.mute {
                true => Muted::Yes,
                false => Muted::No,
            };
            let Ok(at) = speakers.level();
            let Ok(level) = volume(at, muted);

            let Ok(silence) = hush(speakers.index, "sink");
            let Ok(row) = Row::new("Speakers", &level, silence);
            let Ok(turning) = turn(speakers.index, "sink");
            let Ok(levelled) = row.levelled(turning);

            rows.push(levelled);
        }
        None => {},
    }

    for stream in playing {
        let muted = match stream.mute {
            true => Muted::Yes,
            false => Muted::No,
        };
        let Ok(at) = stream.level();
        let Ok(level) = volume(at, muted);
        let Ok(said) = stream.said();

        let Ok(silence) = hush(stream.index, "sink-input");
        let Ok(row) = Row::new(&said, &level, silence);
        let Ok(turning) = turn(stream.index, "sink-input");
        let Ok(levelled) = row.levelled(turning);

        rows.push(levelled);
    }

    match playing.is_empty() {
        true => {
            let Ok(row) = Row::nothing("Nothing else is playing");

            rows.push(row);
        }
        false => {},
    }

    Ok(rows)
}

pub fn warmth(warm: Warmth) -> Result<Row, Never> {
    let says = match warm {
        Warmth::Following => Word::NightColoursOff,
        Warmth::Ordinary => Word::NightColoursOn,
    };
    let Ok(said_says) = say(&says);
    let Ok(mut row) = switch(&said_says, &["/usr/local/bin/console-warm"]);
    row.aside = match warm {
        Warmth::Following => {
            let Ok(on) = say(&Word::On);

            on
        },
        Warmth::Ordinary => {
            let Ok(off) = say(&Word::Off);

            off
        },
    };

    Ok(row)
}

pub fn threshold(level: i32) -> Result<String, Never> {
    Ok(match level {
        battery::NEVER => {
            let Ok(never) = say(&Word::Never);

            never
        },
        level => format!("{level}%"),
    })
}

pub fn dwindling(
    levels: battery::Levels,
    guard: impl Fn(battery::Step) -> Result<Level, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(when_the_battery_gets_low) = say(&Word::WhenTheBatteryGetsLow);
    let Ok(naming) = Row::naming(&when_the_battery_gets_low, "");
    let mut rows = vec![naming];

    rows.extend(battery::EVERY.into_iter().map(|step| {
        let Ok(word) = said_of(step);
        let Ok(level) = levels.at(step);
        let Ok(at) = threshold(level);
        let Ok(said_word) = say(&word);
        let Ok(row) = Row::said(&said_word, &at);
        let Ok(guarding) = guard(step);
        let Ok(levelled) = row.levelled(guarding);

        levelled
    }));

    Ok(rows)
}

fn said_of(step: battery::Step) -> Result<Word, Never> {
    Ok(match step {
        battery::Step::Low => Word::WarnMe,
        battery::Step::Lower => Word::WarnMeAgain,
        battery::Step::Protect => Word::TurnOffBeforeItDies,
    })
}

pub fn home_rows(
    shape: Shape,
    across: Level,
    down: Level,
    sized: Level,
) -> Result<Vec<Row>, Never> {
    let Ok(the_home_screen) = say(&Word::TheHomeScreen);
    let Ok(naming) = Row::naming(&the_home_screen, "");
    let Ok(applications_across) = say(&Word::ApplicationsAcross);
    let Ok(columns) = Row::said(&applications_across, &shape.columns.to_string());
    let Ok(columns) = columns.levelled(across);
    let Ok(applications_down) = say(&Word::ApplicationsDown);
    let Ok(down_row) = Row::said(&applications_down, &shape.rows.to_string());
    let Ok(down_row) = down_row.levelled(down);
    let Ok(word) = said_of_home_size(shape.size);
    let Ok(how_big_they_are) = say(&Word::HowBigTheyAre);
    let Ok(said_word) = say(&word);
    let Ok(big) = Row::said(&how_big_they_are, &said_word);
    let Ok(big) = big.levelled(sized);

    Ok(vec![naming, columns, down_row, big])
}

fn said_of_home_size(size: console_home_screen::shape::Size) -> Result<Word, Never> {
    Ok(match size {
        console_home_screen::shape::Size::Tiny => Word::SizeTiny,
        console_home_screen::shape::Size::Smaller => Word::SizeSmaller,
        console_home_screen::shape::Size::Normal => Word::SizeNormal,
        console_home_screen::shape::Size::Bigger => Word::SizeBigger,
        console_home_screen::shape::Size::Huge => Word::SizeHuge,
    })
}

pub fn screen_rows(
    brightness: Option<i32>,
    dim: Level,
    warm: Warmth,
    standing: Option<Size>,
    home: Vec<Row>,
) -> Result<Vec<Row>, Never> {
    let level = match brightness {
        Some(level) => volume(level, Muted::No)?,
        None => YET.to_string(),
    };
    let Ok(screen_brightness) = say(&Word::ScreenBrightness);
    let Ok(bright) = Row::said(&screen_brightness, &level);
    let Ok(bright) = bright.levelled(dim);
    let Ok(warmth) = warmth(warm);
    let Ok(how_big_everything_is) = say(&Word::HowBigEverythingIs);
    let Ok(naming) = Row::naming(&how_big_everything_is, "");
    let mut rows = vec![bright, warmth, naming];

    rows.extend(EVERY.into_iter().map(|size| {
        let Ok(word) = said_of_size(size);
        let Ok(said_word) = say(&word);
        let Ok(written) = size.written();
        let Ok(mut row) = switch(&said_word, &["/usr/local/bin/console-scale", written]);

        row.aside = match standing == Some(size) {
            true => NOW.to_string(),
            false => String::new(),
        };

        row
    }));
    rows.extend(home);

    Ok(rows)
}

fn said_of_size(size: Size) -> Result<Word, Never> {
    Ok(match size {
        Size::Tiny => Word::SizeTiny,
        Size::Smaller => Word::SizeSmaller,
        Size::Normal => Word::SizeNormal,
        Size::Bigger => Word::SizeBigger,
        Size::Huge => Word::SizeHuge,
    })
}

pub fn battery_rows(
    running: Option<&str>,
    levels: battery::Levels,
    guard: impl Fn(battery::Step) -> Result<Level, Never>,
) -> Result<Vec<Row>, Never> {
    let profile = |says: &str, name: &'static str| {
        let mark = match running {
            Some(running) if running == name => NOW,
            Some(_) | None => "",
        };
        let Ok(powerprofilesctl) = Program::Powerprofilesctl.name();
        let Ok(sets) = Does::run(&[powerprofilesctl, "set", name]);
        let Ok(row) = Row::new(says, mark, sets);

        row
    };
    let Ok(how_fast_the_machine_runs) = say(&Word::HowFastTheMachineRuns);
    let Ok(naming) = Row::naming(&how_fast_the_machine_runs, "");
    let Ok(speed_saving) = say(&Word::SpeedSaving);
    let Ok(speed_normal) = say(&Word::SpeedNormal);
    let Ok(speed_fast) = say(&Word::SpeedFast);
    let mut rows = vec![
        naming,
        profile(&speed_saving, "power-saver"),
        profile(&speed_normal, "balanced"),
        profile(&speed_fast, "performance"),
    ];
    let Ok(dwindling) = dwindling(levels, guard);

    rows.extend(dwindling);

    Ok(rows)
}

pub fn strength(signal: i32) -> Result<String, Never> {
    bar(signal, Muted::No, CELLS.saturating_div(2))
}

pub fn wifi_rows(
    on: wifi::Radio,
    networks: Vec<wifi::Network>,
    known: &[String],
    join: impl Fn(wifi::Network, wifi::Known) -> Result<Does, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(nmcli) = Program::Nmcli.name();

    match on {
        wifi::Radio::Off => {
            let Ok(row) = switch("Turn Wi-Fi on", &[nmcli, "radio", "wifi", "on"]);

            return Ok(vec![row]);
        }
        wifi::Radio::On => {},
    }

    let Ok(off) = switch("Turn Wi-Fi off", &[nmcli, "radio", "wifi", "off"]);
    let mut rows = vec![off];

    for network in networks {
        match network.here {
            true => {
                let Ok(row) = Row::said(&network.name, NOW);

                rows.push(row);
                continue;
            }
            false => {},
        }

        let Ok(aside) = strength(network.signal);
        let says = network.name.clone();
        let already = match known.contains(&network.name) {
            true => wifi::Known::Yes,
            false => wifi::Known::No,
        };
        let Ok(joining) = join(network, already);
        let Ok(row) = Row::new(&says, &aside, joining);

        rows.push(row);
    }

    Ok(rows)
}

pub type Opens = Arc<dyn Fn(&bluetooth::Met, usize, &dyn Showing) + Send + Sync>;

const RADIO: usize = 1;

const INTRODUCES: &str = "console-bluetooth";

const PAIR: &str = "Pair with";

const FORGET: &str = "Forget this device";

const FORGET_SURE: &str = "Forget this device?";

const FORGET_YES: &str = "Forget it";

const JOIN: &str = "Connect";

const LEAVE: &str = "Disconnect";

pub const LOOK: &str = "Look for devices";

pub const LOOKING: &str = "Looking for devices";

pub const WHILE_YOU_LOOK: &str = "600";

pub fn looking_words() -> Result<Vec<&'static str>, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    Ok(vec![bluetoothctl, "--timeout", WHILE_YOU_LOOK, "scan", "on"])
}

pub fn bluetooth_rows(
    on: bluetooth::Radio,
    looking: bluetooth::Looking,
    met: Vec<bluetooth::Met>,
    open: Opens,
) -> Result<Vec<Row>, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    match on {
        bluetooth::Radio::Off => {
            let Ok(row) = switch("Turn Bluetooth on", &[bluetoothctl, "power", "on"]);

            return Ok(vec![row]);
        }
        bluetooth::Radio::On => {},
    }

    let Ok(off) = switch("Turn Bluetooth off", &[bluetoothctl, "power", "off"]);
    let mut rows = vec![off];
    let Ok(met) = bluetooth::in_order(met);

    for (from, met) in met.into_iter().enumerate() {
        let at = from.saturating_add(RADIO);
        let Ok(row) = device_row(met, at, &open);

        rows.push(row);
    }

    let last = match looking {
        bluetooth::Looking::Yes => Row::said(LOOKING, YET),
        bluetooth::Looking::No => {
            let Ok(words) = looking_words();

            switch(LOOK, &words)
        }
    };
    let Ok(last) = last;

    rows.push(last);

    Ok(rows)
}

fn heard_aside(met: &bluetooth::Met) -> Result<String, Never> {
    let heard = match met.heard {
        Some(heard) => heard,
        None => return Ok(YET.to_string()),
    };
    let Ok(share) = bluetooth::share(heard);

    strength(share)
}

fn device_row(met: bluetooth::Met, at: usize, open: &Opens) -> Result<Row, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    match met.known {
        bluetooth::Known::No => {
            let opening = Arc::clone(open);
            let opened = met.clone();
            let Ok(opens) = Does::and_stay(move |showing| opening(&opened, at, showing));
            let Ok(aside) = heard_aside(&met);

            Row::new(&met.device.name, &aside, opens)
        }
        bluetooth::Known::Yes => {
            let doing = match met.joined {
                bluetooth::Joined::Yes => "disconnect",
                bluetooth::Joined::No => "connect",
            };
            let aside = match met.joined {
                bluetooth::Joined::Yes => NOW,
                bluetooth::Joined::No => "",
            };
            let Ok(mut row) = switch(&met.device.name, &[bluetoothctl, doing, &met.device.address]);

            row.aside = aside.to_string();

            let opening = Arc::clone(open);
            let opened = met.clone();

            row.offering(move |showing| {
                opening(&opened, at, showing);

                false
            })
        }
    }
}

pub fn meeting_rows(met: &bluetooth::Met, back: Chosen) -> Result<Vec<Row>, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();
    let leaving = Arc::clone(&back);
    let Ok(way_back) = Row::back(&met.device.name, move |showing| leaving(showing));
    let mut rows = vec![way_back];

    match met.known {
        bluetooth::Known::No => {
            let Ok(introduces) = introducing_words(&met.device.address);
            let words: Vec<&str> = introduces.iter().map(String::as_str).collect();
            let Ok(row) = switch(&format!("{PAIR} {}", met.device.name), &words);

            rows.push(row);
        }
        bluetooth::Known::Yes => {
            let says = match met.joined {
                bluetooth::Joined::Yes => LEAVE,
                bluetooth::Joined::No => JOIN,
            };
            let doing = match met.joined {
                bluetooth::Joined::Yes => "disconnect",
                bluetooth::Joined::No => "connect",
            };
            let Ok(row) = switch(says, &[bluetoothctl, doing, &met.device.address]);

            rows.push(row);

            let Ok(forget) = forget_row(&met.device, back);

            rows.push(forget);
        }
    }

    Ok(rows)
}

fn introducing_words(address: &str) -> Result<Vec<String>, Never> {
    Ok(vec![INTRODUCES.to_string(), introducing::INTRODUCE.to_string(), address.to_string()])
}

fn forget_row(device: &bluetooth::Device, back: Chosen) -> Result<Row, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();
    let argv = vec![bluetoothctl.to_string(), "remove".to_string(), device.address.to_string()];
    let name = device.name.clone();

    let Ok(forgets) = Does::and_stay(move |showing| {
        let argv = argv.clone();
        let back = Arc::clone(&back);

        showing.sure(
            FORGET_SURE,
            &name,
            &[FORGET_YES],
            Arc::new(move |showing, _| {
                showing.later(argv.clone());
                back(showing);
            }),
        );
    });

    Row::new(FORGET, "", forgets)
}

pub fn search_rows(engine: &str, back: Chosen) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(configuration) = configuration();
    let Ok(way_back) = Row::back(&configuration, move |showing| leaving(showing));
    let Ok(naming) = Row::naming("Search with", "");
    let mut rows = vec![way_back, naming];

    for offered in &engines::EVERY {
        let mark = match offered.key == engine {
            true => NOW,
            false => "",
        };
        let key = offered.key;
        let back = Arc::clone(&back);
        let Ok(chooses) = Does::and_stay(move |showing| {
            let Ok(()) = engines::choose(key);
            let Ok(telling) = telling(key);

            showing.later(telling);
            back(showing);
        });
        let Ok(row) = Row::new(offered.says, mark, chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn engine_says(engine: &str) -> Result<String, Never> {
    let known = engines::one(engine)?;

    Ok(known.map(|found| found.says.to_string()).unwrap_or_default())
}

pub fn dictation_rows(language: &str, back: Chosen) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(tab) = the_language();
    let Ok(way_back) = Row::back(&tab, move |showing| leaving(showing));
    let Ok(listens) = say(&Word::WhatItListensFor);
    let Ok(naming) = Row::naming(&listens, "");
    let mut rows = vec![way_back, naming];

    for offered in &languages::EVERY {
        let mark = match offered.key == language {
            true => NOW,
            false => "",
        };
        let key = offered.key;
        let back = Arc::clone(&back);
        let Ok(chooses) = Does::and_stay(move |showing| {
            let Ok(()) = languages::choose(key);

            back(showing);
        });
        let Ok(row) = Row::new(offered.says, mark, chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn dictation_says(language: &str) -> Result<String, Never> {
    let known = languages::one(language)?;

    Ok(known.map(|found| found.says.to_string()).unwrap_or_default())
}

pub fn telling(engine: &str) -> Result<Vec<String>, Never> {
    Program::Sudo.argv(&["-n", "console-engine", engine])
}

pub fn system_rows() -> Result<Vec<Row>, Never> {
    let Ok(game) = Does::run(&["/usr/local/bin/game-mode"]);
    let Ok(game) = Row::new("Game Mode", "", game);
    let Ok(naming) = Row::naming("Power", "");
    let Ok(systemctl) = Program::Systemctl.name();
    let Ok(suspends) = Does::run(&[systemctl, "suspend"]);
    let Ok(sleep) = Row::new("Sleep", "", suspends);
    let Ok(reboots) = Does::run(&[systemctl, "reboot"]);
    let Ok(restart) = Row::new("Restart", "", reboots);
    let Ok(powers_off) = Does::run(&[systemctl, "poweroff"]);
    let Ok(shut_down) = Row::new("Shut down", "", powers_off);

    Ok(vec![game, naming, sleep, restart, shut_down])
}

pub fn notifications_rows(held_back: Quiet) -> Result<Vec<Row>, Never> {
    let says = match held_back {
        Quiet::HeldBack => "Show them on the screen as they arrive",
        Quiet::Coming => "Keep them off the screen",
    };
    let mark = match held_back {
        Quiet::HeldBack => "held back",
        Quiet::Coming => "",
    };
    let Ok(makoctl) = Program::Makoctl.name();
    let Ok(mut row) = switch(says, &[makoctl, "mode", "-t", QUIET]);

    row.aside = mark.to_string();

    let Ok(bell) =
        Row::said("The bell on the bar", "Everything that arrived, held back or not");

    Ok(vec![row, bell])
}

pub fn tabs() -> Result<[String; 10], Never> {
    let Ok(configuration) = configuration();

    let Ok(sound) = say(&Word::Sound);
    let Ok(bluetooth) = say(&Word::Bluetooth);
    let Ok(wifi) = say(&Word::Wifi);
    let Ok(battery) = say(&Word::Battery);
    let Ok(notifications) = say(&Word::Notifications);
    let Ok(screen) = say(&Word::Screen);
    let Ok(wallpaper) = say(&Word::Wallpaper);
    let Ok(language) = say(&Word::Language);
    let Ok(system) = say(&Word::System);

    Ok([
        sound,
        bluetooth,
        wifi,
        battery,
        notifications,
        screen,
        wallpaper,
        language,
        configuration,
        system,
    ])
}

pub fn the_language() -> Result<String, Never> {
    say(&Word::Language)
}

pub fn where_you_are() -> Result<String, Never> {
    say(&Word::WhereYouAre)
}

pub fn the_clock() -> Result<String, Never> {
    say(&Word::TheClock)
}

pub fn configuration() -> Result<String, Never> {
    say(&Word::Configuration)
}


pub fn language_rows(says: &str, types: &str, listens: &str, into: [Does; 3]) -> Result<Vec<Row>, Never> {
    let [words, keyboard, dictation] = into;

    let Ok(what_says) = say(&Word::WhatThisMachineSays);
    let Ok(what_types) = say(&Word::WhatTheKeyboardTypes);
    let Ok(what_listens) = say(&Word::WhatItListensFor);

    let Ok(row) = Row::new(&what_says, says, words);
    let Ok(said) = row.opening();
    let Ok(row) = Row::new(&what_types, types, keyboard);
    let Ok(typed) = row.opening();
    let Ok(row) = Row::new(&what_listens, listens, dictation);
    let Ok(heard) = row.opening();

    let Ok(english) = say(&Word::TheDesktopStaysInEnglish);
    let Ok(standing) = Row::nothing(&english);

    Ok(vec![said, typed, heard, standing])
}

fn leading(says: &str, back: Chosen) -> Result<Vec<Row>, Never> {
    let Ok(tab) = the_language();
    let Ok(way_back) = Row::back(&tab, move |showing| back(showing));
    let Ok(naming) = Row::naming(says, "");

    Ok(vec![way_back, naming])
}

fn running(argv: Vec<String>, note: Option<String>, back: Chosen) -> Result<Does, Never> {
    Does::and_stay(move |showing| {
        match &note {
            Some(said) => showing.note(said),
            None => {},
        }

        showing.later(argv.clone());

        back(showing);
    })
}

pub fn tongue_rows(
    tongues: &[Tongue],
    standing: Option<&Locale>,
    back: Chosen,
    opening: impl Fn(usize, &str) -> Result<Does, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(what_says) = say(&Word::WhatThisMachineSays);
    let Ok(mut rows) = leading(&what_says, back);

    let first = rows.len();
    let here = standing.map(|locale| locale.language.clone()).unwrap_or_default();

    for (at, tongue) in tongues.iter().enumerate() {
        let mark = match tongue.language == here {
            true => NOW,
            false => "",
        };
        let Ok(opens) = opening(at.saturating_add(first), &tongue.language);
        let Ok(row) = Row::new(&tongue.says, mark, opens);
        let Ok(row) = row.opening();

        rows.push(row);
    }

    Ok(rows)
}

pub fn place_rows(
    tongue: &Tongue,
    names: &Names,
    generated: &[String],
    standing: Option<&Locale>,
    back: Chosen,
) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(where_) = say(&Word::WhereItIsSpoken);
    let Ok(mut rows) = leading(&where_, leaving);

    let here = standing.map(|locale| locale.name.clone()).unwrap_or_default();

    for locale in &tongue.locales {
        let mark = match locale.name == here {
            true => NOW.to_string(),
            false => String::new(),
        };
        let Ok(where_) = names.where_(locale);

        let says = match where_.is_empty() {
            true => tongue.says.clone(),
            false => where_,
        };

        let Ok(made) = made(generated, locale);

        let note = match made {
            Made::Yes => None,
            Made::No => {
                let Ok(making) = say(&Word::MakingTheLanguage);

                Some(making)
            }
        };

        let Ok(argv) = Program::Sudo.argv(&[
            "-n",
            "console-machine",
            "language",
            &locale.name,
            &locale.charset,
        ]);
        let Ok(chooses) = running(argv, note, Arc::clone(&back));
        let Ok(row) = Row::new(&says, &mark, chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn alphabet_rows(chosen: &[&'static Alphabet], back: Chosen) -> Result<Vec<Row>, Never> {
    let Ok(what_types) = say(&Word::WhatTheKeyboardTypes);
    let Ok(mut rows) = leading(&what_types, back);

    for alphabet in &alphabets::EVERY {
        let held = chosen.iter().any(|already| already.key == alphabet.key);

        let mark = match held {
            true => NOW,
            false => "",
        };

        match alphabet.key == alphabets::LATIN {
            true => {
                let Ok(row) = Row::said(alphabet.says, mark);

                rows.push(row);
            }
            false => {
                let Ok(turned) = alphabets::turned(chosen, alphabet.key);
                let Ok(words) = Program::Systemctl.argv(&["--user", "restart", KEYBOARD]);
                let Ok(turns) = Does::and_stay(move |showing| {
                    let Ok(()) = alphabets::choose(&turned);

                    showing.later(words.clone());
                    showing.later(vec![
                        console_input_language::NAMED.to_string(),
                        console_input_language::SETTLE.to_string(),
                    ]);
                    showing.refresh();
                });
                let Ok(row) = Row::new(alphabet.says, mark, turns);

                rows.push(row);
            }
        }
    }

    Ok(rows)
}

pub fn region_rows(
    regions: &[String],
    standing: Option<&str>,
    back: Chosen,
    opening: impl Fn(usize, &str) -> Result<Does, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(where_) = say(&Word::WhereYouAre);
    let Ok(mut rows) = leading(&where_, back);

    let first = rows.len();
    let Ok(here) = hours::part_of(standing.unwrap_or_default());

    for (at, region) in regions.iter().enumerate() {
        let mark = match *region == here {
            true => NOW,
            false => "",
        };
        let Ok(opens) = opening(at.saturating_add(first), region);
        let Ok(row) = Row::new(region, mark, opens);
        let Ok(row) = row.opening();

        rows.push(row);
    }

    Ok(rows)
}

pub fn zone_rows(
    region: &str,
    places: &[Place],
    standing: Option<&str>,
    back: Chosen,
) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(mut rows) = leading(region, leaving);

    let here = standing.unwrap_or_default();

    for place in places {
        let mark = match place.zone == here {
            true => NOW,
            false => "",
        };
        let Ok(argv) = Program::Sudo.argv(&["-n", "console-machine", "hour", &place.zone]);
        let Ok(chooses) = running(argv, None, Arc::clone(&back));
        let Ok(row) = Row::new(&place.says, mark, chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn clock_rows(now: Clock, back: Chosen) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(the_clock) = say(&Word::TheClock);
    let Ok(mut rows) = leading(&the_clock, leaving);

    for reading in clock::EVERY {
        let mark = match reading == now {
            true => NOW,
            false => "",
        };
        let Ok(says) = reading.says();
        let leaving = Arc::clone(&back);
        let Ok(chooses) = Does::and_stay(move |showing| {
            let Ok(()) = clock::choose(reading);

            leaving(showing);
        });
        let Ok(row) = Row::new(says, mark, chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn called_row(name: &str) -> Result<Row, Never> {
    let Ok(says) = say(&Word::WhatThisMachineIsCalled);
    let asking = says.clone();
    let Ok(asks) = Does::and_stay(move |showing| {
        showing.ask(
            &asking,
            Arc::new(move |showing, word| {
                let Ok(allowed) = named::allowed(word);

                match allowed {
                    Allowed::Yes => {
                        let Ok(argv) =
                            Program::Sudo.argv(&["-n", "console-machine", "name", word]);

                        showing.later(argv);
                    }
                    Allowed::No => showing.note(NOT_A_NAME),
                }
            }),
        );
    });

    Row::new(&says, name, asks)
}

const NOT_A_NAME: &str = "Letters, digits and hyphens only";

const KEYBOARD: &str = "console-input-keyboard.service";

pub type Chosen = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

#[cfg(test)]
mod tests {
    use console_panel::page::{Heading, InEffect};
    use super::*;

    fn said(word: &Word) -> String {
        let Ok(said) = say(word);

        said
    }

    fn nothing() -> Level {
        std::sync::Arc::new(|_| ())
    }

    fn grid() -> Vec<Row> {
        let Ok(rows) = home_rows(Shape::USUAL, nothing(), nothing(), nothing());

        rows
    }

    fn silence(_: i64, _: &'static str) -> Result<Does, Never> {
        Does::and_stay(|_| ())
    }

    fn now(row: &Row) -> InEffect {
        let Ok(now) = row.now();

        now
    }

    fn heading(row: &Row) -> Heading {
        let Ok(heading) = row.heading();

        heading
    }

    fn marked(rows: &[Row]) -> Vec<&str> {
        rows.iter()
            .filter(|row| now(row) == InEffect::Yes)
            .map(|row| row.says.as_str())
            .collect()
    }

    fn configured() -> String {
        let Ok(configuration) = configuration();

        configuration
    }

    fn sound(sinks: &[sound::Thing], playing: &[sound::Thing], default: &str) -> Vec<Row> {
        let Ok(rows) = sound_rows(sinks, playing, default, silence, turning);

        rows
    }

    fn wifi_of(on: wifi::Radio, networks: Vec<wifi::Network>) -> Vec<Row> {
        let Ok(rows) = wifi_rows(on, networks, &[], |_, _| silence(0, ""));

        rows
    }

    fn bluetooth_of(on: bluetooth::Radio, met: Vec<bluetooth::Met>) -> Vec<Row> {
        let Ok(rows) = bluetooth_rows(on, bluetooth::Looking::No, met, Arc::new(|_, _, _| ()));

        rows
    }

    fn while_looking(met: Vec<bluetooth::Met>) -> Vec<Row> {
        let Ok(rows) = bluetooth_rows(
            bluetooth::Radio::On,
            bluetooth::Looking::Yes,
            met,
            Arc::new(|_, _, _| ()),
        );

        rows
    }

    fn met(
        device: &bluetooth::Device,
        known: bluetooth::Known,
        joined: bluetooth::Joined,
    ) -> bluetooth::Met {
        bluetooth::Met { device: device.clone(), known, joined, heard: None }
    }

    fn heard(device: &bluetooth::Device, heard: i32) -> bluetooth::Met {
        bluetooth::Met {
            device: device.clone(),
            known: bluetooth::Known::No,
            joined: bluetooth::Joined::No,
            heard: Some(heard),
        }
    }

    fn back_to(says: &str) -> String {
        let Ok(row) = Row::back(says, |_| ());

        row.says
    }

    fn meeting(met: &bluetooth::Met) -> Vec<Row> {
        let Ok(rows) = meeting_rows(met, nowhere());

        rows
    }

    fn searching(engine: &str) -> Vec<Row> {
        let Ok(rows) = search_rows(engine, nowhere());

        rows
    }

    fn listening(language: &str) -> Vec<Row> {
        let Ok(rows) = dictation_rows(language, nowhere());

        rows
    }

    fn named() -> [String; 10] {
        let Ok(tabs) = tabs();

        tabs
    }

    fn sized(standing: Option<Size>) -> Vec<Row> {
        let Ok(rows) =
            screen_rows(Some(50), nothing(), Warmth::Ordinary, standing, grid());

        rows
    }

    fn nowhere() -> Chosen {
        Arc::new(|_: &dyn Showing| ())
    }

    struct Nowhere;

    impl Showing for Nowhere {
        fn refresh(&self) {}
        fn replace(&self, _standing_on: usize) {}
        fn forget_typing(&self) {}
        fn ask(&self, _question: &str, _then: console_panel::page::Answer) {}
        fn sure(
            &self,
            _question: &str,
            _about: &str,
            _does: &[&str],
            _then: console_panel::page::Taken,
        ) {
        }
        fn ask_aloud(&self, _question: &str, _then: console_panel::page::Answer) {}
        fn note(&self, _said: &str) {}
        fn later(&self, _argv: Vec<String>) {}
        fn leave_running(&self, _argv: Vec<String>) {}
        fn open_out(&self) {}
        fn turn_to(&self, _tab: usize) {}
    }

    fn turning(_: i64, _: &'static str) -> Result<Level, Never> {
        Ok(nothing())
    }

    fn says(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.says.as_str()).collect()
    }

    fn screen() -> Vec<Row> {
        sized(Some(Size::Normal))
    }

    fn battery() -> Vec<Row> {
        let Ok(rows) = battery_rows(Some("balanced"), battery::Levels::default(), |_| Ok(nothing()));

        rows
    }

    #[test]
    fn the_two_things_that_are_held_at_a_level_are_on_a_panel() {
        assert!(screen()[0].level.is_some(), "the screen is not a level");
        let Ok(sinks) = sound::read(r#"[{"index": 1, "name": "a", "volume": {}}]"#);
        let speakers = sound(&sinks, &[], "a");
        assert!(speakers[0].level.is_some(), "the speakers are not a level");
    }

    #[test]
    fn the_profile_in_use_is_the_one_marked() {
        let Ok(rows) =
            battery_rows(Some("performance"), battery::Levels::default(), |_| Ok(nothing()));
        assert_eq!(marked(&rows), [said(&Word::SpeedFast)]);
    }

    #[test]
    fn the_three_speeds_are_a_named_scale_with_the_least_of_them_first() {
        let rows = battery();
        assert!(rows[0].naming, "the speeds are not named");
        assert_eq!(rows[0].says, said(&Word::HowFastTheMachineRuns));
        assert_eq!(
            says(&rows[1..4]),
            [said(&Word::SpeedSaving), said(&Word::SpeedNormal), said(&Word::SpeedFast)]
        );
    }

    #[test]
    fn the_screen_and_how_hard_the_machine_works_are_two_tabs() {
        let battery = says(&battery()).join("\n");
        for screen in [said(&Word::ScreenBrightness), said(&Word::HowBigEverythingIs)] {
            assert!(!battery.contains(&screen), "{screen:?} is still on the Battery tab");
        }
        assert!(!battery.contains("night colours"), "the evening is still on the Battery tab");
    }

    #[test]
    fn the_sizes_are_a_named_scale_with_the_smallest_of_them_first() {
        let rows = screen();
        let at = rows
            .iter()
            .position(|row| row.says == said(&Word::HowBigEverythingIs))
            .expect("the sizes are named");
        assert!(rows[at].naming, "the name is a row the highlight can land on");
        assert_eq!(
            says(&rows[at + 1..at + 1 + EVERY.len()]),
            [
                said(&Word::SizeTiny),
                said(&Word::SizeSmaller),
                said(&Word::SizeNormal),
                said(&Word::SizeBigger),
                said(&Word::SizeHuge),
            ]
        );
        assert!(
            at > rows.iter().position(|row| row.level.is_some()).expect("the brightness"),
            "the brightness is above the name, not under it"
        );
    }

    #[test]
    fn the_home_screens_own_shape_is_under_the_size_of_everything_else() {
        let rows = screen();
        let named = rows
            .iter()
            .position(|row| row.says == said(&Word::TheHomeScreen))
            .expect("the home screen is named");
        let ladder = rows
            .iter()
            .position(|row| row.says == said(&Word::HowBigEverythingIs))
            .expect("the sizes are named");

        assert!(named > ladder, "the home screen is under the ladder, not over it");
        assert!(rows[named].naming, "the name is a row the highlight can land on");
        assert_eq!(
            says(&rows[named + 1..]),
            [
                said(&Word::ApplicationsAcross),
                said(&Word::ApplicationsDown),
                said(&Word::HowBigTheyAre),
            ]
        );
    }

    #[test]
    fn the_home_screens_rows_say_what_they_are_at_and_can_all_be_moved() {
        let Ok(wide) = Shape::USUAL.across(7);

        let Ok(deep) = wide.down(4);

        let Ok(shape) = deep.sized(console_home_screen::shape::Size::Bigger);

        let Ok(rows) = home_rows(shape, nothing(), nothing(), nothing());
        let moved: Vec<&Row> = rows.iter().filter(|row| row.level.is_some()).collect();

        assert_eq!(moved.len(), 3, "one of them cannot be moved");
        assert!(moved.iter().all(|row| !row.aside.is_empty()), "one of them says nothing");
        assert_eq!(moved[0].aside, "7");
        assert_eq!(moved[1].aside, "4");
        assert_eq!(moved[2].aside, said(&Word::SizeBigger));
    }

    #[test]
    fn the_size_the_screen_is_at_is_the_one_marked() {
        let rows = sized(Some(Size::Bigger));
        assert_eq!(marked(&rows), [said(&Word::SizeBigger)]);
    }

    #[test]
    fn a_screen_at_a_size_of_its_own_marks_none_of_the_rungs() {
        let rows = sized(None);
        assert!(
            !rows.iter().any(|row| now(row) == InEffect::Yes),
            "something is marked"
        );
        let pressable = rows.iter().filter(|row| row.does.is_some()).count();
        assert_eq!(pressable, EVERY.len() + 1, "a rung went missing");
    }

    #[test]
    fn where_the_battery_is_watched_is_three_rows_that_can_be_moved() {
        let Ok(rows) = dwindling(battery::Levels::default(), |_| Ok(nothing()));
        assert_eq!(
            says(&rows),
            [
                said(&Word::WhenTheBatteryGetsLow),
                said(&Word::WarnMe),
                said(&Word::WarnMeAgain),
                said(&Word::TurnOffBeforeItDies),
            ]
        );
        assert!(rows[1..].iter().all(|row| row.level.is_some()), "a threshold that cannot be moved");
        assert_eq!(rows[1].aside, "25%");
    }

    #[test]
    fn a_threshold_turned_off_says_so_in_a_word() {
        assert_eq!(threshold(battery::NEVER), Ok(said(&Word::Never)));
        assert_eq!(threshold(5), Ok("5%".to_string()));
    }

    #[test]
    fn a_radio_that_is_looking_says_so_where_the_press_that_starts_it_was() {
        let looking = while_looking(Vec::new());
        let last = looking.last().expect("a last row");

        assert_eq!(last.says, LOOKING);
        assert!(last.does.is_none(), "a row saying it is looking is not a row to press");

        let idle = bluetooth_of(bluetooth::Radio::On, Vec::new());

        assert_eq!(idle.last().expect("a last row").says, LOOK);
    }

    #[test]
    fn the_loudest_stranger_is_the_nearest_row_to_the_thumb() {
        let Ok(devices) = bluetooth::devices("Device AA Far\nDevice BB Near\nDevice CC Middling");
        let rows = while_looking(vec![
            heard(&devices[0], -95),
            heard(&devices[1], -40),
            heard(&devices[2], -70),
        ]);

        assert_eq!(says(&rows), ["Turn Bluetooth off", "Near", "Middling", "Far", LOOKING]);
    }

    #[test]
    fn a_device_that_only_said_its_address_is_drawn_under_the_ones_that_said_a_name() {
        let Ok(devices) = bluetooth::devices(
            "Device AA:BB:CC:DD:EE:FF AA-BB-CC-DD-EE-FF\nDevice 11:22:33:44:55:66 Blue Keys",
        );
        let rows = while_looking(vec![heard(&devices[0], -40), heard(&devices[1], -95)]);

        assert_eq!(says(&rows), [
            "Turn Bluetooth off",
            "Blue Keys",
            "AA-BB-CC-DD-EE-FF",
            LOOKING
        ]);
    }

    #[test]
    fn what_a_stranger_says_beside_it_is_how_loud_it_is() {
        let Ok(devices) = bluetooth::devices("Device AA Near\nDevice BB Quiet");
        let rows = while_looking(vec![heard(&devices[0], -50), met(
            &devices[1],
            bluetooth::Known::No,
            bluetooth::Joined::No,
        )]);
        let near = rows.iter().find(|row| row.says == "Near").expect("near");
        let quiet = rows.iter().find(|row| row.says == "Quiet").expect("quiet");

        assert_eq!(near.aside, strength(100).unwrap_or_default());
        assert_eq!(quiet.aside, YET, "a device the radio has heard nothing from says nothing");
    }

    #[test]
    fn a_radio_that_is_off_still_has_a_row_to_turn_it_on() {
        assert_eq!(says(&wifi_of(wifi::Radio::Off, Vec::new())), ["Turn Wi-Fi on"]);
        assert_eq!(
            says(&bluetooth_of(bluetooth::Radio::Off, Vec::new())),
            ["Turn Bluetooth on"]
        );
    }

    #[test]
    fn the_one_we_are_on_is_marked_rather_than_offered() {
        let Ok(networks) = wifi::networks("yes:Home:71:WPA2\nno:Cafe:50:");
        let rows = wifi_of(wifi::Radio::On, networks);
        let home = rows.iter().find(|row| row.says == "Home").expect("home");
        assert_eq!(now(home), InEffect::Yes);
        assert!(home.does.is_none(), "there is nothing to do about being where you are");
        let cafe = rows.iter().find(|row| row.says == "Cafe").expect("cafe");
        assert!(cafe.does.is_some());
    }

    #[test]
    fn a_joined_device_is_marked_and_offers_the_way_out_of_it() {
        let Ok(devices) = bluetooth::devices("Device AA Pads\nDevice BB Speaker");
        let rows = bluetooth_of(bluetooth::Radio::On, vec![
            met(&devices[0], bluetooth::Known::Yes, bluetooth::Joined::Yes),
            met(&devices[1], bluetooth::Known::Yes, bluetooth::Joined::No),
        ]);
        assert_eq!(now(rows.iter().find(|row| row.says == "Pads").expect("pads")), InEffect::Yes);
        assert_eq!(
            now(rows.iter().find(|row| row.says == "Speaker").expect("speaker")),
            InEffect::No
        );
    }

    #[test]
    fn a_device_nobody_has_been_introduced_to_is_not_offered_a_word_bluez_would_refuse() {
        let Ok(devices) = bluetooth::devices("Device AA Blue Keys");
        let rows = bluetooth_of(bluetooth::Radio::On, vec![met(
            &devices[0],
            bluetooth::Known::No,
            bluetooth::Joined::No,
        )]);
        let stranger = rows.iter().find(|row| row.says == "Blue Keys").expect("the keyboard");

        assert!(stranger.does.is_some(), "a stranger has to be pressable to become known");
        assert_eq!(stranger.aside, YET, "and has to read as somewhere still to go");
    }

    #[test]
    fn a_stranger_is_offered_the_pairing_and_a_friend_the_road_and_the_way_out() {
        let Ok(devices) = bluetooth::devices("Device AA Blue Keys");

        assert_eq!(
            says(&meeting(&met(&devices[0], bluetooth::Known::No, bluetooth::Joined::No))),
            [&back_to("Blue Keys"), "Pair with Blue Keys"]
        );
        assert_eq!(
            says(&meeting(&met(&devices[0], bluetooth::Known::Yes, bluetooth::Joined::No))),
            [&back_to("Blue Keys"), JOIN, FORGET]
        );
        assert_eq!(
            says(&meeting(&met(&devices[0], bluetooth::Known::Yes, bluetooth::Joined::Yes))),
            [&back_to("Blue Keys"), LEAVE, FORGET]
        );
    }

    #[test]
    fn every_device_row_is_the_row_its_own_page_comes_back_to() {
        let Ok(devices) = bluetooth::devices("Device AA One\nDevice BB Two\nDevice CC Three");
        let seen: Arc<std::sync::Mutex<Vec<usize>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let telling = Arc::clone(&seen);
        let Ok(rows) = bluetooth_rows(
            bluetooth::Radio::On,
            bluetooth::Looking::No,
            devices
                .iter()
                .map(|device| met(device, bluetooth::Known::No, bluetooth::Joined::No))
                .collect(),
            Arc::new(move |_, at, _| match telling.lock() {
                Ok(mut seen) => seen.push(at),
                Err(_) => {},
            }),
        );

        for row in &rows {
            match &row.does {
                Some(Does::Call(act)) => {
                    let _ = act(&Nowhere);
                }
                Some(Does::Run(_)) | None => {},
            }
        }

        let seen = match seen.lock() {
            Ok(seen) => seen.clone(),
            Err(_) => Vec::new(),
        };
        let standing: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| devices.iter().any(|device| device.name == row.says))
            .map(|(at, _)| at)
            .collect();

        assert_eq!(seen, standing);
    }

    #[test]
    fn nothing_playing_is_said_rather_than_left_blank() {
        assert_eq!(says(&sound(&[], &[], "")), ["Nothing else is playing"]);
    }

    #[test]
    fn the_engine_in_use_is_the_one_marked() {
        let rows = searching("startpage");
        assert_eq!(marked(&rows), ["Startpage"]);
    }

    #[test]
    fn the_browsers_are_told_without_stopping_to_ask_for_a_password() {
        let Ok(telling) = telling("startpage");
        let Ok(sudo) = Program::Sudo.name();

        assert_eq!(telling, [sudo, "-n", "console-engine", "startpage"]);
    }

    #[test]
    fn the_list_is_the_way_back_and_then_a_row_that_only_reads() {
        let rows = searching("duckduckgo");
        assert!(rows[0].says.ends_with(&configured()), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Search with");
        assert_eq!(heading(&rows[1]), Heading::Yes);
    }

    #[test]
    fn the_engine_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(engine_says("startpage"), Ok("Startpage".to_string()));
        assert_eq!(engine_says("telepathy"), Ok(String::new()));
    }

    #[test]
    fn the_language_being_listened_for_is_the_one_marked() {
        let rows = listening("nl");
        assert_eq!(marked(&rows), ["Dutch"]);
    }

    #[test]
    fn chinese_is_not_on_the_list() {
        let rows = listening("auto");
        assert!(!says(&rows).contains(&"Chinese"));
        assert_eq!(says(&rows)[2..], ["Whichever is spoken", "English", "Dutch", "Thai"]);
    }

    #[test]
    fn the_languages_are_a_list_under_the_tab_like_the_engines() {
        let rows = listening("auto");
        let Ok(tab) = the_language();
        let Ok(listens) = say(&Word::WhatItListensFor);

        assert!(rows[0].says.ends_with(&tab), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, listens);
        assert_eq!(heading(&rows[1]), Heading::Yes);
    }

    #[test]
    fn the_language_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(dictation_says("th"), Ok("Thai".to_string()));
        assert_eq!(dictation_says("auto"), Ok("Whichever is spoken".to_string()));
        assert_eq!(dictation_says("zh"), Ok(String::new()));
    }


    fn spoken() -> Vec<Tongue> {
        let Ok(names) = Names::none();
        let Ok(supported) = crate::tongues::supported(
            "en_GB.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\nnl_NL.UTF-8 UTF-8\nth_TH.UTF-8 UTF-8",
        );
        let Ok(spoken) = crate::tongues::tongues(&supported, &names);

        spoken
    }

    type Seen = Arc<std::sync::Mutex<Vec<(usize, String)>>>;

    fn opening(_: usize, _: &str) -> Result<Does, Never> {
        Does::and_stay(|_| ())
    }

    fn watching() -> (Seen, impl Fn(usize, &str) -> Result<Does, Never>) {
        let seen: Seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let telling = Arc::clone(&seen);

        (seen, move |at, name| {
            match telling.lock() {
                Ok(mut telling) => telling.push((at, name.to_string())),
                Err(_the_test_that_held_it_failed) => {},
            }

            Does::and_stay(|_| ())
        })
    }

    fn opened(seen: &Seen) -> Vec<(usize, String)> {
        match seen.lock() {
            Ok(mut seen) => std::mem::take(&mut seen),
            Err(_the_test_that_held_it_failed) => Vec::new(),
        }
    }

    fn language() -> Vec<Row> {
        let Ok(nothing) = Does::and_stay(|_| ());
        let Ok(also) = Does::and_stay(|_| ());
        let Ok(third) = Does::and_stay(|_| ());
        let Ok(rows) = language_rows("English (United Kingdom)", "Latin, Thai", "Dutch", [
            nothing, also, third,
        ]);

        rows
    }

    #[test]
    fn the_language_tab_is_three_rows_that_open_and_one_that_says_what_it_does_not_do() {
        let rows = language();
        let Ok(words) = say(&Word::WhatThisMachineSays);
        let Ok(types) = say(&Word::WhatTheKeyboardTypes);
        let Ok(listens) = say(&Word::WhatItListensFor);

        assert_eq!(says(&rows)[..3], [words.as_str(), types.as_str(), listens.as_str()]);

        for row in rows.iter().take(3) {
            assert_eq!(row.acts(), Ok(console_panel::page::Acts::Yes), "{:?}", row.says);
        }

        let Ok(english) = say(&Word::TheDesktopStaysInEnglish);

        assert_eq!(rows.last().map(|row| row.says.clone()), Some(english));
    }

    #[test]
    fn each_of_the_three_says_what_it_is_now_beside_it() {
        let rows = language();
        let asides: Vec<&str> = rows.iter().map(|row| row.aside.as_str()).collect();

        assert_eq!(asides[..3], ["English (United Kingdom)", "Latin, Thai", "Dutch"]);
    }

    #[test]
    fn the_languages_are_the_way_back_a_heading_and_then_every_language() {
        let Ok(rows) = tongue_rows(&spoken(), None, nowhere(), opening);
        let Ok(tab) = the_language();

        assert!(rows[0].says.ends_with(&tab), "{:?} is not the way back", rows[0].says);
        assert_eq!(heading(&rows[1]), Heading::Yes);
        assert_eq!(says(&rows)[2..], ["en", "nl", "th"]);
    }

    #[test]
    fn every_language_carries_the_row_it_was_opened_from_so_b_lands_back_on_it() {
        let (seen, opening) = watching();
        let Ok(rows) = tongue_rows(&spoken(), None, nowhere(), opening);
        let at = opened(&seen);

        for (which, standing) in at.iter().enumerate() {
            let says = rows.get(standing.0).map(|row| row.says.as_str());

            assert_eq!(
                says,
                Some(standing.1.as_str()),
                "the {which} language says it was opened from row {} and that row is {says:?}",
                standing.0
            );
        }
    }

    #[test]
    fn every_part_of_the_world_carries_the_row_it_was_opened_from() {
        let (seen, opening) = watching();
        let Ok(zones) = hours::zones("Europe/Amsterdam\nAsia/Bangkok\nUTC");
        let Ok(regions) = hours::regions(&zones);
        let Ok(rows) = region_rows(&regions, None, nowhere(), opening);
        let at = opened(&seen);

        for standing in &at {
            let says = rows.get(standing.0).map(|row| row.says.as_str());

            assert_eq!(says, Some(standing.1.as_str()), "opened from row {}", standing.0);
        }
    }

    #[test]
    fn the_language_the_machine_is_in_is_the_one_marked() {
        let spoken = spoken();
        let Ok(standing) = crate::tongues::standing(&spoken, Some("nl_NL.UTF-8"));
        let Ok(rows) = tongue_rows(&spoken, standing.as_ref(), nowhere(), opening);

        assert_eq!(marked(&rows), ["nl"]);
    }

    #[test]
    fn the_places_a_language_is_spoken_are_a_list_under_it() {
        let spoken = spoken();
        let Ok(names) = Names::none();
        let english = spoken.iter().find(|tongue| tongue.language == "en").expect("English");
        let Ok(standing) = crate::tongues::standing(&spoken, Some("en_GB.UTF-8"));
        let Ok(rows) = place_rows(english, &names, &[], standing.as_ref(), nowhere());

        assert_eq!(says(&rows)[2..], ["GB", "US"]);
        assert_eq!(marked(&rows), ["GB"]);
    }

    #[test]
    fn latin_is_drawn_and_cannot_be_pressed_and_the_rest_can() {
        let Ok(chosen) = alphabets::read(alphabets::UNLESS_TOLD);
        let Ok(rows) = alphabet_rows(&chosen, nowhere());

        assert_eq!(says(&rows)[2..], [
            "Latin", "Arabic", "Georgian", "Greek", "Hebrew", "Persian", "Russian", "Thai",
        ]);
        assert_eq!(marked(&rows), ["Latin", "Thai"]);
        assert_eq!(rows[2].acts(), Ok(console_panel::page::Acts::Nothing));
        assert_eq!(rows[3].acts(), Ok(console_panel::page::Acts::Yes));
    }

    #[test]
    fn the_parts_of_the_world_open_onto_the_places_in_them() {
        let Ok(zones) = hours::zones("Europe/Amsterdam\nEurope/London\nAsia/Bangkok");
        let Ok(regions) = hours::regions(&zones);
        let Ok(rows) = region_rows(&regions, Some("Europe/Amsterdam"), nowhere(), opening);

        assert_eq!(says(&rows)[2..], ["Asia", "Europe"]);
        assert_eq!(marked(&rows), ["Europe"]);
    }

    #[test]
    fn a_part_of_the_world_is_its_places_with_the_one_it_is_in_marked() {
        let Ok(zones) = hours::zones("Europe/Amsterdam\nEurope/London\nAsia/Bangkok");
        let Ok(places) = hours::places(&zones, "Europe");
        let Ok(rows) = zone_rows("Europe", &places, Some("Europe/London"), nowhere());

        assert_eq!(says(&rows)[2..], ["Amsterdam", "London"]);
        assert_eq!(marked(&rows), ["London"]);
    }

    #[test]
    fn the_clock_is_the_hour_written_both_ways_with_one_of_them_marked() {
        let Ok(rows) = clock_rows(Clock::Twelve, nowhere());

        assert_eq!(says(&rows)[2..], ["14:30", "2:30 pm"]);
        assert_eq!(marked(&rows), ["2:30 pm"]);
    }

    #[test]
    fn the_machines_name_is_a_row_that_asks_rather_than_one_that_opens() {
        let Ok(row) = called_row("legion");
        let Ok(asks) = say(&Word::WhatThisMachineIsCalled);

        assert_eq!(row.says, asks);
        assert_eq!(row.aside, "legion");
        assert_eq!(row.acts(), Ok(console_panel::page::Acts::Yes));
    }

    #[test]
    fn every_tab_is_named_once() {
        let mut sorted = named().to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), named().len());
    }
}
