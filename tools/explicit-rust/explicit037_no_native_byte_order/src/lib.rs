#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;

use rustc_ast::attr::data_structures::CfgEntry;
use rustc_errors::Applicability;
use rustc_hir::{Attribute, attrs::AttributeKind};
use rustc_middle::ty;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_hir::{Expr, ExprKind, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};

dylint_linting::declare_late_lint! {
    /// EXPLICIT037: a number laid out as bytes says which end it starts at.
    ///
    /// `to_ne_bytes` and `from_ne_bytes` are the byte order of whatever machine
    /// compiled the call, which is the one fact about the layout that is
    /// nowhere in the line. Everywhere those bytes are going -- a wire, a file,
    /// a device, another program -- has an order of its own, and the two agree
    /// today because this desktop runs on one kind of machine. That is the
    /// shape of every assumption this suite is against: correct, unwritten, and
    /// checked by nothing.
    ///
    /// `console-bus` is the crate this is about. A D-Bus message carries its
    /// byte order in the first byte of its own header -- the sender says `l` or
    /// `B` and the reader believes it -- so the order is a value that arrives
    /// rather than a property of whoever is reading, and a walk over a
    /// signature that used the native one would be right until the day
    /// something on the other end was not.
    ///
    /// It arrives with nothing to say, which is the point of writing it now.
    /// There is no `ne` in the tree today, and a rule that is already kept
    /// costs a reader nothing and holds the day someone reaches for the
    /// spelling that looks like it means "no conversion". `to_le_bytes` and
    /// `to_be_bytes` say which end, and where the end is a value someone was
    /// told, it is that value the code should be asking.
    pub EXPLICIT037_NO_NATIVE_BYTE_ORDER,
    Deny,
    "a number laid out in the machine's own byte order is a layout no one wrote down"
}

fn says_nothing_about_the_order(named: &str) -> bool {
    matches!(named, "to_ne_bytes" | "from_ne_bytes")
}

// All targets admitted by the gate must agree, including targets other than
// the one on which rustfix happens to run.
fn required_order(cfg: &CfgEntry) -> Option<&'static str> {
    match cfg {
        CfgEntry::NameValue {
            name,
            value: Some(value),
            ..
        } => match (name.as_str(), value.as_str()) {
            ("target_endian", "little") | ("target_arch", "x86" | "x86_64") => Some("le"),
            ("target_endian", "big") => Some("be"),
            _ => None,
        },
        CfgEntry::All(entries, _) => entries.iter().find_map(required_order),
        CfgEntry::Any(entries, _) => {
            let first = required_order(entries.first()?)?;
            entries
                .iter()
                .all(|entry| required_order(entry) == Some(first))
                .then_some(first)
        }
        _ => None,
    }
}

fn enclosing_order(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<&'static str> {
    let map = cx.sess().source_map();
    let file = map.lookup_source_file(expr.span.lo());
    std::iter::once(expr.hir_id)
        .chain(cx.tcx.hir_parent_id_iter(expr.hir_id))
        .find_map(|id| {
            let attrs = cx.tcx.hir_attrs(id);
            // A cfg introduced by cfg_attr only applies in some builds.
            if attrs
                .iter()
                .any(|attr| matches!(attr, Attribute::Parsed(AttributeKind::CfgAttrTrace)))
            {
                return None;
            }
            attrs.iter().find_map(|attr| {
                let Attribute::Parsed(AttributeKind::CfgTrace(entries)) = attr else {
                    return None;
                };
                entries.iter().find_map(|(cfg, span)| {
                    // A module in another file may include this source under other gates.
                    (!span.from_expansion() && file.contains(span.lo()))
                        .then(|| required_order(cfg))
                        .flatten()
                })
            })
        })
}

impl<'tcx> LateLintPass<'tcx> for Explicit037NoNativeByteOrder {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }

        let types = cx.typeck_results();
        let (segment, called, primitive) = match expr.kind {
            ExprKind::MethodCall(path, receiver, _, _) => (
                path,
                types.type_dependent_def_id(expr.hir_id),
                types.expr_ty_adjusted(receiver).peel_refs(),
            ),
            ExprKind::Call(callee, _) => {
                let ExprKind::Path(QPath::TypeRelative(of, segment)) = callee.kind else {
                    return;
                };
                (
                    segment,
                    clippy_utils::fn_def_id(cx, expr),
                    types.node_type(of.hir_id),
                )
            }
            _ => return,
        };
        let named = segment.ident.as_str();
        if !primitive.is_numeric() || !says_nothing_about_the_order(named) {
            return;
        }
        span_lint_and_then(
            cx,
            EXPLICIT037_NO_NATIVE_BYTE_ORDER,
            expr.span,
            format!("`{named}` lays a number out in whatever order this machine happens to use"),
            |diag| {
                // A trait can give a primitive another method of the same name.
                let inherent = called
                    .and_then(|id| cx.tcx.inherent_impl_of_assoc(id))
                    .is_some_and(|id| {
                        cx.tcx.type_of(id).instantiate_identity().skip_norm_wip() == primitive
                    });
                let single_byte = matches!(
                    primitive.kind(),
                    ty::Uint(ty::UintTy::U8) | ty::Int(ty::IntTy::I8)
                );
                if inherent
                    && !segment.ident.span.from_expansion()
                    && let Some(order) = single_byte
                        .then_some("le")
                        .or_else(|| enclosing_order(cx, expr))
                    && cx
                        .sess()
                        .source_map()
                        .span_to_snippet(segment.ident.span)
                        .ok()
                        .as_deref()
                        == Some(named)
                {
                    diag.span_suggestion(
                        segment.ident.span,
                        "name the byte order guaranteed here",
                        named.replace("_ne_", &format!("_{order}_")),
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help("say which end it starts at: `to_le_bytes` and `to_be_bytes`, and their `from_` pair. Where something else declares the order -- a D-Bus header says `l` or `B` in its first byte -- it is what that says that decides, not what the compiler was run on");
                }
            },
        );
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
