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
/// Dark Reader, because everything else on this desktop is dark and a page that
/// is not is a lamp in the face at arm's length. It darkens the page itself
/// rather than asking the site to, which is the only way that works on a web
/// that mostly does not ask. It is on from the moment it arrives, which is the
/// whole of what is wanted from it.
///
/// This desktop's own add-on is not in this list, and the reason is worth
/// writing down because it cost a day to find. A policy will not install an
/// add-on nobody has signed. `xpinstall.signatures.required` is false in
/// LibreWolf and that is not enough: the pref governs a sideload, and the
/// policy path checks the signature whatever it says. Watched on the device --
/// a signed add-on from a `file://` URL installed, ours from the same kind of
/// URL did not, and nothing anywhere said why. So ours is put into the profile
/// instead, by `console-web`, and `crates/console-web` is what it does.
///
/// Both of these go to every browser of the family. Firefox is built with
/// MOZ_REQUIRE_SIGNING and no pref talks it out of that, so a browser-specific
/// list would only ever have held ours, and ours is no longer here.
///
/// Installed rather than forced, the way LibreWolf itself ships uBlock: the
/// browser fetches them on its next start and she can take either out again.
/// An add-on that cannot be removed is a thing on somebody's machine that is
/// not theirs.
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
pub fn mozilla(place: &Where, engine: &Engine, beneath: &str) -> String {
    let known: &Known = match place.file == FIREFOX.file {
        true => &engine.firefox,
        false => &engine.librewolf,
    };
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

    // Each nesting is taken out of the map, filled in, and put back, rather
    // than reached into where it lies. `serde_json` has no way to ask for the
    // object at a key that cannot come back empty-handed, so reaching in meant
    // the same three lines and the same assertion at every level -- guard that
    // what is there is an object, replace it if it is not, and then insist that
    // it now is. Taking it out says the same thing in the types: what comes
    // back is a map either way.
    let mut top = taken_from(match serde_json::from_str::<Value>(beneath) {
        Ok(v) => v,
        Err(_) => json!({}),
    });
    let mut policies = taken(&mut top, "policies");
    policies.insert("SearchEngines".to_string(), Value::Object(searching));

    let mut installed = taken(&mut policies, "ExtensionSettings");

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

    let mut held = taken(&mut policies, "Preferences");

    for (name, value) in preferred() {
        held.insert(name.to_string(), json!({ "Status": "locked", "Value": value }));
    }

    policies.insert("Preferences".to_string(), Value::Object(held));

    for named in ["DisableFirefoxStudies", "DisablePocket"] {
        policies.insert(named.to_string(), json!(true));
    }

    top.insert("policies".to_string(), Value::Object(policies));
    pretty(&Value::Object(top))
}

/// Whatever object that was, or an empty one.
///
/// A policies file that is a list, or a number, or a word, is a file nothing
/// can be merged into. What this desktop is about to write is the part that has
/// to survive, so what was there is given up rather than written around.
fn taken_from(said: Value) -> Map<String, Value> {
    match said {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// The object at `name`, lifted out of the map so it can be filled in.
///
/// Put back by the caller. Taken out rather than borrowed in place because the
/// next thing is another level down, and two nested borrows of the same map is
/// the shape that wanted an assertion at every step.
fn taken(from: &mut Map<String, Value>, name: &str) -> Map<String, Value> {
    from.remove(name).map(taken_from).unwrap_or_default()
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
    match serde_json::to_string_pretty(said) {
        Ok(written) => format!("{written}\n"),
        // These values are built in this file out of literals, so there is no
        // value of them serde cannot write. If that ever stops being true, an
        // empty policy file is a browser with no policy rather than a browser
        // with a broken one, and the journal says which.
        Err(fault) => {
            eprintln!("console-defaults: writing the policies: {fault}");

            String::new()
        }
    }
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
        let firefox = read(&mozilla(&FIREFOX, duckduckgo, ""));
        let librewolf = read(&mozilla(&LIBREWOLF, duckduckgo, ""));
        assert_eq!(firefox["policies"]["SearchEngines"]["Default"], "DuckDuckGo");
        assert_eq!(librewolf["policies"]["SearchEngines"]["Default"], "DuckDuckGo No-AI");
    }

    #[test]
    fn an_engine_the_browser_has_is_chosen_and_not_handed_over() {
        let wikipedia = engine("wikipedia");
        let said = read(&mozilla(&LIBREWOLF, wikipedia, ""));
        assert!(said["policies"]["SearchEngines"]["Add"].is_null());
    }

    #[test]
    fn an_engine_the_browser_has_not_is_handed_over_first() {
        let startpage = engine("startpage");
        let said = read(&mozilla(&FIREFOX, startpage, ""));
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
        let said = read(&mozilla(&LIBREWOLF, duckduckgo, shipped));
        assert_eq!(said["policies"]["DisableTelemetry"], true);
        assert_eq!(said["policies"]["SearchEngines"]["Default"], "DuckDuckGo No-AI");
    }

    #[test]
    fn the_add_ons_are_installed_and_can_still_be_taken_out() {
        let duckduckgo = engine("duckduckgo");
        let said = read(&mozilla(&LIBREWOLF, duckduckgo, ""));
        let bitwarden = &said["policies"]["ExtensionSettings"][ADDONS[0].id];
        assert_eq!(bitwarden["installation_mode"], "normal_installed");
        assert_eq!(bitwarden["install_url"], ADDONS[0].from);
    }

    /// Every add-on named here goes to both, because both can fetch them: a
    /// password is typed with a thumb in either, and a page is as bright in one
    /// as in the other.
    #[test]
    fn every_add_on_named_here_goes_to_all_of_them() {
        let duckduckgo = engine("duckduckgo");
        for place in [&FIREFOX, &LIBREWOLF] {
            let said = read(&mozilla(place, duckduckgo, ""));
            for addon in ADDONS.iter() {
                let held = &said["policies"]["ExtensionSettings"][addon.id];
                assert_eq!(held["install_url"], addon.from, "{} in {}", addon.says, place.file);
                assert_eq!(held["installation_mode"], "normal_installed", "{}", addon.says);
            }
        }
    }

    /// Nothing named here may be a file, and that is the whole of the lesson.
    /// A policy will not install an unsigned add-on, and every add-on that
    /// comes off this disk is unsigned, so one named here would be a policy
    /// that silently does nothing. Ours goes into the profile instead.
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

    /// A page that is not dark is a lamp in the face on a screen held at arm's
    /// length, so it arrives with the rest and is on the moment it does.
    #[test]
    fn the_one_that_darkens_a_page_is_installed_in_both() {
        let duckduckgo = engine("duckduckgo");
        let dark = ADDONS.iter().find(|addon| addon.says == "Dark Reader").expect("Dark Reader");
        assert_eq!(dark.id, "addon@darkreader.org");
        for place in [&FIREFOX, &LIBREWOLF] {
            let said = read(&mozilla(place, duckduckgo, ""));
            assert_eq!(
                said["policies"]["ExtensionSettings"][dark.id]["installation_mode"],
                "normal_installed",
                "{}",
                place.file
            );
        }
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
        let said = read(&mozilla(&LIBREWOLF, duckduckgo, shipped));
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
        let said = read(&mozilla(&FIREFOX, duckduckgo, ""));
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
            let said = read(&mozilla(&FIREFOX, duckduckgo, beneath));
            assert_eq!(said["policies"]["SearchEngines"]["Default"], "DuckDuckGo");
        }
    }
}
