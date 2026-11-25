//! Keep the newest volume preview while the current bell drains.
//!
//! The worker releases its player lock under the mailbox lock so a sender
//! cannot leave a request between the last read and the worker's exit.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use console_audio_streams::{VolumePreview, play_cue};
use console_core_arguments::{Flag, Takes};
use console_core_atomic_writes::{publish, read_text};
use console_core_internal_programs::InternalProgram;
use console_core_iteration::{Step, iterate};
use console_program_lifetime::{let_go, threads};

use crate::{CUE_PCM, PlaybackError, SoundEffects, volume_samples};

pub const DRAIN: Flag = Flag {
  spelling: "--drain-volume-previews",
  takes: Takes::None,
  about: "play pending volume previews for this session",
};

struct Mailbox<'a> {
  pending: PathBuf,
  runtime: &'a Path,
  _guard: File,
}

#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "these files are used for advisory locks and never written"
  )
)]
fn lock_file(path: &Path) -> io::Result<File> {
  OpenOptions::new().create(true).append(true).open(path)
}

impl<'a> Mailbox<'a> {
  fn lock(runtime: &'a Path) -> io::Result<Self> {
    let guard = lock_file(&runtime.join("console-volume-preview-mailbox.lock"))?;
    guard.lock()?;

    Ok(Self {
      pending: runtime.join("console-volume-preview.pending"),
      runtime,
      _guard: guard,
    })
  }

  fn reserve_player(&self) -> io::Result<Option<File>> {
    let player = lock_file(&self.runtime.join("console-volume-preview.lock"))?;

    match player.try_lock() {
      Ok(()) => Ok(Some(player)),
      Err(TryLockError::WouldBlock) => Ok(None),
      Err(TryLockError::Error(fault)) => Err(fault),
    }
  }

  fn replace(&self, sink: &str) -> io::Result<()> {
    publish(&self.pending, sink.as_bytes()).map_err(io::Error::other)
  }

  fn take(&self) -> io::Result<Option<String>> {
    match read_text(&self.pending) {
      Ok(sink) => {
        fs::remove_file(&self.pending)?;

        Ok(Some(sink))
      }
      Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
        Some(()) => Ok(None),
        None => Err(fault),
      },
    }
  }
}

pub fn play_volume(sink: &str) -> Result<(), PlaybackError> {
  submit(sink).map_err(PlaybackError::Preview)
}

fn submit(sink: &str) -> io::Result<()> {
  let Ok(runtime) = console_core_places::runtime_or_user();
  let mailbox = Mailbox::lock(&runtime)?;
  mailbox.replace(sink)?;
  let player = mailbox.reserve_player()?;

  match player {
    Some(_reserved_until_launch) => {
      let Ok(mut command) = InternalProgram::SoundEffect.command();
      command
        .arg(DRAIN.spelling)
        .stdin(Stdio::null())
        .stdout(Stdio::null());
      let mut worker = let_go(&mut command)?;
      let Ok(()) = threads::let_go(std::thread::spawn(move || match worker.wait() {
        Ok(_) => {}
        Err(fault) => console_core_logging::warn!("could not reap the volume preview: {fault}"),
      }));
    }
    None => {}
  }

  Ok(())
}

pub fn drain() -> Result<(), PlaybackError> {
  run_player().map_err(PlaybackError::Preview)
}

fn run_player() -> io::Result<()> {
  let Ok(runtime) = console_core_places::runtime_or_user();
  let mailbox = Mailbox::lock(&runtime)?;
  let reservation = mailbox.reserve_player()?;
  let player = match reservation {
    Some(reserved) => reserved,
    None => return Ok(()),
  };
  let drained = iterate((mailbox, player), |(locked, playing)| {
    let sink = match locked.take() {
      Ok(Some(sink)) => sink,
      Ok(None) => {
        drop(playing);

        return Ok(Step::Halt(Ok(())));
      }
      Err(fault) => {
        drop(playing);

        return Ok(Step::Halt(Err(fault)));
      }
    };
    drop(locked);

    match play(&sink, &playing) {
      Ok(()) => {}
      Err(fault) => console_core_logging::warn!("could not play the volume preview: {fault}"),
    }

    Ok(match Mailbox::lock(&runtime) {
      Ok(next) => Step::Again((next, playing)),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })
  .map_err(io::Error::other)?;

  drained
}

fn play(sink: &str, player: &File) -> Result<(), PlaybackError> {
  let Ok(effects) = SoundEffects::current();

  match effects {
    SoundEffects::Off => return Ok(()),
    SoundEffects::On => {}
  }

  let sound = volume_samples().map_err(PlaybackError::Cue)?;
  let lock = player.try_clone().map_err(PlaybackError::Preview)?;

  play_cue(CUE_PCM, &sound, Some(VolumePreview { target: sink, lock }))
    .map_err(PlaybackError::Output)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rapid_requests_keep_only_the_latest_target_during_playback() -> io::Result<()> {
    let runtime =
      console_core_temporary_directories::private("volume-preview").map_err(io::Error::other)?;
    let mailbox = Mailbox::lock(&runtime)?;
    let reserved = mailbox.reserve_player()?;
    let player = reserved.ok_or_else(|| io::Error::other("player busy"))?;
    mailbox.replace("speakers")?;
    let first = mailbox.take()?;
    assert_eq!(first.as_deref(), Some("speakers"));
    drop(mailbox);

    let while_playing = Mailbox::lock(&runtime)?;
    while_playing.replace("speakers")?;
    while_playing.replace("headphones")?;
    let busy = while_playing.reserve_player()?;
    assert!(busy.is_none());
    let latest = while_playing.take()?;
    assert_eq!(latest.as_deref(), Some("headphones"));
    let empty = while_playing.take()?;
    assert_eq!(empty, None);

    drop(player);
    let finished = while_playing.reserve_player()?;
    assert!(finished.is_some());
    drop(finished);
    drop(while_playing);
    fs::remove_dir_all(runtime)
  }
}
