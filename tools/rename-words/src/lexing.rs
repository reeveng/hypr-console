//! A Rust file with everything that is not code painted over.
//!
//! A string says what somebody reads and a comment says what somebody meant,
//! and neither is a name. What comes back is the same file, byte for byte the
//! same length, with every byte of a string, a character or a comment turned
//! into a space and every newline kept -- so an offset found in it is an
//! offset in the file, and a line counted in it is a line of the file.

fn continues_a_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn after_a_name(bytes: &[u8], at: usize) -> bool {
    at > 0 && continues_a_name(bytes[at - 1])
}

fn raw_string(bytes: &[u8], at: usize) -> Option<usize> {
    let mut i = at;

    if bytes.get(i) == Some(&b'b') {
        i += 1;
    }
    if bytes.get(i) != Some(&b'r') {
        return None;
    }
    i += 1;
    let hashes = bytes[i..].iter().take_while(|b| **b == b'#').count();

    i += hashes;
    if bytes.get(i) != Some(&b'"') {
        return None;
    }
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'"' && bytes[i + 1..].iter().take(hashes).filter(|b| **b == b'#').count() == hashes {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }

    Some(bytes.len())
}

fn quoted(bytes: &[u8], at: usize) -> usize {
    let mut i = at + 1;

    while i < bytes.len() && bytes[i] != b'"' {
        i += match bytes[i] {
            b'\\' => 2,
            _ => 1,
        };
    }

    (i + 1).min(bytes.len())
}

fn character(bytes: &[u8], at: usize) -> Option<usize> {
    let rest = &bytes[at + 1..];

    match rest.first()? {
        b'\\' => {
            let close = rest.iter().skip(1).position(|b| *b == b'\'')?;

            (close < 12).then_some(at + 1 + 1 + close + 1)
        },
        first => {
            let width = match first {
                0x00..=0x7f => 1,
                0xc0..=0xdf => 2,
                0xe0..=0xef => 3,
                _ => 4,
            };

            (rest.get(width) == Some(&b'\'')).then_some(at + 1 + width + 1)
        },
    }
}

pub fn blank(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    let paint = |from: usize, to: usize, out: &mut Vec<u8>| {
        for byte in &mut out[from..to] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };

    while i < bytes.len() {
        let end = match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => Some(bytes[i..].iter().position(|b| *b == b'\n').map_or(bytes.len(), |n| i + n)),
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                Some(src[i + 2..].find("*/").map_or(bytes.len(), |n| i + 2 + n + 2))
            },
            b'b' | b'r' if !after_a_name(bytes, i) => raw_string(bytes, i),
            b'"' => Some(quoted(bytes, i)),
            b'b' if bytes.get(i + 1) == Some(&b'"') && !after_a_name(bytes, i) => Some(quoted(bytes, i + 1)),
            b'\'' => character(bytes, i),
            _ => None,
        };

        match end {
            Some(end) => {
                paint(i, end, &mut out);
                i = end;
            },
            None if continues_a_name(bytes[i]) => {
                while i < bytes.len() && continues_a_name(bytes[i]) {
                    i += 1;
                }
            },
            None => i += 1,
        }
    }

    String::from_utf8(out).unwrap_or_default()
}

pub fn identifiers(code: &str) -> impl Iterator<Item = (usize, &str)> {
    let bytes = code.as_bytes();
    let mut i = 0;

    std::iter::from_fn(move || {
        while i < bytes.len() {
            let start = i;

            match bytes[i] {
                b if b.is_ascii_alphabetic() || b == b'_' => {
                    while i < bytes.len() && continues_a_name(bytes[i]) {
                        i += 1;
                    }

                    return Some((start, &code[start..i]));
                },
                b if b.is_ascii_digit() => {
                    while i < bytes.len() && continues_a_name(bytes[i]) {
                        i += 1;
                    }
                },
                _ => i += 1,
            }
        }

        None
    })
}

#[cfg(test)]
mod tests {
    use super::{blank, identifiers};

    #[test]
    fn only_code_is_left_and_every_offset_still_holds() {
        let src = "let a = \"Held\"; // Held\nlet b = 'x'; let c: &'a str = r#\"Held\"#; Held";
        let code = blank(src);

        assert_eq!(code.len(), src.len());
        assert_eq!(identifiers(&code).filter(|(_, word)| *word == "Held").count(), 1);
        assert!(code.contains("'a"), "a lifetime is code");
    }
}
