//! The settings, drawn.
//!
//! What is here is the reading of the machine. What each tab holds once it has
//! been read is `console_settings::rows`, where it can be asked without a
//! machine to ask.
//!
//! Anything that takes a moment, connecting above all, is done off to one side
//! so the panel keeps answering the buttons while it happens.


use console_number::fitted;
use std::sync::Arc;

use console_defaults::battery;
use console_panel::actor::{self, Addr, Answer};
use console_panel::page::{Does, Level, Page, Rows, Showing};
use console_panel::running::{said, say};
use console_panel::{before, chooser, panel};
use console_settings::defaults::{self, Program};
use console_settings::level::stepped;
use console_settings::rows::{
    Chosen, battery_rows, bluetooth_rows, dictation_rows, dictation_says, engine_says,
    notifications_rows, screen_rows, search_rows, sound_rows, system_rows, tabs, wifi_rows,
};
use console_settings::wallpaper::{Found, Offered, wallpaper_rows};
use console_settings::warm::{self, Warmth};
use console_settings::{bluetooth, screen, size, sound, wifi};
use console_home::shape::{self, Shape};
use console_sky::choose::{Set, Wanted};
use console_sky::place;

/// pactl, as the user whose sound it is.
fn pactl(argv: &[&str]) -> String {
    said(&[&["pactl"], argv].concat())
}

fn of_kind(kind: &str) -> Vec<sound::Thing> {
    sound::read(&pactl(&["-f", "json", "list", &format!("{kind}s")]))
}

fn words(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|word| (*word).to_string()).collect()
}

// ------------------------------------------------------------------- sound

fn hush(index: i64, kind: &'static str) -> Does {
    Does::and_stay(move |_| {
        pactl(&[&format!("set-{kind}-mute"), &index.to_string(), "toggle"]);
    })
}

/// Left and right, five points at a time.
///
/// The reading is asked for again rather than added to: what a level is is the
/// machine's answer, and a panel that adds a step to the number it drew last
/// time is a panel that drifts away from the thing it claims to be showing.
fn turn_to(index: i64, kind: &'static str) -> Level {
    Arc::new(move |step| {
        let Some(thing) = sound::one(&of_kind(kind), index).cloned() else {
            return;
        };

        let going = stepped(thing.level(), step);
        pactl(&[
            &format!("set-{kind}-volume"),
            &index.to_string(),
            &format!("{going}%"),
        ]);
    })
}

/// What the two readings the tab opens on are written down under.
const SINKS: &str = "sinks";
const SPEAKERS: &str = "default sink";

/// pactl, with the answer written down so the tab can be drawn from it before
/// pactl has been asked again.
fn pactl_kept(note: &str, argv: &[&str]) -> String {
    before::said(note, &[&["pactl"], argv].concat())
}

fn sound_tab() -> Vec<console_panel::page::Row> {
    sound_rows(
        &sound::read(&pactl_kept(SINKS, &["-f", "json", "list", "sinks"])),
        &of_kind("sink-input"),
        &pactl_kept(SPEAKERS, &["get-default-sink"]),
        hush,
        turn_to,
    )
}

/// The tab before pactl has answered.
///
/// What is plugged into this machine is what was plugged into it, so the
/// speakers row goes up at once wearing the name and the level it had last
/// time, and pactl's answer lands in a row already on the screen.
///
/// What is playing is not remembered, and that is the difference between the
/// two readings rather than an omission. A sink is a fact about the machine
/// that keeps; a stream is something that was running once, and a row saying a
/// video is playing when the video was closed yesterday is a reading, and it is
/// wrong. So the streams are the one thing here still worth waiting for, and
/// what they cost is a row appearing under a card that is already the right
/// shape rather than the whole card arriving late.
fn sound_meanwhile() -> Vec<console_panel::page::Row> {
    sound_rows(
        &sound::read(&before::last(SINKS)),
        &[],
        &before::last(SPEAKERS),
        hush,
        turn_to,
    )
}

// ----------------------------------------------------------------- battery

/// How bright the screen is, in the range that screen actually shows.
///
/// The panel takes numbers up to 65535 and goes dark near the top of them, so
/// what counts as full is a decision, and it is made once in
/// `console_settings::screen`. This used to run `console-brightness get` to
/// reach that decision, because it lived in a shell script and a shell script
/// is only reachable by running it. It is a function now, so the panel and the
/// program the d-pad runs are the same opinion rather than two that agree.
fn brightness() -> i32 {
    fitted(screen::now().map(screen::as_points).unwrap_or(0))
}

/// One step of the screen, the same step the d-pad takes under L2.
///
/// The function and no longer the program, now that the program says so. What
/// the button runs raises a notice with the level on it, because a press under
/// L2 has a game in front of it and nowhere else to report; this row is that
/// report already, and a card drawn over the top of it would be the panel
/// telling somebody what they are looking at.
///
/// It is still one opinion rather than two that agree. `screen::stepped` is the
/// arithmetic either way -- the program calls the same function -- which is
/// what running the program was for.
fn dim() -> Level {
    Arc::new(|step| {
        let way = if step > 0 { screen::Way::Up } else { screen::Way::Down };

        if let Some(now) = screen::now() {
            let _ = screen::set(screen::stepped(now, way));
        }
    })
}

/// What moving one of the three battery thresholds does.
///
/// Read again on every press rather than held, because the row under the
/// thumb is one of three that constrain each other: a step that stopped where
/// the one below it was would stop at where it was when the tab was drawn,
/// which is a row that goes stiff for no reason a person can see.
fn guard(step: battery::Step) -> Level {
    Arc::new(move |way| {
        let levels = battery::Levels::here();
        levels.set(step, stepped(levels.at(step), way));
    })
}

/// What the profile the machine is running under is written down under.
const PROFILE: &str = "power profile";

fn battery_tab() -> Vec<console_panel::page::Row> {
    battery_rows(
        Some(&before::said(PROFILE, &["powerprofilesctl", "get"])),
        battery::Levels::here(),
        guard,
    )
}

/// Which way the warm switch is standing.
///
/// Read out of the file rather than asked of `console-warm`, because it is the
/// one reading on this tab that is not a subprocess: the daemon cannot be asked
/// what colour it is wearing, so the answer only ever existed as this file, and
/// the panel may as well read it where it is.
fn warmth() -> Warmth {
    let Ok(home) = std::env::var("HOME") else { return Warmth::Following };

    let Ok(said) = std::fs::read_to_string(warm::at(&home)) else { return Warmth::Following };

    Warmth::read(&said)
}

/// The tab before its reading is back.
///
/// The rows are the rows whatever the machine answers, so nothing here has ever
/// moved. What was missing was the one reading in them, and it does not have to
/// be: the profile is a subprocess away, in powerprofilesctl, and a machine's
/// profile is set once and left, so the one it was running under is the one it
/// is running under. The mark lands on the row it was already on rather than
/// arriving onto a tab of three unmarked ones.
fn battery_meanwhile() -> Vec<console_panel::page::Row> {
    battery_rows(Some(&before::last(PROFILE)), battery::Levels::here(), guard)
}

// ------------------------------------------------------------------ screen

/// What the compositor said about its screens is written down under.
const SCREENS: &str = "screens";

/// Which size the screen is standing at.
///
/// The compositor is asked rather than the file `console-scale` writes, because
/// the file is what was last chosen and the compositor is what is on the screen.
/// A machine set to a density of its own stands on none of the three, and none
/// of them is marked.
fn size_tab() -> Vec<console_panel::page::Row> {
    screen_rows(
        Some(brightness()),
        dim(),
        warmth(),
        size::standing(&before::said(SCREENS, &["hyprctl", "monitors", "-j"])),
        home_rows(),
    )
}

/// Where the home screen's own shape is written down.
///
/// A machine with no `HOME` -- or one spelled in bytes that are not words --
/// has nowhere to keep a grid, and that is an absence rather than a fault:
/// each caller already says in its own terms what it does without one.
fn home_at() -> Option<std::path::PathBuf> {
    match std::env::var("HOME") {
        Ok(home) => Some(shape::at(std::path::Path::new(&home))),
        Err(std::env::VarError::NotPresent | std::env::VarError::NotUnicode(_)) => None,
    }
}

/// What the file says, and the usual grid where there is no file or no home.
fn home_shape() -> Shape {
    let Some(at) = home_at() else { return Shape::USUAL };

    match std::fs::read_to_string(&at) {
        Ok(said) => Shape::read(&said),
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => Shape::USUAL,
        Err(fault) => {
            eprintln!("settings-panel: {}: {fault}", at.display());

            Shape::USUAL
        },
    }
}

/// Write the shape down, and tell the home screen it has changed.
///
/// Two halves and both of them matter. The file is what a home screen started
/// after this reads; the word down the door is what the one already running
/// hears, because a layer surface drawn under this panel cannot see a file
/// move and should not be watching one.
///
/// A home screen that is not running is a word said to nobody, which is the
/// whole point of the door being a datagram. Nothing is retried and nothing
/// waits: the file is written either way.
fn home_set(shape: Shape) {
    let Some(at) = home_at() else {
        eprintln!("settings-panel: this machine will not say whose home to write the grid in");

        return;
    };

    if let Some(above) = at.parent()
        && let Err(fault) = std::fs::create_dir_all(above)
    {
        eprintln!("settings-panel: {}: {fault}", above.display());

        return;
    }

    if let Err(fault) = std::fs::write(&at, shape.written()) {
        eprintln!("settings-panel: {}: {fault}", at.display());

        return;
    }

    if let Err(fault) = console_panel::door::telling(console_panel::door::Said::Again) {
        eprintln!("settings-panel: the home screen was not told: {fault}");
    }
}

/// The three rows that shape the home screen, and what a press of one does.
///
/// Read again inside every press rather than held, for the reason the battery's
/// thresholds are: this panel is not the only thing that writes the file, and a
/// step taken from the number the tab was drawn with is a step from where
/// things were rather than from where they are.
fn home_rows() -> Vec<console_panel::page::Row> {
    console_settings::rows::home_rows(
        home_shape(),
        Arc::new(|step| {
            let shape = home_shape();
            home_set(shape.across(stepped_by(shape.columns, step)));
        }),
        Arc::new(|step| {
            let shape = home_shape();
            home_set(shape.down(stepped_by(shape.rows, step)));
        }),
        Arc::new(|step| {
            let shape = home_shape();
            home_set(shape.sized(rung(shape.size, step)));
        }),
    )
}

/// One press of left or right on a count, and never below one.
///
/// The shape clamps to its own ends; this only has to keep the arithmetic off
/// the bottom of a count, because a step down from one would wrap to the
/// largest number there is.
fn stepped_by(now: usize, step: i32) -> usize {
    match step > 0 {
        true => now.saturating_add(1),
        false => now.saturating_sub(1),
    }
}

/// One press of left or right along the ladder, stopping at both ends.
fn rung(now: shape::Size, step: i32) -> shape::Size {
    let at = shape::EVERY.iter().position(|size| *size == now).unwrap_or(0);
    let went = match step > 0 {
        true => at.saturating_add(1),
        false => at.saturating_sub(1),
    };

    shape::EVERY.get(went).copied().unwrap_or(now)
}

/// The tab before the compositor has answered.
///
/// How bright the screen is is one file and which way the evening switch is
/// standing is another, so both are here already. The size is a subprocess
/// away, and it is the size the screen was when this panel was last opened --
/// which is the size it is, on a machine where the only thing that changes it
/// is the three rows below.
fn size_meanwhile() -> Vec<console_panel::page::Row> {
    screen_rows(
        Some(brightness()),
        dim(),
        warmth(),
        size::standing(&before::last(SCREENS)),
        home_rows(),
    )
}

// ------------------------------------------------------------------- wi-fi

/// Connect to one network, asking for the password only if it has to.
fn join(network: wifi::Network, known: wifi::Known) -> Does {
    Does::and_stay(move |showing| {
        let name = network.name.clone();

        if known == wifi::Known::Yes {
            showing.later(words(&["nmcli", "connection", "up", "id", &name]));
            return;
        }

        if !network.locked {
            showing.later(words(&["nmcli", "device", "wifi", "connect", &name]));
            return;
        }

        let asking = name.clone();
        showing.ask(
            &format!("The password for {name}"),
            Arc::new(move |showing, word| {
                showing.later(words(&[
                    "nmcli", "device", "wifi", "connect", &asking, "password", word,
                ]));
            }),
        );
    })
}

/// What the switch's standing is written down under.
const QUIET: &str = "makoctl mode";

/// Whether notifications are drawn on the screen as they arrive.
///
/// `makoctl mode` prints the modes one to a line, and the switch adds or
/// removes the one that means quiet.
fn notifications_tab() -> Vec<console_panel::page::Row> {
    notifications_rows(console_notices::reading::held_back(&before::said(
        QUIET,
        &["makoctl", "mode"],
    )))
}

/// The tab before makoctl has answered.
///
/// Two rows either way, so nothing here moves. What is spared is worse than a
/// shift: the switch says what pressing it will do, and drawn without an answer
/// it says the wrong one and then swaps for the right one. A switch that
/// changes its mind between being read and being pressed is a switch nobody can
/// trust, and it is the reading rather than the shape that makes it one.
fn notifications_meanwhile() -> Vec<console_panel::page::Row> {
    notifications_rows(console_notices::reading::held_back(&before::last(QUIET)))
}

/// What the three readings this tab is made of are written down under.
const WIFI_RADIO: &str = "wifi radio";
const KNOWN: &str = "wifi known";
const IN_RANGE: &str = "wifi in range";

/// The tab, out of three readings, whether they came back a moment ago or last
/// time the panel was up.
///
/// One builder rather than two lists that have to agree: `wifi_meanwhile` is
/// this fed older answers, so a row added here cannot go missing there.
fn wifi_at(radio: &str, known: &str, in_range: &str) -> Vec<console_panel::page::Row> {
    wifi_rows(
        wifi::on(radio),
        wifi::networks(in_range),
        &wifi::saved(known),
        join,
    )
}

fn wifi_tab() -> Vec<console_panel::page::Row> {
    wifi_at(
        &before::said(WIFI_RADIO, &["nmcli", "radio", "wifi"]),
        &before::said(KNOWN, &["nmcli", "-t", "-f", "NAME,TYPE", "connection", "show"]),
        &before::said(
            IN_RANGE,
            &["nmcli", "-t", "-f", "ACTIVE,SSID,SIGNAL,SECURITY", "device", "wifi", "list"],
        ),
    )
}

/// The tab before nmcli has answered.
///
/// Three subprocesses and a radio, on the one tab somebody opens in a hurry
/// because the connection has gone. It went up empty and filled in, and the
/// filling in was the whole card: the row that was about to be pressed was not
/// on the screen when the thumb started moving towards it.
///
/// The networks in a room are the networks that were in that room, so last
/// time's list is very nearly this time's, and the ones that are wrong are
/// wrong for as long as it takes nmcli to say so. `look_again` already says
/// that the list drawn first is a memory and the radio's answer replaces it;
/// this is the same memory, one panel further back.
fn wifi_meanwhile() -> Vec<console_panel::page::Row> {
    wifi_at(&before::last(WIFI_RADIO), &before::last(KNOWN), &before::last(IN_RANGE))
}

/// Arriving on Wi-Fi, ask the radio to look rather than answer from memory.
///
/// nmcli will otherwise report the last scan, which can be minutes old and from
/// another room. The list drawn first is that memory, because a panel that
/// waits for a radio before it appears is a panel that feels broken; it is
/// redrawn when the radio has finished looking.
fn look_again(showing: &dyn Showing) {
    showing.later(words(&["nmcli", "device", "wifi", "rescan"]));
}

// --------------------------------------------------------------- bluetooth

/// What the radio and the list of devices are written down under. One device's
/// own reading is written down under its address, which is what tells the
/// headphones from the pad.
const BLUETOOTH_RADIO: &str = "bluetooth radio";
const INTRODUCED: &str = "bluetooth devices";

fn about(address: &str) -> String {
    format!("bluetooth {address}")
}

/// The tab, out of the radio, the devices, and what each device says about
/// itself.
///
/// `ask` is the difference between the tab and the tab before it: asked of
/// bluetoothctl, or asked of what bluetoothctl said last time.
fn bluetooth_at(
    radio: &str,
    introduced: &str,
    ask: impl Fn(&str) -> String,
) -> Vec<console_panel::page::Row> {
    let devices = bluetooth::devices(introduced)
        .into_iter()
        .map(|device| {
            let joined = bluetooth::joined(&ask(&device.address));
            (device, joined)
        })
        .collect();
    bluetooth_rows(bluetooth::on(radio), devices)
}

fn bluetooth_tab() -> Vec<console_panel::page::Row> {
    bluetooth_at(
        &before::said(BLUETOOTH_RADIO, &["bluetoothctl", "show"]),
        &before::said(INTRODUCED, &["bluetoothctl", "devices"]),
        |address| before::said(&about(address), &["bluetoothctl", "info", address]),
    )
}

/// The tab before bluetoothctl has answered.
///
/// It is one subprocess for the radio, one for the list, and then one more for
/// every pair of headphones this machine has ever been introduced to, which is
/// the slowest tab on the panel and the one that grew most as it filled in.
///
/// A device that has been paired stays paired, so the list is the list. Whether
/// one of them is joined just now is the reading that goes stale, and it is
/// drawn from memory here for the moment before bluetoothctl says otherwise --
/// the same trade the Wi-Fi list makes, and for the same reason.
fn bluetooth_meanwhile() -> Vec<console_panel::page::Row> {
    bluetooth_at(&before::last(BLUETOOTH_RADIO), &before::last(INTRODUCED), |address| {
        before::last(&about(address))
    })
}

// -------------------------------------------------------------- wallpaper

/// What the daemon's answer about the screen is written down under.
const UP: &str = "wallpaper";

/// The picture actually on the screen, asked of the wallpaper daemon.
///
/// Asked rather than worked out. This panel could read the table and the sun
/// and reach the same answer the daemon reached, and then the two would be two
/// programs agreeing about a picture rather than one reporting it, and the day
/// they stopped agreeing the panel would be confidently wrong.
fn on_the_screen() -> String {
    place::showing(&before::said(UP, &["awww", "query"]))
}

/// The picture that was on the screen last time anybody looked.
///
/// Read out of what the daemon said rather than out of the table, so this and
/// `on_the_screen` are one answer at two ages rather than two answers.
fn was_on_the_screen() -> String {
    place::showing(&before::last(UP))
}

/// Write down what was asked of the wallpaper, and say what stopped it.
///
/// This is the whole of the change and it is a file being written, which is
/// instant. Everything that takes a moment is what the daemon does about it
/// afterwards.
fn write_down(wanted: &Wanted) -> Result<(), String> {
    let at = place::asked().ok_or("This machine will not say whose home to write it in.")?;

    if let Some(holding) = at.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{} could not be made: {fault}", holding.display()))?;
    }

    std::fs::write(&at, wanted.written())
        .map_err(|fault| format!("{} could not be written: {fault}", at.display()))
}

/// Write it down, say so in the corner, and put it up off to one side.
///
/// The daemon would come round to this on its own within five minutes. Five
/// minutes after choosing a wallpaper is not choosing a wallpaper, so it is
/// told to do the round now, which is one pass of the same code.
///
/// That pass is not quick. It asks what the weather is doing when the answer
/// could change the picture, it throws away frames the picture before this one
/// left behind, and it hands the daemon a loop that has to be decoded whole
/// before any of it is drawn. Waited for where the panel is drawn, it left the
/// panel deaf to every button for the length of that, which reads as a machine
/// that has crashed rather than one doing what it was asked. So the one thing
/// that can fail is done here, where there is something to say about it, and
/// the round is handed to `later`, which runs it off the drawing and draws the
/// tab again when it is done.
///
/// A tap that does nothing and says nothing is the worst of these to meet,
/// because there is no terminal in front of somebody choosing a wallpaper: the
/// picture simply does not change, twice, and then the tab is a tab that does
/// not work. So it either says what is happening or says what went wrong.
fn ask_for(showing: &dyn Showing, wanted: &Wanted, going_on: &str) {
    match write_down(wanted) {
        Ok(()) => {
            showing.note(going_on);
            showing.later(words(&["console-sky", "--now"]));
        }
        Err(why) => say("wallpaper-choice", "The wallpaper was not changed", &why),
    }
}

/// Every picture on the machine, with whatever is written down about it.
///
/// The table says what the ones it came with are called and who drew them. A
/// picture of hers is in nobody's table, so it is named after its own file.
fn offered() -> Vec<Offered> {
    let held = std::fs::read_to_string(place::table());

    // Read on every draw of the tab, so a table that is not there and one that
    // will not open are both simply a picture named after its own file.
    let table = match held.as_deref() {
        Ok(said) => Set::read(said),
        Err(_) => None,
    };
    let written_down = |name: &str| {
        table
            .as_ref()?
            .pictures
            .iter()
            .find(|picture| picture.name == name)
            .map(|picture| Offered {
                name: picture.name.clone(),
                says: picture.says.clone(),
                by: picture.by.clone(),
            })
    };
    place::every()
        .into_iter()
        .map(|name| written_down(&name).unwrap_or_else(|| Offered::of(&name)))
        .collect()
}

/// How many pictures are waiting to be taken up.
fn dropped() -> usize {
    place::dropped()
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
        .unwrap_or(0)
}

fn wallpaper_tab() -> Vec<console_panel::page::Row> {
    wallpaper_at(&on_the_screen())
}

/// The tab before the wallpaper daemon has said what is up.
///
/// Everything else on it is read off two directories, which is quick. Which of
/// the pictures is on the screen is asked of a daemon, and it is the only thing
/// here worth waiting for, so nothing waits for it: the list goes up wearing
/// last time's mark and the daemon moves it if it has moved.
///
/// It used to go up unmarked, which is a tab of thirty pictures with none of
/// them marked and then one of them marked -- and the one that is marked is the
/// one somebody came to the tab to find. A mark that is nearly always already
/// right is a better first drawing than no mark at all, and the wallpaper is a
/// thing that changes a few times a day at most.
fn wallpaper_meanwhile() -> Vec<console_panel::page::Row> {
    wallpaper_at(&was_on_the_screen())
}

/// What a picture is called on the tab, which is what a note about it says.
///
/// The file's own name is what everything else here holds, and the file's own
/// name is not what was chosen: the row that was pressed said Star Ride, and a
/// note about star-ride is a note about somebody else's filing.
fn named(pictures: &[Offered], name: &str) -> String {
    pictures
        .iter()
        .find(|picture| picture.name == name)
        .map(|picture| picture.says.clone())
        .unwrap_or_else(|| Offered::of(name).says)
}

fn wallpaper_at(up: &str) -> Vec<console_panel::page::Row> {
    let asked = Wanted::asked();
    let up = up.to_string();
    let pictures = offered();
    let waiting = dropped();
    let found = Found {
        pictures: &pictures,
        following: asked.follow,
        up: &up,
        dropped: waiting,
    };
    wallpaper_rows(
        &found,
        |following| {
            let picture = up.clone();
            // What the switch is about to mean, said before it means it. Which
            // picture that leaves up is worth saying: stopping the weather is
            // the one change here that does not change what is on the screen,
            // so without the name it is a press with nothing to show for it.
            let going_on = match (following, up.is_empty()) {
                (true, _) => "The wallpaper is following the weather again".to_string(),
                (false, true) => "The wallpaper is staying where it is".to_string(),
                (false, false) => {
                    format!("The wallpaper is staying on {}", named(&pictures, &up))
                }
            };
            Does::and_stay(move |showing| {
                ask_for(
                    showing,
                    &Wanted {
                        follow: following,
                        picture: picture.clone(),
                    },
                    &going_on,
                );
                showing.refresh();
            })
        },
        |name| {
            let picture = name.to_string();
            // The still of the picture is up in the moment and the loop over it
            // takes as long as it takes, so this is true from the press and
            // stays true while the movement arrives.
            let going_on = format!("{} is going up", named(&pictures, name));
            Does::and_stay(move |showing| {
                ask_for(
                    showing,
                    &Wanted {
                        follow: false,
                        picture: picture.clone(),
                    },
                    &going_on,
                );
                showing.refresh();
            })
        },
        // Pressing a picture takes tens of seconds: it is decoded, graded, cut
        // to this screen and written out again. Done where the panel is drawn
        // it would stop the panel answering the buttons for that whole time,
        // which reads as a machine that has crashed. So it is done off to one
        // side, and the corner says so: nothing else on the tab changes until
        // the press is finished, and a row that appears to have done nothing
        // is a row somebody presses again.
        Does::and_stay(move |showing| {
            showing.note(&match waiting {
                1 => "The picture is being taken up, which takes about a minute".to_string(),
                many => format!("The {many} pictures are being taken up, about a minute each"),
            });
            showing.later(words(&["sky-press", "--dropped"]));
        }),
        // The files are where her photographs are, and this machine has no
        // other file chooser. Pictures, because that is the folder a camera and
        // a screenshot both write into.
        Does::run(&["files-panel", "Pictures"]),
    )
}

// --------------------------------------------------------------- defaults

/// Everywhere a program says for itself what it opens.
///
/// The same directories the menu draws from, in the order xdg reads them, so
/// one of hers under her own home wins over one of the same name installed for
/// everybody.
fn programs() -> Vec<Program> {
    let hers = match std::env::var("HOME") {
        Ok(home) => format!("{home}/.local/share/applications"),

        // A session with no home has no programs of her own to find. Named as
        // a path that will not open rather than left out, so the list stays
        // the three places in the order xdg reads them.
        Err(_) => String::new(),
    };

    let looking = [
        hers,
        "/usr/local/share/applications".to_string(),
        "/usr/share/applications".to_string(),
    ];
    let mut found: Vec<Program> = Vec::new();

    for at in looking {
        let Ok(holding) = std::fs::read_dir(&at) else {
            continue;
        };

        for entry in holding.flatten() {
            let path = entry.path();

            if path.extension().is_none_or(|kind| kind != "desktop") {
                continue;
            }

            let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            if found.iter().any(|held| held.id == id) {
                continue;
            }

            let Ok(held) = std::fs::read_to_string(&path) else { continue };

            if let Some(program) = defaults::program(id, &held) {
                found.push(program);
            }
        }
    }

    found
}

/// Which program opens this kind of thing now.
fn opening(mime: &str) -> String {
    said(&["xdg-mime", "query", "default", mime])
}

/// Make it the one that opens this kind of thing.
///
/// The whole family, not the one type the row is read by. A kind of thing is
/// several types -- Music is mp3 and flac and opus and more -- and setting only
/// the first was a setting that appeared to have worked: the tab said Music
/// opened in the music panel and an `.opus` file went on opening in a browser,
/// because nothing had ever named `audio/x-opus+ogg`.
///
/// A browser is set a second way on top of that. `xdg-mime` writes the handler
/// asked for, and `xdg-settings` writes the family a browser is expected to
/// answer for: http, https, and the html files somebody saved. That one stays
/// where it is, because xdg-settings knows what that family is and this does
/// not.
///
/// Every type is written, including any the chosen program does not itself
/// claim. That is deliberate and it is the lesser of the two wrongs: a program
/// handed a file it cannot play fails in front of somebody, where a type left
/// unset fails by opening in a browser and looking like the setting did not
/// take.
///
/// Then back up to the tab, standing on the setting it was about, which now
/// says the name that was just chosen.
fn use_it(looking: &Held, kind: &defaults::Kind, program: &Program) -> Does {
    let every: Vec<String> = kind.every().map(str::to_string).collect();
    let scheme = kind.mime.starts_with("x-scheme-handler/");
    let id = program.id.clone();
    let looking = looking.clone();
    Does::and_stay(move |showing| {
        for mime in &every {
            said(&["xdg-mime", "default", &id, mime]);
        }

        if scheme {
            said(&["xdg-settings", "set", "default-web-browser", &id]);
        }

        went_up(&looking, showing);
    })
}

/// What the Defaults tab is looking at: the settings themselves, or the list
/// under one of them.
///
/// A tab that goes deeper has to remember where it is, the way each place in
/// the files remembers the folder it is standing in. It is the one thing this
/// panel holds between one drawing and the next, and B unwinds it: out of the
/// list, then out of the panel.
#[derive(Clone, Copy)]
enum Onto {
    Settings,
    Search,
    Dictation,
    Kind(usize),
}

/// The whole of what this panel holds between one drawing and the next, and
/// the only thing that owns it.
///
/// Everything else on the tabs is read off the machine at the moment it
/// is drawn. This is a machine rather than a value behind a lock because the
/// thumb writes it on the main thread and a tab is read on another, and a
/// state with one owner cannot be half written when the reader arrives.
struct Looking {
    onto: Onto,
}

/// Everything that can happen to it, and nothing else.
enum Msg {
    /// Look at something else.
    Look(Onto),
    /// What is it looking at.
    At(Answer<Onto>),
}

impl actor::Machine for Looking {
    type Msg = Msg;

    fn step(self, message: Msg) -> Self {
        match message {
            Msg::Look(onto) => Looking { onto },
            Msg::At(answer) => {
                let _ = answer.say(self.onto);
                self
            },
        }
    }
}

/// Where the panel reaches it. Cloned into every closure that used to be
/// handed the lock.
type Held = Addr<Msg>;

/// What it is looking at, asked of the owner.
///
/// The settings themselves, if the owner has gone: the panel is on its way out
/// by then, and the tab as it opens is a truer thing to draw than a list under
/// a setting nobody chose.
fn looking_at(held: &Held) -> Onto {
    match held.ask(Msg::At) {
        Ok(onto) => onto,
        Err(_) => Onto::Settings,
    }
}

/// Look at something else, and stand on a given row of it.
///
/// Said rather than asked, and the redraw underneath it reads the answer. The
/// two cannot cross: this message goes down the mailbox before `replace` is
/// called, and the question the redraw asks goes down the same mailbox behind
/// it.
fn look(held: &Held, onto: Onto, showing: &dyn Showing, row: usize) {
    let _ = held.tell(Msg::Look(onto));
    showing.replace(row);
}

/// Back to the tab, standing on the setting the list was opened from.
fn went_up(held: &Held, showing: &dyn Showing) {
    let row = row_of(looking_at(held));
    look(held, Onto::Settings, showing, row);
}

/// Which row of the tab a list belongs to.
///
/// The tab is written in `setting_rows` and read here, which are two places
/// that have to agree about a list of rows. They did not: the kinds were
/// counted from the row under Search, and a heading and the buttons had been
/// written in between since, so leaving the list under Music put the highlight
/// back on Video. Hence the names -- a row put in above these moves them, and
/// a row put in above them without moving them is the same fault again.
const SEARCH: usize = 0;
const DICTATION: usize = 1;
/// Past the heading and the buttons under it.
const FIRST_KIND: usize = 4;

fn row_of(onto: Onto) -> usize {
    match onto {
        Onto::Dictation => DICTATION,
        Onto::Kind(at) => FIRST_KIND + at,
        _ => SEARCH,
    }
}

/// The way back up, for a list that builds its own rows.
fn back_up(held: &Held) -> Chosen {
    let held = held.clone();
    Arc::new(move |showing: &dyn Showing| went_up(&held, showing))
}

/// Open the list under one setting.
fn open(held: &Held, onto: Onto) -> Does {
    let held = held.clone();
    Does::and_stay(move |showing| look(&held, onto, showing, DEEPER))
}

/// Where the highlight lands on a list under a setting: past the way back and
/// past the name of what it is about, on the first row that can be chosen.
const DEEPER: usize = 2;

/// What opens what, and what a search is done with.
///
/// The search engine is on this tab rather than a tab of its own because it is
/// the same question: a link typed into the menu goes to a browser, and which
/// browser and which engine are one decision made in two places.
fn defaults_tab(looking: &Held) -> Vec<console_panel::page::Row> {
    match looking_at(looking) {
        Onto::Settings => setting_rows(looking),
        Onto::Search => search_rows(&console_defaults::engines::chosen(), back_up(looking)),
        Onto::Dictation => {
            dictation_rows(&console_voice::languages::chosen(), back_up(looking))
        }
        Onto::Kind(at) => {
            let leaving = looking.clone();
            let chosen = looking.clone();
            defaults::choice_rows(
                &defaults::KINDS[at],
                &programs(),
                &opening,
                move |showing| went_up(&leaving, showing),
                move |kind, program| use_it(&chosen, kind, program),
            )
        }
    }
}

/// The tab before the machine has said what opens what.
///
/// Nothing under a setting, because a list that is open is a place somebody
/// walked to, and putting the tab back under it for a moment would be the panel
/// stepping back out on its own.
fn defaults_meanwhile(looking: &Held) -> Vec<console_panel::page::Row> {
    match looking_at(looking) {
        Onto::Settings => {
            let mut rows = vec![search_row(looking), dictation_row(looking)];
            rows.push(console_panel::page::Row::naming("On this device", ""));
            rows.push(buttons_row());
            rows.extend(defaults::meanwhile_rows(|at| open(looking, Onto::Kind(at))));
            rows
        }
        _ => Vec::new(),
    }
}

/// Which engine a question is asked of, which is read off a file of hers rather
/// than out of the machine, so it is known before anything has been chosen.
fn search_row(looking: &Held) -> console_panel::page::Row {
    let engine = console_defaults::engines::chosen();
    console_panel::page::Row::new("Search", &engine_says(&engine), open(looking, Onto::Search))
        .opening()
}

/// Which language the paddle on the back is listening for, read off the same
/// file as the engine and for the same reason: it is hers rather than the
/// machine's, and it is known before anything has been chosen.
///
/// Under Search because they are the same kind of question -- one answer,
/// chosen once, and nothing on the machine to ask about it -- and above the
/// buttons because a language is a setting and the buttons page is a thing to
/// read.
fn dictation_row(looking: &Held) -> console_panel::page::Row {
    let language = console_voice::languages::chosen();
    let says = dictation_says(&language);
    console_panel::page::Row::new("Dictation", &says, open(looking, Onto::Dictation)).opening()
}

/// Where the buttons on this device are, and which one plays what.
///
/// The profiles bind four paddles and Legion right, and on hardware without
/// them the menu, closing, dictation, the screenshot and the settings are on
/// buttons nobody can press. `console check` says so and an apply raises a
/// notice; this is the row the notice sends somebody to. A on a row asks for
/// the button by putting a card up and waiting for a press.
///
/// Said to open, like every other row on this tab that leads somewhere. It is
/// a panel of its own rather than a list this one grows, which is a difference
/// to the machine and none at all to the person holding it: what they see is a
/// tab of six rows carrying the mark that says there is more through here and
/// one that does not, and the one that does not is the only row on the tab
/// with nothing beside it either.
fn buttons_row() -> console_panel::page::Row {
    console_panel::page::Row::new("Buttons", "", Does::run(&["/usr/local/bin/layout-panel"]))
        .opening()
}

/// The tab itself: every setting, what it is set to, and the way into changing
/// it.
fn setting_rows(looking: &Held) -> Vec<console_panel::page::Row> {
    let mut rows = vec![search_row(looking), dictation_row(looking)];
    rows.push(console_panel::page::Row::naming("On this device", ""));
    rows.push(buttons_row());
    rows.extend(defaults::defaults_rows(&programs(), &opening, |at| {
        open(looking, Onto::Kind(at))
    }));
    rows
}

/// Each tab, what fills it, what to do on arriving, and what to listen to.
///
/// Nothing here is read until its tab is looked at. Reading all of it to open
/// one tab meant scanning for networks before the volume could be shown.
fn pages(looking: &Held) -> Vec<Page> {
    let drawing = looking.clone();
    let backing = looking.clone();
    let waiting = looking.clone();
    vec![
        // The volume rocker on the top edge moves the same number this tab
        // shows, and a panel that goes on showing the old one is worse than one
        // showing nothing: it is a reading, and it is wrong.
        Page::new(&tabs()[0], Rows::asked(sound_tab))
            .meanwhile(sound_meanwhile)
            .watching(console_panel::page::Watch::on(
                &["stdbuf", "-oL", "pactl", "subscribe"],
                "on sink",
            )),
        Page::new(&tabs()[1], Rows::asked(bluetooth_tab)).meanwhile(bluetooth_meanwhile),
        Page::new(&tabs()[2], Rows::asked(wifi_tab))
            .meanwhile(wifi_meanwhile)
            .on_arriving(look_again),
        Page::new(&tabs()[3], Rows::asked(battery_tab)).meanwhile(battery_meanwhile),
        Page::new(&tabs()[4], Rows::asked(notifications_tab)).meanwhile(notifications_meanwhile),
        Page::new(&tabs()[5], Rows::asked(size_tab)).meanwhile(size_meanwhile),
        Page::new(&tabs()[6], Rows::asked(wallpaper_tab)).meanwhile(wallpaper_meanwhile),
        // The one tab that is somewhere as well as something. B out of a list
        // under it is the tab again, and only from the tab does B mean the
        // panel, which is how back means one thing everywhere on this desktop.
        Page::new(&tabs()[7], Rows::asked(move || defaults_tab(&drawing)))
            .meanwhile(move || defaults_meanwhile(&waiting))
            .on_back(move |showing| match looking_at(&backing) {
                Onto::Settings => true,
                _ => {
                    went_up(&backing, showing);
                    false
                }
            }),
        Page::new(&tabs()[8], Rows::asked(system_rows)),
    ]
}

fn main() {
    // A tab may be named, so a tap on the bar lands on the thing it stands for.
    let tab = std::env::args().nth(1);

    // The settings are on the Menu button and on four of the bar's icons, and
    // any of them pressed twice used to stack two identical panels. The tab is
    // part of which door this is: tapping the speaker while the battery is up
    // moves the panel to Sound, and tapping the speaker again puts it away.
    if chooser::alone(
        &format!("settings {}", tab.clone().unwrap_or_default()),
        chooser::Again::Closes,
    ) == chooser::Alone::No
    {
        return;
    }

    // Which list the Defaults tab is on, held here because it outlives every
    // drawing of it: the pages are asked for again on every redraw.
    let looking = actor::supervise(|| Looking { onto: Onto::Settings });
    let held = looking.addr.clone();
    panel::show(Arc::new(move || pages(&held)), 0, tab.as_deref());
    // The panel is down and nothing is going to ask again. Waited for rather
    // than dropped, so a message already in the mailbox is finished with.
    looking.shutdown();
}
