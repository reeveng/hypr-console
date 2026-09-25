#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use clippy_utils::is_lang_item_or_ctor;
use rustc_hir::def::Res;
use rustc_hir::{Arm, Expr, ExprKind, LangItem, Pat, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::Ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT001: a failure met is a failure said, whatever the signature
    /// promises.
    ///
    /// A lint cannot read "fallible" off a signature, so this reads it off the
    /// body instead: a function that swallows someone else's error -- with
    /// `unwrap_or`, `unwrap_or_else`, `unwrap_or_default`, `ok`, `is_ok`,
    /// `is_err` -- is a function that has met a failure and decided not to
    /// mention it, and the caller cannot know there was one.
    ///
    /// This used to let a function that returns `Result` swallow anyway, on the
    /// grounds that it had already said it could fail. What that permitted was
    /// the shape no one reads twice: a `strip_prefix` that quietly hands back
    /// the whole path, a clock reading that quietly becomes zero, a file that
    /// could not be read quietly becoming an empty one. Saying somewhere that
    /// you can fail is not the same as saying that you did. Name the other way
    /// instead -- a `match` with both arms, and the failing arm's binding
    /// saying what went wrong -- which is what `Err(_outside_the_tree)` does
    /// and what `unwrap_or_default` never can.
    ///
    /// The `match` is also where the same swallow hides best. `Err(_) => 0`
    /// is `unwrap_or(0)` with the words moved apart, and a rule that asked only
    /// about method names let it through. An `Err` arm whose pattern names
    /// nothing is asked what it hands back: a fault, `Err(...)` or
    /// `return Err(...)`, is a failure said, and anything else is a failure
    /// answered as a value with nothing said about it. `Err(_fault)` is asked
    /// the same question as `Err(_)`: a name that only says a failure happened
    /// is the `Err` said twice, and not what went wrong.
    pub EXPLICIT001_FALLIBLE_RESULT,
    Deny,
    "a function that swallows a failure must return `Result<T, E>`"
}

fn is_test_build(cx: &LateContext<'_>) -> bool {
    cx.sess().opts.test
}

// `Result`, whatever it is called at the point of use. Asking the type rather
// than the spelling is what makes an alias answer the same as `std`'s own.
fn is_result(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.peel_refs().kind() {
        rustc_middle::ty::Adt(adt, _) => {
            cx.tcx.is_diagnostic_item(rustc_span::sym::Result, adt.did())
        }
        _ => false,
    }
}

// The ways a `Result` is turned into a plain value without a word said. The
// panicking ones (`unwrap`, `expect`) belong to EXPLICIT004 and are left to it.
const SWALLOWS: &[&str] = &[
    "unwrap_or",
    "unwrap_or_else",
    "unwrap_or_default",
    "ok",
    "is_ok",
    "is_err",
];

fn is_err(cx: &LateContext<'_>, res: Res) -> bool {
    res.opt_def_id().is_some_and(|did| is_lang_item_or_ctor(cx, did, LangItem::ResultErr))
}

// `Err(_)`, `Err(..)` and `Err(_fault)`: the failing way, matched and not named. Or-patterns
// and nesting are walked, so `Ok(None) | Err(_)` is asked the same question.
fn unnamed_err(cx: &LateContext<'_>, pat: &Pat<'_>) -> bool {
    let mut found = false;
    pat.walk(|inner| {
        if let PatKind::TupleStruct(qpath, fields, _) = inner.kind
            && is_err(cx, cx.qpath_res(&qpath, inner.hir_id))
            && fields.iter().all(|field| says_nothing(field))
            && !cannot_fail(cx, inner)
        {
            found = true;
        }
        !found
    });
    found
}

// Names that are only the word for a failure: they say that something went
// wrong, which the `Err` already said, and not what.
const EMPTY: &[&str] = &["e", "err", "error", "fault", "failed", "failure", "why", "ignored", "whatever"];

// `_`, or a discarded binding whose name is one of the empty ones. A binding
// that is used says whatever its use says, and is not asked here.
fn says_nothing(field: &Pat<'_>) -> bool {
    match field.kind {
        PatKind::Wild => true,
        PatKind::Binding(_, _, ident, None) => ident
            .name
            .as_str()
            .strip_prefix('_')
            .is_some_and(|rest| EMPTY.contains(&rest)),
        _ => false,
    }
}

// `Err(_)` over a `Result<T, Never>` is there for the match to be whole, and
// there is no failure behind it to swallow.
fn cannot_fail(cx: &LateContext<'_>, err: &Pat<'_>) -> bool {
    match cx.typeck_results().pat_ty(err).kind() {
        rustc_middle::ty::Adt(_, arguments) => arguments.types().nth(1).is_some_and(|fault| match fault.kind() {
            rustc_middle::ty::Adt(fault, _) => fault.is_enum() && fault.variants().is_empty(),
            rustc_middle::ty::Never => true,
            _ => false,
        }),
        _ => false,
    }
}

// Whether an arm's body is a fault handed on rather than a value made up.
fn says_a_fault(cx: &LateContext<'_>, body: &Expr<'_>) -> bool {
    match body.kind {
        ExprKind::Call(func, _) => match func.kind {
            ExprKind::Path(ref qpath) => is_err(cx, cx.qpath_res(qpath, func.hir_id)),
            _ => false,
        },
        ExprKind::Ret(Some(returned)) => says_a_fault(cx, returned),
        ExprKind::Block(block, _) => block.expr.is_some_and(|last| says_a_fault(cx, last)),
        _ => false,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit001FallibleResult {
    fn check_arm(&mut self, cx: &LateContext<'tcx>, arm: &'tcx Arm<'tcx>) {
        if is_test_build(cx) || arm.pat.span.from_expansion() {
            return;
        }
        if !unnamed_err(cx, arm.pat) || says_a_fault(cx, arm.body) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT001_FALLIBLE_RESULT,
            arm.pat.span,
            "an `Err` arm that does not say what went wrong turns a failure into a value with nothing said",
            None,
            "name what went wrong in the binding, match the fault's own variants, or hand it on as `Err`",
        );
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if is_test_build(cx) {
            return;
        }
        let ExprKind::MethodCall(path, receiver, _, _) = expr.kind else {
            return;
        };
        if !SWALLOWS.contains(&path.ident.name.as_str()) {
            return;
        }
        let recv_ty = cx.typeck_results().expr_ty_adjusted(receiver);
        if !is_result(cx, recv_ty) {
            return;
        }

        span_lint_and_help(
            cx,
            EXPLICIT001_FALLIBLE_RESULT,
            expr.span,
            format!("`{}` turns a failure into a value with nothing said", path.ident.name),
            None,
            "propagate it with `?`, or `match` both ways and name the failing one",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
