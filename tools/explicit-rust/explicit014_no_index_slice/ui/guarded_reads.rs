// run-rustfix
// edition:2024

use std::sync::atomic::{AtomicUsize, Ordering};

static CHECKS: AtomicUsize = AtomicUsize::new(0);

fn sees_x(byte: u8) -> bool {
    CHECKS.fetch_add(1, Ordering::Relaxed);
    byte == b'x'
}

struct Scanner<'a> { bytes: &'a [u8], at: usize }

impl Scanner<'_> {
    fn has(&self, byte: u8) -> bool {
        self.at < self.bytes.len() && self.bytes[self.at] == byte
    }
}

fn lookahead(bytes: &[u8], at: usize, enabled: bool) -> bool {
    enabled && at + 1 < bytes.len() && sees_x(bytes[at + 1]) && bytes[at + 1] < b'z'
}

fn starts(bytes: &[u8], byte: u8) -> bool {
    !bytes.is_empty() && bytes[0] == byte
}

fn invalid_first(bytes: &[u8]) -> bool {
    bytes.is_empty() || !sees_x(bytes[0])
}

fn uppercase_hex(bytes: &[u8], at: usize) -> bool {
    at < bytes.len() && bytes[at] | 0x20 == b'x'
}

fn grouped(bytes: Vec<u8>, at: usize, a: bool, b: bool) -> Option<&'static str> {
    ((a || b) && bytes.len() > at && (bytes[at] == b'x' || bytes[at] == b'y') && bytes[at] < b'y')
        .then_some("present")
}

fn skip_x(bytes: &[u8]) -> usize {
    let mut at = 0;
    while at < bytes.len() && bytes[at] == b'x' {
        at += 1;
    }
    at
}

fn main() {
    let reader = Scanner { bytes: b"x", at: 0 };
    assert!(reader.has(b'x'));
    assert!(!reader.has(b'a'));
    assert!(!Scanner { bytes: b"", at: 0 }.has(b'x'));
    assert!(!lookahead(b"xx", usize::MAX, false));
    assert!(!lookahead(b"x", 0, true));
    assert_eq!(CHECKS.load(Ordering::Relaxed), 0);
    assert!(lookahead(b"xx", 0, true));
    assert_eq!(CHECKS.load(Ordering::Relaxed), 1);
    assert!(starts(b"x", b'x'));
    assert!(!starts(b"", b'x'));
    assert!(invalid_first(b""));
    assert!(!invalid_first(b"x"));
    assert!(uppercase_hex(b"X", 0));
    assert!(!uppercase_hex(b"", 0));
    assert_eq!(grouped(vec![b'x'], 0, true, false), Some("present"));
    assert_eq!(grouped(vec![], 0, true, false), None);
    assert_eq!(grouped(vec![b'y'], 0, false, true), None);
    assert_eq!(grouped(vec![b'x'], 0, false, false), None);
    assert_eq!(skip_x(b"xxx"), 3);
    assert_eq!(skip_x(b"xxyx"), 2);
    assert_eq!(skip_x(b""), 0);
}
