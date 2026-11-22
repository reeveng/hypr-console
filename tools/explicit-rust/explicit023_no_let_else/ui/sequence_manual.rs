// edition:2024

fn dynamic(row: &[Option<u8>]) -> u8 {
    let [Some(value)] = row else {
        return 0;
    };
    *value
}

fn rest(row: &[Option<u8>; 2]) -> u8 {
    let [Some(value), ..] = row else {
        return 0;
    };
    *value
}

fn owned() -> Option<String> {
    Some(String::new())
}

fn temporary() -> usize {
    let (Some(left), Some(right)) = (owned(), owned()) else {
        return 0;
    };
    left.len() + right.len()
}

fn main() {}
