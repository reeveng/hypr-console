// run-rustfix
// edition:2024
// aux-build:console_core_api_compatibility.rs
#![crate_type = "lib"]
extern crate console_core_api_compatibility;

pub struct Surface { pub stride: isize }
pub fn row(pixels: &[u8], offset: usize) -> Option<&u8> { pixels.get(offset) }
pub trait PixelRows { fn height(&self) -> usize; }
impl Surface {
    pub fn step(&self, rows: isize) -> isize { self.stride.wrapping_mul(rows) }
}
unsafe extern "C" { pub fn consume_pixels(ptr: *const u8, len: usize); }
pub type OffsetCallback = extern "C" fn(isize) -> usize;
