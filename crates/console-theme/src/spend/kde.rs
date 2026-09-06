//! Qt's colours.
//!
//! Qt applications ask for a role rather than a colour, and Fusion answers out
//! of this file, so a file manager and a print dialogue arrive already wearing
//! the palette without a widget theme being installed to do it.

use console_colour::Short;
use crate::palette::Palette;

fn rgb(code: &str) -> Result<String, Short> {
    let code = code.trim_start_matches('#');
    let mut said: Vec<String> = Vec::new();

    for at in [0usize, 2, 4] {
        let pair = code
            .get(at..at.saturating_add(2))
            .ok_or_else(|| Short(format!("{code} is not the six hex digits a colour is")))?;
        let number = u8::from_str_radix(pair, 16)
            .map_err(|why| Short(format!("{pair}, in {code}, is not a hex number: {why}")))?;
        said.push(number.to_string());
    }

    Ok(said.join(","))
}

const INK: [(&str, &str); 10] = [
    ("DecorationFocus", "pink"), ("DecorationHover", "pink"),
    ("ForegroundActive", "peach"), ("ForegroundInactive", "soft"),
    ("ForegroundLink", "sky"), ("ForegroundNegative", "coral"),
    ("ForegroundNeutral", "butter"), ("ForegroundNormal", "text"),
    ("ForegroundPositive", "leaf"), ("ForegroundVisited", "mauve"),
];

const SECTIONS: [(&str, &str, &str); 6] = [
    ("Colors:Button", "panel", "ground"),
    ("Colors:Complementary", "ground", "night"),
    ("Colors:Header", "ground", "night"),
    ("Colors:Tooltip", "panel", "ground"),
    ("Colors:View", "night", "ground"),
    ("Colors:Window", "ground", "night"),
];

const ON_A_SELECTION: [&str; 8] = [
    "ForegroundActive", "ForegroundInactive", "ForegroundLink",
    "ForegroundNegative", "ForegroundNeutral", "ForegroundNormal",
    "ForegroundPositive", "ForegroundVisited",
];

pub fn spend(palette: &Palette) -> Result<String, Short> {
    let at = |role: &str| palette.must(role).and_then(rgb);

    let mut lines: Vec<String> = Vec::new();

    for (name, normal, alternate) in SECTIONS {
        lines.push(format!("[{name}]"));
        let second = at(alternate)?;
        let first = at(normal)?;

        lines.push(format!("BackgroundAlternate={second}"));
        lines.push(format!("BackgroundNormal={first}"));

        for (role, colour) in INK {
            let ink = at(colour)?;

            lines.push(format!("{role}={ink}"));
        }

        lines.push(String::new());
    }

    lines.push("[Colors:Selection]".to_string());

    let pink = at("pink")?;
    let night = at("night")?;

    for name in ["BackgroundAlternate", "BackgroundNormal", "DecorationFocus", "DecorationHover"] {
        lines.push(format!("{name}={pink}"));
    }

    for role in ON_A_SELECTION {
        lines.push(format!("{role}={night}"));
    }

    lines.push(String::new());

    lines.push("[WM]".to_string());

    for (name, role) in [
        ("activeBackground", "panel"),
        ("activeBlend", "pink"),
        ("activeForeground", "text"),
        ("inactiveBackground", "ground"),
        ("inactiveBlend", "soft"),
        ("inactiveForeground", "soft"),
    ] {
        let colour = at(role)?;

        lines.push(format!("{name}={colour}"));
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn a_colour_is_three_decimal_numbers() {
        assert_eq!(rgb("000000"), Ok("0,0,0".to_string()));
        assert_eq!(rgb("ffffff"), Ok("255,255,255".to_string()));
        assert_eq!(rgb("#ff8040"), Ok("255,128,64".to_string()));
        assert!(rgb("nope").is_err(), "a code that is not a colour is said rather than drawn");
    }

    #[test]
    fn every_section_qt_asks_for_is_written() {
        let ini = spend(&blossom()).expect("every colour it spends is declared");
        for (name, _, _) in SECTIONS {
            assert!(ini.contains(&format!("[{name}]")), "{name} is missing");
        }
        assert!(ini.contains("[Colors:Selection]"));
        assert!(ini.contains("[WM]"));
    }

    #[test]
    fn every_foreground_on_a_selection_is_the_dark_ink() {
        let palette = blossom();
        let ini = spend(&palette);
        let ini = ini.expect("every colour it spends is declared");
        let selection = ini.split_once("[Colors:Selection]").expect("the section").1;
        let selection = selection.split_once("[WM]").map_or(selection, |(head, _)| head);
        for role in ON_A_SELECTION {
            assert!(
                selection.contains(&format!(
                    "{role}={}",
                    rgb(palette.must("night").expect("a declared colour")).expect("a colour parses")
                )),
                "{role} on a selection is not the dark ink"
            );
        }
    }

    #[test]
    fn it_parses_as_the_ini_qt_would_read() {
        for line in spend(&blossom()).expect("every colour it spends is declared").lines().filter(|l| !l.is_empty()) {
            let shaped = line.starts_with('[') && line.ends_with(']') || line.contains('=');
            assert!(shaped, "{line:?} is neither a section nor a setting");
        }
    }
}
