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

/// A RIFF chunk: a tag, a length, the body, and a pad byte if it is odd.
fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 9);
    out.extend_from_slice(tag);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() & 1 == 1 {
        out.push(0);
    }
    out
}

/// The bare picture chunk out of a WebP holding one picture.
pub fn image_of(single: &[u8]) -> Result<&[u8], String> {
    let mut at = 12;
    while at + 8 <= single.len() {
        let tag = &single[at..at + 4];
        let size = u32::from_le_bytes(single[at + 4..at + 8].try_into().expect("four bytes"));
        let whole = 8 + size as usize + (size as usize & 1);
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
            &three(width as u32 - 1),
            &three(height as u32 - 1),
        ]
        .concat(),
    );
    body.extend(chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]));
    for frame in frames {
        // Do not blend and do not dispose: each frame paints over what is under
        // it and leaves it there, so a frame that redraws a band of the picture
        // leaves the rest of the picture alone.
        let head = [
            &three(frame.x as u32 / 2)[..],
            &three(frame.y as u32 / 2),
            &three(frame.width as u32 - 1),
            &three(frame.height as u32 - 1),
            &three(frame.milliseconds),
            &[0b10],
        ]
        .concat();
        body.extend(chunk(
            b"ANMF",
            &[&head[..], image_of(&frame.picture)?].concat(),
        ));
    }
    Ok([
        b"RIFF".to_vec(),
        (body.len() as u32 + 4).to_le_bytes().to_vec(),
        b"WEBP".to_vec(),
        body,
    ]
    .concat())
}

/// One picture, encoded on its own.
///
/// ffmpeg is the encoder because libwebp's own tools are not packaged here,
/// and the chunk it writes is exactly the chunk an animation frame is made of.
pub fn encode(surface: &ImageSurface) -> Vec<u8> {
    let mut png = Vec::new();
    surface
        .write_to_png(&mut png)
        .expect("a picture writes out");

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
        .expect("ffmpeg encodes the frames; install it");
    ffmpeg
        .stdin
        .take()
        .expect("a pipe")
        .write_all(&png)
        .expect("ffmpeg takes the picture");
    let done = ffmpeg.wait_with_output().expect("ffmpeg finishes");
    assert!(done.status.success(), "ffmpeg refused a frame");
    done.stdout
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
        assert_eq!(chunk(b"TEST", b"abc").len(), 12);
        assert_eq!(chunk(b"TEST", b"abcd").len(), 12);
    }

    #[test]
    fn the_picture_is_found_past_the_chunks_in_front_of_it() {
        let single = [
            b"RIFF".to_vec(),
            0u32.to_le_bytes().to_vec(),
            b"WEBP".to_vec(),
            chunk(b"VP8X", &[0; 10]),
            chunk(b"VP8 ", b"a picture"),
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
