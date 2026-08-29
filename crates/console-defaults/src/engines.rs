//! Which engine a question is asked of, and the address that asks it.
//!
//! The menu's search box narrows the list of applications. A line that narrows
//! it to nothing is a question, and this is where questions go.
//!
//! An address is opened and everything else is searched for. Told apart by
//! shape, because there is nothing to ask: a line with a space in it is not an
//! address, and one ending in a dot and letters is nothing else.

/// What a browser already calls an engine, and whether it has to be told.
///
/// A browser that ships an engine names it its own way, and a policy that
/// names it anything else sets the default to something the browser cannot
/// find. LibreWolf ships DuckDuckGo as "DuckDuckGo No-AI" and Firefox as
/// "DuckDuckGo", and neither of them ships Startpage the way the other does.
pub struct Known {
    pub called: &'static str,
    /// Whether the engine has to be handed over as well as chosen. An engine
    /// the browser already has cannot be handed over: a policy is not allowed
    /// to replace one the application provides.
    pub given: bool,
}

/// One engine: what it is called here, what it is called on screen, where a
/// question goes, and what each browser knows it as.
pub struct Engine {
    pub key: &'static str,
    pub says: &'static str,
    /// The address a question goes to, with `{}` where the question sits.
    pub asks: &'static str,
    pub firefox: Known,
    pub librewolf: Known,
}

/// The engines offered, in the order they are drawn.
pub const EVERY: [Engine; 3] = [
    Engine {
        key: "duckduckgo",
        says: "DuckDuckGo",
        asks: "https://duckduckgo.com/?q={}",
        firefox: Known { called: "DuckDuckGo", given: false },
        librewolf: Known { called: "DuckDuckGo No-AI", given: false },
    },
    Engine {
        key: "startpage",
        says: "Startpage",
        asks: "https://www.startpage.com/sp/search?query={}",
        firefox: Known { called: "Startpage", given: true },
        librewolf: Known { called: "Startpage", given: false },
    },
    Engine {
        key: "wikipedia",
        says: "Wikipedia",
        asks: "https://en.wikipedia.org/w/index.php?search={}",
        firefox: Known { called: "Wikipedia (en)", given: false },
        librewolf: Known { called: "Wikipedia (en)", given: false },
    },
];

impl Engine {
    /// The address, with something put where the question sits.
    ///
    /// A browser wants the placeholder left in and spelled its own way; the
    /// menu wants the question itself.
    pub fn asking(&self, question: &str) -> String {
        self.asks.replace("{}", question)
    }
}

/// The one used when nothing has been chosen.
///
/// The browser's own, so the answer is the same whether it was asked from the
/// menu or from the address bar.
pub const UNLESS_TOLD: &str = "duckduckgo";

/// The key of the engine in use.
pub fn chosen() -> String {
    let said = crate::setting("search").unwrap_or_default();
    match one(&said).is_some() {
        true => said,
        false => UNLESS_TOLD.to_string(),
    }
}

/// One engine by its key.
pub fn one(key: &str) -> Option<&'static Engine> {
    EVERY.iter().find(|engine| engine.key == key)
}

/// Remember which engine to ask from now on.
pub fn choose(key: &str) {
    crate::set("search", key);
}

/// The web address a typed line means, or nothing if it means nothing.
pub fn address(said: &str, engine: &Engine) -> Option<String> {
    let said = said.trim();
    if said.is_empty() {
        return None;
    }
    Some(match a_site(said) {
        true => with_a_scheme(said),
        false => engine.asking(&encoded(said)),
    })
}

/// Whether a line is somewhere rather than something to ask about.
fn a_site(said: &str) -> bool {
    if said.split_whitespace().count() != 1 {
        return false;
    }
    said.contains("://") || a_host(said.split(['/', '?', '#']).next().unwrap_or_default())
}

/// Whether the front of a line is a machine's name.
///
/// Every label has something in it, and the last is letters. Without that last
/// part a version number is a website: 3.14 has a dot in it and two labels
/// either side of it, and nobody typing it wants a page.
fn a_host(said: &str) -> bool {
    let labels: Vec<&str> = said.split('.').collect();
    let Some(last) = labels.last() else { return false };
    labels.len() > 1
        && labels.iter().all(|label| !label.is_empty())
        && last.len() > 1
        && last.chars().all(char::is_alphabetic)
}

/// A bare host, given the scheme a browser would have to guess at anyway.
fn with_a_scheme(said: &str) -> String {
    match said.contains("://") {
        true => said.to_string(),
        false => format!("https://{said}"),
    }
}

/// A line, as it is written into a query string.
///
/// Everything that is not a letter, a digit or one of the four characters a
/// URL leaves alone comes out as its bytes.
fn encoded(said: &str) -> String {
    said.bytes()
        .map(|byte| match byte {
            b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'_' | b'a'..=b'z' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duck() -> &'static Engine {
        one(UNLESS_TOLD).expect("the one used when nothing has been chosen")
    }

    #[test]
    fn a_question_is_searched_for() {
        assert_eq!(
            address("how tall is a giraffe", duck()),
            Some("https://duckduckgo.com/?q=how%20tall%20is%20a%20giraffe".to_string())
        );
    }

    #[test]
    fn a_question_that_is_not_letters_is_still_a_question() {
        assert_eq!(
            address("100% & up", duck()),
            Some("https://duckduckgo.com/?q=100%25%20%26%20up".to_string())
        );
        let caffe = address("caffè", duck()).expect("something");
        assert!(caffe.ends_with("caff%C3%A8"), "its bytes, not its letters: {caffe}");
    }

    #[test]
    fn the_engine_chosen_is_the_one_asked() {
        let question = |key: &str| address("beans", one(key).expect(key)).expect("something");
        assert!(question("startpage").starts_with("https://www.startpage.com/"));
        assert!(question("wikipedia").starts_with("https://en.wikipedia.org/"));
    }

    #[test]
    fn an_address_is_opened_rather_than_searched_for() {
        assert_eq!(address("codincod.com", duck()), Some("https://codincod.com".to_string()));
        assert_eq!(
            address("codincod.com/puzzles?page=2", duck()),
            Some("https://codincod.com/puzzles?page=2".to_string())
        );
    }

    #[test]
    fn an_address_that_says_its_own_scheme_keeps_it() {
        assert_eq!(
            address("http://192.168.1.1", duck()),
            Some("http://192.168.1.1".to_string())
        );
    }

    /// A launcher is typed into in a hurry, and most of what lands in it is
    /// words. Anything that could be either is a question.
    #[test]
    fn what_is_not_quite_an_address_is_a_question() {
        for said in ["3.14", "st. peter", "hello world", "wofi", "a.b", "one..com"] {
            let asked = address(said, duck()).expect("something");
            assert!(asked.starts_with("https://duckduckgo.com/?q="), "{said:?} was opened as a site: {asked}");
        }
    }

    #[test]
    fn nothing_typed_means_nothing() {
        assert_eq!(address("", duck()), None);
        assert_eq!(address("   ", duck()), None);
    }

    /// A key written into the file by hand and spelled wrong would otherwise be
    /// a menu whose search box does nothing at all.
    #[test]
    fn the_one_used_when_nothing_has_been_chosen_is_one_of_them() {
        assert!(one(UNLESS_TOLD).is_some());
        assert!(one("askjeeves").is_none());
    }

    /// A placeholder left in the address is a search for the word "{}".
    #[test]
    fn every_engine_has_somewhere_to_put_the_question() {
        for engine in &EVERY {
            assert!(engine.asks.contains("{}"), "{} has nowhere to put it", engine.says);
            assert!(!engine.asking("beans").contains("{}"), "{} kept it", engine.says);
        }
    }

    /// An engine the browser already ships cannot be handed to it, and one it
    /// does not ship has to be. Getting either the wrong way round leaves the
    /// browser looking for an engine by a name nothing answers to.
    #[test]
    fn an_engine_a_browser_already_has_is_not_handed_to_it() {
        let duckduckgo = one("duckduckgo").expect("duckduckgo");
        assert!(!duckduckgo.librewolf.given, "librewolf ships it");
        assert_eq!(duckduckgo.librewolf.called, "DuckDuckGo No-AI");
        assert!(one("startpage").expect("startpage").firefox.given, "firefox does not ship it");
    }

    #[test]
    fn every_engine_is_named_once_and_in_order() {
        let keys: Vec<&str> = EVERY.iter().map(|engine| engine.key).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(keys, sorted, "the engines are out of order or named twice");
    }
}
