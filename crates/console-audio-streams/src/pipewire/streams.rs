//! Parent-bound output and capture, plus completion of independent recorders.

use std::io;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;

use console_operating_system_linux::program_lifetime::{
  BoundToParent, TimedPipe, alongside, interrupt_and_wait,
};
use rustix::process::Signal;

use super::{Output, media_properties, playback, recording};
use crate::Pcm;

#[derive(Clone, Copy)]
pub enum PlaybackApp {
  Music,
  Viewer,
}

pub struct Media {
  pub app: PlaybackApp,
  pub indexed_gain: Option<f64>,
  pub latency: Duration,
}

pub struct PlaybackStream {
  input: TimedPipe,
  _process: BoundToParent,
}

pub fn play_stream(format: Pcm, media: Media) -> io::Result<PlaybackStream> {
  let Ok(properties) = media_properties(media.app, media.indexed_gain);
  let Ok(mut command) = playback(
    format,
    Output {
      target: None,
      properties: Some(&properties),
      latency: Some(media.latency),
    },
  );
  let mut process = alongside(&mut command)?;
  let Ok(input) = process.take_stdin();
  let stdin = input.ok_or_else(|| io::Error::other("playback stream has no input"))?;
  let input_pipe = TimedPipe::new(OwnedFd::from(stdin))?;

  Ok(PlaybackStream {
    input: input_pipe,
    _process: process,
  })
}

impl PlaybackStream {
  pub fn write_some(&self, samples: &[u8], wait: Duration) -> io::Result<u32> {
    self.input.write_some(samples, wait)
  }
}

pub struct Recording(BoundToParent);

impl Recording {
  pub fn start(format: Pcm, output: &Path) -> io::Result<Self> {
    let Ok(mut command) = recording(format, output);
    let recorder = alongside(&mut command)?;

    Ok(Self(recorder))
  }

  pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
    self.0.try_wait()
  }

  pub fn finish(mut self, wait: Duration) -> io::Result<ExitStatus> {
    self.0.interrupt_and_wait(Signal::INT, wait)
  }
}

pub fn finish_recording(process: BorrowedFd<'_>, wait: Duration) -> io::Result<()> {
  interrupt_and_wait(process, Signal::INT, wait)
}
