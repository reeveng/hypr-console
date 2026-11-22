// edition:2024

fn rebound(mut data: &[u8]) -> u8 {
    match data.len() {
        1 => { data = &[]; data[0] }
        _ => 0,
    }
}

fn guarded(data: &[u8], keep: bool) -> u8 {
    match data.len() {
        0 if keep => 0,
        _ => data[0],
    }
}

fn main() {}
