// run-rustfix
// edition:2024
#[allow(explicit051_no_machine_width)]
type Position = usize;

#[allow(explicit051_no_machine_width)]
type Offset = isize;

fn advance(at: Position) -> Position { usize::wrapping_add(at, 1) }
fn wrapped() -> (Position, u8) { (!0, 7) }

fn cast_mask(at: Position, end: Position, bias: Offset) -> Position {
    let _sign = usize::cast_signed(at);
    for _step in 0..end {
        return isize::cast_unsigned(usize::cast_signed(_step) + bias);
    }
    isize::cast_unsigned(isize::wrapping_neg(_sign))
}

fn inferred() -> (Position, Position, Position) {
    let _length = [1_u8, 2].len();
    let _position = _length;
    let mut _written: Position = 2;
    _written = usize::wrapping_add(_written, 3);
    let _from_block = usize::wrapping_add({ let (_offset, _) = wrapped(); _offset }, 1);
    (_written, _from_block, usize::wrapping_mul(_position, 3))
}

fn main() {
    assert_eq!(advance(!0), 0);
    assert_eq!(usize::wrapping_mul([1_u8, 2].len(), 3), 6);
    assert_eq!(inferred(), (5, 0, 6));
    assert_eq!(cast_mask(!0, 2, 5), 5);
    assert_eq!(cast_mask(!0, 0, 5), 1);
    assert_eq!(read_index(&View(Fields { before: 1, index: 3 })), 3);
}

struct Fields { before: u8, index: Position }
struct View(Fields);

impl std::ops::Deref for View {
    type Target = Fields;
    fn deref(&self) -> &Fields { &self.0 }
}

fn read_index(view: &View) -> Offset {
    let _before = view.before;
    usize::cast_signed(view.index)
}
