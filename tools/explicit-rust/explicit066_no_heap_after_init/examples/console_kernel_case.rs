fn boxed() -> Box<u8> {
    //~v WARN EXPLICIT066_NO_HEAP_AFTER_INIT
    Box::new(1)
}

fn listed() -> Vec<u8> {
    //~v WARN EXPLICIT066_NO_HEAP_AFTER_INIT
    Vec::new()
}

fn said() -> String {
    //~v WARN EXPLICIT066_NO_HEAP_AFTER_INIT
    format!("{} lines", 1)
}

fn spent() -> Vec<u8> {
    //~v WARN EXPLICIT066_NO_HEAP_AFTER_INIT
    vec![1, 2]
}

fn filled() -> Vec<u8> {
    //~v WARN EXPLICIT066_NO_HEAP_AFTER_INIT
    vec![0; 4]
}

fn copied() -> String {
    //~v WARN EXPLICIT066_NO_HEAP_AFTER_INIT
    String::from("line")
}

fn spelled() -> String {
    //~v WARN EXPLICIT066_NO_HEAP_AFTER_INIT
    "line".to_string()
}

fn widened(held: u8) -> u32 {
    u32::from(held)
}

fn fixed() -> [u8; 2] {
    [1, 2]
}

fn main() {
    let _ = (boxed(), listed(), said(), spent(), fixed());
    let _ = (filled(), copied(), spelled(), widened(1));
}
