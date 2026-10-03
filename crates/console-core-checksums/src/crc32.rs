//! CRC32, in the one spelling both formats that name it use.
//!
//! A zip entry carries one and so does every chunk of a PNG, which is two
//! callers for the same reflected polynomial. It is the ordinary one --
//! `0xEDB88320`, bits in from the low end, the register begun and ended
//! inverted -- and `123456789` checks to `0xCBF43926`, which is the vector
//! every implementation of it is tested against.
//!
//! It is read eight bytes at a step, through eight tables of what each byte
//! does to the register from where it stands among the eight, rather than a
//! bit at a time. A PNG of a photograph is tens of megabytes of chunks, every
//! one of them checked, and a bit at a time thirty megabytes took a tenth of a
//! second; this takes a quarter of that. The tables are the polynomial's and
//! the same for every caller, so they are made once, the first time anything
//! is checked.

use std::sync::LazyLock;

use console_core_never::Never;
use console_core_number_conversion::index;

const POLYNOMIAL: u32 = 0xEDB8_8320;

type Table = [u32; 256];

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "the tables are the polynomial's, the same eight kilobytes for every caller of every format, and a caller handed them would be handed the same thing every time"
    )
)]
static TABLES: LazyLock<[Table; 8]> = LazyLock::new(|| {
    let Ok(tables) = tables();

    tables
});

fn tables() -> Result<[Table; 8], Never> {
    let mut first = [0u32; 256];

    for (byte, entry) in (0u32..).zip(first.iter_mut()) {
        *entry = (0..8).fold(byte, |crc, _bit| match crc & 1 {
            1 => crc.wrapping_shr(1) ^ POLYNOMIAL,
            _ => crc.wrapping_shr(1),
        });
    }

    let mut tables = [first; 8];
    let mut before = first;

    for table in tables.iter_mut().skip(1) {
        for (entry, was) in table.iter_mut().zip(before.iter()) {
            let [low, ..] = was.to_le_bytes();
            let Ok(moved) = looked_up(&first, low);

            *entry = was.wrapping_shr(8) ^ moved;
        }

        before = *table;
    }

    Ok(tables)
}

#[inline(always)]
fn looked_up(table: &Table, byte: u8) -> Result<u32, Never> {
    let Ok(at) = index(byte);

    Ok(match table.get(at) {
        Some(entry) => *entry,
        None => 0,
    })
}

pub fn of(bytes: &[u8]) -> Result<u32, Never> {
    let [t0, t1, t2, t3, t4, t5, t6, t7] = &*TABLES;
    let (eights, rest) = bytes.as_chunks::<8>();

    let crc = eights.iter().fold(0xFFFF_FFFFu32, |crc, [a, b, c, d, e, f, g, h]| {
        let [w, x, y, z] = (crc ^ u32::from_le_bytes([*a, *b, *c, *d])).to_le_bytes();
        let Ok(w) = looked_up(t7, w);
        let Ok(x) = looked_up(t6, x);
        let Ok(y) = looked_up(t5, y);
        let Ok(z) = looked_up(t4, z);
        let Ok(e) = looked_up(t3, *e);
        let Ok(f) = looked_up(t2, *f);
        let Ok(g) = looked_up(t1, *g);
        let Ok(h) = looked_up(t0, *h);

        w ^ x ^ y ^ z ^ e ^ f ^ g ^ h
    });

    let crc = rest.iter().fold(crc, |crc, byte| {
        let [low, ..] = crc.to_le_bytes();
        let Ok(moved) = looked_up(t0, low ^ *byte);

        crc.wrapping_shr(8) ^ moved
    });

    Ok(!crc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_published_vector_is_the_one_this_answers() {
        assert_eq!(of(b"123456789"), Ok(0xCBF4_3926));
    }

    #[test]
    fn nothing_at_all_checks_to_nothing() {
        assert_eq!(of(b""), Ok(0));
    }

    #[test]
    fn one_byte_changed_is_a_different_answer() {
        let Ok(one) = of(b"console");
        let Ok(two) = of(b"consolf");

        assert_ne!(one, two);
    }

    #[test]
    fn eight_at_a_time_is_the_same_as_a_bit_at_a_time() {
        let bytes: Vec<u8> = (0u32..1000)
            .map(|at| {
                let [_, _, _, high] = at.wrapping_mul(2_654_435_761).to_le_bytes();

                high
            })
            .collect();

        for cut in [0, 1, 7, 8, 9, 63, 64, 999, 1000] {
            let (piece, _) = bytes.split_at(cut.min(bytes.len()));
            let slow = piece.iter().fold(0xFFFF_FFFFu32, |crc, byte| {
                (0..8).fold(crc ^ u32::from(*byte), |crc, _bit| match crc & 1 {
                    1 => crc.wrapping_shr(1) ^ POLYNOMIAL,
                    _ => crc.wrapping_shr(1),
                })
            });

            assert_eq!(of(piece), Ok(!slow), "{cut}");
        }
    }
}
