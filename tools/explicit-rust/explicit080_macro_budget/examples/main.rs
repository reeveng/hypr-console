macro_rules! said {
    ($held:expr) => {
        $held + 1
    };
}

fn held(held: u8) -> u8 {
    held
}

fn main() {
    let _ = (said!(1), held(1));
}
