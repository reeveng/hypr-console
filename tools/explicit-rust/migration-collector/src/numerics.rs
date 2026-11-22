//! Shared typed numeric passes that register edits against an immutable Plan.
use crate::edits::{Outcome, Plan, Span};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub struct Options {
    pub packages: Vec<PathBuf>,
    pub generated: bool,
    pub casts: bool,
    pub arithmetic: bool,
    pub number_crate: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            packages: vec![],
            generated: false,
            casts: true,
            arithmetic: false,
            number_crate: "console_core_number_conversion".into(),
        }
    }
}

fn increment(counts: &mut BTreeMap<String, u64>, name: impl Into<String>) {
    *counts.entry(name.into()).or_default() += 1;
}

fn integer(ty: &str) -> bool {
    matches!(
        ty,
        "u8" | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
    )
}

fn primitive(ty: &str) -> (&str, bool) {
    if let Some(ty) = ty.strip_prefix("&mut ") {
        (ty.trim(), true)
    } else if let Some(ty) = ty.strip_prefix('&') {
        (ty.trim(), true)
    } else {
        (ty, false)
    }
}

fn extracted(call: String) -> String {
    format!("({{ let Ok(numeric_value) = {call}; numeric_value }})")
}

fn fresh_name(plan: &Plan, span: &Span, prefix: &str) -> Outcome<String> {
    let source = &plan
        .sources
        .get(&span.file)
        .ok_or("missing source file")?
        .text;
    let mut name = format!("{prefix}_{}", span.lo);
    while source.contains(&name) {
        name.push('_');
    }
    Ok(name)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Cast {
    Wrapped(String),
    Index,
    Offset,
    NearestSingle,
    TowardZero(String),
    Exact(String),
}

fn cast(source: &str, target: &str) -> Option<Cast> {
    if integer(source) && integer(target) {
        return Some(match target {
            "usize" => Cast::Index,
            "isize" => Cast::Offset,
            _ => Cast::Wrapped(target.into()),
        });
    }
    if source == "u64" && target == "f32" {
        return Some(Cast::NearestSingle);
    }
    if matches!(source, "f32" | "f64")
        && matches!(
            target,
            "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize"
        )
    {
        return Some(Cast::TowardZero(target.into()));
    }
    if (matches!(source, "u8" | "u16" | "i8" | "i16") && matches!(target, "f32" | "f64"))
        || (matches!(source, "u32" | "i32" | "f32") && target == "f64")
        || (source == "bool" && integer(target))
    {
        return Some(Cast::Exact(target.into()));
    }
    None
}

#[derive(Clone)]
struct Function {
    file: PathBuf,
    lo: usize,
    hi: usize,
    constant: bool,
    foreign: bool,
}

fn unsafe_context(
    record: &Value,
    functions: &[Function],
    span: &Span,
) -> Outcome<Option<&'static str>> {
    let kind = record["owner_kind"]
        .as_str()
        .ok_or("missing owner_kind metadata; recapture with the current collector")?;
    let generated = record["generated_owner"]
        .as_bool()
        .ok_or("missing generated_owner metadata; recapture with the current collector")?;
    if generated {
        return Ok(Some("generated_context"));
    }
    if !matches!(kind, "Fn" | "AssocFn" | "Closure") {
        return Ok(Some("constant_or_non_function_context"));
    }
    for function in functions.iter().filter(|function| {
        function.file == span.file && function.lo <= span.lo && span.hi <= function.hi
    }) {
        if function.constant {
            return Ok(Some("constant_function"));
        }
        if function.foreign {
            return Ok(Some("foreign_function"));
        }
    }
    if record["owner_span"].is_null() {
        return Ok(Some("unanchored_owner"));
    }
    Ok(None)
}

pub fn owned(path: &Path, packages: &[PathBuf], generated: bool) -> bool {
    packages.iter().any(|package| {
        path.starts_with(package.join("src"))
            || (generated && path.starts_with(package.join("generated")))
    })
}

fn numeric_record(record: &Value, passes: &BTreeSet<String>) -> bool {
    record["kind"] == "expr"
        && ((record["node"] == "cast" && passes.contains("casts"))
            || (passes.contains("arithmetic")
                && record["integral"] == true
                && matches!(
                    record["node"].as_str(),
                    Some("binary" | "assign_op" | "unary")
                )))
}

pub fn numeric(plan: &mut Plan, records: &[Value], options: &Options) -> Outcome<()> {
    plan.validate_sources(records)?;
    let root = plan.root.clone();
    let packages = options.packages.clone();
    let mut passes = BTreeSet::new();
    if options.casts {
        passes.insert("casts".to_owned());
    }
    if options.arithmetic {
        passes.insert("arithmetic".to_owned());
    }
    let mut functions = vec![];
    for record in records.iter().filter(|record| record["kind"] == "fn") {
        let span = Span::from_json(&record["span"], &root)?;
        functions.push(Function {
            file: span.file,
            lo: span.lo,
            hi: span.hi,
            constant: record["const"] == true,
            foreign: record["foreign_abi"] == true,
        });
    }
    let mut candidates = BTreeMap::<(Span, String), Value>::new();
    let mut excluded = BTreeSet::new();
    for record in records
        .iter()
        .filter(|record| numeric_record(record, &passes))
    {
        let span = Span::from_json(&record["span"], &root)?;
        if !owned(&span.file, &packages, options.generated) {
            continue;
        }
        if span.lo == span.hi {
            increment(&mut plan.review, "dummy_expression");
            continue;
        }
        let node = record["node"]
            .as_str()
            .ok_or("numeric expression has no node")?
            .to_owned();
        let key = (span.clone(), node);
        let operands: &[&str] = match record["node"].as_str() {
            Some("cast" | "unary") => &["value"],
            Some("binary" | "assign_op") => &["left", "right"],
            _ => &[],
        };
        if operands
            .iter()
            .any(|field| record[*field]["file"].as_str().is_none())
        {
            excluded.insert(key);
            increment(&mut plan.review, "generated_operand_without_source");
            continue;
        }
        if let Some(reason) = unsafe_context(record, &functions, &span)? {
            excluded.insert(key);
            increment(&mut plan.review, reason);
            continue;
        }
        if let Some(previous) = candidates.get(&key) {
            for field in [
                "type",
                "left_type",
                "right_type",
                "from_type",
                "op",
                "value",
                "left",
                "right",
            ] {
                if previous[field] != record[field] {
                    return Err(
                        format!("conflicting typed numeric captures at {span:?}: {field}").into(),
                    );
                }
            }
        } else {
            candidates.insert(key, record.clone());
        }
    }
    for ((span, _), record) in candidates
        .into_iter()
        .filter(|(key, _)| !excluded.contains(key))
    {
        let node = record["node"].as_str().ok_or("missing expression node")?;
        if node == "cast" {
            let source = record["from_type"]
                .as_str()
                .ok_or("cast has no source type")?;
            let target = record["type"]
                .as_str()
                .ok_or("cast has no destination type")?;
            let Some(mode) = cast(source, target) else {
                increment(
                    &mut plan.review,
                    format!("cast_needs_domain_conversion:{source}->{target}"),
                );
                continue;
            };
            let value = Span::from_json(&record["value"], &root)?;
            let number = options.number_crate.clone();
            let name = match mode {
                Cast::Exact(_) => "exact_from",
                Cast::TowardZero(_) => "toward_zero",
                Cast::NearestSingle => "single_rounding",
                _ => "integer_bits",
            };
            plan.add(span, name, move |plan, _| {
                let expression = plan.get(&value)?;
                Ok(match &mode {
                    Cast::Wrapped(target) => {
                        extracted(format!("{number}::wrapped::<_, {target}>({expression})"))
                    }
                    Cast::Index => extracted(format!("{number}::wrapped_index({expression})")),
                    Cast::Offset => extracted(format!("{number}::wrapped_offset({expression})")),
                    Cast::NearestSingle => {
                        extracted(format!("{number}::nearest_f32_from_u64({expression})"))
                    }
                    Cast::TowardZero(target) => extracted(format!(
                        "{number}::toward_zero_{target}(f64::from({expression}))"
                    )),
                    Cast::Exact(target) => format!("{target}::from({expression})"),
                })
            })?;
            continue;
        }
        let op = record["op"].as_str().unwrap_or("").replace("Assign", "");
        let method = match op.as_str() {
            "Add" => "wrapping_add",
            "Sub" => "wrapping_sub",
            "Mul" => "wrapping_mul",
            "Shl" => "wrapping_shl",
            "Shr" => "wrapping_shr",
            "Neg" => "wrapping_neg",
            "Div" | "Rem" => {
                increment(&mut plan.review, "divisor_needs_nonzero_policy");
                continue;
            }
            _ => continue,
        };
        if node == "unary" {
            let value = Span::from_json(&record["value"], &root)?;
            if matches!(
                syn::parse_str::<syn::Expr>(plan.text(&value)?),
                Ok(syn::Expr::Lit(_))
            ) {
                continue;
            }
            plan.add(span, "integer_policy", move |plan, _| {
                Ok(format!("({}).wrapping_neg()", plan.get(&value)?))
            })?;
            continue;
        }
        let (target, left_ref) = primitive(
            record["left_type"]
                .as_str()
                .ok_or("binary has no left type")?,
        );
        if !integer(target) {
            increment(&mut plan.review, "non_primitive_arithmetic");
            continue;
        }
        let target = target.to_owned();
        let right_ref = primitive(record["right_type"].as_str().unwrap_or("")).1;
        let right_type = record["right_type"].as_str().unwrap_or("").to_owned();
        let left = Span::from_json(&record["left"], &root)?;
        let right = Span::from_json(&record["right"], &root)?;
        let assign = node == "assign_op";
        let amount_name = fresh_name(plan, &span, "explicit_numeric_amount")?;
        let target_name = fresh_name(plan, &span, "explicit_numeric_target")?;
        let number = options.number_crate.clone();
        let shift_literal = match syn::parse_str::<syn::Expr>(plan.text(&right)?) {
            Ok(syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(value),
                ..
            })) => value.base10_parse::<u32>().ok(),
            _ => None,
        };
        plan.add(span, "integer_policy", move |plan, _| {
            let mut lhs = plan.get(&left)?;
            let mut rhs = plan.get(&right)?;
            if right_ref { rhs = format!("*({rhs})"); }
            if matches!(method, "wrapping_shl" | "wrapping_shr") && right_type != "u32" {
                rhs = match shift_literal {
                    Some(value) => format!("{value}_u32"),
                    None => extracted(format!("{number}::wrapped::<_, u32>({rhs})")),
                };
            }
            Ok(if assign {
                // Primitive compound assignments evaluate their amount before
                // the destination place. Keep both evaluations exactly once.
                format!("{{ let {amount_name} = {rhs}; let {target_name} = &mut ({lhs}); *{target_name} = {target}::{method}(*{target_name}, {amount_name}); }}")
            } else {
                if left_ref { lhs = format!("*({lhs})"); }
                format!("{target}::{method}({lhs}, {rhs})")
            })
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn integer_casts_choose_bit_preserving_boundaries() {
        assert_eq!(cast("i64", "u8"), Some(Cast::Wrapped("u8".into())));
        assert_eq!(cast("i32", "usize"), Some(Cast::Index));
        assert_eq!(cast("u64", "isize"), Some(Cast::Offset));
        assert_eq!(cast("f64", "i16"), Some(Cast::TowardZero("i16".into())));
        assert_eq!(cast("f64", "i128"), None);
        assert_eq!(cast("u64", "f32"), Some(Cast::NearestSingle));
    }

    #[test]
    fn constant_and_generated_owners_are_never_rewritten() -> Outcome<()> {
        let span = Span {
            file: "/fixture.rs".into(),
            lo: 1,
            hi: 4,
        };
        let base = json!({"owner_kind":"Fn","generated_owner":false,"owner_span":{"lo":0,"hi":9}});
        assert_eq!(unsafe_context(&base, &[], &span)?, None);
        for kind in [
            "Const { is_type_const: false }",
            "AssocConst { is_type_const: false }",
            "AnonConst",
            "InlineConst",
            "Static { mutability: Not }",
        ] {
            let mut record = base.clone();
            record["owner_kind"] = kind.into();
            assert!(unsafe_context(&record, &[], &span)?.is_some());
        }
        let mut generated = base.clone();
        generated["generated_owner"] = true.into();
        assert_eq!(
            unsafe_context(&generated, &[], &span)?,
            Some("generated_context")
        );
        let functions = [Function {
            file: span.file.clone(),
            lo: 0,
            hi: 9,
            constant: false,
            foreign: true,
        }];
        assert_eq!(
            unsafe_context(&base, &functions, &span)?,
            Some("foreign_function")
        );
        Ok(())
    }
}
