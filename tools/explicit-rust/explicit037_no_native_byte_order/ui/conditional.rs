// edition:2024

#[cfg_attr(target_pointer_width = "64", cfg(target_endian = "little"))]
fn conditional(value: u32) -> [u8; 4] {
    value.to_ne_bytes()
}

#[cfg(any(target_endian = "little", target_pointer_width = "64"))]
fn mixed(value: u32) -> [u8; 4] {
    value.to_ne_bytes()
}

fn main() {}
