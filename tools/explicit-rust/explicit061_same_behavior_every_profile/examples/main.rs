fn checked(held: u8) -> u8 {
    //~v WARN EXPLICIT061_SAME_BEHAVIOR_EVERY_PROFILE
    debug_assert!(held > 0);

    held
}

#[cfg(debug_assertions)]
fn only_when_debugging(held: u8) -> u8 {
    held + 1
}

#[allow(dead_code)]
fn always(held: u8) -> Option<u8> {
    checked(held).checked_add(1)
}

fn main() {
    let _ = (checked(1), always(1));

    #[allow(dead_code)]
    let _ = only_when_debugging(1);
}
