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

use serde_json::{Value, json};

use crate::store::Kind;

/// How many things one search is worth.
///
/// Ten, because they are walked with a thumb. A list of fifty is a list nobody
/// reaches the end of, and the thing anybody was looking for is almost always
/// in the first few: a search that has to go deeper than ten is one worth
/// typing again with another word.
pub const MANY: usize = 10;

/// The smallest picture of a thing that is still worth having.
///
/// A row draws one 32 points across, so the biggest a site offers is a
/// megabyte fetched over somebody's phone tether to be thrown away by the
/// scaler. This is the width below which the picture starts to look like a
/// mistake once it is drawn.
pub const WIDE: u64 = 200;

/// What is written between the two or three things said beside a row.
pub const BETWEEN: &str = " \u{00b7} ";

/// What a row says when the folder it would land in already holds it.
pub const HAVE_IT: &str = "have it";

/// What stands where the length of a thing would be, when it has no length
/// because it has not finished happening.
pub const LIVE: &str = "live";

/// One thing a search turned up.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Found {
    pub id: String,
    pub title: String,
    pub url: String,
    /// Whose it is: the channel, or whoever uploaded it.
    pub by: String,
    pub seconds: u64,
    pub views: u64,
    pub live: bool,
    /// Where the picture of it is on the site, before anything has fetched it.
    pub picture: String,
}

/// What one search came to: what was asked, and what came back.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Looked {
    pub asked: String,
    /// What went wrong, where something did. Drawn as a row, because a search
    /// that answers nothing and says nothing is a panel that looks broken.
    pub fault: String,
    pub found: Vec<Found>,
}

/// What is looked at: a link, or a search for words.
///
/// A link is worth taking because the browser is the other way anybody arrives
/// at one of these, and pasting one in is cheaper than typing a title with a
/// thumb.
pub fn target(asked: &str) -> String {
    let asked = asked.trim();
    match asked.starts_with("http://") || asked.starts_with("https://") {
        true => asked.to_string(),
        false => format!("ytsearch{MANY}:{asked}"),
    }
}

/// The words that ask for the list.
pub fn search(asked: &str) -> Vec<String> {
    let said = |word: &str| word.to_string();
    vec![
        said("yt-dlp"),
        // The names and the pictures, and no question to the site about any
        // one of them.
        said("--flat-playlist"),
        said("--dump-single-json"),
        said("--no-warnings"),
        // A tether that has gone is a search that hangs, and the row above it
        // says "Looking" for as long as it does.
        said("--socket-timeout"),
        said("15"),
        said("--"),
        target(asked),
    ]
}

/// What yt-dlp said, as things to choose from.
///
/// The same reader serves the file this desktop writes down, because that file
/// is written in the shape yt-dlp answers in. One shape, one reader, and no
/// second opinion about what a result is.
pub fn found_in(said: &str) -> Vec<Found> {
    let Ok(held) = serde_json::from_str::<Value>(said) else {
        return Vec::new();
    };
    match held.get("entries").and_then(Value::as_array) {
        Some(entries) => entries.iter().filter_map(one).collect(),
        // A link rather than a search: one thing, and it is the whole answer.
        None => one(&held).into_iter().collect(),
    }
}

/// One entry, if it is enough of one to draw.
fn one(entry: &Value) -> Option<Found> {
    let said = |key: &str| {
        entry.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
    };
    let counted = |key: &str| entry.get(key).and_then(Value::as_f64).unwrap_or_default() as u64;
    let either = |key: &str, or: &str| match said(key).is_empty() {
        true => said(or),
        false => said(key),
    };
    let id = said("id");
    let title = said("title");
    if id.is_empty() || title.is_empty() {
        return None;
    }
    let url = match either("url", "webpage_url").is_empty() {
        true => format!("https://www.youtube.com/watch?v={id}"),
        false => either("url", "webpage_url"),
    };
    Some(Found {
        by: either("channel", "uploader"),
        id,
        live: said("live_status") == "is_live",
        picture: picture_in(entry),
        seconds: counted("duration"),
        title,
        url,
        views: counted("view_count"),
    })
}

/// The smallest picture of a thing that is still worth drawing.
pub fn picture_in(entry: &Value) -> String {
    let url = |one: &Value| one.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
    let Some(many) = entry.get("thumbnails").and_then(Value::as_array) else {
        return entry.get("thumbnail").and_then(Value::as_str).unwrap_or_default().to_string();
    };
    let wide = |one: &&Value| one.get("width").and_then(Value::as_u64).unwrap_or_default();
    let big_enough = many.iter().filter(|one| wide(one) >= WIDE).min_by_key(wide);
    match big_enough {
        Some(one) => url(one),
        // Everything on offer is small, so the largest of them is the least
        // bad. A row with room kept and nothing in it is worse than a picture
        // that is slightly soft.
        None => many.iter().max_by_key(|one| wide(one)).map(url).unwrap_or_default(),
    }
}

/// A search, written down the way it is read back.
pub fn written(looked: &Looked) -> String {
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
    serde_json::to_string_pretty(&held).unwrap_or_default()
}

/// What was written down, read back.
pub fn kept(said: &str) -> Looked {
    let held: Value = serde_json::from_str(said).unwrap_or(Value::Null);
    let word = |key: &str| held.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    Looked { asked: word("asked"), fault: word("fault"), found: found_in(said) }
}

/// How long a thing is, as a clock says it.
pub fn clock(seconds: u64) -> String {
    if seconds == 0 {
        return String::new();
    }
    let (hours, minutes, seconds) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    match hours {
        0 => format!("{minutes}:{seconds:02}"),
        _ => format!("{hours}:{minutes:02}:{seconds:02}"),
    }
}

/// How many have watched it, in the words a person uses for a number that big.
///
/// Not the number itself. Nobody reads 1288575953, and the only thing it is
/// worth on a row is whether this is the one everybody means or somebody's
/// upload of it.
pub fn counted(views: u64) -> String {
    let said = |many: f64, what: &str| match many < 10.0 {
        true => format!("{many:.1} {what} times"),
        false => format!("{} {what} times", many.round() as u64),
    };
    match views {
        0 => String::new(),
        views if views >= 1_000_000_000 => said(views as f64 / 1e9, "billion"),
        views if views >= 1_000_000 => said(views as f64 / 1e6, "million"),
        views if views >= 1_000 => format!("{} thousand times", views / 1_000),
        views => format!("{views} times"),
    }
}


/// What went wrong, in one line worth putting on a row.
///
/// The last thing yt-dlp complained about rather than the first: what it says
/// first is usually which extractor it tried, and what it says last is why it
/// gave up. Its own word for a fault is dropped, because a row on a card is
/// already the place faults are said, and cut short, because a row is one line
/// and a stack trace pushed the list off the screen.
pub fn complaint(said: &str) -> String {
    let last = said.lines().map(str::trim).rfind(|line| !line.is_empty());
    let said = last.unwrap_or(WENT_WRONG).trim_start_matches("ERROR:").trim();
    match said.char_indices().nth(SHORT) {
        Some((at, _)) => format!("{}\u{2026}", &said[..at]),
        None => said.to_string(),
    }
}

/// What is said when nothing said anything.
pub const WENT_WRONG: &str = "The search would not run";

/// What there is to say when the thing that does the fetching is not there.
pub const NO_YT_DLP: &str = "There is no yt-dlp on this machine to look with";

/// How much of a complaint is worth a row.
pub const SHORT: usize = 90;

/// What is written beside one thing.
///
/// The whole of the difference between the two previews. A song is looked for
/// by name and chosen by whose it is, so the Audio tab says who it is by; a
/// video is chosen by whether it is the one everybody means, so the Video tab
/// says how many have watched it.
pub fn aside(kind: Kind, found: &Found, have: bool) -> String {
    let when = match found.live {
        true => LIVE.to_string(),
        false => clock(found.seconds),
    };
    let said = match kind {
        Kind::Sound => joined(&[&found.by, &when]),
        Kind::Film => joined(&[&when, &counted(found.views)]),
    };
    match have {
        true => joined(&[&said, HAVE_IT]),
        false => said,
    }
}

fn joined(words: &[&str]) -> String {
    let said: Vec<&str> = words.iter().copied().filter(|word| !word.is_empty()).collect();
    said.join(BETWEEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One page of what yt-dlp answers a search with, cut to the keys anything
    /// here reads.
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

    /// The row is 32 points across. The largest picture on offer is a megabyte
    /// fetched to be thrown away by the scaler.
    #[test]
    fn the_picture_taken_is_the_smallest_one_still_worth_drawing() {
        assert!(africa().picture.ends_with("small.jpg"));
    }

    #[test]
    fn a_link_is_looked_at_and_words_are_looked_for() {
        assert_eq!(target("https://youtu.be/abc"), "https://youtu.be/abc");
        assert_eq!(target("  toto africa "), format!("ytsearch{MANY}:toto africa"));
        assert_eq!(search("toto")[0], "yt-dlp");
    }

    /// The file this desktop keeps is written in the shape yt-dlp answers in,
    /// so one reader serves both and neither can drift from the other.
    #[test]
    fn a_search_written_down_is_the_same_search_read_back() {
        let looked = Looked {
            asked: "toto africa".to_string(),
            fault: String::new(),
            found: found_in(SAID),
        };
        let again = kept(&written(&looked));
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
        assert_eq!(kept(&written(&looked)).fault, "no network");
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

    /// The two tabs are one search and two previews, which is the whole reason
    /// they are tabs rather than a switch.
    #[test]
    fn each_tab_says_the_thing_its_own_list_is_chosen_by() {
        let found = africa();
        assert_eq!(aside(Kind::Sound, &found, false), "TOTO \u{00b7} 4:32");
        assert_eq!(aside(Kind::Film, &found, false), "4:32 \u{00b7} 1.3 billion times");
    }

    /// The one thing worth knowing before pressing A, said where the length is.
    #[test]
    fn a_thing_already_in_the_folder_says_so() {
        assert!(aside(Kind::Sound, &africa(), true).ends_with(HAVE_IT));
    }

    #[test]
    fn a_thing_still_happening_has_no_length_and_says_that_instead() {
        let live = Found { live: true, ..africa() };
        assert!(aside(Kind::Film, &live, false).starts_with(LIVE));
    }

    #[test]
    fn a_complaint_is_cut_down_to_the_line_that_says_why() {
        let said = "[youtube] tried\nERROR: Unable to download webpage: timed out\n";
        assert_eq!(complaint(said), "Unable to download webpage: timed out");
        assert_eq!(complaint("   "), WENT_WRONG);
        assert!(complaint(&"x".repeat(400)).chars().count() <= SHORT + 1);
    }
}
