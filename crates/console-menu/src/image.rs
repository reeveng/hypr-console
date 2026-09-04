//! How big a picture is, read off its first few bytes.
//!
//! Only enough of each format to answer the one question Steam's cache asks:
//! whether this file is square.

/// Width and height, or nothing if it is not a format we read.
pub fn size(head: &[u8]) -> Option<(u32, u32)> {
    png(head).or_else(|| jpeg(head))
}

fn png(head: &[u8]) -> Option<(u32, u32)> {
    if head.get(..8)? != b"\x89PNG\r\n\x1a\n" {
        return None;
    }

    Some((four(head, 16)?, four(head, 20)?))
}

/// The first frame header in a JPEG, which is where its size is written.
fn jpeg(head: &[u8]) -> Option<(u32, u32)> {
    if head.get(..2)? != b"\xff\xd8" {
        return None;
    }

    let mut at = 2;

    while at + 9 < head.len() {
        if head[at] != 0xFF {
            at += 1;
            continue;
        }

        let marker = head[at + 1];

        match marker {
            0xC0..=0xC3 => {
                let height = two(head, at + 5)?;
                let width = two(head, at + 7)?;
                return Some((width, height));
            }
            0xD0..=0xD9 => at += 2,
            _ => {
                let Ok(length) = usize::try_from(two(head, at + 2)?) else {
                    return None;
                };

                at += 2 + length;
            }
        }
    }

    None
}

// The slice asked for above is exactly the width of the array below, so neither
// conversion can fail. Both are written out rather than assumed, because the
// two widths are on the same line and nothing but reading it checks they agree.
fn two(head: &[u8], at: usize) -> Option<u32> {
    let Ok(pair) = head.get(at..at + 2)?.try_into() else { return None };

    Some(u32::from(u16::from_be_bytes(pair)))
}

fn four(head: &[u8], at: usize) -> Option<u32> {
    let Ok(quad) = head.get(at..at + 4)?.try_into() else { return None };

    Some(u32::from_be_bytes(quad))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_png(width: u32, height: u32) -> Vec<u8> {
        let mut head = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        head.extend(width.to_be_bytes());
        head.extend(height.to_be_bytes());
        head
    }

    fn a_jpeg(width: u16, height: u16) -> Vec<u8> {
        // A JFIF header of its own, then the frame that carries the size.
        let mut head = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        head.extend([0xFF, 0xC0, 0x00, 0x11, 0x08]);
        head.extend(height.to_be_bytes());
        head.extend(width.to_be_bytes());
        head.extend([0x03, 0x01, 0x11]);
        head
    }

    #[test]
    fn a_png_says_how_big_it_is_in_its_header() {
        assert_eq!(size(&a_png(256, 256)), Some((256, 256)));
        assert_eq!(size(&a_png(600, 900)), Some((600, 900)));
    }

    /// The frame header is past a header of its own, so the markers before it
    /// have to be walked rather than counted.
    #[test]
    fn a_jpeg_says_so_after_whatever_else_it_carries() {
        assert_eq!(size(&a_jpeg(600, 900)), Some((600, 900)));
    }

    #[test]
    fn anything_else_says_nothing_rather_than_guessing() {
        assert_eq!(size(b"<svg></svg>"), None);
        assert_eq!(size(b""), None);
        assert_eq!(size(&a_png(1, 1)[..12]), None, "cut short");
    }
}
