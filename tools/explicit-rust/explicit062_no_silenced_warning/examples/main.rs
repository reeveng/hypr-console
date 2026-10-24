#[allow(dead_code)]
fn never_called(held: u8) -> u8 {
    held
}

fn answered(held: u8) -> u8 {
    held
}

#[allow(dead_code, reason = "the panel beside it is the one drawn")]
fn kept(held: u8) -> u8 {
    held
}

#[expect(dead_code, reason = "the frame writer that calls it is not written yet")]
fn ahead(held: u8) -> u8 {
    held
}

#[allow(clippy::needless_return, reason = "the one lint this case disagrees with, by name")]
fn named(held: u8) -> u8 {
    held
}

fn main() {
    let _ = (never_called(1), answered(1), kept(1), named(1));
}
