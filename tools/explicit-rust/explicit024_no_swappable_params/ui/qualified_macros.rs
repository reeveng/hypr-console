// run-rustfix
// edition:2024

fn cross(a: f64, b: f64, c: f64, d: f64) -> f64 { a * b - c * d }

fn main() {
    let first = cross(2.0, 3.0, 4.0, 5.0);
    let second = cross(4.0, 5.0, 2.0, 3.0);
    ::std::assert!(::core /* separator */ ::matches!(first, -14.0));
    assert_eq!(second, 14.0);
}
