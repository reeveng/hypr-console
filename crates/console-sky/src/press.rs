//! Turning a source loop into the picture that goes on the screen.
//!
//! ffmpeg does the two things it is good at, decoding and encoding, and
//! everything between them is `loops`. The grade travels as a cube written by
//! `grade`, so the colour decision is made in this repository's own terms and
//! then applied at ffmpeg's speed.
//!
//! Frames are taken from ffmpeg one at a time rather than collected. A whole
//! loop at the size of this screen is three gigabytes of raw pixels, and every
//! frame is wanted exactly once: to be compared with the one before it and then
//! never again.
//!
//! Nothing here writes a file. What comes back is the bytes of a picture, and
//! what to do with them is the caller's, because the same press runs against
//! the tree on a laptop and against `/usr/share/backgrounds` on the device.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use console_garden::scene::Frame;
use console_garden::webp;

use crate::loops::{self, Patch};

/// How the picture moves.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Stir {
    /// How long the still holds before the picture moves, in seconds. Zero, and
    /// the picture loops the artist's whole animation instead, which is what
    /// every picture the machine ships with does: a loop that stops is stranger
    /// to look at than one that does not, and the saving that paid for the stop
    /// is now had by not moving at all while anything is in front of it.
    ///
    /// Above zero it is the old behaviour, kept because a picture whose whole
    /// frame moves costs the same whether anybody wants that or not, and this
    /// is the setting that buys it back.
    pub rest_seconds: f64,
    /// How much of the loop is played when it rests between. Ignored when it
    /// loops, where the whole of the artist's animation is played.
    pub seconds: f64,
    pub frames_per_second: u32,
    /// How much a channel may move before it counts as movement. A lossy codec
    /// never holds a still region perfectly still, and a tolerance of zero
    /// finds every frame changed everywhere.
    pub tolerance: u8,
    pub quality: u32,
}

impl Default for Stir {
    fn default() -> Self {
        Stir {
            rest_seconds: 0.0,
            seconds: 4.0,
            frames_per_second: 12,
            tolerance: 6,
            quality: 82,
        }
    }
}

impl Stir {
    /// Whether the picture plays the whole loop over and over.
    pub fn loops(&self) -> bool {
        self.rest_seconds <= 0.0
    }

    /// How many frames the movement is, when it does not loop.
    pub fn frames(&self) -> usize {
        (self.seconds * f64::from(self.frames_per_second)).round() as usize
    }

    /// How long one of them lasts.
    pub fn each_milliseconds(&self) -> u32 {
        (1000.0 / f64::from(self.frames_per_second)).round() as u32
    }

    /// How long the first frame is on the screen.
    ///
    /// The first frame is the whole picture and every frame after it is a
    /// rectangle laid over it, so this is the one frame whose length says
    /// whether the picture loops or waits.
    pub fn opening_milliseconds(&self) -> u32 {
        match self.loops() {
            true => self.each_milliseconds(),
            false => (self.rest_seconds * 1000.0).round() as u32,
        }
    }
}

/// What a press came out as, for whoever is going to say so.
pub struct Pressed {
    pub animation: Vec<u8>,
    pub still: Vec<u8>,
    /// Which frames of the source the movement was taken from.
    pub slice: (usize, usize),
    /// The share of the picture the largest rectangle covers, which is what
    /// says whether cutting to what moved was worth doing.
    pub largest: f64,
}

/// The ffmpeg that decodes a source, graded and cut to the screen.
fn decoding(
    source: &Path,
    cube: &Path,
    size: (u32, u32),
    stir: &Stir,
    slice: Option<(usize, usize)>,
) -> Command {
    let filter = format!(
        "fps={fps},scale={wide}:{tall}:force_original_aspect_ratio=increase,crop={wide}:{tall},lut3d='{cube}'",
        fps = stir.frames_per_second,
        wide = size.0,
        tall = size.1,
        cube = cube.display()
    );
    // Trimmed by frame rather than by time, because the frame numbers came
    // from a pass at this same frame rate and a seek in seconds lands wherever
    // the nearest keyframe is.
    let filter = match slice {
        Some((from, to)) => format!("{filter},select='between(n\\,{from}\\,{to})',setpts=N/TB"),
        None => filter,
    };
    let mut asking = Command::new("ffmpeg");
    asking
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(source)
        .args(["-vf", &filter, "-fps_mode", "passthrough"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"]);
    asking
}

/// Every frame of a source, handed over one at a time as it is decoded.
///
/// Returns how many there were. A frame is borrowed for the length of the call
/// and the buffer is then written over, which is what keeps this to one frame
/// of memory however long the loop is.
fn each_frame(
    source: &Path,
    cube: &Path,
    size: (u32, u32),
    stir: &Stir,
    slice: Option<(usize, usize)>,
    mut take: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<usize, String> {
    let mut ffmpeg = decoding(source, cube, size, stir, slice)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|fault| format!("ffmpeg would not run: {fault}"))?;
    let mut pipe = ffmpeg.stdout.take().ok_or("ffmpeg gave no pipe")?;

    let mut frame = vec![0u8; (size.0 * size.1 * 3) as usize];
    let mut count = 0;
    loop {
        match pipe.read_exact(&mut frame) {
            Ok(()) => {
                take(&frame)?;
                count += 1;
            }
            // A short read at the end is the last frame going by, not a fault.
            Err(fault) if fault.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(fault) => return Err(format!("ffmpeg stopped talking: {fault}")),
        }
    }

    let done = ffmpeg
        .wait_with_output()
        .map_err(|fault| format!("ffmpeg would not finish: {fault}"))?;
    if !done.status.success() {
        return Err(format!(
            "ffmpeg refused {}: {}",
            source.display(),
            String::from_utf8_lossy(&done.stderr).trim()
        ));
    }
    match count {
        0 => Err(format!("{} decoded to nothing", source.display())),
        _ => Ok(count),
    }
}

/// One rectangle of pixels, as a WebP holding one picture.
fn encode(pixels: &[u8], size: (u32, u32), quality: u32) -> Result<Vec<u8>, String> {
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            &format!("{}x{}", size.0, size.1),
            "-i",
            "pipe:0",
            "-c:v",
            "libwebp",
            "-quality",
            &quality.to_string(),
            "-f",
            "webp",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|fault| format!("ffmpeg would not run: {fault}"))?;
    ffmpeg
        .stdin
        .take()
        .ok_or("ffmpeg took no pipe")?
        .write_all(pixels)
        .map_err(|fault| format!("ffmpeg would not take the picture: {fault}"))?;
    let done = ffmpeg
        .wait_with_output()
        .map_err(|fault| format!("ffmpeg would not finish: {fault}"))?;
    match done.status.success() {
        true => Ok(done.stdout),
        false => Err(format!(
            "ffmpeg refused a frame: {}",
            String::from_utf8_lossy(&done.stderr).trim()
        )),
    }
}

/// Which stretch of the loop to keep.
///
/// The whole of it, when the picture loops, which is the plain answer and the
/// one that needs no thinking about where to cut. When it does not loop the
/// movement has to end somewhere it can jump back to the still without the jump
/// being seen, and that is decided at a size chosen to be cheap rather than
/// accurate, because the question is about whole frames and not about pixels.
fn slice(source: &Path, cube: &Path, stir: &Stir) -> Result<(usize, usize), String> {
    const LOOKING: (u32, u32) = (240, 150);

    let mut small = Vec::new();
    let count = each_frame(source, cube, LOOKING, stir, None, |frame| {
        small.push(frame.to_vec());
        Ok(())
    })?;
    match stir.loops() {
        true => Ok((0, count - 1)),
        false => Ok(loops::stir(&small, stir.frames())),
    }
}

/// A whole source loop, made into the picture that goes on the screen.
///
/// The first frame is the whole picture and everything after it is the
/// rectangle that differs from the frame before, painted over what is already
/// there. A loop wraps back onto that first whole frame, so the wrap repairs
/// the picture exactly however many rectangles were laid over it.
pub fn press(
    source: &Path,
    cube: &Path,
    size: (u32, u32),
    stir: &Stir,
) -> Result<Pressed, String> {
    let slice = slice(source, cube, stir)?;

    let mut written: Vec<Frame> = Vec::new();
    let mut before: Option<Vec<u8>> = None;
    let mut largest = 0u64;
    let mut carried = 0;

    each_frame(source, cube, size, stir, Some(slice), |frame| {
        // A pair of frames with nothing moving between them writes no frame,
        // and the time it would have lasted is carried onto the next one that
        // does. Dropping it instead would leave the picture shorter than the
        // artist drew it and running faster.
        let wrote = match &before {
            None => {
                written.push(Frame {
                    x: 0,
                    y: 0,
                    width: size.0 as i32,
                    height: size.1 as i32,
                    milliseconds: stir.opening_milliseconds(),
                    picture: encode(frame, size, stir.quality)?,
                });
                true
            }
            Some(was) => match loops::changed(was, frame, size.0, stir.tolerance) {
                None => false,
                Some(patch) => {
                    largest = largest.max(patch.area());
                    written.push(Frame {
                        x: patch.x as i32,
                        y: patch.y as i32,
                        width: patch.wide as i32,
                        height: patch.tall as i32,
                        milliseconds: stir.each_milliseconds() + std::mem::take(&mut carried),
                        picture: encode(
                            &loops::cut(frame, size.0, &patch),
                            (patch.wide, patch.tall),
                            stir.quality,
                        )?,
                    });
                    true
                }
            },
        };
        if !wrote {
            carried += stir.each_milliseconds();
        }
        before = Some(frame.to_vec());
        Ok(())
    })?;

    // Whatever was left over at the end joins the first frame, which is where
    // the picture goes next whether it loops or rests.
    if let Some(first) = written.first_mut() {
        first.milliseconds += carried;
    }

    Ok(Pressed {
        animation: webp::animation(size.0 as i32, size.1 as i32, &written)?,
        still: written
            .first()
            .map(|frame| frame.picture.clone())
            .ok_or("nothing was pressed")?,
        slice,
        largest: largest as f64 / Patch::whole(size.0, size.1).area() as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stir_is_as_many_frames_as_its_length_and_rate_make() {
        let stir = Stir { seconds: 4.0, frames_per_second: 12, ..Stir::default() };
        assert_eq!(stir.frames(), 48);
        assert_eq!(stir.each_milliseconds(), 83);
    }

    /// What every picture the machine ships with does. The first frame is one
    /// frame long, so nothing about it says it is the first.
    #[test]
    fn a_picture_that_does_not_rest_loops() {
        let stir = Stir { rest_seconds: 0.0, ..Stir::default() };
        assert!(stir.loops());
        assert_eq!(stir.opening_milliseconds(), stir.each_milliseconds());
    }

    /// The rest is the whole saving when it is asked for, so it is worth one
    /// test saying that it is written in seconds and pressed in milliseconds.
    #[test]
    fn a_rest_is_a_frame_duration_like_any_other() {
        let stir = Stir { rest_seconds: 90.0, ..Stir::default() };
        assert!(!stir.loops());
        assert_eq!(stir.opening_milliseconds(), 90_000);
    }

    /// A negative rest is somebody saying no rest in a different way, and it
    /// must not become a frame lasting a negative number of milliseconds.
    #[test]
    fn a_rest_below_nothing_is_no_rest() {
        assert!(Stir { rest_seconds: -1.0, ..Stir::default() }.loops());
    }
}
