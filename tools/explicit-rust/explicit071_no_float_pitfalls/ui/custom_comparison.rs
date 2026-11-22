// edition:2024

struct Threshold;
impl PartialEq<Threshold> for f32 {
    fn eq(&self, _: &Threshold) -> bool { self.is_nan() }
}

fn custom(value: f32) -> bool { value == Threshold }
const fn unknown(left: f64, right: f64) -> bool { left != right }
fn main() {
    assert!(custom(f32::NAN));
    assert!(unknown(f64::NAN, f64::NAN));
}
