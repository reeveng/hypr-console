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

use std::error::Error;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use console_core_color::{Ground, HexColor};
use console_core_color as color;
use console_core_directory_listing::Descend;
use console_core_iteration::{iterate, Step};
use console_core_never::Never;

fn root() -> Result<PathBuf, std::io::Error> {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()
}

const IS_WORD: fn(char) -> bool = |character| character.is_alphanumeric() || character == '_';

const IS_NAME: fn(char) -> bool = |character| character.is_ascii_alphanumeric() || character == '_' || character == '-';

fn run_of(said: &str, taken: impl Fn(char) -> bool) -> Result<(&str, &str), Never> {
    Ok(match said.find(|character: char| !taken(character)) {
        Some(end) => said.split_at(end),
        None => (said, ""),
    })
}

fn hex_six(said: &str) -> Result<Option<(&str, &str)>, Never> {
    let (code, after) = match said.split_at_checked(6) {
        Some(split) => split,
        None => return Ok(None),
    };

    Ok(match code.chars().all(|character| character.is_ascii_hexdigit()) {
        true => Some((code, after)),
        false => None,
    })
}

fn hex_word(said: &str) -> Result<Option<(&str, &str)>, Never> {
    let Ok(six) = hex_six(said);

    let (code, after) = match six {
        Some(six) => six,
        None => return Ok(None),
    };

    Ok(match after.chars().next().is_some_and(IS_WORD) {
        true => None,
        false => Some((code, after)),
    })
}

fn decimal_triple(said: &str) -> Result<bool, Never> {
    let mut left = said;

    for band in 0..3u8 {
        let Ok((digits, after)) = run_of(left, |character| character.is_ascii_digit());

        match (1..=3).contains(&digits.len()) {
            true => {}
            false => return Ok(false),
        }

        left = match band {
            2 => after,
            _ => {
                let past = match after.strip_prefix(',') {
                    Some(past) => past,
                    None => return Ok(false),
                };

                match past.strip_prefix(char::is_whitespace) {
                    Some(spaced) => spaced,
                    None => past,
                }
            }
        };
    }

    Ok(left.is_empty())
}

fn color_at<'a>(rest: &'a str, line: Option<&'a str>) -> Result<Option<(String, &'a str)>, Never> {
    for prefix in ["#", "0x"] {
        let Ok(word) = match rest.strip_prefix(prefix) {
            Some(after) => hex_word(after),
            None => Ok(None),
        };

        match word {
            Some((code, after)) => return Ok(Some((code.to_string(), after))),
            None => {}
        }
    }

    let Ok(six) = match rest.strip_prefix("rgba(") {
        Some(after) => hex_six(after),
        None => Ok(None),
    };

    match six.and_then(|(code, after)| after.strip_prefix("ff)").map(|after| (code, after))) {
        Some((code, after)) => return Ok(Some((code.to_string(), after))),
        None => {}
    }

    let line = match line {
        Some(line) => line,
        None => return Ok(None),
    };
    let Ok((named, after)) = run_of(line, IS_WORD);

    let value = match (named.is_empty(), after.strip_prefix('=')) {
        (false, Some(value)) => value,
        (true, Some(_)) | (true, None) | (false, None) => return Ok(None),
    };

    let past_line = match rest.strip_prefix(line) {
        Some(past_line) => past_line,
        None => return Ok(None),
    };
    let Ok(six) = hex_six(value);

    match six {
        Some((code, "")) => return Ok(Some((code.to_string(), past_line))),
        Some(_) | None => {}
    }

    let Ok(decimal) = decimal_triple(value);

    Ok(match decimal {
        true => Some((value.to_string(), past_line)),
        false => None,
    })
}

fn colors_in(text: &str) -> Result<Vec<String>, Never> {
    let found = iterate((Vec::new(), text, Opens::Line), |(mut found, rest, opens)| {
        match rest.is_empty() {
            true => return Ok(Step::Halt(found)),
            false => {}
        }

        let line = match opens {
            Opens::Line => rest.split('\n').next(),
            Opens::Within => None,
        };
        let Ok(color) = color_at(rest, line);

        let (rest, opens) = match color {
            Some((code, after)) => {
                found.push(code);

                (after, Opens::Within)
            }
            None => {
                let mut chars = rest.chars();

                let opens = match chars.next() == Some('\n') {
                    true => Opens::Line,
                    false => Opens::Within,
                };

                (chars.as_str(), opens)
            }
        };

        Ok(Step::Again((found, rest, opens)))
    });

    Ok(match found {
        Ok(found) => found,
        Err(_endless) => Vec::new(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opens {
    Line,
    Within,
}

fn holds_a_color(text: &str) -> Result<bool, Never> {
    let Ok(colors) = colors_in(text);

    Ok(!colors.is_empty())
}

fn names_asked(code: &str) -> Result<Vec<String>, Never> {
    let found = iterate((Vec::new(), code), |(mut found, left)| {
        let after = match left.split_once('@') {
            Some((_, after)) => after,
            None => return Ok(Step::Halt(found)),
        };

        let opens = after.chars().next().is_some_and(|character| character.is_ascii_alphabetic() || character == '_');

        let left = match opens {
            true => {
                let Ok((name, rest)) = run_of(after, IS_NAME);

                found.push(name.to_string());

                rest
            }
            false => after,
        };

        Ok(Step::Again((found, left)))
    });

    Ok(match found {
        Ok(found) => found,
        Err(_endless) => Vec::new(),
    })
}

fn property_held(line: &str) -> Result<Option<String>, Never> {
    let after = match line.trim_start().strip_prefix("--") {
        Some(after) => after,
        None => return Ok(None),
    };
    let Ok((run, rest)) = run_of(after, IS_NAME);

    Ok(match run.is_empty() {
        true => None,
        false => rest.trim_start().strip_prefix(':').map(|_| format!("--{run}")),
    })
}

fn properties_asked(code: &str) -> Result<Vec<(String, char)>, Never> {
    let found = iterate((Vec::new(), code), |(mut found, left)| {
        let after = match left.split_once("var(") {
            Some((_, after)) => after,
            None => return Ok(Step::Halt(found)),
        };

        let Ok(asked) = asked_at(after);

        let left = match asked {
            Some((name, closed, rest)) => {
                found.push((name, closed));

                rest
            }
            None => after,
        };

        Ok(Step::Again((found, left)))
    });

    Ok(match found {
        Ok(found) => found,
        Err(_endless) => Vec::new(),
    })
}

fn asked_at(after: &str) -> Result<Option<(String, char, &str)>, Never> {
    let named = match after.trim_start().strip_prefix("--") {
        Some(named) => named,
        None => return Ok(None),
    };
    let Ok((run, tail)) = run_of(named, IS_NAME);

    match run.is_empty() {
        true => return Ok(None),
        false => {}
    }

    let mut chars = tail.trim_start().chars();

    let closed = match chars.next() {
        Some(closed) => closed,
        None => return Ok(None),
    };

    Ok(match closed {
        ',' | ')' => Some((format!("--{run}"), closed, chars.as_str())),
        _ => None,
    })
}

fn every_file(files: &Path) -> Result<Vec<(PathBuf, String)>, Never> {
    let Ok(listing) = console_core_directory_listing::recursive(files, |at| match at.ends_with("__pycache__") {
        true => Descend::Past,
        false => Descend::Into,
    });

    let mut paths: Vec<PathBuf> = listing
        .filter_map(|entry| match entry {
            Ok(path) => match (path.ends_with("__pycache__"), path.is_dir()) {
                (true, true) | (true, false) | (false, true) => None,
                (false, false) => Some(path),
            },
            Err(_unlisted) => None,
        })
        .collect();

    paths.sort();

    let mut carried = Vec::new();

    for path in paths {
        match std::fs::read(&path).map(String::from_utf8) {
            Ok(Ok(text)) => carried.push((path, text)),
            Ok(Err(_not_text)) => {}
            Err(_unread) => {}
        }
    }

    Ok(carried)
}

fn forms<'a>(codes: impl Iterator<Item = &'a str>) -> Result<BTreeSet<String>, Never> {
    let mut found = BTreeSet::new();

    for code in codes {
        let decimal: Vec<String> = [code.get(0..2), code.get(2..4), code.get(4..6)]
            .into_iter()
            .map(|pair| match pair.map(|pair| u8::from_str_radix(pair, 16)) {
                Some(Ok(band)) => band.to_string(),
                Some(Err(_not_hex)) => "0".to_string(),
                None => "0".to_string(),
            })
            .collect();

        found.insert(code.to_lowercase());
        found.insert(decimal.join(","));
    }

    Ok(found)
}

const AT_RULES: [&str; 16] = [
    "import", "define-color", "media", "keyframes", "supports", "namespace", "charset",
    "font-face", "layer", "property", "container", "page", "document", "scope", "starting-style",
    "else",
];

fn sheets(root: &Path) -> Result<Vec<(PathBuf, String)>, Never> {
    let mut sheets = Vec::new();

    for tree in [root.join("files"), root.join("crates")] {
        let Ok(carried) = every_file(&tree);

        sheets.extend(carried.into_iter().filter(|(path, _)| path.extension().is_some_and(|end| end == "css")));
    }

    Ok(sheets)
}

fn uncommented(line: &str) -> Result<&str, Never> {
    Ok(match line.split_once("/*") {
        Some((code, _comment)) => code,
        None => line,
    })
}

fn shown<'a>(path: &'a Path, under: &Path) -> Result<std::path::Display<'a>, Never> {
    Ok(match path.strip_prefix(under) {
        Ok(inside) => inside.display(),
        Err(_outside) => path.display(),
    })
}

mod the_names {
    use super::*;

    #[test]
    fn every_name_the_desktop_asks_for_is_defined() -> Result<(), Box<dyn Error>> {
        let root = root()?;
        let files = root.join("files");
        let Ok(sheets) = sheets(&root);
        let rules: BTreeSet<&str> = AT_RULES.into_iter().collect();

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
                let Ok(code) = uncommented(line);
                let Ok(names) = names_asked(code);

                for name in names {
                    match rules.contains(name.as_str()) || defined.contains(&name) {
                        true => {}
                        false => {
                            let Ok(shown) = shown(path, &files);

                            missing.push(format!("{shown} asks for @{name}, which nothing defines"));
                        }
                    }
                }
            }
        }

        missing.dedup();

        assert!(missing.is_empty(), "a color no one defined is a rule GTK drops:\n  {}", missing.join("\n  "));

        Ok(())
    }

    #[test]
    fn every_property_the_browser_asks_for_is_defined() -> Result<(), Box<dyn Error>> {
        let root = root()?;
        let files = root.join("files");
        let Ok(sheets) = sheets(&root);

        let defined: BTreeSet<String> = sheets
            .iter()
            .flat_map(|(_, said)| {
                said.lines().filter_map(|line| {
                    let Ok(held) = property_held(line);

                    held
                })
            })
            .collect();

        assert!(defined.contains("--text"), "the browser's palette defines nothing");

        let mut missing: Vec<String> = Vec::new();

        for (path, said) in &sheets {
            for line in said.lines() {
                let Ok(code) = uncommented(line);
                let Ok(asked) = properties_asked(code);

                for (name, closed) in asked {
                    match closed == ',' || defined.contains(&name) {
                        true => {}
                        false => {
                            let Ok(shown) = shown(path, &files);

                            missing.push(format!("{shown} asks for var({name}), which nothing defines"));
                        }
                    }
                }
            }
        }

        missing.dedup();

        assert!(
            missing.is_empty(),
            "a property no one defined is a declaration the browser throws away:\n  {}",
            missing.join("\n  ")
        );

        Ok(())
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
    fn every_pairing_clears_what_it_declares() -> Result<(), Box<dyn Error>> {
        let done = check()?;

        assert!(done.status.success(), "{}{}", done.stdout, done.stderr);
        assert!(done.stdout.contains("all clearing both measures"), "{}", done.stdout);

        Ok(())
    }

    #[test]
    fn the_files_say_what_the_palette_says() -> Result<(), Box<dyn Error>> {
        let done = check()?;

        assert!(
            done.status.success(),
            "a themed file no longer matches theme/palette.toml. Run `just theme`.\n{}{}",
            done.stdout,
            done.stderr
        );

        Ok(())
    }

    #[test]
    fn every_color_says_what_it_is_for() -> Result<(), Box<dyn Error>> {
        let root = root()?;
        let declared = std::fs::read_to_string(root.join("theme/palette.toml"))?;
        let configuration: toml::Table = declared.parse()?;
        let colors = configuration.get("color").and_then(toml::Value::as_table).ok_or("no colors")?;

        for (name, declared) in colors {
            let spent = declared.get("spent").and_then(toml::Value::as_str);

            assert!(spent.is_some_and(|spent| !spent.is_empty()), "{name} does not say what it is spent on");
        }

        Ok(())
    }
}

mod the_tree {
    use super::*;

    #[test]
    fn no_file_anywhere_carries_a_color_from_outside_the_palette() -> Result<(), Box<dyn Error>> {
        let root = root()?;
        let spent = spent()?;
        let lift = bright_lift()?;
        let mut lifted: Vec<String> = Vec::new();

        for (_, code) in &spent {
            let Ok(code) = color::lift(code, lift);

            lifted.push(code);
        }

        let Ok(declared) = forms(spent.iter().map(|(_, code)| code.as_str()));
        let Ok(brightened) = forms(lifted.iter().map(String::as_str));
        let known: BTreeSet<String> = declared.into_iter().chain(brightened).collect();
        let Ok(carried) = every_file(&root.join("files"));

        for (path, text) in carried {
            let Ok(colors) = colors_in(&text);

            for found in colors {
                let written = found.to_lowercase().replace(' ', "");
                let Ok(shown) = shown(&path, &root);

                assert!(
                    known.contains(&written),
                    "{shown} carries #{written}, which is not a color theme/palette.toml declares"
                );
            }
        }

        Ok(())
    }

    #[test]
    fn only_the_palette_holds_a_color() -> Result<(), Box<dyn Error>> {
        let allowed: BTreeSet<String> = [
            "home/@user@/.config/console/hypr/hyprland.lua",
            "home/@user@/.config/kdeglobals",
            "home/@user@/.config/console/palette.css",
            "home/@user@/.config/console/palette.toml",
            "home/@user@/.librewolf/console/chrome/palette.css",
            "home/@user@/.librewolf/console/user.js",
            "usr/local/lib/console/palette.sh",
            "usr/share/icons/console-placeholder.svg",
        ]
        .iter()
        .map(|name| name.to_string())
        .collect();
        let root = root()?;
        let files = root.join("files");
        let Ok(carried) = every_file(&files);
        let mut holding: BTreeSet<String> = BTreeSet::new();

        for (path, text) in carried {
            let Ok(holds) = holds_a_color(&text);
            let Ok(shown) = shown(&path, &files);

            match holds {
                true => holding.insert(shown.to_string()),
                false => continue,
            };
        }

        assert_eq!(
            holding, allowed,
            "a file outside the palette has grown a color, or one inside it has lost \
             the only color it had"
        );

        Ok(())
    }

    #[test]
    fn every_color_is_spent() -> Result<(), Box<dyn Error>> {
        let root = root()?;
        let Ok(carried) = every_file(&root.join("files"));
        let written: String = carried.into_iter().map(|(_, text)| text.to_lowercase()).collect();

        let spent = spent()?;

        for (name, code) in spent {
            assert!(written.contains(&code.to_lowercase()), "{name} (#{code}) is declared and never used");
        }

        Ok(())
    }
}

struct Output {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn check() -> Result<Output, Box<dyn Error>> {
    let root = root()?;
    let done = std::process::Command::new(env!("CARGO_BIN_EXE_console-palette"))
        .arg("--check")
        .current_dir(root)
        .output()?;

    Ok(Output {
        status: done.status,
        stdout: String::from_utf8_lossy(&done.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&done.stderr).into_owned(),
    })
}

fn spent() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let root = root()?;
    let report = std::fs::read_to_string(root.join("theme/report.md"))?;
    let mut spent = Vec::new();

    for line in report.lines() {
        let mut cells = line.split('|').map(str::trim);

        let (name, code) = match (cells.next(), cells.next(), cells.next()) {
            (Some(""), Some(name), Some(code)) => (name, code),
            (Some(_), Some(_), Some(_)) | (Some(_), Some(_), None) | (Some(_), None, _) | (None, _, _) => continue,
        };

        match (name.starts_with('`'), code.starts_with("`#")) {
            (true, true) => {}
            (true, false) | (false, true) | (false, false) => continue,
        }

        let code = code.trim_matches('`').trim_start_matches('#');

        match code.len() == 6 && code.chars().all(|character| character.is_ascii_hexdigit()) {
            true => spent.push((name.trim_matches('`').to_string(), code.to_string())),
            false => continue,
        }
    }

    Ok(spent)
}

fn bright_lift() -> Result<f64, Box<dyn Error>> {
    let root = root()?;
    let declared = std::fs::read_to_string(root.join("theme/palette.toml"))?;
    let configuration: toml::Table = declared.parse()?;

    let lift = configuration
        .get("terminal")
        .and_then(|terminal| terminal.get("bright_lift"))
        .and_then(toml::Value::as_float)
        .ok_or("a number")?;

    Ok(lift)
}

mod the_scanner {
    use super::*;

    fn strings(said: &[&str]) -> Result<Vec<String>, Never> {
        Ok(said.iter().map(|said| said.to_string()).collect())
    }

    #[test]
    fn six_hex_digits_are_a_color_and_seven_are_something_else() {
        assert_eq!(colors_in("#123456"), strings(&["123456"]));
        assert_eq!(colors_in("#1234567"), strings(&[]));
        assert_eq!(colors_in("#12345"), strings(&[]));
        assert_eq!(colors_in("0xAABBCC."), strings(&["AABBCC"]));
        assert_eq!(colors_in("0xAABBCCD"), strings(&[]));
        assert_eq!(colors_in("#aabbcc\u{00e9}"), strings(&[]));
    }

    #[test]
    fn a_color_with_an_alpha_is_only_the_opaque_one() {
        assert_eq!(colors_in("rgba(112233ff)"), strings(&["112233"]));
        assert_eq!(colors_in("rgba(112233fe)"), strings(&[]));
    }

    #[test]
    fn a_name_and_a_value_are_a_color_only_as_the_whole_line() {
        assert_eq!(colors_in("fg=aabbcc"), strings(&["aabbcc"]));
        assert_eq!(colors_in(" fg=aabbcc"), strings(&[]));
        assert_eq!(colors_in("fg=aabbccd"), strings(&[]));
        assert_eq!(colors_in("a=b=aabbcc"), strings(&[]));
        assert_eq!(colors_in("#aabbcc\nfg=1,2,3\n0xddeeff\n"), strings(&["aabbcc", "1,2,3", "ddeeff"]));
    }

    #[test]
    fn three_numbers_are_a_color_and_a_fourth_digit_is_not() {
        assert_eq!(colors_in("fg=1,2,3"), strings(&["1,2,3"]));
        assert_eq!(colors_in("fg=1, 2, 3"), strings(&["1, 2, 3"]));
        assert_eq!(colors_in("fg=1,  2,3"), strings(&[]));
        assert_eq!(colors_in("fg=1234,2,3"), strings(&[]));
        assert_eq!(colors_in("fg=1,2"), strings(&[]));
    }

    #[test]
    fn a_name_starts_with_a_letter_and_carries_on_with_more_than_letters() {
        assert_eq!(names_asked("@name @-bad @_x-9 a@b @@c"), strings(&["name", "_x-9", "b", "c"]));
    }

    #[test]
    fn a_property_is_declared_only_where_a_declaration_can_start() {
        assert_eq!(property_held("  --x : red;"), Ok(Some("--x".to_string())));
        assert_eq!(property_held("--x:red"), Ok(Some("--x".to_string())));
        assert_eq!(property_held("x --y: red"), Ok(None));
        assert_eq!(property_held("-- : red"), Ok(None));
    }

    #[test]
    fn what_closed_a_var_is_the_difference_between_asking_and_preferring() {
        assert_eq!(
            properties_asked("var(--a) var(--b, x) var(--c)"),
            Ok(vec![("--a".to_string(), ')'), ("--b".to_string(), ','), ("--c".to_string(), ')')])
        );
        assert_eq!(properties_asked("var( --a )"), Ok(vec![("--a".to_string(), ')')]));
        assert_eq!(properties_asked("var(--a;"), Ok(Vec::new()));
        assert_eq!(properties_asked("var(--)"), Ok(Vec::new()));
    }
}
