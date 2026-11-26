//! Rows and the effect driver for the Music Library management tabs.

use std::collections::BTreeMap;
use std::path::PathBuf;

use console_core_external_programs::Program;
use console_events::again::Worth;
use console_panel::page::{ButtonStyle, Subject};
use console_program_contract::Command;

use super::*;
use crate::looking::FileRevision;
use crate::management::AlbumStart;
use crate::scan::{self, Activity};

pub(super) fn manage_effect(
  held: &Panel,
  work: &Work,
  showing: &dyn Showing,
) -> Result<Option<Request>, Never> {
  match work {
    Work::FocusFirst => showing.replace(0),
    Work::Redraw => showing.refresh(),
    Work::Refresh { id } => return scan_task(held, showing, *id),
    Work::Cover { id, album } => {
      return json_task(held, showing, "--cover", album, *id, Job::Cover);
    }
    Work::SearchSong { id, query } => {
      return json_task(held, showing, "--suggest", query, *id, Job::Search);
    }
    Work::Select { id, changes } => {
      let Ok(reply) = saved(*id, Selection::apply(changes));

      return Ok(Some(reply));
    }
    Work::ConfirmSong { id, choice } => {
      let Ok(reply) = saved(*id, crate::suggestions::apply(choice));

      return Ok(Some(reply));
    }
    Work::PlayAlbum(start) => play_album(start, showing)?,
  }

  Ok(None)
}

fn scan_task(held: &Panel, showing: &dyn Showing, id: u64) -> Result<Option<Request>, Never> {
  let Ok(index) = crate::update::INDEX.at();
  let program = match index.to_str() {
    Some(program) => program,
    None => {
      console_core_logging::warn!("could not start the Music Library scan: {index:?}");
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Music Library"),
        "Could not start the Music Library scan",
        None,
      );

      return Ok(Some(Request::Finished {
        id,
        job: Job::Scan,
        status: ExitStatus::Failure(None),
      }));
    }
  };
  let Ok(cache) = console_core_places::Base::Cache.called();
  let Ok(config) = console_core_places::Base::Configuration.called();
  let Ok(state) = console_core_places::Base::State.called();
  let arguments = [
    "--user",
    "--quiet",
    "--collect",
    "--wait",
    "--same-dir",
    "--service-type=exec",
    "--expand-environment=no",
    "--setenv",
    cache,
    "--setenv",
    config,
    "--setenv",
    state,
    "--",
    program,
    "--analyze",
  ];
  let Ok(command) = Command::external(Program::SystemdRun, &arguments);

  let Ok(()) = task(held, showing, command, id, Job::Scan);

  Ok(None)
}

fn json_task<T: serde::Serialize>(
  held: &Panel,
  showing: &dyn Showing,
  flag: &str,
  value: &T,
  id: u64,
  job: Job,
) -> Result<Option<Request>, Never> {
  match serde_json::to_string(value) {
    Ok(encoded) => {
      let Ok(command) = Command::internal(crate::update::INDEX, &[flag, &encoded]);

      let Ok(()) = task(held, showing, command, id, job);

      Ok(None)
    }
    Err(fault) => {
      console_core_logging::warn!("could not prepare lookup: {fault}");

      Ok(Some(Request::Finished {
        id,
        job,
        status: ExitStatus::Failure(None),
      }))
    }
  }
}

fn saved(id: u64, result: std::io::Result<()>) -> Result<Request, Never> {
  let status = match result {
    Ok(()) => ExitStatus::Success,
    Err(fault) => {
      console_core_logging::warn!("could not save the Music Library choice: {fault}");
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Music Library"),
        "Music Library could not save your choice",
        None,
      );

      ExitStatus::Failure(None)
    }
  };

  Ok(Request::Finished {
    id,
    job: Job::Selection,
    status,
  })
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
  let Ok(command) = Command::internal(
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
  command: Command,
  id: u64,
  job: Job,
) -> Result<(), Never> {
  let destination = held.clone();
  let Ok(work) = console_panel::tasks::Task::of(command);
  let Ok(attributed) = work.from_application(console_notifications::Application("Music Library"));
  let Ok(with_completion) = attributed.then(move |answer| {
    let Ok(effects) = decided(
      &destination,
      MusicEvent::Manage(Request::Finished {
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
      (Job::Scan, ExitStatus::Failure(code)) => {
        match (*code == Some(i32::from(scan::BUSY_EXIT_CODE))).then_some(()) {
          Some(()) => return Ok(None),
          None => "Music Library scan ended early",
        }
      }
      (Job::Scan, ExitStatus::Interrupt) => "Music Library scan ended early",
      (Job::Search | Job::Selection, _) => return Ok(None),
    };
    let Ok(note) = Notification::of(said);

    Ok(Some(note))
  });
  showing.run(with_completion);

  Ok(())
}

pub(super) fn manage(held: &Panel, request: Request, showing: &dyn Showing) -> Result<(), Never> {
  press(held, MusicEvent::Manage(request), showing)
}

pub(super) fn action(held: &Panel, request: Request) -> Result<Handler, Never> {
  let destination = held.clone();

  Handler::and_stay(move |showing| {
    let Ok(()) = manage(&destination, request.clone(), showing);
  })
}

pub(super) fn catalog_watch(page: Page) -> Result<Page, Never> {
  let Ok(directory) = scan::directory();

  match directory {
    Some(at) => page.with_subscription(EventGroup::Path(at), catalog_changed),
    None => Ok(page),
  }
}

fn catalog_changed(line: &str) -> Result<Worth, Never> {
  Ok(
    match Path::new(line).file_name().and_then(|name| name.to_str()) {
      Some(
        "music"
        | looking::CATALOG_FILE
        | review::FINDINGS_FILE
        | scan::LOCK_NAME
        | "metadata-suggestions-v1.json",
      ) => Worth::Querying,
      Some(_) | None => Worth::Ignoring,
    },
  )
}

fn scan_activity(status: &Status) -> Result<Activity, Never> {
  match status {
    Status::Pending(_) => Ok(Activity::Running),
    Status::Idle | Status::Finished(_) => scan::activity(),
  }
}

pub(super) fn albums_page(held: &Panel) -> Result<Page, Never> {
  let view = held.clone();
  let leaving = held.clone();
  let Ok(rows) = Rows::computed(move || {
    let Ok(rows) = album_rows(&view);

    rows
  });
  let Ok(page) = Page::new("Albums", rows);
  let Ok(watched) = catalog_watch(page);

  watched.on_back(move |showing| {
    let Ok(state) = panel_state(&leaving);

    match state.management.album {
      Some(_) => {
        let Ok(()) = manage(&leaving, Request::Browse(None), showing);

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

  let Ok(unread) = looking::unread(&songs);
  let reading = match unread {
    0 => Activity::Idle,
    _ => scan_activity(&state.management.scan)?,
  };
  let mut rows = match (reading, albums.is_empty().then_some(())) {
    (Activity::Running, _) => {
      let Ok(row) = Row::placeholder("Reading album tags…");

      vec![row]
    }
    (Activity::Idle, Some(())) => {
      let Ok(row) = Row::placeholder("No tagged albums");

      vec![row]
    }
    (Activity::Idle, None) => Vec::new(),
  };
  let Ok(albums_list) = album_list(held, &albums);
  rows.extend(albums_list);

  Ok(rows)
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

    let does = action(held, Request::Browse(Some(album.key.clone())))?;
    let aside = format!("{} tracks", album.tracks.len());
    let cover = album_art::cached(&album.key)?;
    let Ok(row) = Row::stacked_action(Picture::Sleeve(cover), album.title, Aside(&aside), does);
    let Ok(opening) = row.opening();
    rows.push(opening);
  }

  Ok(rows)
}

fn album_tracks(
  held: &Panel,
  album: &albums::Album<'_>,
  cover_job: &Status,
) -> Result<Vec<Row>, Never> {
  let back = held.clone();
  let Ok(back_2) = Row::back("Albums", move |showing| {
    let Ok(()) = manage(&back, Request::Browse(None), showing);
  });
  let cover = album_art::cached(&album.key)?;
  let Ok(head) = Row::stacked(
    Picture::Sleeve(cover.clone()),
    album.title,
    Aside(album.artist),
  );
  let does = action(
    held,
    Request::PlayAlbum(AlbumStart {
      album: album.key.clone(),
      track: None,
    }),
  )?;
  let Ok(play) = Row::new("Play Album", Aside(""), does);
  let mut rows = vec![back_2, head, play];

  match (cover, cover_job) {
    (None, Status::Pending(_)) => {
      let Ok(row) = Row::placeholder("Fetching cover…");
      rows.push(row);
    }
    (None, Status::Idle | Status::Finished(_)) => {
      let does_2 = action(
        held,
        Request::Cover(album_art::Lookup {
          album: album.key.clone(),
          artist: album.artist.to_owned(),
          title: album.title.to_owned(),
        }),
      )?;
      let Ok(row) = Row::new("Find Album Art", Aside("MusicBrainz, then YouTube"), does_2);
      rows.push(row);
    }
    (Some(_), _) => {}
  }

  for song in &album.tracks {
    let Ok(name) = song.says();
    let does_3 = action(
      held,
      Request::PlayAlbum(AlbumStart {
        album: album.key.clone(),
        track: Some(song.path.clone()),
      }),
    )?;
    let aside = match song.tags.album.track {
      Some(track) => format!("Track {track}"),
      None => song.tags.artist.clone(),
    };
    let Ok(row) = Row::new(name, Aside(&aside), does_3);
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

  let Ok(page) = Page::new("Possible Duplicates", rows);

  catalog_watch(page)
}

fn review_rows(held: &Panel) -> Result<Vec<Row>, Never> {
  let state = panel_state(held)?;
  let active = scan_activity(&state.management.scan)?;
  let scan = match active {
    Activity::Running => Row::placeholder("Scanning your library…")?,
    Activity::Idle => {
      let does = action(held, Request::Refresh)?;

      Row::new("Scan for Possible Duplicates", Aside(""), does)?
    }
  };
  let mut rows = vec![scan];
  let Ok(stored) = review::findings();
  let findings = match stored {
    Some(findings) => findings,
    None => return Ok(rows),
  };
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

  for finding in &findings {
    let tracks: Vec<_> = finding
      .tracks
      .iter()
      .filter_map(|track| {
        by_path
          .get(&track.path)
          .copied()
          .filter(|song| song.revision() == Ok(track.revision))
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

    for track in &tracks {
      let row = review_track(held, &selected, track, &tracks, &folder)?;
      rows.push(row);
    }

    let hidden = tracks
      .iter()
      .any(|track| selected.visibility(&track.path) == Ok(Visibility::Hidden));

    match (hidden || !reviewed).then_some(()) {
      Some(()) => {
        let changes = tracks
          .iter()
          .map(|song| (song.path.clone(), Visibility::Included))
          .collect();
        let Ok(row) = include_all(held, Request::Select(changes));
        rows.push(row);
      }
      None => {}
    }
  }

  match (rows.len() == 1 && active == Activity::Idle).then_some(()) {
    Some(()) => {
      let Ok(row) = Row::placeholder("No possible duplicates to review");
      rows.push(row);
    }
    None => {}
  }

  Ok(rows)
}

fn include_all(held: &Panel, request: Request) -> Result<Row, Never> {
  let destination = held.clone();
  let Ok(button) = ButtonPress::labelled("Include All Versions", Active::No, move |showing| {
    let Ok(()) = manage(&destination, request.clone(), showing);
  });
  let Ok(tinted) = button.styled(ButtonStyle::Tinted);

  Row::pressing(vec![tinted], 0)
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
  let Ok(name) = library::visible_name(relative.as_os_str());
  let aside = format!("{note} · {name}");
  let Ok(row) = Row::stacked(Picture::None, title, Aside(&aside));
  let destination = held.clone();
  let path = song.path.clone();
  let Ok(expected) = song.revision();
  let only: Vec<_> = group
    .iter()
    .map(|track| {
      let keeping = match (track.path == path).then_some(()) {
        Some(()) => Visibility::Included,
        None => Visibility::Hidden,
      };

      (track.path.clone(), keeping)
    })
    .collect();

  let Ok(does) = Handler::and_stay(move |showing| {
    let other = destination.clone();
    let track = path.clone();
    let file = name.clone();
    let include = match visibility {
      Visibility::Included => Visibility::Hidden,
      Visibility::Hidden => Visibility::Included,
    };
    let label = match include {
      Visibility::Included => "Include",
      Visibility::Hidden => "Hide",
    };
    let keep_only = only.clone();

    showing.sure(
      "Track Actions",
      Subject(&name),
      &["Play", label, "Keep Only", "Trash"],
      Box::new(move |picked, chosen| match chosen {
        0 => {
          let Ok(()) = press(
            &other,
            MusicEvent::Chose {
              path: track,
              kind: Kind::ASong,
            },
            picked,
          );
        }
        1 => {
          let Ok(()) = manage(&other, Request::Select(vec![(track, include)]), picked);
        }
        2 => {
          let Ok(()) = manage(&other, Request::Select(keep_only), picked);
        }
        3 => {
          let Ok(()) = confirm_trash(picked, track, expected, &file);
        }
        _ => {}
      }),
    );
  });
  let Ok(actionable) = row.choosing(does);

  actionable.opening()
}

fn confirm_trash(
  showing: &dyn Showing,
  path: PathBuf,
  expected: Option<FileRevision>,
  name: &str,
) -> Result<(), Never> {
  showing.sure(
    "Move this track to Trash?",
    Subject(name),
    &["Move to Trash"],
    Box::new(move |confirmed, _| {
      let Ok(()) = trash_track(confirmed, &path, expected);
    }),
  );

  Ok(())
}

fn trash_track(
  showing: &dyn Showing,
  path: &Path,
  expected: Option<FileRevision>,
) -> Result<(), Never> {
  let Ok(current) = FileRevision::of(path);
  let application = console_notifications::Application("Music Library");

  match current.filter(|revision| Some(*revision) == expected) {
    Some(_) => {}
    None => {
      let Ok(()) = console_notifications::notify(
        application,
        "The track changed or is no longer available",
        None,
      );
      showing.refresh();

      return Ok(());
    }
  }

  let absolute = match std::path::absolute(path) {
    Ok(target) => target,
    Err(fault) => {
      console_core_logging::warn!("could not name {} for Trash: {fault}", path.display());
      let Ok(()) = console_notifications::notify(
        application,
        "Music Library could not move the track to Trash",
        None,
      );

      return Ok(());
    }
  };
  let Ok(url) = review::key(&absolute);
  let Ok(command) = console_gio::trash_command(&[url]);
  let Ok(task) = console_panel::tasks::Task::of(command);
  let Ok(attributed) = task.from_application(application);
  let Ok(trashing) = attributed.then(|answer| match &answer.status {
    ExitStatus::Success => Ok(None),
    ExitStatus::Failure(_) | ExitStatus::Interrupt => {
      console_core_logging::warn!(
        "Music Library could not move the track to Trash: {}",
        answer.error_output,
      );
      let Ok(note) = Notification::of("Music Library could not move the track to Trash");

      Ok(Some(note))
    }
  });
  showing.run(trashing);

  Ok(())
}
