//! Telling the browsers which engine was chosen.
//!
//! The menu asks whatever the Web tab says. A browser asks whatever it was
//! last told in its own settings, and the two disagreeing is the same question
//! answered twice on one machine. So choosing an engine writes it into every
//! browser as well, through the one door each of them leaves open to it.
//!
//! That door is a policy file, and all three of them are under /etc, which is
//! why this is run by console-engine and not by the panel that calls it.
//!
//! LibreWolf is the awkward one. Chromium merges every file in its policy
//! directory, so ours sits alongside whatever else is there; Firefox ships no
//! policy file at all, so ours is the only one. LibreWolf ships its own, full
//! of the hardening that is the reason for using it, and a file under /etc
//! replaces that one outright rather than joining it. So ours is built from
//! theirs every time, read fresh at the moment of writing: an update to
//! LibreWolf is carried in the next time an engine is chosen.

use serde_json::{Map, Value, json};

use crate::engines::{Engine, Known};

/// Where a browser's policy is read from, and what is shipped underneath it.
pub struct Where {
    /// The browser's own program, which is what says whether it is installed.
    /// The policy directory does not: Chromium's package makes one and the
    /// other two leave it to whoever first writes a policy, so a directory
    /// that is not there is as often a browser nobody has configured as a
    /// browser nobody has.
    pub program: &'static str,
    pub file: &'static str,
    /// The browser's own policy file, which ours has to be built from because
    /// it is replaced rather than joined.
    pub beneath: &'static str,
}

/// Its own file in a directory Chromium merges, so whatever else is in there
/// stays. The palette writes a file of its own next to it.
pub const CHROMIUM: Where = Where {
    program: "chromium",
    file: "/etc/chromium/policies/managed/console-search.json",
    beneath: "",
};

pub const FIREFOX: Where = Where {
    program: "firefox",
    file: "/etc/firefox/policies/policies.json",
    beneath: "/usr/lib/firefox/distribution/policies.json",
};

pub const LIBREWOLF: Where = Where {
    program: "librewolf",
    file: "/etc/librewolf/policies/policies.json",
    beneath: "/usr/lib/librewolf/distribution/policies.json",
};

/// What a browser of either family puts where the question goes.
const WHERE_THE_QUESTION_GOES: &str = "{searchTerms}";

/// An add-on this desktop puts in the browser, and where the browser fetches it
/// from.
pub struct Addon {
    pub says: &'static str,
    /// What the add-on calls itself, which is the key a policy names it by.
    pub id: &'static str,
    pub from: &'static str,
}

/// What is installed for her.
///
/// Bitwarden, because every password on this machine is typed with a thumb on
/// an on-screen keyboard, which is the slowest way there is to type one and the
/// best reason there is not to.
///
/// Installed rather than forced, the way LibreWolf itself ships uBlock: the
/// browser fetches it on its next start and she can take it out again. An
/// add-on that cannot be removed is a thing on somebody's machine that is not
/// theirs.
pub const ADDONS: [Addon; 1] = [Addon {
    says: "Bitwarden",
    id: "{446900e4-71c2-419f-a6a7-df9c091e268b}",
    from: "https://addons.mozilla.org/firefox/downloads/latest/bitwarden-password-manager/latest.xpi",
}];

/// Chromium's, which says the engine outright rather than naming one it has.
pub fn chromium(engine: &Engine) -> String {
    let said = json!({
        "DefaultSearchProviderEnabled": true,
        "DefaultSearchProviderName": engine.says,
        "DefaultSearchProviderSearchURL": engine.asking(WHERE_THE_QUESTION_GOES),
    });
    pretty(&said)
}

/// Firefox's and LibreWolf's, laid over whatever the browser ships.
///
/// An engine the browser has is chosen by name. One it has not is handed over
/// first, because a default naming an engine that is not there is a browser
/// that goes on searching with the one it had.
///
/// The add-ons are added to the browser's own list rather than written over it.
/// LibreWolf's says which kinds of thing may be installed at all and ships
/// uBlock in the same breath, and a list of ours in its place would be an
/// add-on arriving and the browser's hardening leaving with it.
///
/// The preferences this desktop holds a browser to go in the same way, and for
/// the same reason.
pub fn mozilla(engine: &Engine, known: &Known, beneath: &str) -> String {
    let mut searching = Map::new();
    if known.given {
        searching.insert(
            "Add".to_string(),
            json!([{
                "Name": known.called,
                "URLTemplate": engine.asking(WHERE_THE_QUESTION_GOES),
                "Method": "GET",
            }]),
        );
    }
    searching.insert("Default".to_string(), json!(known.called));

    let mut said = serde_json::from_str::<Value>(beneath).unwrap_or_else(|_| json!({}));
    if !said.is_object() {
        said = json!({});
    }
    let policies = said
        .as_object_mut()
        .expect("an object")
        .entry("policies")
        .or_insert_with(|| json!({}));
    if !policies.is_object() {
        *policies = json!({});
    }
    let policies = policies.as_object_mut().expect("an object");
    policies.insert("SearchEngines".to_string(), Value::Object(searching));

    let installed = policies.entry("ExtensionSettings").or_insert_with(|| json!({}));
    if !installed.is_object() {
        *installed = json!({});
    }
    let installed = installed.as_object_mut().expect("an object");
    for addon in &ADDONS {
        installed.insert(
            addon.id.to_string(),
            json!({
                "install_url": addon.from,
                "installation_mode": "normal_installed",
                "private_browsing": true,
            }),
        );
    }
    let held = policies.entry("Preferences").or_insert_with(|| json!({}));
    if !held.is_object() {
        *held = json!({});
    }
    let held = held.as_object_mut().expect("an object");
    for (name, value) in preferred() {
        held.insert(name.to_string(), json!({ "Status": "locked", "Value": value }));
    }
    for named in ["DisableFirefoxStudies", "DisablePocket"] {
        policies.insert(named.to_string(), json!(true));
    }
    pretty(&said)
}

/// What this desktop holds a browser of the Firefox family to, whatever engine
/// is chosen.
///
/// Nothing on this machine animates and everything on it is dark, and a browser
/// is told so in its own language the way Hyprland, GTK and Qt each are. The
/// stylesheet is the last of them: the palette is written into the profile as a
/// userChrome file, and a browser that will not read one is a browser wearing
/// somebody else's colours.
///
/// Locked, because these are the desktop's answer rather than a place to start
/// from. They were a file under /etc that nothing installed and nothing knew
/// about, which is a decision that survives until the day something writes over
/// it.
fn preferred() -> Vec<(&'static str, Value)> {
    vec![
        ("browser.fullscreen.animate", json!(false)),
        ("browser.tabs.animate", json!(false)),
        ("toolkit.cosmeticAnimations.enabled", json!(false)),
        ("toolkit.legacyUserProfileCustomizations.stylesheets", json!(true)),
        ("ui.prefersReducedMotion", json!(1)),
        ("ui.systemUsesDarkTheme", json!(1)),
    ]
}

fn pretty(said: &Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(said).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    fn engine(key: &str) -> &'static Engine {
        engines::one(key).unwrap_or_else(|| panic!("{key}"))
    }

    fn read(said: &str) -> Value {
        serde_json::from_str(said).expect("what a browser would read")
    }

    #[test]
    fn chromium_is_told_the_engine_rather_than_a_name_for_one() {
        let said = read(&chromium(engine("startpage")));
        assert_eq!(said["DefaultSearchProviderName"], "Startpage");
        assert_eq!(
            said["DefaultSearchProviderSearchURL"],
            "https://www.startpage.com/sp/search?query={searchTerms}"
        );
    }

    #[test]
    fn a_browser_is_told_the_name_it_knows_the_engine_by() {
        let duckduckgo = engine("duckduckgo");
        let firefox = read(&mozilla(duckduckgo, &duckduckgo.firefox, ""));
        let librewolf = read(&mozilla(duckduckgo, &duckduckgo.librewolf, ""));
        assert_eq!(firefox["policies"]["SearchEngines"]["Default"], "DuckDuckGo");
        assert_eq!(librewolf["policies"]["SearchEngines"]["Default"], "DuckDuckGo No-AI");
    }

    #[test]
    fn an_engine_the_browser_has_is_chosen_and_not_handed_over() {
        let wikipedia = engine("wikipedia");
        let said = read(&mozilla(wikipedia, &wikipedia.librewolf, ""));
        assert!(said["policies"]["SearchEngines"]["Add"].is_null());
    }

    #[test]
    fn an_engine_the_browser_has_not_is_handed_over_first() {
        let startpage = engine("startpage");
        let said = read(&mozilla(startpage, &startpage.firefox, ""));
        let added = &said["policies"]["SearchEngines"]["Add"][0];
        assert_eq!(added["Name"], "Startpage");
        assert_eq!(added["URLTemplate"], "https://www.startpage.com/sp/search?query={searchTerms}");
    }

    /// The whole reason ours is built from theirs. LibreWolf's policy file is
    /// most of what makes LibreWolf worth having, and a file under /etc takes
    /// its place rather than adding to it.
    #[test]
    fn what_the_browser_ships_is_carried_through() {
        let shipped = r#"{"policies": {"DisableTelemetry": true, "SearchEngines": {"Default": "Gone"}}}"#;
        let duckduckgo = engine("duckduckgo");
        let said = read(&mozilla(duckduckgo, &duckduckgo.librewolf, shipped));
        assert_eq!(said["policies"]["DisableTelemetry"], true);
        assert_eq!(said["policies"]["SearchEngines"]["Default"], "DuckDuckGo No-AI");
    }

    #[test]
    fn the_add_ons_are_installed_and_can_still_be_taken_out() {
        let duckduckgo = engine("duckduckgo");
        let said = read(&mozilla(duckduckgo, &duckduckgo.librewolf, ""));
        let bitwarden = &said["policies"]["ExtensionSettings"][ADDONS[0].id];
        assert_eq!(bitwarden["installation_mode"], "normal_installed");
        assert_eq!(bitwarden["install_url"], ADDONS[0].from);
    }

    /// LibreWolf's own list says which kinds of thing may be installed at all,
    /// and ships uBlock in the same breath. Written over, an add-on of ours
    /// would arrive and the browser's hardening would leave with it.
    #[test]
    fn what_the_browser_installs_for_itself_is_kept() {
        let shipped = r#"{"policies": {"ExtensionSettings": {
            "*": {"installation_mode": "allowed"},
            "uBlock0@raymondhill.net": {"installation_mode": "normal_installed"}
        }}}"#;
        let duckduckgo = engine("duckduckgo");
        let said = read(&mozilla(duckduckgo, &duckduckgo.librewolf, shipped));
        let installed = &said["policies"]["ExtensionSettings"];
        assert_eq!(installed["*"]["installation_mode"], "allowed");
        assert_eq!(installed["uBlock0@raymondhill.net"]["installation_mode"], "normal_installed");
        assert!(!installed[ADDONS[0].id].is_null());
    }

    /// Nothing on this machine animates, and a browser is told that the way
    /// Hyprland, GTK and Qt each are.
    #[test]
    fn the_desktops_own_preferences_are_locked() {
        let duckduckgo = engine("duckduckgo");
        let said = read(&mozilla(duckduckgo, &duckduckgo.firefox, ""));
        let held = &said["policies"]["Preferences"];
        assert_eq!(held["ui.prefersReducedMotion"]["Value"], 1);
        assert_eq!(held["ui.prefersReducedMotion"]["Status"], "locked");
        assert_eq!(held["toolkit.cosmeticAnimations.enabled"]["Value"], false);
        assert_eq!(held["toolkit.legacyUserProfileCustomizations.stylesheets"]["Value"], true);
    }

    /// A shipped file that has become nonsense, or a browser that ships none.
    /// Neither is a reason to write nothing: the engine still has to be set.
    #[test]
    fn nothing_underneath_is_still_a_policy() {
        let duckduckgo = engine("duckduckgo");
        for beneath in ["", "not json at all", "[]"] {
            let said = read(&mozilla(duckduckgo, &duckduckgo.firefox, beneath));
            assert_eq!(said["policies"]["SearchEngines"]["Default"], "DuckDuckGo");
        }
    }
}
