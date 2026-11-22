// run-rustfix
// edition:2024

struct Point { x: u8 }

fn points_after_move(points: &[Point], first: bool) -> Option<(u8, &Point, &Point)> {
    let curve = match points.first_chunk::<4>() {
        Some(curve) => curve,
        None => return None,
    };

    match first {
        true => Some((points[1].x, &points[2], &points[3])),
        false => {
            let [beginning, ..] = curve;
            Some((beginning.x, beginning, beginning))
        }
    }
}

fn main() {
    let [first, second, third, fourth] = &[
        Point { x: 1 }, Point { x: 2 }, Point { x: 3 }, Point { x: 4 },
    ];
    let values = [first, second, third, fourth].map(|point| Point { x: point.x });
    let [_, _, third, fourth] = &values;
    match points_after_move(&values, true) {
        Some((x, third_read, fourth_read)) => {
            assert_eq!(x, 2);
            assert!(core::ptr::eq(third_read, third));
            assert!(core::ptr::eq(fourth_read, fourth));
        }
        None => unreachable!(),
    }
    assert!(points_after_move(&[], true).is_none());
}
