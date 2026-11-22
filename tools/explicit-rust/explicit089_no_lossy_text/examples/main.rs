use std::path::Path;

fn named(at: &Path) -> String {
    //~v WARN EXPLICIT089_NO_LOSSY_TEXT
    at.to_string_lossy().into_owned()
}

fn said(bytes: &[u8]) -> String {
    //~v WARN EXPLICIT089_NO_LOSSY_TEXT
    String::from_utf8_lossy(bytes).into_owned()
}

fn asked(at: &Path) -> Option<&str> {
    at.to_str()
}

fn met(bytes: &[u8]) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(bytes)
}

struct Page;

impl Page {
    fn to_string_lossy(&self) -> u8 {
        1
    }
}

fn main() {
    let at = Path::new("somewhere");
    let _ = named(at);
    let _ = said(b"words");
    let _ = asked(at);
    let _ = met(b"words");
    let _ = Page.to_string_lossy();
}
