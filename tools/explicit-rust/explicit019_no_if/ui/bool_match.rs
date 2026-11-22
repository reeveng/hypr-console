// run-rustfix
// edition:2024

fn choice(wide: bool, request: Option<(u8, u8)>) -> u8 {
    if wide {
        9
    } else if let Some((left, right)) = request {
        left + right
    } else {
        0
    }
}

fn unpack(request: Option<u8>) -> u8 {
    if let Ok(number) = request.ok_or(()) {
        return number;
    }
    0
}

fn multiline(left: bool, right: bool) -> u8 {
    if (left
        && right) || left
    {
        return 1;
    }
    0
}

use std::cell::Cell;

struct Decision<'a>(&'a Cell<bool>);

impl Decision<'_> {
    fn ask(&self) -> bool { true }
}

impl Drop for Decision<'_> {
    fn drop(&mut self) { self.0.set(true); }
}

fn branch(flag: bool) -> u8 {
    if flag {
        return 1;
    }

    2
}

fn has_text(mut value: Option<String>) -> Option<String> {
    if value.is_some() {
        value.take()
    } else {
        None
    }
}

fn has_no_text(value: Option<String>) -> u8 {
    if let None = value { 0 } else { 1 }
}

fn score(count: u8) -> u8 {
    match count > 2 {
        false => 0,
        _ => count,
    }
}

fn inferred() -> u8 {
    if Default::default() { 1 } else { 0 }
}

fn is_present(value: Option<String>) -> u8 {
    match !value.is_none() {
        true => 1,
        false => 0,
    }
}

fn recover_failures(mut value: Result<String, String>) -> Result<String, String> {
    if value.is_err() {
        value = Ok(String::from("recovered"));
    }
    value
}

const fn has_value(value: Option<u8>) -> u8 {
    if value.is_some() { 1 } else { 0 }
}

fn drops_after_match() {
    let dropped = Cell::new(false);
    match Decision(&dropped).ask() {
        true => assert!(!dropped.get()),
        false => assert!(!dropped.get()),
    }
    assert!(dropped.get());
}

fn main() {
    let dropped = Cell::new(false);
    let value = if Decision(&dropped).ask() {
        assert!(dropped.get());
        3
    } else {
        0
    };
    assert_eq!(value, 3);
    assert_eq!(branch(true), 1);
    assert_eq!(branch(false), 2);
    assert_eq!(choice(true, Some((1, 2))), 9);
    assert_eq!(choice(false, Some((1, 2))), 3);
    assert_eq!(unpack(Some(4)), 4);
    assert_eq!(unpack(None), 0);
    assert_eq!(multiline(true, false), 1);
    assert_eq!(has_text(Some(String::from("text"))).as_deref(), Some("text"));
    assert_eq!(has_no_text(None), 0);
    assert_eq!(score(4), 4);
    assert_eq!(score(0), 0);
    assert_eq!(inferred(), 0);
    assert_eq!(is_present(Some(String::from("text"))), 1);
    assert_eq!(recover_failures(Err(String::from("input"))).as_deref(), Ok("recovered"));
    assert_eq!(has_value(Some(0)), 1);
    drops_after_match();
}
