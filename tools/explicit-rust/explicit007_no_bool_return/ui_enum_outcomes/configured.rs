// run-rustfix
// edition:2024

mod bytes {
    enum ByteClass {
        Control,
        Data,
    }

    fn classify(byte: u8) -> bool {
        // Both values have the same classification.
        ::core::matches!(byte, 0 | 27)
    }

    pub fn count(bytes: &[u8]) -> usize {
        bytes.iter().filter(|&&byte| classify(byte)).count()
    }

    pub fn answer(byte: u8) -> usize {
        usize::from(!classify(byte))
    }
}

fn main() {
    let _ = (bytes::count(&[0, 1, 27]), bytes::answer(1));
}
