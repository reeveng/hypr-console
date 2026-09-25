//! A function written again somewhere else in the tree, found before it lands.
//!
//! The crate rule says a literal spelled in two crates means one of them is
//! guessing, and the same is true of a function. `clock` was written three
//! times, once each in the music player, the media viewer and the downloads,
//! and the walk along `PATH` that asks whether a program is installed was
//! written four times and gave four answers to an unset `PATH`. None of them
//! looked wrong in the diff that brought it, because the first copy was never
//! in that diff.
//!
//! `cargo dylint` cannot ask this for the reason `the_families` gives: a lint
//! sees one crate. So every function in `crates/` is read here with `syn`, and
//! what it says is reduced to its shape -- a local name is any local name and a
//! string is any string, while a method, a path and a type keep their names,
//! because those are what the function does rather than what it calls things.
//! Two shapes that share most of their runs of tokens are one function in two
//! places, and the fault names both.
//!
//! What is not compared is what the rules dictate. A trait impl has the shape
//! its trait gave it, and a test is a question asked in the shape every test
//! asks one, so neither is read. A body too short to be worth calling is not
//! read either, because a line of it is cheaper to write than to import.
//!
//! `ALIKE` is the groups that are alike and stay apart, each with the reason.
//! `NOT_YET` is the groups that were already in the tree the day this was
//! written, which is the ratchet the lints use: a new copy is red at once, and
//! folding an old one is deleting its line. A group on either that no longer
//! matches is a line to delete, the way an edge in `the_families` is.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::visit::Visit;

use console_core_number_conversion::{fitted, index};

const SHORTEST: u32 = 60;

const RUN: u32 = 6;

const MET: u32 = 8;

const ALIKE_ENOUGH: f64 = 0.7;

const ALIKE: [(&[&str], &str); 9] = [
    (
        &[
            "crates/console-core-color/src/palette.rs::out_of",
            "crates/console-status-bar/src/showing.rs::out_of",
        ],
        "each surface takes the colors it wears by name, and which names is the whole of what differs",
    ),
    (
        &[
            "crates/console-home-screen/src/bin/console-home.rs::worth_asking_after",
            "crates/console-input-controller/src/binds.rs::worth_asking_after",
            "crates/console-status-bar/src/watch.rs::surface_worth_asking_after",
            "crates/console-wallpaper/src/covered.rs::worth_waking_for",
        ],
        "each listener decides which compositor events matter to it, and naming every event with no wildcard is what makes a new one get considered by each",
    ),
    (
        &[
            "crates/console-panel/src/surface.rs::looked_at",
            "crates/console-panel/src/surface.rs::still_of",
        ],
        "two questions over Picture, whether it is a file on screen and whether it is a still, and the no-wildcard rule writes the variants out once per question",
    ),
    (
        &[
            "crates/console-program-contract/src/effect.rs::spawned",
            "crates/console-program-contract/src/effect.rs::written",
        ],
        "one accessor per variant of Effect, and the no-wildcard rule writes the other variants out in each",
    ),
    (
        &[
            "crates/console-media-viewer/src/editing.rs::made",
            "crates/console-pictures/src/lib.rs::made_for",
        ],
        "each crate's tests draw their own fixture with ffmpeg, and sharing it would put a test's helper in the pictures crate's public face",
    ),
    (
        &[
            "crates/console-input-alphabets/src/wearing.rs::every",
            "crates/console-input-bindings/src/active.rs::read",
        ],
        "each reads its own file into its own type with its own default; the reading they share is console_core_atomic_writes::read already",
    ),
    (
        &[
            "crates/console-bus/src/messages.rs::listed",
            "crates/console-bus/src/messages.rs::text",
        ],
        "each walks every variant of one value, and a walk over an enum with no wildcard arm is written out in full once per question asked of it",
    ),
    (
        &[
            "crates/console-music-player/src/tags.rs::an_mp3",
            "crates/console-music-player/src/tags.rs::an_opus",
        ],
        "two ffprobe answers written out as fixtures, one per container, and a fixture is the thing that is meant to be spelled out",
    ),
    (
        &[
            "crates/console-test-stages/src/checking.rs::desktop",
            "crates/console-test-stages/src/checking.rs::here",
        ],
        "the stage's type decides which Body variant is asked for, and a function over two types is two functions",
    ),
];

const NOT_YET: [&[&str]; 4] = [
    &[
        "crates/console-core-places/src/lib.rs::said",
        "crates/console-test-desktop/src/lib.rs::said",
    ],
    &[
        "crates/console-device/src/bin/console-deploy.rs::today",
        "crates/console-input-dictation/src/bin/voice-compare.rs::stamped",
    ],
    &[
        "crates/console-input-controller/src/reading.rs::told",
        "crates/console-panel/src/description.rs::where_to",
    ],
    &[
        "crates/console-notifications/src/serving.rs::kept",
        "crates/console-notifications/src/updating.rs::at",
    ],
];

const KEYWORDS: [&str; 30] = [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "move", "mut", "pub", "ref", "return",
    "self", "static", "struct", "super", "true", "while",
];

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    from.canonicalize().unwrap_or(from)
}

struct Written {
    named: String,
    shape: BTreeSet<u64>,
}

fn named_by_where(this: &TokenTree) -> bool {
    match this {
        TokenTree::Group(group) => group.delimiter() == Delimiter::Parenthesis,
        TokenTree::Punct(punct) => punct.as_char() == ':' || punct.as_char() == '!',
        TokenTree::Ident(_) | TokenTree::Literal(_) => false,
    }
}

fn flatten(stream: TokenStream, into: &mut Vec<String>) {
    let trees: Vec<TokenTree> = stream.into_iter().collect();

    for (at, tree) in trees.iter().enumerate() {
        let before = at.checked_sub(1).and_then(|before| trees.get(before));
        let after = trees.get(at.saturating_add(1));

        match tree {
            TokenTree::Group(group) => {
                into.push(format!("{:?}", group.delimiter()));
                flatten(group.stream(), into);
                into.push("end".to_string());
            }
            TokenTree::Punct(punct) => into.push(punct.as_char().to_string()),
            TokenTree::Literal(literal) => {
                let said = literal.to_string();

                match said.starts_with('"') || said.starts_with('r') {
                    true => into.push("\"\"".to_string()),
                    false => into.push(said),
                }
            }
            TokenTree::Ident(ident) => {
                let said = ident.to_string();

                let meant = KEYWORDS.contains(&said.as_str())
                    || said.starts_with(|first: char| first.is_uppercase())
                    || after.is_some_and(named_by_where)
                    || before.is_some_and(|before| match before {
                        TokenTree::Punct(punct) => punct.as_char() == '.' || punct.as_char() == ':',
                        TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => false,
                    });

                match meant {
                    true => into.push(said),
                    false => into.push("_".to_string()),
                }
            }
        }
    }
}

fn shape(body: &syn::Block) -> Option<BTreeSet<u64>> {
    let mut tokens = Vec::new();

    flatten(body.to_token_stream(), &mut tokens);

    let Ok(long) = fitted::<_, u32>(tokens.len());
    let Ok(run) = index(RUN);

    match long < SHORTEST {
        true => None,
        false => Some(
            tokens
                .windows(run)
                .map(|run| {
                    let mut hasher = DefaultHasher::new();
                    run.hash(&mut hasher);
                    hasher.finish()
                })
                .collect(),
        ),
    }
}

fn is_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| attribute.path().is_ident("test"))
}

struct Reading<'a> {
    file: &'a str,
    found: &'a mut Vec<Written>,
}

impl Reading<'_> {
    fn keep(&mut self, ident: &syn::Ident, attributes: &[syn::Attribute], body: &syn::Block) {
        let Ok(line) = fitted::<_, u32>(ident.span().start().line);

        match (is_test(attributes), shape(body)) {
            (false, Some(shape)) => {
                self.found.push(Written { named: format!("{}::{ident}:{line}", self.file), shape })
            }
            (true, _) | (false, None) => {}
        }
    }
}

impl<'ast> Visit<'ast> for Reading<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.keep(&item.sig.ident, &item.attrs, &item.block);
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        match item.trait_ {
            Some(_) => {}
            None => syn::visit::visit_item_impl(self, item),
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.keep(&item.sig.ident, &item.attrs, &item.block);
        syn::visit::visit_impl_item_fn(self, item);
    }
}

fn read(top: &Path, files: &[PathBuf]) -> Vec<Written> {
    let mut found = Vec::new();

    for at in files {
        let said = match std::fs::read_to_string(at) {
            Ok(said) => said,
            Err(_unreadable) => continue,
        };
        let parsed = match syn::parse_file(&said) {
            Ok(parsed) => parsed,
            Err(_not_rust_syn_reads) => continue,
        };
        let file = at.strip_prefix(top).unwrap_or(at).display().to_string();

        Reading { file: &file, found: &mut found }.visit_file(&parsed);
    }

    found
}

fn written() -> Vec<Written> {
    let top = root();
    let Ok(files) = console_repository::sources::under(&top.join("crates"));
    let Ok(hands) = fitted::<_, u32>(std::thread::available_parallelism().map_or(1, |hands| hands.get()));
    let Ok(many) = fitted::<_, u32>(files.len());
    let Ok(each) = index(many.div_ceil(hands.max(1)).max(1));

    std::thread::scope(|scope| {
        let reading: Vec<_> =
            files.chunks(each).map(|share| scope.spawn(|| read(&top, share))).collect();

        reading.into_iter().flat_map(|one| one.join().unwrap_or_default()).collect()
    })
}

fn pairs() -> &'static [(String, String, f64)] {
    static FOUND: OnceLock<Vec<(String, String, f64)>> = OnceLock::new();

    FOUND.get_or_init(compared)
}

fn compared() -> Vec<(String, String, f64)> {
    let every = written();
    let mut holding: BTreeMap<u64, Vec<u32>> = BTreeMap::new();

    for (one, at) in every.iter().zip(0u32..) {
        for run in &one.shape {
            holding.entry(*run).or_default().push(at);
        }
    }

    let mut met: HashMap<(u32, u32), u32> = HashMap::new();

    for holders in holding.values() {
        for (at, one) in holders.iter().enumerate() {
            for other in holders.iter().skip(at.saturating_add(1)) {
                let counted = met.entry((*one, *other)).or_default();
                *counted = counted.saturating_add(1);
            }
        }
    }

    let mut alike: Vec<(String, String, f64)> = met
        .into_iter()
        .filter(|(_, shared)| *shared >= MET)
        .filter_map(|((one, other), _)| {
            let (one, other) = every.get(index(one).ok()?).zip(every.get(index(other).ok()?))?;
            let Ok(shared) = fitted::<_, u32>(one.shape.intersection(&other.shape).count());
            let Ok(first) = fitted::<_, u32>(one.shape.len());
            let Ok(second) = fitted::<_, u32>(other.shape.len());
            let either = first.saturating_add(second).saturating_sub(shared).max(1);
            let alike = f64::from(shared) / f64::from(either);

            Some((one.named.clone(), other.named.clone(), alike))
        })
        .filter(|(_, _, alike)| *alike >= ALIKE_ENOUGH)
        .collect();

    alike.sort_by(|one, other| other.2.total_cmp(&one.2).then_with(|| one.0.cmp(&other.0)));

    alike
}

fn without_line(named: &str) -> &str {
    named.rsplit_once(':').map_or(named, |(named, _line)| named)
}

fn groups(pairs: &[(String, String, f64)]) -> Vec<BTreeSet<String>> {
    let mut held: Vec<BTreeSet<String>> = Vec::new();

    for (one, other, _) in pairs {
        let (touching, apart): (Vec<BTreeSet<String>>, Vec<BTreeSet<String>>) =
            held.into_iter().partition(|group| group.contains(one) || group.contains(other));

        let mut joined: BTreeSet<String> = touching.into_iter().flatten().collect();
        joined.insert(one.clone());
        joined.insert(other.clone());

        held = apart;
        held.push(joined);
    }

    held.sort();

    held
}

fn let_through() -> Result<impl Iterator<Item = &'static [&'static str]>, console_core_never::Never> {
    Ok(ALIKE.iter().map(|(group, _why)| *group).chain(NOT_YET.iter().copied()))
}

fn accepted(one: &str, other: &str) -> bool {
    let (one, other) = (without_line(one), without_line(other));

    let Ok(mut every) = let_through();

    every.any(|group| group.contains(&one) && group.contains(&other))
}

#[test]
fn no_function_is_written_twice() {
    let twice: Vec<(String, String, f64)> =
        pairs().iter().filter(|(one, other, _)| !accepted(one, other)).cloned().collect();

    let said: Vec<String> = groups(&twice)
        .into_iter()
        .map(|group| group.into_iter().collect::<Vec<String>>().join("\n    "))
        .collect();

    assert!(
        said.is_empty(),
        "a function written again is one of them guessing; call the other, or put the group on ALIKE with why:\n\n    {}",
        said.join("\n\n    ")
    );
}

#[test]
fn every_group_let_stay_is_still_alike() {
    let found = pairs();

    let Ok(every) = let_through();

    let stale: Vec<String> = every
        .flat_map(|group| {
            group.iter().filter(|member| {
                !found.iter().any(|(one, other, _)| {
                    let (one, other) = (without_line(one), without_line(other));

                    (one == **member && group.contains(&other)) || (other == **member && group.contains(&one))
                })
            })
        })
        .map(|member| member.to_string())
        .collect();

    assert!(
        stale.is_empty(),
        "a function no longer alike the rest of its group is a line to delete rather than a permission to keep: {stale:?}"
    );
}
