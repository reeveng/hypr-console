// edition:2024
#![allow(dead_code)]

fn replacement_slice<'a>(mut points: &'a [u8], other: &'a [u8]) -> Option<u8> {
    let _curve = match points.first_chunk::<4>() {
        Some(checked) => checked,
        None => return None,
    };
    points = other;
    Some(points[1])
}

fn shadowed(points: &[u8], curve_other: &[u8; 4]) -> Option<u8> {
    let curve = match points.first_chunk::<4>() {
        Some(checked) => checked,
        None => return None,
    };
    let curve = curve_other;
    core::hint::black_box(curve);
    Some(points[1])
}

fn check_can_fail(points: &[u8]) -> u8 {
    let _curve = match points.first_chunk::<4>() {
        Some(checked) => checked,
        None => &[1, 2, 3, 4],
    };
    points[1]
}

fn outside(points: &[u8]) -> Option<u8> {
    let _curve = match points.first_chunk::<4>() {
        Some(checked) => checked,
        None => return None,
    };
    Some(points[4])
}

fn main() {}
