//! Which engine a question is asked of, and the address that asks it.
//!
//! The menu's search box narrows the list of applications. A line that narrows
//! it to nothing is a question, and this is where questions go.
//!
//! An address is opened and everything else is searched for. Told apart by
//! shape, because there is nothing to ask: a line with a space in it is not an
//! address, and one ending in a dot and letters is nothing else.

use console_core_never::Never;

pub struct Known {
    pub called: &'static str,
    pub given: bool,
}

pub struct Engine {
    pub key: &'static str,
    pub says: &'static str,
    pub asks: &'static str,
    pub firefox: Known,
    pub librewolf: Known,
}

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
    pub fn asking(&self, question: &str) -> Result<String, Never> {
        Ok(self.asks.replace("{}", question))
    }
}

pub const UNLESS_TOLD: &str = "duckduckgo";

pub fn chosen() -> Result<String, Never> {
    let told = crate::setting("search")?;
    let said = told.unwrap_or_default();
    let known = one(&said)?;

    Ok(match known.is_some() {
        true => said,
        false => UNLESS_TOLD.to_string(),
    })
}

pub fn one(key: &str) -> Result<Option<&'static Engine>, Never> {
    Ok(EVERY.iter().find(|engine| engine.key == key))
}

pub fn choose(key: &str) -> Result<(), Never> {
    crate::set("search", key)
}

pub fn address(said: &str, engine: &Engine) -> Result<Option<String>, Never> {
    let said = said.trim();

    match said.is_empty() {
        true => return Ok(None),
        false => {}
    }

    let typed = a_site(said)?;

    Ok(Some(match typed {
        Typed::ASite => with_a_scheme(said)?,
        Typed::AQuestion => {
            let question = encoded(said)?;

            engine.asking(&question)?
        }
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typed {
    ASite,
    AQuestion,
}

fn a_site(said: &str) -> Result<Typed, Never> {
    match said.split_whitespace().count() == 1 {
        true => {}
        false => return Ok(Typed::AQuestion),
    }

    let host = a_host(said.split(['/', '?', '#']).next().unwrap_or_default())?;

    Ok(match said.contains("://") || host == Typed::ASite {
        true => Typed::ASite,
        false => Typed::AQuestion,
    })
}

fn a_host(said: &str) -> Result<Typed, Never> {
    let labels: Vec<&str> = said.split('.').collect();

    let Some(last) = labels.last() else { return Ok(Typed::AQuestion) };

    let named = labels.len() > 1
        && labels.iter().all(|label| !label.is_empty())
        && last.len() > 1
        && last.chars().all(char::is_alphabetic);

    Ok(match named {
        true => Typed::ASite,
        false => Typed::AQuestion,
    })
}

fn with_a_scheme(said: &str) -> Result<String, Never> {
    Ok(match said.contains("://") {
        true => said.to_string(),
        false => format!("https://{said}"),
    })
}

fn encoded(said: &str) -> Result<String, Never> {
    Ok(said
        .bytes()
        .map(|byte| match byte {
            b'-' | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'_' | b'a'..=b'z' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duck() -> &'static Engine {
        let Ok(one) = one(UNLESS_TOLD);

        one.expect("the one used when nothing has been chosen")
    }

    #[test]
    fn a_question_is_searched_for() {
        assert_eq!(
            address("how tall is a giraffe", duck()),
            Ok(Some("https://duckduckgo.com/?q=how%20tall%20is%20a%20giraffe".to_string()))
        );
    }

    #[test]
    fn a_question_that_is_not_letters_is_still_a_question() {
        assert_eq!(
            address("100% & up", duck()),
            Ok(Some("https://duckduckgo.com/?q=100%25%20%26%20up".to_string()))
        );

        let Ok(asked) = address("caffè", duck());
        let caffe = asked.expect("something");

        assert!(caffe.ends_with("caff%C3%A8"), "its bytes, not its letters: {caffe}");
    }

    #[test]
    fn the_engine_chosen_is_the_one_asked() {
        let question = |key: &str| {
            let Ok(known) = one(key);
            let Ok(asked) = address("beans", known.expect(key));

            asked.expect("something")
        };

        assert!(question("startpage").starts_with("https://www.startpage.com/"));
        assert!(question("wikipedia").starts_with("https://en.wikipedia.org/"));
    }

    #[test]
    fn an_address_is_opened_rather_than_searched_for() {
        assert_eq!(address("codincod.com", duck()), Ok(Some("https://codincod.com".to_string())));
        assert_eq!(
            address("codincod.com/puzzles?page=2", duck()),
            Ok(Some("https://codincod.com/puzzles?page=2".to_string()))
        );
    }

    #[test]
    fn an_address_that_says_its_own_scheme_keeps_it() {
        assert_eq!(
            address("http://192.168.1.1", duck()),
            Ok(Some("http://192.168.1.1".to_string()))
        );
    }

    #[test]
    fn what_is_not_quite_an_address_is_a_question() {
        for said in ["3.14", "st. peter", "hello world", "wofi", "a.b", "one..com"] {
            let Ok(answered) = address(said, duck());
            let asked = answered.expect("something");

            assert!(
                asked.starts_with("https://duckduckgo.com/?q="),
                "{said:?} was opened as a site: {asked}"
            );
        }
    }

    #[test]
    fn nothing_typed_means_nothing() {
        assert_eq!(address("", duck()), Ok(None));
        assert_eq!(address("   ", duck()), Ok(None));
    }

    #[test]
    fn the_one_used_when_nothing_has_been_chosen_is_one_of_them() {
        assert_eq!(one(UNLESS_TOLD).map(|found| found.is_some()), Ok(true));
        assert_eq!(one("askjeeves").map(|found| found.is_none()), Ok(true));
    }

    #[test]
    fn every_engine_has_somewhere_to_put_the_question() {
        for engine in &EVERY {
            let Ok(asked) = engine.asking("beans");

            assert!(engine.asks.contains("{}"), "{} has nowhere to put it", engine.says);
            assert!(!asked.contains("{}"), "{} kept it", engine.says);
        }
    }

    #[test]
    fn an_engine_a_browser_already_has_is_not_handed_to_it() {
        let Ok(found) = one("duckduckgo");
        let duckduckgo = found.expect("duckduckgo");

        assert!(!duckduckgo.librewolf.given, "librewolf ships it");
        assert_eq!(duckduckgo.librewolf.called, "DuckDuckGo No-AI");

        let Ok(found) = one("startpage");

        assert!(found.expect("startpage").firefox.given, "firefox does not ship it");
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
