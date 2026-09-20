//! The readings along the bar against the settings they open.
//!
//! A tap on the sound, the bluetooth, the network or the battery opens the
//! panel at the tab that stands for the thing tapped. A name nothing answers
//! to opens the first tab, which is a wrong place rather than an error, so it
//! has to be caught here. So is the order they stand in: the bar is one list
//! of these things and the tabs are another, and two lists of the same four
//! things in two orders is a thumb that has to be told which one it is looking
//! at.

use console_settings::rows::tabs;
use console_status_bar::holding::ALONG;

fn asked() -> Vec<String> {
    ALONG
        .into_iter()
        .map(|what| {
            let Ok(tab) = what.tab();

            tab.to_string()
        })
        .collect()
}

#[test]
fn every_tab_the_bar_asks_for_exists() {
    let Ok(tabs) = tabs();

    for tab in asked() {
        assert!(tabs.contains(&tab), "the bar opens the {tab} tab, which does not exist");
    }
}

#[test]
fn the_tabs_the_bar_opens_stand_in_the_order_the_bar_draws_them() {
    let along_the_bar = asked();
    let Ok(tabs) = tabs();
    let along_the_tabs: Vec<String> =
        tabs.into_iter().filter(|tab| along_the_bar.contains(tab)).collect();

    assert_eq!(along_the_bar, along_the_tabs, "the bar and the tabs are in two orders");
}
