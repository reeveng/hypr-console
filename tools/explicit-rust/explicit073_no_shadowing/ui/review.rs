macro_rules! code_and_spelling {
    ($value:ident) => { ($value, stringify!($value)) };
}

fn from_macro(value: u8) -> (u8, &'static str) {
    let value = value + 1;
    code_and_spelling!(value)
}

fn format_capture(value: u8) -> String {
    let value = value + 1;
    format!("{value}")
}

fn conditional(value: u8) -> u8 {
    let value = value + 1;
    #[cfg(any())]
    let value = value + 2;
    value
}

fn local_macro(value: u8) -> u8 {
    let value = value + 1;
    macro_rules! read {
        () => { value };
    }
    read!()
}

fn nested_function(value: u8) -> u8 {
    #[allow(non_upper_case_globals)]
    const number_2: u8 = 2;
    fn nested(number: u8) -> u8 {
        let number = number + 1;
        number
    }
    nested(value) + number_2
}

fn main() {
    assert_eq!(from_macro(4), (5, "value"));
    assert_eq!(format_capture(5), "6");
    assert_eq!(conditional(6), 7);
    assert_eq!(local_macro(7), 8);
    assert_eq!(nested_function(7), 10);
}
