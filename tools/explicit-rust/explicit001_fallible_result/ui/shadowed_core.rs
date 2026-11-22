// edition:2024

extern crate self as core;

fn float(text: &str) -> Option<f64> {
    text.parse::<f64>().ok()
}

fn main() {
    assert_eq!(float("1.25"), Some(1.25));
}
