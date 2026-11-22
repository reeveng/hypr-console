// run-rustfix
// edition:2024

#[derive(Clone, Copy)]
struct Point { x: u8 }

fn select(points: &[Point]) -> u8 {
    match points.len() {
        0 => 0,
        1 => { let Point { x } = points[0]; x }
        2 => points[0].x + points[1].x,
        n if n & 1 != 0 => points[0].x,
        _ => points[1].x,
    }
}

#[allow(explicit014_no_index_slice)]
fn main() {
    let points = [Point { x: 2 }, Point { x: 3 }, Point { x: 4 }, Point { x: 5 }];
    assert_eq!(select(&[]), 0);
    assert_eq!(select(&points[..1]), 2);
    assert_eq!(select(&points[..2]), 5);
    assert_eq!(select(&points[..3]), 2);
    assert_eq!(select(&points), 3);
}
