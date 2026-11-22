//! EXPLICIT051: a number has a width the source says.
//!
//! TigerStyle: use explicitly sized types like `u32` for everything, and avoid
//! the architecture-specific `usize`, because a type whose range depends on the
//! machine is a range nobody declared. EXPLICIT024 made the argument about two
//! values that are one thing and EXPLICIT045 about a conversion that names
//! neither end; this is the same complaint about a width, and 037 already
//! denies its sibling, a number laid out in the machine's own byte order.
//!
//! It was first written narrow -- only a field of a type that is serialized,
//! on the argument that a position in a list in memory is honest at the width
//! the list is measured in. That argument is the one TigerStyle refuses, and it
//! refuses it for a reason that holds here: the width a list is measured in is
//! the standard library's decision, and a quantity this tree means -- a tab, a
//! row, a count of pages -- is not made honest by being handed to a `get`.
//! The quantity is held at the width it has, and the one place it meets
//! `usize` is the conversion `console-core-number-conversion` already names.
//!
//! What is read is the type as it is written: a `usize` or an `isize` in a
//! signature, a field, a binding or a turbofish, and the standard library's
//! names for the same width. A `let` that holds one without writing it is read
//! too, because the fault is in holding one and a binding the compiler typed
//! holds it exactly as well as one somebody typed. A `usize` a `len()` hands to
//! a comparison or a `get` and nobody keeps is not asked about, because a
//! library handing one over is not this tree holding one. Nor is the binding
//! `console_core_number_conversion::index` or its range conversions hand back, which is the meeting the
//! help sends every other site to; and an array's length, which is a `usize`
//! the compiler keeps inside the type rather than a number anybody holds. A
//! const generic parameter is the same length given a name so a function can
//! be written once for several arrays, and the language will not take one of
//! any other width where an array's length is wanted, so it is left alone too. A
//! binding whose name begins with `_` is a value said to be thrown away, and a
//! number thrown away is not held at any width.
//!
//! Tests are read like everything else. A test that counts in `usize` is a test
//! of a quantity the code it tests no longer holds, and it was first exempt only
//! because every rule here began that way.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_abi;
extern crate rustc_lexer;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_errors;
extern crate rustc_span;

mod assignment;
mod bounded_ranges;
mod immediate_position;
mod fixes;
mod operands;
mod public_width;
mod stored_positions;
#[path = "../../shared/boundary_conversion.rs"]
mod boundary_conversion;
#[path = "../../shared/float_conversion.rs"]
mod float_conversion;

use clippy_utils::diagnostics::span_lint_and_then;
use rustc_ast::{IntTy, UintTy};
use rustc_hir::def::Res;
use rustc_hir::{AmbigArg, Expr, ExprKind, GenericParamKind, LetStmt, PrimTy, QPath, Ty, TyKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;

dylint_linting::impl_late_lint! {
    /// EXPLICIT051: a `usize` or `isize` is a range the machine decides and
    /// the source never declared. Say the width: `u32`, `u64`, `i64`.
    pub EXPLICIT051_NO_MACHINE_WIDTH,
    Deny,
    "a machine-width number; say the width",
    Explicit051NoMachineWidth {
        public_compatibility: dylint_linting::config_or_default("explicit051_public_compatibility"),
    }
}

pub struct Explicit051NoMachineWidth {
    public_compatibility: bool,
}

// The standard library's other spellings of the same width, which name the
// machine as surely as the primitive does.
const CONVERSION_CRATE: &str = boundary_conversion::CRATE;
const CONVERSIONS_TO_POSITION: [&str; 10] = ["index", "indices", "indices_from", "indices_to", "indices_inclusive", "indices_to_inclusive", "wrapped_index", "wrapped_offset", "toward_zero_usize", "toward_zero_isize"];

const POINTER_SIZED_TYPES: [&str; 4] = ["NonZeroUsize", "NonZeroIsize", "AtomicUsize", "AtomicIsize"];

fn written_pointer_sized(written_type: &Ty<'_>) -> Option<String> {
    let TyKind::Path(QPath::Resolved(_, path)) = written_type.kind else {
        return None;
    };

    match path.res {
        Res::PrimTy(PrimTy::Uint(UintTy::Usize)) => Some("usize".to_string()),
        Res::PrimTy(PrimTy::Int(IntTy::Isize)) => Some("isize".to_string()),
        _ => {
            let name = path.segments.last()?.ident.as_str().to_string();

            POINTER_SIZED_TYPES.contains(&name.as_str()).then_some(name)
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit051NoMachineWidth {
    fn check_ty(&mut self, context: &LateContext<'tcx>, written_type: &'tcx Ty<'tcx, AmbigArg>) {
        let written_type: &Ty<'tcx> = written_type.as_unambig_ty();

        if written_type.span.from_expansion() || is_const_parameter(context, written_type) {
            return;
        }

        let Some(width) = written_pointer_sized(written_type).or_else(|| public_width::internal_alias(context, written_type)) else {
            return;
        };

        span_lint_and_then(
            context,
            EXPLICIT051_NO_MACHINE_WIDTH,
            written_type.span,
            format!("`{width}` is as wide as the machine, and the source never says how wide that is"),
            |diagnostic| {
                if self.public_compatibility && let Some(replacement) = public_width::replacement(context, written_type, &width) {
                    diagnostic.span_suggestion(written_type.span, "retain the public interface's native width through the compatibility crate", replacement, rustc_errors::Applicability::MachineApplicable);
                } else if let Some(edits) = fixes::replacement(context, written_type, &width) {
                    if let [(span, edit)] = edits.as_slice() {
                        diagnostic.span_suggestion(*span, "use the conversion that names this machine-width boundary", edit.clone(), rustc_errors::Applicability::MachineApplicable);
                    } else {
                        diagnostic.multipart_suggestion("use the conversion that names this machine-width boundary", edits, rustc_errors::Applicability::MachineApplicable);
                    }
                } else {
                    diagnostic.help(HELP_TEXT);
                }
            },
        );
    }

    fn check_local(&mut self, context: &LateContext<'tcx>, local: &'tcx LetStmt<'tcx>) {
        if local.ty.is_some() || local.span.from_expansion() {
            return;
        }

        if local.init.is_some_and(|initializer| is_conversion_to_position(context, initializer)) {
            return;
        }

        let typeck_results = context.typeck_results();

        local.pat.each_binding(|_, binding, span, name| {
            if name.as_str().starts_with('_') {
                return;
            }

            let Some(width) = inferred_pointer_sized(context, typeck_results.node_type(binding)) else {
                return;
            };

            span_lint_and_then(
                context,
                EXPLICIT051_NO_MACHINE_WIDTH,
                span,
                format!("`{name}` holds a `{width}`, which is as wide as the machine, and the source never says how wide that is"),
                |diagnostic| {
                    if let Some((block, replacement)) = assignment::replacement(context, local) {
                        diagnostic.span_suggestion(
                            block,
                            "update the position through the named wrapping boundary",
                            replacement,
                            rustc_errors::Applicability::MachineApplicable,
                        );
                    } else if let Some(edits) = immediate_position::replacement(context, local) {
                        diagnostic.multipart_suggestion(
                            "pass the returned position directly to its consumer",
                            edits,
                            rustc_errors::Applicability::MachineApplicable,
                        );
                    } else if let Some(edits) = stored_positions::replacement(context, local) {
                        diagnostic.multipart_suggestion(
                            "hold this position at a fixed width and convert it where the API needs a native index",
                            edits,
                            rustc_errors::Applicability::MachineApplicable,
                        );
                    } else {
                        diagnostic.help(HELP_TEXT);
                    }
                },
            );
        });
    }
}

const HELP_TEXT: &str = "say the width: `u32` or `u64` for a count or a position, `i64` for a signed one. Where a \
     library hands a `usize` over or wants one back, meet it there with a named conversion \
     (`console_core_number_conversion::fitted`) and hold the quantity at the width it has";

fn inferred_pointer_sized<'tcx>(context: &LateContext<'tcx>, inferred: ty::Ty<'tcx>) -> Option<String> {
    let mut pending = vec![inferred];

    while let Some(component) = pending.pop() {
        match component.kind() {
            ty::Uint(ty::UintTy::Usize) => return Some("usize".to_string()),
            ty::Int(ty::IntTy::Isize) => return Some("isize".to_string()),
            ty::Adt(definition, arguments) => {
                let name = context.tcx.item_name(definition.did()).to_string();

                if POINTER_SIZED_TYPES.contains(&name.as_str()) {
                    return Some(name);
                }

                pending.extend(arguments.types());
            }
            ty::Array(element, _) | ty::Slice(element) | ty::Ref(_, element, _) | ty::RawPtr(element, _) => {
                pending.push(*element);
            }
            ty::Tuple(elements) => pending.extend(elements.iter()),
            _ => {}
        }
    }

    None
}

fn is_const_parameter(context: &LateContext<'_>, written_type: &Ty<'_>) -> bool {
    let Some(generics) = context.tcx.parent_hir_node(written_type.hir_id).generics() else {
        return false;
    };

    generics.params.iter().any(|parameter| match parameter.kind {
        GenericParamKind::Const { ty, .. } => ty.hir_id == written_type.hir_id,
        GenericParamKind::Lifetime { .. } | GenericParamKind::Type { .. } => false,
    })
}

fn is_conversion_to_position(context: &LateContext<'_>, initializer: &Expr<'_>) -> bool {
    let mut value = initializer;
    let is_checked = if let ExprKind::Match(branch, _, rustc_hir::MatchSource::TryDesugar(_)) = value.kind
        && let ExprKind::Call(_, [operand]) = branch.kind
    {
        value = operand;
        let types = context.typeck_results();
        while let ExprKind::MethodCall(method, receiver, [_], _) = value.kind
            && method.ident.as_str() == "map_err"
            && let Some(result) = types.expr_ty(receiver).ty_adt_def()
            && context.tcx.is_diagnostic_item(rustc_span::sym::Result, result.did())
            && let Some(id) = types.type_dependent_def_id(value.hir_id)
            && id.krate == result.did().krate
            && context.tcx.inherent_impl_of_assoc(id).is_some()
        {
            value = receiver;
        }
        true
    } else {
        false
    };
    let ExprKind::Call(callee, _) = value.kind else {
        return false;
    };

    let ExprKind::Path(ref path) = callee.kind else {
        return false;
    };

    let Res::Def(_, definition) = context.qpath_res(path, callee.hir_id) else {
        return false;
    };

    let name = context.tcx.item_name(definition);
    context.tcx.crate_name(definition.krate).as_str() == CONVERSION_CRATE
        && (CONVERSIONS_TO_POSITION.contains(&name.as_str())
            || is_checked && matches!(name.as_str(), "checked_index" | "checked_offset"))
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}

#[test]
fn public_widths() {
    dylint_testing::ui::Test::src_base(env!("CARGO_PKG_NAME"), "ui_compat")
        .dylint_toml("explicit051_public_compatibility = true")
        .run();
}
