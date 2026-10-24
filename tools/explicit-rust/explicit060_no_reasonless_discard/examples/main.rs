fn thrown_away() {
    let held = Ok::<u8, u8>(1);

    //~v WARN EXPLICIT060_NO_REASONLESS_DISCARD
    let _ = held;
}

#[allow(dead_code)]
fn kept_quiet() {
    let held = Ok::<u8, u8>(1);

    #[allow(explicit060_no_reasonless_discard, reason = "the teardown cannot answer and the fault is nobody's to hold")]
    let _ = held;
}

fn met() -> u8 {
    match Ok::<u8, u8>(1) {
        Ok(one) => one,
        Err(_) => 0,
    }
}

fn main() {
    thrown_away();

    let _ = met();
}
