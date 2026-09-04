#![feature(rustc_private)]
#![warn(unused_extern_crates)]

//! EXPLICIT013: a block that decides something gets a line to itself.
//!
//! The other twelve rules are about what a signature says. This one is about
//! what a screen says before anything is read: an `if`, a `match`, a `while`,
//! a `for`, a `loop` and a function are the places control goes somewhere
//! else, and each of them is set off by a blank line above and below so that
//! the shape of a function can be seen without reading it. Elixir gets this
//! for free from `do`/`end` and the way its formatter spaces them; Rust's
//! braces are quieter, so the space has to be asked for.
//!
//! The rule is only ever about statements. An `if` used as a value --
//! `let width = if wide { 3 } else { 1 };` -- is an expression in the middle
//! of a line and is left alone, because a blank line inside an assignment
//! would say something that is not true. The same goes for the `match` most
//! of this workspace writes, which is the right-hand side of a `let` or the
//! last expression of a function rather than a statement of its own.
//!
//! ## Where the line goes
//!
//! Above the comment, not below it. A comment belongs to the block it
//! explains, so the two are one unit and the space goes above the pair:
//!
//! ```ignore
//!     let title = first(&["title"]);
//!
//!     // A track with no title is filed under its filename.
//!     if title.is_empty() {
//!         return filename(path);
//!     }
//! ```
//!
//! Attributes are read the same way, so a `#[cfg(...)]` above a function
//! counts as part of it rather than as the line before it.
//!
//! ## What it asks
//!
//! Statements: `if`, `match`, `while`, `for`, `loop`, `let … else`, a bare
//! block, and an `unsafe` block. Declarations: `fn`, `struct`, `enum`,
//! `union`, `trait`, `impl`, and `mod name { … }`.
//!
//! `use`, `extern crate`, `const`, `static`, `type` and `mod name;` are left
//! out. Those are written as packed lists on purpose -- a wall of `use` lines,
//! a table of constants above the code that spends them -- and a gap between
//! every one of them would break apart a block that is read as a unit. What is
//! asked for room is what has a body a reader has to step into.
//!
//! ## What already reads as a gap
//!
//! A block does not need a blank line above it when there is nothing above it
//! to separate from: the opening brace of the block it lives in, the start of
//! the file, or an `} else {` it is chained to. It does not need one below
//! when the next line closes the block around it, continues an `else` chain,
//! or ends the file. Those are the cases where a blank line would open a hole
//! rather than close one.

extern crate rustc_ast;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::{
    Block, BlockCheckMode, Expr, ExprKind, Inline, Item, ItemKind, LocalKind, ModKind, Stmt,
    StmtKind, UnsafeSource,
};
use rustc_ast::visit::FnKind;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;
use rustc_span::source_map::SourceMap;

dylint_linting::declare_early_lint! {
    /// EXPLICIT013: every block that decides something -- `if`, `match`,
    /// `while`, `for`, `loop`, `let … else`, `unsafe` -- and every declaration
    /// with a body under it is set off by a blank line above and below.
    pub EXPLICIT013_BREATHING_ROOM,
    Warn,
    "a block that decides something must have a blank line above and below it"
}

// Tests are exempt, as they are in the rest of the suite. A fixture is written
// to be read against the thing it checks, and a table of short cases packed
// together is easier to check than the same table spread over three screens.
// `opts.test` is true only for the harness build of a target -- the ordinary
// build of the same library is linted as production, so nothing real is lost.
fn is_test_build(cx: &EarlyContext<'_>) -> bool {
    cx.sess().opts.test
}

/// One line of the file the span came from, by its one-based number.
///
/// `get_line` counts from zero and everything a diagnostic says counts from
/// one, so the conversion lives here rather than at each of the four call
/// sites, where it was an off-by-one waiting to be written.
fn line_at(sm: &SourceMap, span: Span, which: usize) -> Option<String> {
    let loc = sm.lookup_char_pos(span.lo());
    let text = loc.file.get_line(which.checked_sub(1)?)?;
    Some(text.to_string())
}

/// Whether a line is part of the unit below it rather than the line before it.
///
/// Comments and attributes are carried by the block they sit on: the blank
/// line belongs above the whole group, so the walk upward passes through them
/// looking for the first line that is really the one before.
fn belongs_to_what_follows(line: &str) -> bool {
    let line = line.trim();
    line.starts_with("//")
        || line.starts_with("#[")
        || line.starts_with("#!")
        || line.starts_with("/*")
        || line.starts_with('*')
        || line.ends_with("*/")
}

/// Whether the line above already reads as a gap.
///
/// Not only a blank line. An opening brace means the block is the first thing
/// inside something else and there is nothing above it to be separated from,
/// and `} else {` means it is a continuation of a chain that is already one
/// thought.
fn reads_as_gap_above(line: &str) -> bool {
    let line = line.trim();
    line.is_empty()
        || line.ends_with('{')
        || line.ends_with('(')
        || line.ends_with('|')
        || line.starts_with("} else")
}

/// Whether the line below already reads as a gap.
///
/// A closing brace ends the block this one lives in, an `else` continues the
/// chain, and a closing delimiter means the block was the last argument to
/// something. None of those want a blank line pushed in front of them.
fn reads_as_gap_below(line: &str) -> bool {
    let line = line.trim();
    line.is_empty()
        || line.starts_with('}')
        || line.starts_with("else")
        || line.starts_with(')')
        || line.starts_with(']')
        || line.starts_with(',')
        || line.starts_with(';')
        || line.starts_with('.')
}

/// The two halves of the rule, applied to one span.
///
/// `what` names the construct so the message says `if` or `for` rather than
/// "block", which is the difference between a warning somebody acts on and one
/// they read twice.
fn check_room(cx: &EarlyContext<'_>, span: Span, what: &str) {
    if span.from_expansion() {
        return;
    }

    let sm = cx.sess().source_map();
    let opens = sm.lookup_char_pos(span.lo()).line;
    let closes = sm.lookup_char_pos(span.hi()).line;

    // Upward past whatever belongs to this block, to the first line that is
    // genuinely the one before it.
    let mut above = opens;
    while above > 1 {
        let Some(line) = line_at(sm, span, above - 1) else {
            break;
        };

        if !belongs_to_what_follows(&line) {
            break;
        }

        above -= 1;
    }

    if above > 1
        && let Some(line) = line_at(sm, span, above - 1)
        && !reads_as_gap_above(&line)
    {
        let said = format!("`{what}` must have a blank line above it");

        span_lint_and_help(
            cx,
            EXPLICIT013_BREATHING_ROOM,
            span.shrink_to_lo(),
            said,
            None,
            "leave a blank line before the block, so it reads as its own thought",
        );
    }

    if let Some(line) = line_at(sm, span, closes + 1)
        && !reads_as_gap_below(&line)
    {
        let said = format!("`{what}` must have a blank line below it");

        span_lint_and_help(
            cx,
            EXPLICIT013_BREATHING_ROOM,
            span.shrink_to_hi(),
            said,
            None,
            "leave a blank line after the block, so what follows is its own thought",
        );
    }
}

/// What a statement's expression is, when it is one of the five.
///
/// `None` for everything else, which is most of a program. An `if` or a
/// `match` reached through a `let` never arrives here: `StmtKind::Let` is a
/// different arm, and that is exactly the expression-versus-statement line the
/// rule draws.
fn control_flow(expr: &Expr) -> Option<&'static str> {
    match expr.kind {
        ExprKind::If(..) => Some("if"),
        ExprKind::Match(..) => Some("match"),
        ExprKind::While(..) => Some("while"),
        ExprKind::ForLoop { .. } => Some("for"),
        ExprKind::Loop(..) => Some("loop"),
        _ => None,
    }
}

/// The expression a statement is, when the statement is only an expression.
fn stated(stmt: &Stmt) -> Option<&Expr> {
    match &stmt.kind {
        StmtKind::Expr(expr) | StmtKind::Semi(expr) => Some(expr),
        _ => None,
    }
}

/// What a statement is, for the purpose of the rule, or `None` if it is
/// ordinary code.
///
/// Three things arrive here that `control_flow` does not name. A `let … else`
/// is a branch wearing a `let`, and this workspace writes a great many of
/// them. A bare block is a scope somebody opened on purpose. An `unsafe` block
/// is the loudest of the three, and EXPLICIT012 already asks it for a
/// sentence -- this asks it for the room to be seen.
fn decides(stmt: &Stmt) -> Option<&'static str> {
    if let StmtKind::Let(local) = &stmt.kind {
        return match local.kind {
            LocalKind::InitElse(..) => Some("let … else"),
            _ => None,
        };
    }

    let expr = stated(stmt)?;

    if let ExprKind::Block(block, _) = &expr.kind {
        return match block.rules {
            BlockCheckMode::Unsafe(UnsafeSource::UserProvided) => Some("unsafe"),
            BlockCheckMode::Default => Some("block"),
            BlockCheckMode::Unsafe(UnsafeSource::CompilerGenerated) => None,
        };
    }

    control_flow(expr)
}

/// What a declaration is, or `None` if it is one of the ones written in
/// groups.
///
/// `use`, `extern crate`, `const`, `static` and `type` are left out on
/// purpose. Those are written as packed lists -- a wall of `use` lines at the
/// top of a file, a table of constants above the code that spends them -- and
/// a blank line between every one of them would take a block somebody reads as
/// a unit and shake it apart. What is asked for room is what has a body:
/// something with a `{ … }` under it that a reader has to step into.
///
/// `mod foo;` is in the same position as `use` and is skipped for the same
/// reason; `mod foo { … }` is not, and is asked.
fn declares(item: &Item) -> Option<&'static str> {
    match &item.kind {
        // A function arrives through `check_fn`, which sees the ones written
        // in an `impl` and in a trait as well as the free ones. Naming it here
        // too would report every free function twice.
        ItemKind::Fn(..) => None,
        ItemKind::Struct(..) => Some("struct"),
        ItemKind::Enum(..) => Some("enum"),
        ItemKind::Union(..) => Some("union"),
        ItemKind::Trait(..) => Some("trait"),
        ItemKind::Impl(..) => Some("impl"),
        // `Loaded` is not the same question as "written with a body". A
        // `mod air;` at the top of a lib.rs is `Loaded` too, once the compiler
        // has read air.rs -- so asking only `Loaded` puts a blank line between
        // every line of the module list every crate opens with, which is the
        // packed list this rule exists to leave alone. `Inline::Yes` is the
        // one that means somebody wrote `mod name { … }` here.
        ItemKind::Mod(_, _, ModKind::Loaded(_, Inline::Yes, _)) => Some("mod"),
        _ => None,
    }
}

impl EarlyLintPass for Explicit013BreathingRoom {
    fn check_block(&mut self, cx: &EarlyContext<'_>, block: &Block) {
        if is_test_build(cx) {
            return;
        }

        for stmt in &block.stmts {
            let Some(what) = decides(stmt) else {
                continue;
            };

            check_room(cx, stmt.span, what);
        }
    }

    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if is_test_build(cx) {
            return;
        }

        let Some(what) = declares(item) else {
            return;
        };

        check_room(cx, item.span, what);
    }

    fn check_fn(&mut self, cx: &EarlyContext<'_>, kind: FnKind<'_>, span: Span, _: rustc_ast::NodeId) {
        if is_test_build(cx) {
            return;
        }

        // A closure is an expression in the middle of a line, the same as an
        // `if` that is being assigned. Only a declaration is a declaration.
        if !matches!(kind, FnKind::Fn(..)) {
            return;
        }

        check_room(cx, span, "fn");
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
