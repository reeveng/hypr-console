fn read_at(held: *const u8) -> u8 {
    //~v WARN EXPLICIT058_CONTAIN_UNSAFE
    unsafe { *held }
}

#[allow(dead_code)]
unsafe fn held_at(held: *const u8) -> u8 {
    unsafe { *held }
}

#[allow(dead_code)]
fn answered(held: *const u8) -> Option<u8> {
    match held.is_null() {
        true => None,
        false => Some(read_at(held)),
    }
}

fn main() {
    let one: u8 = 3;
    let _ = read_at(&one as *const u8);
}
