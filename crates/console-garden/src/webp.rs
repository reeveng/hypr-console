//! The container, written out by hand.
//!
//! A WebP animation is a RIFF file holding one frame chunk after another, and
//! each frame chunk carries its own rectangle and its own duration. Neither
//! ffmpeg's muxer nor any tool packaged here will write the durations this
//! needs, so the container is written out here. It is a header and a loop.

use std::io::Write;
use std::process::{Command, Stdio};

use cairo::ImageSurface;

use crate::garden::QUALITY;
use crate::scene::Frame;

/// A three-byte little-endian number, which is what WebP measures in.
fn three(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

/// A length as the number RIFF writes it in.
///
/// RIFF measures every chunk in a `u32`, so a body that does not fit in one
/// cannot be written at all. Nothing drawn here comes near four gigabytes --
/// these are pictures of a handheld's screen -- but it is the file format
/// setting the limit, so it is the file writer that answers for it rather than
/// a cast that quietly wraps.
fn riff_length(bytes: usize) -> Result<u32, String> {
    u32::try_from(bytes).map_err(|_| format!("a chunk of {bytes} bytes is too big for a WebP"))
}

/// A side of a rectangle, as the unsigned number the format measures in.
///
/// Cairo counts a size in `i32` and WebP writes one in three unsigned bytes.
/// A negative is not a size, and this is where that is said rather than where
/// it would otherwise appear: as a very large width, several fields later.
fn side(pixels: i32) -> Result<u32, String> {
    u32::try_from(pixels).map_err(|_| format!("{pixels} is not a size a WebP can hold"))
}

/// A RIFF chunk: a tag, a length, the body, and a pad byte if it is odd.
fn chunk(tag: &[u8; 4], body: &[u8]) -> Result<Vec<u8>, String> {
    let measured = riff_length(body.len())?;
    let mut out = Vec::with_capacity(body.len() + 9);
    out.extend_from_slice(tag);
    out.extend_from_slice(&measured.to_le_bytes());
    out.extend_from_slice(body);

    if body.len() & 1 == 1 {
        out.push(0);
    }

    Ok(out)
}

/// The bare picture chunk out of a WebP holding one picture.
pub fn image_of(single: &[u8]) -> Result<&[u8], String> {
    let mut at = 12;

    while at + 8 <= single.len() {
        let tag = &single[at..at + 4];
        let four: [u8; 4] = single[at + 4..at + 8]
            .try_into()
            .map_err(|_| "that WebP is cut short".to_string())?;
        let size = u32::from_le_bytes(four);
        let counted = usize::try_from(size)
            .map_err(|_| "that WebP holds a chunk longer than this machine can address")?;
        let whole = 8 + counted + (counted & 1);

        if tag == b"VP8 " || tag == b"VP8L" {
            return single
                .get(at..at + whole)
                .ok_or_else(|| "that WebP is cut short".to_string());
        }

        at += whole;
    }

    Err("that WebP holds no picture".to_string())
}

/// Every frame, muxed into one animated WebP.
pub fn animation(width: i32, height: i32, frames: &[Frame]) -> Result<Vec<u8>, String> {
    let mut body = chunk(
        b"VP8X",
        &[
            &[0x02, 0, 0, 0][..],
            &three(side(width)? - 1),
            &three(side(height)? - 1),
        ]
        .concat(),
    )?;
    body.extend(chunk(b"ANIM", &[0, 0, 0, 0, 0, 0])?);

    for frame in frames {
        // Do not blend and do not dispose: each frame paints over what is under
        // it and leaves it there, so a frame that redraws a band of the picture
        // leaves the rest of the picture alone.
        let head = [
            &three(side(frame.x)? / 2)[..],
            &three(side(frame.y)? / 2),
            &three(side(frame.width)? - 1),
            &three(side(frame.height)? - 1),
            &three(frame.milliseconds),
            &[0b10],
        ]
        .concat();

        body.extend(chunk(
            b"ANMF",
            &[&head[..], image_of(&frame.picture)?].concat(),
        )?);
    }

    let whole = riff_length(body.len() + 4)?;

    Ok([
        b"RIFF".to_vec(),
        whole.to_le_bytes().to_vec(),
        b"WEBP".to_vec(),
        body,
    ]
    .concat())
}

/// One picture, encoded on its own.
///
/// ffmpeg is the encoder because libwebp's own tools are not packaged here,
/// and the chunk it writes is exactly the chunk an animation frame is made of.
pub fn encode(surface: &ImageSurface) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    surface
        .write_to_png(&mut png)
        .map_err(|why| format!("the picture would not write out: {why}"))?;

    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            "pipe:0",
            "-c:v",
            "libwebp",
            "-quality",
            &QUALITY.to_string(),
            "-f",
            "webp",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|why| format!("ffmpeg encodes the frames; install it: {why}"))?;
    ffmpeg
        .stdin
        .take()
        .ok_or_else(|| "ffmpeg was given no pipe to read from".to_string())?
        .write_all(&png)
        .map_err(|why| format!("ffmpeg would not take the picture: {why}"))?;
    let done = ffmpeg
        .wait_with_output()
        .map_err(|why| format!("ffmpeg did not finish: {why}"))?;

    if !done.status.success() {
        return Err("ffmpeg refused a frame".to_string());
    }

    Ok(done.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_is_three_bytes_little_endian() {
        assert_eq!(three(1), [1, 0, 0]);
        assert_eq!(three(0x0001_0203), [3, 2, 1]);
    }

    #[test]
    fn an_odd_body_is_padded_and_an_even_one_is_not() {
        assert_eq!(chunk(b"TEST", b"abc").expect("a chunk").len(), 12);
        assert_eq!(chunk(b"TEST", b"abcd").expect("a chunk").len(), 12);
    }

    #[test]
    fn the_picture_is_found_past_the_chunks_in_front_of_it() {
        let single = [
            b"RIFF".to_vec(),
            0u32.to_le_bytes().to_vec(),
            b"WEBP".to_vec(),
            chunk(b"VP8X", &[0; 10]).expect("a chunk"),
            chunk(b"VP8 ", b"a picture").expect("a chunk"),
        ]
        .concat();
        assert_eq!(&image_of(&single).expect("a picture")[8..17], b"a picture");
    }

    #[test]
    fn a_webp_holding_no_picture_says_so() {
        let empty = [
            b"RIFF".to_vec(),
            0u32.to_le_bytes().to_vec(),
            b"WEBP".to_vec(),
        ]
        .concat();
        assert!(image_of(&empty).is_err());
    }
}
