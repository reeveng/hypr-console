//! How big a picture is, read off its first few bytes.
//!
//! Only enough of each format to answer the one question Steam's cache asks:
//! whether this file is square.

use console_core_never::Never;

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

    let mut at: usize = 2;

    while at.saturating_add(9) < head.len() {
        let here = head.get(at).copied();
        let next = head.get(at.saturating_add(1)).copied();

        let marker = match (here, next) {
            (Some(0xFF), Some(marker)) => marker,
            (Some(_), Some(_)) | (Some(_), None) | (None, _) => {
                at = at.saturating_add(1);
                continue;
            }
        };

        match marker {
            0xC0..=0xC3 => {
                let height = two(head, at.saturating_add(5))?;

                let height = match height {
                    Some(height) => height,
                    None => return Ok(None),
                };

                let width = two(head, at.saturating_add(7))?;

                let width = match width {
                    Some(width) => width,
                    None => return Ok(None),
                };

                return Ok(Some((width, height)));
            }
            0xD0..=0xD9 => at = at.saturating_add(2),
            _ => {
                let said = two(head, at.saturating_add(2))?;

                let said = match said {
                    Some(said) => said,
                    None => return Ok(None),
                };

                let length = match usize::try_from(said) {
                    Ok(length) => length,
                    Err(_fault) => return Ok(None),
                };

                at = at.saturating_add(length.saturating_add(2));
            }
        }
    }

    Ok(None)
}

fn two(head: &[u8], at: usize) -> Result<Option<u32>, Never> {
    let bytes = match head.get(at..at.saturating_add(2)) {
        Some(bytes) => bytes,
        None => return Ok(None),
    };

    let pair = match bytes.try_into() {
        Ok(pair) => pair,
        Err(_fault) => return Ok(None),
    };

    Ok(Some(u32::from(u16::from_be_bytes(pair))))
}

fn four(head: &[u8], at: usize) -> Result<Option<u32>, Never> {
    let bytes = match head.get(at..at.saturating_add(4)) {
        Some(bytes) => bytes,
        None => return Ok(None),
    };

    let quad = match bytes.try_into() {
        Ok(quad) => quad,
        Err(_fault) => return Ok(None),
    };

    Ok(Some(u32::from_be_bytes(quad)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(answer: Result<T, Never>) -> T {
        let Ok(value) = answer;

        value
    }

    fn a_png(width: u32, height: u32) -> Vec<u8> {
        let mut head = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        head.extend(width.to_be_bytes());
        head.extend(height.to_be_bytes());
        head
    }

    fn a_jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut head = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        head.extend([0xFF, 0xC0, 0x00, 0x11, 0x08]);
        head.extend(height.to_be_bytes());
        head.extend(width.to_be_bytes());
        head.extend([0x03, 0x01, 0x11]);
        head
    }

    #[test]
    fn a_png_says_how_big_it_is_in_its_header() {
        assert_eq!(ok(size(&a_png(256, 256))), Some((256, 256)));
        assert_eq!(ok(size(&a_png(600, 900))), Some((600, 900)));
    }

    #[test]
    fn a_jpeg_says_so_after_whatever_else_it_carries() {
        assert_eq!(ok(size(&a_jpeg(600, 900))), Some((600, 900)));
    }

    #[test]
    fn anything_else_says_nothing_rather_than_guessing() {
        assert_eq!(ok(size(b"<svg></svg>")), None);
        assert_eq!(ok(size(b"")), None);
        assert_eq!(ok(size(&a_png(1, 1)[..12])), None, "cut short");
    }
}
