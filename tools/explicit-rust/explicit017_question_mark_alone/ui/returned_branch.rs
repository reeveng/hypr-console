// run-rustfix
// edition:2024

fn decode(read: Result<u8, String>, fallback: bool) -> Result<u8, String> {
    match fallback {
        true => Ok(0),
        false => Ok(read?),
    }
}

fn selected(value: Option<u8>, keep: bool) -> Option<u8> {
    if keep { Some(value?) } else { Some(4) }
}

fn main() {
    assert_eq!(decode(Err("fault".into()), false), Err("fault".into()));
    assert_eq!(decode(Err("fault".into()), true), Ok(0));
    assert_eq!(selected(None, true), None);
    assert_eq!(selected(None, false), Some(4));
}
