//! Turning a source loop into the picture that goes on the screen.
//!
//! ffmpeg does the two things it is good at, decoding and encoding, and
//! everything between them is `loops`. The grade travels as a cube written by
//! `grade`, so the color decision is made in this repository's own terms and
//! then applied at ffmpeg's speed.
//!
//! Frames are taken from ffmpeg one at a time rather than collected. A whole
//! loop at the size of this screen is three gigabytes of raw pixels, and every
//! frame is wanted exactly once: to be compared with the one before it and then
//! never again.
//!
//! Nothing here writes a file. What comes back is the bytes of a picture, and
//! what to do with them is the caller's, because the same render runs against
//! the tree on a laptop and against `/usr/share/backgrounds` on the device.

use console_core_external_programs::Program;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index, whole_u32};
use console_core_size::Size;
use console_operating_system_linux::program_lifetime::{TimedPipe, alongside};
use std::os::fd::OwnedFd;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::Unpainted;
use crate::webp::{self, Frame};

use crate::loops::{self, Patch};
use crate::media_pipe::{FrameRead, Output, read_frame, write_frame};
use crate::units::{Fraction, Seconds};

#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stir {
  pub rest_seconds: Seconds,
  pub seconds: Seconds,
  pub frames_per_second: u32,
  pub tolerance: u8,
  pub quality: u32,
}

impl Default for Stir {
  fn default() -> Self {
    Stir {
      rest_seconds: Seconds(0.0),
      seconds: Seconds(4.0),
      frames_per_second: 12,
      tolerance: 6,
      quality: 82,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loops {
  OverAndOver,
  AndRests,
}

impl Stir {
  pub fn loops(&self) -> Result<Loops, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.rest_seconds.0 <= 0.0, ()) {
        ::core::option::Option::Some(()) => Loops::OverAndOver,
        ::core::option::Option::None => Loops::AndRests,
      },
    )
  }

  pub fn frames(&self) -> Result<u32, Never> {
    whole_u32(self.seconds.0 * f64::from(self.frames_per_second))
  }

  pub fn each_milliseconds(&self) -> Result<u32, Never> {
    whole_u32(1000.0 / f64::from(self.frames_per_second))
  }

  pub fn opening_milliseconds(&self) -> Result<u32, Never> {
    let loops = self.loops()?;

    match loops {
      Loops::OverAndOver => self.each_milliseconds(),
      Loops::AndRests => whole_u32(self.rest_seconds.0 * 1000.0),
    }
  }
}

pub struct Rendered {
  pub animation: Vec<u8>,
  pub still: Vec<u8>,
  pub slice: (u32, u32),
  pub largest: Fraction,
}

fn decoding(
  source: &Path,
  cube: &Path,
  size: Size<u32>,
  stir: &Stir,
  slice: Option<(u32, u32)>,
) -> Result<Command, Never> {
  let filter = format!(
    "fps={fps},scale={wide}:{tall}:force_original_aspect_ratio=increase,crop={wide}:{tall},lut3d='{cube}'",
    fps = stir.frames_per_second,
    wide = size.width,
    tall = size.height,
    cube = cube.display()
  );
  let filter_2 = match slice {
    Some((from, to)) => format!("{filter},select='between(n\\,{from}\\,{to})',setpts=N/TB"),
    None => filter,
  };

  let Ok(mut asking) = Program::Ffmpeg.command();

  asking
    .args(["-hide_banner", "-loglevel", "error", "-i"])
    .arg(source)
    .args(["-vf", &filter_2, "-fps_mode", "passthrough"])
    .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"]);

  Ok(asking)
}

const MAX_FRAME_BYTES: u64 = 1 << 30;

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "the RGB byte count is checked against MAX_FRAME_BYTES before reserving memory"
  )
)]
fn frame_buffer(size: Size<u32>) -> Result<Vec<u8>, Unpainted> {
  let bytes = u64::from(size.width)
    .saturating_mul(u64::from(size.height))
    .saturating_mul(3);

  match (bytes != 0 && bytes <= MAX_FRAME_BYTES).then_some(()) {
    Some(()) => {}
    None => return Err(Unpainted::FrameBytes(bytes)),
  }

  let Ok(room) = index(bytes);
  let mut frame = Vec::new();
  frame
    .try_reserve_exact(room)
    .map_err(Unpainted::FrameMemory)?;
  frame.extend(std::iter::repeat_n(0, room));

  Ok(frame)
}

fn each_frame<T>(
  source: &Path,
  cube: &Path,
  size: Size<u32>,
  stir: &Stir,
  slice: Option<(u32, u32)>,
  into: &mut T,
  take: impl Fn(&mut T, &[u8]) -> Result<(), Unpainted>,
) -> Result<u32, Unpainted> {
  let frame = frame_buffer(size)?;
  let Ok(mut command) = decoding(source, cube, size, stir, slice);
  command.stdout(Stdio::piped()).stderr(Stdio::piped());
  let mut child = alongside(&mut command).map_err(Unpainted::NoFfmpeg)?;
  let mut output = Output::take(&mut child)?;
  let read = console_core_iteration::iterate(
    (&mut output, frame, into, 0_u32),
    |(out, mut pixels, written, count)| {
      Ok(match read_frame(out, &mut pixels) {
        Ok(FrameRead::Whole) => match take(written, &pixels) {
          Ok(()) => Step::Again((out, pixels, written, count.saturating_add(1))),
          Err(fault) => Step::Halt(Err(fault)),
        },
        Ok(FrameRead::Eof) => Step::Halt(Ok(count)),
        Err(fault) => Step::Halt(Err(Unpainted::Stopped(fault))),
      })
    },
  );

  let count = match read {
    Ok(counted) => counted?,
    Err(endless) => return Err(Unpainted::Stopped(std::io::Error::other(endless))),
  };
  let ended = child.wait().map_err(Unpainted::Unfinished)?;

  match ended.success().then_some(()) {
    Some(()) => {}
    None => {
      let Ok(diagnostic) = output.details();
      return Err(Unpainted::Rejected(source.to_path_buf(), diagnostic));
    }
  }

  match count {
    0 => Err(Unpainted::DecodedToNothing(source.to_path_buf())),
    _ => Ok(count),
  }
}

fn encode(pixels: &[u8], size: Size<u32>, quality: u32) -> Result<Vec<u8>, Unpainted> {
  let Ok(mut starting) = Program::Ffmpeg.command();

  starting
    .args([
      "-hide_banner",
      "-loglevel",
      "error",
      "-f",
      "rawvideo",
      "-pix_fmt",
      "rgb24",
      "-s",
      &format!("{}x{}", size.width, size.height),
      "-i",
      "pipe:0",
      "-c:v",
      "libwebp",
      "-quality",
      &quality.to_string(),
      "-frames:v",
      "1",
      "-f",
      "webp",
      "pipe:1",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let mut child = alongside(&mut starting).map_err(Unpainted::NoFfmpeg)?;
  let Ok(stdin) = child.take_stdin();
  let input = stdin.ok_or(Unpainted::NoPipeIn)?;
  let writer = TimedPipe::new(OwnedFd::from(input)).map_err(Unpainted::Untaken)?;
  let mut output = Output::take(&mut child)?;

  write_frame(&writer, pixels).map_err(Unpainted::Untaken)?;
  drop(writer);

  let bytes = encoded_bytes(&mut output)?;
  let status = child.wait().map_err(Unpainted::Unfinished)?;

  match status.success().then_some(()) {
    Some(()) => Ok(bytes),
    None => {
      let Ok(diagnostic) = output.details();
      Err(Unpainted::RefusedAFrame(diagnostic))
    }
  }
}

fn encoded_bytes(output: &mut Output) -> Result<Vec<u8>, Unpainted> {
  let collected = console_core_iteration::iterate((output, Vec::new()), |(out, mut bytes)| {
    let mut incoming = [0; 8192];
    let observed = match out.read(&mut incoming) {
      Ok(0) => return Ok(Step::Halt(Ok(bytes))),
      Ok(observed) => observed,
      Err(fault) => return Ok(Step::Halt(Err(Unpainted::Stopped(fault)))),
    };
    let Ok(amount) = index(observed);
    let Ok(held) = fitted::<_, u64>(bytes.len());

    match (held.saturating_add(u64::from(observed)) > MAX_FRAME_BYTES).then_some(()) {
      Some(()) => {
        return Ok(Step::Halt(Err(Unpainted::FrameBytes(
          held.saturating_add(u64::from(observed)),
        ))));
      }
      None => {}
    }

    match incoming.get(..amount) {
      Some(part) => bytes.extend_from_slice(part),
      None => return Ok(Step::Halt(Err(Unpainted::CutShort))),
    }

    Ok(Step::Again((out, bytes)))
  });

  match collected {
    Ok(collected_2) => collected_2,
    Err(endless) => Err(Unpainted::Stopped(std::io::Error::other(endless))),
  }
}

fn slice(source: &Path, cube: &Path, stir: &Stir) -> Result<(u32, u32), Unpainted> {
  const LOOKING: Size<u32> = Size {
    width: 240,
    height: 150,
  };

  let mut small = Vec::new();
  let count = each_frame(
    source,
    cube,
    LOOKING,
    stir,
    None,
    &mut small,
    |small_2, frame| {
      small_2.push(frame.to_vec());

      Ok(())
    },
  )?;

  let Ok(loops) = stir.loops();

  match loops {
    Loops::OverAndOver => Ok((0, count.saturating_sub(1))),
    Loops::AndRests => {
      let Ok(frames) = stir.frames();

      let Ok(slice) = loops::stir(&small, frames);

      Ok(slice)
    }
  }
}

#[derive(Default)]
struct Rendering {
  written: Vec<Frame>,
  before: Option<Vec<u8>>,
  largest: u64,
  carried: u32,
}

pub fn render(
  source: &Path,
  cube: &Path,
  size: Size<u32>,
  stir: &Stir,
) -> Result<Rendered, Unpainted> {
  let slice = slice(source, cube, stir)?;

  let mut rendering = Rendering::default();

  each_frame(
    source,
    cube,
    size,
    stir,
    Some(slice),
    &mut rendering,
    |rendering_2, frame| {
      let wrote = match &rendering_2.before {
        None => {
          let picture = encode(frame, size, stir.quality)?;

          let Ok(opening) = stir.opening_milliseconds();

          let Ok(width) = fitted(size.width);
          let Ok(height) = fitted(size.height);

          rendering_2.written.push(Frame {
            x: 0,
            y: 0,
            width,
            height,
            milliseconds: opening,
            picture,
          });
          true
        }
        Some(was) => {
          let Ok(moved) = loops::changed(was, frame, size.width, stir.tolerance);

          match moved {
            None => false,
            Some(patch) => {
              let Ok(cut) = loops::cut(frame, size.width, &patch);

              let picture = encode(
                &cut,
                Size {
                  width: patch.width,
                  height: patch.height,
                },
                stir.quality,
              )?;

              let Ok(area) = patch.area();

              let Ok(each) = stir.each_milliseconds();

              let Ok(x) = fitted(patch.x);
              let Ok(y) = fitted(patch.y);
              let Ok(width) = fitted(patch.width);
              let Ok(height) = fitted(patch.height);

              rendering_2.largest = rendering_2.largest.max(area);
              rendering_2.written.push(Frame {
                x,
                y,
                width,
                height,
                milliseconds: each.saturating_add(std::mem::take(&mut rendering_2.carried)),
                picture,
              });
              true
            }
          }
        }
      };

      let Ok(each) = stir.each_milliseconds();

      match ::core::primitive::bool::then_some(wrote, ()) {
        ::core::option::Option::Some(()) => {}
        ::core::option::Option::None => {
          rendering_2.carried = rendering_2.carried.saturating_add(each)
        }
      }

      rendering_2.before = Some(frame.to_vec());

      Ok(())
    },
  )?;

  let Rendering {
    mut written,
    largest,
    carried,
    before: _,
  } = rendering;

  match written.first_mut() {
    Some(first) => first.milliseconds = first.milliseconds.saturating_add(carried),
    None => {}
  }

  let Ok(wide) = fitted(size.width);
  let Ok(tall) = fitted(size.height);

  let animation = webp::animation(
    Size {
      width: wide,
      height: tall,
    },
    &written,
  )?;
  let still = written
    .first()
    .map(|frame| frame.picture.clone())
    .ok_or(Unpainted::NothingRendered)?;

  let Ok(whole) = Patch::whole(size);

  let Ok(area) = whole.area();

  let Ok(most) = largest.float();
  let Ok(whole_2) = area.float();

  Ok(Rendered {
    animation,
    still,
    slice,
    largest: Fraction(most / whole_2),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[ignore = "requires the desktop ffmpeg with the libwebp encoder"]
  fn a_fixture_frame_finishes_after_the_encoder_input_closes() -> Result<(), Unpainted> {
    let encoded = encode(
      &[128; 12],
      console_core_size::Size {
        width: 2,
        height: 2,
      },
      82,
    )?;

    assert!(encoded.starts_with(b"RIFF"));

    Ok(())
  }

  #[test]
  fn a_stir_is_as_many_frames_as_its_length_and_rate_make() {
    let stir = Stir {
      seconds: Seconds(4.0),
      frames_per_second: 12,
      ..Stir::default()
    };
    assert_eq!(stir.frames(), Ok(48));
    assert_eq!(stir.each_milliseconds(), Ok(83));
  }

  #[test]
  fn a_picture_that_does_not_rest_loops() {
    let stir = Stir {
      rest_seconds: Seconds(0.0),
      ..Stir::default()
    };
    assert_eq!(stir.loops(), Ok(Loops::OverAndOver));
    assert_eq!(stir.opening_milliseconds(), stir.each_milliseconds());
  }

  #[test]
  fn a_rest_is_a_frame_duration_like_any_other() {
    let stir = Stir {
      rest_seconds: Seconds(90.0),
      ..Stir::default()
    };
    assert_eq!(stir.loops(), Ok(Loops::AndRests));
    assert_eq!(stir.opening_milliseconds(), Ok(90_000));
  }

  #[test]
  fn a_rest_below_nothing_is_no_rest() {
    assert_eq!(
      Stir {
        rest_seconds: Seconds(-1.0),
        ..Stir::default()
      }
      .loops(),
      Ok(Loops::OverAndOver)
    );
  }
}
