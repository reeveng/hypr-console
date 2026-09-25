//! Where a word is defined: an item, a variant of an enum, a field of a struct.
//!
//! This is the unit the plan is written in, because it is the unit a name has
//! one meaning in. Two enums that both have a `Missing` are two decisions, and
//! the key that tells them apart -- `Missing variant of Unbuilt` -- is what a
//! person reads in the plan and what the apply finds again after the lines
//! around it have moved.

use crate::lexing::{blank, identifiers};

const ITEMS: [&str; 8] = ["struct", "enum", "type", "trait", "union", "fn", "const", "static"];

const KEYWORDS: [&str; 12] = ["fn", "struct", "enum", "type", "trait", "union", "const", "static", "impl", "for", "mut", "unsafe"];

pub struct Definition {
    pub word: String,
    pub kind: &'static str,
    pub owner: Option<String>,
    pub line: u32,
    pub column: u32,
    pub context: String,
}

impl Definition {
    fn at(word: &str, kind: &'static str, owner: Option<&str>, offset: usize, src: &str) -> Definition {
        let line_start = src[..offset].rfind('\n').map_or(0, |n| n + 1);
        let line_end = src[offset..].find('\n').map_or(src.len(), |n| offset + n);

        Definition {
            word: word.to_string(),
            kind,
            owner: owner.map(str::to_string),
            line: u32::try_from(src[..offset].matches('\n').count()).unwrap_or(u32::MAX),
            column: u32::try_from(src[line_start..offset].encode_utf16().count()).unwrap_or(u32::MAX),
            context: src[line_start..line_end].trim().to_string(),
        }
    }

    pub fn key(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{} {} of {}", self.word, self.kind, owner),
            None => format!("{} {}", self.word, self.kind),
        }
    }
}

fn closing(code: &[u8], opening: usize) -> usize {
    let mut depth = 0i32;

    for (i, byte) in code.iter().enumerate().skip(opening) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            },
            _ => {},
        }
    }

    code.len()
}

fn members(code: &str, start: usize, end: usize) -> Vec<(usize, &str)> {
    let bytes = code.as_bytes();
    let mut depth = 0i32;
    let mut pieces = Vec::new();
    let mut from = start;

    for i in start..end {
        match bytes[i] {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'>' if bytes[i - 1] != b'-' => depth -= 1,
            b',' if depth == 0 => {
                pieces.push((from, i));
                from = i + 1;
            },
            _ => {},
        }
    }
    pieces.push((from, end));

    pieces.into_iter().filter_map(|(from, to)| member(code, from, to)).collect()
}

fn member(code: &str, from: usize, to: usize) -> Option<(usize, &str)> {
    let bytes = code.as_bytes();
    let mut i = from;

    loop {
        while i < to && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            Some(b'#') => {
                let open = i + code[i..to].find('[')?;
                let mut depth = 0i32;

                i = open;
                while i < to {
                    match bytes[i] {
                        b'[' => depth += 1,
                        b']' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        },
                        _ => {},
                    }
                    i += 1;
                }
                i += 1;
            },
            _ => break,
        }
    }
    let (at, word) = identifiers(&code[i..to]).next()?;
    let at = i + at;

    match word {
        "pub" => {
            let mut after = at + 3;

            while after < to && bytes[after].is_ascii_whitespace() {
                after += 1;
            }
            if bytes.get(after) == Some(&b'(') {
                after += code[after..to].find(')')? + 1;
            }
            identifiers(&code[after..to]).next().map(|(n, word)| (after + n, word))
        },
        _ => Some((at, word)),
    }
}

pub fn definitions(src: &str, wanted: &dyn Fn(&str) -> bool) -> Vec<Definition> {
    let code = blank(src);
    let words: Vec<(usize, &str)> = identifiers(&code).collect();
    let mut found = Vec::new();

    for pair in words.windows(2) {
        let [(_, kind), (at, name)] = pair else { continue };
        let Some(kind) = ITEMS.iter().find(|item| *item == kind) else { continue };

        if KEYWORDS.contains(name) {
            continue;
        }
        if !code[..*at].trim_end().ends_with(kind) {
            continue;
        }
        if wanted(name) {
            found.push(Definition::at(name, kind, None, *at, src));
        }
        if *kind != "enum" && *kind != "struct" {
            continue;
        }
        let after = at + name.len();
        let Some(opening) = code[after..].find(['{', ';', '(']).map(|n| after + n) else { continue };

        if code.as_bytes()[opening] != b'{' {
            continue;
        }
        let member_kind = match *kind {
            "enum" => "variant",
            _ => "field",
        };

        for (offset, member) in members(&code, opening + 1, closing(code.as_bytes(), opening)) {
            if wanted(member) {
                found.push(Definition::at(member, member_kind, Some(name), offset, src));
            }
        }
    }

    found
}

#[cfg(test)]
mod tests {
    use super::definitions;

    #[test]
    fn an_item_a_variant_and_a_field_are_each_found_with_their_owner() {
        let src = "pub enum Unbuilt {\n    #[default]\n    Missing,\n    Held(Vec<(u8, u8)>),\n}\nstruct Size { pub(crate) wide: u32, tall: Box<dyn Fn(u8) -> u8> }\nfn Missing() {}\n";
        let found = definitions(src, &|word| matches!(word, "Missing" | "Held" | "wide" | "tall"));
        let keys: Vec<String> = found.iter().map(|d| d.key()).collect();

        assert_eq!(keys, ["Missing variant of Unbuilt", "Held variant of Unbuilt", "wide field of Size", "tall field of Size", "Missing fn"]);
        assert_eq!((found[0].line, found[0].column), (2, 4));
    }
}
