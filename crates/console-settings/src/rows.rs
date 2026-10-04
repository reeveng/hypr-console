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
use console_lock_screen::LockScreen;
use console_login_window::stored_pattern::StoredPattern;
use console_home_screen::shape::Shape;
use console_core_never::Never;
use console_core_localization::text;

use crate::words::Word;
use console_notifications::reading::DoNotDisturb;
use console_response_times::measuring::{self, Measuring};
use console_books::appearance::{self, Appearance, Paint, ColorRole, Typeface, TYPEFACES};
use console_panel::page::{Aside, ButtonPress, Handler, Active, Level, NOW, Row, Showing, Subject, YET};
use crate::introducing;
use console_input_dictation::languages;

use console_input_alphabets::{self as alphabets, Alphabet};

use console_default_applications::clock::{self, Clock};

use crate::hours::{self, Place};
use crate::level::{CELLS, Muted, bar, volume};
use crate::named::{self, Allowed};
use crate::languages::{Locale, Made, Names, Language, generation_state};
use crate::size::{EVERY, Size};
use crate::turning::{self, Turn};
use crate::learned::Following;
use crate::machine::MachineSetting;
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
            let Ok(leveled) = row.with_level(turning);

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
        let Ok(said) = stream.display_name();

        let Ok(silence) = hush(stream.index, "sink-input");
        let Ok(row) = Row::new(&said, Aside(&level), silence);
        let Ok(turning) = turn(stream.index, "sink-input");
        let Ok(leveled) = row.with_level(turning);

        rows.push(leveled);
    }

    match playing.is_empty() {
        true => {
            let Ok(row) = Row::placeholder("Nothing else is playing");

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
    let Ok(background) = Row::placeholder("Background");
    let Ok(pages) = swatches(ColorRole::Background, now.background);
    let Ok(text) = Row::placeholder("Text");
    let Ok(inks) = swatches(ColorRole::Text, now.text);
    let Ok(font) = Row::placeholder("Font");
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
        let Ok(row) = Row::text(&words, Aside(&at));
        let Ok(guarding) = guard(step);
        let Ok(leveled) = row.with_level(guarding);

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
    let Ok(columns) = Row::text(&applications_across, Aside(&shape.columns.to_string()));
    let Ok(columns) = columns.with_level(across);
    let Ok(applications_down) = text(&Word::Rows);
    let Ok(down_row) = Row::text(&applications_down, Aside(&shape.rows.to_string()));
    let Ok(down_row) = down_row.with_level(down);
    let Ok(word) = word_of_home_size(shape.size);
    let Ok(how_big_they_are) = text(&Word::IconSize);
    let Ok(words) = text(&word);
    let Ok(big) = Row::text(&how_big_they_are, Aside(&words));
    let Ok(big) = big.with_level(sized);

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
    let Ok(bright) = Row::text(&screen_brightness, Aside(&level));
    let Ok(bright) = bright.with_level(dim);
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
                let Ok(row) = Row::text(&name, Aside(NOW));
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
        bluetooth::Looking::Yes => Row::text(LOOKING, Aside(YET)),
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
    let Ok(introduce) = introducing::Bluetooth::Introduce.word();

    Ok(vec![INTRODUCES.to_string(), introduce.to_string(), address.to_string()])
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
            let Ok(telling) = set_engine_command(key);

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

pub fn set_engine_command(engine: &str) -> Result<Vec<String>, Never> {
    Program::Sudo.arguments(&["-n", "console-engine", engine])
}

pub fn power_rows() -> Result<Vec<Row>, Never> {
    let Ok(systemctl) = Program::Systemctl.name();
    let suspending = vec![systemctl.to_string(), "suspend".to_string()];
    let Ok(suspends) = Handler::call(move |_| {
        let Ok(()) = console_music::player::pause();
        let Ok(()) = console_panel::running::left_running(&suspending);

        true
    });
    let Ok(sleep) = Row::new("Sleep", Aside(""), suspends);
    let Ok(reboots) = Handler::run(&[systemctl, "reboot"]);
    let Ok(restart) = Row::new("Restart", Aside(""), reboots);
    let Ok(powers_off) = Handler::run(&[systemctl, "poweroff"]);
    let Ok(shut_down) = Row::new("Shut Down", Aside(""), powers_off);
    let mut rows = vec![sleep, restart, shut_down];
    let Ok(locks) = locking();

    match locks {
        Locks::Yes => {
            let Ok(loginctl) = Program::Loginctl.name();
            let Ok(locking) = Handler::run(&[loginctl, "lock-session"]);
            let Ok(lock) = Row::new(LOCK_NOW, Aside(""), locking);

            rows.push(lock);
        }
        Locks::No => {}
    }

    Ok(rows)
}

const LOCK_NOW: &str = "Lock Now";

const LOCK_SCREEN: &str = "Lock Screen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Locks {
    Yes,
    No,
}

fn locking() -> Result<Locks, Never> {
    let Ok(home) = console_core_places::home();
    let stored = home.as_deref().map(console_login_window::stored_pattern::stored);
    let Ok(chosen) = console_lock_screen::current();

    Ok(match stored {
        Some(Ok(stored)) => {
            let Ok(wanted) = console_lock_screen::desired(stored, chosen);

            match wanted {
                console_lock_screen::Wanted::Lock(_) => Locks::Yes,
                console_lock_screen::Wanted::Absent => Locks::No,
            }
        }
        Some(Err(_unreadable)) => Locks::No,
        None => Locks::No,
    })
}

pub fn lock_screen(chosen: LockScreen) -> Result<Row, Never> {
    let state = match chosen {
        LockScreen::On => ON,
        LockScreen::Off => OFF,
    };
    let Ok(turning) = Handler::and_stay(move |showing| {
        let Ok(turned) = chosen.flipped();
        let Ok(()) = console_lock_screen::choose(turned);

        showing.refresh();
    });

    Row::new(LOCK_SCREEN, Aside(state), turning)
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
            let Ok(chosen) = console_lock_screen::current();
            let Ok(lock) = lock_screen(chosen);
            let Ok(off) = Row::new(TURN_OFF_LOGIN_PATTERN, Aside(""), forgets);

            Ok(vec![change, lock, off])
        }
        Some(Ok(StoredPattern::Absent)) => {
            let Ok(set) = Row::new(LOGIN_PATTERN, Aside(OFF), chooses);

            Ok(vec![set])
        }
        Some(Err(why)) => {
            let Ok(unread) = Row::text(LOGIN_PATTERN, Aside(&why.to_string()));

            Ok(vec![unread])
        }
        None => Ok(Vec::new()),
    }
}

const MEASUREMENTS: &str = "Measurements";

pub fn measurements(measuring: Measuring) -> Result<Row, Never> {
    let state = match measuring {
        Measuring::On => ON,
        Measuring::Off => OFF,
    };
    let Ok(turning) = Handler::and_stay(move |showing| {
        let Ok(turned) = measuring.flipped();
        let Ok(()) = measuring::choose(turned);

        showing.refresh();
    });

    Row::new(MEASUREMENTS, Aside(state), turning)
}

pub fn security_rows() -> Result<Vec<Row>, Never> {
    let Ok(mut rows) = login_rows();
    let Ok(measuring) = measuring::current();
    let Ok(switch) = measurements(measuring);

    rows.push(switch);

    Ok(rows)
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
    let Ok(arguments) = console_notifications::serving::call_arguments(QUIETEN);
    let mut said = vec![busctl];

    said.extend(arguments.iter().map(String::as_str));

    let Ok(row) = switch(says, Aside(mark), &said);

    let Ok(bell) = Row::text(BELL, Aside("All, even silenced"));

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
    let Ok(standing) = Row::placeholder(&english);

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

        let Ok(made) = generation_state(generated, locale);

        let note = match made {
            Made::Yes => None,
            Made::No => {
                let Ok(making) = text(&Word::PreparingLanguage);

                Some(making)
            }
        };

        let Ok(language) = MachineSetting::Language.word();
        let Ok(arguments) = Program::Sudo.arguments(&[
            "-n",
            "console-machine",
            language,
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
                let Ok(row) = Row::text(alphabet.says, Aside(mark));

                rows.push(row);
            }
            false => {
                let Ok(turned) = alphabets::toggle(chosen, alphabet.key);
                let Ok(words) = Program::Systemctl.arguments(&["--user", "restart", KEYBOARD]);
                let Ok(turns) = Handler::and_stay(move |showing| {
                    let Ok(()) = alphabets::choose(&turned);

                    showing.later(words.clone());
                    showing.later(vec![
                        console_input_language::NAMED.to_string(),
                        console_input_language::SETTLE.spelling.to_string(),
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
        let Ok(hour) = MachineSetting::Hour.word();
        let Ok(arguments) = Program::Sudo.arguments(&["-n", "console-machine", hour, &place.zone]);
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
                        let Ok(name) = MachineSetting::Name.word();
                        let Ok(arguments) =
                            Program::Sudo.arguments(&["-n", "console-machine", name, word]);

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
    use std::collections::BTreeSet;

    use console_core_localization::Localized;
    use console_core_number_conversion::{fitted, index};
    use console_panel::page::{Action, Heading, Active, Nowhere};
    use super::*;

    type Failure = Box<dyn std::error::Error>;

    type Standing = (bluetooth::Known, bluetooth::Joined, Option<i32>);

    type Recorded = Arc<std::sync::Mutex<Vec<(u32, String)>>>;

    trait Opening: Fn(u32, &str) -> Result<Handler, Never> {}

    impl<F: Fn(u32, &str) -> Result<Handler, Never>> Opening for F {}

    const STRANGER: Standing = (bluetooth::Known::No, bluetooth::Joined::No, None);

    const FRIEND: Standing = (bluetooth::Known::Yes, bluetooth::Joined::No, None);

    const JOINED: Standing = (bluetooth::Known::Yes, bluetooth::Joined::Yes, None);

    fn nothing() -> Result<Level, Never> {
        Ok(Arc::new(|_| ()))
    }

    fn nowhere() -> Result<Chosen, Never> {
        Ok(Arc::new(|_: &dyn Showing| ()))
    }

    fn grid() -> Result<Vec<Row>, Never> {
        let Ok(rows) = nothing();
        let Ok(columns) = nothing();
        let Ok(size) = nothing();

        home_rows(Shape::USUAL, rows, columns, size)
    }

    fn silence(_: i64, _: &'static str) -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn turning(_: i64, _: &'static str) -> Result<Level, Never> {
        nothing()
    }

    fn opening(_: u32, _: &str) -> Result<Handler, Never> {
        Handler::and_stay(|_| ())
    }

    fn says(rows: &[Row]) -> Result<Vec<&str>, Never> {
        Ok(rows.iter().map(|row| row.says.as_str()).collect())
    }

    fn marked(rows: &[Row]) -> Result<Vec<&str>, Never> {
        Ok(rows.iter().filter(|row| row.now() == Ok(Active::Yes)).map(|row| row.says.as_str()).collect())
    }

    fn row<'a>(rows: &'a [Row], said: &str) -> Result<&'a Row, Failure> {
        let found = rows.iter().find(|row| row.says == said).ok_or(format!("no row says {said:?}"))?;

        Ok(found)
    }

    fn at(rows: &[Row], place: u32) -> Result<&Row, Failure> {
        let Ok(place_at) = index(place);
        let found = rows.get(place_at).ok_or(format!("no row {place} among {}", rows.len()))?;

        Ok(found)
    }

    fn part(rows: &[Row], (from, to): (u32, u32)) -> Result<&[Row], Failure> {
        let Ok(start) = index(from);
        let Ok(end) = index(to);
        let part = rows.get(start..end).ok_or(format!("no rows {from} to {to} among {}", rows.len()))?;

        Ok(part)
    }

    fn after(rows: &[Row], skipped: u32) -> Result<&[Row], Failure> {
        let Ok(start) = index(skipped);
        let part = rows.get(start..).ok_or(format!("no rows after {skipped} among {}", rows.len()))?;

        Ok(part)
    }

    fn from<'a>(rows: &'a [Row], said: &str) -> Result<&'a [Row], Never> {
        Ok(match rows.iter().position(|row| row.says == said) {
            Some(start) => match rows.get(start..) {
                Some(rest) => rest,
                None => &[],
            },
            None => &[],
        })
    }

    fn sound(sinks: &[sound::Thing], default: &str) -> Result<Vec<Row>, Never> {
        sound_rows(sinks, &[], default, silence, turning)
    }

    fn wifi_of(on: wifi::Radio, networks: Vec<wifi::Network>) -> Result<Vec<Row>, Never> {
        wifi_rows(on, networks, &[], |_, _| silence(0, ""), |_| Ok(Arc::new(|_: &dyn Showing| ())))
    }

    fn bluetooth_of(on: bluetooth::Radio, met: Vec<bluetooth::Met>) -> Result<Vec<Row>, Never> {
        bluetooth_rows(on, bluetooth::Looking::No, met, Arc::new(|_, _, _| ()))
    }

    fn while_looking(met: Vec<bluetooth::Met>) -> Result<Vec<Row>, Never> {
        bluetooth_rows(bluetooth::Radio::On, bluetooth::Looking::Yes, met, Arc::new(|_, _, _| ()))
    }

    fn met(devices: &[bluetooth::Device], standing: &[Standing]) -> Result<Vec<bluetooth::Met>, Never> {
        Ok(devices
            .iter()
            .zip(standing)
            .map(|(device, (known, joined, heard))| bluetooth::Met {
                device: device.clone(),
                known: *known,
                joined: *joined,
                heard: *heard,
            })
            .collect())
    }

    fn heard(devices: &[bluetooth::Device], loudness: &[i32]) -> Result<Vec<bluetooth::Met>, Never> {
        let standing: Vec<Standing> =
            loudness.iter().map(|heard| (bluetooth::Known::No, bluetooth::Joined::No, Some(*heard))).collect();

        met(devices, &standing)
    }

    fn meeting(device: &bluetooth::Device, standing: Standing) -> Result<Vec<String>, Failure> {
        let Ok(nowhere) = nowhere();
        let Ok(met) = met(std::slice::from_ref(device), &[standing]);
        let met = met.first().ok_or("the device was not met")?;
        let Ok(rows) = meeting_rows(met, nowhere);

        Ok(rows.into_iter().map(|row| row.says).collect())
    }

    fn back_to(says: &str) -> Result<String, Never> {
        let Ok(row) = Row::back(says, |_| ());

        Ok(row.says)
    }

    fn searching(engine: &str) -> Result<Vec<Row>, Never> {
        let Ok(nowhere) = nowhere();

        search_rows(engine, nowhere)
    }

    fn listening_for(language: &str) -> Result<Vec<Row>, Never> {
        let Ok(nowhere) = nowhere();

        dictation_rows(language, nowhere)
    }

    fn sized(standing: Option<Size>) -> Result<Vec<Row>, Never> {
        let Ok(level) = nothing();
        let Ok(grid) = grid();

        screen_rows(Some(50), level, None, NightShift::Off, standing, None, grid)
    }

    fn battery() -> Result<Vec<Row>, Never> {
        battery_rows(Some("balanced"), battery::Levels::default(), |_| nothing())
    }

    fn sample_languages() -> Result<Vec<Language>, Never> {
        let Ok(names) = Names::none();
        let Ok(supported) = crate::languages::supported(
            "en_GB.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\nnl_NL.UTF-8 UTF-8\nth_TH.UTF-8 UTF-8",
        );

        crate::languages::languages(&supported, &names)
    }

    fn recorder() -> Result<(Recorded, impl Opening), Never> {
        let seen: Recorded = Arc::new(std::sync::Mutex::new(Vec::new()));
        let telling = Arc::clone(&seen);

        Ok((seen, move |at: u32, name: &str| {
            match telling.lock() {
                Ok(mut telling) => telling.push((at, name.to_string())),
                Err(_the_test_that_held_it_failed) => {},
            }

            Handler::and_stay(|_| ())
        }))
    }

    fn opened(seen: &Recorded) -> Result<Vec<(u32, String)>, Never> {
        Ok(match seen.lock() {
            Ok(mut seen) => std::mem::take(&mut seen),
            Err(_the_test_that_held_it_failed) => Vec::new(),
        })
    }

    fn opened_from(rows: &[Row], seen: &[(u32, String)]) -> Result<(), Never> {
        for (place, says) in seen {
            let Ok(place_at) = index(*place);
            let standing = rows.get(place_at).map(|row| row.says.as_str());

            assert_eq!(standing, Some(says.as_str()), "opened from row {place}, and that row is {standing:?}");
        }

        Ok(())
    }

    fn language() -> Result<Vec<Row>, Never> {
        let Ok(nothing) = Handler::and_stay(|_| ());
        let Ok(also) = Handler::and_stay(|_| ());
        let Ok(third) = Handler::and_stay(|_| ());

        language_rows(
            Languages {
                says: "English (United Kingdom)",
                types: "Latin, Thai",
                listens: "Dutch",
            },
            [nothing, also, third],
        )
    }

    #[test]
    fn measurements_say_whether_they_are_on() {
        let Ok(off) = measurements(Measuring::Off);
        let Ok(on) = measurements(Measuring::On);

        assert_eq!((off.says.as_str(), off.aside.as_str()), ("Measurements", OFF));
        assert_eq!(on.aside, ON);
        assert!(off.does.is_some(), "the row is a switch someone can press");
    }

    #[test]
    fn sound_effects_say_whether_they_are_on() {
        let Ok(off) = sound_effects(SoundEffects::Off);
        let Ok(on) = sound_effects(SoundEffects::On);

        assert_eq!((off.says.as_str(), off.aside.as_str()), ("Sound Effects", OFF));
        assert_eq!(on.aside, ON);
        assert!(off.does.is_some(), "the row is a switch someone can press");
    }

    #[test]
    fn the_two_things_that_are_held_at_a_level_are_on_a_panel() -> Result<(), Failure> {
        let Ok(screen) = sized(Some(Size::Normal));
        let Ok(sinks) = sound::read(r#"[{"index": 1, "name": "a", "volume": {}}]"#);
        let Ok(speakers) = sound(&sinks, "a");

        let screen_first = at(&screen, 0)?;
        assert!(screen_first.level.is_some(), "the screen is not a level");
        let speakers_first = at(&speakers, 0)?;
        assert!(speakers_first.level.is_some(), "the speakers are not a level");

        Ok(())
    }

    #[test]
    fn the_profile_in_use_is_the_one_marked() {
        let Ok(rows) = battery_rows(Some("performance"), battery::Levels::default(), |_| nothing());
        let high = Word::HighPower.english();

        assert_eq!(marked(&rows), Ok(vec![high.as_str()]));
    }

    #[test]
    fn the_battery_tab_opens_on_stopping_the_machine() -> Result<(), Failure> {
        let Ok(rows) = battery();
        let rows_part = part(&rows, (0, 3))?;
        let Ok(said) = says(rows_part);

        assert_eq!(said, ["Sleep", "Restart", "Shut Down"]);

        Ok(())
    }

    #[test]
    fn the_three_speeds_are_a_named_scale_with_the_least_of_them_first() -> Result<(), Failure> {
        let Ok(rows) = battery();
        let named = at(&rows, 3)?;
        let rows_part = part(&rows, (4, 7))?;
        let Ok(speeds) = says(rows_part);

        assert!(named.naming, "the speeds are not named");
        assert_eq!(named.says, Word::PowerMode.english());
        assert_eq!(speeds, [Word::LowPower.english(), Word::Automatic.english(), Word::HighPower.english()]);

        Ok(())
    }

    #[test]
    fn the_books_tab_marks_the_page_the_ink_and_the_face_that_were_chosen() -> Result<(), Failure> {
        let Ok(grid) = appearance::grid();
        let page = grid.get(2).and_then(|row| row.get(3)).ok_or("the grid has a third row of four")?;
        let now = Appearance { background: Paint::Chosen(*page), text: Paint::Desktop, typeface: Typeface::Monospaced, ..Appearance::default() };
        let Ok(rows) = books_rows(now);
        let marked: Vec<&str> = rows.iter().filter(|row| row.aside == NOW).map(|row| row.says.as_str()).collect();

        assert_eq!(marked, ["Default", "Monospaced"], "the ink is the desktop's and the face was chosen");

        let lit: Vec<(u32, console_panel::page::Face)> = rows
            .iter()
            .filter_map(|row| row.buttons.as_ref())
            .flat_map(|across| across.presses.iter().filter(|press| press.now == Active::Yes).map(move |press| (across.at, press.face)))
            .collect();

        assert_eq!(lit, [(3, console_panel::page::Face::Swatch(*page))], "the page chosen is the one swatch lit, and it is where the row starts");
        assert_eq!(rows.len(), grid.len().saturating_add(1).saturating_mul(2).saturating_add(3).saturating_add(TYPEFACES.len()));

        Ok(())
    }

    #[test]
    fn the_screen_and_how_hard_the_machine_works_are_two_tabs() {
        let Ok(rows) = battery();
        let Ok(said) = says(&rows);
        let battery = said.join("\n");

        for screen in [Word::Brightness.english(), Word::DisplayZoom.english()] {
            assert!(!battery.contains(&screen), "{screen:?} is still on the Battery tab");
        }

        assert!(!battery.contains("night colors"), "the evening is still on the Battery tab");
    }

    #[test]
    fn the_sizes_are_a_named_scale_with_the_smallest_of_them_first() -> Result<(), Failure> {
        let Ok(rows) = sized(Some(Size::Normal));
        let Ok(named) = from(&rows, &Word::DisplayZoom.english());
        let Ok(rungs) = fitted::<_, u32>(EVERY.len());
        let rungs_part = part(named, (1, rungs.saturating_add(1)))?;
        let Ok(ladder) = says(rungs_part);

        let named_first = at(named, 0)?;
        assert!(named_first.naming, "the name is a row the highlight can land on");
        assert_eq!(
            ladder,
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

        Ok(())
    }

    #[test]
    fn the_home_screens_own_shape_is_under_the_size_of_everything_else() -> Result<(), Failure> {
        let Ok(rows) = sized(Some(Size::Normal));
        let Ok(named) = from(&rows, &Word::HomeScreen.english());
        let Ok(ladder) = from(&rows, &Word::DisplayZoom.english());
        let named_after = after(named, 1)?;
        let Ok(shape) = says(named_after);

        assert!(!ladder.is_empty(), "the sizes are named");
        assert!(ladder.len() > named.len(), "the home screen is under the ladder, not over it");
        let named_first = at(named, 0)?;
        assert!(named_first.naming, "the name is a row the highlight can land on");
        assert_eq!(shape, [Word::Columns.english(), Word::Rows.english(), Word::IconSize.english()]);

        Ok(())
    }

    #[test]
    fn the_home_screens_rows_say_what_they_are_at_and_can_all_be_moved() -> Result<(), Failure> {
        let Ok(wide) = Shape::USUAL.with_columns(7);

        let Ok(deep) = wide.with_rows(4);

        let Ok(shape) = deep.sized(console_home_screen::shape::Size::Bigger);

        let Ok(across) = nothing();
        let Ok(down) = nothing();
        let Ok(size) = nothing();
        let Ok(rows) = home_rows(shape, across, down, size);
        let moved: Vec<&str> = rows.iter().filter(|row| row.level.is_some()).map(|row| row.aside.as_str()).collect();

        assert_eq!(moved, ["7", "4", Word::SizeLarger.english().as_str()], "one of them cannot be moved or says nothing");

        Ok(())
    }

    #[test]
    fn the_way_up_the_screen_stands_is_the_one_marked_and_the_three_are_offered() -> Result<(), Failure> {
        let Ok(level) = nothing();
        let Ok(grid) = grid();
        let Ok(rows) = screen_rows(Some(50), level, None, NightShift::Off, None, Some(Turn::Left), grid);
        let Ok(named) = from(&rows, &Word::Rotation.english());
        let named_part = part(named, (1, 4))?;
        let Ok(ways) = says(named_part);
        let left = Word::RotatedLeft.english();

        let named_first = at(named, 0)?;
        assert!(named_first.naming, "the name is a row the highlight can land on");
        assert_eq!(ways, [Word::RotatedLeft.english(), Word::Standard.english(), Word::RotatedRight.english()]);
        assert_eq!(marked(&rows), Ok(vec![left.as_str()]));

        Ok(())
    }

    #[test]
    fn the_size_the_screen_is_at_is_the_one_marked() {
        let Ok(rows) = sized(Some(Size::Bigger));
        let larger = Word::SizeLarger.english();

        assert_eq!(marked(&rows), Ok(vec![larger.as_str()]));
    }

    #[test]
    fn a_screen_at_a_size_of_its_own_marks_none_of_the_rungs() {
        let Ok(rows) = sized(None);

        assert_eq!(marked(&rows), Ok(Vec::new()), "something is marked");
        assert_eq!(
            rows.iter().filter(|row| row.does.is_some()).count(),
            EVERY.len().saturating_add(turning::EVERY.len()).saturating_add(1),
            "a rung or a way up went missing"
        );
    }

    #[test]
    fn where_the_battery_is_watched_is_three_rows_that_can_be_moved() -> Result<(), Failure> {
        let Ok(rows) = dwindling(battery::Levels::default(), |_| nothing());
        let Ok(said) = says(&rows);

        assert_eq!(
            said,
            [
                Word::LowBattery.english(),
                Word::AlertAt.english(),
                Word::AlertAgainAt.english(),
                Word::ShutDownAt.english(),
            ]
        );
        let rows_after = after(&rows, 1)?;
        assert!(rows_after.iter().all(|row| row.level.is_some()), "a threshold that cannot be moved");
        let second = at(&rows, 1)?;
        assert_eq!(second.aside, "25%");

        Ok(())
    }

    #[test]
    fn a_threshold_turned_off_says_so_in_a_word() {
        assert_eq!(threshold(battery::NEVER), Ok(Word::Never.english()));
        assert_eq!(threshold(5), Ok("5%".to_string()));
    }

    #[test]
    fn a_radio_that_is_looking_says_so_where_the_press_that_starts_it_was() -> Result<(), Failure> {
        let Ok(looking) = while_looking(Vec::new());
        let last = looking.last().ok_or("a last row")?;

        assert_eq!(last.says, LOOKING);
        assert!(last.does.is_none(), "a row saying it is looking is not a row to press");

        let Ok(idle) = bluetooth_of(bluetooth::Radio::On, Vec::new());
        let last = idle.last().ok_or("a last row")?;

        assert_eq!(last.says, LOOK);

        Ok(())
    }

    #[test]
    fn the_loudest_stranger_is_the_nearest_row_to_the_thumb() {
        let Ok(devices) = bluetooth::devices("Device AA Far\nDevice BB Near\nDevice CC Middling");
        let Ok(heard) = heard(&devices, &[-95, -40, -70]);
        let Ok(rows) = while_looking(heard);

        assert_eq!(says(&rows), Ok(vec![BLUETOOTH, "Near", "Middling", "Far", LOOKING]));
    }

    #[test]
    fn a_device_that_only_said_its_address_is_drawn_under_the_ones_that_said_a_name() {
        let Ok(devices) = bluetooth::devices(
            "Device AA:BB:CC:DD:EE:FF AA-BB-CC-DD-EE-FF\nDevice 11:22:33:44:55:66 Blue Keys",
        );
        let Ok(heard) = heard(&devices, &[-40, -95]);
        let Ok(rows) = while_looking(heard);

        assert_eq!(says(&rows), Ok(vec![BLUETOOTH, "Blue Keys", "AA-BB-CC-DD-EE-FF", LOOKING]));
    }

    #[test]
    fn what_a_stranger_says_beside_it_is_how_loud_it_is() -> Result<(), Failure> {
        let Ok(devices) = bluetooth::devices("Device AA Near\nDevice BB Quiet");
        let Ok(met) = met(&devices, &[(bluetooth::Known::No, bluetooth::Joined::No, Some(-50)), STRANGER]);
        let Ok(rows) = while_looking(met);
        let Ok(loud) = strength(100);

        let near = row(&rows, "Near")?;
        assert_eq!(near.aside, loud);
        let quiet = row(&rows, "Quiet")?;
        assert_eq!(quiet.aside, YET, "a device the radio has heard nothing from says nothing");

        Ok(())
    }

    #[test]
    fn a_radio_that_is_off_says_so_beside_its_own_name_rather_than_in_it() {
        let Ok(off) = wifi_of(wifi::Radio::Off, Vec::new());
        let Ok(on) = wifi_of(wifi::Radio::On, Vec::new());
        let Ok(on_says) = says(&on);

        assert_eq!(says(&off), Ok(vec![WIFI]));
        assert_eq!(on_says.first().copied(), Some(WIFI));
        assert_eq!(off.first().map(|row| row.aside.as_str()), Some(OFF));
        assert_eq!(on.first().map(|row| row.aside.as_str()), Some(ON));

        let Ok(dark) = bluetooth_of(bluetooth::Radio::Off, Vec::new());

        assert_eq!(says(&dark), Ok(vec![BLUETOOTH]));
        assert_eq!(dark.first().map(|row| row.aside.as_str()), Some(OFF));
        assert!(
            dark.first().is_some_and(|row| row.does.is_some()),
            "a row that says Off is still the row that turns it on"
        );
    }

    #[test]
    fn the_one_we_are_on_is_marked_rather_than_offered() -> Result<(), Failure> {
        let Ok(networks) = wifi::networks("yes:Home:71:2437 MHz:WPA2\nno:Cafe:50:2437 MHz:");
        let Ok(rows) = wifi_of(wifi::Radio::On, networks);
        let home = row(&rows, "Home")?;
        let cafe = row(&rows, "Cafe")?;

        assert_eq!(home.now(), Ok(Active::Yes));
        assert!(home.does.is_none(), "there is nothing to do about being where you are");
        assert!(cafe.does.is_some());

        Ok(())
    }

    #[test]
    fn the_network_we_are_on_offers_its_code_on_y_and_no_other_does() {
        let Ok(networks) = wifi::networks("yes:Home:71:2437 MHz:WPA2\nno:Cafe:50:2437 MHz:WPA2");
        let Ok(rows) = wifi_of(wifi::Radio::On, networks);
        let offered: Vec<&str> = rows.iter().filter(|row| row.more.is_some()).map(|row| row.says.as_str()).collect();

        assert_eq!(offered, ["Home"]);
        assert_eq!(says(&rows), Ok(vec![WIFI, "Home", "Cafe"]), "the code is a choice on Y rather than a row of its own");
    }

    #[test]
    fn a_joined_device_is_marked_and_offers_the_way_out_of_it() -> Result<(), Failure> {
        let Ok(devices) = bluetooth::devices("Device AA Pads\nDevice BB Speaker");
        let Ok(met) = met(&devices, &[JOINED, FRIEND]);
        let Ok(rows) = bluetooth_of(bluetooth::Radio::On, met);

        let pads = row(&rows, "Pads")?;
        assert_eq!(pads.now(), Ok(Active::Yes));
        let speaker = row(&rows, "Speaker")?;
        assert_eq!(speaker.now(), Ok(Active::No));

        Ok(())
    }

    #[test]
    fn a_device_no_one_has_been_introduced_to_is_not_offered_a_word_bluez_would_refuse() -> Result<(), Failure> {
        let Ok(devices) = bluetooth::devices("Device AA Blue Keys");
        let Ok(met) = met(&devices, &[STRANGER]);
        let Ok(rows) = bluetooth_of(bluetooth::Radio::On, met);
        let stranger = row(&rows, "Blue Keys")?;

        assert!(stranger.does.is_some(), "a stranger has to be pressable to become known");
        assert_eq!(stranger.aside, YET, "and has to read as somewhere still to go");

        Ok(())
    }

    #[test]
    fn a_stranger_is_offered_the_pairing_and_a_friend_the_road_and_the_way_out() -> Result<(), Failure> {
        let Ok(devices) = bluetooth::devices("Device AA Blue Keys");
        let keyboard = devices.first().ok_or("the keyboard")?;
        let Ok(back) = back_to("Blue Keys");

        let stranger_rows = meeting(keyboard, STRANGER)?;
        assert_eq!(stranger_rows, [back.as_str(), PAIR]);
        let friend_rows = meeting(keyboard, FRIEND)?;
        assert_eq!(friend_rows, [back.as_str(), JOIN, FORGET]);
        let joined_rows = meeting(keyboard, JOINED)?;
        assert_eq!(joined_rows, [back.as_str(), LEAVE, FORGET]);

        Ok(())
    }

    #[test]
    fn a_row_that_acts_on_the_device_the_page_names_does_not_name_it_again() -> Result<(), Failure> {
        let Ok(devices) = bluetooth::devices("Device AA Blue Keys");
        let keyboard = devices.first().ok_or("the keyboard")?;

        for standing in [STRANGER, FRIEND] {
            let said = meeting(keyboard, standing)?;

            for row in said.iter().skip(1) {
                assert!(!row.contains("Blue Keys"), "{row:?} says the name the page it is on is already titled with");
            }
        }

        Ok(())
    }

    #[test]
    fn the_word_for_going_ahead_is_short_and_stands_for_nothing() {
        for word in [FORGET_YES, JOIN, LEAVE, PAIR] {
            let words: BTreeSet<String> = word.split_whitespace().map(str::to_lowercase).collect();

            assert!(words.len() <= 2, "{word:?} is a sentence where a verb was wanted");

            for standing in ["it", "them", "this", "that", "these", "those"] {
                assert!(!words.contains(standing), "{word:?} says {standing:?} where the question already said the thing");
            }
        }
    }

    #[test]
    fn every_device_row_is_the_row_its_own_page_comes_back_to() {
        let Ok(devices) = bluetooth::devices("Device AA One\nDevice BB Two\nDevice CC Three");
        let Ok(met) = met(&devices, &[STRANGER, STRANGER, STRANGER]);
        let seen: Arc<std::sync::Mutex<Vec<u32>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let telling = Arc::clone(&seen);
        let Ok(rows) = bluetooth_rows(
            bluetooth::Radio::On,
            bluetooth::Looking::No,
            met,
            Arc::new(move |_, at, _| match telling.lock() {
                Ok(mut seen) => seen.push(at),
                Err(_the_test_that_held_it_failed) => {},
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
            Err(_the_test_that_held_it_failed) => Vec::new(),
        };
        let names: BTreeSet<&str> = devices.iter().map(|device| device.name.as_str()).collect();
        let standing: Vec<u32> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| names.contains(row.says.as_str()))
            .map(|(place, _)| {
                let Ok(place) = fitted(place);

                place
            })
            .collect();

        assert_eq!(seen, standing);
    }

    #[test]
    fn nothing_playing_is_said_rather_than_left_blank() {
        let Ok(rows) = sound(&[], "");

        assert_eq!(says(&rows), Ok(vec!["Nothing else is playing"]));
    }

    #[test]
    fn the_engine_in_use_is_the_one_marked() {
        let Ok(rows) = searching("startpage");

        assert_eq!(marked(&rows), Ok(vec!["Startpage"]));
    }

    #[test]
    fn the_browsers_are_told_without_stopping_to_ask_for_a_password() {
        let Ok(telling) = set_engine_command("startpage");
        let Ok(sudo) = Program::Sudo.name();

        assert_eq!(telling, [sudo, "-n", "console-engine", "startpage"]);
    }

    #[test]
    fn the_list_is_the_way_back_and_then_a_row_that_only_reads() -> Result<(), Failure> {
        let Ok(rows) = searching("duckduckgo");
        let Ok(configured) = configuration();
        let back = at(&rows, 0)?;
        let named = at(&rows, 1)?;

        assert!(back.says.ends_with(&configured), "{:?} is not the way back", back.says);
        assert_eq!(named.says, "Search Engine");
        assert_eq!(named.heading(), Ok(Heading::Yes));

        Ok(())
    }

    #[test]
    fn the_engine_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(engine_says("startpage"), Ok("Startpage".to_string()));
        assert_eq!(engine_says("telepathy"), Ok(String::new()));
    }

    #[test]
    fn the_language_being_listened_for_is_the_one_marked() {
        let Ok(rows) = listening_for("nl");

        assert_eq!(marked(&rows), Ok(vec!["Dutch"]));
    }

    #[test]
    fn chinese_is_not_on_the_list() -> Result<(), Failure> {
        let Ok(rows) = listening_for("auto");
        let Ok(said) = says(&rows);
        let rows_after = after(&rows, 2)?;
        let Ok(offered) = says(rows_after);

        assert!(!said.contains(&"Chinese"));
        assert_eq!(offered, ["Automatic", "English", "Dutch", "Thai"]);

        Ok(())
    }

    #[test]
    fn the_languages_are_a_list_under_the_tab_like_the_engines() -> Result<(), Failure> {
        let Ok(rows) = listening_for("auto");
        let Ok(tab) = the_language();
        let Ok(listens) = text(&Word::DictationLanguage);
        let back = at(&rows, 0)?;
        let named = at(&rows, 1)?;

        assert!(back.says.ends_with(&tab), "{:?} is not the way back", back.says);
        assert_eq!(named.says, listens);
        assert_eq!(named.heading(), Ok(Heading::Yes));

        Ok(())
    }

    #[test]
    fn the_language_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(dictation_says("th"), Ok("Thai".to_string()));
        assert_eq!(dictation_says("auto"), Ok("Automatic".to_string()));
        assert_eq!(dictation_says("zh"), Ok(String::new()));
    }

    #[test]
    fn the_language_tab_is_three_rows_that_open_and_one_that_says_what_it_does_not_do() -> Result<(), Failure> {
        let Ok(rows) = language();
        let Ok(words) = text(&Word::PreferredLanguage);
        let Ok(types) = text(&Word::Keyboards);
        let Ok(listens) = text(&Word::DictationLanguage);
        let Ok(english) = text(&Word::EnglishOnly);
        let three = part(&rows, (0, 3))?;
        let Ok(said) = says(three);

        assert_eq!(said, [words.as_str(), types.as_str(), listens.as_str()]);

        for row in three {
            assert_eq!(row.acts(), Ok(Action::Yes), "{:?}", row.says);
        }

        assert_eq!(rows.last().map(|row| row.says.clone()), Some(english));

        Ok(())
    }

    #[test]
    fn each_of_the_three_says_what_it_is_now_beside_it() {
        let Ok(rows) = language();
        let asides: Vec<&str> = rows.iter().take(3).map(|row| row.aside.as_str()).collect();

        assert_eq!(asides, ["English (United Kingdom)", "Latin, Thai", "Dutch"]);
    }

    #[test]
    fn the_languages_are_the_way_back_a_heading_and_then_every_language() -> Result<(), Failure> {
        let Ok(spoken) = sample_languages();
        let Ok(nowhere) = nowhere();
        let Ok(rows) = language_picker_rows(&spoken, None, nowhere, opening);
        let Ok(tab) = the_language();
        let back = at(&rows, 0)?;
        let rows_after = after(&rows, 2)?;
        let Ok(languages) = says(rows_after);

        assert!(back.says.ends_with(&tab), "{:?} is not the way back", back.says);
        let second = at(&rows, 1)?;
        assert_eq!(second.heading(), Ok(Heading::Yes));
        assert_eq!(languages, ["en", "nl", "th"]);

        Ok(())
    }

    #[test]
    fn every_language_carries_the_row_it_was_opened_from_so_b_lands_back_on_it() {
        let Ok((seen, opening)) = recorder();
        let Ok(spoken) = sample_languages();
        let Ok(nowhere) = nowhere();
        let Ok(rows) = language_picker_rows(&spoken, None, nowhere, opening);
        let Ok(seen) = opened(&seen);
        let Ok(()) = opened_from(&rows, &seen);
    }

    #[test]
    fn every_part_of_the_world_carries_the_row_it_was_opened_from() {
        let Ok((seen, opening)) = recorder();
        let Ok(zones) = hours::zones("Europe/Amsterdam\nAsia/Bangkok\nUTC");
        let Ok(regions) = hours::regions(&zones);
        let Ok(nowhere) = nowhere();
        let Ok(rows) = region_rows(&regions, None, nowhere, opening);
        let Ok(seen) = opened(&seen);
        let Ok(()) = opened_from(&rows, &seen);
    }

    #[test]
    fn the_language_the_machine_is_in_is_the_one_marked() {
        let Ok(spoken) = sample_languages();
        let Ok(standing) = crate::languages::current_locale(&spoken, Some("nl_NL.UTF-8"));
        let Ok(nowhere) = nowhere();
        let Ok(rows) = language_picker_rows(&spoken, standing.as_ref(), nowhere, opening);

        assert_eq!(marked(&rows), Ok(vec!["nl"]));
    }

    #[test]
    fn the_places_a_language_is_spoken_are_a_list_under_it() -> Result<(), Failure> {
        let Ok(spoken) = sample_languages();
        let Ok(names) = Names::none();
        let english = spoken.iter().find(|language| language.language == "en").ok_or("English")?;
        let Ok(standing) = crate::languages::current_locale(&spoken, Some("en_GB.UTF-8"));
        let Ok(nowhere) = nowhere();
        let Ok(rows) = place_rows(english, &names, &[], standing.as_ref(), nowhere);
        let rows_after = after(&rows, 2)?;
        let Ok(places) = says(rows_after);

        assert_eq!(places, ["GB", "US"]);
        assert_eq!(marked(&rows), Ok(vec!["GB"]));

        Ok(())
    }

    #[test]
    fn latin_is_drawn_and_cannot_be_pressed_and_the_rest_can() -> Result<(), Failure> {
        let Ok(chosen) = alphabets::read(alphabets::UNLESS_TOLD);
        let Ok(nowhere) = nowhere();
        let Ok(rows) = alphabet_rows(&chosen, nowhere);
        let rows_after = after(&rows, 2)?;
        let Ok(alphabets) = says(rows_after);

        assert_eq!(alphabets, ["Latin", "Arabic", "Georgian", "Greek", "Hebrew", "Persian", "Russian", "Thai"]);
        assert_eq!(marked(&rows), Ok(vec!["Latin", "Thai"]));
        let third = at(&rows, 2)?;
        assert_eq!(third.acts(), Ok(Action::None));
        let fourth = at(&rows, 3)?;
        assert_eq!(fourth.acts(), Ok(Action::Yes));

        Ok(())
    }

    #[test]
    fn the_parts_of_the_world_open_onto_the_places_in_them() -> Result<(), Failure> {
        let Ok(zones) = hours::zones("Europe/Amsterdam\nEurope/London\nAsia/Bangkok");
        let Ok(regions) = hours::regions(&zones);
        let Ok(nowhere) = nowhere();
        let Ok(rows) = region_rows(&regions, Some("Europe/Amsterdam"), nowhere, opening);
        let rows_after = after(&rows, 2)?;
        let Ok(parts) = says(rows_after);

        assert_eq!(parts, ["Asia", "Europe"]);
        assert_eq!(marked(&rows), Ok(vec!["Europe"]));

        Ok(())
    }

    #[test]
    fn a_part_of_the_world_is_its_places_with_the_one_it_is_in_marked() -> Result<(), Failure> {
        let Ok(zones) = hours::zones("Europe/Amsterdam\nEurope/London\nAsia/Bangkok");
        let Ok(places) = hours::places(&zones, "Europe");
        let Ok(nowhere) = nowhere();
        let Ok(rows) = zone_rows("Europe", &places, Some("Europe/London"), nowhere);
        let rows_after = after(&rows, 2)?;
        let Ok(cities) = says(rows_after);

        assert_eq!(cities, ["Amsterdam", "London"]);
        assert_eq!(marked(&rows), Ok(vec!["London"]));

        Ok(())
    }

    #[test]
    fn the_clock_is_the_hour_written_both_ways_with_one_of_them_marked() -> Result<(), Failure> {
        let Ok(nowhere) = nowhere();
        let Ok(rows) = clock_rows(Clock::Twelve, nowhere);
        let rows_after = after(&rows, 2)?;
        let Ok(ways) = says(rows_after);

        assert_eq!(ways, ["14:30", "2:30 pm"]);
        assert_eq!(marked(&rows), Ok(vec!["2:30 pm"]));

        Ok(())
    }

    #[test]
    fn the_machines_name_is_a_row_that_asks_rather_than_one_that_opens() {
        let Ok(row) = called_row("legion");
        let Ok(asks) = text(&Word::Name);

        assert_eq!(row.says, asks);
        assert_eq!(row.aside, "legion");
        assert_eq!(row.acts(), Ok(Action::Yes));
    }

    #[test]
    fn every_tab_is_named_once() {
        let Ok(tabs) = tabs();
        let named: BTreeSet<&String> = tabs.iter().collect();

        assert_eq!(named.len(), tabs.len());
    }
}
