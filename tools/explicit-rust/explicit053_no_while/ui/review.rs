// edition:2024

fn moving_bound(mut end: i32) -> i32 {
    let mut position = 0;
    while position < end {
        end -= 1;
        position += 1;
    }
    position
}

fn skip_increment(end: i32) -> i32 {
    let mut position = 0;
    'walk: while position < end {
        if position == 0 {
            position += 2;
            continue 'walk;
        }
        position += 1;
    }
    position
}

const fn const_loop(end: i32) -> i32 {
    let mut position = 0;
    while position < end {
        position += 1;
    }
    position
}

fn moving_computed_bound(mut end: u16, max: u16) -> u16 {
    let mut position = 0;
    while position < core::cmp::min(end, max) {
        let ref mut bound = end;
        *bound = bound.saturating_sub(1);
        position = position.wrapping_add(1);
    }
    position
}

fn effectful_bound(mut limit: impl FnMut() -> i32) -> i32 {
    let mut position = 0;
    while position < limit() {
        position += 1;
    }
    position
}

fn unknown_stride(mut position: u8, bound: u8) -> u8 {
    while position.wrapping_add(2) <= bound {
        position = position.wrapping_add(2);
        if position == 0 {
            break;
        }
    }
    position
}

fn iterator_alias(state: &mut (core::array::IntoIter<i32, 2>, i32)) {
    while let Some(item) = state.0.next() {
        state.0.as_slice();
        state.1 += item;
    }
}

fn saved_place_stride(mut index: u32, limit: u32) -> u32 {
    while index < limit {
        {
            let amount = 2;
            let target = &mut index;
            *target = u32::wrapping_add(*target, amount);
        };
    }
    index
}

fn main() {
    assert_eq!(moving_bound(7), 4);
    assert_eq!(skip_increment(7), 7);
    assert_eq!(const_loop(5), 5);
    assert_eq!(moving_computed_bound(7, 5), 4);
    assert_eq!(effectful_bound(|| 5), 5);
    assert_eq!(unknown_stride(254, 3), 0);
    let mut state = ([1, 2].into_iter(), 0);
    iterator_alias(&mut state);
    assert_eq!(state.1, 3);
}
