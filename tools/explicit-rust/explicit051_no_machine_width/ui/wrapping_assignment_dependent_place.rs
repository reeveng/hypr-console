// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

fn main() {
    let Ok(position) = console_core_number_conversion::wrapped_index(2u8);
    let mut positions = [position, position];
    {
        let step = 1;
        let place = &mut positions[step];
        *place = (*place).wrapping_add(step);
    };
}
