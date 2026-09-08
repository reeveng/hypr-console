//! The settings against the bar that opens them.
//!
//! The bar opens the panel at the tab that stands for the thing tapped. A name
//! nothing answers to opens the first tab, which is a wrong place rather than
//! an error, so it has to be caught here. So is the order they stand in: the
//! bar is one list of these things and the tabs are another, and two lists of
//! the same four things in two orders is a thumb that has to be told which one
//! it is looking at.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use console_settings::rows::tabs;

fn root() -> PathBuf {
    {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}
}

fn config() -> String {
    let at = root().join("files/home/@user@/.config/waybar/config.jsonc");
    let read = std::fs::read_to_string(&at).expect("the bar's own file");
    read.lines()
        .map(|line| line.split_once("//").map_or(line, |(said, _)| said))
        .collect::<Vec<&str>>()
        .join("\n")
}

fn asked(said: &str) -> Vec<&str> {
    said.split_whitespace()
        .skip_while(|word| word.contains('=') && !word.starts_with('-'))
        .collect()
}

fn bar() -> Vec<(String, String)> {
    config()
        .lines()
        .filter(|line| line.contains("\"on-"))
        .filter_map(|line| {
            let (_, rest) = line.split_once(':')?;
            let said = rest.trim().trim_matches(|letter| letter == ',' || letter == '"');
            let mut words = asked(said).into_iter();
            Some((words.next()?.to_string(), words.next().unwrap_or_default().to_string()))
        })
        .collect()
}

fn quoted(said: &str) -> Vec<String> {
    said.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

fn drawn_along_the_bar(read: &str) -> Vec<String> {
    let listed = read.split_once("\"modules-right\"").expect("the right-hand end of the bar").1;
    quoted(listed.split_once(']').expect("a list that ends").0)
}

fn opens(read: &str) -> BTreeMap<String, String> {
    let mut opens = BTreeMap::new();
    let mut icon = String::new();
    for line in read.lines() {
        let named = line.trim_end().strip_prefix("  \"");
        if let Some((name, _)) = named.and_then(|rest| rest.split_once("\": {")) {
            icon = name.to_string();
        }
        let said = match line.trim().strip_prefix("\"on-click\":") {
            Some(said) => said,
            None => continue,
        };
        let said = said.trim().trim_matches(|letter| letter == ',' || letter == '"');
        let mut words = asked(said).into_iter();
        let program = words.next().unwrap_or_default();
        let tab = words.next().unwrap_or_default();
        if program.ends_with("settings-panel") && !tab.is_empty() {
            opens.insert(icon.clone(), tab.to_string());
        }
    }
    opens
}

#[test]
fn every_tab_the_bar_asks_for_exists() {
    for (program, argument) in bar() {
        if !program.ends_with("settings-panel") || argument.is_empty() {
            continue;
        }
        let Ok(tabs) = tabs();

        assert!(
            tabs.contains(&argument),
            "the bar opens the {argument} tab, which does not exist"
        );
    }
}

#[test]
fn the_tabs_the_bar_opens_stand_in_the_order_the_bar_draws_them() {
    let read = config();
    let opens = opens(&read);
    let along_the_bar: Vec<String> = drawn_along_the_bar(&read)
        .into_iter()
        .filter_map(|icon| opens.get(&icon).cloned())
        .collect();
    let Ok(tabs) = tabs();
    let along_the_tabs: Vec<String> = tabs
        .into_iter()
        .filter(|tab| along_the_bar.contains(tab))
        .collect();
    assert_eq!(along_the_bar.len(), opens.len(), "an icon that opens a tab and is not drawn");
    assert_eq!(along_the_bar, along_the_tabs, "the bar and the tabs are in two orders");
}

#[test]
fn the_bar_opens_the_settings_at_a_tab() {
    let named = bar()
        .into_iter()
        .any(|(program, argument)| program.ends_with("settings-panel") && !argument.is_empty());
    assert!(named, "nothing on the bar opens the settings at a tab of its own");
}
