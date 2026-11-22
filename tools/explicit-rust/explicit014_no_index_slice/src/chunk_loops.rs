//! Fixed-width slice iterators can hand their loop an array of element references.
use super::counted::place;
use super::fixes::{clean, function_source, position};
use clippy_utils::{higher::ForLoop, msrvs::Msrv, sym, visitors::for_each_expr};
use rustc_hir::{BindingMode, Expr, ExprKind, HirId, Node, Pat, PatKind, QPath, def::Res};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use std::ops::ControlFlow;

type Edits = Vec<(Span, String)>;

struct Chunk {
    iterator: String,
    edits: Edits,
    accesses: Vec<HirId>,
}

fn chunk<'tcx>(
    cx: &LateContext<'tcx>,
    iterator: &'tcx Expr<'tcx>,
    pattern: &'tcx Pat<'tcx>,
    body: &'tcx Expr<'tcx>,
    iterator_required: bool,
) -> Option<Chunk> {
    let PatKind::Binding(BindingMode::NONE, binding, ident, None) = pattern.kind else {
        return None;
    };
    let ExprKind::MethodCall(_, base, [width], _) = iterator.kind else {
        return None;
    };
    let types = cx.typeck_results();
    let method = types.type_dependent_def_id(iterator.hir_id)?;
    let name = cx.tcx.item_name(method);
    let (minimum, mutable) = match name.as_str() {
        "chunks_exact" => (88, false),
        "chunks_exact_mut" => (88, true),
        "windows" => (94, false),
        _ => return None,
    };
    let implementation = cx.tcx.inherent_impl_of_assoc(method)?;
    if !cx
        .tcx
        .type_of(implementation)
        .instantiate_identity()
        .skip_norm_wip()
        .is_slice()
    {
        return None;
    }
    let mut msrv = Msrv::default();
    msrv.read_cargo(cx.sess());
    if !msrv.meets(
        cx,
        rustc_hir::RustcVersion {
            major: 1,
            minor: minimum,
            patch: 0,
        },
    ) {
        return None;
    }
    let width = usize::from(position(width, 1)?);
    if !(1..=16).contains(&width) {
        return None;
    }
    // These receivers coerce directly to a slice for the fully qualified call.
    let base_ty = types.expr_ty(base);
    match base_ty.peel_refs().kind() {
        ty::Array(..) | ty::Slice(_) => {}
        ty::Adt(adt, _) if cx.tcx.is_diagnostic_item(sym::Vec, adt.did()) => {}
        _ => return None,
    }
    clean(cx, iterator.span)?;
    clean(cx, pattern.span)?;
    let source = function_source(cx, body)?;
    // A macro may use this binding only in another build. Its expansion
    // cannot establish that every reference is one of the indexed uses below.
    let body_source = cx.sess().source_map().span_to_snippet(body.span).ok()?;
    if source.contains('#') || source.contains("unsafe") || body_source.contains('!') {
        return None;
    }
    let receiver = clean(cx, base.span)?;
    let storage = if base_ty.is_ref() {
        receiver
    } else {
        let borrow = if mutable { "&mut " } else { "&" };
        match base.kind {
            ExprKind::Path(..) | ExprKind::Field(..) => format!("{borrow}{receiver}"),
            _ => format!("{borrow}({receiver})"),
        }
    };
    let iterator = match name.as_str() {
        "windows" => format!("<[_]>::array_windows::<{width}>({storage})"),
        "chunks_exact" if iterator_required => {
            format!("<[_]>::as_chunks::<{width}>({storage}).0.iter()")
        }
        "chunks_exact" => format!("<[_]>::as_chunks::<{width}>({storage}).0"),
        _ if iterator_required => {
            format!("<[_]>::as_chunks_mut::<{width}>({storage}).0.iter_mut()")
        }
        _ => format!("<[_]>::as_chunks_mut::<{width}>({storage}).0"),
    };
    let mut slots = vec!["_".to_owned(); width];
    let mut edits = Vec::new();
    let mut accesses = Vec::new();
    let owner = cx.tcx.hir_enclosing_body_owner(body.hir_id);
    let unsupported = for_each_expr(cx, body, |use_| {
        if !matches!(use_.kind, ExprKind::Path(QPath::Resolved(None, path)) if path.res == Res::Local(binding))
        {
            return ControlFlow::Continue(());
        }
        let Node::Expr(access) = cx.tcx.parent_hir_node(use_.hir_id) else {
            return ControlFlow::Break(());
        };
        let ExprKind::Index(base, at, _) = access.kind else {
            return ControlFlow::Break(());
        };
        if base.hir_id != use_.hir_id
            || cx.tcx.hir_enclosing_body_owner(access.hir_id) != owner
            || clean(cx, access.span).is_none()
        {
            return ControlFlow::Break(());
        }
        let Some(index) = position(at, 8).map(usize::from).filter(|&at| at < width) else {
            return ControlFlow::Break(());
        };
        let name = format!("{}_{}", ident.name, index);
        let Some((_, parentheses)) = place(cx, access) else {
            return ControlFlow::Break(());
        };
        if source.contains(&name) {
            return ControlFlow::Break(());
        }
        slots[index] = name.clone();
        edits.push((
            access.span,
            if parentheses {
                format!("(*{name})")
            } else {
                format!("*{name}")
            },
        ));
        accesses.push(access.hir_id);
        ControlFlow::Continue(())
    });
    if unsupported.is_some() || accesses.is_empty() {
        return None;
    }
    edits.push((pattern.span, format!("[{}]", slots.join(", "))));
    Some(Chunk {
        iterator,
        edits,
        accesses,
    })
}

pub(super) fn edits<'tcx>(
    cx: &LateContext<'tcx>,
    walk: &ForLoop<'tcx>,
) -> Option<(Vec<HirId>, Edits)> {
    if !cx.sess().edition().at_least_rust_2018() {
        return None;
    }
    let mut chunks = if let Some(single) = chunk(cx, walk.arg, walk.pat, walk.body, false) {
        single
    } else {
        let PatKind::Tuple([left_pat, right_pat], rest) = walk.pat.kind else {
            return None;
        };
        let ExprKind::MethodCall(_, lhs, args, _) = walk.arg.kind else {
            return None;
        };
        let types = cx.typeck_results();
        let adapter = types.type_dependent_def_id(walk.arg.hir_id)?;
        if rest.as_opt_usize().is_some()
            || !cx
                .tcx
                .trait_of_assoc(adapter)
                .is_some_and(|id| cx.tcx.is_diagnostic_item(sym::Iterator, id))
            || !types.expr_adjustments(lhs).is_empty()
        {
            return None;
        }
        clean(cx, walk.arg.span)?;
        if cx.tcx.item_name(adapter).as_str() == "enumerate" && args.is_empty() {
            let mut item = chunk(cx, lhs, right_pat, walk.body, true)?;
            item.edits.push((
                walk.arg.span,
                format!("::core::iter::Iterator::enumerate({})", item.iterator),
            ));
            return Some((item.accesses, item.edits));
        }
        let [rhs] = args else { return None };
        if cx.tcx.item_name(adapter).as_str() != "zip" {
            return None;
        }
        let left = chunk(cx, lhs, left_pat, walk.body, true);
        let right = chunk(cx, rhs, right_pat, walk.body, false);
        if left.is_none() && right.is_none() {
            return None;
        }
        let left_text = left
            .as_ref()
            .map(|c| c.iterator.clone())
            .or_else(|| clean(cx, lhs.span))?;
        let right_text = right
            .as_ref()
            .map(|c| c.iterator.clone())
            .or_else(|| clean(cx, rhs.span))?;
        let mut joined = Chunk {
            iterator: format!("::core::iter::Iterator::zip({left_text}, {right_text})"),
            edits: Vec::new(),
            accesses: Vec::new(),
        };
        for chunk in [left, right].into_iter().flatten() {
            joined.edits.extend(chunk.edits);
            joined.accesses.extend(chunk.accesses);
        }
        joined
    };
    chunks.edits.push((walk.arg.span, chunks.iterator));
    Some((chunks.accesses, chunks.edits))
}
