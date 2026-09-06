//! What each tab holds, as a function of what the machine said.
//!
//! Reading the machine is one thing and knowing what to draw from it is
//! another. Everything here is the second, so the shape of every tab can be
//! asked without a machine to ask.

use std::sync::Arc;

use console_defaults::{battery, engines};
use console_external_programs::Program;
use console_home_screen::shape::Shape;
use console_never::Never;
use console_translation::say;

use crate::words::Word;
use console_notifications::reading::{QUIET, Quiet};
use console_panel::page::{Does, Level, NOW, Row, Showing, YET};
use console_dictation::languages;

use crate::level::{CELLS, Muted, bar, volume};
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

pub fn bluetooth_rows(
    on: bluetooth::Radio,
    devices: Vec<(bluetooth::Device, bluetooth::Joined)>,
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

    for (device, joined) in devices {
        let doing = match joined {
            bluetooth::Joined::Yes => "disconnect",
            bluetooth::Joined::No => "connect",
        };
        let aside = match joined {
            bluetooth::Joined::Yes => NOW,
            bluetooth::Joined::No => "",
        };
        let Ok(mut row) = switch(&device.name, &[bluetoothctl, doing, &device.address]);

        row.aside = aside.to_string();
        rows.push(row);
    }

    let Ok(looking) = switch("Look for devices", &[bluetoothctl, "--timeout", "8", "scan", "on"]);

    rows.push(looking);

    Ok(rows)
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
    let Ok(configuration) = configuration();
    let Ok(way_back) = Row::back(&configuration, move |showing| leaving(showing));
    let Ok(naming) = Row::naming("Listen for", "");
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

pub fn tabs() -> Result<[String; 9], Never> {
    let Ok(configuration) = configuration();

    let Ok(sound) = say(&Word::Sound);
    let Ok(bluetooth) = say(&Word::Bluetooth);
    let Ok(wifi) = say(&Word::Wifi);
    let Ok(battery) = say(&Word::Battery);
    let Ok(notifications) = say(&Word::Notifications);
    let Ok(screen) = say(&Word::Screen);
    let Ok(wallpaper) = say(&Word::Wallpaper);
    let Ok(system) = say(&Word::System);

    Ok([
        sound,
        bluetooth,
        wifi,
        battery,
        notifications,
        screen,
        wallpaper,
        configuration,
        system,
    ])
}

pub fn configuration() -> Result<String, Never> {
    say(&Word::Configuration)
}

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

    fn bluetooth_of(
        on: bluetooth::Radio,
        devices: Vec<(bluetooth::Device, bluetooth::Joined)>,
    ) -> Vec<Row> {
        let Ok(rows) = bluetooth_rows(on, devices);

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

    fn named() -> [String; 9] {
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
        let rows = bluetooth_of(
            bluetooth::Radio::On,
            vec![
                (devices[0].clone(), bluetooth::Joined::Yes),
                (devices[1].clone(), bluetooth::Joined::No),
            ],
        );
        assert_eq!(now(rows.iter().find(|row| row.says == "Pads").expect("pads")), InEffect::Yes);
        assert_eq!(
            now(rows.iter().find(|row| row.says == "Speaker").expect("speaker")),
            InEffect::No
        );
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
        assert!(rows[0].says.ends_with(&configured()), "{:?} is not the way back", rows[0].says);
        assert_eq!(rows[1].says, "Listen for");
        assert_eq!(heading(&rows[1]), Heading::Yes);
    }

    #[test]
    fn the_language_is_named_the_way_it_is_named_on_its_own_row() {
        assert_eq!(dictation_says("th"), Ok("Thai".to_string()));
        assert_eq!(dictation_says("auto"), Ok("Whichever is spoken".to_string()));
        assert_eq!(dictation_says("zh"), Ok(String::new()));
    }

    #[test]
    fn every_tab_is_named_once() {
        let mut sorted = named().to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), named().len());
    }
}
