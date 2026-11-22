// run-rustfix
// edition:2024

const fn remaining(count: u8) -> u8 {
    if count == 0 { 4 } else { count }
}

fn separator(character: char) -> &'static str {
    if ':' != character { "text" } else { "separator" }
}

const fn marker(byte: u8) -> &'static str {
    match byte == b'!' {
        true => "bang",
        false => "other",
    }
}

const fn operation(code: u8) -> &'static str {
    match code != 0 {
        false => "stop",
        _ => "continue",
    }
}

fn main() {
    assert_eq!(remaining(0), 4);
    assert_eq!(remaining(2), 2);
    assert_eq!(separator(':'), "separator");
    assert_eq!(separator('é'), "text");
    assert_eq!(marker(b'!'), "bang");
    assert_eq!(marker(b'.'), "other");
    assert_eq!(operation(0), "stop");
    assert_eq!(operation(1), "continue");
}
