// edition:2024

fn evaluate(result: Option<Option<u8>>) -> Option<u8> {
    if let Some(ref score) = result? {
        return Some(*score);
    }
    Some(0)
}

fn main() {
    assert_eq!(evaluate(Some(Some(5))), Some(5));
    assert!(read_address().is_some());
}

fn address(place: &u32) -> Option<Option<u64>> {
    Some(Some(std::ptr::from_ref(place).addr() as u64))
}

fn word() -> u32 {
    5
}

fn read_address() -> Option<u64> {
    if let Some(address) = address(&word())? {
        return Some(address);
    }
    None
}
