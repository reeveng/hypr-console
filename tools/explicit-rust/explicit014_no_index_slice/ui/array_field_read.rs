// run-rustfix
// rustfix-only-machine-applicable
// edition:2024
#![warn(explicit014_no_index_slice)]

struct Glyph { rect: [i16; 4] }

fn height(glyphs: &[Glyph], at: &usize) -> i16 {
    glyphs[*at].rect[3] - glyphs[*at].rect[1]
}

fn edge(blocks: &[(u8, [i16; 4])], at: usize) -> i16 {
    blocks[at].1[1].max(7)
}

trait Address {
    fn address(&self) -> *const Self;
}

impl Address for i16 {
    fn address(&self) -> *const Self { self }
}

fn address(blocks: &[(u8, [i16; 4])], at: usize) -> *const i16 {
    blocks[at].1[1].address()
}

fn main() {
    let glyphs = [Glyph { rect: [0, 2, 4, 8] }];
    let blocks = [(0, [1, 8, 2, 3])];
    assert_eq!(height(&glyphs, &0), 6);
    assert_eq!(edge(&blocks, 0), 8);
    let [(_, values)] = &blocks;
    assert_eq!(address(&blocks, 0), values.as_ptr().wrapping_add(1));
}
