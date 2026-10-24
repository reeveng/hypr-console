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

extern crate rustc_hir;
extern crate rustc_span;
extern crate rustc_middle;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Body, Expr, ExprKind, intravisit::FnKind, Pat, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use std::collections::HashSet;

dylint_linting::declare_late_lint! {
    /// EXPLICIT073: a `let` that reuses a name alive outside it makes one
    /// word mean two values. Name the second value what it is, or bind it
    /// `_`-prefixed if nobody will read it again.
    pub EXPLICIT073_NO_SHADOWING,
    Warn,
    "a name that means two values in one scope; the new value wants a word of its own"
}

struct Walker<'tcx> {
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    scopes: Vec<HashSet<String>>,
    hits: Vec<Span>,
}

impl<'tcx> Walker<'tcx> {
    // A binding is a shadow only when the name is alive somewhere outside
    // the scope being entered. A name reused in a sibling scope -- two arms
    // of one match -- never meets itself.
    fn shadows(&self, name: &str) -> bool {
        self.scopes.iter().any(|scope| scope.contains(name))
    }

    fn bind(&mut self, name: &str) {
        match self.scopes.last_mut() {
            Some(scope) => {
                scope.insert(name.to_owned());
            }
            None => {}
        }
    }
}

impl<'tcx> Visitor<'tcx> for Walker<'tcx> {
    fn visit_pat(&mut self, pat: &'tcx Pat<'tcx>) {
        // A lowering's own bindings are nobody's names.
        if pat.span.from_expansion() {
            return;
        }

        // Every half of an or-pattern binds the same names, so the first
        // half says them all and the rest would only say them again.
        if let PatKind::Or(halves) = pat.kind {
            if let Some(first) = halves.first() {
                self.visit_pat(first);
            }

            return;
        }

        if let PatKind::Binding(_, _, ident, _) = pat.kind {
            let name = ident.as_str();

            // The one spelling that says nobody will read the name again.
            if !name.starts_with('_') {
                if self.shadows(name) {
                    self.hits.push(ident.span);
                }

                self.bind(name);
            }
        }

        intravisit::walk_pat(self, pat);
    }

    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) {
        self.scopes.push(HashSet::new());
        intravisit::walk_block(self, block);
        self.scopes.pop();
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        match expr.kind {
            // Each arm is its own scope: the arms are alternatives, not a
            // sequence, and the same name in two arms is two values nobody
            // could confuse.
            ExprKind::Match(_, arms, _) => {
                for arm in arms {
                    self.scopes.push(HashSet::new());

                    self.visit_pat(arm.pat);

                    if let Some(guard) = arm.guard {
                        self.visit_expr(guard);
                    }

                    self.visit_expr(arm.body);

                    self.scopes.pop();
                }
            }

            // A closure's parameters are a scope of their own. The nested
            // body is resolved by hand because this visitor carries no
            // filter: its walk stays inside the function it was started for.
            ExprKind::Closure(closure) => {
                self.scopes.push(HashSet::new());

                let body = self.tcx.hir_body(closure.body);

                for param in body.params {
                    self.visit_pat(param.pat);
                }

                self.visit_expr(body.value);

                self.scopes.pop();
            }

            _ => intravisit::walk_expr(self, expr),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit073NoShadowing {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _decl: &'tcx rustc_hir::FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _span: rustc_span::Span,
        _def_id: rustc_hir::def_id::LocalDefId,
    ) {
        // Closures inside the function are walked as part of its body; only
        // the function itself starts a walk.
        if matches!(kind, FnKind::Closure) {
            return;
        }

        let mut walker = Walker {
            tcx: cx.tcx,
            scopes: vec![HashSet::new()],
            hits: Vec::new(),
        };

        for param in body.params {
            walker.visit_pat(param.pat);
        }

        walker.visit_expr(body.value);

        for hit in walker.hits {
            span_lint_and_help(
                cx,
                EXPLICIT073_NO_SHADOWING,
                hit,
                "this name is already bound outside the scope being entered",
                None,
                "name the new value what it is -- a conversion names what it becomes, EXPLICIT045's \
                 own sentence -- or bind it `_`-prefixed if nobody will read it again. Where the \
                 second binding really is the same value narrowed, allow this rule here and let \
                 the reason say so",
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "main");
}
