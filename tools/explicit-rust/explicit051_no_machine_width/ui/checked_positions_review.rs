// edition:2024
// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

struct Count;
impl TryFrom<Count> for usize {
    type Error = ();
    fn try_from(_: Count) -> Result<Self, ()> { Err(()) }
}

fn main() {
    let _custom = usize::try_from(Count);
    macro_rules! observe { ($e:expr) => { (stringify!($e), $e) }; }
    let _observed = observe!(usize::try_from(-1_i32));
    let mapped = console_core_number_conversion::checked_index(-2_i32)
        .map_err(|_| 3usize);
    assert!(mapped.is_err());
}
