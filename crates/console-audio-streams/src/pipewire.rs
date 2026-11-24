//! Map audio requests to PipeWire client commands and WirePlumber properties.

use std::fs::File;
use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use console_core_external_programs::Program;
use console_core_state_machine::Never;

use crate::{Pcm, Sample};

#[cfg(target_os = "linux")]
mod streams;
#[cfg(target_os = "linux")]
pub use streams::{Media, PlaybackApp, PlaybackStream, Recording, finish_recording, play_stream};

struct Output<'a> {
  target: Option<&'a str>,
  properties: Option<&'a str>,
  latency: Option<Duration>,
}

const VOLUME_PREVIEW: &str = concat!(
  "{ application.name = \"Volume Preview\" ",
  "console.balance.preview = true ",
  "state.restore-props = false state.default-volume = 1.0 ",
  "state.restore-target = false node.dont-fallback = true ",
  "node.dont-reconnect = true node.dont-move = true }",
);

pub(crate) fn cue(
  format: Pcm,
  target: Option<&str>,
  lock: Option<File>,
) -> Result<Command, Never> {
  let Ok(mut command) = playback(
    format,
    Output {
      target,
      properties: target.map(|_| VOLUME_PREVIEW),
      latency: None,
    },
  );

  match lock {
    Some(reservation) => {
      command.stdout(reservation);
    }
    None => {}
  }

  Ok(command)
}

fn playback(format: Pcm, output: Output<'_>) -> Result<Command, Never> {
  let Ok(mut command) = Program::PwCat.command();

  match output.target {
    Some(target) => {
      command.args(["--target", target]);
    }
    None => {}
  }

  match output.properties {
    Some(properties) => {
      command.args(["--properties", properties]);
    }
    None => {}
  }

  command.args(["--playback", "--raw"]);
  let Ok(()) = pcm(&mut command, format);

  match output.latency {
    Some(latency) => {
      command.args(["--latency", &format!("{}ms", latency.as_millis())]);
    }
    None => {}
  }

  command
    .arg("-")
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null());

  Ok(command)
}

pub fn record_wav(format: Pcm, output: &Path) -> io::Result<Child> {
  let Ok(mut command) = recording(format, output);

  command.spawn()
}

fn recording(format: Pcm, output: &Path) -> Result<Command, Never> {
  let Ok(mut command) = Program::PwRecord.command();
  let Ok(()) = pcm(&mut command, format);

  command
    .arg(output)
    .stdin(Stdio::null())
    .stdout(Stdio::null());

  Ok(command)
}

fn pcm(command: &mut Command, format: Pcm) -> Result<(), Never> {
  let sample = match format.sample {
    Sample::I16 => "s16",
    Sample::F32 => "f32",
  };
  command.args([
    "--format",
    sample,
    "--rate",
    &format.rate_hz.to_string(),
    "--channels",
    &format.channels.to_string(),
  ]);

  Ok(())
}

#[cfg(target_os = "linux")]
fn media_properties(app: crate::PlaybackApp, indexed_gain: Option<f64>) -> Result<String, Never> {
  let identity = match app {
    crate::PlaybackApp::Music => "application.id = org.console.Music application.name = Music",
    crate::PlaybackApp::Viewer => {
      "application.id = org.console.Viewer application.name = \"Media Viewer\""
    }
  };

  Ok(match indexed_gain {
    Some(gain) => format!("{{ {identity} console.balance.indexed-gain = {gain} }}"),
    None => format!("{{ {identity} }}"),
  })
}

#[cfg(all(test, unix))]
mod tests {
  use std::ffi::OsStr;
  use std::os::unix::ffi::OsStrExt;

  use super::*;

  #[test]
  fn pass_the_pcm_format_and_original_recording_path_bytes() {
    let raw = OsStr::from_bytes(b"/run/said-\xff.wav");
    let Ok(command) = recording(
      Pcm {
        rate_hz: 16_000,
        channels: 1,
        sample: Sample::I16,
      },
      Path::new(raw),
    );

    assert!(command.get_args().eq([
      OsStr::new("--format"),
      OsStr::new("s16"),
      OsStr::new("--rate"),
      OsStr::new("16000"),
      OsStr::new("--channels"),
      OsStr::new("1"),
      raw,
    ]));
  }
}
