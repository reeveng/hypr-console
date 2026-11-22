//! What nothing in this process reads yet, handed to ffmpeg.
//!
//! HEIF and AVIF, which is what a phone camera writes, and TIFF, are still
//! read by ffmpeg, as is a film in a container the film reader does not open.
//! They were all read that way until the decoders in this crate and in
//! `console-films` took over everything else, and this is what is
//! left of that path: ffprobe says how large the picture is, and ffmpeg is
//! handed the exact width and height to draw it at, so the run that comes back
//! is the size this crate already said it would be and a short one is a fault.
//! It goes when a HEIF item reader and a TIFF reader are written here.

use std::path::Path;
use std::process::Stdio;

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_shapes::Pixels;

use console_core_pixel_buffers::BYTES_PER_PIXEL;

use crate::{PictureError, fit_within};

pub(crate) fn measured(at: &Path) -> Result<Option<Size<u32>>, PictureError> {
  let Ok(mut asking) = Program::Ffprobe.command();

  let said = asking
    .args([
      "-v",
      "error",
      "-select_streams",
      "v:0",
      "-show_entries",
      "stream=width,height",
    ])
    .args(["-of", "csv=p=0"])
    .arg(at)
    .output()
    .map_err(|fault| PictureError::Query("ffprobe", fault))?;

  match ::core::primitive::bool::then_some(said.status.success(), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(None),
  }

  let stdout = match std::str::from_utf8(&said.stdout) {
    Ok(text) => text,
    Err(fault) => {
      return Err(PictureError::Unmeasured(
        at.to_path_buf(),
        fault.to_string(),
      ));
    }
  };
  let printed = stdout.trim();

  let (wide, tall) = match (printed.split_once(','), printed.is_empty()) {
    (Some((wide, tall)), _) => (wide.trim(), tall.trim()),
    (None, true) => return Ok(None),
    (None, false) => {
      return Err(PictureError::Unmeasured(
        at.to_path_buf(),
        printed.to_string(),
      ));
    }
  };

  match (wide.parse::<u32>(), tall.parse::<u32>()) {
    (Ok(wide_2), Ok(tall_2)) => Ok(Some(Size {
      width: wide_2,
      height: tall_2,
    })),
    (Err(_), _) | (_, Err(_)) => Err(PictureError::Unmeasured(
      at.to_path_buf(),
      printed.to_string(),
    )),
  }
}

pub(crate) fn fitted(at: &Path, within: Size<u32>) -> Result<Option<Pixels>, PictureError> {
  let measured = measured(at)?;

  let had = match measured {
    Some(had) => had,
    None => return Ok(None),
  };

  let Ok(made) = fit_within(had, within);
  let Size {
    width: wide,
    height: tall,
  } = made;
  let drawn = drawn(at, &format!("scale={wide}:{tall}"))?;

  match drawn {
    Some(bytes) => pixels(at, (made, bytes)),
    None => Ok(None),
  }
}

pub(crate) fn squared(at: &Path, size: Size<u32>) -> Result<Option<Pixels>, PictureError> {
  let measured = measured(at)?;

  let had = match measured {
    Some(had) => had,
    None => return Ok(None),
  };

  let side = had.width.min(had.height);
  let left = had.width.saturating_sub(side).saturating_div(2);
  let top = had.height.saturating_sub(side).saturating_div(2);
  let Size {
    width: wide,
    height: tall,
  } = size;
  let drawn = drawn(
    at,
    &format!("crop={side}:{side}:{left}:{top},scale={wide}:{tall}"),
  )?;

  match drawn {
    Some(bytes) => pixels(at, (size, bytes)),
    None => Ok(None),
  }
}

fn drawn(at: &Path, filter: &str) -> Result<Option<Vec<u8>>, PictureError> {
  let Ok(mut asking) = Program::Ffmpeg.command();

  let said = asking
    .args(["-v", "error", "-i"])
    .arg(at)
    .args(["-vf", filter, "-frames:v", "1"])
    .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
    .stderr(Stdio::piped())
    .output()
    .map_err(|fault| PictureError::Query("ffmpeg", fault))?;

  Ok(match ::core::primitive::bool::then_some(said.status.success(), ()) {
    ::core::option::Option::Some(()) => Some(said.stdout),
    ::core::option::Option::None => None,
  })
}

fn pixels(at: &Path, drawn: (Size<u32>, Vec<u8>)) -> Result<Option<Pixels>, PictureError> {
  let (made, bytes) = drawn;
  let wanted = u64::from(made.width)
    .saturating_mul(u64::from(BYTES_PER_PIXEL))
    .saturating_mul(u64::from(made.height));
  let Ok(got) = console_core_number_conversion::fitted::<_, u64>(bytes.len());

  match ::core::primitive::bool::then_some(got == wanted, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(PictureError::Short {
        of: at.to_path_buf(),
        wanted,
        got,
      });
    }
  }

  let Ok(pixels) = Pixels::packed(made, bytes);

  Ok(Some(pixels))
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_never::Never;

  #[test]
  fn a_file_ffmpeg_cannot_read_is_no_picture() -> Result<(), Never> {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let measured = measured(&at);

    assert!(matches!(measured, Ok(None)), "{measured:?}");

    Ok(())
  }
}
