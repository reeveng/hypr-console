// edition:2024

mod callback {
    fn draw(x: u32, y: u32) -> u32 { x + y }
    pub fn entry() -> u32 {
        let address: fn(u32, u32) -> u32 = draw;
        draw(1, 2) + address(3, 4)
    }
}

mod exported {
    pub fn draw(x: u32, y: u32) -> u32 { x + y }
    pub fn entry() -> u32 { draw(1, 2) }
}

mod disabled {
    fn draw(x: u32, y: u32) -> u32 { x + y }
    #[cfg(any())]
    fn hidden() { draw(7, 8); }
    pub fn entry() -> u32 { draw(1, 2) }
}

mod nonlocal {
    fn draw(x: u32, y: u32) -> u32 { x + y }
    mod child { pub fn one() -> u32 { super::draw(1, 2) } }
    pub fn entry() -> u32 { child::one() }
}

mod collision {
    pub struct DrawArguments;
    fn draw(x: u32, y: u32) -> u32 { x + y }
    pub fn entry() -> u32 { draw(1, 2) }
}

mod macro_input {
    fn draw(x: u32, y: u32) -> u32 { x + y }
    pub fn entry() { assert_eq!(draw(1, 2), 3); }
}

mod foreign {
    extern "C" fn draw(x: u32, y: u32) -> u32 { x + y }
    pub fn entry() -> u32 { draw(1, 2) }
}

mod generic {
    fn draw<T>(x: u32, y: u32, _value: T) -> u32 { x + y }
    pub fn entry() -> u32 { draw(1, 2, ()) }
}

mod spelling {
    macro_rules! show { ($e:expr) => { (stringify!($e), $e) } }
    fn draw(x: u32, y: u32) -> u32 { x + y }
    pub fn entry() { let _ = show!(draw(1, 2)); }
}

fn main() {}
