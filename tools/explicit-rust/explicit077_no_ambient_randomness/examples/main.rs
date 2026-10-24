use rand::{thread_rng, Rng};


fn shaken() -> u8 {
    thread_rng().r#gen::<u8>()
}

fn taken() -> u8 {
    let _ = std::time::SystemTime::now();

    shaken()
}

fn main() {
    let _ = taken();
}
