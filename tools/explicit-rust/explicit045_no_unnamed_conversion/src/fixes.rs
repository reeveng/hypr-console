//! Turn the standard blanket `Into` implementation back into its `From` call.

use clippy_utils::{is_in_const_context, sym, ty::implements_trait};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{CRATE_DEF_ID, CrateNum, DefId, LocalModDefId, ModDefId};
use rustc_hir::{BorrowKind, Expr, ExprKind};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::adjustment::{
    Adjust, Adjustment, AutoBorrow, AutoBorrowMutability, DerefAdjustKind,
};
use rustc_middle::ty::{self, GenericArgKind, Ty};
use rustc_span::{BytePos, Span, Symbol};

#[path = "../../shared/local_type_path.rs"]
mod local_type_path;
use local_type_path::{identifier, local_path};

fn extern_name(cx: &LateContext<'_>, crate_num: CrateNum) -> Option<String> {
    let source = cx.tcx.used_crate_source(crate_num);
    // Match the compiler's loaded artifact to its command-line name. A Cargo
    // rename or another version of the same package can make crate_name wrong.
    let (name, _) = cx.sess().opts.externs.iter().find(|(_, entry)| {
        entry.add_prelude
            && entry.files().is_some_and(|mut files| {
                files.any(|file| {
                    [&source.rmeta, &source.rlib, &source.dylib]
                        .into_iter()
                        .flatten()
                        .any(|path| file.canonicalized() == path)
                })
            })
    })?;
    Some(identifier(cx, Symbol::intern(name)))
}

fn external_path(cx: &LateContext<'_>, mut id: DefId) -> Option<String> {
    let mut segments = Vec::new();
    let parents = cx.tcx.visible_parent_map(());
    // Each step must be a public module export of the exact definition.
    while !id.is_crate_root() {
        let parent = *parents.get(&id)?;
        if cx.tcx.def_kind(parent) != DefKind::Mod || segments.len() == 16 {
            return None;
        }
        let child = cx
            .tcx
            .module_children(ModDefId::new_unchecked(parent))
            .iter()
            .find(|child| child.res.opt_def_id() == Some(id) && child.vis.is_public())?;
        segments.push(identifier(cx, child.ident.name));
        id = parent;
    }
    segments.push(extern_name(cx, id.krate)?);
    segments.reverse();
    Some(format!("::{}", segments.join("::")))
}

fn named_type<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
    module: LocalModDefId,
    budget: u8,
) -> Option<String> {
    let next = budget.checked_sub(1)?;
    match ty.kind() {
        ty::Int(width) if width.bit_width().is_none() => None,
        ty::Uint(width) if width.bit_width().is_none() => None,
        ty::Bool | ty::Char | ty::Str | ty::Int(_) | ty::Uint(_) | ty::Float(_) => {
            Some(format!("::core::primitive::{ty}"))
        }
        ty::Ref(region, inner, rustc_hir::Mutability::Not) => {
            let lifetime = if region.is_static() { "'static " } else { "" };
            Some(format!(
                "&{lifetime}{}",
                named_type(cx, *inner, module, next)?
            ))
        }
        ty::Slice(inner) => Some(format!("[{}]", named_type(cx, *inner, module, next)?)),
        ty::Array(inner, length) => Some(format!(
            "[{}; {}]",
            named_type(cx, *inner, module, next)?,
            length.try_to_target_usize(cx.tcx)?,
        )),
        ty::Tuple(items) => {
            let mut fields = items
                .iter()
                .map(|item| named_type(cx, item, module, next))
                .collect::<Option<Vec<_>>>()?;
            if fields.len() == 1 {
                fields.push(String::new());
            }
            Some(format!("({})", fields.join(", ")))
        }
        ty::Adt(adt, args) => {
            let id = adt.did();
            let no_std = rustc_hir::find_attr!(cx.tcx, CRATE_DEF_ID, NoStd);
            let mut args = args.as_slice();
            let path = if id.is_local() {
                local_path(cx, id, module)?
            } else if cx.tcx.is_diagnostic_item(sym::Option, id) {
                "::core::option::Option".to_owned()
            } else if cx.tcx.is_diagnostic_item(sym::Result, id) {
                "::core::result::Result".to_owned()
            } else if !no_std && cx.tcx.lang_items().string() == Some(id) {
                "::std::string::String".to_owned()
            } else if !no_std && cx.tcx.is_diagnostic_item(sym::OsString, id) {
                "::std::ffi::OsString".to_owned()
            } else if !no_std
                && cx.tcx.crate_name(id.krate).as_str() == "std"
                && cx.tcx.def_path_str(id) == "std::path::PathBuf"
            {
                "::std::path::PathBuf".to_owned()
            } else if !no_std
                && (cx.tcx.is_diagnostic_item(sym::Vec, id)
                    || cx.tcx.lang_items().owned_box() == Some(id))
            {
                let [element, allocator] = args else {
                    return None;
                };
                let ty::Adt(allocator, _) = allocator.as_type()?.kind() else {
                    return None;
                };
                if cx.tcx.lang_items().global_alloc_ty() != Some(allocator.did()) {
                    return None;
                }
                args = std::slice::from_ref(element);
                if cx.tcx.lang_items().owned_box() == Some(id) {
                    "::std::boxed::Box".to_owned()
                } else {
                    "::std::vec::Vec".to_owned()
                }
            } else {
                external_path(cx, id)?
            };
            let parameters = args
                .iter()
                .map(|arg| match arg.kind() {
                    GenericArgKind::Lifetime(region) => {
                        Some(if region.is_static() { "'static" } else { "'_" }.to_owned())
                    }
                    GenericArgKind::Type(inner) => named_type(cx, inner, module, next),
                    GenericArgKind::Const(_) => None,
                })
                .collect::<Option<Vec<_>>>()?;
            if parameters.is_empty() {
                Some(path)
            } else {
                Some(format!("{path}<{}>", parameters.join(", ")))
            }
        }
        _ => None,
    }
}

fn argument_span(cx: &LateContext<'_>, receiver: &Expr<'_>) -> Option<Span> {
    let span = receiver.span;
    if !matches!(receiver.kind, ExprKind::AddrOf(BorrowKind::Ref, ..)) {
        return Some(span);
    }
    let source = cx.sess().source_map().span_to_snippet(span).ok()?;
    if source.contains("//") || source.contains("/*") {
        return Some(span);
    }
    // A reference needs grouping to be a method receiver, but a function
    // argument can take it directly. The HIR kind rules out tuple syntax.
    let mut argument = source.as_str();
    while let Some(inside) = argument
        .strip_prefix('(')
        .and_then(|text| text.strip_suffix(')'))
    {
        argument = inside.trim();
    }
    let start = BytePos(u32::try_from(source.find(argument)?).ok()?);
    let width = BytePos(u32::try_from(argument.len()).ok()?);
    Some(
        span.with_lo(span.lo() + start)
            .with_hi(span.lo() + start + width),
    )
}

pub(super) fn named_conversion<'tcx>(
    cx: &LateContext<'tcx>,
    call: &Expr<'tcx>,
    receiver: &Expr<'tcx>,
    from: Ty<'tcx>,
    to: Ty<'tcx>,
) -> Option<Vec<(Span, String)>> {
    let types = cx.typeck_results();
    if !cx.sess().edition().at_least_rust_2018()
        || is_in_const_context(cx)
        || receiver.span.from_expansion()
        || !call.span.contains(receiver.span)
        || types.expr_ty(receiver) != from
    {
        return None;
    }
    // Calling a by-value method on a shared reference can add an ordinary
    // reborrow. Other adjustments can call user code or change the type.
    match types.expr_adjustments(receiver) {
        [] => {}
        [
            Adjustment {
                kind: Adjust::Deref(DerefAdjustKind::Builtin),
                ..
            },
            Adjustment {
                kind: Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not)),
                ..
            },
        ] if matches!(from.kind(), ty::Ref(_, _, rustc_hir::Mutability::Not)) => {}
        _ => return None,
    }
    let method = types.type_dependent_def_id(call.hir_id)?;
    if !cx.tcx.is_diagnostic_item(sym::Into, cx.tcx.parent(method)) {
        return None;
    }
    let owner = cx.tcx.hir_enclosing_body_owner(call.hir_id);
    let module = cx.tcx.parent_module_from_def_id(owner);
    let target_type = named_type(cx, to, module, 8)?;
    // When this pair implements From, coherence makes the standard blanket
    // implementation the only possible implementation of the standard Into.
    let prefix = if implements_trait(
        cx,
        to,
        cx.tcx.get_diagnostic_item(sym::From)?,
        &[cx.tcx.erase_and_anonymize_regions(from).into()],
    ) {
        named_type(cx, from, module, 8).map(|source_type| {
            format!("<{target_type} as ::core::convert::From<{source_type}>>::from(")
        })
    } else {
        None
    };
    // A local value has a fixed source type. Even when its only conversion
    // bound is Into, a qualified Into call can say exactly where it goes.
    let prefix = prefix.or_else(|| match receiver.kind {
        ExprKind::Path(ref path)
            if matches!(cx.qpath_res(path, receiver.hir_id), Res::Local(_)) =>
        {
            Some(format!(
                "<_ as ::core::convert::Into<{target_type}>>::into("
            ))
        }
        _ => None,
    })?;
    let start = call.span.with_hi(receiver.span.lo());
    let suffix = call.span.with_lo(receiver.span.hi());
    let map = cx.sess().source_map();
    if !map.span_to_snippet(start).ok()?.trim().is_empty()
        || map
            .span_to_snippet(suffix)
            .ok()?
            .split_whitespace()
            .collect::<String>()
            != ".into()"
    {
        return None;
    }
    let argument = argument_span(cx, receiver)?;
    Some(vec![
        (start.with_hi(argument.lo()), prefix),
        (suffix.with_lo(argument.hi()), ")".to_owned()),
    ])
}
