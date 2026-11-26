//! Metadata review keeps the recording list, local matches, and online matches
//! in separate views. A lookup changes only its recording's results.

use console_panel::page::{PagePosition, Subject};

use super::catalog::{action, catalog_watch, manage};
use super::*;
use crate::management::{Cached, Lookup, Management, MetadataView};
use crate::review::FoundTrack;
use crate::suggestions::{self, Proposal, Search, Source};

const TITLE: &str = "Metadata Suggestions";

pub(super) fn suggestions_page(held: &Panel) -> Result<Page, Never> {
  let reading = held.clone();
  let returning = held.clone();
  let position = held.clone();
  let moving = held.clone();
  let Ok(rows) = Rows::computed(move || {
    let Ok(displayed) = metadata_rows(&reading);

    displayed
  });
  let Ok(page) = Page::new(TITLE, rows);
  let Ok(watched) = catalog_watch(page);
  let Ok(with_back) = watched.on_back(move |showing| {
    let Ok(standing) = panel_state(&returning);

    match standing.management.metadata {
      MetadataView::Recordings => ActionOutcome::Close,
      MetadataView::Recording(_) | MetadataView::MusicBrainz(_) => {
        let Ok(()) = manage(&returning, Request::MetadataBack, showing);

        ActionOutcome::Stay
      }
    }
  });

  with_back.remembering_position(
    move || {
      let Ok(standing) = panel_state(&position);
      let Ok(saved) = standing.management.metadata_position();

      saved
    },
    move |view: PagePosition| {
      let Ok(_no_work) = decided(&moving, MusicEvent::Manage(Request::ViewedMetadata(view)));
    },
  )
}

fn metadata_rows(held: &Panel) -> Result<Vec<Row>, Never> {
  let folder = folder()?;
  let files = visible_songs(&folder)?;
  let state = panel_state(held)?;

  match &state.management.metadata {
    MetadataView::Recordings => recording_list(held, &files),
    MetadataView::Recording(file) => recording(held, &files, file, &state.management),
    MetadataView::MusicBrainz(file) => musicbrainz(held, &files, file, &state.management),
  }
}

fn recording_list(held: &Panel, files: &[Song]) -> Result<Vec<Row>, Never> {
  let mut rows = Vec::new();

  for file in files {
    let missing = match (file.tags.artist.trim(), file.tags.title.trim()) {
      ("", "") => "Missing artist and title",
      ("", _) => "Missing artist",
      (_, "") => "Missing title",
      (_, _) => continue,
    };
    let Ok(query) = suggestions::query(file);
    let key = query.file.path.clone();
    let Ok(does) = action(held, Request::OpenMetadata(query.file));
    let Ok(row) = Row::new(&file.name, Aside(missing), does);
    let Ok(pick) = row.opening();
    let Ok(identified) = pick.selectable(&key);
    let entry = match rows.as_slice() {
      [] => identified,
      [_, ..] => {
        let Ok(separated) = identified.with_space_above();

        separated
      }
    };

    rows.push(entry);
  }

  match rows.is_empty().then_some(()) {
    Some(()) => {
      let Ok(complete) = Row::placeholder("All visible tracks have an artist and title");
      rows.push(complete);
    }
    None => {}
  }

  Ok(rows)
}

fn back_row(held: &Panel, title: &str) -> Result<Row, Never> {
  let returning = held.clone();

  Row::back(title, move |showing| {
    let Ok(()) = manage(&returning, Request::MetadataBack, showing);
  })
}

fn original(held: &Panel, file: &Song) -> Result<Row, Never> {
  let Ok(play) = plays(held, &file.path, Kind::ASong);

  Row::new(&file.name, Aside("Play recording"), play)
}

fn section(title: &str) -> Result<Row, Never> {
  let Ok(row) = Row::naming(title, Aside(""));

  row.with_space_above()
}

fn track<'a>(files: &'a [Song], file: &FoundTrack) -> Result<Option<&'a Song>, Never> {
  let Ok(path) = console_core_file_urls::path(&file.path);

  Ok(path.and_then(|target| {
    files
      .iter()
      .find(|song| song.path == target && song.revision() == Ok(file.revision))
  }))
}

fn recording(
  held: &Panel,
  files: &[Song],
  file: &FoundTrack,
  state: &Management,
) -> Result<Vec<Row>, Never> {
  let Ok(back) = back_row(held, TITLE);
  let Ok(heading) = section("Original Recording");
  let Ok(found) = track(files, file);
  let song = match found {
    Some(song) => song,
    None => return unavailable(back),
  };
  let Ok(play) = original(held, song);
  let mut rows = vec![back, heading, play];
  let Ok(missing) = suggestions::missing(song);

  match missing {
    Some(()) => {
      let Ok(online) = musicbrainz_door(held, song, state);
      rows.push(online);
      let Ok(local) = library_matches(held, file);
      rows.extend(local);
    }
    None => {
      let Ok(saved) = section("Current Metadata");
      let Ok(tags) = Row::stacked(Picture::None, &song.tags.title, Aside(&song.tags.artist));
      rows.extend([saved, tags]);
    }
  }

  Ok(rows)
}

fn unavailable(back: Row) -> Result<Vec<Row>, Never> {
  let Ok(status) = Row::placeholder("This recording is unavailable or has changed");

  Ok(vec![back, status])
}

fn musicbrainz_door(held: &Panel, song: &Song, state: &Management) -> Result<Row, Never> {
  let Ok(query) = suggestions::query(song);
  let Ok(previous) = suggestions::online_results(&query.file);
  let Ok(activity) = state.lookup(&query.file);
  let cached = match previous {
    Some(_) => Cached::Yes,
    None => Cached::No,
  };
  let (title, status) = match (activity, cached) {
    (Lookup::Pending, _) => ("MusicBrainz Results", "Searching…"),
    (Lookup::Failed, _) => ("MusicBrainz Results", "Search failed"),
    (Lookup::Ready, Cached::Yes) => ("MusicBrainz Results", ""),
    (Lookup::Ready, Cached::No) => ("Search MusicBrainz", ""),
  };
  let Ok(does) = action(held, Request::OpenMusicBrainz { query, cached });
  let Ok(row) = Row::new(title, Aside(status), does);
  let Ok(opening) = row.opening();

  opening.with_space_above()
}

fn library_matches(held: &Panel, file: &FoundTrack) -> Result<Vec<Row>, Never> {
  let Ok(heading) = section("Matches in Your Library");
  let Ok(local) = suggestions::local_results();
  let mut rows = vec![heading];

  for proposal in local.iter().filter(|proposal| proposal.file == *file) {
    let Ok(row) = proposal_row(held, proposal);
    rows.push(row);
  }

  match rows.as_slice() {
    [_heading] => {
      let Ok(empty) = Row::placeholder("No matches in your library");
      rows.push(empty);
    }
    [] | [_, _, ..] => {}
  }

  Ok(rows)
}

fn musicbrainz(
  held: &Panel,
  files: &[Song],
  file: &FoundTrack,
  state: &Management,
) -> Result<Vec<Row>, Never> {
  let Ok(back) = back_row(held, "Original Recording");
  let Ok(found) = track(files, file);
  let song = match found {
    Some(song) => song,
    None => return unavailable(back),
  };
  let Ok(play) = original(held, song);
  let Ok(missing) = suggestions::missing(song);
  let mut rows = vec![back, play];

  match missing {
    Some(()) => {
      let Ok(heading) = section("MusicBrainz Results");
      rows.push(heading);
      let Ok(results) = musicbrainz_rows(held, song, state);
      rows.extend(results);
    }
    None => {
      let Ok(heading) = section("Current Metadata");
      let Ok(saved) = Row::stacked(Picture::None, &song.tags.title, Aside(&song.tags.artist));
      rows.extend([heading, saved]);
    }
  }

  Ok(rows)
}

fn musicbrainz_rows(held: &Panel, song: &Song, state: &Management) -> Result<Vec<Row>, Never> {
  let Ok(query) = suggestions::query(song);
  let Ok(lookup) = state.lookup(&query.file);
  let Ok(previous) = suggestions::online_results(&query.file);
  let Ok(status) = search_row(held, query, lookup, previous.as_deref());
  let mut rows = vec![status];

  for proposal in previous.iter().flatten() {
    let Ok(row) = proposal_row(held, proposal);
    rows.push(row);
  }

  Ok(rows)
}

fn search_row(
  held: &Panel,
  query: Search,
  lookup: Lookup,
  results: Option<&[Proposal]>,
) -> Result<Row, Never> {
  let previous = match results {
    Some([_, ..]) => "Showing previous matches",
    Some([]) | None => "",
  };
  let (title, aside) = match (lookup, results) {
    (Lookup::Pending, _) => return Row::text("Searching MusicBrainz…", Aside(previous)),
    (Lookup::Failed, _) => ("Retry Search", "Search failed"),
    (Lookup::Ready, Some([])) => ("Search Again", "No matches found"),
    (Lookup::Ready, Some([_, ..])) => ("Search Again", ""),
    (Lookup::Ready, None) => ("Search MusicBrainz", ""),
  };
  let Ok(does) = action(held, Request::SearchSong(query));

  Row::new(title, Aside(aside), does)
}

fn proposal_row(held: &Panel, proposal: &Proposal) -> Result<Row, Never> {
  let mut detail = proposal.tags.artist.clone();

  match proposal.tags.album.as_str() {
    "" => {}
    release => detail.push_str(&format!(" · {release}")),
  }

  let subject = format!("{} · {detail}", proposal.tags.title);

  match proposal.source {
    Source::Audio | Source::Filename => {
      let Ok(evidence) = proposal.source.label();
      detail.push_str(&format!(" · {evidence}"));
    }
    Source::MusicBrainz => {}
  }

  let destination = held.clone();
  let choice = proposal.clone();
  let Ok(does) = Handler::and_stay(move |showing| {
    let result = destination.clone();
    let confirmed = choice.clone();

    showing.sure(
      "Use this metadata?",
      Subject(&subject),
      &["Use in Music Library"],
      Box::new(move |confirmation, _| {
        let Ok(()) = manage(&result, Request::ConfirmSong(confirmed), confirmation);
      }),
    );
  });
  let Ok(row) = Row::stacked(Picture::None, &proposal.tags.title, Aside(&detail));

  row.choosing(does)
}
