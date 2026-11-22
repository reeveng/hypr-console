// run-rustfix
// edition:2024
// aux-build:console_core_number_conversion.rs
extern crate console_core_number_conversion;

use core::cell::RefCell;

#[allow(explicit051_no_machine_width)]
type Position = usize;

fn main() {
    let Ok(mut position) = console_core_number_conversion::wrapped_index(-1i8);
    {
        let step = 2;
        let place = &mut position;
        *place = Position::wrapping_add(*place, step);
    };
    assert_eq!(position, 1);

    let Ok(mut displacement) = console_core_number_conversion::wrapped_offset(4i8);
    {
        let Ok(step) = console_core_number_conversion::wrapped_offset(7i8);
        let place = &mut displacement;
        *place = (*place).wrapping_sub(step);
    };
    assert_eq!(displacement, -3);

    let values = RefCell::new([3u8; 1]);
    let mut positions: [Position; 1] = [position];
    {
        let _factor = values.borrow().len().wrapping_add(1);
        let place = &mut positions[{ values.borrow_mut()[0] = 9; 0 }];
        *place = (core::num::Wrapping(*place) * core::num::Wrapping(_factor)).0;
    };
    assert_eq!(positions, [2]);
    assert_eq!(*values.borrow(), [9]);
}
