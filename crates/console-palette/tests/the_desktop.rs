//! The palette says what this desktop looks like. These are the ways it can lie.
//!
//! Three things are checked, and the middle one is the reason the other two
//! are here. Colors can be wrong by being unreadable, which is what the
//! ratios are for. They can be wrong by having been changed in one file and
//! not in another, which is what the drift check is for. And the engine that
//! computes both can itself be wrong, which is what the vectors at the bottom
//! are for: they were produced by a different implementation in a different
//! language, and if this one ever stops agreeing with them then every number
//! in the report is a number no one should trust.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_core_color::{Ground, HexColor};
use console_core_color as color;

fn root() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

const HEX: u32 = 6;

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn is_name(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

fn run_of(said: &str, taken: impl Fn(char) -> bool) -> (&str, &str) {
    match said.find(|character: char| !taken(character)) {
        Some(end) => said.split_at(end),
        None => (said, ""),
    }
}

fn hex_six(said: &str) -> Option<(&str, &str)> {
    let (code, after) = said.split_at_checked(HEX.try_into().ok()?)?;

    match code.chars().all(|character| character.is_ascii_hexdigit()) {
        true => Some((code, after)),
        false => None,
    }
}

fn hex_word(said: &str) -> Option<(&str, &str)> {
    let (code, after) = hex_six(said)?;

    match after.chars().next() {
        Some(character) if is_word(character) => None,
        Some(_) | None => Some((code, after)),
    }
}

fn decimal_triple(said: &str) -> bool {
    let mut left = said;

    for band in 0..3u8 {
        let (digits, after) = run_of(left, |character| character.is_ascii_digit());

        match (1..=3).contains(&digits.len()) {
            true => {}
            false => return false,
        }

        left = match band {
            2 => after,
            _ => {
                let past = match after.strip_prefix(',') {
                    Some(past) => past,
                    None => return false,
                };

                past.strip_prefix(char::is_whitespace).unwrap_or(past)
            }
        };
    }

    left.is_empty()
}

fn color_at<'a>(rest: &'a str, line: Option<&'a str>) -> Option<(String, &'a str)> {
    match rest.strip_prefix('#').and_then(hex_word) {
        Some((code, after)) => return Some((code.to_string(), after)),
        None => {}
    }

    match rest.strip_prefix("0x").and_then(hex_word) {
        Some((code, after)) => return Some((code.to_string(), after)),
        None => {}
    }

    let opaque = rest
        .strip_prefix("rgba(")
        .and_then(hex_six)
        .and_then(|(code, after)| after.strip_prefix("ff)").map(|after| (code, after)));

    match opaque {
        Some((code, after)) => return Some((code.to_string(), after)),
        None => {}
    }

    let line = line?;
    let (named, after) = run_of(line, is_word);
    let value = match named.is_empty() {
        true => return None,
        false => after.strip_prefix('=')?,
    };
    let past_line = rest.strip_prefix(line)?;

    match hex_six(value) {
        Some((code, "")) => return Some((code.to_string(), past_line)),
        Some(_) | None => {}
    }

    match decimal_triple(value) {
        true => Some((value.to_string(), past_line)),
        false => None,
    }
}

fn colors_in(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = text;
    let mut opens = true;

    while !rest.is_empty() {
        let line = match opens {
            true => rest.split('\n').next(),
            false => None,
        };

        match color_at(rest, line) {
            Some((code, after)) => {
                found.push(code);
                rest = after;
                opens = false;
            }
            None => {
                let mut chars = rest.chars();
                opens = chars.next() == Some('\n');
                rest = chars.as_str();
            }
        }
    }

    found
}

fn holds_a_color(text: &str) -> bool {
    !colors_in(text).is_empty()
}

fn names_asked(code: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut left = code;

    while let Some((_, after)) = left.split_once('@') {
        let opens = after.chars().next().is_some_and(|character| character.is_ascii_alphabetic() || character == '_');

        left = match opens {
            true => {
                let (name, rest) = run_of(after, is_name);
                found.push(name.to_string());
                rest
            }
            false => after,
        };
    }

    found
}

fn property_held(line: &str) -> Option<String> {
    let after = line.trim_start().strip_prefix("--")?;
    let (run, rest) = run_of(after, is_name);

    match run.is_empty() {
        true => None,
        false => rest.trim_start().strip_prefix(':').map(|_| format!("--{run}")),
    }
}

fn properties_asked(code: &str) -> Vec<(String, char)> {
    let mut found: Vec<(String, char)> = Vec::new();
    let mut left = code;

    while let Some((_, after)) = left.split_once("var(") {
        left = match asked_at(after) {
            Some((name, closed, rest)) => {
                found.push((name, closed));
                rest
            }
            None => after,
        };
    }

    found
}

fn asked_at(after: &str) -> Option<(String, char, &str)> {
    let (run, tail) = run_of(after.trim_start().strip_prefix("--")?, is_name);

    match run.is_empty() {
        true => return None,
        false => {}
    }

    let mut chars = tail.trim_start().chars();
    let closed = chars.next()?;

    match closed {
        ',' | ')' => Some((format!("--{run}"), closed, chars.as_str())),
        _ => None,
    }
}

fn carrying(files: &Path) -> Vec<(PathBuf, String)> {
    fn walk(at: &Path, into: &mut Vec<PathBuf>) {
        let entries = match std::fs::read_dir(at) {
            Ok(entries) => entries,
            Err(_fault) => return,
        };
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
        .filter_map(|path| std::fs::read(&path).ok().map(|held| (path, held)))
        .filter_map(|(path, held)| String::from_utf8(held).ok().map(|text| (path, text)))
        .collect()
}

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

const AT_RULES: [&str; 16] = [
    "import", "define-color", "media", "keyframes", "supports", "namespace", "charset",
    "font-face", "layer", "property", "container", "page", "document", "scope", "starting-style",
    "else",
];

mod the_names {
    use super::*;

    #[test]
    fn every_name_the_desktop_asks_for_is_defined() {
        let files = root().join("files");
        let sheets: Vec<(PathBuf, String)> = [files.clone(), root().join("crates")]
            .iter()
            .flat_map(|tree| carrying(tree))
            .filter(|(path, _)| path.extension().is_some_and(|end| end == "css"))
            .collect();

        assert!(!sheets.is_empty(), "no stylesheets under {}", files.display());

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
                let code = line.split("/*").next().unwrap_or(line);

                for name in names_asked(code) {
                    if AT_RULES.contains(&name.as_str()) || defined.contains(&name) {
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

        assert!(missing.is_empty(), "a color no one defined is a rule GTK drops:\n  {}", missing.join("\n  "));
    }

    #[test]
    fn every_property_the_browser_asks_for_is_defined() {
        let files = root().join("files");
        let sheets: Vec<(PathBuf, String)> = [files.clone(), root().join("crates")]
            .iter()
            .flat_map(|tree| carrying(tree))
            .filter(|(path, _)| path.extension().is_some_and(|end| end == "css"))
            .collect();

        let defined: BTreeSet<String> = sheets
            .iter()
            .flat_map(|(_, said)| said.lines().filter_map(property_held))
            .collect();

        assert!(defined.contains("--text"), "the browser's palette defines nothing");

        let mut missing: Vec<String> = Vec::new();

        for (path, said) in &sheets {
            for line in said.lines() {
                let code = line.split("/*").next().unwrap_or(line);

                for (name, closed) in properties_asked(code) {
                    if closed == ',' || defined.contains(&name) {
                        continue;
                    }

                    missing.push(format!(
                        "{} asks for var({name}), which nothing defines",
                        path.strip_prefix(&files).unwrap_or(path).display(),
                    ));
                }
            }
        }

        missing.dedup();

        assert!(
            missing.is_empty(),
            "a property no one defined is a declaration the browser throws away:\n  {}",
            missing.join("\n  ")
        );
    }
}

mod the_engine {
    use super::*;

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
            let Ok(got) = color::hexcode(color::Oklch { lightness, chroma, hue });

            assert_eq!(got, expected, "at oklch({lightness} {chroma} {hue})");

            let Ok(reached) = color::contrast(HexColor(&got), Ground("2b212e"));

            assert!(
                (reached - ratio).abs() < 1e-4,
                "#{got} on #2b212e is {reached:.4}:1, recorded as {ratio}:1"
            );
        }
    }

    const APCA: [(&str, &str, f64); 3] = [
        ("000000", "ffffff", 106.04),
        ("ffffff", "000000", -107.88),
        ("888888", "ffffff", 63.06),
    ];

    #[test]
    fn the_apca_numbers_are_the_published_ones() {
        for (ink, ground, expected) in APCA {
            let Ok(got) = color::lightness_contrast(HexColor(ink), Ground(ground));

            assert!(
                (got - expected).abs() < 0.01,
                "#{ink} on #{ground} is Contrast {got:.3}, published as Contrast {expected}"
            );
        }
    }

    #[test]
    fn the_polarity_is_the_whole_point_and_is_not_symmetric() {
        let Ok(one) = color::contrast(HexColor("000000"), Ground("ffffff"));
        let Ok(other) = color::contrast(HexColor("ffffff"), Ground("000000"));
        let Ok(white_on_black) = color::lightness_contrast(HexColor("ffffff"), Ground("000000"));
        let Ok(black_on_white) = color::lightness_contrast(HexColor("000000"), Ground("ffffff"));

        assert_eq!(one, other);
        assert!(white_on_black.abs() != black_on_white.abs());
    }

    #[test]
    fn a_color_on_itself_is_no_contrast_in_either_measure() {
        let Ok(ratio) = color::contrast(HexColor("372c3a"), Ground("372c3a"));
        let Ok(lightness_contrast) = color::lightness_contrast(HexColor("372c3a"), Ground("372c3a"));

        assert!((ratio - 1.0).abs() < 1e-12);
        assert_eq!(lightness_contrast, 0.0);
    }

    #[test]
    fn wcag_flatters_a_dark_pair_and_apca_does_not() {
        let Ok(on_black) = color::contrast(HexColor("767676"), Ground("000000"));
        let Ok(on_white) = color::contrast(HexColor("767676"), Ground("ffffff"));

        assert!(on_black > on_white, "{on_black} should beat {on_white}");

        let Ok(black) = color::lightness_contrast(HexColor("767676"), Ground("000000"));
        let Ok(white) = color::lightness_contrast(HexColor("767676"), Ground("ffffff"));

        let (lightness_contrast_black, lightness_contrast_white) = (black.abs(), white.abs());
        assert!(lightness_contrast_black < lightness_contrast_white, "Contrast {lightness_contrast_black} should be under Contrast {lightness_contrast_white}");
    }
}

mod the_palette {
    use super::*;

    #[test]
    fn every_pairing_clears_what_it_declares() {
        let done = check();
        assert!(done.status.success(), "{}{}", done.stdout, done.stderr);
        assert!(done.stdout.contains("all clearing both measures"), "{}", done.stdout);
    }

    #[test]
    fn the_files_say_what_the_palette_says() {
        let done = check();
        assert!(
            done.status.success(),
            "a themed file no longer matches theme/palette.toml. Run `just theme`.\n{}{}",
            done.stdout,
            done.stderr
        );
    }

    #[test]
    fn every_color_says_what_it_is_for() {
        let declared = std::fs::read_to_string(root().join("theme/palette.toml")).expect("read");
        let configuration: toml::Table = declared.parse().expect("it parses");
        let colors = configuration["color"].as_table().expect("a table of colors");
        for (name, declared) in colors {
            let spent = declared.get("spent").and_then(toml::Value::as_str).unwrap_or("");
            assert!(!spent.is_empty(), "{name} does not say what it is spent on");
        }
    }
}

mod the_tree {
    use super::*;

    #[test]
    fn no_file_anywhere_carries_a_color_from_outside_the_palette() {
        let (root, spent) = (root(), spent());
        let lifted: Vec<String> = spent
            .iter()
            .map(|(_, code)| {
                let Ok(lifted) = color::lift(code, bright_lift());

                lifted
            })
            .collect();
        let known: BTreeSet<String> = forms(spent.iter().map(|(_, code)| code.as_str()))
            .into_iter()
            .chain(forms(lifted.iter().map(String::as_str)))
            .collect();

        for (path, text) in carrying(&root.join("files")) {
            for found in colors_in(&text) {
                let written = found.to_lowercase().replace(' ', "");
                assert!(
                    known.contains(&written),
                    "{} carries #{written}, which is not a color theme/palette.toml declares",
                    path.strip_prefix(&root).unwrap_or(&path).display()
                );
            }
        }
    }

    #[test]
    fn only_the_palette_holds_a_color() {
        let allowed: BTreeSet<&str> = BTreeSet::from([
            "home/@user@/.config/console/hypr/hyprland.lua",
            "home/@user@/.config/kdeglobals",
            "home/@user@/.config/console/palette.css",
            "home/@user@/.config/console/palette.toml",
            "home/@user@/.librewolf/console/chrome/palette.css",
            "home/@user@/.librewolf/console/user.js",
            "usr/local/lib/console/palette.sh",
            "usr/share/icons/console-placeholder.svg",
        ]);
        let files = root().join("files");
        let holding: BTreeSet<String> = carrying(&files)
            .into_iter()
            .filter(|(_, text)| holds_a_color(text))
            .map(|(path, _)| path.strip_prefix(&files).expect("under files/").display().to_string())
            .collect();
        let allowed: BTreeSet<String> = allowed.iter().map(|name| name.to_string()).collect();
        assert_eq!(
            holding, allowed,
            "a file outside the palette has grown a color, or one inside it has lost \
             the only color it had"
        );
    }

    #[test]
    fn every_color_is_spent() {
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

struct Output {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn check() -> Output {
    let done = std::process::Command::new(env!("CARGO_BIN_EXE_console-palette"))
        .arg("--check")
        .current_dir(root())
        .output()
        .expect("console-palette runs");
    Output {
        status: done.status,
        stdout: String::from_utf8_lossy(&done.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&done.stderr).into_owned(),
    }
}

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
        .filter(|(_, code)| code.len() == 6 && code.chars().all(|character| character.is_ascii_hexdigit()))
        .collect()
}

fn bright_lift() -> f64 {
    let declared = std::fs::read_to_string(root().join("theme/palette.toml")).expect("read");
    let configuration: toml::Table = declared.parse().expect("it parses");
    configuration["terminal"]["bright_lift"].as_float().expect("a number")
}

mod the_scanner {
    use super::*;

    #[test]
    fn six_hex_digits_are_a_color_and_seven_are_something_else() {
        assert_eq!(colors_in("#123456"), ["123456"]);
        assert_eq!(colors_in("#1234567"), [] as [String; 0]);
        assert_eq!(colors_in("#12345"), [] as [String; 0]);
        assert_eq!(colors_in("0xAABBCC."), ["AABBCC"]);
        assert_eq!(colors_in("0xAABBCCD"), [] as [String; 0]);
        assert_eq!(colors_in("#aabbcc\u{00e9}"), [] as [String; 0]);
    }

    #[test]
    fn a_color_with_an_alpha_is_only_the_opaque_one() {
        assert_eq!(colors_in("rgba(112233ff)"), ["112233"]);
        assert_eq!(colors_in("rgba(112233fe)"), [] as [String; 0]);
    }

    #[test]
    fn a_name_and_a_value_are_a_color_only_as_the_whole_line() {
        assert_eq!(colors_in("fg=aabbcc"), ["aabbcc"]);
        assert_eq!(colors_in(" fg=aabbcc"), [] as [String; 0]);
        assert_eq!(colors_in("fg=aabbccd"), [] as [String; 0]);
        assert_eq!(colors_in("a=b=aabbcc"), [] as [String; 0]);
        assert_eq!(colors_in("#aabbcc\nfg=1,2,3\n0xddeeff\n"), ["aabbcc", "1,2,3", "ddeeff"]);
    }

    #[test]
    fn three_numbers_are_a_color_and_a_fourth_digit_is_not() {
        assert_eq!(colors_in("fg=1,2,3"), ["1,2,3"]);
        assert_eq!(colors_in("fg=1, 2, 3"), ["1, 2, 3"]);
        assert_eq!(colors_in("fg=1,  2,3"), [] as [String; 0]);
        assert_eq!(colors_in("fg=1234,2,3"), [] as [String; 0]);
        assert_eq!(colors_in("fg=1,2"), [] as [String; 0]);
    }

    #[test]
    fn a_name_starts_with_a_letter_and_carries_on_with_more_than_letters() {
        assert_eq!(names_asked("@name @-bad @_x-9 a@b @@c"), ["name", "_x-9", "b", "c"]);
    }

    #[test]
    fn a_property_is_declared_only_where_a_declaration_can_start() {
        assert_eq!(property_held("  --x : red;"), Some("--x".to_string()));
        assert_eq!(property_held("--x:red"), Some("--x".to_string()));
        assert_eq!(property_held("x --y: red"), None);
        assert_eq!(property_held("-- : red"), None);
    }

    #[test]
    fn what_closed_a_var_is_the_difference_between_asking_and_preferring() {
        assert_eq!(
            properties_asked("var(--a) var(--b, x) var(--c)"),
            [
                ("--a".to_string(), ')'),
                ("--b".to_string(), ','),
                ("--c".to_string(), ')'),
            ]
        );
        assert_eq!(properties_asked("var( --a )"), [("--a".to_string(), ')')]);
        assert_eq!(properties_asked("var(--a;"), [] as [(String, char); 0]);
        assert_eq!(properties_asked("var(--)"), [] as [(String, char); 0]);
    }
}
