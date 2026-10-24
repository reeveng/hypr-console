#[derive(Debug)]
enum CloseTogether {
    One = 0,
    Two = 1,
}

#[derive(Debug)]
enum Spaced {
    One = 0b0000,
    Two = 0b0011,
    Three = 0b0101,
    Four = 0b1001,
}

#[derive(Debug)]
enum Unsaid {
    One,
    Two,
    Three,
}

fn main() {
    let _ = (CloseTogether::Two as u8, Spaced::Four as u8, Unsaid::Three as u8);
}
