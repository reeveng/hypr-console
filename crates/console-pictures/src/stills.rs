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
use std::sync::Arc;

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_shapes::Pixels;

use crate::{BYTES_A_PIXEL, PictureError, fit_within};

pub(crate) fn measured(at: &Path) -> Result<Option<Size<u32>>, PictureError> {
    let Ok(mut asking) = Program::Ffprobe.command();

    let said = asking
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height"])
        .args(["-of", "csv=p=0"])
        .arg(at)
        .output()
        .map_err(|fault| PictureError::Query("ffprobe", fault))?;

    match said.status.success() {
        true => {},
        false => return Ok(None),
    }

    let printed = String::from_utf8_lossy(&said.stdout).trim().to_string();

    let (wide, tall) = match (printed.split_once(','), printed.is_empty()) {
        (Some((wide, tall)), _) => (wide.trim().to_string(), tall.trim().to_string()),
        (None, true) => return Ok(None),
        (None, false) => return Err(PictureError::Unmeasured(at.to_path_buf(), printed)),
    };

    match (wide.parse::<u32>(), tall.parse::<u32>()) {
        (Ok(wide), Ok(tall)) => Ok(Some(Size { width: wide, height: tall })),
        (Err(_), _) | (_, Err(_)) => Err(PictureError::Unmeasured(at.to_path_buf(), printed)),
    }
}

pub(crate) fn fitted(at: &Path, within: Size<u32>) -> Result<Option<Pixels>, PictureError> {
    let measured = measured(at)?;

    let had = match measured {
        Some(had) => had,
        None => return Ok(None),
    };

    let Ok(made) = fit_within(had, within);
    let Size { width: wide, height: tall } = made;
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
    let Size { width: wide, height: tall } = size;
    let drawn = drawn(at, &format!("crop={side}:{side}:{left}:{top},scale={wide}:{tall}"))?;

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

    Ok(match said.status.success() {
        true => Some(said.stdout),
        false => None,
    })
}

fn pixels(at: &Path, drawn: (Size<u32>, Vec<u8>)) -> Result<Option<Pixels>, PictureError> {
    let (made, bytes) = drawn;
    let wanted = u64::from(made.width).saturating_mul(u64::from(BYTES_A_PIXEL)).saturating_mul(u64::from(made.height));
    let Ok(got) = console_core_number_conversion::fitted::<_, u64>(bytes.len());

    match got == wanted {
        true => {},
        false => return Err(PictureError::Short { of: at.to_path_buf(), wanted, got }),
    }

    let stride = made.width.saturating_mul(BYTES_A_PIXEL);

    Ok(Some(Pixels { width: made.width, height: made.height, stride, bytes: Arc::new(bytes) }))
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
