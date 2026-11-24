//! Play interleaved PCM and capture WAV audio through the desktop audio service.
//!
//! Media streams and attached recordings stop when dropped. Independent
//! recordings can be finished from another invocation by process descriptor.

mod cue;
mod pipewire;

pub use cue::{CueError, VolumePreview, play_cue};
pub use pipewire::record_wav;

#[cfg(target_os = "linux")]
pub use pipewire::{
  Media, PlaybackApp, PlaybackStream, Recording, finish_recording, play_stream,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sample {
  I16,
  F32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pcm {
  pub rate_hz: u32,
  pub channels: u32,
  pub sample: Sample,
}
