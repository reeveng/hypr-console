fn parse() -> Result<u8, std::num::ParseIntError> {
    "20".parse()
}

fn main() {
    parse();
    let lock = std::sync::Mutex::new(0);
    lock.lock();
}
