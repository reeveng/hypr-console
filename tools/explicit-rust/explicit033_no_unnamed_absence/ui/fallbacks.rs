// run-rustfix
// edition:2024

fn fallback(input: Option<u32>, alternate: u32) -> u32 {
    let width = input.unwrap_or(alternate);
    width
}

fn defaults(input: Option<&str>) -> &str {
    input.unwrap_or_default()
}

fn argument(input: Option<u32>) -> String {
    stringify(input.unwrap_or(3))
}

fn stringify(value: u32) -> String {
    value.to_string()
}

fn chained(input: Option<(u32, bool)>) -> u32 {
    input.unwrap_or((u32::MAX, true)).0
}

fn lazy(input: Option<&str>) -> &str {
    input.unwrap_or_else(|| "untitled")
}

fn lazy_block(input: Option<String>) -> String {
    input.unwrap_or_else(|| {
        let mut title = String::from("untitled");
        title.push('!');
        title
    })
}

fn empty_slice(input: Option<&[u8]>) -> &[u8] {
    input.unwrap_or_default()
}

fn mutable_slice(input: Option<&mut [u8]>) -> &mut [u8] {
    input.unwrap_or_default()
}

fn standard_defaults(input: Option<(Vec<u8>, String, [u16; 3])>) -> (Vec<u8>, String, [u16; 3]) {
    input.unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Open,
    Closed,
}

fn assign(input: Option<Mode>, destination: &mut Mode) {
    *destination = input.unwrap_or(Mode::Closed);
}

fn array(input: Option<[u16; 3]>, alternate: (u16,)) -> [u16; 3] {
    input.unwrap_or([alternate.0, 20, 30])
}

fn eager(counter: &mut u32) -> u32 {
    has_value(counter).unwrap_or(compute_fallback(counter))
}

fn has_value(counter: &mut u32) -> Option<u32> {
    *counter = 1;
    Some(7)
}

fn compute_fallback(counter: &mut u32) -> u32 {
    assert_eq!(*counter, 1);
    *counter = 2;
    9
}

fn main() {
    assert_eq!(fallback(Some(1), 2), 1);
    assert_eq!(fallback(None, 2), 2);
    assert_eq!(defaults(None), "");
    assert_eq!(argument(None), "3");
    assert_eq!(chained(None), u32::MAX);
    assert_eq!(lazy(None), "untitled");
    assert_eq!(lazy_block(None), "untitled!");
    assert_eq!(empty_slice(None), &[]);
    assert_eq!(mutable_slice(None), &mut []);
    assert_eq!(standard_defaults(None), (Vec::new(), String::new(), [0; 3]));
    let mut mode = Mode::Open;
    assign(None, &mut mode);
    assert_eq!(mode, Mode::Closed);
    assert_eq!(array(None, (10,)), [10, 20, 30]);
    let mut next = 0;
    assert_eq!(eager(&mut next), 7);
    assert_eq!(next, 2);
}
