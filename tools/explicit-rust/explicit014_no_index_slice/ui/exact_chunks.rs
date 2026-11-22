// run-rustfix
// edition:2024
// rustc-env:CARGO_PKG_RUST_VERSION=1.94

// Read packed RGB triples and leave an incomplete final pixel out.
fn pixels(input: &[u8]) -> Vec<[u8; 4]> {
    let mut output = Vec::new();
    for px in input.chunks_exact(3) {
        output.push([px[0], px[1], px[2], 255]);
    }
    output
}

fn apply_transfer(data: &mut Vec<u8>) {
    for data in data.chunks_exact_mut(3) {
        data[0] = data[0].wrapping_add(1);
        data[1] = data[1].wrapping_add(2);
        data[2] = data[2].wrapping_add(3);
    }
}

fn mask(pixel: &[u8], bounds: &[u8]) -> bool {
    for (component, min_max) in pixel.iter().zip(bounds.chunks_exact(2)) {
        if *component > min_max[1] || *component < min_max[0] {
            return true;
        }
    }
    false
}

fn copy_channels(input: &[u8], output: &mut [u8]) {
    for (src, dst) in input.chunks_exact(2).zip(output.chunks_exact_mut(2)) {
        dst[0] = src[1];
        dst[1] = src[0];
    }
}

fn deltas(input: &[i16]) -> Vec<i16> {
    let mut output = Vec::new();
    for pair in input.windows(2) {
        output.push(pair[1] - pair[0]);
    }
    output
}

fn glyph_ids(bytes: &[u8]) -> Vec<(usize, u16)> {
    let mut output = Vec::new();
    for (cid, gid) in bytes.chunks_exact(2).enumerate() {
        output.push((cid, u16::from_be_bytes([gid[0], gid[1]])));
    }
    output
}

fn main() {
    let mut data = vec![1, 2, 3, 4, 5, 6, 99];
    assert_eq!(pixels(&data), vec![[1, 2, 3, 255], [4, 5, 6, 255]]);
    apply_transfer(&mut data);
    assert_eq!(data, vec![2, 4, 6, 5, 7, 9, 99]);
    assert!(mask(&[24, 12], &[23, 25, 13, 15, 34]));
    assert!(!mask(&[24], &[23, 25, 13, 15, 34]));
    let mut out = [0; 5];
    copy_channels(&[1, 2, 3, 4, 5], &mut out);
    assert_eq!(out, [2, 1, 4, 3, 0]);
    assert_eq!(deltas(&[4, 9, 3]), [5, -6]);
    assert!(deltas(&[]).is_empty());
    assert_eq!(glyph_ids(&[1, 2, 3, 4, 255]), [(0, 0x0102), (1, 0x0304)]);
}
