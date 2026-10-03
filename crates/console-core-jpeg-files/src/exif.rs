//! Which way up a photograph is, from the EXIF a camera writes into it.
//!
//! A phone writes its pixels the way the sensor reads them, whichever way the
//! phone was held, and says in the EXIF how to turn them. A picture drawn
//! without reading that is a portrait lying on its side. Only the one tag is
//! read: IFD0's orientation, in the byte order the TIFF header names, and
//! anything missing or strange in the way of it is a picture as it is stored.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::index;

const ORIENTATION: u16 = 0x0112;

const ENTRY: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Orientation {
    AsStored,
    Mirrored,
    HalfTurn,
    Flipped,
    Transposed,
    QuarterTurnClockwise,
    Transversed,
    QuarterTurnCounterclockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Placing {
    Straight,
    Turned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    Little,
    Big,
}

impl Orientation {
    pub(crate) fn placing(self) -> Result<Placing, Never> {
        Ok(match self {
            Orientation::AsStored => Placing::Straight,
            Orientation::Mirrored
            | Orientation::HalfTurn
            | Orientation::Flipped
            | Orientation::Transposed
            | Orientation::QuarterTurnClockwise
            | Orientation::Transversed
            | Orientation::QuarterTurnCounterclockwise => Placing::Turned,
        })
    }

    pub(crate) fn upright(self, stored: Size<u32>) -> Result<Size<u32>, Never> {
        Ok(match self {
            Orientation::AsStored | Orientation::Mirrored | Orientation::HalfTurn | Orientation::Flipped => stored,
            Orientation::Transposed
            | Orientation::QuarterTurnClockwise
            | Orientation::Transversed
            | Orientation::QuarterTurnCounterclockwise => Size { width: stored.height, height: stored.width },
        })
    }

    pub(crate) fn source(self, upright: Point<u32>, stored: Size<u32>) -> Result<Point<u32>, Never> {
        let Point { x, y } = upright;
        let last = Point { x: stored.width.saturating_sub(1), y: stored.height.saturating_sub(1) };

        Ok(match self {
            Orientation::AsStored => upright,
            Orientation::Mirrored => Point { x: last.x.saturating_sub(x), y },
            Orientation::HalfTurn => Point { x: last.x.saturating_sub(x), y: last.y.saturating_sub(y) },
            Orientation::Flipped => Point { x, y: last.y.saturating_sub(y) },
            Orientation::Transposed => Point { x: y, y: x },
            Orientation::QuarterTurnClockwise => Point { x: y, y: last.y.saturating_sub(x) },
            Orientation::Transversed => Point { x: last.x.saturating_sub(y), y: last.y.saturating_sub(x) },
            Orientation::QuarterTurnCounterclockwise => Point { x: last.x.saturating_sub(y), y: x },
        })
    }
}

fn read_u16(order: Order, bytes: &[u8], at: u32) -> Result<Option<u16>, Never> {
    let Ok(at) = index(at);

    Ok(bytes.get(at..).and_then(<[u8]>::first_chunk::<2>).map(|pair| match order {
        Order::Little => u16::from_le_bytes(*pair),
        Order::Big => u16::from_be_bytes(*pair),
    }))
}

fn read_u32(order: Order, bytes: &[u8], at: u32) -> Result<Option<u32>, Never> {
    let Ok(at) = index(at);

    Ok(bytes.get(at..).and_then(<[u8]>::first_chunk::<4>).map(|four| match order {
        Order::Little => u32::from_le_bytes(*four),
        Order::Big => u32::from_be_bytes(*four),
    }))
}

pub(crate) fn orientation(payload: &[u8]) -> Result<Orientation, Never> {
    let tiff = match payload.strip_prefix(b"Exif\0\0") {
        Some(tiff) => tiff,
        None => return Ok(Orientation::AsStored),
    };

    let order = match tiff.first_chunk::<2>() {
        Some(b"II") => Order::Little,
        Some(b"MM") => Order::Big,
        Some(_) | None => return Ok(Orientation::AsStored),
    };

    let Ok(first) = read_u32(order, tiff, 4);
    let Ok(said) = tagged(order, tiff, first);

    Ok(match said {
        Some(2) => Orientation::Mirrored,
        Some(3) => Orientation::HalfTurn,
        Some(4) => Orientation::Flipped,
        Some(5) => Orientation::Transposed,
        Some(6) => Orientation::QuarterTurnClockwise,
        Some(7) => Orientation::Transversed,
        Some(8) => Orientation::QuarterTurnCounterclockwise,
        Some(_) | None => Orientation::AsStored,
    })
}

fn tagged(order: Order, tiff: &[u8], first: Option<u32>) -> Result<Option<u16>, Never> {
    let first = match first {
        Some(first) => first,
        None => return Ok(None),
    };

    let Ok(count) = read_u16(order, tiff, first);
    let Ok(entries) = index(first.saturating_add(2));
    let Ok(wide) = index(ENTRY);

    let (count, entries) = match (count, tiff.get(entries..)) {
        (Some(count), Some(entries)) => (count, entries),
        (None, _) | (_, None) => return Ok(None),
    };

    let Ok(count) = index(count);

    Ok(entries.chunks_exact(wide).take(count).find_map(|entry| {
        let Ok(tag) = read_u16(order, entry, 0);
        let Ok(value) = read_u16(order, entry, 8);

        match (tag, value) {
            (Some(ORIENTATION), Some(value)) => Some(value),
            (Some(_) | None, _) => None,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exif(tiff: &[u8]) -> Result<Vec<u8>, Never> {
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(tiff);

        Ok(payload)
    }

    #[test]
    fn a_phone_held_on_its_side_says_so_in_either_byte_order() {
        let Ok(little) = exif(&[b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0]);
        let Ok(big) = exif(&[b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 8, 0, 0, 0, 0, 0, 0]);

        assert_eq!(orientation(&little), Ok(Orientation::QuarterTurnClockwise));
        assert_eq!(orientation(&big), Ok(Orientation::QuarterTurnCounterclockwise));
    }

    #[test]
    fn anything_else_is_a_picture_as_it_is_stored() {
        assert_eq!(orientation(b"http://ns.adobe.com/xap/1.0/"), Ok(Orientation::AsStored));
        let Ok(short) = exif(b"II*");
        let Ok(far) = exif(&[b'I', b'I', 42, 0, 200, 0, 0, 0]);

        assert_eq!(orientation(&short), Ok(Orientation::AsStored));
        assert_eq!(orientation(&far), Ok(Orientation::AsStored));
    }

    #[test]
    fn a_quarter_turn_puts_the_left_edge_along_the_top() {
        let stored = Size { width: 4, height: 2 };
        let turned = Orientation::QuarterTurnClockwise;

        assert_eq!(turned.upright(stored), Ok(Size { width: 2, height: 4 }));
        assert_eq!(turned.source(Point { x: 1, y: 0 }, stored), Ok(Point { x: 0, y: 0 }));
        assert_eq!(turned.source(Point { x: 0, y: 0 }, stored), Ok(Point { x: 0, y: 1 }));
        assert_eq!(turned.source(Point { x: 0, y: 3 }, stored), Ok(Point { x: 3, y: 1 }));
    }

    #[test]
    fn every_way_up_reads_each_stored_pixel_exactly_once() {
        let stored = Size { width: 3, height: 2 };
        let ways = [
            Orientation::AsStored,
            Orientation::Mirrored,
            Orientation::HalfTurn,
            Orientation::Flipped,
            Orientation::Transposed,
            Orientation::QuarterTurnClockwise,
            Orientation::Transversed,
            Orientation::QuarterTurnCounterclockwise,
        ];

        for way in ways {
            let Ok(upright) = way.upright(stored);
            let mut read: Vec<Point<u32>> = Vec::new();

            for y in 0..upright.height {
                for x in 0..upright.width {
                    let Ok(from) = way.source(Point { x, y }, stored);

                    read.push(from);
                }
            }

            read.sort_by_key(|point| (point.y, point.x));
            read.dedup();

            assert_eq!(read.len(), 6, "{way:?}");
            assert!(read.iter().all(|point| point.x < 3 && point.y < 2), "{way:?}");
        }
    }
}
