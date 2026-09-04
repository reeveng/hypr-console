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

/// The bar's own file, with what is said about it taken out.
fn config() -> String {
    let at = root().join("files/home/@user@/.config/waybar/config.jsonc");
    let read = std::fs::read_to_string(&at).expect("the bar's own file");
    read.lines()
        .map(|line| line.split_once("//").map_or(line, |(said, _)| said))
        .collect::<Vec<&str>>()
        .join("\n")
}

/// What every on-click in the bar runs, as the program and its argument.
fn bar() -> Vec<(String, String)> {
    config()
        .lines()
        .filter(|line| line.contains("\"on-"))
        .filter_map(|line| {
            let (_, rest) = line.split_once(':')?;
            let said = rest.trim().trim_matches(|letter| letter == ',' || letter == '"');
            let mut words = said.split_whitespace();
            Some((words.next()?.to_string(), words.next().unwrap_or_default().to_string()))
        })
        .collect()
}

/// Every quoted word in a stretch of the file, in the order they are written.
fn quoted(said: &str) -> Vec<String> {
    said.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

/// What the bar draws along its right-hand end, in the order it draws them.
///
/// The list it is given rather than the order the modules happen to be written
/// down in below it, because the list is what waybar reads.
fn drawn_along_the_bar(read: &str) -> Vec<String> {
    let listed = read.split_once("\"modules-right\"").expect("the right-hand end of the bar").1;
    quoted(listed.split_once(']').expect("a list that ends").0)
}

/// Which tab each of the bar's icons opens the settings at.
///
/// Only the ones that open a tab. Two of the icons along that edge open a panel
/// of their own and the rest open nothing at all.
fn opens(read: &str) -> BTreeMap<String, String> {
    let mut opens = BTreeMap::new();
    let mut icon = String::new();
    for line in read.lines() {
        let named = line.trim_end().strip_prefix("  \"");
        if let Some((name, _)) = named.and_then(|rest| rest.split_once("\": {")) {
            icon = name.to_string();
        }
        let Some(said) = line.trim().strip_prefix("\"on-click\":") else { continue };
        let said = said.trim().trim_matches(|letter| letter == ',' || letter == '"');
        let mut words = said.split_whitespace();
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
        assert!(
            tabs().contains(&argument),
            "the bar opens the {argument} tab, which does not exist"
        );
    }
}

/// The bar stays on the screen while the panel is being read, so the four
/// things that are both an icon up there and a tab down here are read one after
/// the other. In two orders that is a thumb going back the way it came: the
/// speaker was the second icon and the second tab from the other end, and the
/// battery was at one end of the bar and the other end of the tabs.
///
/// The tabs nothing on the bar opens are not in it. They are free to sit
/// wherever they read best, which is after the four.
#[test]
fn the_tabs_the_bar_opens_stand_in_the_order_the_bar_draws_them() {
    let read = config();
    let opens = opens(&read);
    let along_the_bar: Vec<String> = drawn_along_the_bar(&read)
        .into_iter()
        .filter_map(|icon| opens.get(&icon).cloned())
        .collect();
    let along_the_tabs: Vec<String> = tabs()
        .into_iter()
        .filter(|tab| along_the_bar.contains(tab))
        .collect();
    assert_eq!(along_the_bar.len(), opens.len(), "an icon that opens a tab and is not drawn");
    assert_eq!(along_the_bar, along_the_tabs, "the bar and the tabs are in two orders");
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
