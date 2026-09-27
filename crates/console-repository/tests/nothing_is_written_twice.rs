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
use std::sync::Mutex;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use syn::visit::Visit;

use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const SHORTEST: u32 = 60;

const RUN: u32 = 6;

const MET: u32 = 8;

const ALIKE_ENOUGH: f64 = 0.7;

const ALIKE: [(&[&str], &str); 8] = [
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
            "crates/console-program-contract/src/effect.rs::as_spawn",
            "crates/console-program-contract/src/effect.rs::as_file_write",
        ],
        "one accessor per variant of Effect, and the no-wildcard rule writes the other variants out in each",
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
    (
        &[
            "crates/console-core-color/src/palette.rs::out_of",
            "crates/console-status-bar/src/showing.rs::out_of",
        ],
        "each surface's Wearing names the colours it spends and no others, and the reading they share is find_color already",
    ),
];

const NOT_YET: [&[&str]; 4] = [
    &[
        "crates/console-core-places/src/lib.rs::env_var",
        "crates/console-test-desktop/src/lib.rs::env_var",
    ],
    &[
        "crates/console-device/src/bin/console-deploy.rs::today",
        "crates/console-input-dictation/src/bin/voice-compare.rs::stamped",
    ],
    &[
        "crates/console-input-controller/src/reading.rs::path_from_environment",
        "crates/console-panel/src/description.rs::where_to",
    ],
    &[
        "crates/console-notifications/src/serving.rs::inbox_path",
        "crates/console-notifications/src/updating.rs::at",
    ],
];

const KEYWORDS: [&str; 30] = [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "move", "mut", "pub", "ref", "return",
    "self", "static", "struct", "super", "true", "while",
];

type Pair = (String, String, f64);

fn root() -> Result<PathBuf, Never> {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    match from.canonicalize() {
        Ok(found) => Ok(found),
        Err(_not_there) => Ok(from),
    }
}

struct Written {
    named: String,
    shape: BTreeSet<u64>,
}

#[derive(Clone, Copy)]
enum Closing {
    End,
    Top,
}

struct Level {
    trees: Vec<TokenTree>,
    at: u32,
    closing: Closing,
}

#[derive(Clone, Copy)]
enum Meaning {
    Named,
    AnyName,
}

fn meaning(said: &str, (before, after): (Option<&TokenTree>, Option<&TokenTree>)) -> Result<Meaning, Never> {
    let called = after.is_some_and(|after| match after {
        TokenTree::Group(group) => group.delimiter() == Delimiter::Parenthesis,
        TokenTree::Punct(punct) => punct.as_char() == ':' || punct.as_char() == '!',
        TokenTree::Ident(_) | TokenTree::Literal(_) => false,
    });

    let reached = before.is_some_and(|before| match before {
        TokenTree::Punct(punct) => punct.as_char() == '.' || punct.as_char() == ':',
        TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => false,
    });

    let meant = KEYWORDS.contains(&said) || said.starts_with(|first: char| first.is_uppercase()) || called || reached;

    Ok(match meant {
        true => Meaning::Named,
        false => Meaning::AnyName,
    })
}

fn flatten(stream: TokenStream) -> Result<Vec<String>, Never> {
    let walking = vec![Level { trees: stream.into_iter().collect(), at: 0, closing: Closing::Top }];
    let flattened = console_core_iteration::iterate((Vec::new(), walking), |(mut into, mut walking)| {
        let top = match walking.last_mut() {
            Some(top) => top,
            None => return Ok(Step::Halt(into)),
        };

        let Ok(at) = index(top.at);
        let tree = top.trees.get(at).cloned();
        let before = at.checked_sub(1).and_then(|before| top.trees.get(before)).cloned();
        let after = top.trees.get(at.saturating_add(1)).cloned();

        top.at = top.at.saturating_add(1);

        match tree {
            None => {
                let closing = top.closing;

                walking.pop();

                match closing {
                    Closing::End => into.push("end".to_string()),
                    Closing::Top => {}
                }
            }
            Some(TokenTree::Group(group)) => {
                into.push(format!("{:?}", group.delimiter()));
                walking.push(Level { trees: group.stream().into_iter().collect(), at: 0, closing: Closing::End });
            }
            Some(TokenTree::Punct(punct)) => into.push(punct.as_char().to_string()),
            Some(TokenTree::Literal(literal)) => {
                let said = literal.to_string();

                match said.starts_with('"') || said.starts_with('r') {
                    true => into.push("\"\"".to_string()),
                    false => into.push(said),
                }
            }
            Some(TokenTree::Ident(ident)) => {
                let said = ident.to_string();
                let Ok(meant) = meaning(&said, (before.as_ref(), after.as_ref()));

                match meant {
                    Meaning::Named => into.push(said),
                    Meaning::AnyName => into.push("_".to_string()),
                }
            }
        }

        Ok(Step::Again((into, walking)))
    });

    Ok(match flattened {
        Ok(into) => into,
        Err(_endless) => Vec::new(),
    })
}

fn shape(body: &syn::Block) -> Result<Option<BTreeSet<u64>>, Never> {
    let Ok(tokens) = flatten(body.to_token_stream());
    let Ok(long) = fitted::<_, u32>(tokens.len());
    let Ok(run) = index(RUN);

    Ok(match long < SHORTEST {
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
    })
}

#[derive(Clone, Copy)]
enum Caller {
    ByATest,
    ByTheTree,
}

fn asked_by(attributes: &[syn::Attribute]) -> Result<Caller, Never> {
    Ok(match attributes.iter().any(|attribute| attribute.path().is_ident("test")) {
        true => Caller::ByATest,
        false => Caller::ByTheTree,
    })
}

struct Reading<'a> {
    file: &'a str,
    found: &'a mut Vec<Written>,
}

impl Reading<'_> {
    fn keep(&mut self, (ident, attributes, body): (&syn::Ident, &[syn::Attribute], &syn::Block)) -> Result<(), Never> {
        let Ok(line) = fitted::<_, u32>(ident.span().start().line);
        let Ok(asked) = asked_by(attributes);
        let Ok(shape) = shape(body);

        match (asked, shape) {
            (Caller::ByTheTree, Some(shape)) => {
                self.found.push(Written { named: format!("{}::{ident}:{line}", self.file), shape })
            }
            (Caller::ByATest, _) | (Caller::ByTheTree, None) => {}
        }

        Ok(())
    }
}

impl<'ast> Visit<'ast> for Reading<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let Ok(()) = self.keep((&item.sig.ident, &item.attrs, &item.block));

        syn::visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        match item.trait_ {
            Some(_) => {}
            None => syn::visit::visit_item_impl(self, item),
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let Ok(()) = self.keep((&item.sig.ident, &item.attrs, &item.block));

        syn::visit::visit_impl_item_fn(self, item);
    }
}

fn read(top: &Path, files: &[PathBuf]) -> Result<Vec<Written>, Never> {
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
        let file = match at.strip_prefix(top) {
            Ok(inside) => inside.display().to_string(),
            Err(_outside_the_tree) => at.display().to_string(),
        };

        Reading { file: &file, found: &mut found }.visit_file(&parsed);
    }

    Ok(found)
}

fn scan_sources() -> Result<Vec<Written>, Never> {
    let Ok(top) = root();
    let Ok(files) = console_repository::sources::under(&top.join("crates"));
    let Ok(hands) = fitted::<_, u32>(std::thread::available_parallelism().map_or(1, |hands| hands.get()));
    let Ok(many) = fitted::<_, u32>(files.len());
    let Ok(each) = index(many.div_ceil(hands.max(1)).max(1));
    let found: Mutex<Vec<Written>> = Mutex::new(Vec::new());
    let (top, held) = (&top, &found);

    std::thread::scope(|scope| {
        for share in files.chunks(each) {
            scope.spawn(move || {
                let Ok(read) = read(top, share);

                match held.lock() {
                    Ok(mut held) => held.extend(read),
                    Err(poisoned) => poisoned.into_inner().extend(read),
                }
            });
        }
    });

    Ok(match found.into_inner() {
        Ok(found) => found,
        Err(poisoned) => poisoned.into_inner(),
    })
}

fn similar_pairs() -> Result<Vec<Pair>, Never> {
    let Ok(every) = scan_sources();
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

    let mut alike: Vec<Pair> = met
        .into_iter()
        .filter(|(_, shared)| *shared >= MET)
        .filter_map(|((one, other), _)| {
            let Ok(one) = index(one);
            let Ok(other) = index(other);
            let (one, other) = every.get(one).zip(every.get(other))?;
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

    Ok(alike)
}

fn without_line(named: &str) -> Result<&str, Never> {
    Ok(named.rsplit_once(':').map_or(named, |(named, _line)| named))
}

fn groups(pairs: &[Pair]) -> Result<Vec<BTreeSet<String>>, Never> {
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

    Ok(held)
}

fn let_through() -> Result<impl Iterator<Item = &'static [&'static str]>, Never> {
    Ok(ALIKE.iter().map(|(group, _why)| *group).chain(NOT_YET.iter().copied()))
}

fn allowed() -> Result<BTreeSet<(&'static str, &'static str)>, Never> {
    let Ok(every) = let_through();

    Ok(every
        .flat_map(|group| group.iter().flat_map(move |one| group.iter().map(move |other| (*one, *other))))
        .collect())
}

#[test]
fn no_function_is_written_twice() {
    let Ok(pairs) = similar_pairs();
    let Ok(allowed) = allowed();

    let twice: Vec<Pair> = pairs
        .into_iter()
        .filter(|(one, other, _)| {
            let (Ok(one), Ok(other)) = (without_line(one), without_line(other));

            !allowed.contains(&(one, other))
        })
        .collect();

    let Ok(grouped) = groups(&twice);

    let said: Vec<String> = grouped
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
    let Ok(pairs) = similar_pairs();
    let Ok(allowed) = allowed();
    let Ok(every) = let_through();

    let met: BTreeSet<&str> = pairs
        .iter()
        .filter_map(|(one, other, _)| {
            let (Ok(one), Ok(other)) = (without_line(one), without_line(other));

            match allowed.contains(&(one, other)) {
                true => Some([one, other]),
                false => None,
            }
        })
        .flatten()
        .collect();

    let stale: Vec<String> = every
        .flatten()
        .filter(|member| !met.contains(**member))
        .map(|member| member.to_string())
        .collect();

    assert!(
        stale.is_empty(),
        "a function no longer alike the rest of its group is a line to delete rather than a permission to keep: {stale:?}"
    );
}
