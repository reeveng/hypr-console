//! Search completions interleaved while the real downloads machine is running.

use std::error::Error;

use console_application_downloads::looking::Looked;
use console_application_downloads::standing::{
  Downloads, DownloadsEffect, DownloadsEvent, SearchFailure, SearchId,
};
use console_application_downloads::store::Kind;
use console_core_never::Never;
use console_core_state_machine::{interleavings, run, run_from};
use console_program_contract::{Effect, Event};

fn typed(tab: u32, word: &str) -> Result<Event<DownloadsEvent>, Never> {
  Ok(Event::Custom(DownloadsEvent::Typed {
    tab,
    word: word.to_string(),
  }))
}

fn search(tab: u32, kind: Kind) -> Result<Event<DownloadsEvent>, Never> {
  Ok(Event::Custom(DownloadsEvent::LookFor { tab, kind }))
}

fn landed(tab: u32, id: SearchId, asked: &str) -> Result<Event<DownloadsEvent>, Never> {
  Ok(Event::Custom(DownloadsEvent::Landed {
    tab,
    id,
    result: Ok(Looked {
      asked: asked.to_string(),
      ..Looked::default()
    }),
  }))
}

#[test]
fn independent_tabs_keep_their_results_under_every_delivery_order() -> Result<(), Box<dyn Error>>
{
  let Ok(audio_typed) = typed(0, "a song");
  let Ok(audio_search) = search(0, Kind::Sound);
  let Ok(audio_done) = landed(0, SearchId(1), "a song");
  let Ok(video_typed) = typed(1, "a film");
  let Ok(video_search) = search(1, Kind::Film);
  let Ok(video_done) = landed(1, SearchId(1), "a film");
  let audio = [audio_typed, audio_search, audio_done];
  let video = [video_typed, video_search, video_done];
  let schedules = interleavings(&[&audio, &video], 20)?;

  assert_eq!(schedules.len(), 20);

  for requests in schedules {
    let trace = run::<Downloads>(&(), &requests)?;
    let song = trace.state.at(0)?;
    let film = trace.state.at(1)?;

    assert_eq!(song.asking, None, "{requests:?}");
    assert_eq!(film.asking, None, "{requests:?}");
    assert_eq!(
      song.result.as_ref().map(|result| result.asked.as_str()),
      Some("a song")
    );
    assert_eq!(
      film.result.as_ref().map(|result| result.asked.as_str()),
      Some("a film")
    );
  }

  Ok(())
}

#[test]
fn a_stale_attempt_cannot_complete_or_replace_a_repeated_search() -> Result<(), Box<dyn Error>> {
  let Ok(typed) = typed(0, "song");
  let Ok(search) = search(0, Kind::Sound);
  let Ok(stale) = landed(0, SearchId(1), "song");
  let Ok(current) = landed(0, SearchId(2), "song");
  let prefix = [typed, search.clone(), search];
  let started = run::<Downloads>(&(), &prefix)?;
  let effects = started.effects()?;
  assert!(effects.contains(&Effect::Custom(DownloadsEffect::Cancel {
    tab: 0,
    id: SearchId(1)
  })));

  let late = run_from::<Downloads>(started.state, &[stale.clone()])?;
  let tab = late.state.at(0)?;
  assert_eq!(tab.asking.map(|query| query.id), Some(SearchId(2)));
  assert_eq!(tab.result, None);
  let late_effects = late.effects()?;
  assert!(late_effects.is_empty());

  let completed = run_from::<Downloads>(late.state, &[current])?;
  let done = completed.state.at(0)?;
  assert_eq!(done.asking, None);
  let completed_effects = completed.effects()?;
  assert!(
    completed_effects
      .iter()
      .any(|effect| matches!(effect, Effect::Custom(DownloadsEffect::Retain(_))))
  );

  let duplicate = run_from::<Downloads>(completed.state.clone(), &[stale])?;
  assert_eq!(duplicate.state, completed.state);
  let duplicate_effects = duplicate.effects()?;
  assert!(duplicate_effects.is_empty());

  Ok(())
}

#[test]
fn a_failed_search_clears_progress_and_can_be_retried() -> Result<(), Box<dyn Error>> {
  let Ok(typed) = typed(0, "song");
  let Ok(search) = search(0, Kind::Sound);
  let trace = run::<Downloads>(
    &(),
    &[
      typed,
      search.clone(),
      Event::Custom(DownloadsEvent::Landed {
        tab: 0,
        id: SearchId(1),
        result: Err(SearchFailure::InvalidReply),
      }),
    ],
  )?;
  let tab = trace.state.at(0)?;
  assert_eq!(tab.asking, None);
  assert!(tab.result.is_some_and(|looked| !looked.fault.is_empty()));
  let effects = trace.on(2)?;
  assert_eq!(
    effects,
    Some([Effect::Custom(DownloadsEffect::Refresh)].as_slice())
  );

  let retry = run_from::<Downloads>(trace.state, &[search])?;
  let retried = retry.state.at(0)?;
  assert_eq!(retried.asking.map(|query| query.id), Some(SearchId(2)));

  Ok(())
}

#[test]
fn the_scoped_finder_returns_its_result_without_touching_the_shared_cache()
-> Result<(), Box<dyn Error>> {
  use std::{fs, os::unix::fs::PermissionsExt, process::Command};

  let area = console_core_temporary_directories::fresh("finder-reply")?;
  let stub = area.join("yt-dlp");
  console_core_atomic_writes::whole_with_permissions(
    &stub,
    b"#!/bin/sh\nprintf '%s\\n' '{\"entries\":[]}'\n",
    fs::Permissions::from_mode(0o700),
  )?;
  let cache = area.join("cache");
  let output = |args: &[&str]| {
    Command::new(env!("CARGO_BIN_EXE_downloads-find"))
      .env("PATH", &area)
      .env("XDG_CACHE_HOME", &cache)
      .args(args)
      .output()
  };
  let cached = console_application_downloads::store::found_at(&cache, Kind::Sound)?;
  let old = output(&["--audio", "--", "old"])?;
  assert!(old.status.success());
  let before = fs::read_to_string(&cached)?;

  let scratch = area.join("scratch");
  fs::create_dir(&scratch)?;
  let scratch_2 = scratch.to_str().ok_or("scratch path is not UTF-8")?;
  let result = output(&["--audio", "--report", scratch_2, "--", "new"])?;
  assert!(result.status.success(), "{result:?}");
  let stdout = String::from_utf8(result.stdout)?;
  let looked = console_application_downloads::looking::parse(&stdout)?;
  assert_eq!(looked.asked, "new");
  let after = fs::read_to_string(cached)?;
  assert_eq!(after, before);

  Ok(())
}
