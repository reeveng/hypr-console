#![feature(rustc_private)]
#![warn(unused_extern_crates)]

//! EXPLICIT020: the code says what it does, and nothing beside it says it again.
//!
//! Every `///` and every `//` in this workspace was deleted once, on the rule
//! that a sentence beside a line of code is a second statement of the same
//! thing and only one of the two is compiled. What was worth keeping was moved
//! into `docs/`, where it is read by somebody deciding something rather than
//! by somebody scrolling past. Then they came back: a `///` over a new
//! function, three lines of argument above a `match`, a `// TODO` nobody
//! returned to. They came back because the rule was a paragraph in a document
//! and not a thing the build could fail on, and a rule of that kind lasts
//! exactly as long as the memory of whoever wrote it. This is that rule in the
//! build.
//!
//! Two comments stay, and both say something the code cannot.
//!
//! **`//!`** is a module head. What a file is *for* -- the decision it stands
//! on, what was tried before it and why that was wrong -- is not derivable
//! from any line inside it, and the head is where the argument for the whole
//! goes. A head is prose that has to earn its place like any other prose, and
//! nothing here judges it; that is a reading, not a lint.
//!
//! **`// SAFETY:`** is required by EXPLICIT012, which will not pass an
//! `unsafe` block that has no reason above it. A rule that forbade it would
//! put every `unsafe` in the tree between two rules that cannot both be kept.
//! The reason often runs past one line, so it is the *block* that is allowed
//! once its first line says `SAFETY:` -- the same way 012 reads it.
//!
//! ## Why it reads the file rather than the syntax tree
//!
//! A comment is not in the tree. `///` survives as an attribute and could be
//! caught there, but `//` is thrown away before any lint pass runs, so half
//! the rule would have to be written against the text anyway -- and a rule
//! enforced by two mechanisms is a rule with two sets of edge cases. So this
//! reads the source of every file the crate is built from, with the
//! compiler's own lexer, which knows that `//` inside a string is a string.
//!
//! ## Tests are not exempt, alone in this suite
//!
//! Every other rule here returns early on a test build, and the reason is
//! always that the harm it names is absent in a test: a panic in a test is the
//! test failing, an `as` in a fixture is arithmetic nobody ships. Nothing of
//! that kind is true here. A comment in a test is prose beside code, read by
//! the same person, going stale at the same rate -- and a test is the place a
//! reader goes to find out what a thing is supposed to do, so it is the last
//! place that should be explaining itself twice. Exempting tests would also
//! split the rule along a line nobody could see: `tests/the_tree.rs` is a
//! test build and a `#[cfg(test)] mod` inside a library is not, so the same
//! comment would be legal in one file and not in the other.

extern crate rustc_ast;
extern crate rustc_lexer;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::Crate;
use rustc_lexer::{DocStyle, FrontmatterAllowed, TokenKind};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::{BytePos, FileName, SourceFile, Span, SyntaxContext};

dylint_linting::declare_early_lint! {
    /// EXPLICIT020: no comments. A module head (`//!`) and a `// SAFETY:`
    /// reason are the two that stay; everything else the code says itself, or
    /// belongs in `docs/`.
    pub EXPLICIT020_NO_COMMENT,
    Deny,
    "a comment that is neither a module head nor a `// SAFETY:` reason"
}

/// A run of comment lines that are read as one thing.
///
/// Consecutive because that is how a reason is written: `// SAFETY:` on the
/// first line and the rest of the sentence under it, or four `///` lines that
/// are one paragraph. Reporting each line on its own would answer a paragraph
/// with a screen of identical errors, and would ask the first line of a
/// SAFETY reason a question the other three cannot answer.
struct Run {
    from: u32,
    to: u32,
    style: Option<DocStyle>,
    said: String,
}

/// Where a file's text is on disk, when it is somewhere this rule may speak
/// about.
///
/// The source map holds every file the compiler has opened, which is not the
/// same set as the files somebody in this repository wrote. A dependency
/// pulled out of the registry, a file generated under `target/` by a build
/// script, and anything the standard library brought in are all code this
/// workspace does not get to have an opinion about -- and a lint that reported
/// them would be unanswerable, because there is no line to delete that is ours
/// to delete.
///
/// `include_str!` is the other one. It registers whatever it read, whether or
/// not it is Rust, so a stylesheet full of `//` would arrive here as a file
/// full of comments. Only `.rs` is tokenised.
fn ours(file: &SourceFile) -> Option<&str> {
    let path = match &file.name {
        FileName::Real(real) => real.local_path()?,
        _ => return None,
    };

    match path.extension().and_then(|end| end.to_str()) {
        Some("rs") => {}
        _ => return None,
    }

    let spelled = path.to_string_lossy();

    match spelled.contains("/.cargo/") || spelled.contains("/target/") || spelled.contains("/rustlib/") {
        true => None,
        false => file.src.as_ref().map(|text| text.as_str()),
    }
}

/// Whitespace that ends a run rather than continuing it.
///
/// A blank line is where one thought stops and the next starts, which is how
/// EXPLICIT012 reads an `unsafe` block's reason too: it walks up until the
/// lines stop being comments. Without this a `// SAFETY:` two paragraphs above
/// would license everything under it.
fn breaks_the_run(gap: &str) -> bool {
    gap.matches('\n').count() > 1
}

/// Every run of comments in a file, in the order they are written.
fn runs(text: &str) -> Vec<Run> {
    let mut found: Vec<Run> = Vec::new();
    let mut open = false;
    let mut at: u32 = 0;
    let mut gap = "";

    for token in rustc_lexer::tokenize(text, FrontmatterAllowed::Yes) {
        let from = at;
        let to = at.saturating_add(token.len);
        at = to;
        let said = text.get(from as usize..to as usize).unwrap_or_default();

        let style = match token.kind {
            TokenKind::LineComment { doc_style } => doc_style,
            TokenKind::BlockComment { doc_style, .. } => doc_style,
            TokenKind::Whitespace => {
                gap = said;
                continue;
            }
            _ => {
                open = false;
                gap = "";
                continue;
            }
        };

        let carries_on = match found.last() {
            Some(last) => open && last.style == style && !breaks_the_run(gap),
            None => false,
        };

        match carries_on {
            true => {
                if let Some(last) = found.last_mut() {
                    last.to = to;
                }
            }
            false => found.push(Run { from, to, style, said: said.to_string() }),
        }

        open = true;
        gap = "";
    }

    found
}

/// Whether a run is one of the two that stay.
fn allowed(run: &Run) -> bool {
    match run.style {
        Some(DocStyle::Inner) => true,
        Some(DocStyle::Outer) => false,
        None => run.said.starts_with("//") && run.said.contains("SAFETY:"),
    }
}

impl EarlyLintPass for Explicit020NoComment {
    fn check_crate(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        let files: Vec<_> = cx.sess().source_map().files().iter().cloned().collect();

        for file in files {
            let Some(text) = ours(&file) else {
                continue;
            };

            for run in runs(text) {
                match allowed(&run) {
                    true => continue,
                    false => {}
                }

                let lo = file.start_pos + BytePos(run.from);
                let hi = file.start_pos + BytePos(run.to);
                let span = Span::new(lo, hi, SyntaxContext::root(), None);

                span_lint_and_help(
                    cx,
                    EXPLICIT020_NO_COMMENT,
                    span,
                    "a comment that is neither a module head nor a `// SAFETY:` reason",
                    None,
                    "delete it: what the code says, it says. What it cannot say goes in the module's `//!` head or in `docs/`",
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
