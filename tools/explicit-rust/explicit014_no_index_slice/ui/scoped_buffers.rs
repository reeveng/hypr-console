// run-rustfix
// edition:2024
// aux-build: archmage_macros.rs

extern crate archmage_macros;

#[archmage_macros::rite]
#[inline(always)]
fn columns() -> [usize; 8] {
    let empty = [0; 8];
    let mut cols = empty;
    for x in 0..8 {
        cols[x] = x;
    }
    cols
}

fn transform_rows(rows: &mut [[u8; 8]; 2]) {
    for row in rows {
        let mut scratch = [0u8; 8];
        for x in 0..8 {
            scratch[x] = x as u8;
        }
        scratch.reverse();
        for x in 0..8 {
            if let Some(output) = row.get_mut(x) { *output = scratch[x]; }
        }
    }
    #[cfg(feature = "other")]
    std::hint::black_box(());
}

fn main() {
    assert_eq!(columns(), [0, 1, 2, 3, 4, 5, 6, 7]);
    let mut rows = [[0; 8]; 2];
    transform_rows(&mut rows);
    assert_eq!(rows, [[7, 6, 5, 4, 3, 2, 1, 0]; 2]);
}
