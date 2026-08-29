//! The settings, drawn.
//!
//! What is here is the reading of the machine. What each tab holds once it has
//! been read is `console_settings::rows`, where it can be asked without a
//! machine to ask.
//!
//! Anything that takes a moment, connecting above all, is done off to one side
//! so the panel keeps answering the buttons while it happens.

use std::sync::{Arc, Mutex};

use console_panel::page::{Does, Level, Page, Rows, Showing};
use console_panel::running::{said, say};
use console_panel::{chooser, panel};
use console_settings::defaults::{self, Program};
use console_settings::level::stepped;
use console_settings::rows::{
    Chosen, TABS, battery_rows, bluetooth_rows, engine_says, search_rows, sound_rows, system_rows,
    wifi_rows,
};
use console_settings::wallpaper::{Found, Offered, wallpaper_rows};
use console_settings::{bluetooth, sound, wifi};
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

fn sound_tab() -> Vec<console_panel::page::Row> {
    let default = pactl(&["get-default-sink"]);
    sound_rows(
        &of_kind("sink"),
        &of_kind("sink-input"),
        &default,
        hush,
        turn_to,
    )
}

// ----------------------------------------------------------------- battery

/// How bright the screen is, asked of the program that owns the range.
///
/// The panel takes numbers up to 65535 and goes dark near the top of them, so
/// what counts as full is a decision, and it is made once in console-brightness.
/// Reading the file here would be a second opinion about the same screen, and
/// the two would part company the day one of them moved.
fn brightness() -> i32 {
    said(&["console-brightness", "get"]).parse().unwrap_or(0)
}

/// One step of the screen, the same step the d-pad takes under L2.
fn dim() -> Level {
    Arc::new(|step| {
        said(&["console-brightness", if step > 0 { "up" } else { "down" }]);
    })
}

fn battery_tab() -> Vec<console_panel::page::Row> {
    battery_rows(
        Some(brightness()),
        Some(&said(&["powerprofilesctl", "get"])),
        dim(),
    )
}

/// The tab before either of its two readings is back.
///
/// Both are a subprocess away: the screen is asked of console-brightness and the
/// profile of powerprofilesctl. The four rows are the four rows whatever they
/// answer, so they go up at once and the answers land in them.
fn battery_meanwhile() -> Vec<console_panel::page::Row> {
    battery_rows(None, None, dim())
}

// ------------------------------------------------------------------- wi-fi

/// Connect to one network, asking for the password only if it has to.
fn join(network: wifi::Network, known: bool) -> Does {
    Does::and_stay(move |showing| {
        let name = network.name.clone();
        if known {
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

fn wifi_tab() -> Vec<console_panel::page::Row> {
    let on = wifi::on(&said(&["nmcli", "radio", "wifi"]));
    let known = wifi::saved(&said(&[
        "nmcli",
        "-t",
        "-f",
        "NAME,TYPE",
        "connection",
        "show",
    ]));
    let listed = said(&[
        "nmcli",
        "-t",
        "-f",
        "ACTIVE,SSID,SIGNAL,SECURITY",
        "device",
        "wifi",
        "list",
    ]);
    wifi_rows(on, wifi::networks(&listed), &known, join)
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

fn bluetooth_tab() -> Vec<console_panel::page::Row> {
    let on = bluetooth::on(&said(&["bluetoothctl", "show"]));
    let devices = bluetooth::devices(&said(&["bluetoothctl", "devices"]))
        .into_iter()
        .map(|device| {
            let joined = bluetooth::joined(&said(&["bluetoothctl", "info", &device.address]));
            (device, joined)
        })
        .collect();
    bluetooth_rows(on, devices)
}

// -------------------------------------------------------------- wallpaper

/// The picture actually on the screen, asked of the wallpaper daemon.
///
/// Asked rather than worked out. This panel could read the table and the sun
/// and reach the same answer the daemon reached, and then the two would be two
/// programs agreeing about a picture rather than one reporting it, and the day
/// they stopped agreeing the panel would be confidently wrong.
fn on_the_screen() -> String {
    place::showing(&said(&["awww", "query"]))
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
    let table = std::fs::read_to_string(place::table())
        .ok()
        .as_deref()
        .and_then(Set::read);
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
        .and_then(|at| std::fs::read_dir(at).ok())
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
/// here worth waiting for, so nothing waits for it: the list goes up unmarked
/// and the mark lands a moment later.
fn wallpaper_meanwhile() -> Vec<console_panel::page::Row> {
    wallpaper_at("")
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
    let home = std::env::var("HOME").unwrap_or_default();
    let looking = [
        format!("{home}/.local/share/applications"),
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
            if let Some(program) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|held| defaults::program(id, &held))
            {
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
/// A browser is set twice. `xdg-mime` writes the one handler asked for, and
/// `xdg-settings` writes the whole family a browser is expected to answer for:
/// http, https, and the html files somebody saved. Setting only the first
/// leaves a machine that opens links in one browser and saved pages in another.
///
/// Then back up to the tab, standing on the setting it was about, which now
/// says the name that was just chosen.
fn use_it(looking: &Looking, kind: &defaults::Kind, program: &Program) -> Does {
    let mime = kind.mime.to_string();
    let id = program.id.clone();
    let looking = Arc::clone(looking);
    Does::and_stay(move |showing| {
        said(&["xdg-mime", "default", &id, &mime]);
        if mime.starts_with("x-scheme-handler/") {
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
    Kind(usize),
}

type Looking = Arc<Mutex<Onto>>;

fn looking_at(held: &Looking) -> Onto {
    *held.lock().expect("what the defaults are looking at")
}

/// Look at something else, and stand on a given row of it.
fn look(held: &Looking, onto: Onto, showing: &dyn Showing, row: usize) {
    *held.lock().expect("what the defaults are looking at") = onto;
    showing.replace(row);
}

/// Back to the tab, standing on the setting the list was opened from.
fn went_up(held: &Looking, showing: &dyn Showing) {
    let row = row_of(looking_at(held));
    look(held, Onto::Settings, showing, row);
}

/// Which row of the tab a list belongs to. Search is written first, and the
/// kinds follow it in the order they are declared.
fn row_of(onto: Onto) -> usize {
    match onto {
        Onto::Kind(at) => at + 1,
        _ => 0,
    }
}

/// The way back up, for a list that builds its own rows.
fn back_up(held: &Looking) -> Chosen {
    let held = Arc::clone(held);
    Arc::new(move |showing: &dyn Showing| went_up(&held, showing))
}

/// Open the list under one setting.
fn open(held: &Looking, onto: Onto) -> Does {
    let held = Arc::clone(held);
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
fn defaults_tab(looking: &Looking) -> Vec<console_panel::page::Row> {
    match looking_at(looking) {
        Onto::Settings => setting_rows(looking),
        Onto::Search => search_rows(&console_defaults::engines::chosen(), back_up(looking)),
        Onto::Kind(at) => {
            let leaving = Arc::clone(looking);
            let chosen = Arc::clone(looking);
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
fn defaults_meanwhile(looking: &Looking) -> Vec<console_panel::page::Row> {
    match looking_at(looking) {
        Onto::Settings => {
            let mut rows = vec![search_row(looking)];
            rows.extend(defaults::meanwhile_rows(|at| open(looking, Onto::Kind(at))));
            rows
        }
        _ => Vec::new(),
    }
}

/// Which engine a question is asked of, which is read off a file of hers rather
/// than out of the machine, so it is known before anything has been asked.
fn search_row(looking: &Looking) -> console_panel::page::Row {
    let engine = console_defaults::engines::chosen();
    console_panel::page::Row::new("Search", &engine_says(&engine), open(looking, Onto::Search))
        .opening()
}

/// The tab itself: every setting, what it is set to, and the way into changing
/// it.
fn setting_rows(looking: &Looking) -> Vec<console_panel::page::Row> {
    let mut rows = vec![search_row(looking)];
    rows.extend(defaults::defaults_rows(&programs(), &opening, |at| {
        open(looking, Onto::Kind(at))
    }));
    rows
}

/// Each tab, what fills it, what to do on arriving, and what to listen to.
///
/// Nothing here is read until its tab is looked at. Reading all of it to open
/// one tab meant scanning for networks before the volume could be shown.
fn pages(looking: &Looking) -> Vec<Page> {
    let drawing = Arc::clone(looking);
    let backing = Arc::clone(looking);
    let waiting = Arc::clone(looking);
    vec![
        Page::new(TABS[0], Rows::asked(battery_tab)).meanwhile(battery_meanwhile),
        // The volume rocker on the top edge moves the same number this tab
        // shows, and a panel that goes on showing the old one is worse than one
        // showing nothing: it is a reading, and it is wrong.
        Page::new(TABS[1], Rows::asked(sound_tab)).watching(console_panel::page::Watch::on(
            &["stdbuf", "-oL", "pactl", "subscribe"],
            "on sink",
        )),
        Page::new(TABS[2], Rows::asked(wifi_tab)).on_arriving(look_again),
        Page::new(TABS[3], Rows::asked(bluetooth_tab)),
        Page::new(TABS[4], Rows::asked(wallpaper_tab)).meanwhile(wallpaper_meanwhile),
        // The one tab that is somewhere as well as something. B out of a list
        // under it is the tab again, and only from the tab does B mean the
        // panel, which is how back means one thing everywhere on this desktop.
        Page::new(TABS[5], Rows::asked(move || defaults_tab(&drawing)))
            .meanwhile(move || defaults_meanwhile(&waiting))
            .on_back(move |showing| match looking_at(&backing) {
                Onto::Settings => true,
                _ => {
                    went_up(&backing, showing);
                    false
                }
            }),
        Page::new(TABS[6], Rows::asked(system_rows)),
    ]
}

fn main() {
    // A tab may be named, so a tap on the bar lands on the thing it stands for.
    let tab = std::env::args().nth(1);

    // The settings are on the Menu button and on four of the bar's icons, and
    // any of them pressed twice used to stack two identical panels. The tab is
    // part of which door this is: tapping the speaker while the battery is up
    // moves the panel to Sound, and tapping the speaker again puts it away.
    if !chooser::alone(
        &format!("settings {}", tab.clone().unwrap_or_default()),
        chooser::Again::Closes,
    ) {
        return;
    }
    // Which list the Defaults tab is on, held here because it outlives every
    // drawing of it: the pages are asked for again on every redraw.
    let looking: Looking = Arc::new(Mutex::new(Onto::Settings));
    panel::show(Arc::new(move || pages(&looking)), 0, tab.as_deref());
}
