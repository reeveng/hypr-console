// edition:2024
// compile-flags: --test
#![allow(dead_code)]

fn value() -> Option<u8> { Some(1) }

macro_rules! exact_tokens { ($value:ident().unwrap()) => { $value().unwrap() }; }
macro_rules! opaque { ($e:expr) => { $e }; }

#[test]
fn keep_opaque_inputs_and_dynamic_messages() {
    assert!(opaque!(value().unwrap()) > 0);
    assert!(exact_tokens!(value().unwrap()) > 0);
    let text = String::from("lookup");
    assert!(value().expect(&text) > 0);
    let callback = || value().unwrap();
    assert_eq!(callback(), 1);
}

#[test]
#[should_panic(expected = "missing glyph")]
fn expects_a_panic() {
    assert!(Option::<u8>::None.expect("missing glyph") > 0);
}
