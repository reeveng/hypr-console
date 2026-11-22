// run-rustfix
// edition:2024
struct Pixel { channels: [u8; 4] }
fn convert(x: &[u16; 2]) -> u32 { u32::from(x[1]) }
fn main() {
    let p = Pixel { channels: [1, 2, 3, 4] };
    let red = p.channels[0];
    assert_eq!(red, 1);
    assert_eq!(convert(&[5, 7]), 7);
    assert_eq!(p.channels[2] + p.channels[3], 7);
}
