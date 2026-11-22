// run-rustfix
// edition:2024

fn chroma_row(row: usize, log2_ctb: u32) -> (usize, usize) {
    let ctb = 1usize.wrapping_shl(log2_ctb);
    (row / ctb, row % ctb)
}

fn tile_offset(position: u32, log: u32) -> u32 {
    position / u32::wrapping_shl(1, log)
}

fn font_depth(items: u16, records: u32) -> (u32, u32) {
    (16 - items.leading_zeros(), u32::BITS - u32::leading_zeros(records))
}

fn emitted_bits(byte: i8) -> u32 {
    8 - byte.count_ones()
}

fn surrogate(high: u32, low: u32) -> u32 {
    let payload = u32::wrapping_shl(high & 0x3FF, 10) + (low & 0x3FF);
    payload + 0x10000
}

fn shifted_bits(byte: u8) -> u8 {
    byte.wrapping_shr(4) + 240
}

fn film_grain(block_size: i32, x: usize, is_sx: bool, is_sy: bool) -> (i32, usize) {
    let sx = <usize as ::core::convert::From<bool>>::from(is_sx);
    let sy = is_sy as usize;
    (block_size >> sx, x << sy)
}

fn main() {
    for n in [0, 15, 31, 32, 63, 64, 65, u32::MAX] {
        let mask = 1usize.wrapping_shl(n);
        assert_eq!(chroma_row(usize::MAX, n),
            (usize::MAX.wrapping_div(mask), usize::MAX.wrapping_rem(mask)));
        assert_eq!(tile_offset(u32::MAX, n), u32::MAX.wrapping_div(1u32.wrapping_shl(n)));
    }
    assert_eq!(font_depth(0, 0), (0, 0));
    assert_eq!(font_depth(u16::MAX, u32::MAX), (16, 32));
    assert_eq!(surrogate(u32::MAX, u32::MAX), 0x10FFFF);
    assert_eq!(surrogate(0, 0), 0x10000);
    for flag in [false, true] {
        assert_eq!(film_grain(i32::MIN, usize::MAX, flag, flag),
            (i32::MIN.wrapping_shr(u32::from(flag)), usize::MAX.wrapping_shl(u32::from(flag))));
    }
    for byte in u8::MIN..=u8::MAX {
        assert_eq!(emitted_bits(byte.cast_signed()), byte.count_zeros());
        assert_eq!(shifted_bits(byte), 0xF0 | byte.wrapping_shr(4));
    }
}
