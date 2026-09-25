//! What each tab holds, as a function of what the machine said.
//!
//! Reading the machine is one thing and knowing what to draw from it is
//! another. Everything here is the second, so the shape of every tab can be
//! asked without a machine to ask.

use std::collections::BTreeSet;
use std::sync::Arc;

use console_battery as battery;
use console_default_applications::engines;
use console_core_external_programs::Program;
use console_core_internal_programs::InternalProgram;
use console_login_window::stored_pattern::StoredPattern;
use console_home_screen::shape::Shape;
use console_core_never::Never;
use console_core_localization::text;

use crate::words::Word;
use console_notifications::reading::DoNotDisturb;
use console_books::appearance::{self, Appearance, Paint, ColorRole, Typeface, TYPEFACES};
use console_panel::page::{Aside, ButtonPress, Handler, Active, Level, NOW, Row, Showing, Subject, YET};
use crate::introducing;
use console_input_dictation::languages;

use console_input_alphabets::{self as alphabets, Alphabet};

use console_default_applications::clock::{self, Clock};

use crate::hours::{self, Place};
use crate::level::{CELLS, Muted, bar, volume};
use crate::named::{self, Allowed};
use crate::languages::{Locale, Made, Names, Language, made};
use crate::size::{EVERY, Size};
use crate::turning::{self, Turn};
use crate::learned::Following;
use crate::warm::NightShift;
use console_sound_effects::SoundEffects;
use crate::{bluetooth, sound, wifi};

const NOWHERE_SAID: &str = "";


pub const ON: &str = "On";

pub const OFF: &str = "Off";

pub fn switch(says: &str, state: Aside<'_>, arguments: &[&str]) -> Result<Row, Never> {
    let arguments: Vec<String> = arguments.iter().map(|word| (*word).to_string()).collect();
    let Ok(later) = Handler::and_stay(move |showing| showing.later(arguments.clone()));

    Row::new(says, state, later)
}

pub fn sound_rows(
    sinks: &[sound::Thing],
    playing: &[sound::Thing],
    default: &str,
    hush: impl Fn(i64, &'static str) -> Result<Handler, Never>,
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
            let Ok(row) = Row::new("Speakers", Aside(&level), silence);
            let Ok(turning) = turn(speakers.index, "sink");
            let Ok(leveled) = row.leveled(turning);

            rows.push(leveled);
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
        let Ok(row) = Row::new(&said, Aside(&level), silence);
        let Ok(turning) = turn(stream.index, "sink-input");
        let Ok(leveled) = row.leveled(turning);

        rows.push(leveled);
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

fn chosen_among<T>(says: &str, it: T, now: T, choose: fn(T) -> Result<(), Never>) -> Result<Row, Never>
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    let Ok(choosing) = Handler::and_stay(move |showing| {
        let Ok(()) = choose(it);

        showing.refresh();
    });

    Row::new(says, Aside(match it == now {
        true => NOW,
        false => "",
    }), choosing)
}

fn swatches(painted: ColorRole, now: Paint) -> Result<Vec<Row>, Never> {
    let Ok(default) = Handler::and_stay(move |showing| {
        let Ok(()) = painted.choose(Paint::Desktop);

        showing.refresh();
    });
    let Ok(default) = Row::new("Default", Aside(match now {
        Paint::Desktop => NOW,
        Paint::Chosen(_) => "",
    }), default);
    let Ok(grid) = appearance::grid();
    let mut rows = vec![default];

    for colors in grid {
        let mut presses = Vec::new();
        let mut at = 0;

        for (column, color) in (0_u32..).zip(colors) {
            let chosen = Paint::Chosen(color);
            let in_effect = match chosen == now {
                true => {
                    at = column;

                    Active::Yes
                },
                false => Active::No,
            };
            let Ok(press) = ButtonPress::swatch(color, in_effect, move |showing| {
                let Ok(()) = painted.choose(chosen);

                showing.refresh();
            });

            presses.push(press);
        }

        let Ok(row) = Row::pressing(presses, at);

        rows.push(row);
    }

    Ok(rows)
}

pub fn books_rows(now: Appearance) -> Result<Vec<Row>, Never> {
    let Ok(background) = Row::nothing("Background");
    let Ok(pages) = swatches(ColorRole::Background, now.background);
    let Ok(text) = Row::nothing("Text");
    let Ok(inks) = swatches(ColorRole::Text, now.text);
    let Ok(font) = Row::nothing("Font");
    let mut rows = vec![background];

    rows.extend(pages);
    rows.push(text);
    rows.extend(inks);
    rows.push(font);

    for typeface in TYPEFACES {
        let Ok(says) = typeface.says();
        let Ok(row) = chosen_among(says, *typeface, now.typeface, Typeface::choose);

        rows.push(row);
    }

    Ok(rows)
}

pub fn sound_effects(effects: SoundEffects) -> Result<Row, Never> {
    let state = match effects {
        SoundEffects::On => ON,
        SoundEffects::Off => OFF,
    };
    let Ok(turning) = Handler::and_stay(move |showing| {
        let Ok(turned) = effects.flipped();
        let Ok(()) = turned.choose();

        showing.refresh();
    });

    Row::new("Sound Effects", Aside(state), turning)
}

pub fn following(following: Following) -> Result<Row, Never> {
    let Ok(follow_the_room) = text(&Word::AutoBrightness);

    let state = match following {
        Following::Yes => Word::On,
        Following::No => Word::Off,
    };

    let Ok(on_or_off) = text(&state);

    let Ok(brightness) = InternalProgram::Brightness.path();

    switch(&follow_the_room, Aside(&on_or_off), &[brightness, "follow-or-not"])
}

pub fn warmth(warm: NightShift) -> Result<Row, Never> {
    let Ok(night_colors) = text(&Word::NightShift);

    let state = match warm {
        NightShift::Scheduled => Word::On,
        NightShift::Off => Word::Off,
    };

    let Ok(on_or_off) = text(&state);

    let Ok(night_shift) = InternalProgram::NightShift.path();

    switch(&night_colors, Aside(&on_or_off), &[night_shift])
}

pub fn threshold(level: i32) -> Result<String, Never> {
    Ok(match level {
        battery::NEVER => {
            let Ok(never) = text(&Word::Never);

            never
        },
        level => format!("{level}%"),
    })
}

pub fn dwindling(
    levels: battery::Levels,
    guard: impl Fn(battery::Step) -> Result<Level, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(when_the_battery_gets_low) = text(&Word::LowBattery);
    let Ok(naming) = Row::naming(&when_the_battery_gets_low, Aside(""));
    let mut rows = vec![naming];

    rows.extend(battery::EVERY.into_iter().map(|step| {
        let Ok(word) = word_of(step);
        let Ok(level) = levels.at(step);
        let Ok(at) = threshold(level);
        let Ok(words) = text(&word);
        let Ok(row) = Row::said(&words, Aside(&at));
        let Ok(guarding) = guard(step);
        let Ok(leveled) = row.leveled(guarding);

        leveled
    }));

    Ok(rows)
}

fn word_of(step: battery::Step) -> Result<Word, Never> {
    Ok(match step {
        battery::Step::Low => Word::AlertAt,
        battery::Step::Lower => Word::AlertAgainAt,
        battery::Step::Protect => Word::ShutDownAt,
    })
}

pub fn home_rows(
    shape: Shape,
    across: Level,
    down: Level,
    sized: Level,
) -> Result<Vec<Row>, Never> {
    let Ok(the_home_screen) = text(&Word::HomeScreen);
    let Ok(naming) = Row::naming(&the_home_screen, Aside(""));
    let Ok(applications_across) = text(&Word::Columns);
    let Ok(columns) = Row::said(&applications_across, Aside(&shape.columns.to_string()));
    let Ok(columns) = columns.leveled(across);
    let Ok(applications_down) = text(&Word::Rows);
    let Ok(down_row) = Row::said(&applications_down, Aside(&shape.rows.to_string()));
    let Ok(down_row) = down_row.leveled(down);
    let Ok(word) = word_of_home_size(shape.size);
    let Ok(how_big_they_are) = text(&Word::IconSize);
    let Ok(words) = text(&word);
    let Ok(big) = Row::said(&how_big_they_are, Aside(&words));
    let Ok(big) = big.leveled(sized);

    Ok(vec![naming, columns, down_row, big])
}

fn word_of_home_size(size: console_home_screen::shape::Size) -> Result<Word, Never> {
    Ok(match size {
        console_home_screen::shape::Size::Tiny => Word::SizeSmallest,
        console_home_screen::shape::Size::Smaller => Word::SizeSmaller,
        console_home_screen::shape::Size::Normal => Word::SizeDefault,
        console_home_screen::shape::Size::Bigger => Word::SizeLarger,
        console_home_screen::shape::Size::Huge => Word::SizeLargest,
    })
}

pub fn screen_rows(
    brightness: Option<i32>,
    dim: Level,
    room: Option<Following>,
    warm: NightShift,
    standing: Option<Size>,
    turned: Option<Turn>,
    home: Vec<Row>,
) -> Result<Vec<Row>, Never> {
    let level = match brightness {
        Some(level) => volume(level, Muted::No)?,
        None => YET.to_string(),
    };
    let Ok(screen_brightness) = text(&Word::Brightness);
    let Ok(bright) = Row::said(&screen_brightness, Aside(&level));
    let Ok(bright) = bright.leveled(dim);
    let Ok(warmth) = warmth(warm);
    let Ok(how_big_everything_is) = text(&Word::DisplayZoom);
    let Ok(naming) = Row::naming(&how_big_everything_is, Aside(""));
    let mut rows = vec![bright];

    match room {
        Some(room) => {
            let Ok(row) = following(room);

            rows.push(row);
        }
        None => {},
    }

    rows.push(warmth);
    rows.push(naming);

    rows.extend(EVERY.into_iter().map(|size| {
        let Ok(word) = word_of_size(size);
        let Ok(words) = text(&word);
        let Ok(written) = size.written();
        let Ok(scale) = InternalProgram::Scale.path();
        let Ok(mut row) = switch(&words, Aside(""), &[scale, written]);

        row.aside = match standing == Some(size) {
            true => NOW.to_string(),
            false => String::new(),
        };

        row
    }));

    let Ok(which_way_up) = text(&Word::Rotation);
    let Ok(naming) = Row::naming(&which_way_up, Aside(""));

    rows.push(naming);
    rows.extend(turning::EVERY.into_iter().map(|turn| {
        let Ok(word) = word_of_turn(turn);
        let Ok(words) = text(&word);
        let Ok(written) = turn.written();
        let Ok(scale) = InternalProgram::Scale.path();
        let Ok(mut row) = switch(&words, Aside(""), &[scale, written]);

        row.aside = match turned == Some(turn) {
            true => NOW.to_string(),
            false => String::new(),
        };

        row
    }));
    rows.extend(home);

    Ok(rows)
}

fn word_of_turn(turn: Turn) -> Result<Word, Never> {
    Ok(match turn {
        Turn::Left => Word::RotatedLeft,
        Turn::Upright => Word::Standard,
        Turn::Right => Word::RotatedRight,
        Turn::Over => Word::UpsideDown,
    })
}

fn word_of_size(size: Size) -> Result<Word, Never> {
    Ok(match size {
        Size::Tiny => Word::SizeSmallest,
        Size::Smaller => Word::SizeSmaller,
        Size::Normal => Word::SizeDefault,
        Size::Bigger => Word::SizeLarger,
        Size::Huge => Word::SizeLargest,
    })
}

pub fn battery_rows(
    running: Option<&str>,
    levels: battery::Levels,
    guard: impl Fn(battery::Step) -> Result<Level, Never>,
) -> Result<Vec<Row>, Never> {
    let profile = |says: &str, name: &'static str| {
        let mark = match running {
            Some(running) => match running == name {
                true => NOW,
                false => "",
            },
            None => "",
        };
        let Ok(powerprofilesctl) = Program::Powerprofilesctl.name();
        let Ok(sets) = Handler::run(&[powerprofilesctl, "set", name]);
        let Ok(row) = Row::new(says, Aside(mark), sets);

        row
    };
    let Ok(how_fast_the_machine_runs) = text(&Word::PowerMode);
    let Ok(naming) = Row::naming(&how_fast_the_machine_runs, Aside(""));
    let Ok(speed_saving) = text(&Word::LowPower);
    let Ok(speed_normal) = text(&Word::Automatic);
    let Ok(speed_fast) = text(&Word::HighPower);
    let Ok(mut rows) = power_rows();

    rows.extend([
        naming,
        profile(&speed_saving, "power-saver"),
        profile(&speed_normal, "balanced"),
        profile(&speed_fast, "performance"),
    ]);

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
    join: impl Fn(wifi::Network, wifi::Known) -> Result<Handler, Never>,
    share: impl Fn(wifi::Network) -> Result<Shares, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(nmcli) = Program::Nmcli.name();

    match on {
        wifi::Radio::Off => {
            let Ok(row) = switch(WIFI, Aside(OFF), &[nmcli, "radio", "wifi", "on"]);

            return Ok(vec![row]);
        }
        wifi::Radio::On => {},
    }

    let Ok(off) = switch(WIFI, Aside(ON), &[nmcli, "radio", "wifi", "off"]);
    let mut rows = vec![off];
    let known: BTreeSet<&str> = known.iter().map(String::as_str).collect();

    for network in networks {
        match network.here {
            true => {
                let name = network.name.clone();
                let Ok(shares) = share(network);
                let Ok(row) = Row::said(&name, Aside(NOW));
                let Ok(row) = row.offering(move |showing| {
                    let shares = Arc::clone(&shares);

                    showing.sure(&name, Subject(""), &[SHOW_CODE], Arc::new(move |showing, _only_one| shares(showing)));

                    false
                });

                rows.push(row);
                continue;
            }
            false => {},
        }

        let Ok(aside) = strength(network.signal);
        let says = network.name.clone();
        let already = match known.contains(network.name.as_str()) {
            true => wifi::Known::Yes,
            false => wifi::Known::No,
        };
        let Ok(joining) = join(network, already);
        let Ok(row) = Row::new(&says, Aside(&aside), joining);

        rows.push(row);
    }

    Ok(rows)
}

pub type Opens = Arc<dyn Fn(&bluetooth::Met, u32, &dyn Showing) + Send + Sync>;

const RADIO: u32 = 1;

const INTRODUCES: &str = "console-bluetooth";

const WIFI: &str = "Wi-Fi";

pub const SHOW_CODE: &str = "Show Network QR Code";

pub type Shares = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

const BLUETOOTH: &str = "Bluetooth";

const PAIR: &str = "Pair";

const FORGET: &str = "Forget This Device";

const FORGET_SURE: &str = "Forget This Device?";

const FORGET_YES: &str = "Forget";

const JOIN: &str = "Connect";

const LEAVE: &str = "Disconnect";

pub const LOOK: &str = "Search for Devices";

pub const LOOKING: &str = "Searching for Devices";

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
            let Ok(row) = switch(BLUETOOTH, Aside(OFF), &[bluetoothctl, "power", "on"]);

            return Ok(vec![row]);
        }
        bluetooth::Radio::On => {},
    }

    let Ok(off) = switch(BLUETOOTH, Aside(ON), &[bluetoothctl, "power", "off"]);
    let mut rows = vec![off];
    let Ok(met) = bluetooth::in_order(met);

    for (from, met) in met.into_iter().enumerate() {
        let Ok(from) = console_core_number_conversion::fitted::<_, u32>(from);
        let at = from.saturating_add(RADIO);
        let Ok(row) = device_row(met, at, &open);

        rows.push(row);
    }

    let last = match looking {
        bluetooth::Looking::Yes => Row::said(LOOKING, Aside(YET)),
        bluetooth::Looking::No => {
            let Ok(words) = looking_words();

            switch(LOOK, Aside(""), &words)
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

fn device_row(met: bluetooth::Met, at: u32, open: &Opens) -> Result<Row, Never> {
    let Ok(bluetoothctl) = Program::Bluetoothctl.name();

    match met.known {
        bluetooth::Known::No => {
            let opening = Arc::clone(open);
            let opened = met.clone();
            let Ok(opens) = Handler::and_stay(move |showing| opening(&opened, at, showing));
            let Ok(aside) = heard_aside(&met);

            Row::new(&met.device.name, Aside(&aside), opens)
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
            let Ok(mut row) = switch(&met.device.name, Aside(""), &[bluetoothctl, doing, &met.device.address]);

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
            let Ok(row) = switch(PAIR, Aside(""), &words);

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
            let Ok(row) = switch(says, Aside(""), &[bluetoothctl, doing, &met.device.address]);

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
    let arguments = vec![bluetoothctl.to_string(), "remove".to_string(), device.address.to_string()];
    let name = device.name.clone();

    let Ok(forgets) = Handler::and_stay(move |showing| {
        let arguments = arguments.clone();
        let back = Arc::clone(&back);

        showing.sure(
            FORGET_SURE,
            Subject(&name),
            &[FORGET_YES],
            Arc::new(move |showing, _| {
                showing.later(arguments.clone());
                back(showing);
            }),
        );
    });

    Row::new(FORGET, Aside(""), forgets)
}

pub fn search_rows(engine: &str, back: Chosen) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(configuration) = configuration();
    let Ok(way_back) = Row::back(&configuration, move |showing| leaving(showing));
    let Ok(naming) = Row::naming("Search Engine", Aside(""));
    let mut rows = vec![way_back, naming];

    for offered in &engines::EVERY {
        let mark = match offered.key == engine {
            true => NOW,
            false => "",
        };
        let key = offered.key;
        let back = Arc::clone(&back);
        let Ok(chooses) = Handler::and_stay(move |showing| {
            let Ok(()) = engines::choose(key);
            let Ok(telling) = telling(key);

            showing.later(telling);
            back(showing);
        });
        let Ok(row) = Row::new(offered.says, Aside(mark), chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn engine_says(engine: &str) -> Result<String, Never> {
    let known = engines::one(engine)?;

    Ok(match known.map(|found| found.says.to_string()) {
        Some(says) => says,
        None => String::new(),
    })
}

pub fn dictation_rows(language: &str, back: Chosen) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(tab) = the_language();
    let Ok(way_back) = Row::back(&tab, move |showing| leaving(showing));
    let Ok(listens) = text(&Word::DictationLanguage);
    let Ok(naming) = Row::naming(&listens, Aside(""));
    let mut rows = vec![way_back, naming];

    for offered in &languages::EVERY {
        let mark = match offered.key == language {
            true => NOW,
            false => "",
        };
        let key = offered.key;
        let back = Arc::clone(&back);
        let Ok(chooses) = Handler::and_stay(move |showing| {
            let Ok(()) = languages::choose(key);

            back(showing);
        });
        let Ok(row) = Row::new(offered.says, Aside(mark), chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn dictation_says(language: &str) -> Result<String, Never> {
    let known = languages::one(language)?;

    Ok(match known.map(|found| found.says.to_string()) {
        Some(says) => says,
        None => String::new(),
    })
}

pub fn telling(engine: &str) -> Result<Vec<String>, Never> {
    Program::Sudo.arguments(&["-n", "console-engine", engine])
}

pub fn power_rows() -> Result<Vec<Row>, Never> {
    let Ok(systemctl) = Program::Systemctl.name();
    let Ok(suspends) = Handler::run(&[systemctl, "suspend"]);
    let Ok(sleep) = Row::new("Sleep", Aside(""), suspends);
    let Ok(reboots) = Handler::run(&[systemctl, "reboot"]);
    let Ok(restart) = Row::new("Restart", Aside(""), reboots);
    let Ok(powers_off) = Handler::run(&[systemctl, "poweroff"]);
    let Ok(shut_down) = Row::new("Shut Down", Aside(""), powers_off);

    Ok(vec![sleep, restart, shut_down])
}

pub fn game_row() -> Result<Row, Never> {
    let Ok(session_game) = InternalProgram::SessionGame.path();
    let Ok(game) = Handler::run(&[session_game]);

    Row::new("Game Mode", Aside(""), game)
}

const LOGIN_PATTERN: &str = "Login Pattern";

const TURN_OFF_LOGIN_PATTERN: &str = "Turn Off Login Pattern";

pub fn login_rows() -> Result<Vec<Row>, Never> {
    let Ok(home) = console_core_places::home();
    let stored = home.as_deref().map(console_login_window::stored_pattern::stored);
    let Ok(setter) = InternalProgram::SettingsLoginPattern.name();
    let Ok(chooses) = Handler::run(&[setter]);
    let Ok(forgets) = Handler::run(&[setter, "off"]);

    match stored {
        Some(Ok(StoredPattern::Hash(_))) => {
            let Ok(change) = Row::new(LOGIN_PATTERN, Aside(ON), chooses);
            let Ok(off) = Row::new(TURN_OFF_LOGIN_PATTERN, Aside(""), forgets);

            Ok(vec![change, off])
        }
        Some(Ok(StoredPattern::Absent)) => {
            let Ok(set) = Row::new(LOGIN_PATTERN, Aside(OFF), chooses);

            Ok(vec![set])
        }
        Some(Err(why)) => {
            let Ok(unread) = Row::said(LOGIN_PATTERN, Aside(&why.to_string()));

            Ok(vec![unread])
        }
        None => Ok(Vec::new()),
    }
}

pub const QUIETEN: &str = "Quieten";

const NOTIFICATIONS: &str = "Notifications";

const BELL: &str = "Notification Center";

pub fn notifications_rows(do_not_disturb: DoNotDisturb) -> Result<Vec<Row>, Never> {
    let says = NOTIFICATIONS;
    let mark = match do_not_disturb {
        DoNotDisturb::On => OFF,
        DoNotDisturb::Off => ON,
    };
    let Ok(busctl) = Program::Busctl.name();
    let Ok(arguments) = console_notifications::serving::asking(QUIETEN);
    let mut said = vec![busctl];

    said.extend(arguments.iter().map(String::as_str));

    let Ok(row) = switch(says, Aside(mark), &said);

    let Ok(bell) = Row::said(BELL, Aside("All, even silenced"));

    Ok(vec![row, bell])
}

pub fn tabs() -> Result<[String; 11], Never> {
    let Ok(configuration) = configuration();

    let Ok(sound) = text(&Word::Sound);
    let Ok(bluetooth) = text(&Word::Bluetooth);
    let Ok(wifi) = text(&Word::Wifi);
    let Ok(battery) = text(&Word::Battery);
    let Ok(notifications) = text(&Word::Notifications);
    let Ok(screen) = text(&Word::Display);
    let Ok(wallpaper) = text(&Word::Wallpaper);
    let Ok(language) = text(&Word::Language);
    let Ok(books) = text(&Word::Books);
    let Ok(security) = text(&Word::Security);

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
        books,
        security,
    ])
}

pub fn the_language() -> Result<String, Never> {
    text(&Word::Language)
}

pub fn where_you_are() -> Result<String, Never> {
    text(&Word::TimeZone)
}

pub fn the_clock() -> Result<String, Never> {
    text(&Word::TimeFormat)
}

pub fn configuration() -> Result<String, Never> {
    text(&Word::General)
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Languages<'a> {
    pub says: &'a str,
    pub types: &'a str,
    pub listens: &'a str,
}

pub fn language_rows(languages: Languages<'_>, into: [Handler; 3]) -> Result<Vec<Row>, Never> {
    let [words, keyboard, dictation] = into;

    let Ok(what_says) = text(&Word::PreferredLanguage);
    let Ok(what_types) = text(&Word::Keyboards);
    let Ok(what_listens) = text(&Word::DictationLanguage);

    let Ok(row) = Row::new(&what_says, Aside(languages.says), words);
    let Ok(said) = row.opening();
    let Ok(row) = Row::new(&what_types, Aside(languages.types), keyboard);
    let Ok(typed) = row.opening();
    let Ok(row) = Row::new(&what_listens, Aside(languages.listens), dictation);
    let Ok(heard) = row.opening();

    let Ok(english) = text(&Word::EnglishOnly);
    let Ok(standing) = Row::nothing(&english);

    Ok(vec![said, typed, heard, standing])
}

fn leading(says: &str, back: Chosen) -> Result<Vec<Row>, Never> {
    let Ok(tab) = the_language();
    let Ok(way_back) = Row::back(&tab, move |showing| back(showing));
    let Ok(naming) = Row::naming(says, Aside(""));

    Ok(vec![way_back, naming])
}

fn running(arguments: Vec<String>, note: Option<String>, back: Chosen) -> Result<Handler, Never> {
    Handler::and_stay(move |showing| {
        match &note {
            Some(said) => showing.note(said),
            None => {},
        }

        showing.later(arguments.clone());

        back(showing);
    })
}

pub fn language_picker_rows(
    languages: &[Language],
    standing: Option<&Locale>,
    back: Chosen,
    opening: impl Fn(u32, &str) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(what_says) = text(&Word::PreferredLanguage);
    let Ok(mut rows) = leading(&what_says, back);

    let Ok(first) = console_core_number_conversion::fitted::<_, u32>(rows.len());
    let here = match standing.map(|locale| locale.language.clone()) {
        Some(here) => here,
        None => String::new(),
    };

    for (at, language) in languages.iter().enumerate() {
        let mark = match language.language == here {
            true => NOW,
            false => "",
        };
        let Ok(at) = console_core_number_conversion::fitted::<_, u32>(at);
        let Ok(opens) = opening(at.saturating_add(first), &language.language);
        let Ok(row) = Row::new(&language.says, Aside(mark), opens);
        let Ok(row) = row.opening();

        rows.push(row);
    }

    Ok(rows)
}

pub fn place_rows(
    language: &Language,
    names: &Names,
    generated: &[String],
    standing: Option<&Locale>,
    back: Chosen,
) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(where_) = text(&Word::Region);
    let Ok(mut rows) = leading(&where_, leaving);

    let here = match standing.map(|locale| locale.name.clone()) {
        Some(here) => here,
        None => String::new(),
    };

    for locale in &language.locales {
        let mark = match locale.name == here {
            true => NOW.to_string(),
            false => String::new(),
        };
        let Ok(where_) = names.where_(locale);

        let says = match where_.is_empty() {
            true => language.says.clone(),
            false => where_,
        };

        let Ok(made) = made(generated, locale);

        let note = match made {
            Made::Yes => None,
            Made::No => {
                let Ok(making) = text(&Word::PreparingLanguage);

                Some(making)
            }
        };

        let Ok(arguments) = Program::Sudo.arguments(&[
            "-n",
            "console-machine",
            "language",
            &locale.name,
            &locale.charset,
        ]);
        let Ok(chooses) = running(arguments, note, Arc::clone(&back));
        let Ok(row) = Row::new(&says, Aside(&mark), chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn alphabet_rows(chosen: &[&'static Alphabet], back: Chosen) -> Result<Vec<Row>, Never> {
    let Ok(what_types) = text(&Word::Keyboards);
    let Ok(mut rows) = leading(&what_types, back);

    for alphabet in &alphabets::EVERY {
        let held = chosen.iter().any(|already| already.key == alphabet.key);

        let mark = match held {
            true => NOW,
            false => "",
        };

        match alphabet.key == alphabets::LATIN {
            true => {
                let Ok(row) = Row::said(alphabet.says, Aside(mark));

                rows.push(row);
            }
            false => {
                let Ok(turned) = alphabets::turned(chosen, alphabet.key);
                let Ok(words) = Program::Systemctl.arguments(&["--user", "restart", KEYBOARD]);
                let Ok(turns) = Handler::and_stay(move |showing| {
                    let Ok(()) = alphabets::choose(&turned);

                    showing.later(words.clone());
                    showing.later(vec![
                        console_input_language::NAMED.to_string(),
                        console_input_language::SETTLE.to_string(),
                    ]);
                    showing.refresh();
                });
                let Ok(row) = Row::new(alphabet.says, Aside(mark), turns);

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
    opening: impl Fn(u32, &str) -> Result<Handler, Never>,
) -> Result<Vec<Row>, Never> {
    let Ok(where_) = text(&Word::TimeZone);
    let Ok(mut rows) = leading(&where_, back);

    let Ok(first) = console_core_number_conversion::fitted::<_, u32>(rows.len());
    let told = match standing {
        Some(told) => told,
        None => NOWHERE_SAID,
    };

    let Ok(here) = hours::part_of(told);

    for (at, region) in regions.iter().enumerate() {
        let mark = match *region == here {
            true => NOW,
            false => "",
        };
        let Ok(at) = console_core_number_conversion::fitted::<_, u32>(at);
        let Ok(opens) = opening(at.saturating_add(first), region);
        let Ok(row) = Row::new(region, Aside(mark), opens);
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

    let here = match standing {
        Some(here) => here,
        None => NOWHERE_SAID,
    };

    for place in places {
        let mark = match place.zone == here {
            true => NOW,
            false => "",
        };
        let Ok(arguments) = Program::Sudo.arguments(&["-n", "console-machine", "hour", &place.zone]);
        let Ok(chooses) = running(arguments, None, Arc::clone(&back));
        let Ok(row) = Row::new(&place.says, Aside(mark), chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn clock_rows(now: Clock, back: Chosen) -> Result<Vec<Row>, Never> {
    let leaving = Arc::clone(&back);
    let Ok(the_clock) = text(&Word::TimeFormat);
    let Ok(mut rows) = leading(&the_clock, leaving);

    for reading in clock::EVERY {
        let mark = match reading == now {
            true => NOW,
            false => "",
        };
        let Ok(says) = reading.says();
        let leaving = Arc::clone(&back);
        let Ok(chooses) = Handler::and_stay(move |showing| {
            let Ok(()) = clock::choose(reading);

            leaving(showing);
        });
        let Ok(row) = Row::new(says, Aside(mark), chooses);

        rows.push(row);
    }

    Ok(rows)
}

pub fn called_row(name: &str) -> Result<Row, Never> {
    let Ok(says) = text(&Word::Name);
    let asking = says.clone();
    let Ok(asks) = Handler::and_stay(move |showing| {
        showing.ask(
            &asking,
            Arc::new(move |showing, word| {
                let Ok(allowed) = named::allowed(word);

                match allowed {
                    Allowed::Yes => {
                        let Ok(arguments) =
                            Program::Sudo.arguments(&["-n", "console-machine", "name", word]);

                        showing.later(arguments);
                    }
                    Allowed::No => showing.note(NOT_A_NAME),
                }
            }),
        );
    });

    Row::new(&says, Aside(name), asks)
}

const NOT_A_NAME: &str = "Use only letters, numbers and hyphens";

const KEYBOARD: &str = "console-input-keyboard.service";

pub type Chosen = Arc<dyn Fn(&dyn Showing) + Send + Sync>;

#[cfg(test)]
mod tests {
    use console_core_localization::Localized;
    use console_panel::page::{Heading, Active, Nowhere};
    use super::*;

    fn nothing() -> Level {
        std::sync::Arc::new(|_| ())
    }

    fn grid() -> Vec<Row> {
        let Ok(rows) = home_rows(Shape::USUAL, nothing(), nothing(), nothing());

        rows
    }

    fn silence(_: i64, _: &'static str) -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn now(row: &Row) -> Active {
        let Ok(now) = row.now();

        now
    }

    fn heading(row: &Row) -> Heading {
        let Ok(heading) = row.heading();

        heading
    }

    fn marked(rows: &[Row]) -> Vec<&str> {
        rows.iter()
            .filter(|row| now(row) == Active::Yes)
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

    #[test]
    fn sound_effects_say_whether_they_are_on() {
        let Ok(off) = sound_effects(SoundEffects::Off);
        let Ok(on) = sound_effects(SoundEffects::On);

        assert_eq!((off.says.as_str(), off.aside.as_str()), ("Sound Effects", OFF));
        assert_eq!(on.aside, ON);
        assert!(off.does.is_some(), "the row is a switch someone can press");
    }

    fn wifi_of(on: wifi::Radio, networks: Vec<wifi::Network>) -> Vec<Row> {
        let Ok(rows) = wifi_rows(on, networks, &[], |_, _| silence(0, ""), |_| Ok(Arc::new(|_: &dyn Showing| ())));

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

    fn named() -> [String; 11] {
        let Ok(tabs) = tabs();

        tabs
    }

    fn sized(standing: Option<Size>) -> Vec<Row> {
        let Ok(rows) =
            screen_rows(Some(50), nothing(), None, NightShift::Off, standing, None, grid());

        rows
    }

    fn nowhere() -> Chosen {
        Arc::new(|_: &dyn Showing| ())
    }

    fn turning(_: i64, _: &'static str) -> Result<Level, Never> {
        Ok(nothing())
    }

    fn says(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.says.as_str()).collect()
    }

    fn from<'a>(rows: &'a [Row], said: &str) -> &'a [Row] {
        let mut rest = rows;

        while let Some((first, after)) = rest.split_first() {
            match first.says == said {
                true => return rest,
                false => rest = after,
            }
        }

        rest
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
        assert_eq!(marked(&rows), [Word::HighPower.english()]);
    }

    #[test]
    fn the_battery_tab_opens_on_stopping_the_machine() {
        assert_eq!(says(&battery()[0..3]), ["Sleep", "Restart", "Shut Down"]);
    }

    #[test]
    fn the_three_speeds_are_a_named_scale_with_the_least_of_them_first() {
        let rows = battery();
        assert!(rows[3].naming, "the speeds are not named");
        assert_eq!(rows[3].says, Word::PowerMode.english());
        assert_eq!(
            says(&rows[4..7]),
            [Word::LowPower.english(), Word::Automatic.english(), Word::HighPower.english()]
        );
    }

    #[test]
    fn the_books_tab_marks_the_page_the_ink_and_the_face_that_were_chosen() {
        let grid = appearance::grid().expect("the grid");
        let page = grid[2][3];
        let now = Appearance { background: Paint::Chosen(page), text: Paint::Desktop, typeface: Typeface::Monospaced };
        let rows = books_rows(now).expect("the rows");
        let marked: Vec<&str> = rows.iter().filter(|row| row.aside == NOW).map(|row| row.says.as_str()).collect();

        assert_eq!(marked, ["Default", "Monospaced"], "the ink is the desktop's and the face was chosen");

        let lit: Vec<(u32, console_panel::page::Face)> = rows
            .iter()
            .filter_map(|row| row.buttons.as_ref())
            .flat_map(|across| across.presses.iter().filter(|press| press.now == Active::Yes).map(move |press| (across.at, press.face)))
            .collect();

        assert_eq!(lit, [(3, console_panel::page::Face::Swatch(page))], "the page chosen is the one swatch lit, and it is where the row starts");
        assert_eq!(rows.len(), 3 + 2 * (1 + grid.len()) + TYPEFACES.len());
    }

    #[test]
    fn the_screen_and_how_hard_the_machine_works_are_two_tabs() {
        let battery = says(&battery()).join("\n");
        for screen in [Word::Brightness.english(), Word::DisplayZoom.english()] {
            assert!(!battery.contains(&screen), "{screen:?} is still on the Battery tab");
        }
        assert!(!battery.contains("night colors"), "the evening is still on the Battery tab");
    }

    #[test]
    fn the_sizes_are_a_named_scale_with_the_smallest_of_them_first() {
        let rows = screen();
        let named = from(&rows, &Word::DisplayZoom.english());
        assert!(named.first().expect("the sizes are named").naming, "the name is a row the highlight can land on");
        assert_eq!(
            says(&named[1..1 + EVERY.len()]),
            [
                Word::SizeSmallest.english(),
                Word::SizeSmaller.english(),
                Word::SizeDefault.english(),
                Word::SizeLarger.english(),
                Word::SizeLargest.english(),
            ]
        );
        assert!(
            rows.iter().take_while(|row| row.says != Word::DisplayZoom.english()).any(|row| row.level.is_some()),
            "the brightness is above the name, not under it"
        );
    }

    #[test]
    fn the_home_screens_own_shape_is_under_the_size_of_everything_else() {
        let rows = screen();
        let named = from(&rows, &Word::HomeScreen.english());
        let ladder = from(&rows, &Word::DisplayZoom.english());

        assert!(!ladder.is_empty(), "the sizes are named");
        assert!(ladder.len() > named.len(), "the home screen is under the ladder, not over it");
        assert!(named.first().expect("the home screen is named").naming, "the name is a row the highlight can land on");
        assert_eq!(
            says(&named[1..]),
            [
                Word::Columns.english(),
                Word::Rows.english(),
                Word::IconSize.english(),
            ]
        );
    }

    #[test]
    fn the_home_screens_rows_say_what_they_are_at_and_can_all_be_moved() {
        let Ok(wide) = Shape::USUAL.with_columns(7);

        let Ok(deep) = wide.with_rows(4);

        let Ok(shape) = deep.sized(console_home_screen::shape::Size::Bigger);

        let Ok(rows) = home_rows(shape, nothing(), nothing(), nothing());
        let moved: Vec<&Row> = rows.iter().filter(|row| row.level.is_some()).collect();

        assert_eq!(moved.len(), 3, "one of them cannot be moved");
        assert!(moved.iter().all(|row| !row.aside.is_empty()), "one of them says nothing");
        assert_eq!(moved[0].aside, "7");
        assert_eq!(moved[1].aside, "4");
        assert_eq!(moved[2].aside, Word::SizeLarger.english());
    }

    #[test]
    fn the_way_up_the_screen_stands_is_the_one_marked_and_the_three_are_offered() {
        let Ok(rows) =
            screen_rows(Some(50), nothing(), None, NightShift::Off, None, Some(Turn::Left), grid());
        let named = from(&rows, &Word::Rotation.english());

        assert!(named.first().expect("the ways up are named").naming, "the name is a row the highlight can land on");
        assert_eq!(
            says(&named[1..4]),
            [Word::RotatedLeft.english(), Word::Standard.english(), Word::RotatedRight.english()]
        );
        assert_eq!(marked(&rows), [Word::RotatedLeft.english()]);
    }

    #[test]
    fn the_size_the_screen_is_at_is_the_one_marked() {
        let rows = sized(Some(Size::Bigger));
        assert_eq!(marked(&rows), [Word::SizeLarger.english()]);
    }

    #[test]
    fn a_screen_at_a_size_of_its_own_marks_none_of_the_rungs() {
        let rows = sized(None);
        assert!(
            !rows.iter().any(|row| now(row) == Active::Yes),
            "something is marked"
        );
        assert_eq!(
            rows.iter().filter(|row| row.does.is_some()).count(),
            EVERY.len() + turning::EVERY.len() + 1,
            "a rung or a way up went missing"
        );
    }

    #[test]
    fn where_the_battery_is_watched_is_three_rows_that_can_be_moved() {
        let Ok(rows) = dwindling(battery::Levels::default(), |_| Ok(nothing()));
        assert_eq!(
            says(&rows),
            [
                Word::LowBattery.english(),
                Word::AlertAt.english(),
                Word::AlertAgainAt.english(),
                Word::ShutDownAt.english(),
            ]
        );
        assert!(rows[1..].iter().all(|row| row.level.is_some()), "a threshold that cannot be moved");
        assert_eq!(rows[1].aside, "25%");
    }

    #[test]
    fn a_threshold_turned_off_says_so_in_a_word() {
        assert_eq!(threshold(battery::NEVER), Ok(Word::Never.english()));
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

        assert_eq!(says(&rows), [BLUETOOTH, "Near", "Middling", "Far", LOOKING]);
    }

    #[test]
    fn a_device_that_only_said_its_address_is_drawn_under_the_ones_that_said_a_name() {
        let Ok(devices) = bluetooth::devices(
            "Device AA:BB:CC:DD:EE:FF AA-BB-CC-DD-EE-FF\nDevice 11:22:33:44:55:66 Blue Keys",
        );
        let rows = while_looking(vec![heard(&devices[0], -40), heard(&devices[1], -95)]);

        assert_eq!(says(&rows), [
            BLUETOOTH,
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
    fn a_radio_that_is_off_says_so_beside_its_own_name_rather_than_in_it() {
        let off = wifi_of(wifi::Radio::Off, Vec::new());
        let on = wifi_of(wifi::Radio::On, Vec::new());

        assert_eq!(says(&off), [WIFI]);
        assert_eq!(says(&on).first().copied(), Some(WIFI));
        assert_eq!(off.first().map(|row| row.aside.as_str()), Some(OFF));
        assert_eq!(on.first().map(|row| row.aside.as_str()), Some(ON));

        let dark = bluetooth_of(bluetooth::Radio::Off, Vec::new());

        assert_eq!(says(&dark), [BLUETOOTH]);
        assert_eq!(dark.first().map(|row| row.aside.as_str()), Some(OFF));

        assert!(
            dark.first().is_some_and(|row| row.does.is_some()),
            "a row that says Off is still the row that turns it on"
        );
    }

    #[test]
    fn the_one_we_are_on_is_marked_rather_than_offered() {
        let Ok(networks) = wifi::networks("yes:Home:71:2437 MHz:WPA2\nno:Cafe:50:2437 MHz:");
        let rows = wifi_of(wifi::Radio::On, networks);
        let home = rows.iter().find(|row| row.says == "Home").expect("home");
        assert_eq!(now(home), Active::Yes);
        assert!(home.does.is_none(), "there is nothing to do about being where you are");
        let cafe = rows.iter().find(|row| row.says == "Cafe").expect("cafe");
        assert!(cafe.does.is_some());
    }

    #[test]
    fn the_network_we_are_on_offers_its_code_on_y_and_no_other_does() {
        let Ok(networks) = wifi::networks("yes:Home:71:2437 MHz:WPA2\nno:Cafe:50:2437 MHz:WPA2");
        let rows = wifi_of(wifi::Radio::On, networks);
        let offered: Vec<&str> = rows.iter().filter(|row| row.more.is_some()).map(|row| row.says.as_str()).collect();
        let said: Vec<&str> = rows.iter().map(|row| row.says.as_str()).collect();

        assert_eq!(offered, ["Home"]);
        assert_eq!(said, [WIFI, "Home", "Cafe"], "the code is a choice on Y rather than a row of its own");
    }

    #[test]
    fn a_joined_device_is_marked_and_offers_the_way_out_of_it() {
        let Ok(devices) = bluetooth::devices("Device AA Pads\nDevice BB Speaker");
        let rows = bluetooth_of(bluetooth::Radio::On, vec![
            met(&devices[0], bluetooth::Known::Yes, bluetooth::Joined::Yes),
            met(&devices[1], bluetooth::Known::Yes, bluetooth::Joined::No),
        ]);
        assert_eq!(now(rows.iter().find(|row| row.says == "Pads").expect("pads")), Active::Yes);
        assert_eq!(
            now(rows.iter().find(|row| row.says == "Speaker").expect("speaker")),
            Active::No
        );
    }

    #[test]
    fn a_device_no_one_has_been_introduced_to_is_not_offered_a_word_bluez_would_refuse() {
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
            [&back_to("Blue Keys"), PAIR]
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
    fn a_row_that_acts_on_the_device_the_page_names_does_not_name_it_again() {
        let Ok(devices) = bluetooth::devices("Device AA Blue Keys");

        for known in [bluetooth::Known::No, bluetooth::Known::Yes] {
            for row in says(&meeting(&met(&devices[0], known, bluetooth::Joined::No))).iter().skip(1)
            {
                assert!(
                    !row.contains("Blue Keys"),
                    "{row:?} says the name the page it is on is already titled with"
                );
            }
        }
    }

    #[test]
    fn the_word_for_going_ahead_is_short_and_stands_for_nothing() {
        for word in [FORGET_YES, JOIN, LEAVE, PAIR] {
            let words: Vec<&str> = word.split_whitespace().collect();

            assert!(words.len() <= 2, "{word:?} is a sentence where a verb was wanted");

            for standing in ["it", "them", "this", "that", "these", "those"] {
                assert!(
                    !words.iter().any(|said| said.to_lowercase() == standing),
                    "{word:?} says {standing:?} where the question already said the thing"
                );
            }
        }
    }

    #[test]
    fn every_device_row_is_the_row_its_own_page_comes_back_to() {
        let Ok(devices) = bluetooth::devices("Device AA One\nDevice BB Two\nDevice CC Three");
        let seen: Arc<std::sync::Mutex<Vec<u32>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
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
                Some(Handler::Call(act)) => {
                    let _ = act(&Nowhere);
                }
                Some(Handler::Run(_)) | None => {},
            }
        }

        let seen = match seen.lock() {
            Ok(seen) => seen.clone(),
            Err(_) => Vec::new(),
        };
        let standing: Vec<u32> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| devices.iter().any(|device| device.name == row.says))
            .map(|(at, _)| at as u32)
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
        assert_eq!(rows[1].says, "Search Engine");
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
        assert_eq!(says(&rows)[2..], ["Automatic", "English", "Dutch", "Thai"]);
    }

    #[test]
    fn the_languages_are_a_list_under_the_tab_like_the_engines() {
        let rows = listening("auto");
        let Ok(tab) = the_language();
        let Ok(listens) = text(&Word::DictationLanguage);

        assert!(rows[0].says.ends_with(&tab), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, listens);
        assert_eq!(heading(&rows[1]), Heading::Yes);
    }

    #[test]
    fn the_language_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(dictation_says("th"), Ok("Thai".to_string()));
        assert_eq!(dictation_says("auto"), Ok("Automatic".to_string()));
        assert_eq!(dictation_says("zh"), Ok(String::new()));
    }


    fn spoken() -> Vec<Language> {
        let Ok(names) = Names::none();
        let Ok(supported) = crate::languages::supported(
            "en_GB.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\nnl_NL.UTF-8 UTF-8\nth_TH.UTF-8 UTF-8",
        );
        let Ok(spoken) = crate::languages::languages(&supported, &names);

        spoken
    }

    type Recorded = Arc<std::sync::Mutex<Vec<(u32, String)>>>;

    fn opening(_: u32, _: &str) -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn watching() -> (Recorded, impl Fn(u32, &str) -> Result<Handler, Never>) {
        let seen: Recorded = Arc::new(std::sync::Mutex::new(Vec::new()));
        let telling = Arc::clone(&seen);

        (seen, move |at, name| {
            match telling.lock() {
                Ok(mut telling) => telling.push((at, name.to_string())),
                Err(_the_test_that_held_it_failed) => {},
            }

            Handler::and_stay(|_| ())
        })
    }

    fn opened(seen: &Recorded) -> Vec<(u32, String)> {
        match seen.lock() {
            Ok(mut seen) => std::mem::take(&mut seen),
            Err(_the_test_that_held_it_failed) => Vec::new(),
        }
    }

    fn language() -> Vec<Row> {
        let Ok(nothing) = Handler::and_stay(|_| ());
        let Ok(also) = Handler::and_stay(|_| ());
        let Ok(third) = Handler::and_stay(|_| ());
        let Ok(rows) = language_rows(
            Languages {
                says: "English (United Kingdom)",
                types: "Latin, Thai",
                listens: "Dutch",
            },
            [nothing, also, third],
        );

        rows
    }

    #[test]
    fn the_language_tab_is_three_rows_that_open_and_one_that_says_what_it_does_not_do() {
        let rows = language();
        let Ok(words) = text(&Word::PreferredLanguage);
        let Ok(types) = text(&Word::Keyboards);
        let Ok(listens) = text(&Word::DictationLanguage);

        assert_eq!(says(&rows)[..3], [words.as_str(), types.as_str(), listens.as_str()]);

        for row in rows.iter().take(3) {
            assert_eq!(row.acts(), Ok(console_panel::page::Action::Yes), "{:?}", row.says);
        }

        let Ok(english) = text(&Word::EnglishOnly);

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
        let Ok(rows) = language_picker_rows(&spoken(), None, nowhere(), opening);
        let Ok(tab) = the_language();

        assert!(rows[0].says.ends_with(&tab), "{:?} is not the way back", rows[0].says);
        assert_eq!(heading(&rows[1]), Heading::Yes);
        assert_eq!(says(&rows)[2..], ["en", "nl", "th"]);
    }

    #[test]
    fn every_language_carries_the_row_it_was_opened_from_so_b_lands_back_on_it() {
        let (seen, opening) = watching();
        let Ok(rows) = language_picker_rows(&spoken(), None, nowhere(), opening);
        let at = opened(&seen);

        for (which, standing) in at.iter().enumerate() {
            let says = rows.get(console_core_number_conversion::index(standing.0).unwrap()).map(|row| row.says.as_str());

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
            let says = rows.get(console_core_number_conversion::index(standing.0).unwrap()).map(|row| row.says.as_str());

            assert_eq!(says, Some(standing.1.as_str()), "opened from row {}", standing.0);
        }
    }

    #[test]
    fn the_language_the_machine_is_in_is_the_one_marked() {
        let spoken = spoken();
        let Ok(standing) = crate::languages::standing(&spoken, Some("nl_NL.UTF-8"));
        let Ok(rows) = language_picker_rows(&spoken, standing.as_ref(), nowhere(), opening);

        assert_eq!(marked(&rows), ["nl"]);
    }

    #[test]
    fn the_places_a_language_is_spoken_are_a_list_under_it() {
        let spoken = spoken();
        let Ok(names) = Names::none();
        let english = spoken.iter().find(|language| language.language == "en").expect("English");
        let Ok(standing) = crate::languages::standing(&spoken, Some("en_GB.UTF-8"));
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
        assert_eq!(rows[2].acts(), Ok(console_panel::page::Action::None));
        assert_eq!(rows[3].acts(), Ok(console_panel::page::Action::Yes));
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
        let Ok(asks) = text(&Word::Name);

        assert_eq!(row.says, asks);
        assert_eq!(row.aside, "legion");
        assert_eq!(row.acts(), Ok(console_panel::page::Action::Yes));
    }

    #[test]
    fn every_tab_is_named_once() {
        let mut sorted = named().to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), named().len());
    }
}
