// run-rustfix
// edition:2024
// normalize-stderr-test: "(?m)^LL \+ $" -> "LL +"

fn pair(input: Option<(u8, u8)>) -> u8 {
    let Some((mut left, right)) = input else {
        return 0;
    };
    left += 1;
    left + right
}

fn nested(input: Result<Option<String>, String>) -> String {
    let Ok(Some(word)) = input else {
        return String::new();
    };
    word
}

fn simple_call(input: Option<u8>) -> u8 {
    let Ok(number) = input.ok_or(()) else {
        return 0;
    };
    number
}

use std::option::Option::Some as Available;
use std::cell::Cell;

fn read(input: Option<String>) -> String {
    let Available(mut word) = input else {
        return String::new();
    };
    word.push('!');
    word
}

fn length(input: &Option<String>) -> usize {
    let Some(word) = input else {
        return 0;
    };
    word.len()
}

fn samples(input: Option<&[u8; 2]>) -> [u8; 2] {
    let Some(&copied) = input else {
        return [0; 2];
    };
    copied
}

struct Probe<'a>(&'a Cell<bool>);

impl Drop for Probe<'_> {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

impl Probe<'_> {
    fn read(&self, succeeds: bool) -> Option<u32> {
        succeeds.then_some(2)
    }
}

fn after_drop(dropped: &Cell<bool>, succeeds: bool) -> u32 {
    let Some(value) = Probe(dropped).read(succeeds) else {
        return u32::from(dropped.get());
    };
    assert!(dropped.get());
    value
}

fn main() {
    assert_eq!(read(Some(String::from("word"))), "word!");
    assert_eq!(length(&None), 0);
    assert_eq!(samples(Some(&[3, 6])), [3, 6]);
    assert_eq!(after_drop(&Cell::new(false), false), 1);
    assert_eq!(after_drop(&Cell::new(false), true), 2);
    assert_eq!(pair(Some((1, 2))), 4);
    assert_eq!(pair(None), 0);
    assert_eq!(nested(Ok(Some(String::from("text")))), "text");
    assert_eq!(nested(Ok(None)), "");
    assert_eq!(nested(Err(String::from("fault"))), "");
    assert_eq!(simple_call(Some(3)), 3);
    assert_eq!(simple_call(None), 0);
}
