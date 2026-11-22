// run-rustfix
// edition:2024

#[allow(explicit051_no_machine_width)]
type Position = usize;

fn compound(mut position: Position) -> Position {
    let _amount = 2;
    let _target = &mut position;
    *_target = usize::wrapping_add(*_target, _amount);
    position
}

fn annotated_closure(position: Position) -> Position {
    let advance = |distance: Position| usize::wrapping_mul(distance, 3);
    advance(position)
}

fn subtract(order: &mut Vec<u8>, position: Position) -> Position {
    usize::wrapping_sub({ order.push(1); 10 }, { order.push(2); position })
}

fn main() {
    assert_eq!(compound(Position::MAX), 1);
    assert_eq!(annotated_closure(4), 12);
    let mut order = Vec::new();
    assert_eq!(subtract(&mut order, 3), 7);
    assert_eq!(order, [1, 2]);
}
