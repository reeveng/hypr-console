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


use console_external_programs::Program;
use console_never::Never;
use console_number_conversion::{Float, fitted, whole_u32, whole_usize};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::webp::{self, Frame};

use crate::loops::{self, Patch};

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct Stir {
    pub rest_seconds: f64,
    pub seconds: f64,
    pub frames_per_second: u32,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loops {
    OverAndOver,
    AndRests,
}

impl Stir {
    pub fn loops(&self) -> Result<Loops, Never> {
        Ok(match self.rest_seconds <= 0.0 {
            true => Loops::OverAndOver,
            false => Loops::AndRests,
        })
    }

    pub fn frames(&self) -> Result<usize, Never> {
        whole_usize(self.seconds * f64::from(self.frames_per_second))
    }

    pub fn each_milliseconds(&self) -> Result<u32, Never> {
        whole_u32(1000.0 / f64::from(self.frames_per_second))
    }

    pub fn opening_milliseconds(&self) -> Result<u32, Never> {
        let loops = self.loops()?;

        match loops {
            Loops::OverAndOver => self.each_milliseconds(),
            Loops::AndRests => whole_u32(self.rest_seconds * 1000.0),
        }
    }
}

pub struct Pressed {
    pub animation: Vec<u8>,
    pub still: Vec<u8>,
    pub slice: (usize, usize),
    pub largest: f64,
}

fn decoding(
    source: &Path,
    cube: &Path,
    size: (u32, u32),
    stir: &Stir,
    slice: Option<(usize, usize)>,
) -> Result<Command, Never> {
    let filter = format!(
        "fps={fps},scale={wide}:{tall}:force_original_aspect_ratio=increase,crop={wide}:{tall},lut3d='{cube}'",
        fps = stir.frames_per_second,
        wide = size.0,
        tall = size.1,
        cube = cube.display()
    );
    let filter = match slice {
        Some((from, to)) => format!("{filter},select='between(n\\,{from}\\,{to})',setpts=N/TB"),
        None => filter,
    };

    let Ok(mut asking) = Program::Ffmpeg.command();

    asking
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(source)
        .args(["-vf", &filter, "-fps_mode", "passthrough"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"]);

    Ok(asking)
}

fn each_frame(
    source: &Path,
    cube: &Path,
    size: (u32, u32),
    stir: &Stir,
    slice: Option<(usize, usize)>,
    mut take: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<usize, String> {
    let Ok(mut decoding) = decoding(source, cube, size, stir, slice);

    let mut ffmpeg = decoding
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|fault| format!("ffmpeg would not run: {fault}"))?;
    let mut pipe = ffmpeg.stdout.take().ok_or("ffmpeg gave no pipe")?;

    let Ok(room) = fitted(size.0.saturating_mul(size.1).saturating_mul(3));

    let mut frame = vec![0u8; room];
    let mut count: usize = 0;

    loop {
        match pipe.read_exact(&mut frame) {
            Ok(()) => {
                take(&frame)?;
                count = count.saturating_add(1);
            }
            Err(fault) if fault.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(fault) => return Err(format!("ffmpeg stopped talking: {fault}")),
        }
    }

    let done = ffmpeg
        .wait_with_output()
        .map_err(|fault| format!("ffmpeg would not finish: {fault}"))?;

    match done.status.success() {
        true => {},
        false => {
            return Err(format!(
                "ffmpeg refused {}: {}",
                source.display(),
                String::from_utf8_lossy(&done.stderr).trim()
            ));
        }
    }

    match count {
        0 => Err(format!("{} decoded to nothing", source.display())),
        _ => Ok(count),
    }
}

fn encode(pixels: &[u8], size: (u32, u32), quality: u32) -> Result<Vec<u8>, String> {
    let Ok(mut starting) = Program::Ffmpeg.command();

    let mut ffmpeg = starting
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
    let mut taking = ffmpeg.stdin.take().ok_or("ffmpeg took no pipe")?;

    taking
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

fn slice(source: &Path, cube: &Path, stir: &Stir) -> Result<(usize, usize), String> {
    const LOOKING: (u32, u32) = (240, 150);

    let mut small = Vec::new();
    let count = each_frame(source, cube, LOOKING, stir, None, |frame| {
        small.push(frame.to_vec());
        Ok(())
    })?;

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
        let wrote = match &before {
            None => {
                let picture = encode(frame, size, stir.quality)?;

                let Ok(opening) = stir.opening_milliseconds();

                let Ok(width) = fitted(size.0);
                let Ok(height) = fitted(size.1);

                written.push(Frame {
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
                let Ok(moved) = loops::changed(was, frame, size.0, stir.tolerance);

                match moved {
                    None => false,
                    Some(patch) => {
                        let Ok(cut) = loops::cut(frame, size.0, &patch);

                        let picture = encode(&cut, (patch.wide, patch.tall), stir.quality)?;

                        let Ok(area) = patch.area();

                        let Ok(each) = stir.each_milliseconds();

                        let Ok(x) = fitted(patch.x);
                        let Ok(y) = fitted(patch.y);
                        let Ok(width) = fitted(patch.wide);
                        let Ok(height) = fitted(patch.tall);

                        largest = largest.max(area);
                        written.push(Frame {
                            x,
                            y,
                            width,
                            height,
                            milliseconds: each.saturating_add(std::mem::take(&mut carried)),
                            picture,
                        });
                        true
                    }
                }
            }
        };

        let Ok(each) = stir.each_milliseconds();

        match wrote {
            true => {},
            false => carried = carried.saturating_add(each),
        }

        before = Some(frame.to_vec());
        Ok(())
    })?;

    match written.first_mut() {
        Some(first) => first.milliseconds = first.milliseconds.saturating_add(carried),
        None => {},
    }

    let Ok(wide) = fitted(size.0);
    let Ok(tall) = fitted(size.1);

    let animation = webp::animation(wide, tall, &written)?;
    let still = written
        .first()
        .map(|frame| frame.picture.clone())
        .ok_or("nothing was pressed")?;

    let Ok(whole) = Patch::whole(size.0, size.1);

    let Ok(area) = whole.area();

    let Ok(most) = largest.float();
    let Ok(whole) = area.float();

    Ok(Pressed { animation, still, slice, largest: most / whole })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stir_is_as_many_frames_as_its_length_and_rate_make() {
        let stir = Stir { seconds: 4.0, frames_per_second: 12, ..Stir::default() };
        assert_eq!(stir.frames(), Ok(48));
        assert_eq!(stir.each_milliseconds(), Ok(83));
    }

    #[test]
    fn a_picture_that_does_not_rest_loops() {
        let stir = Stir { rest_seconds: 0.0, ..Stir::default() };
        assert_eq!(stir.loops(), Ok(Loops::OverAndOver));
        assert_eq!(stir.opening_milliseconds(), stir.each_milliseconds());
    }

    #[test]
    fn a_rest_is_a_frame_duration_like_any_other() {
        let stir = Stir { rest_seconds: 90.0, ..Stir::default() };
        assert_eq!(stir.loops(), Ok(Loops::AndRests));
        assert_eq!(stir.opening_milliseconds(), Ok(90_000));
    }

    #[test]
    fn a_rest_below_nothing_is_no_rest() {
        assert_eq!(Stir { rest_seconds: -1.0, ..Stir::default() }.loops(), Ok(Loops::OverAndOver));
    }
}
