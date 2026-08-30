//! What the manifest says, held against the tree it is a manifest of.
//!
//! These need the repository rather than a fixture, so they live out here
//! rather than beside the code. Everything that can be decided from a string
//! alone is tested next to the function that decides it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

fn console(args: &[&str]) -> (bool, String) {
    let done = Command::new(env!("CARGO_BIN_EXE_console"))
        .args(args)
        .output()
        .expect("console runs");
    (done.status.success(), String::from_utf8_lossy(&done.stdout).into_owned())
}

/// Every file in the tree, as the path it is installed to.
///
/// Bytecode is not one of them. It is written beside whatever imports it, git
/// is already told to ignore it, and this tree is worked in by more than one
/// person at once: a stray file from somebody else's test run is not a desktop
/// file nobody installs.
fn carried() -> Vec<(PathBuf, String)> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(at) else { return };
        for path in entries.flatten().map(|entry| entry.path()) {
            match path {
                path if path.ends_with("__pycache__") => {}
                path if path.is_dir() => walk(&path, into),
                path => into.push(path),
            }
        }
    }
    let files = root().join("files");
    let mut found = Vec::new();
    walk(&files, &mut found);
    found.sort();
    found
        .into_iter()
        .map(|path| {
            let live = format!("/{}", path.strip_prefix(&files).expect("under files/").display());
            (path, live)
        })
        .collect()
}

#[test]
fn the_manifest_this_desktop_wears_is_one_the_engine_can_read() {
    let (ok, said) = console(&["list", "--root", root().to_str().expect("a path")]);
    assert!(ok, "console list could not read desktop.conf:\n{said}");
    for section in ["[packages]", "[build]", "[files]", "[services]", "[masked]"] {
        assert!(said.contains(section), "{section} is not in the manifest");
    }
}

/// The service brings the daemon up and sets the ground, and `console-sky`
/// decides what goes on it. A picture named here would be a second opinion
/// about the wallpaper, and it used to be one: the cherry blossom garden was
/// painted here and was what anybody saw for the moment before a picture
/// arrived.
#[test]
fn the_paper_service_sets_a_ground_and_paints_no_picture_of_its_own() {
    let unit = root().join("files/etc/systemd/user/console-paper.service");
    let held = std::fs::read_to_string(&unit).expect("the paper service");
    let sets = held
        .lines()
        .filter(|line| line.starts_with("ExecStartPost="))
        .collect::<Vec<_>>();
    assert!(
        sets.iter().any(|line| line.contains("awww clear ")),
        "the paper service sets no ground colour, so the screen is black until \
         console-sky paints: {sets:?}"
    );
    assert!(
        !sets.iter().any(|line| line.contains(".webp")),
        "the paper service paints a picture of its own: {sets:?}"
    );
}

/// Steam is asked to leave before the desktop is.
///
/// The menu launches a game through Steam, so Steam can be running on the
/// desktop when the button for Game Mode is pressed. Killed with the session
/// it was in, it leaves its installation marked unclean, and the Game Mode
/// start after that fetches a client manifest over the network and verifies
/// every executable's checksum before it draws anything.
#[test]
fn the_way_to_game_mode_shuts_steam_down_before_the_compositor() {
    let at = root().join("files/usr/local/bin/steamos-session-select");
    let held = std::fs::read_to_string(&at).expect("the session switcher");
    let asked = held.find("\n    settle\n").expect("nothing asks Steam to go");
    let left = held.find("hyprctl dispatch").expect("nothing leaves the compositor");
    assert!(asked < left, "Steam is asked to go once the desktop it was on has gone");
}

/// Neither way to a session does anything when the machine is already in it.
///
/// The desktop's controller daemon has been seen running through a whole Game
/// Mode session, and it answers the left Legion button by running `game-mode`.
/// One press over there would ask Steam to shut down, wait ten seconds for it,
/// and then fail to leave a compositor that is not running. The daemon being
/// there at all is a fault of its own; this is the half that makes the button
/// harmless while it is.
#[test]
fn neither_way_to_a_session_acts_when_the_machine_is_already_in_it() {
    for (script, switch) in
        [("game-mode", "steamos-session-select gamescope"), ("desktop-mode", "plasma")]
    {
        let at = root().join("files/usr/local/bin").join(script);
        let held = std::fs::read_to_string(&at).unwrap_or_else(|_| panic!("{script}"));
        let asked = held
            .find("is-active --quiet gamescope-session.target")
            .unwrap_or_else(|| panic!("{script} switches sessions without asking where it is"));
        let switches = held.find(switch).unwrap_or_else(|| panic!("{script} switches nothing"));
        assert!(asked < switches, "{script} asks where it is after it has already left");
    }
}

#[test]
fn everything_meant_to_be_run_will_be_installed_able_to_run() {
    for (path, live) in carried() {
        let head: Vec<u8> = std::fs::read(&path).unwrap_or_default().into_iter().take(4).collect();
        let a_program = matches!(head.as_slice(), [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..]);
        if a_program {
            assert_eq!(
                mode_of(&live, &head),
                0o755,
                "{live} is a program and would be installed unrunnable"
            );
        }
    }
}

#[test]
fn files_in_the_users_home_are_installed_as_the_user() {
    let held = std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf");
    let files = section(&held, "files");
    assert!(!files.is_empty(), "the manifest names no files");
    for path in files {
        let expected = match path.starts_with("/home/@user@/") {
            true => SOMEBODY,
            false => "root",
        };
        assert_eq!(owner_of(&path), expected, "{path} would be installed as the wrong user");
    }
}

/// Every program the device compiles for itself is one this repository holds.
///
/// A crate is named for what it is and the program it makes is named for what
/// somebody types, so the two are not always the same word and the manifest
/// names the program.
#[test]
fn every_program_the_device_builds_is_one_this_repository_holds() {
    let held = std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf");
    let made = programs();
    for name in section(&held, "build") {
        assert!(
            made.contains(&name),
            "the manifest builds {name} and nothing here makes a program called that; \
             this repository makes {made:?}"
        );
    }
}

/// Every program the workspace makes, by the name it is installed under.
fn programs() -> Vec<String> {
    let crates = root().join("crates");
    std::fs::read_dir(&crates)
        .expect("crates/")
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("Cargo.toml")).ok())
        .filter_map(|held| held.parse::<toml::Table>().ok())
        .flat_map(|held| {
            let named = |at: &toml::Value| {
                at.get("name").and_then(toml::Value::as_str).map(str::to_owned)
            };
            match held.get("bin").and_then(toml::Value::as_array) {
                Some(bins) => bins.iter().filter_map(named).collect::<Vec<_>>(),
                // A crate with no [[bin]] table makes a program named for
                // itself, if it makes one at all.
                None => held.get("package").and_then(named).into_iter().collect(),
            }
        })
        .collect()
}

// The rules under test, written here rather than reached for, because the
// engine is a binary and its insides are its own. Any of these three drifting
// from the engine's own is caught by the engine's own tests, which assert the
// same rules against the same cases.

fn mode_of(live: &str, head: &[u8]) -> u32 {
    match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    }
}

/// Whoever the machine this runs on belongs to. The mark stands for them and
/// the name is never this tree's to know, so the test picks one.
const SOMEBODY: &str = "ada";

fn owner_of(live: &str) -> &'static str {
    match live.starts_with("/home/@user@/") {
        true => SOMEBODY,
        false => "root",
    }
}

fn named_by(unit: &str) -> Vec<String> {
    unit.lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| key.starts_with("Exec"))
        .flat_map(|(_, command)| command.split_whitespace())
        .map(|word| word.trim_start_matches(['-', '@', ':', '+', '!']))
        .filter(|word| word.starts_with('/'))
        .map(str::to_owned)
        .collect()
}

fn section(held: &str, wanted: &str) -> Vec<String> {
    held.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .fold((Vec::new(), None), |(mut out, at), line| {
            match line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
                Some(name) => (out, Some(name.to_string())),
                None => {
                    if at.as_deref() == Some(wanted) {
                        out.push(line.to_string());
                    }
                    (out, at)
                }
            }
        })
        .0
}

// ------------------------------------------------------- the manifest's word
//
// Everything below is a thing `console apply` would happily do, and a person
// would then find out about at the wrong moment: a file listed with nothing
// behind it, a file kept in the tree that is never installed anywhere, a script
// that will not parse, a service that starts a program the manifest does not
// carry.

/// Everything this desktop is allowed to reach for.
///
/// A program the manifest does not carry is normally a mistake: apply installs
/// half of a working pair and the missing half is found later, by somebody
/// holding the device. `[elsewhere]` is how the other case is said out loud. It
/// exists for a program that is somebody else's to publish, which is why the
/// public copy of this repository has one and this does not.
///
/// A built program is carried as its source rather than as itself, so it is
/// named here by where `console apply` puts it.
fn carried_or_declared(held: &str) -> BTreeSet<String> {
    section(held, "files")
        .into_iter()
        .chain(section(held, "elsewhere"))
        .chain(section(held, "build").into_iter().map(|name| format!("/usr/local/bin/{name}")))
        .collect()
}

fn manifest() -> String {
    std::fs::read_to_string(root().join("desktop.conf")).expect("desktop.conf")
}

/// Every file under a directory in the tree, whatever it is called.
fn every(under: &str, ending: &str) -> Vec<PathBuf> {
    carried()
        .into_iter()
        .map(|(path, _)| path)
        .filter(|path| path.to_string_lossy().contains(under))
        .filter(|path| path.to_string_lossy().ends_with(ending))
        .collect()
}

#[test]
fn every_file_the_manifest_lists_is_in_the_tree() {
    let files = root().join("files");
    for path in section(&manifest(), "files") {
        assert!(
            files.join(path.trim_start_matches('/')).is_file(),
            "{path} is listed and there is nothing behind it"
        );
    }
}

/// A file nobody lists is a file `console apply` never installs. It reads as part
/// of the desktop and is not part of it.
#[test]
fn every_file_in_the_tree_is_listed() {
    let listed: BTreeSet<String> = section(&manifest(), "files").into_iter().collect();
    for (_, live) in carried() {
        assert!(listed.contains(&live), "{live} is in the tree and nothing installs it");
    }
}

#[test]
fn every_service_has_a_unit_the_manifest_carries() {
    let held = manifest();
    let listed: BTreeSet<String> = section(&held, "files").into_iter().collect();
    for service in section(&held, "services") {
        assert!(
            listed.contains(&format!("/etc/systemd/user/{service}")),
            "{service} is enabled and its unit is not carried"
        );
    }
}

/// The target is what the compositor starts, and the only thing it starts. A
/// service enabled but not wanted by it never runs; one wanted by it and not
/// enabled is a unit systemd will not have.
#[test]
fn the_target_pulls_in_exactly_the_services_that_are_enabled() {
    let held = manifest();
    let enabled: BTreeSet<String> = section(&held, "services").into_iter().collect();
    let wanted: BTreeSet<String> = enabled
        .iter()
        .filter(|service| {
            let unit = root().join("files/etc/systemd/user").join(service);
            std::fs::read_to_string(unit)
                .unwrap_or_default()
                .lines()
                .any(|line| line.trim() == "WantedBy=console.target")
        })
        .cloned()
        .collect();
    assert_eq!(wanted, enabled);
}

#[test]
fn every_program_a_unit_starts_is_carried() {
    let held = manifest();
    let listed = carried_or_declared(&held);
    for unit in every("/etc/systemd/user/", "") {
        let said = std::fs::read_to_string(&unit).unwrap_or_default();
        let name = unit.file_name().unwrap_or_default().to_string_lossy().to_string();
        for command in named_by(&said).into_iter().filter(|at| at.starts_with("/usr/local/")) {
            assert!(listed.contains(&command), "{name} starts {command}, which is not carried");
        }
    }
}

/// A script that calls another by its full path is a dependency the manifest has
/// to know about, or apply installs half of a working pair.
#[test]
fn every_program_a_carried_script_reaches_for_is_carried() {
    let held = manifest();
    let listed = carried_or_declared(&held);
    for path in every("/usr/local/bin/", "") {
        // A compiled program reaches for nothing that can be read here.
        let Ok(said) = std::fs::read_to_string(&path) else { continue };
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        for at in reaches_for(&said) {
            assert!(listed.contains(&at), "{name} runs {at}, which is not carried");
        }
    }
}

/// Every program under /usr/local/bin a piece of text names.
fn reaches_for(said: &str) -> BTreeSet<String> {
    said.match_indices("/usr/local/bin/")
        .map(|(at, _)| {
            let rest = &said[at + "/usr/local/bin/".len()..];
            let end = rest
                .find(|letter: char| !letter.is_alphanumeric() && letter != '-' && letter != '_')
                .unwrap_or(rest.len());
            format!("/usr/local/bin/{}", &rest[..end])
        })
        .filter(|at| at.len() > "/usr/local/bin/".len())
        .collect()
}

#[test]
fn every_shell_script_parses() {
    for (path, live) in carried() {
        let Ok(said) = std::fs::read_to_string(&path) else { continue };
        let first = said.lines().next().unwrap_or_default();
        if !(first.starts_with("#!") && (first.contains("/sh") || first.contains("bash"))) {
            continue;
        }
        let done = Command::new("sh").arg("-n").arg(&path).output().expect("sh");
        assert!(done.status.success(), "{live}: {}", String::from_utf8_lossy(&done.stderr).trim());
    }
}

#[test]
fn every_yaml_file_parses() {
    for path in every("", ".yaml") {
        let said = std::fs::read_to_string(&path).expect("a yaml file");
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&said)
            .unwrap_or_else(|fault| panic!("{}: {fault}", path.display()));
    }
}

#[test]
fn every_json_file_parses() {
    for path in every("", ".json") {
        let said = std::fs::read_to_string(&path).expect("a json file");
        serde_json::from_str::<serde_json::Value>(&said)
            .unwrap_or_else(|fault| panic!("{}: {fault}", path.display()));
    }
}

// ------------------------------------------------------------------ by finger
//
// The screen is a touchscreen, and the device is put down as often as it is
// held. Everything below is something a hand with no controller in it could not
// do at all until it was there, so each of these is a way back to that.

/// Programs the bar may reach for that come from a package rather than the tree.
/// `[packages]` is what holds these to the machine; `the_programs` is what
/// holds them to a package name.
const OUTSIDE: [&str; 3] = ["activate", "makoctl", "wpctl"];

/// What every on-click in the bar runs, as the module and the first word.
fn bar_commands() -> Vec<(String, String)> {
    let config = root().join("files/home/@user@/.config/waybar/config.jsonc");
    let said = std::fs::read_to_string(config).expect("the bar");
    let without_comments: String = said
        .lines()
        .map(|line| match line.trim_start().starts_with("//") {
            true => "",
            false => line,
        })
        .collect::<Vec<&str>>()
        .join("\n");
    let read: serde_json::Value = serde_json::from_str(&without_comments).expect("the bar reads");
    read.as_object()
        .expect("a bar of modules")
        .iter()
        .filter_map(|(module, about)| about.as_object().map(|about| (module, about)))
        .flat_map(|(module, about)| {
            about
                .iter()
                .filter(|(key, _)| key.starts_with("on-"))
                .filter_map(|(_, command)| command.as_str())
                .filter_map(|command| command.split_whitespace().next())
                .map(|word| (module.clone(), word.to_string()))
                .collect::<Vec<(String, String)>>()
        })
        .collect()
}

/// The bar is the one place a program is named where nothing will complain if it
/// is gone: the button simply does nothing, and a person decides the machine is
/// broken. A script that gets renamed has to be renamed here too.
#[test]
fn every_program_the_bar_runs_is_carried() {
    let listed = carried_or_declared(&manifest());
    for (module, command) in bar_commands() {
        if OUTSIDE.contains(&command.as_str()) {
            continue;
        }
        assert!(
            listed.contains(&format!("/usr/local/bin/{command}")),
            "the bar's {module} runs {command}, which is not carried"
        );
    }
}

/// The two things a finger has no other road to. Every other button on the pad
/// has an icon on the bar or a row in a panel; these two had neither, so a
/// person holding nothing could not open an application or type a letter.
#[test]
fn the_bar_has_a_door_for_the_menu_and_for_the_keyboard() {
    let runs: BTreeSet<String> = bar_commands().into_iter().map(|(_, command)| command).collect();
    assert!(runs.contains("launcher"), "there is no way to open the menu by hand");
    assert!(runs.contains("osk"), "there is no way to ask for the keyboard by hand");
}

/// polkitd asks the session for a password and gives up if nothing answers. With
/// no agent running, installing something is not a refusal, it is a button that
/// does nothing and says nothing about why.
#[test]
fn something_answers_when_a_password_is_asked_for() {
    let held = manifest();
    assert!(section(&held, "services").iter().any(|name| name == "console-polkit.service"));
    let unit = root().join("files/etc/systemd/user/console-polkit.service");
    let said = std::fs::read_to_string(unit).expect("the polkit service");
    let starts = named_by(&said);
    assert!(
        starts.iter().any(|at| at.contains("polkit")),
        "the polkit service starts {starts:?}"
    );
}
