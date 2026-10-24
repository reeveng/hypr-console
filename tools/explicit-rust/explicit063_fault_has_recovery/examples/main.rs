fn met_with_nothing() {
    match Ok::<u8, u8>(1) {
        Ok(_) => {}
        //~v WARN EXPLICIT063_FAULT_HAS_RECOVERY
        Err(_) => {}
    }
}

fn nothing_left(written: Result<(), u8>) -> u8 {
    match written {
        Ok(()) => {}
        Err(_) => return 1,
    }

    0
}

fn handed_up() -> Result<u8, u8> {
    match Ok::<u8, u8>(1) {
        Ok(one) => Ok(one),
        Err(_) => Err(2),
    }
}

fn degraded() -> u8 {
    match Err::<u8, u8>(1) {
        Ok(one) => one,
        Err(_) => 0,
    }
}

fn main() {
    met_with_nothing();

    let _ = (handed_up().ok(), degraded(), nothing_left(Ok(())));
}
