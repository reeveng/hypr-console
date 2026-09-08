//! Which words this machine is in, as glibc and localectl say them.
//!
//! Two questions look like one. What language the *desktop* says its own rows
//! in is `console_core_localization`, and there is one of those; what language
//! everything else on the machine is in -- the browser's menus, the shop, the
//! names of the months, which way round a date is written -- is a locale, and
//! glibc has five hundred of them. This is the second. A person who reads
//! Dutch gets a Dutch browser and a Dutch calendar out of it, and the rows
//! this desktop draws stay English until somebody writes them, which is a
//! thing the panel says rather than a thing it hides.
//!
//! A locale has to be *made* before it can be used: glibc ships the recipes in
//! `/usr/share/i18n` and `locale-gen` compiles the ones `/etc/locale.gen`
//! names. So the list is every locale this machine could have, marked with the
//! ones it already has, and choosing one that has not been made makes it. That
//! is the whole of what "install a language" means here, and it is why this
//! goes through `console-machine`: all three files are root's.
//!
//! Only the UTF-8 half of `SUPPORTED` is offered. The other half is
//! ISO-8859-15 and TIS-620 and the rest of what the world used before, and a
//! handheld that draws Thai and Greek in the same font as its own rows has one
//! encoding. Offering a second would be offering a way to make the keyboard
//! and the screen disagree.
//!
//! The names come from `iso-codes`, which is the system's own answer to what a
//! language is called, in the same way `xkeyboard-config` is its answer to what
//! a key produces. Writing them here would be five hundred English nouns in a
//! Rust file, wrong the first time a country is renamed, and untranslatable the
//! day this desktop speaks a second language. A machine without `iso-codes`
//! draws the codes instead and says why once.

use std::collections::BTreeMap;

use console_core_never::Never;

pub const SUPPORTED: &str = "/usr/share/i18n/SUPPORTED";

pub const LANGUAGES: &str = "/usr/share/iso-codes/json/iso_639-3.json";

pub const PLACES: &str = "/usr/share/iso-codes/json/iso_3166-1.json";

const UTF8: &str = "UTF-8";

const NOBODY_CHOOSES: [&str; 2] = ["C", "POSIX"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub name: String,
    pub charset: String,
    pub language: String,
    pub place: String,
    pub how: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Made {
    Yes,
    No,
}

impl Locale {
    pub fn line(&self) -> Result<String, Never> {
        Ok(format!("{} {}", self.name, self.charset))
    }

    pub fn lang(&self) -> Result<String, Never> {
        Ok(match self.name.contains('.') {
            true => self.name.clone(),
            false => format!("{}.{UTF8}", self.name),
        })
    }
}

pub fn read(name: &str, charset: &str) -> Result<Option<Locale>, Never> {
    let stem = name.split('.').next().unwrap_or_default();
    let mut parts = stem.split('@');
    let named = parts.next().unwrap_or_default();
    let how = parts.next().unwrap_or_default();

    let mut halves = named.split('_');
    let language = halves.next().unwrap_or_default();
    let place = halves.next().unwrap_or_default();

    let nobody = NOBODY_CHOOSES.contains(&language);

    match language.is_empty() || nobody {
        true => Ok(None),
        false => Ok(Some(Locale {
            name: name.to_string(),
            charset: charset.to_string(),
            language: language.to_string(),
            place: place.to_string(),
            how: how.to_string(),
        })),
    }
}

pub fn supported(said: &str) -> Result<Vec<Locale>, Never> {
    let mut kept: Vec<Locale> = Vec::new();

    for line in said.lines() {
        let mut words = line.split_whitespace();

        let (name, charset) = match (words.next(), words.next()) {
            (Some(name), Some(charset)) => (name, charset),
            (Some(_), None) | (None, Some(_)) | (None, None) => continue,
        };

        match charset == UTF8 {
            true => {},
            false => continue,
        }

        let one = read(name, charset)?;

        match one {
            Some(locale) => kept.push(locale),
            None => {},
        }
    }

    Ok(kept)
}

pub fn generated(said: &str) -> Result<Vec<String>, Never> {
    Ok(said.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect())
}

fn plainly(name: &str) -> Result<String, Never> {
    Ok(name.to_lowercase().replace('-', ""))
}

pub fn made(generated: &[String], locale: &Locale) -> Result<Made, Never> {
    let Ok(lang) = locale.lang();
    let Ok(wanted) = plainly(&lang);

    let found = generated.iter().any(|already| {
        let Ok(said) = plainly(already);

        said == wanted
    });

    Ok(match found {
        true => Made::Yes,
        false => Made::No,
    })
}

pub fn chosen(said: &str) -> Result<Option<String>, Never> {
    for line in said.lines() {
        for word in line.split_whitespace() {
            match word.strip_prefix("LANG=") {
                Some(name) => return Ok(Some(name.to_string())),
                None => {},
            }
        }
    }

    Ok(None)
}

pub const LOCALE_GEN: &str = "/etc/locale.gen";

pub const LOCALE_CONF: &str = "/etc/locale.conf";

pub fn generating(said: &str, line: &str) -> Result<String, Never> {
    let mut written: Vec<String> = Vec::new();
    let mut found = Found::No;

    for held in said.lines() {
        let bare = held.trim_start_matches('#').trim();

        match bare == line {
            true => {
                written.push(line.to_string());

                found = Found::Yes;
            }
            false => written.push(held.to_string()),
        }
    }

    match found {
        Found::Yes => {},
        Found::No => written.push(line.to_string()),
    }

    Ok(format!("{}\n", written.join("\n")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Found {
    Yes,
    No,
}

pub struct Names {
    languages: BTreeMap<String, String>,
    places: BTreeMap<String, String>,
}

fn table(said: &str, codes: &[&str]) -> Result<BTreeMap<String, String>, Never> {
    let mut names: BTreeMap<String, String> = BTreeMap::new();

    let read: serde_json::Value = match serde_json::from_str(said) {
        Ok(read) => read,
        Err(_the_list_is_not_the_shape_iso_codes_writes) => return Ok(names),
    };

    let holding = match read.as_object() {
        Some(holding) => holding,
        None => return Ok(names),
    };

    for (_, entries) in holding {
        let entries = match entries.as_array() {
            Some(entries) => entries,
            None => continue,
        };

        for entry in entries {
            let says = match entry.get("name").and_then(serde_json::Value::as_str) {
                Some(says) => says,
                None => continue,
            };

            for code in codes {
                match entry.get(code).and_then(serde_json::Value::as_str) {
                    Some(code) => {
                        names.entry(code.to_string()).or_insert_with(|| says.to_string());
                    }
                    None => {},
                }
            }
        }
    }

    Ok(names)
}

impl Names {
    pub fn read(languages: &str, places: &str) -> Result<Self, Never> {
        let Ok(languages) = table(languages, &["alpha_2", "alpha_3"]);
        let Ok(places) = table(places, &["alpha_2"]);

        Ok(Names { languages, places })
    }

    pub fn none() -> Result<Self, Never> {
        Ok(Names { languages: BTreeMap::new(), places: BTreeMap::new() })
    }

    pub fn here() -> Result<Self, Never> {
        let languages = std::fs::read_to_string(LANGUAGES);
        let places = std::fs::read_to_string(PLACES);

        match (languages, places) {
            (Ok(languages), Ok(places)) => Names::read(&languages, &places),
            (Err(_), _) | (_, Err(_)) => {
                eprintln!(
                    "settings-panel: {LANGUAGES} and {PLACES} are what a language and a country \
                     are called, and they will not be read. The rows are drawn with the codes \
                     instead, which are right and are not words anybody says."
                );

                Names::none()
            }
        }
    }

    pub fn language(&self, code: &str) -> Result<String, Never> {
        Ok(self.languages.get(code).cloned().unwrap_or_else(|| code.to_string()))
    }

    pub fn place(&self, code: &str) -> Result<String, Never> {
        Ok(self.places.get(code).cloned().unwrap_or_else(|| code.to_string()))
    }

    pub fn where_(&self, locale: &Locale) -> Result<String, Never> {
        let Ok(place) = self.place(&locale.place);

        let said = match (locale.place.is_empty(), locale.how.is_empty()) {
            (true, true) => String::new(),
            (true, false) => locale.how.clone(),
            (false, true) => place,
            (false, false) => format!("{place}, {}", locale.how),
        };

        Ok(said)
    }

    pub fn says(&self, locale: &Locale) -> Result<String, Never> {
        let Ok(language) = self.language(&locale.language);
        let Ok(where_) = self.where_(locale);

        Ok(match where_.is_empty() {
            true => language,
            false => format!("{language} ({where_})"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tongue {
    pub language: String,
    pub says: String,
    pub locales: Vec<Locale>,
}

pub fn tongues(supported: &[Locale], names: &Names) -> Result<Vec<Tongue>, Never> {
    let mut spoken: Vec<Tongue> = Vec::new();

    for locale in supported {
        let standing = spoken.iter().position(|tongue| tongue.language == locale.language);

        match standing {
            Some(at) => {
                match spoken.get_mut(at) {
                    Some(tongue) => tongue.locales.push(locale.clone()),
                    None => {},
                }
            }
            None => {
                let Ok(says) = names.language(&locale.language);

                spoken.push(Tongue {
                    language: locale.language.clone(),
                    says,
                    locales: vec![locale.clone()],
                });
            }
        }
    }

    for tongue in &mut spoken {
        tongue.locales.sort_by_key(|locale| {
            let Ok(where_) = names.where_(locale);

            where_.to_lowercase()
        });
    }

    spoken.sort_by_key(|tongue| tongue.says.to_lowercase());

    Ok(spoken)
}

pub fn standing(tongues: &[Tongue], lang: Option<&str>) -> Result<Option<Locale>, Never> {
    let said = match lang {
        Some(said) => said,
        None => return Ok(None),
    };

    let Ok(wanted) = plainly(said);

    for tongue in tongues {
        for locale in &tongue.locales {
            let Ok(lang) = locale.lang();
            let Ok(said) = plainly(&lang);

            match said == wanted {
                true => return Ok(Some(locale.clone())),
                false => {},
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "\
C.UTF-8 UTF-8
en_GB.UTF-8 UTF-8
en_US.UTF-8 UTF-8
en_US ISO-8859-1
nl_NL.UTF-8 UTF-8
nl_NL@euro ISO-8859-15
sr_RS UTF-8
sr_RS@latin UTF-8
th_TH.UTF-8 UTF-8
th_TH TIS-620
eo UTF-8";

    const NAMES: &str = r#"{"639-3":[
        {"alpha_2":"en","alpha_3":"eng","name":"English"},
        {"alpha_2":"nl","alpha_3":"nld","name":"Dutch"},
        {"alpha_2":"th","alpha_3":"tha","name":"Thai"},
        {"alpha_2":"sr","alpha_3":"srp","name":"Serbian"},
        {"alpha_2":"eo","alpha_3":"epo","name":"Esperanto"}]}"#;

    const PLACED: &str = r#"{"3166-1":[
        {"alpha_2":"GB","name":"United Kingdom"},
        {"alpha_2":"US","name":"United States"},
        {"alpha_2":"NL","name":"Netherlands"},
        {"alpha_2":"RS","name":"Serbia"},
        {"alpha_2":"TH","name":"Thailand"}]}"#;

    fn names() -> Names {
        let Ok(names) = Names::read(NAMES, PLACED);

        names
    }

    fn spoken() -> Vec<Tongue> {
        let Ok(supported) = supported(SAID);
        let Ok(tongues) = tongues(&supported, &names());

        tongues
    }

    fn of(language: &str) -> Tongue {
        spoken()
            .into_iter()
            .find(|tongue| tongue.language == language)
            .expect("a language in the list")
    }

    #[test]
    fn only_the_encoding_this_machine_draws_in_is_offered() {
        let Ok(supported) = supported(SAID);

        assert!(supported.iter().all(|locale| locale.charset == UTF8));
        assert!(!supported.iter().any(|locale| locale.name == "th_TH"));
    }

    #[test]
    fn the_one_that_is_not_a_language_is_not_in_the_list() {
        assert!(!spoken().iter().any(|tongue| tongue.language == "C"));
    }

    #[test]
    fn a_language_gathers_the_places_it_is_spoken() {
        let english = of("en");

        assert_eq!(english.says, "English");
        assert_eq!(english.locales.len(), 2);
    }

    #[test]
    fn a_language_with_nowhere_after_it_is_still_a_language() {
        let esperanto = of("eo");

        let Ok(says) = names().says(esperanto.locales.first().expect("the one locale"));

        assert_eq!(says, "Esperanto");
    }

    #[test]
    fn how_it_is_written_is_part_of_where_it_is_written() {
        let serbian = of("sr");
        let latin = serbian
            .locales
            .iter()
            .find(|locale| locale.how == "latin")
            .expect("the latin one");

        let Ok(says) = names().says(latin);

        assert_eq!(says, "Serbian (Serbia, latin)");
    }

    #[test]
    fn the_languages_are_in_the_order_somebody_would_look_for_them() {
        let says: Vec<String> = spoken().into_iter().map(|tongue| tongue.says).collect();

        assert_eq!(says, ["Dutch", "English", "Esperanto", "Serbian", "Thai"]);
    }

    #[test]
    fn a_machine_with_no_iso_codes_draws_the_codes() {
        let Ok(none) = Names::none();
        let Ok(supported) = supported(SAID);
        let Ok(tongues) = tongues(&supported, &none);

        assert!(tongues.iter().any(|tongue| tongue.says == "nl"));
    }

    #[test]
    fn a_locale_glibc_spells_without_the_encoding_still_asks_for_it_by_name() {
        let serbian = of("sr");
        let plain = serbian.locales.iter().find(|locale| locale.how.is_empty()).expect("sr_RS");

        assert_eq!(plain.line(), Ok("sr_RS UTF-8".to_string()));
        assert_eq!(plain.lang(), Ok("sr_RS.UTF-8".to_string()));
    }

    #[test]
    fn the_ones_already_made_are_the_ones_locale_a_names_however_it_spells_them() {
        let Ok(generated) = generated("C\nC.utf8\nen_US.utf8\nPOSIX\n");
        let english = of("en");
        let american =
            english.locales.iter().find(|locale| locale.place == "US").expect("en_US");
        let british =
            english.locales.iter().find(|locale| locale.place == "GB").expect("en_GB");

        assert_eq!(made(&generated, american), Ok(Made::Yes));
        assert_eq!(made(&generated, british), Ok(Made::No));
    }

    #[test]
    fn a_language_that_is_already_named_and_commented_out_is_uncommented_where_it_stands() {
        let said = "#en_GB.UTF-8 UTF-8  \n#nl_NL.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\n";
        let Ok(written) = generating(said, "nl_NL.UTF-8 UTF-8");

        assert_eq!(written, "#en_GB.UTF-8 UTF-8  \nnl_NL.UTF-8 UTF-8\nen_US.UTF-8 UTF-8\n");
    }

    #[test]
    fn a_language_the_file_has_never_heard_of_is_written_at_the_end() {
        let Ok(written) = generating("en_US.UTF-8 UTF-8\n", "th_TH.UTF-8 UTF-8");

        assert_eq!(written, "en_US.UTF-8 UTF-8\nth_TH.UTF-8 UTF-8\n");
    }

    #[test]
    fn one_already_made_is_left_exactly_as_it_was() {
        let said = "en_US.UTF-8 UTF-8\n";
        let Ok(written) = generating(said, "en_US.UTF-8 UTF-8");

        assert_eq!(written, said);
    }

    #[test]
    fn what_the_machine_is_set_to_is_read_out_of_what_localectl_says() {
        let said = "   System Locale: LANG=en_US.UTF-8\n       VC Keymap: us\n";

        assert_eq!(chosen(said), Ok(Some("en_US.UTF-8".to_string())));
        assert_eq!(chosen("System Locale: n/a"), Ok(None));
    }

    #[test]
    fn the_row_at_the_top_says_which_of_the_five_hundred_this_machine_is_on() {
        let tongues = spoken();
        let Ok(on) = standing(&tongues, Some("en_US.UTF-8"));

        assert_eq!(on.map(|locale| locale.name), Some("en_US.UTF-8".to_string()));

        let Ok(nothing) = standing(&tongues, None);

        assert_eq!(nothing, None);
    }
}
