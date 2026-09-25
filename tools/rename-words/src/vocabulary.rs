//! What `vocabulary.conf` says, read as a mapping from a retired word to the
//! words it may become.
//!
//! The file is prose that argues as much as it is a table, so only a line of
//! the shape `Old = New` is read, and of its right side only what could be an
//! identifier: a note after `--`, a clause after `, where`, a backticked aside
//! are for the reader. `A, B` and `A or B` are options. A `[what X was]`
//! section is the choice already made per crate or module, which is the one
//! thing the table says about a place rather than a word. The words under
//! `[what a person reads]` are Apple's for a screen; as a name they are only
//! ever offered, never assumed.

use std::collections::BTreeMap;
use std::path::Path;

pub struct Vocabulary {
    pub words: BTreeMap<String, Vec<String>>,
    pub sites: BTreeMap<String, Vec<Site>>,
}

pub struct Site {
    pub place: String,
    pub name: String,
}

pub fn is_identifier(word: &str) -> bool {
    let mut chars = word.chars();

    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => chars.all(|c| c.is_ascii_alphanumeric() || c == '_'),
        _ => false,
    }
}

pub fn qualified(word: &str) -> Option<(&str, &str)> {
    let (owner, member) = word.split_once("::")?;

    match is_identifier(owner) && is_identifier(member) {
        true => Some((owner, member)),
        false => None,
    }
}

pub fn snake(word: &str) -> String {
    let mut out = String::new();
    let mut previous_lower = false;

    for c in word.chars() {
        if c.is_ascii_uppercase() && previous_lower {
            out.push('_');
        }
        previous_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        out.push(c.to_ascii_lowercase());
    }

    out
}

fn spoken(words: &str) -> String {
    words
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();

            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

fn options_of(right: &str) -> Vec<String> {
    let cut = [" -- ", ";", ", where", ", for", ", the", ", whichever"]
        .iter()
        .filter_map(|mark| right.find(mark))
        .min()
        .unwrap_or(right.len());
    let mut kept = String::new();
    let mut quoted = false;

    for c in right[..cut].chars() {
        match c {
            '`' => quoted = !quoted,
            _ if !quoted => kept.push(c),
            _ => {},
        }
    }

    kept.split(',')
        .flat_map(|piece| piece.split(" or "))
        .flat_map(|piece| piece.split('|'))
        .map(str::trim)
        .filter(|piece| is_identifier(piece) || qualified(piece).is_some())
        .map(str::to_string)
        .collect()
}

fn per_site(section: &str) -> Option<Vec<String>> {
    let inside = section.strip_prefix("[what ")?.strip_suffix(']')?;
    let named = inside.strip_suffix(" was").or_else(|| inside.strip_suffix(" were"))?;

    Some(named.split(", ").flat_map(|piece| piece.split(" and ")).map(str::to_string).collect())
}

pub fn read(root: &Path) -> std::io::Result<Vocabulary> {
    let text = std::fs::read_to_string(root.join("vocabulary.conf"))?;
    let mut words: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut sites: BTreeMap<String, Vec<Site>> = BTreeMap::new();
    let mut section = String::new();

    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = line.split(" (").next().unwrap_or(line).to_string();
            continue;
        }
        let Some((left, right)) = line.split_once(" = ") else { continue };
        let (left, right) = (left.trim(), right.trim());

        if let Some(named) = per_site(&section) {
            if qualified(left).is_none() {
                let (places, inside) = match left.split_once(" (") {
                    Some((places, rest)) => (places, rest.strip_suffix(')')),
                    None => (left, None),
                };
                let word = match inside {
                    Some(inside) if named.iter().any(|n| n == inside) => inside.to_string(),
                    _ => named.first().cloned().unwrap_or_default(),
                };
                let module = inside.filter(|inside| !named.iter().any(|n| n == inside));

                for place in places.split(',').map(str::trim) {
                    let place = match module {
                        Some(module) => format!("{place}/{module}"),
                        None => place.to_string(),
                    };

                    sites.entry(word.clone()).or_default().push(Site { place, name: right.to_string() });
                }
                words.entry(word).or_default();
                continue;
            }
        }
        if left.is_empty() || !(is_identifier(left) || qualified(left).is_some()) {
            continue;
        }
        let options = match section.as_str() {
            "[what a person reads]" => {
                let name = spoken(right);

                match is_identifier(&name) {
                    true => vec![name],
                    false => Vec::new(),
                }
            },
            _ => options_of(right),
        };

        if left.eq_ignore_ascii_case(right) || options == [left] {
            continue;
        }
        let left = match options.iter().any(|o| o.starts_with(|c: char| c.is_ascii_lowercase())) && qualified(left).is_none() {
            true => snake(left),
            false => left.to_string(),
        };
        let entry = words.entry(left.clone()).or_default();

        for option in options {
            if option != left && !entry.contains(&option) {
                entry.push(option);
            }
        }
    }

    Ok(Vocabulary { words, sites })
}

impl Vocabulary {
    pub fn site_choice(&self, word: &str, relative: &str) -> Option<&str> {
        let mut best: Option<(u8, &str)> = None;

        for site in self.sites.get(word)? {
            let (krate, module) = match site.place.split_once('/') {
                Some((krate, module)) => (krate, Some(module)),
                None => (site.place.as_str(), None),
            };

            if !relative.starts_with(&format!("crates/{krate}/")) {
                continue;
            }
            if let Some(module) = module {
                if !relative.contains(&format!("/{module}.rs")) && !relative.contains(&format!("/{module}/")) {
                    continue;
                }
            }
            let rank = match module {
                Some(_) => 2,
                None => 1,
            };

            if best.is_none_or(|(held, _)| rank > held) {
                best = Some((rank, site.name.as_str()));
            }
        }

        best.map(|(_, name)| name)
    }
}
