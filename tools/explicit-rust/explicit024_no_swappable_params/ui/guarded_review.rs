// edition:2024

mod storage {
    macro_rules! inspect { () => { 0 }; (deferred) => { render(4, 5) } }
    pub(crate) use inspect;
}

mod pixels {
    use super::storage::inspect;
    fn render(x: usize, y: usize) -> usize { x + y }
    fn gated(x: usize, y: usize) -> usize { x * y }
    #[cfg(any())]
    fn target_only() { let other: fn(usize, usize) -> usize = gated; }
    pub fn verify() -> usize { inspect!() + gated(1, 2) + render(0, 0) }
}

mod guard_mismatch {
    #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
    fn check(x: usize, y: usize) -> usize { x + y }
    pub fn verify() -> usize { check(2, 5) }
}

mod qualified_macro_argument {
    fn sum(a: usize, b: usize) -> usize { a + b }
    pub fn verify() { ::std::assert_eq!(sum(3, 4), 7); }
}

fn main() {
    assert_eq!(pixels::verify() + guard_mismatch::verify(), 9);
    qualified_macro_argument::verify();
}
