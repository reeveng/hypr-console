use std::vec::Vec;

use console_core_number_conversion::index;

use super::*;

struct Bin(u8);

struct Encoder {
    low: u32,
    range: u32,
    outstanding: u32,
    first: bool,
    bits: Vec<u8>,
}

impl Encoder {
    fn new() -> Result<Self, Never> {
        Ok(Self {
            low: 0,
            range: 510,
            outstanding: 0,
            first: true,
            bits: Vec::new(),
        })
    }

    fn put(&mut self, bit: u8) -> Result<(), Never> {
        match self.first {
            true => self.first = false,
            false => self.bits.push(bit),
        }

        for _pending in 0..self.outstanding {
            self.bits.push(bit ^ 1);
        }

        self.outstanding = 0;

        Ok(())
    }

    fn renormalize(&mut self) -> Result<(), Never> {
        for _shift in 0..8 {
            match self.range >= 256 {
                true => break,
                false => {}
            }

            match (self.low < 256, self.low >= 512) {
                (true, _) => {
                    let Ok(()) = self.put(0);
                }
                (false, true) => {
                    self.low = self.low.wrapping_sub(512);

                    let Ok(()) = self.put(1);
                }
                (false, false) => {
                    self.low = self.low.wrapping_sub(256);
                    self.outstanding = self.outstanding.wrapping_add(1);
                }
            }

            self.range = self.range.wrapping_shl(1);
            self.low = self.low.wrapping_shl(1);
        }

        Ok(())
    }

    #[cfg_attr(
        dylint_lib = "explicit014_no_index_slice",
        allow(
            explicit014_no_index_slice,
            reason = "the reference encoder receives only seven-bit contexts and computes a two-bit range column"
        )
    )]
    fn decision(&mut self, context: &mut u8, bin: Bin) -> Result<(), Never> {
        let state = *context;
        let Ok(row) = index(state.wrapping_shr(1));
        let Ok(column) = index(self.range.wrapping_shr(6) & 3);
        let Ok(at) = index(state);
        let lps = u32::from(RANGE_TAB_LPS[row][column]);

        self.range = self.range.wrapping_sub(lps);

        match bin.0 != state & 1 {
            true => {
                self.low = self.low.wrapping_add(self.range);
                self.range = lps;
                *context = NEXT_STATE[at][1];
            }
            false => *context = NEXT_STATE[at][0],
        }

        self.renormalize()
    }

    fn bypass(&mut self, bin: u8) -> Result<(), Never> {
        self.low = self.low.wrapping_shl(1);

        match bin {
            0 => {}
            1.. => self.low = self.low.wrapping_add(self.range),
        }

        match (self.low >= 1024, self.low < 512) {
            (true, _) => {
                let Ok(()) = self.put(1);

                self.low = self.low.wrapping_sub(1024);
            }
            (false, true) => {
                let Ok(()) = self.put(0);
            }
            (false, false) => {
                self.low = self.low.wrapping_sub(512);
                self.outstanding = self.outstanding.wrapping_add(1);
            }
        }

        Ok(())
    }

    fn terminate_zero(&mut self) -> Result<(), Never> {
        self.range = self.range.wrapping_sub(2);

        self.renormalize()
    }

    fn finish(&mut self) -> Result<Vec<u8>, Never> {
        self.range = self.range.wrapping_sub(2);
        self.low = self.low.wrapping_add(self.range);
        self.range = 2;

        let Ok(()) = self.renormalize();
        let Ok(()) = self.put(u8::from(self.low & 512 != 0));

        self.bits.push(u8::from(self.low & 256 != 0));
        self.bits.push(1);

        for _pad in 0..7 {
            match self.bits.len().is_multiple_of(8) {
                true => break,
                false => self.bits.push(0),
            }
        }

        Ok(self
            .bits
            .chunks(8)
            .map(|chunk| {
                chunk
                    .iter()
                    .fold(0u8, |byte, bit| byte.wrapping_shl(1) | bit)
            })
            .collect())
    }
}

fn next_value(seed: &mut u32) -> Result<u32, Never> {
    *seed ^= seed.wrapping_shl(13);
    *seed ^= seed.wrapping_shr(17);
    *seed ^= seed.wrapping_shl(5);

    Ok(*seed)
}

#[test]
fn mixed_decisions_bypass_and_termination_round_trip_from_every_state() -> Result<(), InvalidOffset>
{
    for initial in 0..128u8 {
        let Ok(mut encoder) = Encoder::new();
        let mut context = initial;
        let mut seed = u32::from(initial).wrapping_add(12345);
        let mut operations = Vec::new();

        for _bin in 0..5000 {
            let Ok(value) = next_value(&mut seed);
            let bin = u8::from(value.wrapping_rem(10) >= 8) ^ (initial & 1);
            let kind = value.wrapping_rem(7);

            match kind {
                0 => {
                    let Ok(()) = encoder.bypass(bin);
                }
                1 => {
                    let Ok(()) = encoder.terminate_zero();
                }
                2.. => {
                    let Ok(()) = encoder.decision(&mut context, Bin(bin));
                }
            }

            operations.push((kind, bin, context));
        }

        let Ok(bytes) = encoder.finish();
        let mut decoder = Decoder::new(&bytes, BytePosition(0))?;
        let mut decoded_context = initial;

        for (kind, bin, state) in operations {
            let Ok(decoded) = match kind {
                0 => decoder.decode_bypass(),
                1 => decoder.decode_terminate(),
                2.. => decoder.decode_decision(&mut decoded_context),
            };

            assert_eq!(
                decoded,
                match kind {
                    1 => 0,
                    _other => u32::from(bin),
                },
                "initial state {initial}"
            );
            assert_eq!(decoded_context, state);
        }

        assert_eq!(decoder.decode_terminate(), Ok(1));
    }

    Ok(())
}

#[test]
fn batched_bypass_matches_single_bins_across_refills() -> Result<(), InvalidOffset> {
    let data = [0x51, 0xA3, 0x75, 0x22, 0xF9, 0x19, 0xCB];

    for count in 0..=255u8 {
        let mut batched = Decoder::new(&data, BytePosition(0))?;
        let mut single = Decoder::new(&data, BytePosition(0))?;

        for start in 0..40u8 {
            let mut context_a = start;
            let mut context_b = start;

            assert_eq!(
                batched.decode_decision(&mut context_a),
                single.decode_decision(&mut context_b)
            );

            let mut expected = 0u32;

            for _bit in 0..count {
                let Ok(bin) = single.decode_bypass();

                expected = expected.wrapping_shl(1) | bin;
            }

            assert_eq!(batched.decode_bypass_bits(count), Ok(expected));
            assert_eq!(batched.bit_position(), single.bit_position());
            assert_eq!(
                (batched.value, batched.range, batched.bits),
                (single.value, single.range, single.bits)
            );
        }
    }

    Ok(())
}

#[test]
fn nonzero_starts_and_pcm_restarts_preserve_bit_positions() -> Result<(), InvalidOffset> {
    let data = [0xFF, 0xFF, 0x00, 0x7C, 0x25, 0xB9, 0x10, 0x52];
    let mut decoder = Decoder::new(&data, BytePosition(2))?;

    assert_eq!(decoder.bit_position(), Ok(BitPosition(25)));
    assert_eq!(decoder.data(), Ok(data.as_slice()));

    let Ok(_bin) = decoder.decode_bypass();

    assert_eq!(decoder.bit_position(), Ok(BitPosition(26)));

    let Ok(()) = decoder.set_byte_position(BytePosition(4));

    decoder.init_engine()?;

    let mut fresh = Decoder::new(&data, BytePosition(4))?;
    let mut context_a = 23;
    let mut context_b = context_a;

    assert_eq!(decoder.bit_position(), Ok(BitPosition(41)));

    for _bin in 0..512 {
        assert_eq!(
            decoder.decode_decision(&mut context_a),
            fresh.decode_decision(&mut context_b)
        );
        assert_eq!(decoder.bit_position(), fresh.bit_position());
    }

    Ok(())
}

#[test]
fn initial_offsets_510_and_511_are_rejected() -> Result<(), InvalidOffset> {
    for last in [0, 0x7F, 0x80, 0xFF] {
        assert!(matches!(
            Decoder::new(&[0xFF, last], BytePosition(0)),
            Err(InvalidOffset)
        ));
    }

    let mut decoder = Decoder::new(&[0, 0, 0xFF, 0], BytePosition(0))?;
    let Ok(()) = decoder.set_byte_position(BytePosition(2));

    assert_eq!(decoder.init_engine(), Err(InvalidOffset));

    Ok(())
}

#[test]
fn short_refills_supply_zeros_and_keep_the_original_overrun_allowance()
-> Result<(), std::boxed::Box<dyn std::error::Error>> {
    let input = [0x12, 0x89, 0xF1, 0x24, 0x65, 0xF0, 0x3D];

    for length in 0..=7u8 {
        let Ok(at) = index(length);
        let data = input.get(..at).ok_or("test length exceeds input")?;
        let mut padded = data.to_vec();

        padded.resize(128, 0);

        let mut short = Decoder::new(data, BytePosition(0))?;
        let mut full = Decoder::new(&padded, BytePosition(0))?;

        for _bin in 0..512 {
            assert_eq!(short.decode_bypass(), full.decode_bypass());
            assert_eq!(short.bit_position(), full.bit_position());
        }

        assert_eq!(short.exhaustion(), Ok(Exhaustion::Overrun));
    }

    let mut extreme = Decoder::new(&[], BytePosition(u64::MAX))?;

    for _bin in 0..128 {
        assert_eq!(extreme.decode_bypass(), Ok(0));
    }

    assert_eq!(extreme.exhaustion(), Ok(Exhaustion::Overrun));

    Ok(())
}

#[test]
fn padding_crosses_the_original_read_ahead_allowance_after_sixteen_loaded_bytes()
-> Result<(), InvalidOffset> {
    let mut decoder = Decoder::new(&[], BytePosition(0))?;

    assert_eq!(decoder.exhaustion(), Ok(Exhaustion::WithinAllowance));

    for _bit in 0..119 {
        assert_eq!(decoder.decode_bypass(), Ok(0));
        assert_eq!(decoder.exhaustion(), Ok(Exhaustion::WithinAllowance));
    }

    assert_eq!(decoder.bit_position(), Ok(BitPosition(128)));
    assert_eq!(decoder.decode_bypass(), Ok(0));
    assert_eq!(decoder.bit_position(), Ok(BitPosition(129)));
    assert_eq!(decoder.exhaustion(), Ok(Exhaustion::Overrun));

    Ok(())
}

#[test]
fn decision_decoding_preserves_the_original_packed_byte_mask() -> Result<(), InvalidOffset> {
    for initial in 0..128u8 {
        let mut plain = initial;
        let mut flagged = initial | 128;
        let mut a = Decoder::new(&[0x49, 0xA5, 0x6D], BytePosition(0))?;
        let mut b = Decoder::new(&[0x49, 0xA5, 0x6D], BytePosition(0))?;

        for _bin in 0..128 {
            assert_eq!(
                a.decode_decision(&mut plain),
                b.decode_decision(&mut flagged)
            );
            assert_eq!(plain, flagged);
        }
    }

    Ok(())
}
