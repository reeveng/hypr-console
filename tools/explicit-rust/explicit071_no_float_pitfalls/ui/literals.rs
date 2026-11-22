// run-rustfix
// edition:2024

const fn at_origin(x: f32, y: f32) -> bool {
    x == -0.0 && 0.0 == y
}

fn has_rotation(rotation: f64) -> Option<bool> {
    (rotation != 0.0).then_some(true)
}

fn scaled(scale: f64, read: impl FnOnce() -> f32) -> bool {
    scale != 1.0 || -2.5 == read()
}

fn main() {
    assert!(at_origin(-0.0, 0.0));
    assert!(!at_origin(f32::NAN, 0.0));
    assert_eq!(has_rotation(-0.0), None);
    assert_eq!(has_rotation(f64::NAN), Some(true));
    assert!(scaled(1.0, || -2.5));
    assert!(!scaled(1.0, || f32::NAN));
    assert!(scaled(f64::NAN, || panic!("short circuit")));
}
