//! What the manifest says, held against the tree it is a manifest of.
//!
//! These need the repository rather than a fixture, so they live out here
//! rather than beside the code. Everything that can be decided from a string
//! alone is tested next to the function that decides it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use console_core_external_programs::Program;

fn root() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn console(args: &[&str]) -> (bool, String) {
    let done = Command::new(env!("CARGO_BIN_EXE_console"))
        .args(args)
        .output()
        .expect("console runs");
    (done.status.success(), String::from_utf8_lossy(&done.stdout).into_owned())
}

fn carried() -> Vec<(PathBuf, String)> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let entries = match std::fs::read_dir(at) {
            Ok(entries) => entries,
            Err(_fault) => return,
        };
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

#[test]
fn the_keyboard_follows_nothing_because_it_takes_the_devices_itself() {
    let unit = root().join("files/etc/systemd/user/console-input-keyboard.service");
    let held = std::fs::read_to_string(&unit).expect("the keyboard service");
    let ordered: Vec<&str> = held
        .lines()
        .filter(|line| line.starts_with("After=") || line.starts_with("Requires="))
        .collect();
    assert!(
        ordered.is_empty(),
        "the keyboard is ordered against something again: {ordered:?}. It takes the pad and the \
         keyboard InputPlumber publishes when its surface goes up and hands them back when it \
         comes down, so there is nothing left for an ordering to protect."
    );
}

#[test]
fn the_way_to_game_mode_shuts_steam_down_before_the_compositor() {
    let at = root().join("files/usr/local/bin/steamos-session-select");
    let held = std::fs::read_to_string(&at).expect("the session switcher");
    let asked = held.find("\n    settle\n").expect("nothing asks Steam to go");
    let left = held.find("hyprctl dispatch").expect("nothing leaves the compositor");
    assert!(asked < left, "Steam is asked to go once the desktop it was on has gone");
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

#[test]
fn the_font_the_bar_draws_its_icons_in_is_one_the_manifest_installs() {
    let style = std::fs::read_to_string(
        root().join("files/home/@user@/.config/waybar/style.css"),
    )
    .expect("the bar's stylesheet");
    let asked = style
        .lines()
        .find(|line| line.trim_start().starts_with("font-family:"))
        .expect("a font-family");
    assert!(
        asked.contains("Nerd Font Mono"),
        "the bar asks for {asked:?}. Only the Mono cut draws these glyphs centred in \
         their cell; in the others the ink overflows the advance and hangs off the right, \
         which puts every icon a different distance off centre"
    );
    let packages: BTreeSet<String> = section(&manifest(), "packages").into_iter().collect();
    assert!(
        packages.contains("ttf-fantasque-nerd"),
        "[packages] does not name the font the stylesheet asks for"
    );
}

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
                None => held.get("package").and_then(named).into_iter().collect(),
            }
        })
        .collect()
}

fn mode_of(live: &str, head: &[u8]) -> u32 {
    match live {
        path if path.contains("/bin/") || path.contains("/sbin/") => 0o755,
        _ => match head {
            [b'#', b'!', ..] | [0x7f, b'E', b'L', b'F', ..] => 0o755,
            _ => 0o644,
        },
    }
}

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
                        out.push(line.split_whitespace().next().unwrap_or("").to_string());
                    }
                    (out, at)
                }
            }
        })
        .0
}

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

#[test]
fn every_program_a_carried_script_reaches_for_is_carried() {
    let held = manifest();
    let listed = carried_or_declared(&held);
    for path in every("/usr/local/bin/", "") {
        let said = match std::fs::read_to_string(&path) {
            Ok(said) => said,
            Err(_fault) => continue,
        };
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        for at in reaches_for(&said) {
            assert!(listed.contains(&at), "{name} runs {at}, which is not carried");
        }
    }
}

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
        let said = match std::fs::read_to_string(&path) {
            Ok(said) => said,
            Err(_fault) => continue,
        };
        let first = said.lines().next().unwrap_or_default();
        if !(first.starts_with("#!") && (first.contains("/sh") || first.contains("bash"))) {
            continue;
        }
        let Ok(mut asking) = Program::Sh.command();
        let done = asking.arg("-n").arg(&path).output().expect("sh");
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

const OUTSIDE: [&str; 3] = ["activate", "makoctl", "wpctl"];

fn bar_modules() -> Vec<(String, serde_json::Map<String, serde_json::Value>)> {
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
    let bars = match read {
        serde_json::Value::Array(bars) => bars,
        one => vec![one],
    };
    bars.into_iter()
        .filter_map(|bar| bar.as_object().cloned())
        .flat_map(|bar| {
            bar.into_iter()
                .filter_map(|(module, about)| {
                    about.as_object().map(|about| (module, about.clone()))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn bar_commands() -> Vec<(String, String)> {
    bar_modules()
        .into_iter()
        .flat_map(|(module, about)| {
            about
                .iter()
                .filter(|(key, _)| key.starts_with("on-"))
                .filter_map(|(_, command)| command.as_str())
                .filter_map(|command| {
                    command
                        .split_whitespace()
                        .find(|word| !word.contains('=') || word.starts_with('-'))
                })
                .map(|word| (module.clone(), word.to_string()))
                .collect::<Vec<(String, String)>>()
        })
        .collect()
}

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

#[test]
fn the_bar_has_a_door_for_the_menu_and_for_the_keyboard() {
    let runs: BTreeSet<String> = bar_commands().into_iter().map(|(_, command)| command).collect();
    assert!(runs.contains("launcher"), "there is no way to open the menu by hand");
    assert!(runs.contains("keyboard-toggle"), "there is no way to ask for the keyboard by hand");
}

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

#[test]
fn nothing_matches_a_process_by_a_name_the_kernel_cannot_hold() {
    const COMM: usize = 15;

    let bin = root().join("files/usr/local/bin");
    let inside = std::fs::read_dir(&bin).expect("the installed scripts");
    let mut asked = 0;
    for found in inside.flatten() {
        let path = found.path();
        let held = match std::fs::read_to_string(&path) {
            Ok(held) => held,
            Err(_fault) => continue,
        };
        for line in held.lines() {
            let line = line.trim();
            if line.starts_with('#') || !(line.contains("pkill") || line.contains("pgrep")) {
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            let by_name = words.contains(&"-x") && !words.contains(&"-f");
            if !by_name {
                continue;
            }
            asked += 1;
            let pattern = words
                .iter()
                .skip_while(|word| **word != "-x")
                .nth(1)
                .unwrap_or(&"");
            assert!(
                pattern.len() <= COMM,
                "{} matches a process by the name {pattern:?}, which is {} characters. The \
                 kernel keeps {COMM}, so this matches nothing and fails silently. Match the \
                 path with -f instead.",
                path.display(),
                pattern.len()
            );
        }
    }
    let _ = asked;
}

#[test]
fn the_toggle_names_the_keyboard_the_manifest_installs() {
    let installed = format!("/usr/local/bin/{}", console_input_controller::mode::KEYBOARD);
    assert_eq!(
        console_input_keyboard::asked::asking(std::path::Path::new(&installed)),
        Ok(format!("^{installed}( |$)")),
        "the two ways of asking signal a path the manifest does not install, so X reaches nothing"
    );
    let held = manifest();
    let carried = section(&held, "files").iter().any(|path| path == &installed);
    let built = section(&held, "build")
        .iter()
        .any(|name| installed == format!("/usr/local/bin/{name}"));
    assert!(
        carried || built,
        "the manifest neither carries nor builds {installed}, so the toggle names a program \
         nobody has"
    );
}

#[test]
fn the_bar_and_the_engine_agree_on_the_signal() {
    let engine =
        std::fs::read_to_string(root().join("crates/console-notifications/src/updating.rs"))
            .expect("updating.rs");
    let sent = engine
        .lines()
        .find_map(|line| line.split_once("\"-RTMIN+"))
        .and_then(|(_, rest)| rest.split('"').next())
        .expect("nothing names a real-time signal to wake the bar with");

    let config = std::fs::read_to_string(root().join("files/home/@user@/.config/waybar/config.jsonc"))
        .expect("the waybar config");
    let module = config
        .split_once("\"custom/updating\"")
        .expect("the bar has no updating module")
        .1;
    let heard = module
        .split_once("\"signal\"")
        .expect("the updating module is never signalled, so it would never update")
        .1
        .trim_start_matches([':', ' '])
        .split([',', '\n', '}'])
        .next()
        .expect("a number")
        .trim();

    assert_eq!(
        sent, heard,
        "the engine sends SIGRTMIN+{sent} and the bar listens for SIGRTMIN+{heard}, so the bar \
         would never hear that an apply had moved on"
    );
}

#[test]
fn every_module_reads_from_a_program_the_manifest_builds() {
    let built = programs();
    let mut asked = 0;
    for (module, about) in bar_modules() {
        let run = match about.get("exec").and_then(|run| run.as_str()) {
            Some(run) => run,
            None => continue,
        };
        let program = run.split_whitespace().next().expect("something to run");
        asked += 1;
        assert!(
            built.iter().any(|one| one == program),
            "the bar's {module} reads from {program}, which is not in the manifest's [build]"
        );
    }
    assert!(asked > 1, "no module reads from anything, so this test asked nothing");
}

#[test]
fn the_strip_is_as_wide_as_the_screen() {
    let screen = std::fs::read_to_string(root().join("files/home/@user@/.config/hypr/hyprland.lua"))
        .expect("the compositor's config");
    let field = |name: &str| {
        screen
            .lines()
            .find_map(|line| line.trim().strip_prefix(name)?.split_once('='))
            .map(|(_, said)| said.trim().trim_matches([',', '"', ' ']).to_string())
            .unwrap_or_else(|| panic!("the monitor has no {name}"))
    };
    let mode = field("mode");
    let (across, down) = mode.split_once('x').expect("a mode like 1600x2560@144");
    let across: f64 = across.parse().expect("a width");
    let down: f64 = down.split('@').next().expect("a height").parse().expect("a height");
    let scale: f64 = field("scale").parse().expect("a scale");
    let turned = field("transform").parse::<u32>().expect("a transform") % 2 == 1;
    let wide = match turned {
        true => down,
        false => across,
    };
    let logical = (wide / scale).round() as u32;

    let style = std::fs::read_to_string(root().join("files/home/@user@/.config/waybar/style.css"))
        .expect("the waybar stylesheet");
    let written: u32 = style
        .lines()
        .find_map(|line| line.trim().strip_prefix("min-width:")?.trim().strip_suffix("px;"))
        .expect("nothing on the bar sets a min-width")
        .trim()
        .parse()
        .expect("a width in pixels");

    assert_eq!(
        written, logical,
        "the strip is {written} logical pixels wide and the screen is {logical}, so an apply \
         would fill it to the wrong place"
    );
}

#[test]
fn the_compositors_file_binds_nothing() {
    let lua = std::fs::read_to_string(root().join("files/home/@user@/.config/hypr/hyprland.lua"))
        .expect("the compositor's config");

    let bound: Vec<&str> = lua.lines().filter(|line| line.contains("hl.bind(")).collect();

    assert!(
        bound.is_empty(),
        "the table is the only place that says what a press does, and this file says it too: {}",
        bound.join("\n")
    );
}

#[test]
fn putting_the_screen_back_puts_the_panel_on_before_it_reads_the_note() {
    let said =
        std::fs::read_to_string(root().join("crates/console-settings/src/bin/console-brightness.rs"))
            .expect("console-brightness");
    let body = said.split("fn undim(").nth(1).expect("undim");
    let body = body.split("\nfn ").next().expect("the end of it");

    let on = body.find("panel_on()").expect("undim does not put the panel on");
    let note = body.find("remembered()").expect("undim does not read the note");

    assert!(on < note, "the panel is put on only after a note that can send undim home early");
    assert!(
        said.contains(r#"hl.dsp.dpms({ action = "enable" })"#),
        "the dpms is not the Lua form, which is the one this compositor answers"
    );
}

#[test]
fn one_thing_puts_the_panel_back_on_when_the_machine_wakes() {
    let held =
        std::fs::read_to_string(root().join("files/home/@user@/.config/hypr/hypridle.conf"))
            .expect("hypridle.conf");

    let waking: Vec<&str> = held
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("on-resume"))
        .collect();

    assert!(!waking.is_empty(), "nothing in hypridle answers a machine waking up");

    for line in waking {
        assert!(
            line.contains("console-brightness undim"),
            "{line}\nhypridle resumes every listener that timed out at the same instant, and \
             `console-brightness undim` already puts the panel on with this dispatch. A second \
             one races it: both say ok, the compositor reads as on, and the screen stays dark."
        );
    }
}

fn holding() -> Vec<(String, PathBuf)> {
    let crates = root().join("crates");
    std::fs::read_dir(&crates)
        .expect("crates/")
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|at| std::fs::read_to_string(at.join("Cargo.toml")).ok().map(|held| (at, held)))
        .filter_map(|(at, held)| held.parse::<toml::Table>().ok().map(|held| (at, held)))
        .flat_map(|(at, held)| {
            let bins = held.get("bin").and_then(toml::Value::as_array).cloned().unwrap_or_default();
            bins.into_iter()
                .filter_map(move |one| {
                    let name = one.get("name").and_then(toml::Value::as_str)?.to_owned();
                    let path = one.get("path").and_then(toml::Value::as_str)?;
                    Some((name, at.join(path)))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn modules(held: &str) -> BTreeSet<(String, String)> {
    let ident = |from: &str| -> String {
        from.chars().take_while(|one| one.is_alphanumeric() || *one == '_').collect()
    };

    held.match_indices("console_")
        .filter_map(|(at, _)| {
            let rest = held.get(at..)?;
            let whole = ident(rest);
            let after = rest.get(whole.len()..)?.strip_prefix("::")?;
            let module = ident(after);

            match module.is_empty() || module.chars().next()?.is_uppercase() {
                true => None,
                false => Some((whole.replace('_', "-"), module)),
            }
        })
        .collect()
}

fn reached_by(bin: &Path) -> String {
    let own = std::fs::read_to_string(bin).unwrap_or_default();
    let crates = root().join("crates");

    let mut held = own.clone();

    for (what, module) in modules(&own) {
        let src = crates.join(&what).join("src");
        let one = src.join(format!("{module}.rs"));
        let folded = src.join(&module).join("mod.rs");

        held.push_str(&std::fs::read_to_string(&one).unwrap_or_default());
        held.push_str(&std::fs::read_to_string(&folded).unwrap_or_default());
    }

    held
}

const FORCES: [&str; 15] = [
    "LockPersonality",
    "MemoryDenyWriteExecute",
    "NoNewPrivileges",
    "PrivateDevices",
    "ProtectClock",
    "ProtectHostname",
    "ProtectKernelLogs",
    "ProtectKernelModules",
    "ProtectKernelTunables",
    "RestrictAddressFamilies",
    "RestrictNamespaces",
    "RestrictRealtime",
    "RestrictSUIDSGID",
    "SystemCallArchitectures",
    "SystemCallFilter",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    Seccomp,
    Free,
}

fn bound(value: &str) -> Bound {
    match value.trim() {
        "" | "no" | "false" | "off" | "0" => Bound::Free,
        _ => Bound::Seccomp,
    }
}

fn read_for(unit: &Path) -> Vec<PathBuf> {
    let named = unit.file_name().expect("a unit name").to_string_lossy().into_owned();
    let whose = match named.strip_suffix(".service") {
        Some(whose) => whose,
        None => return Vec::new(),
    };

    let mut theirs: Vec<PathBuf> = every("etc/systemd/user", ".conf")
        .into_iter()
        .filter(|path| {
            let over = path
                .parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let stem = match over.strip_suffix(".service.d") {
                Some(stem) => stem,
                None => return false,
            };

            stem == whose || (stem.ends_with('-') && whose.starts_with(stem))
        })
        .collect();

    theirs.sort_by_key(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()));

    theirs
}

fn still_seccomp(unit: &Path) -> BTreeSet<String> {
    let mut standing: BTreeMap<String, Bound> = BTreeMap::new();

    for path in std::iter::once(unit.to_path_buf()).chain(read_for(unit)) {
        let held = std::fs::read_to_string(&path).unwrap_or_default();

        for line in section(&held, "Service") {
            let (key, value) = match line.split_once('=') {
                Some((key, value)) => (key, value),
                None => continue,
            };
            let key = key.trim();

            match FORCES.contains(&key) {
                true => {
                    standing.insert(key.to_owned(), bound(value));
                }
                false => {}
            }
        }
    }

    standing
        .into_iter()
        .filter(|(_, how)| *how == Bound::Seccomp)
        .map(|(key, _)| key)
        .collect()
}

#[test]
fn every_unit_that_can_cross_to_game_mode_may_still_become_root() {
    let switcher = std::fs::read_to_string(root().join("files/usr/local/bin/steamos-session-select"))
        .expect("the session switcher");
    assert!(
        switcher.contains("pkexec"),
        "the switcher no longer becomes root, and what is below is only about the fact that it does"
    );

    let confining = std::fs::read_to_string(
        root().join("files/etc/systemd/user/console-.service.d/confining.conf"),
    )
    .expect("the confinement");
    assert!(
        confining.contains("NoNewPrivileges=yes"),
        "nothing takes the setuid bit away any more, so nothing has to be given it back"
    );

    let holds = holding();
    let crossing: BTreeSet<String> = holds
        .iter()
        .filter(|(_, bin)| reached_by(bin).contains("console_session::"))
        .map(|(name, _)| name.clone())
        .collect();

    assert!(!crossing.is_empty(), "nothing in the tree crosses to Game Mode");

    for unit in every("etc/systemd/user", ".service") {
        let held = std::fs::read_to_string(&unit).unwrap_or_default();
        let starts: BTreeSet<String> = named_by(&held)
            .into_iter()
            .filter_map(|path| path.rsplit_once('/').map(|(_, name)| name.to_string()))
            .collect();

        let reaches = starts.iter().any(|name| {
            crossing.contains(name)
                || holds
                    .iter()
                    .filter(|(held, _)| held == name)
                    .any(|(_, bin)| {
                        let near = reached_by(bin);
                        crossing.iter().any(|far| near.contains(far.as_str()))
                    })
        });

        match reaches {
            false => continue,
            true => {}
        }

        let named = unit.file_name().expect("a unit name").to_string_lossy().into_owned();
        let standing = still_seccomp(&unit);

        assert!(
            standing.is_empty(),
            "{named} starts a program that crosses to Game Mode, which is done with pkexec, and \
             after everything read for it {standing:?} is still set. Each of those is seccomp, \
             and a unit of an unprivileged manager carrying any seccomp at all is a process with \
             the no-new-privileges bit whatever NoNewPrivileges= says: pkexec fails with \
             \"pkexec must be setuid root\", the session is never recorded, and the button does \
             nothing."
        );
    }
}
