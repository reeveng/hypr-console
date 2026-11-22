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

use std::time::Duration;

use console_core_external_programs::Program;
use console_core_localization::positional;
use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_u64, whole_u64};
use serde_json::{Value, json};

use crate::getting::Have;
use crate::store::Kind;

pub const MANY: u32 = 10;

pub const WIDE: u64 = 200;

pub const BETWEEN: &str = " \u{00b7} ";

pub const HAVE_IT: &str = "downloaded";

pub const LIVE: &str = "live";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveStatus {
  Live,
  #[default]
  NotLive,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Found {
  pub id: String,
  pub title: String,
  pub url: String,
  pub by: String,
  pub seconds: u64,
  pub views: u64,
  pub live: LiveStatus,
  pub picture: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Looked {
  pub asked: String,
  pub fault: String,
  pub found: Vec<Found>,
}

pub fn target(asked: &str) -> Result<String, Never> {
  let asked_2 = asked.trim();

  Ok(
    match ::core::primitive::bool::then_some(asked_2.starts_with("http://") || asked_2.starts_with("https://"), ()) {
      ::core::option::Option::Some(()) => asked_2.to_string(),
      ::core::option::Option::None => format!("ytsearch{MANY}:{asked}", asked = asked_2),
    },
  )
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

#[cfg_attr(
  dylint_lib = "explicit098_no_walked_json",
  expect(
    explicit098_no_walked_json,
    reason = "yt-dlp supplies an open metadata dictionary. This bounded decoder projects only the supported keys and validates each one before constructing a Found value"
  )
)]
fn reply(said: &str) -> Result<Option<Value>, Never> {
  let Ok(bytes) = console_core_number_conversion::fitted::<_, u64>(said.len());
  let ceiling = u64::from(console_core_atomic_writes::MAX_TEXT_BYTES);

  match (bytes > ceiling).then_some(()) {
    Some(()) => {
      console_core_logging::warn!("downloads: search reply is larger than {ceiling} bytes");

      return Ok(None);
    }
    None => {}
  }

  Ok(match serde_json::from_str(said) {
    Ok(value) => Some(value),
    Err(fault) => {
      console_core_logging::debug!("downloads: reading a search reply: {fault}");

      None
    }
  })
}

fn found_videos(held: &Value) -> Result<Vec<Found>, Never> {
  Ok(match held.get("entries").and_then(Value::as_array) {
    Some(entries) => entries
      .iter()
      .filter_map(|entry| {
        let Ok(found) = one(entry);

        found
      })
      .collect(),
    None => {
      let Ok(found) = one(held);

      found.into_iter().collect()
    }
  })
}

pub fn found_in(said: &str) -> Result<Vec<Found>, Never> {
  let Ok(held) = reply(said);

  match held {
    Some(value) => found_videos(&value),
    None => Ok(Vec::new()),
  }
}

fn first_word_in(entry: &Value, [preferred, fallback]: [&str; 2]) -> Result<String, Never> {
  let Ok(word) = word_in(entry, preferred);

  match word.is_empty().then_some(()) {
    Some(()) => word_in(entry, fallback),
    None => Ok(word),
  }
}

fn one(entry: &Value) -> Result<Option<Found>, Never> {
  let Ok(id) = word_in(entry, "id");
  let Ok(title) = word_in(entry, "title");

  match (id.is_empty() || title.is_empty()).then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let Ok(link) = first_word_in(entry, ["url", "webpage_url"]);
  let url = match link.is_empty().then_some(()) {
    Some(()) => format!("https://www.youtube.com/watch?v={id}"),
    None => link,
  };
  let Ok(duration) = measured_in(entry, "duration");
  let Ok(view_count) = measured_in(entry, "view_count");
  let Ok(seconds) = toward_zero_u64(duration);
  let Ok(views) = toward_zero_u64(view_count);
  let Ok(picture) = picture_in(entry);
  let Ok(by) = first_word_in(entry, ["channel", "uploader"]);
  let live = match entry.get("live_status").and_then(Value::as_str) {
    Some("is_live") => LiveStatus::Live,
    Some(_) | None => LiveStatus::NotLive,
  };

  Ok(Some(Found { id, title, url, by, seconds, views, live, picture }))
}

pub const SAID_NOTHING: &str = "";

pub const COUNTED_NOTHING: u64 = 0;

pub const MEASURED_NOTHING: f64 = 0.0;

pub fn word_in(held: &Value, key: &str) -> Result<String, Never> {
  Ok(match held.get(key).and_then(Value::as_str) {
    Some(said) => said.to_string(),
    None => SAID_NOTHING.to_string(),
  })
}

pub fn measured_in(held: &Value, key: &str) -> Result<f64, Never> {
  Ok(match held.get(key).and_then(Value::as_f64) {
    Some(measured) => measured,
    None => MEASURED_NOTHING,
  })
}

pub fn counted_in(held: &Value, key: &str) -> Result<u64, Never> {
  Ok(match held.get(key).and_then(Value::as_u64) {
    Some(counted) => counted,
    None => COUNTED_NOTHING,
  })
}

pub fn picture_in(entry: &Value) -> Result<String, Never> {
  let url = |one: &Value| {
    let Ok(url) = word_in(one, "url");

    url
  };

  let many = match entry.get("thumbnails").and_then(Value::as_array) {
    Some(many) => many,
    None => return word_in(entry, "thumbnail"),
  };

  let wide = |one: &&Value| {
    let Ok(wide) = counted_in(one, "width");

    wide
  };
  let big_enough = many.iter().filter(|one| wide(one) >= WIDE).min_by_key(wide);

  Ok(match big_enough {
    Some(one) => url(one),
    None => match many.iter().max_by_key(|one| wide(one)).map(url) {
      Some(url_2) => url_2,
      None => SAID_NOTHING.to_string(),
    },
  })
}

#[derive(Debug)]
pub struct Unwritten(pub serde_json::Error);

impl std::fmt::Display for Unwritten {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(to, "writing down what the search found: {}", self.0)
  }
}

impl std::error::Error for Unwritten {}

pub fn serialize(looked: &Looked) -> Result<String, Unwritten> {
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
              LiveStatus::Live => "is_live",
              LiveStatus::NotLive => "not_live",
          },
          "thumbnail": found.picture,
      })
    })
    .collect();
  let held = json!({ "asked": looked.asked, "fault": looked.fault, "entries": entries });

  serde_json::to_string_pretty(&held).map_err(Unwritten)
}

pub fn parse(said: &str) -> Result<Looked, Never> {
  let Ok(held) = reply(said);
  let reply = match held {
    Some(value) => value,
    None => return Ok(Looked::default()),
  };
  let Ok(found) = found_videos(&reply);
  let Ok(asked) = word_in(&reply, "asked");
  let Ok(fault) = word_in(&reply, "fault");

  Ok(Looked { asked, fault, found })
}

pub fn counted(views: u64) -> Result<String, Never> {
  let said = |many: f64, what: &str| {
    let Ok(whole) = whole_u64(many);

    match ::core::primitive::bool::then_some(many < 10.0, ()) {
      ::core::option::Option::Some(()) => format!("{many:.1}{what} views"),
      ::core::option::Option::None => format!("{whole}{what} views"),
    }
  };
  let Ok(many) = views.float();

  Ok(match views {
    0 => String::new(),
    1..1_000 => format!("{views} views"),
    1_000..1_000_000 => format!("{}K views", views.saturating_div(1_000)),
    1_000_000..1_000_000_000 => said(many / 1e6, "M"),
    1_000_000_000.. => said(many / 1e9, "B"),
  })
}

pub fn complaint(said: &str) -> Result<String, Never> {
  let last = said.lines().map(str::trim).rfind(|line| !line.is_empty());

  let complained = match last {
    Some(complained) => complained,
    None => WENT_WRONG,
  };

  let said_2 = complained.trim_start_matches("ERROR:").trim();

  let Ok(short) = console_core_number_conversion::index(SHORT);

  Ok(
    match said_2
      .char_indices()
      .nth(short)
      .and_then(|(at, _)| said_2.get(..at))
    {
      Some(head) => format!("{head}\u{2026}"),
      None => said_2.to_string(),
    },
  )
}

pub const WENT_WRONG: &str = "Search failed";

pub const NO_YT_DLP: &str = "yt-dlp isn't installed";

pub const NO_CURL: &str = "curl isn't installed";

pub fn missing_tool_message(kind: Kind) -> Result<&'static str, Never> {
  Ok(match kind {
    Kind::Sound | Kind::Film => NO_YT_DLP,
    Kind::Book => NO_CURL,
  })
}

pub const SHORT: u32 = 90;

pub fn aside(kind: Kind, found: &Found, have: Have) -> Result<String, Never> {
  let when = match (found.live, found.seconds) {
    (LiveStatus::Live, _) => LIVE.to_string(),
    (LiveStatus::NotLive, 0) => String::new(),
    (LiveStatus::NotLive, seconds) => {
      let Ok(clock) = positional(Duration::from_secs(seconds));

      clock
    }
  };
  let Ok(said) = match kind {
    Kind::Sound => joined(&[&found.by, &when]),
    Kind::Film => {
      let Ok(counted) = counted(found.views);

      joined(&[&when, &counted])
    }
    Kind::Book => joined(&[&found.by]),
  };

  match have {
    Have::It => joined(&[&said, HAVE_IT]),
    Have::Not => Ok(said),
  }
}

fn joined(words: &[&str]) -> Result<String, Never> {
  let said: Vec<&str> = words
    .iter()
    .copied()
    .filter(|word| !word.is_empty())
    .collect();
  Ok(said.join(BETWEEN))
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_number_conversion::fitted;

  type Failure = Box<dyn std::error::Error>;

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

  fn africa() -> Result<Found, Failure> {
    let Ok(found) = found_in(SAID);
    let first = found.first().cloned().ok_or("nothing found")?;

    Ok(first)
  }

  #[test]
  fn what_a_search_answers_becomes_things_to_choose_from() -> Result<(), Failure> {
    let Ok(found) = found_in(SAID);
    let first = found.first().ok_or("nothing found")?;

    assert_eq!(found.len(), 1, "an entry with no id is not a row");
    assert_eq!(first.title, "Toto - Africa (Official HD Video)");
    assert_eq!(first.by, "TOTO");
    assert_eq!(first.seconds, 272);
    assert_eq!(first.live, LiveStatus::NotLive);

    Ok(())
  }

  #[test]
  fn the_picture_taken_is_the_smallest_one_still_worth_drawing() -> Result<(), Failure> {
    let africa = africa()?;

    assert!(africa.picture.ends_with("small.jpg"));

    Ok(())
  }

  #[test]
  fn a_link_is_looked_at_and_words_are_looked_for() {
    let Ok(yt_dlp) = Program::YtDlp.name();
    let Ok(search) = search("toto");

    assert_eq!(
      target("https://youtu.be/abc"),
      Ok("https://youtu.be/abc".to_string())
    );
    assert_eq!(
      target("  toto africa "),
      Ok(format!("ytsearch{MANY}:toto africa"))
    );
    assert_eq!(search.first().map(String::as_str), Some(yt_dlp));
  }

  #[test]
  fn a_search_written_down_is_the_same_search_read_back() -> Result<(), Failure> {
    let Ok(found) = found_in(SAID);
    let looked = Looked {
      asked: "toto africa".to_string(),
      fault: String::new(),
      found,
    };
    let said = serialize(&looked)?;
    let Ok(again) = parse(&said);

    assert_eq!(again.asked, looked.asked);
    assert_eq!(again.found, looked.found);

    Ok(())
  }

  #[test]
  fn what_went_wrong_is_kept_with_the_search_that_went_wrong() -> Result<(), Failure> {
    let looked = Looked {
      asked: "toto".to_string(),
      fault: "no network".to_string(),
      found: Vec::new(),
    };
    let said = serialize(&looked)?;
    let Ok(kept) = parse(&said);

    assert_eq!(kept.fault, "no network");

    Ok(())
  }

  #[test]
  fn a_length_nobody_said_is_left_out_rather_than_said_as_nothing() -> Result<(), Failure> {
    let africa = africa()?;
    let unsaid = Found {
      seconds: 0,
      ..africa
    };

    assert_eq!(
      aside(Kind::Sound, &unsaid, Have::Not),
      Ok(String::from("TOTO"))
    );
    assert_eq!(
      aside(Kind::Film, &unsaid, Have::Not),
      Ok(String::from("1.3B views"))
    );

    Ok(())
  }

  #[test]
  fn how_many_have_watched_it_is_said_in_words() {
    assert_eq!(counted(1_288_575_953), Ok("1.3B views".to_string()));
    assert_eq!(counted(21_150_346), Ok("21M views".to_string()));
    assert_eq!(counted(4_100), Ok("4K views".to_string()));
    assert_eq!(counted(0), Ok("".to_string()));
  }

  #[test]
  fn each_tab_says_the_thing_its_own_list_is_chosen_by() -> Result<(), Failure> {
    let found = africa()?;

    assert_eq!(
      aside(Kind::Sound, &found, Have::Not),
      Ok("TOTO \u{00b7} 4:32".to_string())
    );
    assert_eq!(
      aside(Kind::Film, &found, Have::Not),
      Ok("4:32 \u{00b7} 1.3B views".to_string())
    );

    Ok(())
  }

  #[test]
  fn a_thing_already_in_the_folder_says_so() -> Result<(), Failure> {
    let africa = africa()?;
    let Ok(aside) = aside(Kind::Sound, &africa, Have::It);

    assert!(aside.ends_with(HAVE_IT));

    Ok(())
  }

  #[test]
  fn a_thing_still_happening_has_no_length_and_says_that_instead() -> Result<(), Failure> {
    let africa = africa()?;
    let live = Found {
      live: LiveStatus::Live,
      ..africa
    };
    let Ok(aside) = aside(Kind::Film, &live, Have::Not);

    assert!(aside.starts_with(LIVE));

    Ok(())
  }

  #[test]
  fn a_complaint_is_cut_down_to_the_line_that_says_why() {
    let said = "[youtube] tried\nERROR: Unable to download webpage: timed out\n";
    let Ok(long) = complaint(&"x".repeat(400));
    let Ok(letters) = fitted::<_, u32>(long.chars().count());

    assert_eq!(
      complaint(said),
      Ok("Unable to download webpage: timed out".to_string())
    );
    assert_eq!(complaint("   "), Ok(WENT_WRONG.to_string()));
    assert!(letters <= SHORT.saturating_add(1));
  }
}
