// aux-build:console_core_number_conversion.rs

extern crate console_core_number_conversion;

fn main() {
    let bytes = [10, 20, 30];
    let Ok(position) = console_core_number_conversion::index(1);
    let Ok(span) = console_core_number_conversion::indices(0..2);
    let Ok(from) = console_core_number_conversion::indices_from(1..);
    let Ok(to) = console_core_number_conversion::indices_to(..2);
    let Ok(inclusive) = console_core_number_conversion::indices_inclusive(0..=1);
    let Ok(to_inclusive) = console_core_number_conversion::indices_to_inclusive(..=1);

    let Ok(wrapped_position) = console_core_number_conversion::wrapped_index(1);
    let Ok(offset) = console_core_number_conversion::wrapped_offset(-1);

    assert_eq!(bytes.get(wrapped_position), Some(&20));
    assert_eq!(offset, -1);
    assert_eq!(bytes.get(position), Some(&20));
    assert_eq!(bytes.get(span), Some([10, 20].as_slice()));
    assert_eq!(bytes.get(from), Some([20, 30].as_slice()));
    assert_eq!(bytes.get(to), Some([10, 20].as_slice()));
    assert_eq!(bytes.get(inclusive), Some([10, 20].as_slice()));
    assert_eq!(bytes.get(to_inclusive), Some([10, 20].as_slice()));
}
