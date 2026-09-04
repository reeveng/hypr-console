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
    {
    // Tidied by `canonicalize` where that works and left as it stands where it
    // does not. What `CARGO_MANIFEST_DIR` gives is already absolute and already
    // right; canonicalizing only takes the `../..` out of the middle. It fails
    // under a sandbox that will not let a process resolve a path it can
    // otherwise read, and a test that stops there reports the sandbox as a
    // missing repository.
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
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

/// The at-words GTK's stylesheet language has of its own, which name a rule
/// rather than a colour. Everything else after an `@` is a colour somebody
/// defined, or meant to.
const AT_RULES: [&str; 16] = [
    "import", "define-color", "media", "keyframes", "supports", "namespace", "charset",
    "font-face", "layer", "property", "container", "page", "document", "scope", "starting-style",
    "else",
];

mod the_names {
    use super::*;

    /// Every colour the desktop asks for by name is one something defines.
    ///
    /// GTK does not fail on `@fill` where nothing defined `fill`. It drops the
    /// one declaration and carries on, so the file parses, the widget lays out,
    /// and the only sign is a thing that never paints. The strip under the bar
    /// spent a release like that: every one of its gradient rules named a
    /// colour the palette did not write, so it filled to no percentage an apply
    /// ever reported. Nothing logged, nothing failed, and it was never seen.
    ///
    /// So the two lists are held together here rather than by whoever next
    /// reads both files: a name asked for, and a name defined.
    #[test]
    fn every_name_the_desktop_asks_for_is_defined() {
        // Both trees. `files` is what is laid on the machine, and `crates` is
        // where a program keeps the stylesheet it loads from a string of its
        // own -- the home screen's is there, and asked for a colour nobody had
        // defined for as long as it existed. A check that looked at only the
        // first of these would have gone on passing over it.
        let files = root().join("files");
        let sheets: Vec<(PathBuf, String)> = [files.clone(), root().join("crates")]
            .iter()
            .flat_map(|tree| carrying(tree))
            .filter(|(path, _)| path.extension().is_some_and(|end| end == "css"))
            .collect();

        assert!(!sheets.is_empty(), "no stylesheets under {}", files.display());

        let at = Regex::new(r"@([A-Za-z_][A-Za-z0-9_-]*)").expect("a pattern");

        // Every name defined anywhere among them. The sheets `@import` one
        // another, so a name defined in the palette is a name the bar may use.
        let defined: BTreeSet<String> = sheets
            .iter()
            .flat_map(|(_, said)| {
                said.lines()
                    .filter_map(|line| line.trim().strip_prefix("@define-color "))
                    .filter_map(|rest| rest.split_whitespace().next())
                    .map(str::to_string)
            })
            .collect();

        assert!(defined.contains("fill"), "the palette defines no `fill`, and the strip asks for it");

        let mut missing: Vec<String> = Vec::new();

        for (path, said) in &sheets {
            for line in said.lines() {
                // A comment can hold an `@` as prose -- the strip's own
                // stylesheet explains itself in one -- and prose is not a rule.
                let code = line.split("/*").next().unwrap_or(line);

                for found in at.captures_iter(code) {
                    let name = &found[1];

                    if AT_RULES.contains(&name) || defined.contains(name) {
                        continue;
                    }

                    missing.push(format!(
                        "{} asks for @{name}, which nothing defines",
                        path.strip_prefix(&files).unwrap_or(path).display()
                    ));
                }
            }
        }

        missing.dedup();

        assert!(missing.is_empty(), "a colour nobody defined is a rule GTK drops:\n  {}", missing.join("\n  "));
    }

    /// The same, for the half of the desktop that speaks the browser's CSS.
    ///
    /// The add-on's stylesheets do not say `@name`; they say `var(--name)`, and
    /// the palette written for them defines custom properties rather than GTK
    /// colours. So the check above sweeps those files and finds nothing to look
    /// at in them -- the names it knows how to read are not the names they use.
    ///
    /// The failure is the same failure. A `var()` naming a property nobody
    /// defined is invalid at computed-value time, which is the browser's way of
    /// dropping one declaration and carrying on: the rule parses, the element
    /// lays out, and the colour is simply not the one anybody wrote. In a shadow
    /// root nothing is even logged.
    ///
    /// A `var()` given a fallback is not this fault -- it named a second answer
    /// on purpose -- so those are left alone.
    #[test]
    fn every_property_the_browser_asks_for_is_defined() {
        let files = root().join("files");
        let sheets: Vec<(PathBuf, String)> = [files.clone(), root().join("crates")]
            .iter()
            .flat_map(|tree| carrying(tree))
            .filter(|(path, _)| path.extension().is_some_and(|end| end == "css"))
            .collect();

        let held = Regex::new(r"(?m)^\s*(--[A-Za-z0-9_-]+)\s*:").expect("a pattern");
        let asked = Regex::new(r"var\(\s*(--[A-Za-z0-9_-]+)\s*([,)])").expect("a pattern");

        let defined: BTreeSet<String> = sheets
            .iter()
            .flat_map(|(_, said)| held.captures_iter(said).map(|found| found[1].to_string()))
            .collect();

        assert!(defined.contains("--text"), "the browser's palette defines nothing");

        let mut missing: Vec<String> = Vec::new();

        for (path, said) in &sheets {
            for line in said.lines() {
                let code = line.split("/*").next().unwrap_or(line);

                for found in asked.captures_iter(code) {
                    // A comma is a fallback, which is somebody saying what to
                    // do when the name is not there. That is an answer, not a
                    // hole.
                    if &found[2] == "," || defined.contains(&found[1]) {
                        continue;
                    }

                    missing.push(format!(
                        "{} asks for var({}), which nothing defines",
                        path.strip_prefix(&files).unwrap_or(path).display(),
                        &found[1]
                    ));
                }
            }
        }

        missing.dedup();

        assert!(
            missing.is_empty(),
            "a property nobody defined is a declaration the browser throws away:\n  {}",
            missing.join("\n  ")
        );
    }
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

    /// The three pairings APCA's own documentation states an answer for.
    ///
    /// The same argument as the vectors above and the more important half of
    /// it: the ratio is arithmetic anybody can check by hand, and this is not.
    /// It has two exponents, a soft clamp and an offset, and getting any of
    /// them slightly wrong gives numbers that look entirely plausible and are
    /// wrong everywhere. These are the published values, so they are the only
    /// thing here that did not come out of this implementation.
    const APCA: [(&str, &str, f64); 3] = [
        ("000000", "ffffff", 106.04),
        ("ffffff", "000000", -107.88),
        ("888888", "ffffff", 63.06),
    ];

    #[test]
    fn the_apca_numbers_are_the_published_ones() {
        for (ink, ground, expected) in APCA {
            let got = col::lc(ink, ground);
            assert!(
                (got - expected).abs() < 0.01,
                "#{ink} on #{ground} is Lc {got:.3}, published as Lc {expected}"
            );
        }
    }

    #[test]
    fn the_polarity_is_the_whole_point_and_is_not_symmetric() {
        // The fact a ratio cannot express. Swap the ink and the ground and
        // WCAG gives the same number back; APCA does not, and the difference
        // is which of the two is the paper.
        assert_eq!(col::contrast("000000", "ffffff"), col::contrast("ffffff", "000000"));
        assert!(col::lc("ffffff", "000000").abs() != col::lc("000000", "ffffff").abs());
    }

    #[test]
    fn a_colour_on_itself_is_no_contrast_in_either_measure() {
        assert!((col::contrast("372c3a", "372c3a") - 1.0).abs() < 1e-12);
        assert_eq!(col::lc("372c3a", "372c3a"), 0.0);
    }

    #[test]
    fn wcag_flatters_a_dark_pair_and_apca_does_not() {
        // The reason this palette asks for both, in one pair of assertions.
        // The same grey is a better ratio on black than on white and a far
        // worse Lc, and only one of those two claims matches what an eye does.
        let (on_black, on_white) = (
            col::contrast("767676", "000000"),
            col::contrast("767676", "ffffff"),
        );
        assert!(on_black > on_white, "{on_black} should beat {on_white}");

        let (lc_black, lc_white) = (
            col::lc("767676", "000000").abs(),
            col::lc("767676", "ffffff").abs(),
        );
        assert!(lc_black < lc_white, "Lc {lc_black} should be under Lc {lc_white}");
    }
}

mod the_palette {
    use super::*;

    #[test]
    fn every_pairing_clears_what_it_declares() {
        // The whole promise, in one assertion.
        let done = check();
        assert!(done.status.success(), "{}{}", done.stdout, done.stderr);
        assert!(done.stdout.contains("all clearing both measures"), "{}", done.stdout);
    }

    #[test]
    fn the_files_say_what_the_palette_says() {
        // Nothing has been edited in place since the palette was last spent.
        let done = check();
        assert!(
            done.status.success(),
            "a themed file no longer matches theme/palette.toml. Run `just theme`.\n{}{}",
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
