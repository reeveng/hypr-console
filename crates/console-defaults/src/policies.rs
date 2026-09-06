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

use console_never::Never;
use serde_json::{Map, Value, json};

use crate::engines::{Engine, Known};

pub struct Where {
    pub program: &'static str,
    pub file: &'static str,
    pub beneath: &'static str,
}

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

const WHERE_THE_QUESTION_GOES: &str = "{searchTerms}";

pub struct Addon {
    pub says: &'static str,
    pub id: &'static str,
    pub from: &'static str,
}

pub const ADDONS: [Addon; 2] = [
    Addon {
        says: "Bitwarden",
        id: "{446900e4-71c2-419f-a6a7-df9c091e268b}",
        from: "https://addons.mozilla.org/firefox/downloads/latest/bitwarden-password-manager/latest.xpi",
    },
    Addon {
        says: "Dark Reader",
        id: "addon@darkreader.org",
        from: "https://addons.mozilla.org/firefox/downloads/latest/darkreader/latest.xpi",
    },
];

pub fn chromium(engine: &Engine) -> Result<String, Never> {
    let asking = engine.asking(WHERE_THE_QUESTION_GOES)?;
    let said = json!({
        "DefaultSearchProviderEnabled": true,
        "DefaultSearchProviderName": engine.says,
        "DefaultSearchProviderSearchURL": asking,
    });

    pretty(&said)
}

pub fn mozilla(place: &Where, engine: &Engine, beneath: &str) -> Result<String, Never> {
    let known: &Known = match place.file == FIREFOX.file {
        true => &engine.firefox,
        false => &engine.librewolf,
    };
    let mut searching = Map::new();

    match known.given {
        true => {
            let asking = engine.asking(WHERE_THE_QUESTION_GOES)?;

            searching.insert(
                "Add".to_string(),
                json!([{
                    "Name": known.called,
                    "URLTemplate": asking,
                    "Method": "GET",
                }]),
            );
        }
        false => {}
    }

    searching.insert("Default".to_string(), json!(known.called));

    let said = match serde_json::from_str::<Value>(beneath) {
        Ok(v) => v,
        Err(_) => json!({}),
    };
    let mut top = taken_from(said)?;
    let mut policies = taken(&mut top, "policies")?;
    policies.insert("SearchEngines".to_string(), Value::Object(searching));

    let mut installed = taken(&mut policies, "ExtensionSettings")?;

    for addon in ADDONS.iter() {
        installed.insert(
            addon.id.to_string(),
            json!({
                "install_url": addon.from,
                "installation_mode": "normal_installed",
                "private_browsing": true,
            }),
        );
    }

    policies.insert("ExtensionSettings".to_string(), Value::Object(installed));

    let mut held = taken(&mut policies, "Preferences")?;

    let wanted = preferred()?;

    for (name, value) in wanted {
        held.insert(name.to_string(), json!({ "Status": "locked", "Value": value }));
    }

    policies.insert("Preferences".to_string(), Value::Object(held));

    for named in ["DisableFirefoxStudies", "DisablePocket"] {
        policies.insert(named.to_string(), json!(true));
    }

    top.insert("policies".to_string(), Value::Object(policies));

    pretty(&Value::Object(top))
}

fn taken_from(said: Value) -> Result<Map<String, Value>, Never> {
    Ok(match said {
        Value::Object(map) => map,
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            Map::new()
        }
    })
}

fn taken(from: &mut Map<String, Value>, name: &str) -> Result<Map<String, Value>, Never> {
    match from.remove(name) {
        Some(said) => taken_from(said),
        None => Ok(Map::new()),
    }
}

fn preferred() -> Result<Vec<(&'static str, Value)>, Never> {
    Ok(vec![
        ("browser.fullscreen.animate", json!(false)),
        ("browser.tabs.animate", json!(false)),
        ("toolkit.cosmeticAnimations.enabled", json!(false)),
        ("toolkit.legacyUserProfileCustomizations.stylesheets", json!(true)),
        ("ui.prefersReducedMotion", json!(1)),
        ("ui.systemUsesDarkTheme", json!(1)),
    ])
}

fn pretty(said: &Value) -> Result<String, Never> {
    Ok(match serde_json::to_string_pretty(said) {
        Ok(written) => format!("{written}\n"),
        Err(fault) => {
            eprintln!("console-defaults: writing the policies: {fault}");

            String::new()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines;

    fn engine(key: &str) -> &'static Engine {
        let Ok(found) = engines::one(key);

        found.unwrap_or_else(|| panic!("{key}"))
    }

    fn policy(place: &Where, engine: &Engine, beneath: &str) -> Value {
        let Ok(said) = mozilla(place, engine, beneath);

        read(&said)
    }

    fn read(said: &str) -> Value {
        serde_json::from_str(said).expect("what a browser would read")
    }

    #[test]
    fn chromium_is_told_the_engine_rather_than_a_name_for_one() {
        let Ok(written) = chromium(engine("startpage"));
        let said = read(&written);

        assert_eq!(said["DefaultSearchProviderName"], "Startpage");
        assert_eq!(
            said["DefaultSearchProviderSearchURL"],
            "https://www.startpage.com/sp/search?query={searchTerms}"
        );
    }

    #[test]
    fn a_browser_is_told_the_name_it_knows_the_engine_by() {
        let duckduckgo = engine("duckduckgo");
        let firefox = policy(&FIREFOX, duckduckgo, "");
        let librewolf = policy(&LIBREWOLF, duckduckgo, "");
        assert_eq!(firefox["policies"]["SearchEngines"]["Default"], "DuckDuckGo");
        assert_eq!(librewolf["policies"]["SearchEngines"]["Default"], "DuckDuckGo No-AI");
    }

    #[test]
    fn an_engine_the_browser_has_is_chosen_and_not_handed_over() {
        let wikipedia = engine("wikipedia");
        let said = policy(&LIBREWOLF, wikipedia, "");
        assert!(said["policies"]["SearchEngines"]["Add"].is_null());
    }

    #[test]
    fn an_engine_the_browser_has_not_is_handed_over_first() {
        let startpage = engine("startpage");
        let said = policy(&FIREFOX, startpage, "");
        let added = &said["policies"]["SearchEngines"]["Add"][0];
        assert_eq!(added["Name"], "Startpage");
        assert_eq!(added["URLTemplate"], "https://www.startpage.com/sp/search?query={searchTerms}");
    }

    #[test]
    fn what_the_browser_ships_is_carried_through() {
        let shipped = r#"{"policies": {"DisableTelemetry": true, "SearchEngines": {"Default": "Gone"}}}"#;
        let duckduckgo = engine("duckduckgo");
        let said = policy(&LIBREWOLF, duckduckgo, shipped);
        assert_eq!(said["policies"]["DisableTelemetry"], true);
        assert_eq!(said["policies"]["SearchEngines"]["Default"], "DuckDuckGo No-AI");
    }

    #[test]
    fn the_add_ons_are_installed_and_can_still_be_taken_out() {
        let duckduckgo = engine("duckduckgo");
        let said = policy(&LIBREWOLF, duckduckgo, "");
        let bitwarden = &said["policies"]["ExtensionSettings"][ADDONS[0].id];
        assert_eq!(bitwarden["installation_mode"], "normal_installed");
        assert_eq!(bitwarden["install_url"], ADDONS[0].from);
    }

    #[test]
    fn every_add_on_named_here_goes_to_all_of_them() {
        let duckduckgo = engine("duckduckgo");
        for place in [&FIREFOX, &LIBREWOLF] {
            let said = policy(place, duckduckgo, "");
            for addon in ADDONS.iter() {
                let held = &said["policies"]["ExtensionSettings"][addon.id];
                assert_eq!(held["install_url"], addon.from, "{} in {}", addon.says, place.file);
                assert_eq!(held["installation_mode"], "normal_installed", "{}", addon.says);
            }
        }
    }

    #[test]
    fn nothing_a_policy_installs_comes_off_the_disk() {
        for addon in ADDONS.iter() {
            assert!(
                addon.from.starts_with("https://"),
                "{} is installed from {}, which a policy will not do unsigned",
                addon.says,
                addon.from
            );
        }
    }

    #[test]
    fn the_one_that_darkens_a_page_is_installed_in_both() {
        let duckduckgo = engine("duckduckgo");
        let dark = ADDONS.iter().find(|addon| addon.says == "Dark Reader").expect("Dark Reader");
        assert_eq!(dark.id, "addon@darkreader.org");
        for place in [&FIREFOX, &LIBREWOLF] {
            let said = policy(place, duckduckgo, "");
            assert_eq!(
                said["policies"]["ExtensionSettings"][dark.id]["installation_mode"],
                "normal_installed",
                "{}",
                place.file
            );
        }
    }

    #[test]
    fn what_the_browser_installs_for_itself_is_kept() {
        let shipped = r#"{"policies": {"ExtensionSettings": {
            "*": {"installation_mode": "allowed"},
            "uBlock0@raymondhill.net": {"installation_mode": "normal_installed"}
        }}}"#;
        let duckduckgo = engine("duckduckgo");
        let said = policy(&LIBREWOLF, duckduckgo, shipped);
        let installed = &said["policies"]["ExtensionSettings"];
        assert_eq!(installed["*"]["installation_mode"], "allowed");
        assert_eq!(installed["uBlock0@raymondhill.net"]["installation_mode"], "normal_installed");
        assert!(!installed[ADDONS[0].id].is_null());
    }

    #[test]
    fn the_desktops_own_preferences_are_locked() {
        let duckduckgo = engine("duckduckgo");
        let said = policy(&FIREFOX, duckduckgo, "");
        let held = &said["policies"]["Preferences"];
        assert_eq!(held["ui.prefersReducedMotion"]["Value"], 1);
        assert_eq!(held["ui.prefersReducedMotion"]["Status"], "locked");
        assert_eq!(held["toolkit.cosmeticAnimations.enabled"]["Value"], false);
        assert_eq!(held["toolkit.legacyUserProfileCustomizations.stylesheets"]["Value"], true);
    }

    #[test]
    fn nothing_underneath_is_still_a_policy() {
        let duckduckgo = engine("duckduckgo");
        for beneath in ["", "not json at all", "[]"] {
            let said = policy(&FIREFOX, duckduckgo, beneath);
            assert_eq!(said["policies"]["SearchEngines"]["Default"], "DuckDuckGo");
        }
    }
}
