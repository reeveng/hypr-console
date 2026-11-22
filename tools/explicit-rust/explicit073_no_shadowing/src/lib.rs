//! EXPLICIT073: one name, one value, one scope.
//!
//! Shadowing is a `let` that reuses a name: the reader who saw `held` bound
//! at the top of the block and meets `held` again three screens later is
//! holding two values under one word, and which one the next line means is a
//! question the source does not answer. Rust blesses it -- the second binding
//! is a new binding, and conversions like `let path = path.join(name)` are
//! the idiom it was designed for -- but the idiom is the fault this rule
//! names: `path` meant a directory for half a block and a file for the rest,
//! and nothing on either line says which half is which.
//!
//! What the rule asks is the naming this tree does anyway: a conversion names
//! what it becomes (EXPLICIT045's own sentence -- `let joined = path.join()`),
//! and a rebinding for a narrower life gets a narrower word. A `_`-prefixed
//! binding is left alone: it is the one spelling that says "this is not a
//! name anybody will read again", which is the disclaimer the rule asks the
//! shadowed value to carry.
//!
//! The scopes the rule reads are the ones the language gives: a block is a
//! scope, a match arm is a scope, a closure's parameters are a scope, and a
//! binding shadows any binding alive outside it. Two arms of one match
//! binding the same name are two scopes and are left alone -- the arms are
//! alternatives, not one after the other -- and so are the two halves of an
//! or-pattern, `A(held) | B(held)`, which bind one name once whichever half
//! matched. A binding the compiler wrote is not one a reader meets: the
//! `iter` a `for` lowers into and the `residual` a `?` does are named at no
//! line of the source, so they neither shadow nor are shadowed.
//!
//! It arrives `Warn`: the tree has conversions that shadow, and each wants
//! either the new name or the allow saying the two values are one value,
//! being narrowed.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

mod fixes;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, HirId, LocalSource, Node, Pat, PatKind, intravisit::FnKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::{hir::nested_filter::OnlyBodies, middle::region::ScopeTree, ty::TyCtxt};
use rustc_span::{Span, Symbol, symbol::Ident};

use std::collections::HashMap;

dylint_linting::impl_late_lint! {
    /// EXPLICIT073: a `let` that reuses a name alive outside it makes one
    /// word mean two values. Name the second value what it is, or bind it
    /// `_`-prefixed if nobody will read it again.
    pub EXPLICIT073_NO_SHADOWING,
    Warn,
    "a name that means two values in one scope; the new value wants a word of its own",
    Explicit073NoShadowing {
        expression_macros: dylint_linting::config_or_default("explicit073_expression_macros"),
        body_macros: dylint_linting::config_or_default("explicit073_body_macros"),
    }
}

pub struct Explicit073NoShadowing {
    expression_macros: Vec<String>,
    body_macros: Vec<String>,
}

struct Walker<'tcx> {
    tcx: TyCtxt<'tcx>,
    scopes: &'tcx ScopeTree,
    bindings: HashMap<Symbol, Vec<HirId>>,
    hits: Vec<(HirId, Ident)>,
}

impl<'tcx> Visitor<'tcx> for Walker<'tcx> {
    type MaybeTyCtxt = TyCtxt<'tcx>;
    type NestedFilter = OnlyBodies;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_pat(&mut self, pat: &'tcx Pat<'tcx>) {
        if let PatKind::Binding(_, id, ident, _) = pat.kind
            && id == pat.hir_id
            && !pat.span.from_expansion()
            && !ident.as_str().starts_with('_')
            && !matches!(
                self.tcx.hir_parent_iter(id).map(|(_, node)| node)
                    .find(|node| !matches!(node, Node::Pat(_) | Node::PatField(_))),
                Some(Node::LetStmt(local)) if matches!(local.source, LocalSource::AssignDesugar)
            )
            && let Some(scope) = self.scopes.var_scope(id.local_id)
        {
            let earlier = self.bindings.entry(ident.name).or_default();
            if earlier.iter().rev().any(|previous| {
                previous.owner == id.owner
                    && self
                        .scopes
                        .var_scope(previous.local_id)
                        .is_some_and(|outer| self.scopes.is_subscope_of(scope, outer))
            }) {
                self.hits.push((id, ident));
            }
            earlier.push(id);
        }

        intravisit::walk_pat(self, pat);
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit073NoShadowing {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx rustc_hir::FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: rustc_hir::def_id::LocalDefId,
    ) {
        // Closures inside the function are walked as part of its body; only
        // the function itself starts a walk.
        if matches!(kind, FnKind::Closure) {
            return;
        }

        let mut walker = Walker {
            tcx: cx.tcx,
            scopes: cx.tcx.region_scope_tree(def_id),
            bindings: HashMap::new(),
            hits: Vec::new(),
        };
        walker.visit_body(body);

        if walker.hits.is_empty() {
            return;
        }

        let mut fixes = fixes::Renames::new(
            cx,
            body,
            span,
            &walker.hits,
            &self.expression_macros,
            &self.body_macros,
        );

        for (id, ident) in walker.hits {
            span_lint_and_then(
                cx,
                EXPLICIT073_NO_SHADOWING,
                ident.span,
                "this name is already bound outside the scope being entered",
                |diagnostic| {
                    if let Some((name, edits)) =
                        fixes.as_mut().and_then(|fixes| fixes.take(id, ident))
                    {
                        diagnostic.multipart_suggestion(
                            format!("rename this binding to `{name}`"),
                            edits,
                            rustc_errors::Applicability::MachineApplicable,
                        );
                    } else {
                        diagnostic.help(
                            "name the new value what it is -- a conversion names what it becomes, \
                             EXPLICIT045's own sentence -- or bind it `_`-prefixed if nobody will \
                             read it again. Where the second binding really is the same value \
                             narrowed, allow this rule here and let the reason say so",
                        );
                    }
                },
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}

#[test]
fn renames() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn reviewed_macros() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_macros")
        .dylint_toml(
            r#"explicit073_expression_macros = ["expressions::twice", "spelling::pass"]
explicit073_body_macros = ["body_wrapper::keep_body"]"#,
        )
        .run();
}
