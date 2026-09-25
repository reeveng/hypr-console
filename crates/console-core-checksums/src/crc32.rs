//! CRC32, in the one spelling both formats that name it use.
//!
//! A zip entry carries one and so does every chunk of a PNG, which is two
//! callers for the same reflected polynomial. It is the ordinary one --
//! `0xEDB88320`, bits in from the low end, the register begun and ended
//! inverted -- and `123456789` checks to `0xCBF43926`, which is the vector
//! every implementation of it is tested against.

use console_core_never::Never;

pub fn of(bytes: &[u8]) -> Result<u32, Never> {
    let mut crc = 0xFFFF_FFFFu32;

    for byte in bytes {
        crc ^= u32::from(*byte);

        for _ in 0..8 {
            let odd = crc & 1 == 1;
            crc = crc.wrapping_shr(1);

            crc = match odd {
                true => crc ^ 0xEDB8_8320,
                false => crc,
            };
        }
    }

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
}
