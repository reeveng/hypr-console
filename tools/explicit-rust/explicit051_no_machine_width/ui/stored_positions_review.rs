// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

macro_rules! show { ($i:expr) => { ($i, stringify!($i)) }; }
fn main() {
    let bytes = &[10, 20, 30];
    let captured = bytes.len();
    let _print = format!("{captured}");
    let borrowed = bytes.len();
    let _ref = &borrowed;
    let text = bytes.len();
    let _code = show!(text);
    let scheduled = bytes.len();
    let next = move || bytes.get(scheduled);
    let _result = next();
    let guarded = bytes.len();
    #[cfg(any())]
    let _untested = format!("{guarded}");
    let _read = bytes.get(guarded);
}
