#![allow(dead_code)]
fn make<T: Default>() -> T { T::default() }
fn main() {
    std::hint::black_box(usize::wrapping_add(0, 1));
    std::hint::black_box(usize::wrapping_add(make(), 1));
    let _from_context = 0;
    std::hint::black_box(usize::wrapping_add(_from_context, 1));
    let _still_generic: _ = make();
    std::hint::black_box(usize::wrapping_add(_still_generic, 1));
    std::hint::black_box(usize::cast_signed(make()));
    let _unsigned = 0;
    std::hint::black_box(usize::cast_signed(_unsigned));
}
