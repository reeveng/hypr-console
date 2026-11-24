//! Rows and the effect driver for the Music Library management tabs.

use std::collections::BTreeMap;

use super::*;
use crate::management::AlbumStart;

pub(super) fn manage_effect(
  held: &Panel,
  work: &Work,
  showing: &dyn Showing,
) -> Result<(), Never> {
  match work {
    Work::FocusFirst => showing.replace(0),
    Work::Redraw => showing.refresh(),
    Work::Refresh { id } => {
      let Ok(command) = RunCommand::internal(crate::update::INDEX, &["--analyze"]);
      let Ok(()) = task(held, showing, command, *id, Job::Scan);
    }
    Work::Cover { id, album } => {
      let Ok(()) = json_task(held, showing, "--cover", album, *id, Job::Cover);
    }
    Work::SearchSong { id, query } => {
      let Ok(()) = json_task(held, showing, "--suggest", query, *id, Job::Search);
    }
    Work::Select { id, changes } => {
      let Ok(()) = saved(held, showing, *id, Selection::apply(changes));
    }
    Work::ConfirmSong { id, choice } => {
      let Ok(()) = saved(held, showing, *id, crate::suggestions::apply(choice));
    }
    Work::PlayAlbum(start) => play_album(start, showing)?,
  }

  Ok(())
}

fn json_task<T: serde::Serialize>(
  held: &Panel,
  showing: &dyn Showing,
  flag: &str,
  value: &T,
  id: u64,
  job: Job,
) -> Result<(), Never> {
  match serde_json::to_string(value) {
    Ok(encoded) => {
      let Ok(command) = RunCommand::internal(crate::update::INDEX, &[flag, &encoded]);

      task(held, showing, command, id, job)
    }
    Err(fault) => {
      console_core_logging::warn!("could not prepare lookup: {fault}");

      manage(
        held,
        Manage::Finished {
          id,
          job,
          status: ExitStatus::Failure(None),
        },
        showing,
      )
    }
  }
}

fn saved(
  held: &Panel,
  showing: &dyn Showing,
  id: u64,
  result: std::io::Result<()>,
) -> Result<(), Never> {
  let status = match result {
    Ok(()) => ExitStatus::Success,
    Err(fault) => {
      console_core_logging::warn!("could not save the Music Library choice: {fault}");
      showing.note("Could not save your choice");

      ExitStatus::Failure(None)
    }
  };

  manage(
    held,
    Manage::Finished {
      id,
      job: Job::Selection,
      status,
    },
    showing,
  )
}

fn play_album(start: &AlbumStart, showing: &dyn Showing) -> Result<(), Never> {
  use console_music_player::library::{AlbumQueue, album_argument};

  let folder = folder()?;
  let songs = visible_songs(&folder)?;
  let albums = albums::groups(&songs)?;
  let album = match albums.iter().find(|album| album.key == start.album) {
    Some(album) => album,
    None => return Ok(()),
  };
  let first = match start.track.as_ref() {
    Some(track) => match (0_u32..)
      .zip(&album.tracks)
      .find(|(_, song)| &song.path == track)
    {
      Some((first, _song)) => first,
      None => return Ok(()),
    },
    None => 0,
  };
  let tracks = albums::paths(album)?;
  let Ok(arguments) = album_argument(&AlbumQueue { tracks, first });
  let Ok(command) = RunCommand::internal(
    console_core_internal_programs::InternalProgram::MusicPlayer,
    &["--album-uris", &arguments],
  );
  let Ok(words) = running::words_of(&command);
  showing.leave_running(words);

  Ok(())
}

fn task(
  held: &Panel,
  showing: &dyn Showing,
  command: RunCommand,
  id: u64,
  job: Job,
) -> Result<(), Never> {
  let destination = held.clone();
  let Ok(task) = console_panel::tasks::Task::of(command);
  let Ok(task) = task.then(move |answer| {
    let Ok(effects) = decided(
      &destination,
      MusicEvent::Manage(Manage::Finished {
        id,
        job,
        status: answer.status.clone(),
      }),
    );
    match effects.is_empty().then_some(()) {
      Some(()) => return Ok(None),
      None => {}
    }

    let said = match (job, &answer.status) {
      (Job::Cover, ExitStatus::Success) => "Album cover saved",
      (Job::Cover, ExitStatus::Failure(Some(2))) => "No matching cover found",
      (Job::Cover, ExitStatus::Failure(_) | ExitStatus::Interrupt) => "Album cover lookup ended",
      (Job::Scan, ExitStatus::Success) => "Music Library scan finished",
      (Job::Scan, ExitStatus::Failure(_) | ExitStatus::Interrupt) => {
        "Music Library scan ended early"
      }
      (Job::Search, ExitStatus::Success) => "MusicBrainz search finished",
      (Job::Search, ExitStatus::Failure(_) | ExitStatus::Interrupt) => {
        "MusicBrainz search ended early"
      }
      (Job::Selection, _) => return Ok(None),
    };
    let Ok(note) = Notification::of(said);

    Ok(Some(note))
  });
  showing.run(task);

  Ok(())
}

fn manage(held: &Panel, request: Manage, showing: &dyn Showing) -> Result<(), Never> {
  press(held, MusicEvent::Manage(request), showing)
}

fn action(held: &Panel, request: Manage) -> Result<Handler, Never> {
  let destination = held.clone();

  Handler::and_stay(move |showing| {
    let Ok(()) = manage(&destination, request.clone(), showing);
  })
}

pub(super) fn albums_page(held: &Panel) -> Result<Page, Never> {
  let view = held.clone();
  let leaving = held.clone();
  let Ok(rows) = Rows::computed(move || {
    let Ok(rows) = album_rows(&view);

    rows
  });
  let Ok(page) = Page::new("Albums", rows);

  page.on_back(move |showing| {
    let Ok(state) = panel_state(&leaving);

    match state.management.album {
      Some(_) => {
        let Ok(()) = manage(&leaving, Manage::Browse(None), showing);

        ActionOutcome::Stay
      }
      None => ActionOutcome::Close,
    }
  })
}

fn album_rows(held: &Panel) -> Result<Vec<Row>, Never> {
  let folder = folder()?;
  let songs = visible_songs(&folder)?;
  let albums = albums::groups(&songs)?;
  let state = panel_state(held)?;

  match state.management.album.as_ref() {
    Some(key) => match albums.iter().find(|album| &album.key == key) {
      Some(album) => return album_tracks(held, album, &state.management.cover),
      None => {}
    },
    None => {}
  }

  match albums.is_empty().then_some(()) {
    Some(()) => {
      let what = match state.management.scan {
        Status::Pending(_) => "Reading album tags…",
        Status::Idle | Status::Finished(_) => "No tagged albums",
      };
      let Ok(row) = Row::placeholder(what);

      Ok(vec![row])
    }
    None => album_list(held, &albums),
  }
}

fn album_list(held: &Panel, albums: &[albums::Album<'_>]) -> Result<Vec<Row>, Never> {
  let mut rows = Vec::new();
  let mut artist = None;

  for album in albums {
    match (artist != Some(album.artist)).then_some(()) {
      Some(()) => {
        let label = match album.artist {
          "" => "Unknown artist",
          name => name,
        };
        let Ok(row) = Row::naming(label, Aside(""));
        rows.push(row);
        artist = Some(album.artist);
      }
      None => {}
    }

    let does = action(held, Manage::Browse(Some(album.key.clone())))?;
    let aside = format!("{} tracks", album.tracks.len());
    let Ok(row) = Row::new(album.title, Aside(&aside), does);
    let cover = album_art::cached(&album.key)?;
    let Ok(row) = row.picturing(Picture::Sleeve(cover));
    let Ok(row) = row.opening();
    rows.push(row);
  }

  Ok(rows)
}

fn album_tracks(
  held: &Panel,
  album: &albums::Album<'_>,
  cover_job: &Status,
) -> Result<Vec<Row>, Never> {
  let back = held.clone();
  let Ok(back) = Row::back("Albums", move |showing| {
    let Ok(()) = manage(&back, Manage::Browse(None), showing);
  });
  let cover = album_art::cached(&album.key)?;
  let Ok(head) = Row::stacked(
    Picture::Sleeve(cover.clone()),
    album.title,
    Aside(album.artist),
  );
  let does = action(
    held,
    Manage::PlayAlbum(AlbumStart {
      album: album.key.clone(),
      track: None,
    }),
  )?;
  let Ok(play) = Row::new("Play Album", Aside(""), does);
  let mut rows = vec![back, head, play];

  match (cover, cover_job) {
    (None, Status::Pending(_)) => {
      let Ok(row) = Row::placeholder("Fetching cover…");
      rows.push(row);
    }
    (None, Status::Idle | Status::Finished(_)) => {
      let does = action(
        held,
        Manage::Cover(album_art::Lookup {
          album: album.key.clone(),
          artist: album.artist.to_owned(),
          title: album.title.to_owned(),
        }),
      )?;
      let Ok(row) = Row::new("Find Album Art", Aside("MusicBrainz, then YouTube"), does);
      rows.push(row);
    }
    (Some(_), _) => {}
  }

  for song in &album.tracks {
    let Ok(name) = song.says();
    let does = action(
      held,
      Manage::PlayAlbum(AlbumStart {
        album: album.key.clone(),
        track: Some(song.path.clone()),
      }),
    )?;
    let aside = match song.tags.album.track {
      Some(track) => format!("Track {track}"),
      None => song.tags.artist.clone(),
    };
    let Ok(row) = Row::new(name, Aside(&aside), does);
    rows.push(row);
  }

  Ok(rows)
}

pub(super) fn review_page(held: &Panel) -> Result<Page, Never> {
  let reading = held.clone();
  let Ok(rows) = Rows::computed(move || {
    let Ok(rows) = review_rows(&reading);

    rows
  });

  Page::new("Possible Duplicates", rows)
}

fn review_rows(held: &Panel) -> Result<Vec<Row>, Never> {
  let state = panel_state(held)?;
  let scan = match state.management.scan {
    Status::Pending(_) => Row::placeholder("Scanning your library…")?,
    Status::Idle | Status::Finished(_) => {
      let does = action(held, Manage::Refresh)?;

      Row::new("Scan for Possible Duplicates", Aside(""), does)?
    }
  };
  let mut rows = vec![scan];
  let folder = folder()?;
  let songs = songs(&folder)?;
  let by_path: BTreeMap<_, _> = songs
    .iter()
    .map(|song| {
      let Ok(key) = review::key(&song.path);

      (key, song)
    })
    .collect();
  let selected = selection()?;
  let findings = review::findings()?;

  for finding in &findings {
    let tracks: Vec<_> = finding
      .tracks
      .iter()
      .filter_map(|track| {
        by_path
          .get(&track.path)
          .copied()
          .filter(|song| song.revision == track.revision)
      })
      .collect();

    let reviewed = tracks.iter().all(|track| {
      selected
        .decision(&track.path)
        .is_ok_and(|decision| decision.is_some())
    });
    match tracks.as_slice() {
      [first, _, ..] => {
        let Ok(title) = first.says();
        let Ok(evidence) = finding.evidence.label();
        let label = match reviewed.then_some(()) {
          Some(()) => format!("{evidence} · Reviewed"),
          None => evidence.to_owned(),
        };
        let Ok(head) = Row::naming(title, Aside(&label));
        rows.push(head);
      }
      [] | [_] => continue,
    }

    let hidden = tracks
      .iter()
      .any(|track| selected.visibility(&track.path) == Ok(Visibility::Hidden));
    match (hidden || !reviewed).then_some(()) {
      Some(()) => {
        let all = tracks
          .iter()
          .map(|song| (song.path.clone(), Visibility::Included))
          .collect();
        let does = action(held, Manage::Select(all))?;
        let Ok(row) = Row::new("Include All Versions", Aside(""), does);
        rows.push(row);
      }
      None => {}
    }

    for track in &tracks {
      let row = review_track(held, &selected, track, &tracks, &folder)?;

      rows.push(row);
    }
  }

  match (rows.len() == 1 && !matches!(state.management.scan, Status::Pending(_))).then_some(()) {
    Some(()) => {
      let Ok(row) = Row::placeholder("No possible duplicates to review");
      rows.push(row);
    }
    None => {}
  }

  Ok(rows)
}

fn review_track(
  held: &Panel,
  selected: &Selection,
  song: &Song,
  group: &[&Song],
  folder: &Path,
) -> Result<Row, Never> {
  let title = song.says()?;
  let visibility = selected.visibility(&song.path)?;
  let note = match visibility {
    Visibility::Included => "Included",
    Visibility::Hidden => "Hidden",
  };
  let relative = match song.path.strip_prefix(folder) {
    Ok(relative) => relative,
    Err(_outside) => &song.path,
  };
  let aside = format!("{note} · {}", relative.display());
  let plays = plays(held, &song.path, Kind::ASong)?;
  let Ok(row) = Row::new(title, Aside(&aside), plays);
  let destination = held.clone();
  let path = song.path.clone();
  let only: Vec<_> = group
    .iter()
    .map(|track| {
      let visibility = match (track.path == path).then_some(()) {
        Some(()) => Visibility::Included,
        None => Visibility::Hidden,
      };

      (track.path.clone(), visibility)
    })
    .collect();

  row.offering(move |showing| {
    let other = destination.clone();
    let include = match visibility {
      Visibility::Included => Visibility::Hidden,
      Visibility::Hidden => Visibility::Included,
    };
    let label = match include {
      Visibility::Included => "Include this version",
      Visibility::Hidden => "Hide this version",
    };
    let changes = vec![(path.clone(), include)];
    let only = only.clone();

    showing.sure(
      "Which versions should appear in Music Library?",
      console_panel::page::Subject(title_of(&path)),
      &[label, "Keep Only This Version"],
      Box::new(move |showing, chosen| {
        let changes = match chosen {
          0 => changes,
          1 => only,
          _ => return,
        };
        let Ok(()) = manage(&other, Manage::Select(changes), showing);
      }),
    );
    ActionOutcome::Stay
  })
}

fn title_of(path: &Path) -> &str {
  path
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or("Recording")
}

pub(super) fn suggestions_page(held: &Panel) -> Result<Page, Never> {
  let reading = held.clone();
  let Ok(rows) = Rows::computed(move || {
    let Ok(rows) = metadata_rows(&reading);

    rows
  });

  Page::new("Metadata Suggestions", rows)
}

fn metadata_rows(held: &Panel) -> Result<Vec<Row>, Never> {
  use crate::suggestions;

  let folder = folder()?;
  let files = visible_songs(&folder)?;
  let local = suggestions::local_results()?;
  let state = panel_state(held)?;
  let mut by_path = BTreeMap::<&str, Vec<&suggestions::Proposal>>::new();

  for proposal in &local {
    by_path
      .entry(&proposal.file.path)
      .or_default()
      .push(proposal);
  }

  let mut rows = Vec::new();
  for file in &files {
    match suggestions::missing(file) {
      Ok(Some(())) => {}
      Ok(None) => continue,
    }
    let Ok(query) = suggestions::query(file);
    let Ok(does) = plays(held, &file.path, Kind::ASong);
    let Ok(track) = Row::new(&file.name, Aside("Play recording"), does);
    rows.push(track);

    let local = by_path
      .get(query.file.path.as_str())
      .into_iter()
      .flatten()
      .filter(|proposal| proposal.file == query.file);
    for proposal in local {
      let Ok(row) = proposal_row(held, proposal);
      rows.push(row);
    }

    let Ok(online) = suggestions::online_results(&query.file);
    match online {
      Some(found) => match found.as_slice() {
        [] => {
          let Ok(row) = Row::placeholder("No MusicBrainz match");
          rows.push(row);
        }
        [_first, ..] => {
          for proposal in &found {
            let Ok(row) = proposal_row(held, proposal);
            rows.push(row);
          }
        }
      },
      None => {}
    }

    match state.management.search {
      Status::Pending(_) => {}
      Status::Idle | Status::Finished(_) => {
        let does = action(held, Manage::SearchSong(query))?;
        let Ok(row) = Row::new("Search MusicBrainz", Aside(""), does);
        rows.push(row);
      }
    }
  }

  match rows.is_empty().then_some(()) {
    Some(()) => {
      let Ok(row) = Row::placeholder("All visible tracks have an artist and title");
      rows.push(row);
    }
    None => {}
  }

  Ok(rows)
}

fn proposal_row(held: &Panel, proposal: &crate::suggestions::Proposal) -> Result<Row, Never> {
  let description = match proposal.tags.album.as_str() {
    "" => format!("{} · {}", proposal.tags.artist, proposal.tags.title),
    release => format!(
      "{} · {} · {release}",
      proposal.tags.artist, proposal.tags.title
    ),
  };
  let Ok(evidence) = proposal.source.label();
  let subject = description.clone();
  let choice = proposal.clone();
  let held = held.clone();
  let Ok(does) = Handler::and_stay(move |showing| {
    let result = held.clone();
    let choice = choice.clone();

    showing.sure(
      "Is this the song?",
      console_panel::page::Subject(&subject),
      &["Use in Music Library"],
      Box::new(move |showing, _| {
        let Ok(()) = manage(&result, Manage::ConfirmSong(choice), showing);
      }),
    );
  });

  Row::new(&description, Aside(evidence), does)
}
