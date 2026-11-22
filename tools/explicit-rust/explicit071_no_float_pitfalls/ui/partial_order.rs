// run-rustfix
// edition:2024

struct Point { x: f32 }

fn equal(left: f64, right: f64) -> bool { left == right }
fn unequal(left: &mut f32, right: &mut f32) -> bool { left != right }
fn same_column(a: &Point, b: &Point) -> bool { a.x == b.x }
fn is_min(a: &Point) -> bool { a.x == f32::MIN }
fn same_member(a: &(f32, f32), b: &f32) -> bool { a.1 == *b }
fn whole(a: &Point) -> bool { a.x == a.x.trunc() }

fn main() {
    let values = [0.0_f32, -0.0, 1.0, -1.0, f32::MIN, f32::INFINITY,
        f32::NEG_INFINITY, f32::NAN, f32::from_bits(0xffc00023)];
    for a in values {
        for b in values {
            let expected = a.partial_cmp(&b) == Some(core::cmp::Ordering::Equal);
            assert_eq!(equal(f64::from(a), f64::from(b)), expected);
            assert_eq!(unequal(&mut a.clone(), &mut b.clone()), !expected);
            assert_eq!(same_column(&Point { x: a }, &Point { x: b }), expected);
            assert_eq!(same_member(&(0.0, a), &b), expected);
        }
    }
    assert!(is_min(&Point { x: f32::MIN }));
    assert!(!is_min(&Point { x: f32::NAN }));
    assert!(whole(&Point { x: -0.0 }));
    assert!(!whole(&Point { x: -0.75 }));
}
