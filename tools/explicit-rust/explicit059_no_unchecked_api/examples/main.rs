fn skipped(held: &[u8], at: usize) -> Option<u8> {
    //~v ERROR EXPLICIT059_NO_UNCHECKED_API
    let one = unsafe { held.get_unchecked(at) };

    Some(*one)
}

fn asked(held: &[u8], at: usize) -> Option<u8> {
    held.get(at).copied()
}

struct Remembered;

impl Remembered {
    fn forget(&self) {}

    fn leak(&self) {}
}

fn named_alike() {
    Remembered.forget();
    Remembered.leak();
}

fn main() {
    let _ = (skipped(&[1, 2], 0), asked(&[1, 2], 0));

    named_alike();
}
