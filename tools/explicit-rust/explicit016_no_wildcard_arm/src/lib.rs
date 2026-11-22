#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::{is_refutable, peel_hir_pat_refs, sym};
use rustc_hir::def::CtorKind;
use rustc_hir::def_id::DefId;
use rustc_hir::{
    Arm, BindingMode, Expr, ExprKind, HirId, ItemKind, MatchSource, Node, Pat, PatExpr,
    PatExprKind, PatKind, QPath, TyKind,
};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, AdtDef};
use rustc_span::Span;
use std::collections::HashMap;

#[path = "../../shared/macro_input.rs"]
mod macro_input;

dylint_linting::declare_late_lint! {
    /// EXPLICIT016: a `match` over an enum may not have a wildcard arm. `_ =>`
    /// is a decision made by omission -- add a variant next year and every
    /// catch-all votes on it without the compiler asking. Named variants are
    /// how a new variant becomes a build error at every site that has an
    /// opinion about it.
    ///
    /// A foreign `#[non_exhaustive]` enum was exempt here once, because the
    /// compiler demands a catch-all over one and a rule cannot ask for what
    /// will not compile. It asks for something else instead: do not match on
    /// that enum. The question a catch-all over `io::ErrorKind` is really
    /// asking is whether the kind is one particular kind, and that is an
    /// answer with two named outcomes -- `match bool::then_some(fault.kind() == NotFound, ())
    /// { Some(()) => …, None => … }`. What is left unwritable is a wide reading
    /// of somebody else's open enum, which is the thing that quietly votes on
    /// a variant added next year.
    ///
    /// A guarded arm was left alone here once, on the argument that the
    /// unguarded arm it falls through to answers for the rest. EXPLICIT019 now
    /// denies the guard itself, so there is nothing left to exempt.
    pub EXPLICIT016_NO_WILDCARD_ARM,
    Deny,
    "a wildcard arm on an enum decides future variants by omission; name every variant"
}

fn error_kind_query<'tcx>(
    cx: &LateContext<'tcx>,
    expression: &Expr<'tcx>,
    scrutinee: &Expr<'tcx>,
    adt: AdtDef<'tcx>,
    arms: &[Arm<'tcx>],
) -> Option<Vec<(Span, String)>> {
    let [wanted, fallback] = arms else {
        return None;
    };
    let definition = adt.did();
    if cx.tcx.crate_name(definition.krate).as_str() != "core"
        || cx.tcx.def_path_str(definition) != "std::io::ErrorKind"
        || !matches!(
            scrutinee.kind,
            ExprKind::MethodCall(..)
                | ExprKind::Call(..)
                | ExprKind::Path(..)
                | ExprKind::Field(..)
        )
        || !cx.sess().edition().at_least_rust_2018()
        || cx.typeck_results().expr_ty(scrutinee).is_ref()
        || wanted.guard.is_some()
        || fallback.guard.is_some()
    {
        return None;
    }
    let PatKind::Expr(PatExpr {
        kind: PatExprKind::Path(path),
        hir_id,
        ..
    }) = wanted.pat.kind
    else {
        return None;
    };
    let kind = cx.qpath_res(path, *hir_id).opt_def_id()?;
    if !adt
        .variants()
        .iter()
        .any(|variant| variant.def_id == kind || variant.ctor_def_id() == Some(kind))
    {
        return None;
    }
    let map = cx.sess().source_map();
    if map.span_to_snippet(expression.span).ok()?.contains('#') {
        return None;
    }
    match fallback.pat.kind {
        PatKind::Wild => {}
        PatKind::Binding(BindingMode::NONE, binding, ident, None)
            if !clippy_utils::visitors::is_local_used(cx, fallback.body, binding)
                && !map
                    .span_to_snippet(fallback.body.span)
                    .ok()?
                    .contains(ident.as_str()) => {}
        _ => return None,
    }
    let written = |span: Span| {
        if span.from_expansion() {
            return None;
        }
        let text = map.span_to_snippet(span).ok()?;
        (!text.contains("/*") && !text.contains("//")).then_some(text)
    };
    let value = written(scrutinee.span)?;
    let wanted_variant = written(wanted.pat.span)?;
    written(fallback.pat.span)?;
    Some(vec![
        (
            scrutinee.span,
            format!("::core::primitive::bool::then_some({value} == {wanted_variant}, ())"),
        ),
        (wanted.pat.span, "::core::option::Option::Some(())".into()),
        (fallback.pat.span, "::core::option::Option::None".into()),
    ])
}

// A pattern that matches every variant without naming one: `_`, a bare
// binding, or an or-pattern with either inside it.
fn swallows_every_variant(pat: &Pat<'_>) -> bool {
    match pat.kind {
        PatKind::Wild => true,
        PatKind::Binding(_, _, _, None) => true,
        PatKind::Or(alternatives) => alternatives.iter().any(swallows_every_variant),
        _ => false,
    }
}

enum Coverage<'tcx> {
    WholeVariant,
    OneScalarLiteral,
    UnitPayload {
        adt: AdtDef<'tcx>,
        covered: Vec<DefId>,
    },
    Partial,
}

impl Coverage<'_> {
    fn merge(&mut self, other: Self) {
        match (self, other) {
            (
                Self::UnitPayload { adt, covered },
                Self::UnitPayload {
                    adt: other_adt,
                    covered: additional,
                },
            ) if *adt == other_adt => covered.extend(additional),
            (coverage, _) => *coverage = Self::Partial,
        }
    }
}

fn closed_unit_prefix(cx: &LateContext<'_>, adt: AdtDef<'_>) -> Option<&'static str> {
    let definition = adt.did();
    if cx.tcx.crate_name(definition.krate).as_str() != "core"
        || adt.is_variant_list_non_exhaustive()
        || adt
            .variants()
            .iter()
            .any(|variant| variant.ctor_kind() != Some(CtorKind::Const))
    {
        return None;
    }
    match cx.tcx.def_path_str(definition).as_str() {
        "core::cmp::Ordering" | "std::cmp::Ordering" => Some("::core::cmp::Ordering"),
        "core::num::FpCategory" | "std::num::FpCategory" => Some("::core::num::FpCategory"),
        _ => None,
    }
}

fn absolute_variant(cx: &LateContext<'_>, path: &QPath<'_>, prefix: &str, variant: &str) -> bool {
    let QPath::Resolved(None, path) = path else {
        return false;
    };
    cx.sess()
        .source_map()
        .span_to_snippet(path.span)
        .is_ok_and(|source| source == format!("{prefix}::{variant}"))
}

fn stable_core_prelude(cx: &LateContext<'_>) -> Option<()> {
    let root = cx.tcx.hir_root_module();
    if cx.sess().opts.externs.get("core").is_some()
        || root.item_ids.iter().any(|&id| {
            matches!(cx.tcx.hir_item(id).kind, ItemKind::ExternCrate(Some(original), alias)
                if alias.name == sym::core && original != sym::core)
        })
    {
        return None;
    }
    let source = cx
        .sess()
        .source_map()
        .span_to_snippet(root.spans.inner_span)
        .ok()?;
    // The written root also contains extern aliases disabled in this build.
    let mut tokens = tokenize(&source, FrontmatterAllowed::No)
        .scan(0, |offset, token| {
            let start = *offset;
            *offset += token.len as usize;
            Some((token.kind, &source[start..*offset]))
        })
        .filter_map(|(kind, text)| {
            (!matches!(
                kind,
                TokenKind::Whitespace
                    | TokenKind::LineComment { .. }
                    | TokenKind::BlockComment { .. }
            ))
            .then_some(text)
        });
    while tokens.by_ref().any(|token| token == "extern") {
        if tokens.next() == Some("crate")
            && tokens.next().is_some_and(|original| original != "core")
            && tokens.next() == Some("as")
            && matches!(tokens.next(), Some("core" | "r#core"))
        {
            return None;
        }
    }
    Some(())
}

fn unit_payload<'tcx>(cx: &LateContext<'tcx>, field: &Pat<'tcx>) -> Option<Coverage<'tcx>> {
    let ty::Adt(adt, _) = cx.typeck_results().pat_ty(field).peel_refs().kind() else {
        return None;
    };
    let prefix = closed_unit_prefix(cx, *adt)?;
    let mut patterns = Vec::new();
    collect_patterns(field, &mut patterns);
    let covered = patterns
        .into_iter()
        .map(|pat| {
            if pat.span.from_expansion() {
                return None;
            }
            let PatKind::Expr(PatExpr {
                kind: PatExprKind::Path(path),
                hir_id,
                ..
            }) = pat.kind
            else {
                return None;
            };
            let id = cx.qpath_res(path, *hir_id).opt_def_id()?;
            adt.variants()
                .iter()
                .find(|variant| {
                    variant.ctor_def_id() == Some(id)
                        && absolute_variant(cx, path, prefix, variant.name.as_str())
                })
                .map(|variant| variant.def_id)
        })
        .collect::<Option<_>>()?;
    Some(Coverage::UnitPayload { adt: *adt, covered })
}

fn field_coverage<'pat, 'tcx: 'pat>(
    cx: &LateContext<'tcx>,
    fields: impl Iterator<Item = &'pat Pat<'tcx>>,
) -> Coverage<'tcx> {
    let mut coverage = Coverage::WholeVariant;
    for field in fields {
        if !is_refutable(cx, field) {
            continue;
        }
        if matches!(
            field.kind,
            PatKind::Expr(PatExpr {
                kind: PatExprKind::Lit { .. },
                ..
            })
        ) {
            let ty = cx.typeck_results().node_type(field.hir_id);
            if ty.is_integral() || ty.is_bool() || ty.is_char() {
                return Coverage::OneScalarLiteral;
            }
        }
        coverage = Coverage::Partial;
    }
    coverage
}

fn variant_path<'pat, 'tcx>(
    cx: &LateContext<'tcx>,
    pat: &'pat Pat<'tcx>,
) -> Option<(&'pat QPath<'tcx>, HirId, Coverage<'tcx>)> {
    match pat.kind {
        PatKind::TupleStruct(ref path, [field], rest) if rest.as_opt_usize().is_none() => Some((
            path,
            pat.hir_id,
            unit_payload(cx, field).unwrap_or_else(|| field_coverage(cx, [field].into_iter())),
        )),
        PatKind::TupleStruct(ref path, fields, _) => {
            Some((path, pat.hir_id, field_coverage(cx, fields.iter())))
        }
        PatKind::Struct(ref path, fields, _) => Some((
            path,
            pat.hir_id,
            field_coverage(cx, fields.iter().map(|field| field.pat)),
        )),
        PatKind::Expr(PatExpr {
            kind: PatExprKind::Path(path),
            hir_id,
            ..
        }) => Some((path, *hir_id, Coverage::WholeVariant)),
        _ => None,
    }
}

fn collect_patterns<'hir>(pattern: &'hir Pat<'hir>, alternatives: &mut Vec<&'hir Pat<'hir>>) {
    let (pattern, _) = peel_hir_pat_refs(pattern);
    match pattern.kind {
        PatKind::Or(patterns) => {
            for pattern in patterns {
                collect_patterns(pattern, alternatives);
            }
        }
        PatKind::Binding(_, _, _, Some(inner)) => collect_patterns(inner, alternatives),
        PatKind::Never => {}
        _ => alternatives.push(pattern),
    }
}

fn enum_prefix(cx: &LateContext<'_>, path: &QPath<'_>) -> Option<String> {
    let map = cx.sess().source_map();
    match path {
        QPath::Resolved(None, path) if path.segments.len() > 1 => {
            let last = path.segments.last()?;
            let text = map
                .span_to_snippet(path.span.with_hi(last.ident.span.lo()))
                .ok()?;
            let prefix = text.trim_end().strip_suffix("::")?;
            (!prefix.is_empty() && !prefix.contains(['<', '/', '\n'])).then(|| prefix.to_owned())
        }
        QPath::TypeRelative(ty, _) if matches!(ty.kind, TyKind::Path(QPath::Resolved(None, _))) => {
            let prefix = map.span_to_snippet(ty.span).ok()?;
            (!prefix.contains(['<', '/', '\n'])).then_some(prefix)
        }
        _ => None,
    }
}

fn stable_local_enum(cx: &LateContext<'_>, adt: AdtDef<'_>) -> Option<()> {
    let Node::Item(item) = cx.tcx.hir_node_by_def_id(adt.did().as_local()?) else {
        return None;
    };
    if item.span.from_expansion()
        || adt
            .variants()
            .iter()
            .any(|variant| cx.tcx.def_span(variant.def_id).from_expansion())
    {
        return None;
    }
    let source = cx.sess().source_map().span_to_snippet(item.span).ok()?;
    // Conditional variants cannot be enumerated from one compilation.
    (!source.contains('#')).then_some(())
}

fn has_attribute(source: &str) -> bool {
    tokenize(source, FrontmatterAllowed::No).any(|token| token.kind == TokenKind::Pound)
}

fn stable_match_arms(cx: &LateContext<'_>, expression: &Expr<'_>, arms: &[Arm<'_>]) -> Option<()> {
    let map = cx.sess().source_map();
    let mut start = expression.span.lo();
    for arm in arms {
        let body = arm.body.span.source_callsite();
        if arm.pat.span.from_expansion() || !expression.span.contains(body) || body.lo() < start {
            return None;
        }
        // An inactive arm has no HIR body span, so its attribute remains in a gap.
        let header = expression.span.with_lo(start).with_hi(body.lo());
        if has_attribute(&map.span_to_snippet(header).ok()?) {
            return None;
        }
        start = body.hi();
    }
    let tail = expression.span.with_lo(start);
    (!has_attribute(&map.span_to_snippet(tail).ok()?)).then_some(())
}

fn name_fallback<'tcx>(
    cx: &LateContext<'tcx>,
    expression: &Expr<'_>,
    adt: AdtDef<'tcx>,
    arms: &[Arm<'tcx>],
) -> Option<String> {
    let (fallback, earlier) = arms.split_last()?;
    if fallback.guard.is_some() || !cx.sess().edition().at_least_rust_2018() {
        return None;
    }
    let core_unit_prefix = closed_unit_prefix(cx, adt);
    let mut prefix = match cx.tcx.get_diagnostic_name(adt.did()) {
        Some(sym::Option) => Some("::core::option::Option".to_owned()),
        Some(sym::Result) => Some("::core::result::Result".to_owned()),
        _ if let Some(prefix) = core_unit_prefix => Some(prefix.to_owned()),
        _ => {
            stable_local_enum(cx, adt)?;
            None
        }
    };
    stable_match_arms(cx, expression, arms)?;
    let core_enum = prefix.is_some();
    let mut absolute_variants = true;
    let mut any_absolute_variant = false;
    let binding = match fallback.pat.kind {
        PatKind::Wild => None,
        PatKind::Binding(_, _, ident, None) => {
            let map = cx.sess().source_map();
            let span = fallback.pat.span;
            let source = map.span_to_snippet(span).ok()?;
            let name = map.span_to_snippet(ident.span).ok()?;
            // A plain binding can keep the entire scrutinee with `@`.
            if source != name {
                return None;
            }
            Some(name)
        }
        _ => return None,
    };
    let mut missing: Vec<_> = adt.variants().iter().collect();
    let mut partial_coverage: HashMap<_, Coverage<'_>> = HashMap::new();
    for arm in earlier {
        let mut alternatives = Vec::new();
        collect_patterns(arm.pat, &mut alternatives);
        for pat in alternatives {
            if pat.span.from_expansion() {
                return None;
            }
            let Some((path, variant, coverage)) =
                variant_path(cx, pat).and_then(|(path, id, coverage)| {
                    let id = cx.qpath_res(path, id).opt_def_id()?;
                    let variant = adt.variants().iter().find(|variant| {
                        variant.def_id == id || variant.ctor_def_id() == Some(id)
                    })?;
                    Some((path, variant, coverage))
                })
            else {
                arm.guard?;
                continue;
            };
            if prefix.is_none() {
                prefix = enum_prefix(cx, path);
            }
            if core_enum {
                let absolute = prefix.as_deref().is_some_and(|prefix| {
                    absolute_variant(cx, path, prefix, variant.name.as_str())
                });
                absolute_variants &= absolute;
                any_absolute_variant |= absolute;
            }
            if arm.guard.is_some() {
                continue;
            }
            if matches!(coverage, Coverage::WholeVariant) {
                missing.retain(|missing| missing.def_id != variant.def_id);
            } else if let Some(current) = partial_coverage.get_mut(&variant.def_id) {
                current.merge(coverage);
            } else {
                partial_coverage.insert(variant.def_id, coverage);
            }
        }
    }
    // Several partial patterns can cover a variant together. A single
    // scalar literal leaves every other value of that field available.
    if missing.iter().any(|variant| {
        matches!(
            partial_coverage.get(&variant.def_id),
            Some(Coverage::Partial)
        )
    }) {
        return None;
    }
    // A very wide enum is usually better served by a domain-specific query.
    if missing.is_empty() || missing.len() > 16 {
        return None;
    }
    // A cfg-selected import can resolve to a different variant on another build.
    // Require source paths that fix the identity of every earlier variant for
    // complements that depend on the members of a closed standard enum.
    if core_unit_prefix.is_some()
        || missing.iter().any(|variant| {
            matches!(
                partial_coverage.get(&variant.def_id),
                Some(Coverage::UnitPayload { .. })
            )
        })
    {
        if !absolute_variants || !any_absolute_variant {
            return None;
        }
        stable_core_prelude(cx)?;
    }
    let prefix = prefix?;
    let mut alternatives = Vec::new();
    for variant in missing {
        let name = variant.name;
        if let Some(Coverage::UnitPayload { adt, covered }) = partial_coverage.get(&variant.def_id)
        {
            let payload_prefix = closed_unit_prefix(cx, *adt)?;
            let remainder = adt
                .variants()
                .iter()
                .filter(|candidate| !covered.contains(&candidate.def_id))
                .map(|candidate| format!("{payload_prefix}::{}", candidate.name))
                .collect::<Vec<_>>();
            if !remainder.is_empty() {
                alternatives.push(format!("{prefix}::{name}({})", remainder.join(" | ")));
            }
            continue;
        }
        let fields = match variant.ctor_kind() {
            Some(CtorKind::Const) => "",
            Some(CtorKind::Fn) if variant.fields.len() == 1 => "(_)",
            Some(CtorKind::Fn) => "(..)",
            None => " { .. }",
        };
        alternatives.push(format!("{prefix}::{name}{fields}"));
    }
    if alternatives.is_empty() {
        return None;
    }
    let alternatives = alternatives.join(" | ");
    match binding {
        Some(binding) => Some(format!("{binding} @ ({alternatives})")),
        None => Some(alternatives),
    }
}

impl<'tcx> LateLintPass<'tcx> for Explicit016NoWildcardArm {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        let ExprKind::Match(scrutinee, arms, MatchSource::Normal) = expr.kind else {
            return;
        };

        if expr.span.from_expansion() {
            return;
        }

        let scrutinee_ty = cx.typeck_results().expr_ty(scrutinee).peel_refs();

        let rustc_middle::ty::Adt(adt, _) = scrutinee_ty.kind() else {
            return;
        };

        if !adt.is_enum() {
            return;
        }

        for (position, arm) in arms.iter().enumerate() {
            if swallows_every_variant(arm.pat) {
                span_lint_and_then(
                    cx,
                    EXPLICIT016_NO_WILDCARD_ARM,
                    arm.pat.span,
                    "this arm matches every variant of the enum without naming one",
                    |diagnostic| {
                        if position == 1
                            && let Some(edits) = error_kind_query(cx, expr, scrutinee, *adt, arms)
                            && macro_input::outside_macro_input(cx, expr.span)
                        {
                            diagnostic.multipart_suggestion(
                                "ask whether the I/O error has the selected kind",
                                edits,
                                rustc_errors::Applicability::MachineApplicable,
                            );
                        } else if position + 1 == arms.len()
                            && let Some(pattern) = name_fallback(cx, expr, *adt, arms)
                            && macro_input::outside_macro_input(cx, expr.span)
                        {
                            diagnostic.span_suggestion(
                                arm.pat.span,
                                "name the remaining variants",
                                pattern,
                                rustc_errors::Applicability::MachineApplicable,
                            );
                        } else {
                            diagnostic.help("name every variant, so a variant added later is a compile error here rather than a silent vote -- and over somebody else's open enum, ask the question instead: `match bool::then_some(value == Wanted, ()) { Some(()) => …, None => … }`");
                        }
                    },
                );
            }
        }
    }
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
