// edition:2024
// aux-build:console_core_api_compatibility.rs
#![crate_type = "lib"]
extern crate console_core_api_compatibility;

struct Hidden { pub stride: usize }
fn width(value: usize) -> usize { value }
mod private { pub fn invisible(x: isize) -> isize { x } }
pub fn local(data: &[u8]) -> u8 {
    let length: usize = data.len();
    let held: console_core_api_compatibility::NativeSize = length;
    data.get(held).copied().unwrap_or_default()
}
