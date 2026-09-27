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

use console_core_never::Never;
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
    let asking = engine.search_url(WHERE_THE_QUESTION_GOES)?;
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
            let asking = engine.search_url(WHERE_THE_QUESTION_GOES)?;

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
        Ok(value) => value,
        Err(_not_json) => json!({}),
    };
    let mut top = taken_from(said)?;
    let mut policies = take_object(&mut top, "policies")?;
    policies.insert("SearchEngines".to_string(), Value::Object(searching));

    let mut installed = take_object(&mut policies, "ExtensionSettings")?;

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

    let mut held = take_object(&mut policies, "Preferences")?;

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

fn take_object(from: &mut Map<String, Value>, name: &str) -> Result<Map<String, Value>, Never> {
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
            eprintln!("console-default-applications: writing the policies: {fault}");

            String::new()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use crate::engines;

    fn engine(key: &str) -> Result<&'static Engine, Box<dyn Error>> {
        let Ok(found) = engines::one(key);
        let engine = found.ok_or("no engine is called that")?;

        Ok(engine)
    }

    fn policy(place: &Where, engine: &Engine, beneath: &str) -> Result<Value, serde_json::Error> {
        let Ok(said) = mozilla(place, engine, beneath);

        serde_json::from_str(&said)
    }

    fn installing(addon: &Addon, what: &str) -> Result<String, Never> {
        Ok(format!("/policies/ExtensionSettings/{}/{what}", addon.id))
    }

    #[test]
    fn chromium_is_told_the_engine_rather_than_a_name_for_one() -> Result<(), Box<dyn Error>> {
        let startpage = engine("startpage")?;
        let Ok(written) = chromium(startpage);
        let said: Value = serde_json::from_str(&written)?;

        assert_eq!(said.pointer("/DefaultSearchProviderName").and_then(Value::as_str), Some("Startpage"));
        assert_eq!(
            said.pointer("/DefaultSearchProviderSearchURL").and_then(Value::as_str),
            Some("https://www.startpage.com/sp/search?query={searchTerms}")
        );

        Ok(())
    }

    #[test]
    fn a_browser_is_told_the_name_it_knows_the_engine_by() -> Result<(), Box<dyn Error>> {
        let duckduckgo = engine("duckduckgo")?;
        let firefox = policy(&FIREFOX, duckduckgo, "")?;
        let librewolf = policy(&LIBREWOLF, duckduckgo, "")?;

        assert_eq!(firefox.pointer("/policies/SearchEngines/Default").and_then(Value::as_str), Some("DuckDuckGo"));
        assert_eq!(
            librewolf.pointer("/policies/SearchEngines/Default").and_then(Value::as_str),
            Some("DuckDuckGo No-AI")
        );

        Ok(())
    }

    #[test]
    fn an_engine_the_browser_has_is_chosen_and_not_handed_over() -> Result<(), Box<dyn Error>> {
        let wikipedia = engine("wikipedia")?;
        let said = policy(&LIBREWOLF, wikipedia, "")?;

        assert_eq!(said.pointer("/policies/SearchEngines/Add"), None);

        Ok(())
    }

    #[test]
    fn an_engine_the_browser_has_not_is_handed_over_first() -> Result<(), Box<dyn Error>> {
        let startpage = engine("startpage")?;
        let said = policy(&FIREFOX, startpage, "")?;
        let added = said.pointer("/policies/SearchEngines/Add/0").ok_or("nothing was added")?;

        assert_eq!(added.pointer("/Name").and_then(Value::as_str), Some("Startpage"));
        assert_eq!(
            added.pointer("/URLTemplate").and_then(Value::as_str),
            Some("https://www.startpage.com/sp/search?query={searchTerms}")
        );

        Ok(())
    }

    #[test]
    fn what_the_browser_ships_is_carried_through() -> Result<(), Box<dyn Error>> {
        let shipped = r#"{"policies": {"DisableTelemetry": true, "SearchEngines": {"Default": "Gone"}}}"#;
        let duckduckgo = engine("duckduckgo")?;
        let said = policy(&LIBREWOLF, duckduckgo, shipped)?;

        assert_eq!(said.pointer("/policies/DisableTelemetry").and_then(Value::as_bool), Some(true));
        assert_eq!(
            said.pointer("/policies/SearchEngines/Default").and_then(Value::as_str),
            Some("DuckDuckGo No-AI")
        );

        Ok(())
    }

    #[test]
    fn the_add_ons_are_installed_and_can_still_be_taken_out() -> Result<(), Box<dyn Error>> {
        let duckduckgo = engine("duckduckgo")?;
        let said = policy(&LIBREWOLF, duckduckgo, "")?;
        let bitwarden = ADDONS.first().ok_or("no add-ons at all")?;
        let Ok(mode) = installing(bitwarden, "installation_mode");
        let Ok(from) = installing(bitwarden, "install_url");

        assert_eq!(said.pointer(&mode).and_then(Value::as_str), Some("normal_installed"));
        assert_eq!(said.pointer(&from).and_then(Value::as_str), Some(bitwarden.from));

        Ok(())
    }

    #[test]
    fn every_add_on_named_here_goes_to_all_of_them() -> Result<(), Box<dyn Error>> {
        let duckduckgo = engine("duckduckgo")?;

        for place in [&FIREFOX, &LIBREWOLF] {
            let said = policy(place, duckduckgo, "")?;

            for addon in ADDONS.iter() {
                let Ok(mode) = installing(addon, "installation_mode");
                let Ok(from) = installing(addon, "install_url");

                assert_eq!(
                    said.pointer(&from).and_then(Value::as_str),
                    Some(addon.from),
                    "{} in {}",
                    addon.says,
                    place.file
                );
                assert_eq!(said.pointer(&mode).and_then(Value::as_str), Some("normal_installed"), "{}", addon.says);
            }
        }

        Ok(())
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
    fn the_one_that_darkens_a_page_is_installed_in_both() -> Result<(), Box<dyn Error>> {
        let duckduckgo = engine("duckduckgo")?;
        let dark = ADDONS.iter().find(|addon| addon.says == "Dark Reader").ok_or("Dark Reader")?;
        let Ok(mode) = installing(dark, "installation_mode");

        assert_eq!(dark.id, "addon@darkreader.org");

        for place in [&FIREFOX, &LIBREWOLF] {
            let said = policy(place, duckduckgo, "")?;

            assert_eq!(
                said.pointer(&mode).and_then(Value::as_str),
                Some("normal_installed"),
                "{}",
                place.file
            );
        }

        Ok(())
    }

    #[test]
    fn what_the_browser_installs_for_itself_is_kept() -> Result<(), Box<dyn Error>> {
        let shipped = r#"{"policies": {"ExtensionSettings": {
            "*": {"installation_mode": "allowed"},
            "uBlock0@raymondhill.net": {"installation_mode": "normal_installed"}
        }}}"#;
        let duckduckgo = engine("duckduckgo")?;
        let said = policy(&LIBREWOLF, duckduckgo, shipped)?;
        let bitwarden = ADDONS.first().ok_or("no add-ons at all")?;
        let Ok(mode) = installing(bitwarden, "installation_mode");

        assert_eq!(
            said.pointer("/policies/ExtensionSettings/*/installation_mode").and_then(Value::as_str),
            Some("allowed")
        );
        assert_eq!(
            said.pointer("/policies/ExtensionSettings/uBlock0@raymondhill.net/installation_mode")
                .and_then(Value::as_str),
            Some("normal_installed")
        );
        assert!(said.pointer(&mode).is_some());

        Ok(())
    }

    #[test]
    fn the_desktops_own_preferences_are_locked() -> Result<(), Box<dyn Error>> {
        let duckduckgo = engine("duckduckgo")?;
        let said = policy(&FIREFOX, duckduckgo, "")?;
        let held = said.pointer("/policies/Preferences").ok_or("no preferences")?;

        assert_eq!(held.pointer("/ui.prefersReducedMotion/Value").and_then(Value::as_i64), Some(1));
        assert_eq!(held.pointer("/ui.prefersReducedMotion/Status").and_then(Value::as_str), Some("locked"));
        assert_eq!(
            held.pointer("/toolkit.cosmeticAnimations.enabled/Value").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            held.pointer("/toolkit.legacyUserProfileCustomizations.stylesheets/Value").and_then(Value::as_bool),
            Some(true)
        );

        Ok(())
    }

    #[test]
    fn nothing_underneath_is_still_a_policy() -> Result<(), Box<dyn Error>> {
        let duckduckgo = engine("duckduckgo")?;

        for beneath in ["", "not json at all", "[]"] {
            let said = policy(&FIREFOX, duckduckgo, beneath)?;

            assert_eq!(said.pointer("/policies/SearchEngines/Default").and_then(Value::as_str), Some("DuckDuckGo"));
        }

        Ok(())
    }
}
