// run-rustfix
// edition:2024

enum Never {}

fn infallible(value: Result<String, Never>) -> Option<String> {
    value.ok()
}

fn impossible_success(value: Result<core::convert::Infallible, String>) -> Option<String> {
    value.err()
}

fn followed_by_a_method(value: Result<u32, Never>) -> bool {
    value.ok().is_some()
}

fn main() {
    assert_eq!(infallible(Ok(String::from("value"))).as_deref(), Some("value"));
    assert_eq!(impossible_success(Err(String::from("fault"))).as_deref(), Some("fault"));
    assert!(followed_by_a_method(Ok(4)));
}
