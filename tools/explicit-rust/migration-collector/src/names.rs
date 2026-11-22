//! A name for a value lifted out of an expression, taken from what the expression asks for.
//!
//! A lifted value is read by a person afterwards, so it is named the way a person would name it:
//! after the method or function without its `get_`, `to_` or `as_`, after the field, after the type
//! a constructor builds, or after the argument a conversion is handed. A name already said in the
//! function, a keyword, a primitive type's name and a single letter are refused, and a value whose
//! name cannot be derived is left for a person rather than numbered.

use std::collections::BTreeSet;
use syn::{Expr, Member};

const PREFIXES: [&str; 9] = ["get_", "to_", "as_", "into_", "try_", "read_", "parse_", "decode_", "next_"];

const CONVERSIONS: [&str; 13] = [
    "wrapped",
    "fitted",
    "index",
    "wrapped_index",
    "wrapped_offset",
    "nearest_f32_from_u64",
    "narrow_f32",
    "from",
    "try_from",
    "Some",
    "Ok",
    "clone",
    "unwrap",
];

const COMBINATORS: [&str; 22] = [
    "map",
    "and_then",
    "or_else",
    "unwrap_or",
    "unwrap_or_else",
    "unwrap_or_default",
    "ok_or",
    "ok_or_else",
    "ok",
    "err",
    "cloned",
    "copied",
    "clone",
    "to_owned",
    "to_vec",
    "to_string",
    "as_ref",
    "as_mut",
    "as_deref",
    "unwrap",
    "expect",
    "filter",
];

const REFUSED: [&str; 24] = [
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
    "bool", "char", "str", "new", "from", "get", "it", "value", "result", "self",
];

pub struct Namer<'a> {
    pub taken: &'a BTreeSet<String>,
    pub own_type: Option<&'a str>,
}

pub fn snake(word: &str) -> String {
    let mut out = String::new();
    let characters = word.chars().collect::<Vec<_>>();
    for (index, character) in characters.iter().enumerate() {
        if character.is_uppercase() {
            let after_lower = index > 0 && characters[index - 1].is_lowercase();
            let before_lower = characters.get(index + 1).is_some_and(|next| next.is_lowercase());
            let after_upper = index > 0 && characters[index - 1].is_uppercase();
            if index > 0 && (after_lower || (after_upper && before_lower)) {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        } else {
            out.push(*character);
        }
    }
    out
}

pub fn singular(word: &str) -> String {
    if let Some(stem) = word.strip_suffix("ies") {
        return format!("{stem}y");
    }
    for ending in ["sses", "xes", "ches", "shes"] {
        if let Some(stem) = word.strip_suffix(ending) {
            return format!("{stem}{}", &ending[..ending.len() - 2]);
        }
    }
    match word.strip_suffix('s') {
        Some(stem) if !stem.ends_with('s') && stem.len() > 1 => stem.to_owned(),
        _ => word.to_owned(),
    }
}

fn stripped(word: &str) -> String {
    let mut word = word;
    for prefix in PREFIXES {
        if let Some(rest) = word.strip_prefix(prefix) {
            word = rest;
            break;
        }
    }
    match word {
        "len" => "length".to_owned(),
        _ => word.to_owned(),
    }
}

const ARITHMETIC: [&str; 12] = [
    "wrapping_", "checked_", "saturating_", "overflowing_", "min", "max", "abs", "pow", "clamp", "signum", "sqrt", "powi",
];

fn usable(word: &str) -> bool {
    word.len() > 1
        && !ARITHMETIC.iter().any(|arithmetic| word == *arithmetic || (arithmetic.ends_with('_') && word.starts_with(arithmetic)))
        && !REFUSED.contains(&word)
        && !word.starts_with(char::is_numeric)
        && word.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !word.starts_with('_')
        && syn::parse_str::<syn::Ident>(word).is_ok()
}

fn residue(expression: &Expr) -> Option<&Expr> {
    let block = match expression {
        Expr::Paren(paren) => match &*paren.expr {
            Expr::Block(block) => block,
            _ => return None,
        },
        Expr::Block(block) => block,
        _ => return None,
    };
    match block.block.stmts.as_slice() {
        [syn::Stmt::Local(local), syn::Stmt::Expr(Expr::Path(_), None)] => local.init.as_ref().map(|init| &*init.expr),
        _ => None,
    }
}

fn matched(expression: &Expr) -> Option<&Expr> {
    let choice = match expression {
        Expr::Paren(paren) => match &*paren.expr {
            Expr::Match(choice) => choice,
            _ => return None,
        },
        Expr::Match(choice) => choice,
        _ => return None,
    };
    match choice.arms.as_slice() {
        [arm] if arm.guard.is_none() && matches!(&*arm.body, Expr::Path(_)) => Some(&*choice.expr),
        _ => None,
    }
}

fn owner(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Path(path) => path.path.get_ident().map(|ident| ident.to_string()),
        Expr::Field(field) => owner(&field.base),
        Expr::MethodCall(call) => owner(&call.receiver),
        Expr::Index(index) => owner(&index.expr),
        Expr::Paren(inner) => owner(&inner.expr),
        Expr::Reference(inner) => owner(&inner.expr),
        Expr::Unary(inner) => owner(&inner.expr),
        Expr::Try(inner) => owner(&inner.expr),
        _ => None,
    }
}

fn last_segment(path: &syn::Path) -> Option<String> {
    path.segments.last().map(|segment| segment.ident.to_string())
}

impl Namer<'_> {
    fn of_type(&self, path: &syn::Path) -> Option<String> {
        let segments = path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>();
        let owner = match segments.len() {
            0 | 1 => return None,
            n => segments[n - 2].clone(),
        };
        let owner = match owner.as_str() {
            "Self" => self.own_type?.to_owned(),
            _ => owner,
        };
        match owner.starts_with(char::is_uppercase) {
            true => Some(snake(&owner)),
            false => None,
        }
    }

    pub fn words(&self, expression: &Expr) -> Vec<String> {
        match expression {
            Expr::MethodCall(call) => {
                let method = call.method.to_string();
                if COMBINATORS.contains(&method.as_str()) {
                    return self.words(&call.receiver);
                }
                if method == "get" || method == "get_mut" || method == "first" || method == "last" {
                    let held = self.words(&call.receiver);
                    let mut found = held.iter().map(|word| singular(word)).collect::<Vec<_>>();
                    if method == "first" || method == "last" {
                        found.extend(held.iter().map(|word| format!("{method}_{}", singular(word))));
                    }
                    return found;
                }
                let base = stripped(&method);
                if !usable(&base) {
                    return vec![];
                }
                let mut found = vec![base.clone()];
                let held = owner(&call.receiver).or_else(|| self.words(&call.receiver).into_iter().next());
                match held.as_deref() {
                    Some("self") => found.push(format!("own_{base}")),
                    Some(owner) => found.push(format!("{owner}_{base}")),
                    None => {}
                }
                found
            }
            Expr::Call(call) => {
                let Expr::Path(callee) = &*call.func else {
                    return vec![];
                };
                let Some(name) = last_segment(&callee.path) else {
                    return vec![];
                };
                let conversion = CONVERSIONS.contains(&name.as_str())
                    || name.starts_with("toward_zero_")
                    || name.starts_with("whole_");
                let constructor = name == "new"
                    || name == "default"
                    || name.starts_with("from_")
                    || name.starts_with("with_")
                    || name.starts_with("new_");
                let mut found = vec![];
                if conversion {
                    if let Some(first) = call.args.first() {
                        found.extend(self.words(first));
                    }
                }
                if constructor || (conversion && found.is_empty()) {
                    found.extend(self.of_type(&callee.path));
                }
                if !conversion && !constructor && usable(&stripped(&name)) {
                    found.push(stripped(&name));
                    if let Some(owner) = self.of_type(&callee.path) {
                        found.push(format!("{owner}_{}", stripped(&name)));
                    }
                }
                found
            }
            Expr::Field(field) => match &field.member {
                Member::Named(name) => {
                    let name = name.to_string();
                    let mut found = vec![name.clone()];
                    match owner(&field.base).as_deref() {
                        Some("self") => found.push(format!("own_{name}")),
                        Some(owner) => found.push(format!("{owner}_{name}")),
                        None => {}
                    }
                    found
                }
                Member::Unnamed(_) => self.words(&field.base),
            },
            Expr::Path(path) => match path.path.get_ident() {
                Some(ident) => vec![ident.to_string()],
                None => last_segment(&path.path).map(|name| snake(&name)).into_iter().collect(),
            },
            Expr::Index(index) => self.words(&index.expr).iter().map(|word| singular(word)).collect(),
            Expr::Paren(inner) => match residue(expression).or_else(|| matched(expression)) {
                Some(init) => self.words(init),
                None => self.words(&inner.expr),
            },
            Expr::Match(_) => matched(expression).map_or_else(Vec::new, |init| self.words(init)),
            Expr::Block(_) => residue(expression).map_or_else(Vec::new, |init| self.words(init)),
            Expr::Group(inner) => self.words(&inner.expr),
            Expr::Reference(inner) => self.words(&inner.expr),
            Expr::Unary(inner) => self.words(&inner.expr),
            Expr::Try(inner) => self.words(&inner.expr),
            Expr::Cast(inner) => self.words(&inner.expr),
            Expr::Await(inner) => self.words(&inner.base),
            _ => vec![],
        }
    }

    pub fn name(&self, expression: &Expr, chosen: &BTreeSet<String>) -> Option<String> {
        self.words(expression)
            .into_iter()
            .map(|word| word.trim_matches('_').to_owned())
            .find(|word| usable(word) && !self.taken.contains(word) && !chosen.contains(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(text: &str, taken: &[&str]) -> Option<String> {
        let taken = taken.iter().map(|word| word.to_string()).collect();
        let namer = Namer { taken: &taken, own_type: Some("IntRect") };
        namer.name(&syn::parse_str(text).unwrap(), &BTreeSet::new())
    }

    #[test]
    fn methods_lose_their_prefix_and_fields_keep_their_name() {
        assert_eq!(named("self.right()", &[]).as_deref(), Some("right"));
        assert_eq!(named("self.get_width()", &[]).as_deref(), Some("width"));
        assert_eq!(named("glyph.to_unicode()", &[]).as_deref(), Some("unicode"));
        assert_eq!(named("self.data.name", &[]).as_deref(), Some("name"));
    }

    #[test]
    fn a_taken_name_falls_back_to_its_owner() {
        assert_eq!(named("other.right()", &["right"]).as_deref(), Some("other_right"));
        assert_eq!(named("self.right()", &["right"]).as_deref(), Some("own_right"));
        assert_eq!(
            named("other.data.name.as_expanded_name(other.doc)", &["expanded_name"]).as_deref(),
            Some("other_expanded_name")
        );
        assert_eq!(named("tokenizer::Stream::new(self.text)", &[]).as_deref(), Some("stream"));
        assert_eq!(
            named("({ let Ok(explicit_value) = PathStroker::new(); explicit_value }).stroke(self, stroke)", &["stroke"]).as_deref(),
            Some("path_stroker_stroke")
        );
    }

    #[test]
    fn constructors_are_named_for_their_type() {
        assert_eq!(named("IntRect::from_xywh(1, 2, 3, 4)", &[]).as_deref(), Some("int_rect"));
        assert_eq!(named("Self::new(1)", &[]).as_deref(), Some("int_rect"));
    }

    #[test]
    fn conversions_are_named_for_what_they_convert() {
        assert_eq!(
            named("console_core_number_conversion::wrapped::<_, i32>(self.width.get())", &[]).as_deref(),
            Some("width")
        );
        assert_eq!(named("index(self.glyphs.len())", &[]).as_deref(), Some("length"));
    }

    #[test]
    fn collections_give_their_element_a_singular_name() {
        assert_eq!(named("self.entries.get(at)", &[]).as_deref(), Some("entry"));
        assert_eq!(named("glyphs[0]", &[]).as_deref(), Some("glyph"));
    }

    #[test]
    fn nothing_derivable_is_left_rather_than_numbered() {
        assert_eq!(named("1 + 2", &[]), None);
        assert_eq!(named("reader.read_u16()", &[]), None);
        assert_eq!(named("self.x()", &[]), None);
        assert_eq!(named("u64::wrapping_add(a, b)", &[]), None);
        assert_eq!(named("a.min(b)", &[]), None);
    }

    #[test]
    fn snake_case_reads_acronyms_as_one_word() {
        assert_eq!(snake("IntRect"), "int_rect");
        assert_eq!(snake("PDFObject"), "pdf_object");
        assert_eq!(singular("boxes"), "box");
    }
}
