// run-rustfix
// edition:2024
use std::fmt::Write;

fn serialize(name: u8, took: f64) -> String {
    let name = name + 1;
    let took = took * 2.0;
    format!("name={name}, {{name}}, {name:x}, {took:.1}")
}

fn width(width: usize) -> String {
    let width = width + 1;
    format!(r#"width {number:width$.width$}"#, number = 3.5,)
}

fn write_value(value: u8) -> String {
    let mut out = String::new();
    let value = value + 1;
    write!(out, "{} {value}", "value").unwrap();
    writeln!(out, " {value}").unwrap();
    out
}

fn select(value: u8) -> String {
    let text = format!("outer {value}");
    let value = value + 1;
    format!("{} {value}", format!("inner {value}"),) + &text
}

fn keyword(r#type: u8) -> String {
    let r#type = r#type + 1;
    format!("{type}")
}

fn print_value(value: u8) {
    let value = value + 1;
    print!("out {value}");
    println!(" {value}");
    eprint!("err {value}");
    eprintln!(" {value}");
    std::fmt::write(&mut String::new(), format_args!("{value}")).unwrap();
}

fn explicit(value: u8) -> String {
    let value = value + 1;
    format!("value={{value}} / {}", value)
}

fn separate_bindings(value: u8) -> (String, [String; 1]) {
    let value = value + 1;
    (format!("{value}"), [7].map(|value| format!("{value}")))
}

struct ErrorMessage;
impl std::fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", std::fmt::from_fn(|f| f.write_str("first")),
            std::fmt::from_fn(|f| f.write_str("second")))
    }
}

fn main() {
    assert_eq!(serialize(15, 1.25), "name=16, {name}, 10, 2.5");
    assert_eq!(width(2), "width 3.500");
    assert_eq!(write_value(4), "value 5 5\n");
    assert_eq!(select(2), "inner 3 3outer 2");
    assert_eq!(keyword(1), "2");
    print_value(5);
    assert_eq!(explicit(2), "value={value} / 3");
    assert_eq!(separate_bindings(1), ("2".to_owned(), ["7".to_owned()]));
    assert_eq!(ErrorMessage.to_string(), "first second");
}
