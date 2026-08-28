//! Qt's colours.
//!
//! Qt applications ask for a role rather than a colour, and Fusion answers out
//! of this file, so a file manager and a print dialogue arrive already wearing
//! the palette without a widget theme being installed to do it.

use crate::palette::Palette;

/// A colour as the three decimal numbers a KDE config file wants.
fn rgb(code: &str) -> String {
    let code = code.trim_start_matches('#');
    [0, 2, 4]
        .map(|at| u8::from_str_radix(&code[at..at + 2], 16).unwrap_or(0).to_string())
        .join(",")
}

/// The foregrounds every section carries, which do not change between them.
const INK: [(&str, &str); 10] = [
    ("DecorationFocus", "pink"), ("DecorationHover", "pink"),
    ("ForegroundActive", "peach"), ("ForegroundInactive", "soft"),
    ("ForegroundLink", "sky"), ("ForegroundNegative", "coral"),
    ("ForegroundNeutral", "butter"), ("ForegroundNormal", "text"),
    ("ForegroundPositive", "leaf"), ("ForegroundVisited", "mauve"),
];

/// Each section, and the two grounds it is drawn on.
const SECTIONS: [(&str, &str, &str); 6] = [
    ("Colors:Button", "panel", "ground"),
    ("Colors:Complementary", "ground", "night"),
    ("Colors:Header", "ground", "night"),
    ("Colors:Tooltip", "panel", "ground"),
    ("Colors:View", "night", "ground"),
    ("Colors:Window", "ground", "night"),
];

/// The eight names a foreground can go by, all of which are the dark ink on a
/// selection. The fill is the thing carrying contrast there, and a second hue
/// on top of it would be the one unreadable place in the palette.
const ON_A_SELECTION: [&str; 8] = [
    "ForegroundActive", "ForegroundInactive", "ForegroundLink",
    "ForegroundNegative", "ForegroundNeutral", "ForegroundNormal",
    "ForegroundPositive", "ForegroundVisited",
];

pub fn spend(palette: &Palette) -> String {
    let at = |role: &str| rgb(&palette[role]);

    let sections = SECTIONS.into_iter().flat_map(|(name, normal, alternate)| {
        [
            format!("[{name}]"),
            format!("BackgroundAlternate={}", at(alternate)),
            format!("BackgroundNormal={}", at(normal)),
        ]
        .into_iter()
        .chain(INK.map(|(role, colour)| format!("{role}={}", at(colour))))
        .chain([String::new()])
    });

    let selection = [
        "[Colors:Selection]".to_string(),
        format!("BackgroundAlternate={}", at("pink")),
        format!("BackgroundNormal={}", at("pink")),
        format!("DecorationFocus={}", at("pink")),
        format!("DecorationHover={}", at("pink")),
    ]
    .into_iter()
    .chain(ON_A_SELECTION.map(|role| format!("{role}={}", at("night"))))
    .chain([String::new()]);

    let window_manager = [
        "[WM]".to_string(),
        format!("activeBackground={}", at("panel")),
        format!("activeBlend={}", at("pink")),
        format!("activeForeground={}", at("text")),
        format!("inactiveBackground={}", at("ground")),
        format!("inactiveBlend={}", at("soft")),
        format!("inactiveForeground={}", at("soft")),
    ];

    sections
        .chain(selection)
        .chain(window_manager)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn a_colour_is_three_decimal_numbers() {
        assert_eq!(rgb("000000"), "0,0,0");
        assert_eq!(rgb("ffffff"), "255,255,255");
        assert_eq!(rgb("#ff8040"), "255,128,64");
    }

    #[test]
    fn every_section_qt_asks_for_is_written() {
        let ini = spend(&blossom());
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
        let selection = ini.split_once("[Colors:Selection]").expect("the section").1;
        let selection = selection.split_once("[WM]").map_or(selection, |(head, _)| head);
        for role in ON_A_SELECTION {
            assert!(
                selection.contains(&format!("{role}={}", rgb(&palette["night"]))),
                "{role} on a selection is not the dark ink"
            );
        }
    }

    #[test]
    fn it_parses_as_the_ini_qt_would_read() {
        for line in spend(&blossom()).lines().filter(|l| !l.is_empty()) {
            let shaped = line.starts_with('[') && line.ends_with(']') || line.contains('=');
            assert!(shaped, "{line:?} is neither a section nor a setting");
        }
    }
}
