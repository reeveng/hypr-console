//! The container, written out by hand.
//!
//! A WebP animation is a RIFF file holding one frame chunk after another, and
//! each frame chunk carries its own rectangle and its own duration. Neither
//! ffmpeg's muxer nor any tool packaged here will write the durations this
//! needs, so the container is written out here. It is a header and a loop.

use console_core_geometry::Size;
use console_core_never::Never;

use crate::Unpainted;

pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub milliseconds: u32,
    pub picture: Vec<u8>,
}

fn three(value: u32) -> Result<[u8; 3], Never> {
    let [first, second, third, _] = value.to_le_bytes();

    Ok([first, second, third])
}

fn riff_length(bytes: usize) -> Result<u32, Unpainted> {
    u32::try_from(bytes).map_err(|_| Unpainted::ChunkTooBig(bytes))
}

fn side(pixels: i32) -> Result<u32, Unpainted> {
    u32::try_from(pixels).map_err(|_| Unpainted::NotASide(pixels))
}

fn chunk(tag: &[u8; 4], body: &[u8]) -> Result<Vec<u8>, Unpainted> {
    let measured = riff_length(body.len())?;
    let mut out = Vec::with_capacity(body.len().saturating_add(9));
    out.extend_from_slice(tag);
    out.extend_from_slice(&measured.to_le_bytes());
    out.extend_from_slice(body);

    match body.len() & 1 {
        1 => out.push(0),
        _ => {},
    }

    Ok(out)
}

pub fn image_of(single: &[u8]) -> Result<&[u8], Unpainted> {
    let mut at: usize = 12;

    while at.saturating_add(8) <= single.len() {
        let tag = match single.get(at..at.saturating_add(4)) {
            Some(tag) => tag,
            None => return Err(Unpainted::CutShort),
        };

        let said = match single.get(at.saturating_add(4)..at.saturating_add(8)) {
            Some(said) => said,
            None => return Err(Unpainted::CutShort),
        };

        let four: [u8; 4] = said.try_into().map_err(|_| Unpainted::CutShort)?;
        let size = u32::from_le_bytes(four);
        let counted = usize::try_from(size).map_err(|_| Unpainted::ChunkTooLong)?;
        let whole = counted.saturating_add(counted & 1).saturating_add(8);

        match tag == b"VP8 " || tag == b"VP8L" {
            true => {
                return single
                    .get(at..at.saturating_add(whole))
                    .ok_or(Unpainted::CutShort);
            }
            false => {},
        }

        at = at.saturating_add(whole);
    }

    Err(Unpainted::NoPicture)
}

pub fn animation(size: Size<i32>, frames: &[Frame]) -> Result<Vec<u8>, Unpainted> {
    let across = side(size.wide)?;
    let down = side(size.tall)?;

    let Ok(wide) = three(across.saturating_sub(1));

    let Ok(tall) = three(down.saturating_sub(1));

    let mut body = chunk(
        b"VP8X",
        &[[0x02u8, 0, 0, 0].as_slice(), &wide, &tall].concat(),
    )?;
    let anim = chunk(b"ANIM", &[0, 0, 0, 0, 0, 0])?;

    body.extend(anim);

    for frame in frames {
        let x = side(frame.x)?;
        let y = side(frame.y)?;
        let wide = side(frame.width)?;
        let tall = side(frame.height)?;

        let Ok(across) = three(x.saturating_div(2));

        let Ok(down) = three(y.saturating_div(2));

        let Ok(over) = three(wide.saturating_sub(1));

        let Ok(under) = three(tall.saturating_sub(1));

        let Ok(lasting) = three(frame.milliseconds);

        let head = [across.as_slice(), &down, &over, &under, &lasting, &[0b10]].concat();
        let picture = image_of(&frame.picture)?;
        let anmf = chunk(b"ANMF", &[head.as_slice(), picture].concat())?;

        body.extend(anmf);
    }

    let whole = riff_length(body.len().saturating_add(4))?;

    Ok([
        b"RIFF".to_vec(),
        whole.to_le_bytes().to_vec(),
        b"WEBP".to_vec(),
        body,
    ]
    .concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_length_is_three_bytes_little_endian() {
        assert_eq!(three(1), Ok([1, 0, 0]));
        assert_eq!(three(0x0001_0203), Ok([3, 2, 1]));
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
