//! MD5, because the thumbnail standard names it and nothing else here does.
//!
//! What names it is not this desktop: the thumbnail managing standard says a
//! picture is filed under the MD5 of the address of the thing it is of, and a
//! picture filed under anything else is one no other file manager finds. So it
//! is arithmetic this tree has to be able to do, and it was being done by
//! `g_compute_checksum_for_string` -- one call into a toolkit that was also
//! drawing the panel, for a hash whose whole definition is in RFC 1321 and has
//! not moved since.
//!
//! The rounds wrap on purpose, which is the one place in this tree where
//! `wrapping_*` is the policy the algorithm states rather than a size being
//! bounded: MD5 is defined as arithmetic modulo two to the thirty-second.

use std::num::NonZeroU32;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const BLOCK: u32 = 64;

const WORDS: NonZeroU32 = match NonZeroU32::new(16) {
    Some(many) => many,
    None => NonZeroU32::MIN,
};

const BLOCKS: NonZeroU32 = match NonZeroU32::new(BLOCK) {
    Some(many) => many,
    None => NonZeroU32::MIN,
};

const STANDING: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];

const TURN: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const SINE: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
    0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
    0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
    0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
    0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
    0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
    0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
    0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
    0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

fn padded(said: &[u8]) -> Result<Vec<u8>, Never> {
    let mut held = said.to_vec();

    held.push(0x80);

    let Ok(long) = fitted::<_, u32>(held.len());
    let tail = long % BLOCKS;
    let want = BLOCK.saturating_sub(8);
    let Ok(zeros) = index(want.saturating_add(BLOCK).saturating_sub(tail) % BLOCKS);

    held.extend(std::iter::repeat_n(0u8, zeros));

    let long = match u64::try_from(said.len()) {
        Ok(long) => long,
        Err(_no_message_here_is_wider_than_this_machine_counts) => u64::MAX,
    };

    let bits = long.wrapping_mul(8);

    held.extend_from_slice(&bits.to_le_bytes());

    Ok(held)
}

fn words(block: &[u8]) -> Result<Vec<u32>, Never> {
    Ok(block
        .chunks_exact(4)
        .map(|four| match four {
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _the_chunk_is_four_wide => 0,
        })
        .collect())
}

fn mixed(round: u32, held: [u32; 4]) -> Result<(u32, u32), Never> {
    let [_a, b, c, d] = held;

    Ok(match round {
        0..=15 => ((b & c) | (!b & d), round),
        16..=31 => ((d & b) | (!d & c), round.wrapping_mul(5).wrapping_add(1) % WORDS),
        32..=47 => (b ^ c ^ d, round.wrapping_mul(3).wrapping_add(5) % WORDS),
        _the_last_sixteen => (c ^ (b | !d), round.wrapping_mul(7) % WORDS),
    })
}

pub fn of(said: &str) -> Result<String, Never> {
    let held = padded(said.as_bytes())?;
    let mut standing = STANDING;
    let Ok(block_wide) = index(BLOCK);

    for block in held.chunks_exact(block_wide) {
        let Ok(word) = words(block);
        let mut turning = standing;

        for round in 0..64 {
            let Ok((mixed, at)) = mixed(round, turning);

            let [a, b, c, d] = turning;
            let Ok(round) = index(round);
            let Ok(at) = index(at);
            let (sine, turn, word) = (SINE.get(round), TURN.get(round), word.get(at));

            let (sine, turn, word) = match (sine, turn, word) {
                (Some(sine), Some(turn), Some(word)) => (*sine, *turn, *word),
                _nothing_is_past_the_end_of_three_tables_of_sixty_four => (0, 0, 0),
            };

            let sum = mixed.wrapping_add(a).wrapping_add(sine).wrapping_add(word);

            turning = [d, b.wrapping_add(sum.rotate_left(turn)), b, c];
        }

        let [a, b, c, d] = turning;
        let [was_a, was_b, was_c, was_d] = standing;

        standing = [
            was_a.wrapping_add(a),
            was_b.wrapping_add(b),
            was_c.wrapping_add(c),
            was_d.wrapping_add(d),
        ];
    }

    let mut written = String::new();

    for number in standing {
        for byte in number.to_le_bytes() {
            written.push_str(&format!("{byte:02x}"));
        }
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vectors_rfc_1321_ends_with() {
        assert_eq!(of(""), Ok("d41d8cd98f00b204e9800998ecf8427e".to_string()));
        assert_eq!(of("a"), Ok("0cc175b9c0f1b6a831c399e269772661".to_string()));
        assert_eq!(of("abc"), Ok("900150983cd24fb0d6963f7d28e17f72".to_string()));
        assert_eq!(of("message digest"), Ok("f96b697d7cb7938d525a2f31aaf161d0".to_string()));
        assert_eq!(
            of("abcdefghijklmnopqrstuvwxyz"),
            Ok("c3fcd3d76192e4007dfb496cca67e13b".to_string())
        );
        assert_eq!(
            of("12345678901234567890123456789012345678901234567890123456789012345678901234567890"),
            Ok("57edf4a22be3c955ac49da2e2107b67a".to_string())
        );
    }

    #[test]
    fn a_message_that_ends_where_the_length_goes_takes_a_block_of_its_own() {
        assert_eq!(
            of("12345678901234567890123456789012345678901234567890123"),
            Ok("cc741494163f0816c4c0ab6502553ac9".to_string())
        );
        assert_eq!(
            of("123456789012345678901234567890123456789012345678901234"),
            Ok("f40a0ec3fbf6cf062c9faf3752bd6e6c".to_string())
        );
    }
}
