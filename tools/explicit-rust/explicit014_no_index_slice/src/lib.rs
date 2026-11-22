#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

#[path = "../../shared/authored_function.rs"]
mod authored_function;
mod binary_search;
mod chunk_loops;
mod counted;
mod counted_position;
mod fixes;
mod guarded;
mod length_match;
mod prefix;
mod recovery;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::higher::ForLoop;
use clippy_utils::is_in_const_context;
use rustc_hir::{Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use std::collections::{HashMap, HashSet};

dylint_linting::impl_late_lint! {
    /// EXPLICIT014: indexing and slicing are forbidden. `xs[i]` and `&s[a..b]`
    /// are panics no one declared -- the harm EXPLICIT004 names, arriving
    /// through syntax instead of a call. `get` and `get_mut` turn the absence
    /// into a value, and EXPLICIT005 then makes sure the value is met.
    ///
    /// Const contexts are left alone, for the reason EXPLICIT015 leaves them
    /// alone: an index the compiler evaluates and finds out of range fails the
    /// build. That is a failure with a name, at the right time, on the machine
    /// that has a screen -- which is the whole of what this rule is asking
    /// for, already granted.
    pub EXPLICIT014_NO_INDEX_SLICE,
    Deny,
    "indexing and slicing are forbidden; ask with `get` and meet the `None`",
    Explicit014NoIndexSlice {
        loops: HashMap::new(),
        chunk_accesses: HashSet::new(),
        length_matches: HashMap::new(),
        bounds_errors: dylint_linting::config_or_default("explicit014_bounds_errors"),
        fix_functions: dylint_linting::config_or_default("explicit014_fix_functions"),
        none_on_bounds: dylint_linting::config_or_default("explicit014_none_on_bounds"),
        fixed_places: dylint_linting::config_or_default("explicit014_fixed_places"),
        round_trip_loops: dylint_linting::config_or_default("explicit014_round_trip_loops"),
    }
}

pub struct Explicit014NoIndexSlice {
    loops: HashMap<HirId, Vec<(Span, String)>>,
    chunk_accesses: HashSet<HirId>,
    length_matches: HashMap<HirId, Vec<(Span, String)>>,
    bounds_errors: HashMap<String, String>,
    fix_functions: Vec<String>,
    none_on_bounds: Vec<String>,
    fixed_places: bool,
    round_trip_loops: bool,
}

const INDEX_HELP: &str = "ask with `get` / `get_mut` and meet the `None`, so the absent element is a case the code names";

impl<'tcx> LateLintPass<'tcx> for Explicit014NoIndexSlice {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let fix_here = || {
            self.fix_functions.is_empty()
                || self.fix_functions.contains(&recovery::body_name(cx, expr))
        };

        if let Some(loop_) = ForLoop::hir(expr) {
            if fix_here() && !is_in_const_context(cx) {
                if let Some((first_access, edits)) = counted::edits(cx, &loop_, self.round_trip_loops) {
                    self.loops.insert(first_access, edits);
                } else if let Some((accesses, edits)) = chunk_loops::edits(cx, &loop_)
                    && let Some((first, rest)) = accesses.split_first()
                {
                    self.loops.insert(*first, edits);
                    self.chunk_accesses.extend(rest.iter().copied());
                }
            }
            return;
        }

        if matches!(expr.kind, ExprKind::Match(_, _, rustc_hir::MatchSource::Normal))
            && fix_here() && !is_in_const_context(cx)
            && let Some((first, edits)) = length_match::edits(cx, expr)
        {
            self.length_matches.insert(first, edits);
        }

        if !matches!(expr.kind, ExprKind::Index(..)) {
            return;
        }

        // An index the compiler works out is an index the compiler checks. Out
        // of range there is a build that stops, not a handheld that does.
        if is_in_const_context(cx) {
            return;
        }

        // Code a macro wrote is the macro author's to answer for; what is
        // linted here is what someone typed into this file.
        if expr.span.from_expansion() {
            return;
        }

        let fix_here = fix_here();
        span_lint_and_then(
            cx,
            EXPLICIT014_NO_INDEX_SLICE,
            expr.span,
            "indexing is an implicit panic: the missing case has no name here",
            |diagnostic| {
                if !fix_here {
                    diagnostic.help(INDEX_HELP);
                } else if let Some(edits) = self.length_matches.remove(&expr.hir_id) {
                    diagnostic.multipart_suggestion("bind the elements in the slice patterns that establish their bounds",
                        edits, rustc_errors::Applicability::MachineApplicable);
                } else if let Some(edits) = self.loops.remove(&expr.hir_id) {
                    diagnostic.multipart_suggestion(
                        "iterate over the elements selected by this loop",
                        edits,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if self.chunk_accesses.remove(&expr.hir_id) {
                    diagnostic.help("destructure this fixed-width iterator together with the other elements used by the loop");
                } else if let Some(edits) = fixes::temporary_array_slice(cx, expr) {
                    diagnostic.multipart_suggestion("borrow the temporary array as a slice at the binding",
                        edits, rustc_errors::Applicability::MachineApplicable);
                } else if let Some(edits) = fixes::edits(cx, expr) {
                    diagnostic.multipart_suggestion(
                        "access the array with an irrefutable pattern",
                        edits,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if let Some(replacement) = prefix::replacement(cx, expr) {
                    diagnostic.span_suggestion(
                        expr.span,
                        "read the element from the fixed prefix that was already checked",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if let Some(edits) = binary_search::edits(cx, expr) {
                    diagnostic.multipart_suggestion(
                        "get the element from the same collection that the search checked",
                        edits,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if let Some((condition, replacement)) = guarded::replacement(cx, expr) {
                    diagnostic.span_suggestion(
                        condition,
                        "use one checked lookup for the bounds guard and the comparison",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if let Some((borrow, replacement)) = recovery::whole_slice(cx, expr) {
                    diagnostic.span_suggestion(
                        borrow,
                        "borrow the whole slice directly",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if let Some(replacement) =
                    recovery::replacement(cx, expr, &self.bounds_errors, &self.none_on_bounds)
                {
                    diagnostic.span_suggestion(
                        expr.span,
                        "return the configured failure if the element or range is absent",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else if self.fixed_places
                    && let Some(replacement) = recovery::fixed_projection(cx, expr)
                {
                    diagnostic.span_suggestion(
                        expr.span,
                        "project the fixed array element with an irrefutable borrow pattern",
                        replacement,
                        rustc_errors::Applicability::MachineApplicable,
                    );
                } else {
                    diagnostic.help(INDEX_HELP);
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn checked_recovery() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_recovery")
        .dylint_toml(
            r#"explicit014_fixed_places = true
[explicit014_bounds_errors]
"recovery::Fault" = "Bounds"
"recovery::Message" = "Invalid"
"recovery::legacy" = "()"
"temporary_arrays::ReadError" = "OutOfBounds"
"temporary_ranges::ReadError" = "OutOfBounds"
"wrapped_slices::Fault" = "Bounds"
"custom_index::Fault" = "Bounds"
"#,
        )
        .run();
}

#[test]
fn scoped_recovery() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_scoped_recovery")
        .dylint_toml(
            r#"explicit014_fix_functions = ["scoped::selected", "scoped::character", "scoped::update", "scoped::option_without_policy"]
explicit014_none_on_bounds = ["scoped::character", "scoped::update"]
[explicit014_bounds_errors]
"scoped::Fault" = "Bounds"
"#,
        )
        .run();
}

#[test]
fn round_trip_loops() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_round_trip")
        .dylint_toml("explicit014_round_trip_loops = true")
        .run();
}
