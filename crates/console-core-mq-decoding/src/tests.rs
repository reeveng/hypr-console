use super::*;

const ENCODED: [u8; 30] = [
    0x84, 0xC7, 0x3B, 0xFC, 0xE1, 0xA1, 0x43, 0x04, 0x02, 0x20, 0, 0, 0x41, 0x0D, 0xBB, 0x86, 0xF4,
    0x31, 0x7F, 0xFF, 0x88, 0xFF, 0x37, 0x47, 0x1A, 0xDB, 0x6A, 0xDF, 0xFF, 0xAC,
];

#[test]
fn the_port_preserves_the_decoded_bitstream() {
    let expected = [
        0x00, 0x02, 0x00, 0x51, 0x00, 0x00, 0x00, 0xC0, 0x03, 0x52, 0x87, 0x2A, 0xAA, 0xAA, 0xAA,
        0xAA, 0x82, 0xC0, 0x20, 0x00, 0xFC, 0xD7, 0x9E, 0xF6, 0xBF, 0x7F, 0xED, 0x90, 0x4F, 0x46,
        0xA3, 0xBF,
    ];
    let Ok(mut decoder) = Decoder::new(&ENCODED);
    let mut context = Context::default();

    for byte in expected {
        let mut decoded = 0u32;

        for _bit in 0..8 {
            let Ok(bit) = decoder.read_bit(&mut context);

            decoded = decoded.wrapping_shl(1) | bit;
        }

        assert_eq!(decoded, byte);
    }
}

#[test]
fn stuffed_bytes_and_markers_are_distinct_at_the_boundary() {
    for (next, count, remaining) in [(0x8F, 7, 2), (0x90, 8, 3)] {
        let data = [0xFF, next, 0x80];
        let mut decoder = Decoder {
            data: &data,
            c: 0,
            a: 0x8000,
            shift_count: 0,
        };
        let Ok(()) = decoder.read_byte();

        assert_eq!(decoder.shift_count, count);
        assert_eq!(decoder.data.len(), remaining);
    }
}

#[test]
fn exhausted_input_and_markers_supply_ones_without_consuming_a_marker() {
    for input in [
        [].as_slice(),
        [0xFF].as_slice(),
        [0xFF, 0x90, 0x00].as_slice(),
        [0xFF, 0xAC].as_slice(),
    ] {
        let Ok(mut decoder) = Decoder::new(input);
        let mut context = Context::default();

        for _bit in 0..4096 {
            assert_eq!(decoder.read_bit(&mut context), Ok(1));
        }

        assert_eq!(decoder.data, input);
    }
}

#[test]
fn resets_clear_the_mps_and_restore_the_requested_probability() -> Result<(), InvalidState> {
    let mut context = Context(0x80 | 12);
    let Ok(()) = context.reset();

    assert_eq!(context, Context::default());

    for index in 0..=46 {
        context.0 = 0x80 | 12;
        context.reset_with_index(index)?;

        assert_eq!(context.0, index);

        let Ok(mut decoder) = Decoder::new(&ENCODED);
        let mut fresh = Context::default();

        fresh.reset_with_index(index)?;

        let Ok(mut other) = Decoder::new(&ENCODED);

        for _bit in 0..512 {
            assert_eq!(decoder.read_bit(&mut context), other.read_bit(&mut fresh));
            assert_eq!(context, fresh);
        }
    }

    Ok(())
}

#[test]
fn invalid_resets_are_reported_and_leave_the_context_intact() {
    let mut context = Context(0x80 | 12);

    for index in 47..=255 {
        assert_eq!(context.reset_with_index(index), Err(InvalidState(index)));
        assert_eq!(context, Context(0x80 | 12));
    }
}

#[test]
fn every_probability_transition_stays_within_the_table() -> Result<(), InvalidState> {
    for index in 0..=46 {
        for input in [
            ENCODED.as_slice(),
            [0xFF, 0x8F, 0x80].as_slice(),
            [].as_slice(),
        ] {
            let Ok(mut decoder) = Decoder::new(input);
            let mut context = Context::default();

            context.reset_with_index(index)?;

            for _bit in 0..4096 {
                let Ok(bit) = decoder.read_bit(&mut context);

                assert!(bit <= 1);
                assert!(context.0 & 0x7F <= 46);
                assert!((0x8000..=0xFFFF).contains(&decoder.a));
            }
        }
    }

    Ok(())
}

#[test]
fn contexts_still_fit_in_one_byte() {
    assert_eq!(core::mem::size_of::<Context>(), 1);
}
