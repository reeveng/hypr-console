// UI test for EXPLICIT037 — a byte order nobody wrote down.

// BAD EXPLICIT037 — the order is whatever compiled this.
fn laid(said: u32) -> [u8; 4] {
    //~v EXPLICIT037_NO_NATIVE_BYTE_ORDER
    said.to_ne_bytes()
}

// BAD EXPLICIT037 — the same, read back.
fn read(of: [u8; 4]) -> u32 {
    //~v EXPLICIT037_NO_NATIVE_BYTE_ORDER
    u32::from_ne_bytes(of)
}

// GOOD — which end is said at the site.
fn little(said: u32) -> [u8; 4] {
    said.to_le_bytes()
}

// GOOD — and the other end, for something that asked for it.
fn big(of: [u8; 4]) -> u32 {
    u32::from_be_bytes(of)
}

fn main() {
    let _ = laid(1);
    let _ = read([0, 0, 0, 1]);
    let _ = little(1);
    let _ = big([0, 0, 0, 1]);
}
