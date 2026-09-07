//! What there is to be had, as far as one search can say.
//!
//! yt-dlp is asked for the list and nothing else: `--flat-playlist` is the
//! whole of why a search takes a second rather than a minute, because without
//! it every result on the page is opened and asked what formats it has, and a
//! panel wants ten names and ten pictures.
//!
//! So what comes back has no file sizes in it. That is not a loss: nothing here
//! asks a person to pick a format anyway, and which file is fetched is decided
//! in `getting` by a rule that never changes.


use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_u64, whole_u64};
use serde_json::{Value, json};

use crate::getting::Have;
use crate::store::Kind;

pub const MANY: usize = 10;

pub const WIDE: u64 = 200;

pub const BETWEEN: &str = " \u{00b7} ";

pub const HAVE_IT: &str = "have it";

pub const LIVE: &str = "live";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Found {
    pub id: String,
    pub title: String,
    pub url: String,
    pub by: String,
    pub seconds: u64,
    pub views: u64,
    pub live: bool,
    pub picture: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Looked {
    pub asked: String,
    pub fault: String,
    pub found: Vec<Found>,
}

pub fn target(asked: &str) -> Result<String, Never> {
    let asked = asked.trim();

    Ok(match asked.starts_with("http://") || asked.starts_with("https://") {
        true => asked.to_string(),
        false => format!("ytsearch{MANY}:{asked}"),
    })
}

pub fn search(asked: &str) -> Result<Vec<String>, Never> {
    let said = |word: &str| word.to_string();
    let Ok(target) = target(asked);
    let Ok(yt_dlp) = Program::YtDlp.name();

    Ok(vec![
        said(yt_dlp),
        said("--flat-playlist"),
        said("--dump-single-json"),
        said("--no-warnings"),
        said("--socket-timeout"),
        said("15"),
        said("--"),
        target,
    ])
}

pub fn found_in(said: &str) -> Result<Vec<Found>, Never> {
    let Ok(held) = serde_json::from_str::<Value>(said) else {
        return Ok(Vec::new());
    };

    Ok(match held.get("entries").and_then(Value::as_array) {
        Some(entries) => entries
            .iter()
            .filter_map(|entry| {
                let Ok(one) = one(entry);

                one
            })
            .collect(),
        None => {
            let Ok(one) = one(&held);

            one.into_iter().collect()
        },
    })
}

fn one(entry: &Value) -> Result<Option<Found>, Never> {
    let said = |key: &str| {
        entry.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
    };
    let counted = |key: &str| {
        let Ok(counted) =
            toward_zero_u64(entry.get(key).and_then(Value::as_f64).unwrap_or_default());

        counted
    };
    let either = |key: &str, or: &str| match said(key).is_empty() {
        true => said(or),
        false => said(key),
    };
    let id = said("id");
    let title = said("title");

    match id.is_empty() || title.is_empty() {
        true => return Ok(None),
        false => {},
    }

    let url = match either("url", "webpage_url").is_empty() {
        true => format!("https://www.youtube.com/watch?v={id}"),
        false => either("url", "webpage_url"),
    };
    let Ok(picture) = picture_in(entry);

    Ok(Some(Found {
        by: either("channel", "uploader"),
        id,
        live: said("live_status") == "is_live",
        picture,
        seconds: counted("duration"),
        title,
        url,
        views: counted("view_count"),
    }))
}

pub fn picture_in(entry: &Value) -> Result<String, Never> {
    let url = |one: &Value| one.get("url").and_then(Value::as_str).unwrap_or_default().to_string();

    let Some(many) = entry.get("thumbnails").and_then(Value::as_array) else {
        return Ok(entry.get("thumbnail").and_then(Value::as_str).unwrap_or_default().to_string());
    };

    let wide = |one: &&Value| one.get("width").and_then(Value::as_u64).unwrap_or_default();
    let big_enough = many.iter().filter(|one| wide(one) >= WIDE).min_by_key(wide);

    Ok(match big_enough {
        Some(one) => url(one),
        None => many.iter().max_by_key(|one| wide(one)).map(url).unwrap_or_default(),
    })
}

pub fn written(looked: &Looked) -> Result<String, String> {
    let entries: Vec<Value> = looked
        .found
        .iter()
        .map(|found| {
            json!({
                "id": found.id,
                "title": found.title,
                "url": found.url,
                "channel": found.by,
                "duration": found.seconds,
                "view_count": found.views,
                "live_status": match found.live {
                    true => "is_live",
                    false => "not_live",
                },
                "thumbnail": found.picture,
            })
        })
        .collect();
    let held = json!({ "asked": looked.asked, "fault": looked.fault, "entries": entries });

    serde_json::to_string_pretty(&held)
        .map_err(|fault| format!("writing down what the search found: {fault}"))
}

pub fn kept(said: &str) -> Result<Looked, Never> {
    let Ok(held) = serde_json::from_str::<Value>(said) else {
        return Ok(Looked::default());
    };

    let word = |key: &str| held.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let Ok(found) = found_in(said);

    Ok(Looked { asked: word("asked"), fault: word("fault"), found })
}

pub fn clock(seconds: u64) -> Result<String, Never> {
    match seconds {
        0 => return Ok(String::new()),
        _ => {},
    }

    let (hours, minutes, seconds) = (
        seconds.saturating_div(3600),
        seconds.wrapping_rem(3600).saturating_div(60),
        seconds.wrapping_rem(60),
    );

    Ok(match hours {
        0 => format!("{minutes}:{seconds:02}"),
        _ => format!("{hours}:{minutes:02}:{seconds:02}"),
    })
}

pub fn counted(views: u64) -> Result<String, Never> {
    let said = |many: f64, what: &str| {
        let Ok(whole) = whole_u64(many);

        match many < 10.0 {
            true => format!("{many:.1} {what} times"),
            false => format!("{whole} {what} times"),
        }
    };
    let Ok(many) = views.float();

    Ok(match views {
        0 => String::new(),
        views if views >= 1_000_000_000 => said(many / 1e9, "billion"),
        views if views >= 1_000_000 => said(many / 1e6, "million"),
        views if views >= 1_000 => format!("{} thousand times", views.saturating_div(1_000)),
        views => format!("{views} times"),
    })
}


pub fn complaint(said: &str) -> Result<String, Never> {
    let last = said.lines().map(str::trim).rfind(|line| !line.is_empty());
    let said = last.unwrap_or(WENT_WRONG).trim_start_matches("ERROR:").trim();

    Ok(match said.char_indices().nth(SHORT).and_then(|(at, _)| said.get(..at)) {
        Some(head) => format!("{head}\u{2026}"),
        None => said.to_string(),
    })
}

pub const WENT_WRONG: &str = "The search would not run";

pub const NO_YT_DLP: &str = "There is no yt-dlp on this machine to look with";

pub const SHORT: usize = 90;

pub fn aside(kind: Kind, found: &Found, have: Have) -> Result<String, Never> {
    let when = match found.live {
        true => LIVE.to_string(),
        false => {
            let Ok(clock) = clock(found.seconds);

            clock
        },
    };
    let Ok(said) = match kind {
        Kind::Sound => joined(&[&found.by, &when]),
        Kind::Film => {
            let Ok(counted) = counted(found.views);

            joined(&[&when, &counted])
        },
    };

    match have {
        Have::It => joined(&[&said, HAVE_IT]),
        Have::Not => Ok(said),
    }
}

fn joined(words: &[&str]) -> Result<String, Never> {
    let said: Vec<&str> = words.iter().copied().filter(|word| !word.is_empty()).collect();
    Ok(said.join(BETWEEN))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(asked: &str) -> String {
        let Ok(target) = super::target(asked);

        target
    }

    fn search(asked: &str) -> Vec<String> {
        let Ok(search) = super::search(asked);

        search
    }

    fn found_in(said: &str) -> Vec<Found> {
        let Ok(found) = super::found_in(said);

        found
    }

    fn kept(said: &str) -> Looked {
        let Ok(kept) = super::kept(said);

        kept
    }

    fn clock(seconds: u64) -> String {
        let Ok(clock) = super::clock(seconds);

        clock
    }

    fn counted(views: u64) -> String {
        let Ok(counted) = super::counted(views);

        counted
    }

    fn complaint(said: &str) -> String {
        let Ok(complaint) = super::complaint(said);

        complaint
    }

    fn aside(kind: Kind, found: &Found, have: Have) -> String {
        let Ok(aside) = super::aside(kind, found, have);

        aside
    }

    const SAID: &str = r#"{
        "entries": [
            {
                "id": "FTQbiNvZqaY",
                "title": "Toto - Africa (Official HD Video)",
                "url": "https://www.youtube.com/watch?v=FTQbiNvZqaY",
                "duration": 272,
                "channel": "TOTO",
                "view_count": 1288575953,
                "live_status": null,
                "thumbnails": [
                    {"url": "https://i.ytimg.com/vi/FTQbiNvZqaY/small.jpg", "width": 360},
                    {"url": "https://i.ytimg.com/vi/FTQbiNvZqaY/large.jpg", "width": 720}
                ]
            },
            {
                "id": "",
                "title": "half an answer"
            }
        ]
    }"#;

    fn africa() -> Found {
        found_in(SAID).first().cloned().expect("the first thing found")
    }

    #[test]
    fn what_a_search_answers_becomes_things_to_choose_from() {
        let found = found_in(SAID);

        assert_eq!(found.len(), 1, "an entry with no id is not a row");
        assert_eq!(found[0].title, "Toto - Africa (Official HD Video)");
        assert_eq!(found[0].by, "TOTO");
        assert_eq!(found[0].seconds, 272);
        assert!(!found[0].live);
    }

    #[test]
    fn the_picture_taken_is_the_smallest_one_still_worth_drawing() {
        assert!(africa().picture.ends_with("small.jpg"));
    }

    #[test]
    fn a_link_is_looked_at_and_words_are_looked_for() {
        assert_eq!(target("https://youtu.be/abc"), "https://youtu.be/abc");
        assert_eq!(target("  toto africa "), format!("ytsearch{MANY}:toto africa"));
        let Ok(yt_dlp) = Program::YtDlp.name();

        assert_eq!(search("toto")[0], yt_dlp);
    }

    #[test]
    fn a_search_written_down_is_the_same_search_read_back() {
        let looked = Looked {
            asked: "toto africa".to_string(),
            fault: String::new(),
            found: found_in(SAID),
        };
        let again = kept(&written(&looked).expect("a search this program built writes down"));

        assert_eq!(again.asked, looked.asked);
        assert_eq!(again.found, looked.found);
    }

    #[test]
    fn what_went_wrong_is_kept_with_the_search_that_went_wrong() {
        let looked = Looked {
            asked: "toto".to_string(),
            fault: "no network".to_string(),
            found: Vec::new(),
        };
        let said = written(&looked).expect("a search this program built writes down");
        assert_eq!(kept(&said).fault, "no network");
    }

    #[test]
    fn a_length_is_said_the_way_a_clock_says_it() {
        assert_eq!(clock(272), "4:32");
        assert_eq!(clock(59), "0:59");
        assert_eq!(clock(3725), "1:02:05");
        assert_eq!(clock(0), "");
    }

    #[test]
    fn how_many_have_watched_it_is_said_in_words() {
        assert_eq!(counted(1_288_575_953), "1.3 billion times");
        assert_eq!(counted(21_150_346), "21 million times");
        assert_eq!(counted(4_100), "4 thousand times");
        assert_eq!(counted(0), "");
    }

    #[test]
    fn each_tab_says_the_thing_its_own_list_is_chosen_by() {
        let found = africa();
        assert_eq!(aside(Kind::Sound, &found, Have::Not), "TOTO \u{00b7} 4:32");
        assert_eq!(aside(Kind::Film, &found, Have::Not), "4:32 \u{00b7} 1.3 billion times");
    }

    #[test]
    fn a_thing_already_in_the_folder_says_so() {
        assert!(aside(Kind::Sound, &africa(), Have::It).ends_with(HAVE_IT));
    }

    #[test]
    fn a_thing_still_happening_has_no_length_and_says_that_instead() {
        let live = Found { live: true, ..africa() };
        assert!(aside(Kind::Film, &live, Have::Not).starts_with(LIVE));
    }

    #[test]
    fn a_complaint_is_cut_down_to_the_line_that_says_why() {
        let said = "[youtube] tried\nERROR: Unable to download webpage: timed out\n";
        assert_eq!(complaint(said), "Unable to download webpage: timed out");
        assert_eq!(complaint("   "), WENT_WRONG);
        assert!(complaint(&"x".repeat(400)).chars().count() <= SHORT + 1);
    }
}
