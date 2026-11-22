//! EXPLICIT086: a name joined to a path is a name somebody checked.
//!
//! `base.join(named)` reads as "the thing called `named`, under `base`", and
//! it is that for exactly the names nobody would think to test with. A name
//! that begins with `/` replaces the base altogether, which the
//! documentation says and the call does not; a name with `..` in it walks
//! back out; an empty one is the base itself. So a join with a name that
//! arrived -- an entry in an archive, a key in a file, a field on a bus, the
//! title of a book -- puts the result wherever the name's author wanted it,
//! and the code around the join goes on believing it is under `base`. An
//! archive unpacked that way writes over whatever its maker chose, which is
//! old enough to have a name of its own.
//!
//! Stock clippy has `path_buf_push_overwrite` and it reads literals, which
//! are the one kind of name that cannot surprise anybody. This rule asks the
//! other way round: a literal that stays under the base is fine and so is a
//! `const`, and what is joined otherwise is a type of this tree's own rather
//! than a `str`, a `String` or a path. That is EXPLICIT024's argument and
//! EXPLICIT065's -- a name that may not climb is a different quantity from a
//! name, it is built by one constructor that refuses a `/`, a `..` and
//! nothing at all, and the join that takes it cannot be handed the other
//! kind. Where the name was made here and never arrived from anywhere, the
//! site says so.
//!
//! It arrives `Warn`, and it will stand there a while: nearly every join in
//! the tree is one of these, and most of them are a name this desktop made.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;

dylint_linting::declare_late_lint! {
    /// EXPLICIT086: `Path::join` and `PathBuf::push` take a literal that
    /// stays under the base, a `const`, or a type of this tree's own whose
    /// constructor refused a name that climbs. Not a `str`, a `String` or
    /// another path.
    pub EXPLICIT086_NO_CLIMBING_JOIN,
    Warn,
    "a name joined to a path unchecked goes wherever whoever wrote the name wanted"
}

fn joins(path: &str) -> bool {
    matches!(path, "std::path::Path::join" | "std::path::PathBuf::push")
}

fn stays_under(named: &str) -> bool {
    !named.is_empty() && !named.starts_with('/') && named.split('/').all(|part| part != "..")
}

enum Joined {
    Checked,
    Climbs(String),
    Unchecked,
}

fn peeled<'tcx>(expr: &'tcx Expr<'tcx>) -> &'tcx Expr<'tcx> {
    match expr.kind {
        ExprKind::AddrOf(_, _, inner) => peeled(inner),
        _ => expr,
    }
}

fn what_is_joined(cx: &LateContext<'_>, named: &Expr<'_>) -> Joined {
    let inner = peeled(named);

    match inner.kind {
        ExprKind::Lit(said) => match said.node {
            rustc_ast::LitKind::Str(text, _) => match stays_under(text.as_str()) {
                true => return Joined::Checked,
                false => return Joined::Climbs(text.to_string()),
            },
            _ => {}
        },
        ExprKind::Path(ref qpath) => {
            if matches!(
                cx.qpath_res(qpath, inner.hir_id),
                Res::Def(DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::Static { .. }, _)
            ) {
                return Joined::Checked;
            }
        }
        _ => {}
    }

    match cx.typeck_results().expr_ty(inner).peel_refs().kind() {
        ty::Adt(made, _) => {
            match matches!(cx.tcx.crate_name(made.did().krate).as_str(), "core" | "alloc" | "std") {
                true => Joined::Unchecked,
                false => Joined::Checked,
            }
        }
        _ => Joined::Unchecked,
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit086NoClimbingJoin {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let ExprKind::MethodCall(_, _, [named], _) = expr.kind else {
            return;
        };

        let Some(id) = cx.typeck_results().type_dependent_def_id(expr.hir_id) else {
            return;
        };

        if !joins(cx.tcx.def_path_str(id).as_str()) {
            return;
        }

        let said = match what_is_joined(cx, named) {
            Joined::Checked => return,
            Joined::Climbs(text) => format!("\"{text}\" does not stay under what it is joined to"),
            Joined::Unchecked => {
                "this name is joined as it arrived: a `/` at the front of it replaces the base, and a `..` walks out of it"
                    .to_string()
            }
        };

        span_lint_and_help(
            cx,
            EXPLICIT086_NO_CLIMBING_JOIN,
            named.span.source_callsite(),
            said,
            None,
            "join a name that is a type: one constructor that refuses a `/`, a `..` and an empty \
             name, private fields, and an `AsRef<Path>` so the join takes it and takes nothing \
             else. Where the name was made here and arrived from nowhere, allow this rule and let \
             the reason say where it was made",
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
