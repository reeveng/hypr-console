// edition:2024
macro_rules! spelling {
    ($expression:expr) => { ($expression, stringify!($expression)) };
}

fn code_and_spelling(value: u8) -> (String, &'static str) {
    let value = value + 1;
    spelling!(format!("{value}"))
}

fn named_argument(value: u8) -> (String, u8) {
    let value = value + 1;
    (format!("{value}", value = 61), value)
}

fn escaped_capture(value: u8) -> String {
    let value = value + 1;
    format!("{\u{76}alue}")
}

mod local {
    macro_rules! format {
        ($literal:literal) => { (std::format!($literal), stringify!($literal)) };
    }
    pub fn render(value: u8) -> (String, &'static str) {
        let value = value + 1;
        format!("{value}")
    }
}

fn main() {
    assert_eq!(code_and_spelling(3), ("4".to_owned(), "format!(\"{value}\")"));
    assert_eq!(named_argument(3), ("61".to_owned(), 4));
    assert_eq!(escaped_capture(4), "5");
    assert_eq!(local::render(5), ("6".to_owned(), "\"{value}\""));
}
