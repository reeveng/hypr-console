//! Adler-32, which zlib closes every stream with.
//!
//! A PNG's pixels are one zlib stream, and a zlib stream ends with the
//! Adler-32 of what it unpacks to. It is two sums kept modulo 65521, the
//! largest prime under sixty-four thousand: one of the bytes and one of the
//! first sum after each byte. Both are taken modulo once every 5552 bytes
//! rather than after every byte, which is as many as can be added before the
//! second sum could pass what thirty-two bits hold, and is zlib's own figure.
//! `Wikipedia` checks to `0x11E60398`, which is the vector the format's own
//! article gives.

use console_core_never::Never;
use console_core_number_conversion::index;

const MODULUS: u32 = 65_521;

const BEFORE_IT_COULD_OVERFLOW: u32 = 5552;

pub fn of(bytes: &[u8]) -> Result<u32, Never> {
    let Ok(run) = index(BEFORE_IT_COULD_OVERFLOW);

    let (low, high) = bytes.chunks(run).fold((1u32, 0u32), |(low, high), run| {
        let (low, high) = run.iter().fold((low, high), |(low, high), byte| {
            let low = low.wrapping_add(u32::from(*byte));

            (low, high.wrapping_add(low))
        });

        (low.rem_euclid(MODULUS), high.rem_euclid(MODULUS))
    });

    Ok(high.wrapping_shl(16) | low)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_published_vector_is_the_one_this_answers() {
        assert_eq!(of(b"Wikipedia"), Ok(0x11E6_0398));
    }

    #[test]
    fn nothing_at_all_checks_to_one() {
        assert_eq!(of(b""), Ok(1));
    }

    #[test]
    fn a_long_run_of_the_largest_byte_is_taken_modulo_before_it_overflows() {
        let ones = vec![0xFFu8; 100_000];
        let slow = ones.iter().fold((1u64, 0u64), |(low, high), byte| {
            let low = low.wrapping_add(u64::from(*byte)).rem_euclid(65_521);

            (low, high.wrapping_add(low).rem_euclid(65_521))
        });
        let (low, high) = slow;

        assert_eq!(of(&ones).map(u64::from), Ok(high.wrapping_shl(16) | low));
    }
}
