/// Encode an element ID (already carrying its marker bits).
pub fn write_id(out: &mut Vec<u8>, id: u32) {
    let n = if id >= 0x100_0000 {
        4
    } else if id >= 0x1_0000 {
        3
    } else if id >= 0x100 {
        2
    } else {
        1
    };
    out.extend_from_slice(&id.to_be_bytes()[4 - n..]);
}

/// Encode a data size as a VINT using the shortest length (or `min_len` if larger).
pub fn write_size(out: &mut Vec<u8>, size: u64, min_len: usize) {
    let mut len = 1;
    while len < 8 && size >= (1u64 << (7 * len)) - 1 {
        len += 1;
    }
    let len = len.max(min_len).min(8);
    let v = size | (1u64 << (7 * len));
    out.extend_from_slice(&v.to_be_bytes()[8 - len..]);
}

/// Encode the "unknown size" marker of the given length.
pub fn write_unknown_size(out: &mut Vec<u8>, len: usize) {
    out.push(0xFFu8 >> (len - 1));
    out.extend(std::iter::repeat_n(0xFF, len - 1));
}
