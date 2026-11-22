use std::fs::File;
use std::num::NonZeroU32;

fn declared(at: *const u8, long: usize) -> &'static [u8] {
    // SAFETY: the case is about the call, and nothing here is run.
    //~v WARN EXPLICIT096_NO_UNCHECKED_MEMORY
    unsafe { std::slice::from_raw_parts(at, long) }
}

fn lengthened(held: &mut Vec<u8>, long: usize) {
    // SAFETY: the case is about the call, and nothing here is run.
    //~v WARN EXPLICIT096_NO_UNCHECKED_MEMORY
    unsafe { held.set_len(long) }
}

fn nothing_at_all() -> u32 {
    // SAFETY: the case is about the call.
    //~v WARN EXPLICIT096_NO_UNCHECKED_MEMORY
    unsafe { std::mem::zeroed() }
}

fn not_asked(many: u32) -> NonZeroU32 {
    // SAFETY: the case is about the call, and nothing here is run.
    //~v WARN EXPLICIT096_NO_UNCHECKED_MEMORY
    unsafe { NonZeroU32::new_unchecked(many) }
}

fn summed(one: u32, other: u32) -> u32 {
    // SAFETY: the case is about the call, and nothing here is run.
    //~v WARN EXPLICIT096_NO_UNCHECKED_MEMORY
    unsafe { one.unchecked_add(other) }
}

fn asked(many: u32) -> Option<NonZeroU32> {
    NonZeroU32::new(many)
}

fn emptied(held: &File) -> std::io::Result<()> {
    held.set_len(0)
}

fn filled(held: &mut Vec<u8>, long: usize) {
    held.resize(long, 0);
}

#[allow(dead_code)]
fn initialized(value: &mut std::mem::MaybeUninit<u8>) {
    value.write(1);
    // SAFETY: write initialized the byte above.
    let _ = unsafe { value.assume_init_read() };
    // SAFETY: the byte remains initialized and is Copy.
    let _ = unsafe { value.assume_init_ref() };
    // SAFETY: the value is initialized and exclusively borrowed.
    let _ = unsafe { value.assume_init_mut() };
    // SAFETY: the value was initialized and will not be accessed again.
    unsafe { value.assume_init_drop() };
}

#[allow(dead_code)]
fn manual_destruction() {
    let mut owned = std::mem::ManuallyDrop::new(String::new());
    // SAFETY: owned contains a live value and will not be accessed again.
    unsafe { std::mem::ManuallyDrop::drop(&mut owned) };

    let mut owned = std::mem::ManuallyDrop::new(String::new());
    // SAFETY: the returned value takes ownership and owned is never accessed again.
    let _ = unsafe { std::mem::ManuallyDrop::take(&mut owned) };

    let mut owned = std::mem::ManuallyDrop::new(String::new());
    // SAFETY: the value is live and the wrapper will not drop it again.
    unsafe { std::ptr::drop_in_place(&mut *owned) };

    let byte = 1_u8;
    // SAFETY: source is valid and the destination is the same type.
    let _: u8 = unsafe { std::mem::transmute_copy(&byte) };
}

#[allow(dead_code)]
fn raw_reads() {
    let owned = std::mem::ManuallyDrop::new(String::new());
    // SAFETY: the raw read transfers ownership from the manual wrapper.
    let _: String = unsafe { std::ptr::read(&*owned) };

    let owned = std::mem::ManuallyDrop::new(String::new());
    let pointer: *const String = &*owned;
    // SAFETY: the raw read transfers ownership from the manual wrapper.
    let _: String = unsafe { pointer.read() };

    let byte = 1_u8;
    // SAFETY: the pointer is live and the byte is Copy.
    let _: u8 = unsafe { std::ptr::read(&byte) };
}

fn main() {
    let mut held = vec![1u8];
    filled(&mut held, 2);
    let _ = asked(1);
    let _ = nothing_at_all();
    let _ = summed(1, 2);
    let _ = File::open("somewhere").map(|held| emptied(&held));
    let _ = declared;
    let _ = lengthened;
    let _ = not_asked;
}
