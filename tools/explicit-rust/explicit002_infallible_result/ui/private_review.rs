// edition:2024
// aux-build:console_core_never.rs
extern crate console_core_never;

mod callbacks {
    fn example_table() -> u8 { 8 }
    pub fn verify() -> Result<(), ()> {
        let make: fn() -> u8 = example_table;
        let value = example_table();
        assert_eq!(make(), value);
        Ok(())
    }
}
mod inactive {
    fn sample() -> u8 { 7 }
    #[cfg(any())]
    const ALTERNATE: fn() -> u8 = sample;
    pub fn verify() -> Result<(), ()> {
        let value = sample();
        assert_eq!(value, 7);
        Ok(())
    }
}
mod public_api {
    pub fn table() -> u8 { 4 }
}
mod early_return {
    fn select(choice: bool) -> u8 {
        if choice { return 8; }
        9
    }
    pub fn verify() -> Result<(), ()> {
        let selected = select(false);
        assert_eq!(selected, 9);
        Ok(())
    }
}
mod macro_input {
    fn sample() -> u8 { 5 }
    pub fn verify() -> Result<(), ()> {
        assert_eq!({ let value = sample(); value }, 5);
        Ok(())
    }
}
fn main() {
    assert!(callbacks::verify().is_ok());
    assert!(macro_input::verify().is_ok());
    assert!(inactive::verify().is_ok());
    assert!(early_return::verify().is_ok());
    assert_eq!(public_api::table(), 4);
}
