//! Music, video and books off the net, drawn.
//!
//! ```text
//!     downloads
//!     downloads Video
//!     downloads Books
//! ```
//!
//! Two tabs over one search. What is typed is the same question either way, and
//! the tab decides what is asked for out of the answer: the Audio tab puts the
//! sound of a thing into the folder the music player reads, the Video tab puts
//! the whole of it into Videos. That is also why the two lists are drawn
//! differently -- a song is chosen by whose it is and a video by whether it is
//! the one everyone means -- and why they are two tabs of one app rather
//! than two programs.
//!
//! Nothing slow happens here. Looking is `downloads-find` and fetching is
//! `downloads-get`. The card owns search lifetimes and draws accepted replies.
//! Fetches use the panel's tasks, so a thing being fetched says so on its own row
//! for as long as `downloads-get` runs, so a second press of it is a press on a
//! row with nothing to press rather than a second download of the same file.
//!
//! What is here is the machine: the folders, the pictures on the disk, and
//! whether a thing has already arrived. Where each tab is standing and what a
//! press does about it is `crate::standing`, which is a
//! `console_core_state_machine::Machine`. The actor is what makes that state
//! reachable from a closure on GTK's thread; a press is handed to it with
//! `send`, and the effects it answers with are carried out in the callback
//! holding the surface.

use std::path::Path;

use crate::getting;
use crate::looking::{self, Found, Looked};
use crate::rows::{self, DOWNLOADING};
use crate::searches::Searches;
use crate::standing::{
  Closes, Destination, Downloads, DownloadsEffect, DownloadsEvent, Standing, Tab, closes,
  fetched_by,
};
use crate::store::{self, Kind};
use console_actor::Actor;
use console_core_never::Never;
use console_panel::card::Card;
use console_panel::page::{ActionOutcome, Aside, Handler, Page, Picture, Row, Rows, Showing};
use console_program_contract::{Effect, Event};

type Panel = Actor<Downloads>;

fn at(held: &Panel, tab: u32) -> Result<Tab, Never> {
  Ok(match held.get() {
    Ok(standing) => {
      let Ok(at) = standing.at(tab);

      at
    }
    Err(_the_actor_has_gone) => {
      console_core_logging::warn!(
        "downloads: the panel's own state is missing, so it drew as it opened"
      );

      Tab::default()
    }
  })
}

fn decided(held: &Panel, heard: DownloadsEvent) -> Result<Vec<Effect<DownloadsEffect>>, Never> {
  Ok(match held.send(Event::Custom(heard)) {
    Ok(effects) => effects,
    Err(_the_actor_has_gone) => {
      console_core_logging::warn!(
        "downloads: the panel's own state is missing, so the press did nothing"
      );

      Vec::new()
    }
  })
}

fn press(held: &Panel, heard: DownloadsEvent, showing: &dyn Showing) -> Result<(), Never> {
  let Ok(effects) = decided(held, heard);

  for effect in &effects {
    let Ok(()) = carry(effect, showing);
  }

  Ok(())
}

fn carry(effect: &Effect<DownloadsEffect>, showing: &dyn Showing) -> Result<(), Never> {
  match effect {
    Effect::Custom(DownloadsEffect::Replace(row)) => {
      let Ok(row_2) = console_core_number_conversion::fitted(*row);

      showing.replace(row_2)
    }
    Effect::Custom(DownloadsEffect::Refresh) => showing.refresh(),
    Effect::Custom(DownloadsEffect::Notify(said)) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Downloads"),
        said,
        None,
      );
    }
    Effect::Custom(DownloadsEffect::ForgetTyping) => showing.forget_typing(),

    Effect::Custom(
      DownloadsEffect::Search { .. }
      | DownloadsEffect::Cancel { .. }
      | DownloadsEffect::Retain(_),
    ) => {}

    Effect::Run(runs) => {
      let Ok(task) = console_panel::tasks::Task::of(runs.clone());

      showing.run(task);
    }

    Effect::Stream(_)
    | Effect::Fork(_)
    | Effect::Interrupt(_)
    | Effect::Prompt(_)
    | Effect::Spawn(_)
    | Effect::Subscribe(_)
    | Effect::Unsubscribe(_)
    | Effect::Write(_)
    | Effect::Notify(_)
    | Effect::Print(_)
    | Effect::Stop(_) => {}
  }

  Ok(())
}

fn looked(kind: Kind, result: Option<Looked>) -> Result<Looked, Never> {
  match result {
    Some(result_2) => {
      return Ok(result_2);
    }
    ::core::option::Option::None => {}
  }

  let Ok(cache) = store::cache();

  let cache_3 = match cache {
    Some(cache_2) => cache_2,
    None => return Ok(Looked::default()),
  };

  let Ok(at) = store::found_at(&cache_3, kind);

  let said = match console_core_atomic_writes::read_text(&at) {
    Ok(said) => said,
    Err(_unreadable) => return Ok(Looked::default()),
  };

  looking::parse(&said)
}

fn folder(kind: Kind) -> Result<String, Never> {
  let Ok(into) = getting::into(kind);

  Ok(match into.file_name() {
    Some(named) => match named.to_str() {
      Some(text) => text.to_string(),
      None => format!("{named:?}"),
    },
    None => into.display().to_string(),
  })
}

fn rows_of(held: &Panel, tab: u32, kind: Kind, searches: &Searches) -> Result<Vec<Row>, Never> {
  let Ok(Tab {
    typed,
    asking,
    onto,
    result,
    ..
  }) = at(held, tab);

  match onto {
    Destination::Ways { found, .. } => ways(held, tab, kind, &found),
    Destination::List => {
      let Ok(looked) = looked(kind, result);
      let Ok(cache) = store::cache();

      let cache_3 = match cache {
        Some(cache_2) => cache_2,
        None => return Ok(Vec::new()),
      };

      let Ok(into) = getting::into(kind);
      let Ok(looking_for) = looking_for(held, tab, kind, searches);

      let Ok(rows) = rows::rows(
        &typed,
        asking.as_ref().map(|query| query.word.as_str()),
        &looked,
        looking_for,
        &|at, found| {
          let Ok(thing) = thing(held, tab, kind, Line(at), found, &cache_3, &into);

          thing
        },
      );

      rows::noted(kind, rows)
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Line(u32);

fn thing(
  held: &Panel,
  tab: u32,
  kind: Kind,
  at: Line,
  found: &Found,
  cache: &Path,
  into: &Path,
) -> Result<Row, Never> {
  let Ok(have) = getting::holds(into, &found.id);
  let Ok(aside) = looking::aside(kind, found, have);
  let Ok(chose) = chose(held, tab, kind, found);
  let Ok(picture) = picture(cache, found);
  let Ok(offers) = offers(held, tab, found, at);

  let Ok(fetching) = fetched_by(kind, found);

  let Ok(row) = Row::new(&found.title, Aside(&aside), chose);
  let Ok(pictured) = row.picturing(picture);
  let Ok(pictured_2) = pictured.selectable(&found.id);
  let Ok(pictured_3) =
    pictured_2.while_running(&fetching, &format!("{DOWNLOADING} {}", found.title));

  pictured_3.offering(offers)
}

fn chose(held: &Panel, tab: u32, kind: Kind, found: &Found) -> Result<Handler, Never> {
  let held_2 = held.clone();
  let found_2 = found.clone();

  Handler::and_stay(move |showing| {
    let Ok(into) = getting::into(kind);
    let Ok(have) = getting::holds(&into, &found_2.id);
    let Ok(folder) = folder(kind);

    let Ok(()) = press(
      &held_2,
      DownloadsEvent::Chose {
        tab,
        kind,
        found: found_2.clone(),
        have,
        into: folder,
      },
      showing,
    );
  })
}

fn offers(
  held: &Panel,
  tab: u32,
  found: &Found,
  from: Line,
) -> Result<impl Fn(&dyn Showing) -> ActionOutcome + Send + Sync + 'static, Never> {
  let held_2 = held.clone();
  let found_2 = found.clone();
  let from_2 = from.0;

  Ok(move |showing: &dyn Showing| {
    let Ok(()) = press(
      &held_2,
      DownloadsEvent::Offered {
        tab,
        found: found_2.clone(),
        from: from_2,
      },
      showing,
    );

    ActionOutcome::Stay
  })
}

fn picture(cache: &Path, found: &Found) -> Result<Picture, Never> {
  let Ok(at) = store::picture_of(cache, &found.id);

  Ok(match at {
    Some(at_2) => match console_core_atomic_writes::read_prefix(&at_2, 1) {
      Ok(_header) => Picture::At(at_2),
      Err(_not_cached) => Picture::Space,
    },
    None => Picture::Space,
  })
}

fn ways(held: &Panel, tab: u32, kind: Kind, found: &Found) -> Result<Vec<Row>, Never> {
  let Ok(other) = kind.other();
  let backing = held.clone();

  let other_3 = match other {
    Some(other_2) => {
      let Ok(chose) = chose(held, tab, other_2, found);

      Some((other_2, chose))
    }
    None => None,
  };

  let selecting = held.clone();
  let id = found.id.clone();
  let Ok(ways) = rows::ways(
    found,
    move |showing| {
      let Ok(()) = press(&backing, DownloadsEvent::Back { tab }, showing);
    },
    other_3,
  );
  let Ok(selects) = Handler::and_stay(move |showing| {
    let Ok(()) = press(&selecting, DownloadsEvent::Back { tab }, showing);

    showing.select(vec![id.clone()]);
  });
  let Ok(select) = Row::new(console_panel::page::SELECT, Aside(""), selects);

  Ok([ways, vec![select]].concat())
}

fn fetched_together(
  held: &Panel,
  tab: u32,
  kind: Kind,
  showing: &dyn Showing,
  keys: &[String],
) -> Result<(), Never> {
  let Ok(current) = at(held, tab);
  let Ok(looked) = looked(kind, current.result);
  let wanted: std::collections::BTreeSet<&String> = keys.iter().collect();

  for found in looked
    .found
    .iter()
    .filter(|found| wanted.contains(&found.id))
  {
    let Ok(into) = getting::into(kind);
    let Ok(have) = getting::holds(&into, &found.id);
    let Ok(folder) = folder(kind);

    let Ok(()) = press(
      held,
      DownloadsEvent::Chose {
        tab,
        kind,
        found: found.clone(),
        have,
        into: folder,
      },
      showing,
    );
  }

  Ok(())
}

fn looking_for(
  held: &Panel,
  tab: u32,
  kind: Kind,
  searches: &Searches,
) -> Result<Handler, Never> {
  let held_2 = held.clone();
  let searches_2 = searches.clone();

  Handler::and_stay(move |showing| {
    let Ok(effects) = decided(&held_2, DownloadsEvent::LookFor { tab, kind });

    for effect in effects {
      match effect {
        Effect::Custom(DownloadsEffect::Search {
          tab: tab_2,
          kind: kind_2,
          query,
        }) => {
          let Ok(()) = searches_2.start(&held_2, tab_2, kind_2, query);
        }
        Effect::Custom(DownloadsEffect::Cancel { tab: tab_3, id }) => {
          let Ok(()) = searches_2.cancel(tab_3, id);
        }
        effect_2 @ (Effect::Custom(
          DownloadsEffect::Replace(_)
          | DownloadsEffect::Refresh
          | DownloadsEffect::Notify(_)
          | DownloadsEffect::ForgetTyping
          | DownloadsEffect::Retain(_),
        )
        | Effect::Run(_)
        | Effect::Stream(_)
        | Effect::Fork(_)
        | Effect::Interrupt(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_)) => {
          let Ok(()) = carry(&effect_2, showing);
        }
      }
    }
  })
}

fn pages(held: &Panel, searches: &Searches) -> Result<Vec<Page>, Never> {
  Ok(
    Kind::ALL
      .iter()
      .enumerate()
      .map(|(tab, kind)| {
        let Ok(tab_2) = console_core_number_conversion::fitted::<_, u32>(tab);
        let Ok(page) = page(held, tab_2, *kind, searches);

        page
      })
      .collect(),
  )
}

const DOWNLOAD: &str = "Download";

fn page(held: &Panel, tab: u32, kind: Kind, searches: &Searches) -> Result<Page, Never> {
  let reading = held.clone();
  let backing = held.clone();
  let Ok(word) = kind.tab();
  let searches_2 = searches.clone();
  let Ok(asked) = Rows::computed(move || {
    let Ok(rows) = rows_of(&reading, tab, kind, &searches_2);

    rows
  });
  let Ok(page) = Page::new(word, asked);
  let fetching = held.clone();
  let Ok(page_2) = page.selecting(DOWNLOAD, move |showing, keys| {
    let Ok(()) = fetched_together(&fetching, tab, kind, showing, keys);
  });
  let Ok(page_3) = page_2.on_back(move |showing| {
    let Ok(at) = at(&backing, tab);
    let was = Standing { tabs: vec![at] };

    let Ok(()) = press(&backing, DownloadsEvent::Back { tab }, showing);

    let Ok(closes) = closes(&was, 0);

    match closes {
      Closes::Yes => ActionOutcome::Close,
      Closes::No => ActionOutcome::Stay,
    }
  });
  let Ok(held_at) = at(held, tab);

  match held_at.onto {
    Destination::List => {}
    Destination::Ways { .. } => return Ok(page_3),
  }

  let typing = held.clone();

  let Ok(about) = rows::about(kind);

  page_3.searching(about, move |showing, word_2| {
    let Ok(()) = press(
      &typing,
      DownloadsEvent::Typed {
        tab,
        word: word_2.to_string(),
      },
      showing,
    );
  })
}

pub const WHO: &str = "downloads";

pub fn card(arguments: &[String]) -> Result<Card, Never> {
  let Ok((searches, owner)) = Searches::open();
  let Ok(card) = Card::supervised::<Downloads, _>((), move |held| pages(held, &searches));
  let done = card.done;
  let card_2 = Card {
    done: Box::new(move || {
      let Ok(()) = owner.close();
      done()
    }),
    ..card
  };

  card_2.opening_at(arguments.first().map(String::as_str))
}
