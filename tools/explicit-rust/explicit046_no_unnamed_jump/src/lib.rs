//! EXPLICIT046: a jump out of a nested loop says which loop it leaves.
//!
//! EXPLICIT019 wants both outcomes of a decision named, and EXPLICIT023 says
//! the same of a `let ... else` whose `else` names nothing. A bare `break`
//! inside two loops is the case neither of them reaches. It leaves one of the
//! two and the code does not say which, so the reader counts braces, and the
//! next person to wrap a loop around it changes where it goes without touching
//! the line.
//!
//! The rule is only about the nested case. One loop has one answer and a label
//! there would be ceremony, which is why this does not read as a rule about
//! labels: it is a rule about a jump whose destination is a guess.
//!
//! Kast is where the shape came from. It has no anonymous non-local exit at
//! all: `unwindable_block` makes a block and hands the body a token, and
//! `unwind token value` names the token it returns to, so the destination of a
//! jump is a value that had to be passed in before it could be used. A label is
//! the smallest version of that available here.
//!
//! What is not touched is a jump out of a loop inside a closure. A closure is
//! its own body and the loops outside it are not reachable from within, so the
//! `break` has one destination and says it by being where it is.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_hir_and_then;
use clippy_utils::higher::ForLoop;
use clippy_utils::is_lint_allowed;
use rustc_errors::Applicability;
use rustc_hir::{Destination, Expr, ExprKind, HirId, LoopSource, Node, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{BytePos, Span, hygiene::DesugaringKind};
use std::collections::HashMap;

dylint_linting::impl_late_lint! {
    /// EXPLICIT046: a bare `break` or `continue` inside a nested loop leaves
    /// one of the loops and does not say which, so the destination moves when
    /// someone wraps another loop around it. Name the loop with a label.
    pub EXPLICIT046_NO_UNNAMED_JUMP,
    Deny,
    "a jump out of a nested loop that does not name the loop it leaves",
    Explicit046NoUnnamedJump::default()
}

#[derive(Default)]
pub struct Explicit046NoUnnamedJump {
    jumps: HashMap<HirId, Vec<Jump>>,
}

struct Jump {
    id: HirId,
    span: Span,
    word: &'static str,
}

fn jumping(kind: &ExprKind<'_>) -> Option<(Destination, &'static str)> {
    match kind {
        ExprKind::Break(to, _value) => Some((*to, "break")),
        ExprKind::Continue(to) => Some((*to, "continue")),
        _ => None,
    }
}

// How many loops this jump is standing inside, stopping at the closure or the
// body that ends the walk. A labelled block is not a loop and is not counted:
// a `break` cannot reach one without naming it.
fn loops_around(cx: &LateContext<'_>, expr: &Expr<'_>) -> usize {
    let mut found = 0;

    for (_id, node) in cx.tcx.hir_parent_iter(expr.hir_id) {
        match node {
            Node::Expr(above) => match above.kind {
                ExprKind::Loop(..) => found += 1,
                ExprKind::Closure(..) => return found,
                _ => {}
            },
            Node::Item(_) => return found,
            _ => {}
        }
    }

    found
}

fn loop_name(cx: &LateContext<'_>, expr: &Expr<'_>, source: LoopSource) -> Option<(Span, String)> {
    let span = match source {
        LoopSource::ForLoop if expr.span.desugaring_kind() == Some(DesugaringKind::ForLoop) => {
            expr.span.parent_callsite()?
        }
        _ => expr.span,
    };
    let source_map = cx.sess().source_map();
    let keyword = source.name();
    if span.from_expansion()
        || source_map
            .span_to_snippet(span.with_hi(span.lo() + BytePos(keyword.len() as u32)))
            .ok()?
            .as_str()
            != keyword
    {
        return None;
    }

    let depth = loops_around(cx, expr) + 1;
    let mut name = format!("'inner_{keyword}_{depth}");
    if source == LoopSource::ForLoop {
        for (_, parent) in cx.tcx.hir_parent_iter(expr.hir_id).take(4) {
            if let Node::Expr(parent) = parent
                && let Some(authored) = ForLoop::hir(parent)
                && authored.loop_id == expr.hir_id
                && let PatKind::Binding(_, _, ident, None) = authored.pat.kind
                && ident.name.as_str().is_ascii()
            {
                name = format!("'over_{}_{depth}", ident.name);
                break;
            }
        }
    }
    let file = source_map.lookup_source_file(span.lo());
    let text = file.src.as_ref()?;
    while text.contains(&name) {
        name.push('_');
    }

    Some((span.shrink_to_lo(), name))
}

fn jump_edit(
    cx: &LateContext<'_>,
    jump: &Jump,
    target: HirId,
    name: &str,
) -> Option<(Span, String)> {
    for (id, node) in cx.tcx.hir_parent_iter(jump.id) {
        if id == target {
            break;
        }
        if let Node::Expr(expr) = node
            && let ExprKind::Block(_, Some(label)) | ExprKind::Loop(_, Some(label), ..) = expr.kind
            && label.ident.name.as_str() == name
        {
            return None;
        }
    }
    let word = jump
        .span
        .with_hi(jump.span.lo() + BytePos(jump.word.len() as u32));
    let snippet = cx.sess().source_map().span_to_snippet(word).ok()?;
    (snippet == jump.word).then(|| (word.shrink_to_hi(), format!(" {name}")))
}

impl<'tcx> LateLintPass<'tcx> for Explicit046NoUnnamedJump {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let Some((to, word)) = jumping(&expr.kind) else {
            return;
        };

        let (None, Ok(target)) = (to.label, to.target_id) else {
            return;
        };

        if loops_around(cx, expr) > 1
            && !is_lint_allowed(cx, EXPLICIT046_NO_UNNAMED_JUMP, expr.hir_id)
        {
            self.jumps.entry(target).or_default().push(Jump {
                id: expr.hir_id,
                span: expr.span,
                word,
            });
        }
    }

    fn check_expr_post(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Loop(_, label, source, ..) = expr.kind else {
            return;
        };
        let Some(jumps) = self.jumps.remove(&expr.hir_id) else {
            return;
        };

        let (insertion, name) = match label {
            Some(label) if !label.ident.span.from_expansion() => {
                (None, Some(label.ident.name.to_string()))
            }
            Some(_) => (None, None),
            None => match loop_name(cx, expr, source) {
                Some((span, name)) => (Some(span), Some(name)),
                None => (None, None),
            },
        };
        let edits: Vec<_> = name
            .iter()
            .flat_map(|name| {
                jumps.iter().filter_map(|jump| {
                    jump_edit(cx, jump, expr.hir_id, name).map(|edit| (jump.id, edit))
                })
            })
            .collect();

        for jump in jumps {
            span_lint_hir_and_then(
                cx,
                EXPLICIT046_NO_UNNAMED_JUMP,
                jump.id,
                jump.span,
                format!(
                    "this `{}` is inside more than one loop and names none of them",
                    jump.word
                ),
                |diagnostic| {
                    if let (Some(span), Some(name), Some((first, _))) =
                        (insertion, &name, edits.first())
                    {
                        if jump.id == *first {
                            let mut group = vec![(span, format!("{name}: "))];
                            group.extend(edits.iter().map(|(_, edit)| edit.clone()));
                            diagnostic.multipart_suggestion(
                                "label the destination loop and its jumps",
                                group,
                                Applicability::MachineApplicable,
                            );
                            return;
                        }
                    } else if let Some((_, (span, replacement))) =
                        edits.iter().find(|(id, _)| *id == jump.id)
                    {
                        diagnostic.span_suggestion(
                            *span,
                            "name the destination loop",
                            replacement.clone(),
                            Applicability::MachineApplicable,
                        );
                        return;
                    }
                    diagnostic.help("label the loop this leaves and say it here: `'over_rows: for ...` and `break 'over_rows`. A bare jump leaves the innermost loop, which is a fact about where the line happens to sit rather than about what it means, and it moves the next time someone wraps a loop around it");
                },
            );
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
