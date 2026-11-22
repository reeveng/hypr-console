// edition:2024

fn shrink(data: &mut Vec<u8>, mut pos: usize) -> usize {
    while pos < data.len() {
        data.pop();
        pos = pos.wrapping_add(1);
    }
    pos
}

struct Limit {
    end: u8,
}

fn change(mut limit: Limit, mut pos: u8) -> u8 {
    while pos < limit.end {
        limit.end = limit.end.wrapping_sub(1);
        pos = pos.wrapping_add(1);
    }
    pos
}

fn borrow_one(input: &mut [u8]) {
    while let Some(value) = input.get_mut(0) {
        *value = 8;
        break;
    }
}

fn main() {
    assert_eq!(shrink(&mut vec![0, 0, 0], 0), 2);
    assert_eq!(change(Limit { end: 3 }, 0), 2);
    let mut input = [0];
    borrow_one(&mut input);
    assert_eq!(input, [8]);
}
