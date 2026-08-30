//! The palette says what this desktop looks like. These are the ways it can lie.
//!
//! Three things are checked, and the middle one is the reason the other two
//! are here. Colours can be wrong by being unreadable, which is what the
//! ratios are for. They can be wrong by having been changed in one file and
//! not in another, which is what the drift check is for. And the engine that
//! computes both can itself be wrong, which is what the vectors at the bottom
//! are for: they were produced by a different implementation in a different
//! language, and if this one ever stops agreeing with them then every number
//! in the report is a number nobody should trust.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_colour as col;
use regex::Regex;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

/// Every way a colour is written down on this machine.
///
/// A stylesheet says `#rrggbb`, a terminal says `0xrrggbb`, the compositor
/// says `rgba(rrggbbaa)`, a shell variable says the digits bare, and KDE says
/// three decimal numbers. Anchoring the last two to an assignment keeps a font
/// size out of it: KDE writes `font=Noto Sans,16,-1,5,400,0,0` in the same
/// file as its colours.
fn colour() -> Regex {
    // Joined rather than written as one string: a raw string takes no line
    // continuation, so a pattern split over lines to be read carries the
    // newlines into itself and quietly stops matching most of what it names.
    let ways = [
        r"#([0-9a-fA-F]{6})\b",
        r"0x([0-9a-fA-F]{6})\b",
        r"rgba\(([0-9a-fA-F]{6})ff\)",
        r"^\w+=([0-9a-fA-F]{6})$",
        r"^\w+=(\d{1,3},\s?\d{1,3},\s?\d{1,3})$",
    ];
    Regex::new(&format!("(?m){}", ways.join("|"))).expect("the pattern compiles")
}

/// Every file under `files/` that a person could have typed a colour into.
fn carrying(files: &Path) -> Vec<(PathBuf, String)> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(at) else { return };
        let mut found: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        found.sort();
        for path in found {
            match path {
                path if path.ends_with("__pycache__") => {}
                path if path.is_dir() => walk(&path, into),
                path => into.push(path),
            }
        }
    }
    let mut paths = Vec::new();
    walk(files, &mut paths);
    paths
        .into_iter()
        // The keyboard and hyprsession are compiled programs.
        .filter_map(|path| std::fs::read(&path).ok().map(|held| (path, held)))
        .filter_map(|(path, held)| String::from_utf8(held).ok().map(|text| (path, text)))
        .collect()
}

/// A declared colour as any of the ways it may be written.
fn forms<'a>(codes: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    codes
        .flat_map(|code| {
            let decimal = [0, 2, 4]
                .map(|at| u8::from_str_radix(&code[at..at + 2], 16).unwrap_or(0).to_string())
                .join(",");
            [code.to_lowercase(), decimal]
        })
        .collect()
}

mod the_engine {
    use super::*;

    /// Produced by `Codincod.Design.Oklch`, which is the same arithmetic
    /// written independently in Elixir for the site's themes and checked by
    /// its own tests. Two implementations agreeing on a colour and a ratio is
    /// worth more than one implementation agreeing with itself, and these are
    /// the numbers that agreement was recorded at.
    const VECTORS: [(f64, f64, f64, &str, f64); 11] = [
        (0.125, 0.014, 318.0, "08050a", 1.3119),
        (0.215, 0.020, 318.0, "1d1720", 1.1372),
        (0.290, 0.026, 318.0, "312734", 1.0814),
        (0.480, 0.030, 318.0, "655969", 2.3433),
        (0.560, 0.038, 318.0, "7e6e83", 3.2692),
        (0.860, 0.022, 335.0, "dbccd7", 10.0245),
        (0.760, 0.038, 332.0, "c0a9bc", 7.0917),
        (0.855, 0.105, 342.0, "ffb5e2", 9.5312),
        (0.855, 0.080, 178.0, "95e1cf", 10.2588),
        (0.855, 0.095, 20.0, "ffbbba", 9.6117),
        (0.930, 0.085, 238.0, "d1ecff", 12.6153),
    ];

    #[test]
    fn it_agrees_with_the_other_implementation() {
        for (lightness, chroma, hue, expected, ratio) in VECTORS {
            let got = col::hexcode(lightness, chroma, hue);
            assert_eq!(got, expected, "at oklch({lightness} {chroma} {hue})");
            let reached = col::contrast(&got, "2b212e");
            assert!(
                (reached - ratio).abs() < 1e-4,
                "#{got} on #2b212e is {reached:.4}:1, recorded as {ratio}:1"
            );
        }
    }
}

mod the_palette {
    use super::*;

    #[test]
    fn every_pairing_clears_what_it_declares() {
        // The whole promise, in one assertion.
        let done = check();
        assert!(done.status.success(), "{}{}", done.stdout, done.stderr);
        assert!(done.stdout.contains("all clearing what they declare"), "{}", done.stdout);
    }

    #[test]
    fn the_files_say_what_the_palette_says() {
        // Nothing has been edited in place since the palette was last spent.
        let done = check();
        assert!(
            done.status.success(),
            "a themed file no longer matches theme/palette.toml. Run `make theme`.\n{}{}",
            done.stdout,
            done.stderr
        );
    }

    #[test]
    fn every_colour_says_what_it_is_for() {
        let declared = std::fs::read_to_string(root().join("theme/palette.toml")).expect("read");
        let spec: toml::Table = declared.parse().expect("it parses");
        let colours = spec["colour"].as_table().expect("a table of colours");
        for (name, declared) in colours {
            let spent = declared.get("spent").and_then(toml::Value::as_str).unwrap_or("");
            assert!(!spent.is_empty(), "{name} does not say what it is spent on");
        }
    }
}

mod the_tree {
    use super::*;

    /// Every hex installed on the machine is one the palette declares.
    ///
    /// Not only the files the generator writes: the whole tree, so that a
    /// colour typed in by hand is caught wherever somebody types it. That is
    /// the drift this was built to stop. A hex put in by hand is invisible
    /// until somebody looks at the screen in the right light, and by then it
    /// has been there for months.
    #[test]
    fn no_file_anywhere_carries_a_colour_from_outside_the_palette() {
        let (root, spent) = (root(), spent());
        let lifted: Vec<String> = spent
            .iter()
            .map(|(_, code)| col::lift(code, bright_lift()))
            .collect();
        let known: BTreeSet<String> = forms(spent.iter().map(|(_, code)| code.as_str()))
            .into_iter()
            .chain(forms(lifted.iter().map(String::as_str)))
            .collect();

        let pattern = colour();
        for (path, text) in carrying(&root.join("files")) {
            for found in pattern.captures_iter(&text) {
                let written = found
                    .iter()
                    .skip(1)
                    .flatten()
                    .next()
                    .expect("one group matched")
                    .as_str()
                    .to_lowercase()
                    .replace(' ', "");
                assert!(
                    known.contains(&written),
                    "{} carries #{written}, which is not a colour theme/palette.toml declares",
                    path.strip_prefix(&root).unwrap_or(&path).display()
                );
            }
        }
    }

    /// And the rest of the desktop imports it.
    ///
    /// A stylesheet, a terminal, a keyboard and a browser can each import a
    /// file written in their own language, so each of them does, and the hex
    /// lives in one place per language rather than in every file that spends
    /// it. The ones that cannot import anything are KDE's ini format and
    /// mako's, neither of which has an include, a `user.js`, which is a list
    /// of literals, the compositor, whose config is written rather than
    /// imported because a Lua file that fails to load takes the session with
    /// it, and a picture.
    #[test]
    fn only_the_palette_holds_a_colour() {
        let allowed: BTreeSet<&str> = BTreeSet::from([
            "home/@user@/.config/hypr/hyprland.lua",
            "home/@user@/.config/kdeglobals",
            "home/@user@/.config/mako/config",
            "home/@user@/.config/console/palette.css",
            "home/@user@/.config/console/palette.toml",
            "home/@user@/.librewolf/console/chrome/palette.css",
            "home/@user@/.librewolf/console/user.js",
            "usr/local/lib/console/palette.sh",
            "usr/share/icons/console-placeholder.svg",
        ]);
        let files = root().join("files");
        let pattern = colour();
        let holding: BTreeSet<String> = carrying(&files)
            .into_iter()
            .filter(|(_, text)| pattern.is_match(text))
            .map(|(path, _)| path.strip_prefix(&files).expect("under files/").display().to_string())
            .collect();
        let allowed: BTreeSet<String> = allowed.iter().map(|name| name.to_string()).collect();
        assert_eq!(
            holding, allowed,
            "a file outside the palette has grown a colour, or one inside it has lost \
             the only colour it had"
        );
    }

    /// A colour nothing uses is a colour nobody maintains.
    #[test]
    fn every_colour_is_spent() {
        let written: String = carrying(&root().join("files"))
            .into_iter()
            .map(|(_, text)| text.to_lowercase())
            .collect();
        for (name, code) in spent() {
            assert!(
                written.contains(&code.to_lowercase()),
                "{name} (#{code}) is declared and never used"
            );
        }
    }
}

// ------------------------------------------------------------ what runs it

struct Said {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn check() -> Said {
    let done = std::process::Command::new(env!("CARGO_BIN_EXE_console-theme"))
        .arg("--check")
        .current_dir(root())
        .output()
        .expect("console-theme runs");
    Said {
        status: done.status,
        stdout: String::from_utf8_lossy(&done.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&done.stderr).into_owned(),
    }
}

/// The palette as it stands, as (name, six hex digits).
///
/// Taken by running the tool rather than by linking to it, because the tool is
/// a binary and its insides are its own. The report is the palette written
/// down, so it is read from there.
fn spent() -> Vec<(String, String)> {
    let report = std::fs::read_to_string(root().join("theme/report.md")).expect("the report");
    report
        .lines()
        .filter_map(|line| {
            let mut cells = line.split('|').map(str::trim);
            match (cells.next(), cells.next(), cells.next()) {
                (Some(""), Some(name), Some(code))
                    if name.starts_with('`') && code.starts_with("`#") =>
                {
                    Some((
                        name.trim_matches('`').to_string(),
                        code.trim_matches('`').trim_start_matches('#').to_string(),
                    ))
                }
                _ => None,
            }
        })
        .filter(|(_, code)| code.len() == 6 && code.chars().all(|c| c.is_ascii_hexdigit()))
        .collect()
}

fn bright_lift() -> f64 {
    let declared = std::fs::read_to_string(root().join("theme/palette.toml")).expect("read");
    let spec: toml::Table = declared.parse().expect("it parses");
    spec["terminal"]["bright_lift"].as_float().expect("a number")
}
