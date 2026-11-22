// edition:2024

#[repr(packed)]
struct Exchange { tag: u8, price: f64 }

fn packed(left: &Exchange, right: f64) -> bool { left.price == right }
fn mutation(mut value: f32) -> bool { value == { value = 2.0; value } }
const fn constant(left: f32, right: f32) -> bool { left == right }
fn annotated(left: f64, right: f64) -> bool { left /* units */ == right }
macro_rules! explained { ($e:expr) => { ($e, stringify!($e)) }; }
fn explanation(left: f32, right: f32) -> (bool, &'static str) { explained!(left == right) }
fn literal_explanation(left: f32) -> (bool, &'static str) { explained!(left != 0.0) }

fn main() {
    assert!(packed(&Exchange {tag: 1, price: 3.0}, 3.0));
    assert!(!mutation(1.0));
    assert!(constant(0.0, -0.0));
    assert!(annotated(1.0, 1.0));
    assert_eq!(explanation(1.0, 1.0), (true, "left == right"));
    assert_eq!(literal_explanation(-0.0), (false, "left != 0.0"));
}
