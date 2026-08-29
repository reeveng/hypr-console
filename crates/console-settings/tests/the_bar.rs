//! The settings against the bar that opens them.
//!
//! The bar opens the panel at the tab that stands for the thing tapped. A name
//! nothing answers to opens the first tab, which is a wrong place rather than
//! an error, so it has to be caught here.

use std::path::{Path, PathBuf};

use console_settings::rows::TABS;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

/// What every on-click in the bar runs, as the program and its argument.
fn bar() -> Vec<(String, String)> {
    let config = root().join("files/home/@user@/.config/waybar/config.jsonc");
    let read = std::fs::read_to_string(&config).expect("the bar's own file");
    read.lines()
        .map(|line| line.split_once("//").map_or(line, |(said, _)| said))
        .filter(|line| line.contains("\"on-"))
        .filter_map(|line| {
            let (_, rest) = line.split_once(':')?;
            let said = rest.trim().trim_matches(|letter| letter == ',' || letter == '"');
            let mut words = said.split_whitespace();
            Some((words.next()?.to_string(), words.next().unwrap_or_default().to_string()))
        })
        .collect()
}

#[test]
fn every_tab_the_bar_asks_for_exists() {
    for (program, argument) in bar() {
        if !program.ends_with("settings-panel") || argument.is_empty() {
            continue;
        }
        assert!(
            TABS.contains(&argument.as_str()),
            "the bar opens the {argument} tab, which does not exist"
        );
    }
}

/// If the bar stops naming a tab at all, this test would pass by having
/// nothing to say. It is the reason the panel is on the bar in the first place.
#[test]
fn the_bar_opens_the_settings_at_a_tab() {
    let named = bar()
        .into_iter()
        .any(|(program, argument)| program.ends_with("settings-panel") && !argument.is_empty());
    assert!(named, "nothing on the bar opens the settings at a tab of its own");
}
