// run-rustfix
// edition:2024

#[allow(unused_parens)]
mod chroma {
    fn interpolated(near: u16, mut far: u16) -> u16 {
        far = far.wrapping_add(1);
        near.wrapping_mul(3).wrapping_add(far)
    }

    pub fn pair(near: u16) -> [u16; 2] {
        let mut next = 0;
        let mut step = || { next += 1; next };
        [
            interpolated((step()), (step())),
            interpolated(near, interpolated(near, step()),),
        ]
    }
}

mod borrows {
    // Other arguments and temporaries keep their evaluation and drop order.
    struct Mark<'a>(&'a std::cell::Cell<u32>, u32);
    impl Drop for Mark<'_> {
        fn drop(&mut self) { self.0.set(self.0.get() * 10 + self.1); }
    }
    fn sample(_before: Mark<'_>, even: u32, start: u8, end: u8, limit: u16, _after: Mark<'_>) -> u8 {
        if even == 2 && limit == 16 { end - start } else { 0 }
    }
    pub fn run() -> (u8, u32) {
        let record = std::cell::Cell::new(0);
        let v = sample(Mark(&record, 1), 2, 5, 9, 16, Mark(&record, 2));
        (v, record.get())
    }
}

fn main() {
    assert_eq!(chroma::pair(4), [6, 29]);
    assert_eq!(borrows::run(), (4, 21));
}
