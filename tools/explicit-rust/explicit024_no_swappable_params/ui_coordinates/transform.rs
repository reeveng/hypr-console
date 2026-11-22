// run-rustfix
// edition:2024
// aux-build:console_core_paths.rs
extern crate console_core_paths;
use console_core_paths::Transform;

fn main() {
    let Ok(transform) = Transform::from_translate(-0.0, 2.0);
    let (x, y) = (3.0, 4.0);
    let Ok(result) = transform.post_scale(x, y);
    assert_eq!(result.0.x.to_bits(), (-0.0_f32).to_bits());
    assert_eq!(result.0.y, 8.0);
    let Ok(borrowed) = (&transform).post_scale(x, y);
    assert_eq!(borrowed.0.y, 8.0);

    let constructor = Transform::from_translate;
    let Ok(through_value) = constructor(5.0, 6.0);
    assert_eq!(through_value.0.y, 6.0);
}
