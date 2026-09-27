//! How big a picture is, read off its first few bytes.
//!
//! Only enough of each format to answer the one question Steam's cache asks:
//! whether this file is square.

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub fn size(head: &[u8]) -> Result<Option<(u32, u32)>, Never> {
    let drawn = png(head)?;

    match drawn {
        Some(drawn) => Ok(Some(drawn)),
        None => jpeg(head),
    }
}

fn png(head: &[u8]) -> Result<Option<(u32, u32)>, Never> {
    let magic = match head.get(..8) {
        Some(magic) => magic,
        None => return Ok(None),
    };

    match magic == b"\x89PNG\r\n\x1a\n" {
        true => {},
        false => return Ok(None),
    }

    let width = four(head, 16)?;

    let width = match width {
        Some(width) => width,
        None => return Ok(None),
    };

    let height = four(head, 20)?;

    let height = match height {
        Some(height) => height,
        None => return Ok(None),
    };

    Ok(Some((width, height)))
}

fn jpeg(head: &[u8]) -> Result<Option<(u32, u32)>, Never> {
    let magic = match head.get(..2) {
        Some(magic) => magic,
        None => return Ok(None),
    };

    match magic == b"\xff\xd8" {
        true => {},
        false => return Ok(None),
    }

    Ok(std::iter::successors(Some(Marker::Next(2)), |marker| match marker {
        Marker::Next(at) => {
            let Ok(after) = after(head, *at);

            Some(after)
        }
        Marker::Sized(_) | Marker::Invalid => None,
    })
    .find_map(|marker| match marker {
        Marker::Sized(size) => Some(size),
        Marker::Next(_) | Marker::Invalid => None,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Next(u32),
    Sized((u32, u32)),
    Invalid,
}

fn after(head: &[u8], at: u32) -> Result<Marker, Never> {
    let Ok(long) = fitted::<_, u32>(head.len());

    match at.saturating_add(9) < long {
        true => {},
        false => return Ok(Marker::Invalid),
    }

    let Ok(here) = byte(head, at);
    let Ok(next) = byte(head, at.saturating_add(1));

    let marker = match (here, next) {
        (Some(0xFF), Some(marker)) => marker,
        (Some(_), Some(_)) | (Some(_), None) | (None, _) => return Ok(Marker::Next(at.saturating_add(1))),
    };

    Ok(match marker {
        0xC0..=0xC3 => {
            let Ok(height) = two(head, at.saturating_add(5));
            let Ok(width) = two(head, at.saturating_add(7));

            match (width, height) {
                (Some(width), Some(height)) => Marker::Sized((width, height)),
                (None, _) | (_, None) => Marker::Invalid,
            }
        }
        0xD0..=0xD9 => Marker::Next(at.saturating_add(2)),
        _ => {
            let Ok(said) = two(head, at.saturating_add(2));

            match said {
                Some(said) => Marker::Next(at.saturating_add(said.saturating_add(2))),
                None => Marker::Invalid,
            }
        }
    })
}

fn byte(head: &[u8], at: u32) -> Result<Option<u8>, Never> {
    let Ok(at) = index(at);

    Ok(head.get(at).copied())
}

struct Wide(u32);

fn big_endian(head: &[u8], at: u32, Wide(wide): Wide) -> Result<Option<u32>, Never> {
    let Ok(at) = index(at);
    let Ok(wide) = index(wide);

    Ok(head
        .get(at..at.saturating_add(wide))
        .map(|bytes| bytes.iter().fold(0u32, |read, byte| read.wrapping_shl(8) | u32::from(*byte))))
}

fn two(head: &[u8], at: u32) -> Result<Option<u32>, Never> {
    big_endian(head, at, Wide(2))
}

fn four(head: &[u8], at: u32) -> Result<Option<u32>, Never> {
    big_endian(head, at, Wide(4))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_png((width, height): (u32, u32)) -> Result<Vec<u8>, Never> {
        let mut head = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        head.extend(width.to_be_bytes());
        head.extend(height.to_be_bytes());

        Ok(head)
    }

    fn a_jpeg((width, height): (u16, u16)) -> Result<Vec<u8>, Never> {
        let mut head = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        head.extend([0xFF, 0xC0, 0x00, 0x11, 0x08]);
        head.extend(height.to_be_bytes());
        head.extend(width.to_be_bytes());
        head.extend([0x03, 0x01, 0x11]);

        Ok(head)
    }

    #[test]
    fn a_png_says_how_big_it_is_in_its_header() {
        let Ok(square) = a_png((256, 256));
        let Ok(tall) = a_png((600, 900));

        assert_eq!(size(&square), Ok(Some((256, 256))));
        assert_eq!(size(&tall), Ok(Some((600, 900))));
    }

    #[test]
    fn a_jpeg_says_so_after_whatever_else_it_carries() {
        let Ok(tall) = a_jpeg((600, 900));

        assert_eq!(size(&tall), Ok(Some((600, 900))));
    }

    #[test]
    fn anything_else_says_nothing_rather_than_guessing() {
        assert_eq!(size(b"<svg></svg>"), Ok(None));
        assert_eq!(size(b""), Ok(None));
        let Ok(mut dot) = a_png((1, 1));

        dot.truncate(12);

        assert_eq!(size(&dot), Ok(None), "cut short");
    }
}
