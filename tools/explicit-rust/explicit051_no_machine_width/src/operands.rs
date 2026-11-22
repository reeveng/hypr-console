//! Type constraints that survive the removal of an explicit integer receiver.

use super::fixes::{clean, concrete_annotation};
use clippy_utils::{higher::ForLoop, sym};
use rustc_ast::ast::{LitIntType, LitKind};
use rustc_hir::{Expr, ExprKind, HirId, Node, PatKind, QPath, def::Res};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;

// The first field of the standard Enumerate item has a fixed native type,
// independent of the iterator's own item and of any arithmetic in the loop.
pub(super) fn enumerated_position(
    cx: &LateContext<'_>,
    walk: &ForLoop<'_>,
    binding: HirId,
) -> bool {
    let PatKind::Tuple([position, _], rest) = walk.pat.kind else {
        return false;
    };
    if rest.as_opt_usize().is_some() || position.hir_id != binding {
        return false;
    }

    let types = cx.typeck_results();
    let method = match walk.arg.kind {
        ExprKind::MethodCall(_, _, [], _) => types.type_dependent_def_id(walk.arg.hir_id),
        ExprKind::Call(callee, [_]) => match types.expr_ty(callee).kind() {
            ty::FnDef(id, _) => Some(*id),
            _ => None,
        },
        _ => None,
    };
    method.is_some_and(|id| {
        cx.tcx.item_name(id).as_str() == "enumerate"
            && cx
                .tcx
                .trait_of_assoc(id)
                .is_some_and(|owner| cx.tcx.is_diagnostic_item(sym::Iterator, owner))
    })
}

// SIMD calls often take eight indexed lanes. Rust checks the non-closure
// arguments in source order. A written primitive call in an earlier lane
// gives its local receiver a width before method lookup in the next lane.
pub(super) fn earlier_operand_types_local<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    receiver: &Expr<'_>,
    method: rustc_hir::def_id::DefId,
) -> bool {
    if !source_or_vec_argument(cx, call) {
        return false;
    }
    let ExprKind::Path(path) = receiver.kind else {
        return false;
    };
    let Res::Local(binding) = cx.qpath_res(&path, receiver.hir_id) else {
        return false;
    };
    let mut child = call.hir_id;
    for (id, parent) in cx.tcx.hir_parent_iter(child) {
        let Node::Expr(parent) = parent else {
            return false;
        };
        let arguments = match parent.kind {
            ExprKind::Call(callee, arguments)
                if matches!(cx.typeck_results().expr_ty(callee).kind(), ty::FnDef(..)) =>
            {
                arguments
            }
            ExprKind::Array(arguments) | ExprKind::Tup(arguments) => arguments,
            ExprKind::Index(..)
            | ExprKind::MethodCall(..)
            | ExprKind::Unary(..)
            | ExprKind::Field(..) => {
                child = id;
                continue;
            }
            _ => return false,
        };
        let Some(index) = arguments.iter().position(|arg| arg.hir_id == child) else {
            return false;
        };
        if index != 0 && clean(cx, parent.span).is_some_and(|src| !src.contains('#')) {
            let found = arguments[..index].iter().any(|arg| {
                clippy_utils::visitors::for_each_expr_without_closures(arg, |earlier| {
                    if let ExprKind::Call(callee, [left, right]) = earlier.kind
                        && let ExprKind::Path(QPath::TypeRelative(written, _)) = callee.kind
                        && concrete_annotation(written)
                        && matches!(cx.typeck_results().expr_ty(callee).kind(), ty::FnDef(id, _) if *id == method)
                        && let ExprKind::Path(path) = left.kind
                        && cx.qpath_res(&path, left.hir_id) == Res::Local(binding)
                        && matches!(right.kind, ExprKind::Lit(lit) if matches!(lit.node, LitKind::Int(_, LitIntType::Unsuffixed)))
                        && !earlier.span.from_expansion()
                        && parent.span.contains(earlier.span)
                        && earlier.span.hi() < call.span.lo()
                        && source_or_vec_argument(cx, earlier)
                    {
                        std::ops::ControlFlow::Break(())
                    } else {
                        std::ops::ControlFlow::Continue(())
                    }
                }).is_some()
            });
            if found {
                return true;
            }
        }
        child = id;
    }
    false
}

// A macro can also stringify a captured expression. The standard repetition
// form of vec! passes its count to the allocator as an ordinary argument.
pub(super) fn source_or_vec_argument(cx: &LateContext<'_>, call: &Expr<'_>) -> bool {
    cx.tcx
        .hir_parent_id_iter(call.hir_id)
        .take_while(|id| id.owner == call.hir_id.owner)
        .all(|id| {
            clippy_utils::macros::macro_backtrace(cx.tcx.hir_span(id)).all(|mac| {
                cx.tcx
                    .is_diagnostic_item(clippy_utils::sym::vec_macro, mac.def_id)
            })
        })
}

// A grid or buffer with literal dimensions has the same element count on
// every target if both operands and the result fit the narrowest pointer.
// Keep the machine type when the surrounding API independently requires it.
pub(super) fn literal_arithmetic<'tcx>(
    cx: &LateContext<'tcx>,
    call: &'tcx Expr<'tcx>,
    left: &Expr<'_>,
    right: &Expr<'_>,
    method: &str,
) -> Option<(Span, String)> {
    let literal = |operand: &Expr<'_>| match operand.kind {
        ExprKind::Lit(lit) if !operand.span.from_expansion() => match lit.node {
            LitKind::Int(value, LitIntType::Unsuffixed) => Some(value.get()),
            _ => None,
        },
        _ => None,
    };
    let left = literal(left)?;
    let right = literal(right)?;
    let value = match method {
        "wrapping_mul" => left.checked_mul(right)?,
        "wrapping_add" => left.checked_add(right)?,
        "wrapping_sub" => left.checked_sub(right)?,
        _ => return None,
    };
    let ty = cx.typeck_results().expr_ty(call);
    let max = match ty.kind() {
        ty::Uint(ty::UintTy::Usize) => u128::from(u16::MAX),
        ty::Int(ty::IntTy::Isize) => i16::MAX as u128,
        _ => return None,
    };
    if [left, right, value].iter().any(|n| *n > max) || !source_or_vec_argument(cx, call) {
        return None;
    }
    let used =
        clippy_utils::expr_use_sites(cx.tcx, cx.typeck_results(), call.span.ctxt(), call).next()?;
    if !used.adjustments.is_empty() {
        return None;
    }
    let receiver = used.use_node(cx);
    // Removing the receiver's width must not choose an integer fallback or
    // prevent lookup of a subsequent inherent method.
    if receiver.is_recv() {
        return None;
    }
    let typed = match receiver {
        clippy_utils::ExprUseNode::FnArg(callee, index) => {
            let ty::FnDef(id, _) = *cx.typeck_results().expr_ty(callee).kind() else {
                return None;
            };
            // An enclosing primitive call may itself become method syntax in
            // this same pass. Its argument then loses its type constraint.
            if cx
                .tcx
                .inherent_impl_of_assoc(id)
                .is_some_and(|implementation| {
                    cx.tcx
                        .type_of(implementation)
                        .instantiate_identity()
                        .skip_norm_wip()
                        == ty
                })
            {
                return None;
            }
            cx.tcx
                .fn_sig(id)
                .skip_binder()
                .inputs()
                .skip_binder()
                .get(index)
                == Some(&ty)
        }
        _ => match receiver.defined_ty(cx)? {
            clippy_utils::DefinedTy::Hir(ty) => concrete_annotation(ty),
            clippy_utils::DefinedTy::Mir {
                ty: destination, ..
            } => destination.skip_binder() == ty,
        },
    };
    typed.then(|| (call.span, value.to_string()))
}
