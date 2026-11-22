//! Replace a loop's bounded indexing of fixed arrays with element iteration.

use std::collections::{BTreeMap, HashMap};

use clippy_utils::higher::{ForLoop, Range};
use clippy_utils::msrvs::Msrv;
use clippy_utils::sym;
use clippy_utils::usage::is_potentially_mutated;
use rustc_ast::RangeLimits;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BinOpKind, BindingMode, BorrowKind, Expr, ExprKind, HirId, Node, PatKind, QPath};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::hir::nested_filter::OnlyBodies;
use rustc_middle::ty::{
    self, TyCtxt,
    adjustment::{Adjust, DerefAdjustKind},
};
use rustc_span::Span;

use super::counted_position::Position;
use super::fixes::{
    clean, function_source, length_named_by, loop_length, position,
};

fn local(expr: &Expr<'_>) -> Option<HirId> {
    if let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind
        && let Res::Local(id) = path.res
    {
        return Some(id);
    }
    None
}

fn field_root(cx: &LateContext<'_>, mut place: &Expr<'_>) -> Option<HirId> {
    let types = cx.typeck_results();
    while let ExprKind::Field(base, _) = place.kind {
        if place.span.from_expansion()
            || !types.expr_adjustments(base).iter().all(|adjustment| {
                matches!(adjustment.kind, Adjust::Deref(DerefAdjustKind::Builtin))
            })
            || !types.expr_ty_adjusted(base).ty_adt_def().is_some_and(|adt| adt.is_struct())
        {
            return None;
        }
        place = base;
    }
    local(place)
}

fn same_place(left: &Expr<'_>, right: &Expr<'_>) -> bool {
    match (left.kind, right.kind) {
        (ExprKind::Field(left, a), ExprKind::Field(right, b)) => {
            a.name == b.name && same_place(left, right)
        }
        _ => local(left).is_some_and(|binding| local(right) == Some(binding)),
    }
}

struct Access<'tcx> {
    expr: &'tcx Expr<'tcx>,
    base: &'tcx Expr<'tcx>,
    shape: Shape,
    lane: u16,
    end: Option<u16>,
    counter_uses: usize,
    position_uses: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Element,
    Matrix,
    Flat(u16),
}

#[derive(Clone, Copy)]
enum Limit<'tcx> {
    Literal { start: u16, end: u16 },
    Parameter(DefId),
    Named(DefId, Span),
    Length(&'tcx Expr<'tcx>),
}

struct Uses<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    counter: HirId,
    paths: HashMap<HirId, usize>,
    accesses: HashMap<HirId, Vec<Access<'tcx>>>,
    position: Option<Position<'tcx>>,
    length_place: Option<&'tcx Expr<'tcx>>,
    has_unsafe: bool,
}

impl<'tcx> Visitor<'tcx> for Uses<'_, 'tcx> {
    type MaybeTyCtxt = TyCtxt<'tcx>;
    type NestedFilter = OnlyBodies;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.cx.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if let Some(binding) = local(expr) {
            *self.paths.entry(binding).or_default() += 1;
        }
        if let ExprKind::Index(base, index, _) = expr.kind
            && expr.hir_id.owner == self.counter.owner
            && !expr.span.from_expansion()
            && let Some((access, offset)) = self.project(expr, base, index)
            && let Some(binding) = local(access.base).or_else(|| {
                let length_place = self.length_place?;
                same_place(access.base, length_place)
                    .then(|| field_root(self.cx, access.base))
                    .flatten()
            })
        {
            let root = access.base;
            self.accesses.entry(binding).or_default().push(access);
            self.visit_expr(root);
            self.visit_expr(offset);
            return;
        }
        intravisit::walk_expr(self, expr);
    }

    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) {
        self.has_unsafe |= matches!(block.rules, rustc_hir::BlockCheckMode::UnsafeBlock(_))
            && !simd_access_block(self.cx, block);
        intravisit::walk_block(self, block);
    }
}

impl<'tcx> Uses<'_, 'tcx> {
    fn project(
        &self,
        expr: &'tcx Expr<'tcx>,
        base: &'tcx Expr<'tcx>,
        index: &'tcx Expr<'tcx>,
    ) -> Option<(Access<'tcx>, &'tcx Expr<'tcx>)> {
        let (base, offset, shape, lane, end, counter_uses, position_uses) =
            if let ExprKind::Index(outer, row, _) = base.kind
                && local(row) == Some(self.counter)
                && let Some(column) = position(index, 8)
            {
                (outer, row, Shape::Matrix, column, None, 1, 0)
            } else if local(index) == Some(self.counter) {
                (base, index, Shape::Element, 0, None, 1, 0)
            } else if let Some(range) = Range::hir(self.cx, index)
                && range.limits == RangeLimits::HalfOpen
            {
                let (stride, start, first_position) =
                    flat_index(self.cx, range.start?, self.counter, self.position.as_ref())?;
                let (end_stride, end, last_position) =
                    flat_index(self.cx, range.end?, self.counter, self.position.as_ref())?;
                if stride != end_stride || start >= end || end > stride {
                    return None;
                }
                let position_uses = first_position + last_position;
                (
                    base,
                    index,
                    Shape::Flat(stride),
                    start,
                    Some(end),
                    2 - position_uses,
                    position_uses,
                )
            } else {
                let (stride, lane, position_uses) =
                    flat_index(self.cx, index, self.counter, self.position.as_ref())?;
                if lane >= stride {
                    return None;
                }
                (
                    base,
                    index,
                    Shape::Flat(stride),
                    lane,
                    None,
                    1 - position_uses,
                    position_uses,
                )
            };
        Some((
            Access {
                expr,
                base,
                shape,
                lane,
                end,
                counter_uses,
                position_uses,
            },
            offset,
        ))
    }
}

fn simd_access_block(cx: &LateContext<'_>, block: &rustc_hir::Block<'_>) -> bool {
    // These macros take typed references and load or store immediately. Their
    // unchecked feature uses raw intrinsics only inside the macro definition.
    block
        .span
        .ctxt()
        .outer_expn_data()
        .macro_def_id
        .is_some_and(|id| {
            cx.tcx.crate_name(id.krate).as_str() == "console_core_video_decoding_av1"
                && matches!(
                    cx.tcx.def_path_str(id).as_str(),
                    "src::safe_simd::pixel_access::loadu_64"
                        | "src::safe_simd::pixel_access::loadu_128"
                        | "src::safe_simd::pixel_access::loadu_256"
                        | "src::safe_simd::pixel_access::loadu_512"
                        | "src::safe_simd::pixel_access::storeu_128"
                        | "src::safe_simd::pixel_access::storeu_256"
                        | "src::safe_simd::pixel_access::storeu_512"
                )
        })
}

fn operands<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &'tcx Expr<'tcx>,
    op: BinOpKind,
    method: &str,
) -> Option<(&'tcx Expr<'tcx>, &'tcx Expr<'tcx>)> {
    if expr.span.from_expansion() || !cx.typeck_results().expr_ty(expr).is_usize() {
        return None;
    }
    let (id, lhs, rhs) = match expr.kind {
        ExprKind::Binary(operator, lhs, rhs) if operator.node == op => {
            let types = cx.typeck_results();
            return (types.expr_ty(lhs).is_usize() && types.expr_ty(rhs).is_usize())
                .then_some((lhs, rhs));
        }
        ExprKind::Field(binary, field) if field.name.as_str() == "0" => {
            let ExprKind::Binary(operator, lhs, rhs) = binary.kind else {
                return None;
            };
            return (operator.node == op)
                .then_some((wrapping_operand(cx, lhs)?, wrapping_operand(cx, rhs)?));
        }
        ExprKind::MethodCall(_, lhs, [rhs], _) => (
            cx.typeck_results().type_dependent_def_id(expr.hir_id)?,
            lhs,
            rhs,
        ),
        ExprKind::Call(callee, [lhs, rhs]) => {
            let ExprKind::Path(ref path) = callee.kind else {
                return None;
            };
            (cx.qpath_res(path, callee.hir_id).opt_def_id()?, lhs, rhs)
        }
        _ => return None,
    };
    let implementation = cx.tcx.inherent_impl_of_assoc(id)?;
    (cx.tcx.item_name(id).as_str() == method
        && cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .skip_norm_wip()
            .is_usize())
    .then_some((lhs, rhs))
}

fn wrapping_operand<'tcx>(
    cx: &LateContext<'tcx>,
    wrapped: &'tcx Expr<'tcx>,
) -> Option<&'tcx Expr<'tcx>> {
    let ExprKind::Call(callee, [value]) = wrapped.kind else {
        return None;
    };
    let ExprKind::Path(path) = callee.kind else {
        return None;
    };
    let Res::Def(DefKind::Ctor(..), constructor) = cx.qpath_res(&path, callee.hir_id) else {
        return None;
    };
    (cx.tcx
        .is_diagnostic_item(sym::Wrapping, cx.tcx.parent(constructor))
        && cx.typeck_results().expr_ty(value).is_usize())
    .then_some(value)
}

fn flat_index<'tcx>(
    cx: &LateContext<'tcx>,
    index: &'tcx Expr<'tcx>,
    counter: HirId,
    saved: Option<&Position<'tcx>>,
) -> Option<(u16, u16, usize)> {
    let (offset, lane) =
        if let Some((offset, lane)) = operands(cx, index, BinOpKind::Add, "wrapping_add") {
            (offset, position(lane, 8)?)
        } else {
            (index, 0)
        };
    let original = saved.and_then(|position| position.original(cx, offset));
    let (left, right) = operands(
        cx,
        original.unwrap_or(offset),
        BinOpKind::Mul,
        "wrapping_mul",
    )?;
    let stride = if local(left) == Some(counter) {
        position(right, 8)?
    } else if local(right) == Some(counter) {
        position(left, 8)?
    } else {
        return None;
    };
    Some((stride, lane, usize::from(original.is_some())))
}

// Keep the replacement a place. Limit the parent contexts to ones where
// a copy, borrow, or write has the same meaning through an element reference.
pub(super) fn place(cx: &LateContext<'_>, access: &Expr<'_>) -> Option<(bool, bool)> {
    let ty = cx.typeck_results();
    let id = access.hir_id;
    let (write, parentheses) = match cx.tcx.parent_hir_node(id) {
        Node::Expr(parent) => match parent.kind {
            ExprKind::Assign(lhs, _, _) | ExprKind::AssignOp(_, lhs, _) => {
                (lhs.hir_id == id, false)
            }
            ExprKind::AddrOf(BorrowKind::Ref, mutability, _) => (mutability.is_mut(), false),
            ExprKind::Call(_, args) if args.iter().any(|arg| arg.hir_id == id) => (false, false),
            ExprKind::MethodCall(_, _, args, _) if args.iter().any(|arg| arg.hir_id == id) => {
                (false, false)
            }
            ExprKind::MethodCall(_, receiver, _, _) if receiver.hir_id == id => (false, true),
            ExprKind::Field(base, _) if base.hir_id == id => (place(cx, parent)?.0, true),
            ExprKind::Array(_)
            | ExprKind::Tup(_)
            | ExprKind::Cast(..)
            | ExprKind::Ret(_)
            | ExprKind::Binary(..)
            | ExprKind::Unary(..) => (false, false),
            _ => return None,
        },
        Node::LetStmt(statement)
            if matches!(
                statement.pat.kind,
                PatKind::Binding(BindingMode::NONE | BindingMode::MUT, _, _, None)
            ) =>
        {
            (false, false)
        }
        Node::ExprField(field) if field.expr.hir_id == id => (false, false),
        Node::Block(block) if block.expr.is_some_and(|tail| tail.hir_id == id) => (false, false),
        _ => return None,
    };
    let autoref = ty.expr_adjustments(access).iter().any(|adjustment| {
        matches!(
            adjustment.kind,
            ty::adjustment::Adjust::Borrow(ty::adjustment::AutoBorrow::Ref(
                ty::adjustment::AutoBorrowMutability::Mut { .. },
            ))
        )
    });
    // An explicit mutable reborrow can disable two-phase borrow checking.
    if autoref {
        return None;
    }
    Some((write, parentheses))
}

struct Iteration {
    pattern: String,
    iterator: String,
    changes: Vec<(Span, String)>,
    mutable: bool,
}

struct Enumerated {
    storage: HirId,
    length: u16,
    iterator: Span,
    item: Span,
}

fn enumerate<'tcx>(
    cx: &LateContext<'tcx>,
    expression: &'tcx Expr<'tcx>,
    item: Span,
    body: Span,
) -> Option<Enumerated> {
    let types = cx.typeck_results();
    let ExprKind::MethodCall(_, iterator, [], _) = expression.kind else {
        return None;
    };
    let method = types.type_dependent_def_id(expression.hir_id)?;
    let iterator_trait = cx.tcx.trait_of_assoc(method)?;
    if !cx.tcx.is_diagnostic_item(sym::Iterator, iterator_trait)
        || cx.tcx.item_name(method).as_str() != "enumerate"
    {
        return None;
    }
    let (method, mut storage) = match iterator.kind {
        ExprKind::MethodCall(_, storage, [], _) => {
            (types.type_dependent_def_id(iterator.hir_id)?, storage)
        }
        ExprKind::Call(callee, [storage]) => match types.expr_ty(callee).kind() {
            ty::FnDef(method, _) => (*method, storage),
            _ => return None,
        },
        _ => return None,
    };
    let implementation = cx.tcx.inherent_impl_of_assoc(method)?;
    if cx.tcx.crate_name(method.krate).as_str() != "core"
        || !matches!(cx.tcx.item_name(method).as_str(), "iter" | "iter_mut")
        || !cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .skip_norm_wip()
            .is_slice()
    {
        return None;
    }
    if let ExprKind::AddrOf(BorrowKind::Ref, _, base) = storage.kind {
        storage = base;
    }
    let ty::Array(_, length) = types.expr_ty(storage).peel_refs().kind() else {
        return None;
    };
    let length = length.try_to_target_usize(cx.tcx)?;
    if length == 0 || !loop_length(cx, storage, 0, length, body) {
        return None;
    }
    Some(Enumerated {
        storage: local(storage)?,
        length: u16::try_from(length).ok()?,
        iterator: iterator.span,
        item,
    })
}

fn limit<'tcx>(cx: &LateContext<'tcx>, range: &Range<'tcx>) -> Option<Limit<'tcx>> {
    let start = position(range.start?, 8)?;
    let end = range.end?;
    if let Some(height) = position(end, 8) {
        let end = match range.limits {
            RangeLimits::HalfOpen => height,
            RangeLimits::Closed => height.checked_add(1)?,
        };
        return (start < end).then_some(Limit::Literal { start, end });
    }
    if start != 0 || range.limits != RangeLimits::HalfOpen {
        return None;
    }
    match end.kind {
        ExprKind::Path(path) => match cx.qpath_res(&path, end.hir_id) {
            Res::Def(DefKind::ConstParam, id) => Some(Limit::Parameter(id)),
            Res::Def(
                DefKind::Const {
                    is_type_const: false,
                },
                id,
            ) if !end.span.from_expansion() => Some(Limit::Named(id, end.span)),
            _ => None,
        },
        ExprKind::MethodCall(_, receiver, [], _) => {
            let method = cx.typeck_results().type_dependent_def_id(end.hir_id)?;
            if cx.tcx.item_name(method).as_str() != "len"
                || !matches!(cx.tcx.crate_name(method.krate).as_str(), "core" | "alloc")
                || cx.tcx.inherent_impl_of_assoc(method).is_none()
            {
                return None;
            }
            field_root(cx, receiver)?;
            Some(Limit::Length(receiver))
        }
        _ => None,
    }
}

fn symbolic_element<'tcx>(
    cx: &LateContext<'tcx>,
    access: &Access<'tcx>,
    limit: Limit<'tcx>,
) -> Option<ty::Ty<'tcx>> {
    if access.shape != Shape::Element {
        return None;
    }
    let base = cx.typeck_results().expr_ty(access.base).peel_refs();
    match (limit, base.kind()) {
        (Limit::Length(storage), kind) if same_place(access.base, storage) => match kind {
            ty::Slice(item) | ty::Array(item, _) => Some(*item),
            ty::Adt(adt, args) if cx.tcx.is_diagnostic_item(sym::Vec, adt.did()) => {
                Some(args.type_at(0))
            }
            _ => None,
        },
        (Limit::Parameter(id), ty::Array(item, length)) => {
            let ty::ConstKind::Param(parameter) = length.kind() else {
                return None;
            };
            let owner = cx.tcx.hir_enclosing_body_owner(access.base.hir_id);
            let offset = usize::try_from(parameter.index).ok()?;
            (cx.tcx.generics_of(owner).param_at(offset, cx.tcx).def_id == id).then_some(*item)
        }
        (Limit::Named(id, span), ty::Array(item, _)) => {
            length_named_by(cx, access.base, id, span)?;
            Some(*item)
        }
        _ => None,
    }
}

fn array<'tcx>(
    cx: &LateContext<'tcx>,
    accesses: &[Access<'tcx>],
    limit: Limit<'tcx>,
    source: &str,
    body: Span,
) -> Option<Iteration> {
    let first = accesses.first()?;
    let base_ty = cx.typeck_results().expr_ty(first.base);
    let (element, width, take) = if let Limit::Literal { end, .. } = limit {
        let ty::Array(item, length) = base_ty.peel_refs().kind() else {
            return None;
        };
        let length = length.try_to_target_usize(cx.tcx)?;
        let (element, width, rows) = match first.shape {
            Shape::Element => (*item, 1, length),
            Shape::Matrix => {
                let ty::Array(element, width) = item.kind() else {
                    return None;
                };
                let width = width.try_to_target_usize(cx.tcx)?;
                if !loop_length(cx, first.base, 1, width, body) {
                    return None;
                }
                (*element, u16::try_from(width).ok()?, length)
            }
            Shape::Flat(width) => {
                let mut msrv = Msrv::default();
                msrv.read_cargo(cx.sess());
                if !msrv.meets(
                    cx,
                    rustc_hir::RustcVersion {
                        major: 1,
                        minor: 88,
                        patch: 0,
                    },
                ) {
                    return None;
                }
                (*item, width, length.checked_div(u64::from(width))?)
            }
        };
        if !(1..=32).contains(&width)
            || rows < u64::from(end)
            || !loop_length(cx, first.base, 0, length, body)
            || (matches!(first.shape, Shape::Flat(_)) && end.checked_mul(width).is_none())
        {
            return None;
        }
        (element, width, (rows != u64::from(end)).then_some(end))
    } else {
        (symbolic_element(cx, first, limit)?, 1, None)
    };
    let length_bounded = matches!(limit, Limit::Length(storage) if same_place(first.base, storage));
    if !length_bounded && !cx.type_is_copy_modulo_regions(element) {
        return None;
    }
    let base = clean(cx, first.base.span)?;
    let stem = match first.base.kind {
        ExprKind::Field(_, name) if name.as_str().starts_with(|c: char| c.is_ascii_digit()) => {
            format!("field_{}", name.as_str())
        }
        ExprKind::Field(_, name) => name.as_str().to_owned(),
        _ => base.trim_start_matches("r#").to_owned(),
    };

    let mut changes = Vec::new();
    let mut names = BTreeMap::new();
    let has_window = accesses.iter().any(|access| access.end.is_some());
    let row = format!("{stem}_row");
    if has_window && source.contains(&row) {
        return None;
    }
    let mut mutable = false;
    for Access {
        expr,
        base,
        shape,
        lane,
        end,
        ..
    } in accesses
    {
        if *shape != first.shape || *lane >= width
            || (length_bounded && !same_place(first.base, base))
        {
            return None;
        }
        clean(cx, expr.span)?;
        let (write, parentheses) = place(cx, expr)?;
        mutable |= write;
        let field = matches!(cx.tcx.parent_hir_node(expr.hir_id), Node::Expr(parent)
            if matches!(parent.kind, ExprKind::Field(base, _) if base.hir_id == expr.hir_id));
        let replacement = if has_window {
            row_projection(&row, *lane, *end, width, write)?
        } else {
            let name = names.entry(*lane).or_insert_with(|| {
                if *shape == Shape::Element {
                    format!("{stem}_element")
                } else {
                    format!("{stem}_lane_{lane}")
                }
            });
            if source.contains(name.as_str()) {
                return None;
            }
            if field {
                name.clone()
            } else {
                format!("*{name}")
            }
        };
        let replacement = if parentheses && (!field || has_window) {
            format!("({replacement})")
        } else {
            replacement
        };
        changes.push((expr.span, replacement));
    }

    let (borrow, iteration, chunks) = if mutable {
        ("&mut ", "iter_mut", "as_chunks_mut")
    } else {
        ("&", "iter", "as_chunks")
    };
    let storage = if base_ty.is_ref() {
        base
    } else {
        format!("{borrow}{base}")
    };
    let storage = if let Shape::Flat(_) = first.shape {
        format!("<[_]>::{chunks}::<{width}>({storage}).0")
    } else {
        storage
    };
    let iterator = format!("<[_]>::{iteration}({storage})");
    let iterator = match take {
        Some(end) => format!("{iterator}.take({end})"),
        None => iterator,
    };
    let pattern = if has_window {
        row
    } else if first.shape == Shape::Element {
        names.remove(&0)?
    } else {
        let lanes: Vec<_> = (0..width)
            .map(|lane| names.get(&lane).map_or("_", String::as_str))
            .collect();
        format!("[{}]", lanes.join(", "))
    };
    Some(Iteration {
        pattern,
        iterator,
        changes,
        mutable,
    })
}

fn row_projection(
    row: &str,
    start: u16,
    end: Option<u16>,
    width: u16,
    write: bool,
) -> Option<String> {
    let mutability = if write { "mut " } else { "" };
    let prefix = "_, ".repeat(usize::from(start));
    let (pattern, value) = match end {
        Some(end) => {
            let suffix = ", _".repeat(usize::from(width.checked_sub(end)?));
            let pattern = format!("[{prefix}_window @ ..{suffix}]");
            let value = format!("let _slice: &{mutability}[_] = _window; _slice");
            (pattern, value)
        }
        None => (format!("[{prefix}_window, ..]"), "_window".to_owned()),
    };
    Some(format!(
        "*{{ let {pattern} = &{mutability}*{row}; {value} }}"
    ))
}

pub(super) fn edits<'tcx>(
    cx: &LateContext<'tcx>,
    walk: &ForLoop<'tcx>,
    round_trip_loops: bool,
) -> Option<(HirId, Vec<(Span, String)>)> {
    if !cx.sess().edition().at_least_rust_2018() {
        return None;
    }
    let (counter, limit, enumerated, header) = if let Some(range) = Range::hir(cx, walk.arg) {
        let PatKind::Binding(BindingMode::NONE, counter, _, None) = walk.pat.kind else {
            return None;
        };
        (
            counter,
            limit(cx, &range)?,
            None,
            walk.pat.span.with_hi(range.span.hi()),
        )
    } else {
        let PatKind::Tuple([index, item], middle) = walk.pat.kind else {
            return None;
        };
        let PatKind::Binding(BindingMode::NONE, counter, _, None) = index.kind else {
            return None;
        };
        if middle.as_opt_usize().is_some() {
            return None;
        }
        let enumerated = enumerate(cx, walk.arg, item.span, walk.body.span)?;
        (
            counter,
            Limit::Literal {
                start: 0,
                end: enumerated.length,
            },
            Some(enumerated),
            walk.pat.span.with_hi(walk.arg.span.hi()),
        )
    };
    clean(cx, header)?;
    let source = function_source(cx, walk.body)?;
    if source.contains("unsafe") {
        return None;
    }
    // Inactive bindings and uses of a struct can change what its field names
    // or which parts of it are borrowed while the iterator is alive.
    let length_place = match limit {
        Limit::Length(storage) if matches!(storage.kind, ExprKind::Field(..)) => {
            if source.contains('#') {
                return None;
            }
            Some(storage)
        }
        _ => None,
    };
    let mut uses = Uses {
        cx,
        counter,
        paths: HashMap::new(),
        accesses: HashMap::new(),
        position: if round_trip_loops && !source.contains('#') {
            Position::new(cx, walk.body)
        } else {
            None
        },
        length_place,
        has_unsafe: false,
    };
    uses.visit_expr(walk.body);
    if uses.has_unsafe {
        return None;
    }

    let mut changes = Vec::new();
    let mut arrays = Vec::new();
    let mut first = None;
    let mut consumed = 0;
    let mut consumed_positions = 0;
    let mut groups: Vec<_> = uses.accesses.iter().collect();
    groups.sort_by_key(|(_, accesses)| accesses.first().map(|access| access.expr.span.lo()));
    for (binding, accesses) in groups {
        if enumerated
            .as_ref()
            .is_some_and(|input| input.storage == *binding)
        {
            continue;
        }
        let declaration = cx.tcx.hir_span(*binding);
        if declaration.from_expansion() || declaration.hi() > header.lo() {
            continue;
        }
        if let Some(iteration) = array(cx, accesses, limit, &source, walk.body.span) {
            if uses.paths.get(binding) != Some(&accesses.len()) {
                let base = cx.typeck_results().expr_ty(accesses.first()?.base);
                if length_place.is_some()
                    || iteration.mutable
                    || !cx.type_is_copy_modulo_regions(base)
                    || is_potentially_mutated(*binding, walk.body, cx)
                {
                    continue;
                }
            }
            first = first.or_else(|| accesses.first().map(|access| access.expr.hir_id));
            arrays.push((iteration.pattern, iteration.iterator));
            changes.extend(iteration.changes);
            consumed += accesses
                .iter()
                .map(|access| access.counter_uses)
                .sum::<usize>();
            consumed_positions += accesses
                .iter()
                .map(|access| access.position_uses)
                .sum::<usize>();
        }
    }
    if arrays.is_empty() {
        return None;
    }
    if consumed_positions != 0
        && let Some(position) = &uses.position
        && uses.paths.get(&position.binding) == Some(&consumed_positions)
    {
        changes.push((position.standalone_statement(cx)?, String::new()));
        consumed += 1;
    }
    let consumed_counter = uses.paths.get(&counter) == Some(&consumed);
    let mut arrays = arrays.into_iter();
    let (mut pattern, mut iterator) = if let Some(input) = &enumerated {
        if consumed_counter {
            (clean(cx, input.item)?, clean(cx, input.iterator)?)
        } else {
            (clean(cx, walk.pat.span)?, clean(cx, walk.arg.span)?)
        }
    } else {
        arrays.next()?
    };
    for (element, next) in arrays {
        pattern = format!("({pattern}, {element})");
        iterator.push_str(&format!(".zip({next})"));
    }
    if enumerated.is_none() && !consumed_counter {
        let counter = clean(cx, walk.pat.span)?;
        pattern = format!("({counter}, {pattern})");
        iterator.push_str(".enumerate()");
    }
    if let Limit::Literal { start, .. } = limit
        && start != 0
    {
        iterator.push_str(&format!(".skip({start})"));
    }
    changes.push((header, format!("{pattern} in {iterator}")));
    Some((first?, changes))
}
