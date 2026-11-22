//! Batch explicit result signatures and resolved callers against one typed snapshot.
#![feature(rustc_private)]
extern crate rustc_driver;
extern crate rustc_lexer;
#[path = "../edits.rs"]
pub mod edits;
#[path = "../numerics.rs"]
pub mod numerics;
use edits::{Fingerprints, Outcome, Plan, Span};
use quote::ToTokens;
use rustc_lexer::{tokenize, DocStyle, FrontmatterAllowed, TokenKind};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};
use syn::{
    parse::Parser,
    visit::{self, Visit},
    visit_mut::{self, VisitMut},
    Expr, Pat,
};

fn records(paths: &[PathBuf]) -> Outcome<Vec<Value>> {
    let mut out = vec![];
    for path in paths {
        out.extend(serde_json::from_str::<Vec<Value>>(&fs::read_to_string(
            path,
        )?)?);
    }
    Ok(out)
}
fn extracted(expression: &str) -> String {
    format!("({{ let Ok(explicit_value) = {expression}; explicit_value }})")
}

fn relative_offset(text: &str, position: proc_macro2::LineColumn) -> Outcome<usize> {
    let mut lines = text.split_inclusive('\n');
    let before = lines
        .by_ref()
        .take(position.line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let line = lines.next().unwrap_or("");
    let column = line
        .char_indices()
        .map(|(byte, _)| byte)
        .chain([line.len()])
        .nth(position.column)
        .ok_or("invalid source column")?;
    Ok(before + column)
}

fn return_operand(text: &str, span: &Span) -> Outcome<Option<Span>> {
    use syn::spanned::Spanned;
    let returned = syn::parse_str::<syn::ExprReturn>(text)?;
    returned
        .expr
        .map(|expression| {
            Ok(Span {
                file: span.file.clone(),
                lo: span.lo + relative_offset(text, expression.span().start())?,
                hi: span.lo + relative_offset(text, expression.span().end())?,
            })
        })
        .transpose()
}

fn branch_attributes(text: &str, binding: bool) -> Outcome<String> {
    let attributes = if binding {
        match syn::parse_str::<syn::Stmt>(text)? {
            syn::Stmt::Local(local) => local.attrs,
            _ => return Err("expected let-else statement".into()),
        }
    } else {
        match syn::parse_str::<Expr>(text)? {
            Expr::If(expression) => expression.attrs,
            _ => return Err("expected conditional expression".into()),
        }
    };
    Ok(attributes
        .iter()
        .map(|attribute| attribute.to_token_stream().to_string())
        .collect::<Vec<_>>()
        .join(" "))
}
fn bump(plan: &mut Plan, name: &str) {
    *plan.review.entry(name.into()).or_default() += 1;
}
fn belongs(file: &Path, packages: &BTreeMap<String, PathBuf>, generated: bool) -> Option<String> {
    packages.iter().find_map(|(name, directory)| {
        (file.starts_with(directory.join("src"))
            || generated && file.starts_with(directory.join("generated")))
        .then(|| name.clone())
    })
}
fn from(value: &Value, root: &Path) -> Outcome<Span> {
    Span::from_json(value, root)
}

#[derive(Clone)]
struct Function {
    record: Value,
    span: Span,
    body: Span,
    package: String,
    role: Option<Value>,
}
fn enum_path(
    function: &Function,
    caller: &Path,
    packages: &BTreeMap<String, PathBuf>,
    config: &Value,
) -> Outcome<String> {
    let prefix = if caller.starts_with(&packages[&function.package]) {
        "crate".into()
    } else {
        let mut directory = caller.parent().ok_or("caller has no directory")?;
        while !directory.join("Cargo.toml").exists() {
            directory = directory.parent().ok_or("caller has no Cargo manifest")?;
        }
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(directory.join("Cargo.toml"))?)?;
        let caller_name = manifest["package"]["name"]
            .as_str()
            .ok_or("caller missing package name")?;
        if let Some(alias) = config["aliases"][caller_name][&function.package].as_str() {
            alias.to_owned()
        } else {
            fn search(table: &toml::Value, wanted: &str, found: &mut BTreeSet<String>) {
                for name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    for (alias, value) in table
                        .get(name)
                        .and_then(toml::Value::as_table)
                        .into_iter()
                        .flatten()
                    {
                        if value
                            .get("package")
                            .and_then(toml::Value::as_str)
                            .unwrap_or(alias)
                            == wanted
                        {
                            found.insert(alias.replace('-', "_"));
                        }
                    }
                }
                for target in table
                    .get("target")
                    .and_then(toml::Value::as_table)
                    .into_iter()
                    .flatten()
                    .map(|(_, value)| value)
                {
                    search(target, wanted, found);
                }
            }
            let mut found = BTreeSet::new();
            search(&manifest, &function.package, &mut found);
            if found.len() != 1 {
                return Err(
                    format!("configure alias {caller_name} -> {}", function.package).into(),
                );
            }
            found.pop_first().ok_or("missing alias")?
        }
    };
    Ok(format!(
        "{prefix}::explicit_answers::{}",
        function.role.as_ref().ok_or("not a boolean role")?["name"]
            .as_str()
            .ok_or("role has no name")?
    ))
}
fn call_text(function: &Function, path: Option<&str>, expression: &str) -> String {
    let value = extracted(expression);
    match (&function.role, path) {
        (Some(role), Some(path)) => format!(
            "matches!({value}, {path}::{})",
            role["yes"].as_str().unwrap()
        ),
        _ => value,
    }
}
fn role_valid(role: &Value) -> bool {
    role["yes"] != role["no"]
        && ["name", "yes", "no"].iter().all(|key| {
            role[*key]
                .as_str()
                .is_some_and(|value| syn::parse_str::<syn::Ident>(value).is_ok())
        })
}

fn compatible_roles(left: &Value, right: &Value) -> bool {
    left["name"] == right["name"]
        && BTreeSet::from([left["yes"].as_str(), left["no"].as_str()])
            == BTreeSet::from([right["yes"].as_str(), right["no"].as_str()])
}

fn result_pass(
    plan: &mut Plan,
    facts: &[Value],
    syntax: &[Value],
    packages: &BTreeMap<String, PathBuf>,
    config: &Value,
    generated: bool,
) -> Outcome<()> {
    let mut functions = BTreeMap::<String, Function>::new();
    let mut declarations = BTreeMap::<String, BTreeMap<String, Value>>::new();
    for record in facts.iter().filter(|r| r["kind"] == "fn") {
        if record["trait_impl"] == true
            || record["test"] == true
            || record["result"] == true
            || record["body"].is_null()
            || record["ret"].is_null()
            || record["output"] == "!"
        {
            continue;
        }
        let span = from(&record["span"], &plan.root)?;
        let Some(package) = belongs(&span.file, packages, generated) else {
            continue;
        };
        if record["foreign_abi"] == true {
            bump(plan, "foreign_abi");
            continue;
        }
        let function = syn::parse_str::<syn::ImplItemFn>(plan.text(&span)?).map_err(|error| {
            format!(
                "function {} at {}:{}..{}: {error}",
                record["id"],
                span.file.display(),
                span.lo,
                span.hi
            )
        })?;
        if function.sig.ident == "main" {
            continue;
        }
        let identity = record["id"]
            .as_str()
            .ok_or("function lacks identity")?
            .to_owned();
        let role = if record["output"] == "bool" {
            let name = function.sig.ident.to_string();
            let role = config["answers"]
                .get(&identity)
                .or_else(|| config["answers"].get(&name));
            let Some(role) = role else {
                bump(plan, "boolean_api_needs_role");
                continue;
            };
            if !role_valid(role) {
                return Err(format!("invalid boolean role for {identity}").into());
            }
            let name = role["name"].as_str().unwrap().to_owned();
            let enums = declarations.entry(package.clone()).or_default();
            if enums
                .get(&name)
                .is_some_and(|old| !compatible_roles(old, role))
            {
                return Err(format!("conflicting role {name}").into());
            }
            enums.entry(name).or_insert_with(|| role.clone());
            Some(role.clone())
        } else {
            None
        };
        let selected = Function {
            record: record.clone(),
            span,
            body: from(&record["body"], &plan.root)?,
            package,
            role,
        };
        if let Some(old) = functions.get(&identity) {
            if old.record != selected.record {
                return Err(format!("conflicting function capture: {identity}").into());
            }
        }
        functions.insert(identity, selected);
    }
    for function in functions.values() {
        let ret = from(&function.record["ret"], &plan.root)?;
        let never = "console_core_never::Never";
        let default = function.record["default_ret"] == true;
        let output = if function.role.is_some() {
            enum_path(function, &function.span.file, packages, config)?
        } else {
            plan.text(&ret)?.to_owned()
        };
        plan.add(ret, "result_signature", move |_, _| {
            Ok(if default {
                format!(" -> core::result::Result<(), {never}>")
            } else {
                format!("core::result::Result<{output}, {never}>")
            })
        })?;
        let parsed =
            syn::parse_str::<syn::ImplItemFn>(plan.text(&function.span)?).map_err(|error| {
                format!(
                    "selected function {} at {:?}: {error}",
                    function.record["id"], function.span
                )
            })?;
        let returns = matches!(
            parsed.block.stmts.last(),
            Some(syn::Stmt::Expr(Expr::Return(_), _))
        );
        if !returns {
            let role = function.role.clone();
            let name = if role.is_some() {
                Some(enum_path(function, &function.span.file, packages, config)?)
            } else {
                None
            };
            let unit = function.record["output"] == "()";
            plan.add(function.body.clone(), "result_body", move |_, text| {
                let body = &text[1..text.len() - 1];
                Ok(match (&role, &name) {
                    (Some(role), Some(name)) => format!(
                        "{{ Ok(match {{{body}}} {{ true => {name}::{}, false => {name}::{} }}) }}",
                        role["yes"].as_str().unwrap(),
                        role["no"].as_str().unwrap()
                    ),
                    _ if unit => format!("{{ {{{body}}}; Ok(()) }}"),
                    _ => format!("{{ Ok({{{body}}}) }}"),
                })
            })?;
        }
    }
    let called = facts
        .iter()
        .filter(|r| r["kind"] == "expr" && r["node"] == "call" && !r["callee"].is_null())
        .map(|r| from(&r["callee"], &plan.root))
        .collect::<Outcome<BTreeSet<_>>>()?;
    for record in facts
        .iter()
        .filter(|r| r["kind"] == "expr" && r["generated_owner"] != true)
    {
        let span = from(&record["span"], &plan.root)?;
        if span.lo == span.hi {
            continue;
        }
        if let Some(function) = record["def"].as_str().and_then(|id| functions.get(id)) {
            let node = record["node"].as_str().unwrap_or("");
            if node == "call" || node == "method" || node == "path" && !called.contains(&span) {
                let path = if function.role.is_some() {
                    Some(enum_path(function, &span.file, packages, config)?)
                } else {
                    None
                };
                let function = function.clone();
                let callback = node == "path";
                plan.add(
                    span.clone(),
                    if node == "path" {
                        "result_callback"
                    } else {
                        "result_call"
                    },
                    move |_, text| {
                        if callback {
                            let parameters =
                                (0..function.record["params"].as_array().map_or(0, Vec::len))
                                    .map(|i| format!("explicit_argument_{i}"))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                            Ok(format!(
                                "(|{parameters}| {})",
                                call_text(
                                    &function,
                                    path.as_deref(),
                                    &format!("{text}({parameters})")
                                )
                            ))
                        } else {
                            Ok(call_text(&function, path.as_deref(), text))
                        }
                    },
                )?;
            }
        }
        if record["node"] == "return" {
            let Some(function) = record["owner"].as_str().and_then(|id| functions.get(id)) else {
                continue;
            };
            // Closure owners can share their enclosing item's identity. Syntax
            // ownership keeps their returns in the closure's original contract.
            if syntax.iter().filter(|r| r["kind"] == "closure").any(|r| {
                from(&r["span"], &plan.root).is_ok_and(|closure| {
                    closure.file == span.file
                        && function.body.lo <= closure.lo
                        && closure.lo <= span.lo
                        && span.hi <= closure.hi
                })
            }) {
                continue;
            }
            let value = return_operand(plan.text(&span)?, &span).map_err(|error| {
                format!("return in {} at {span:?}: {error}", function.record["id"])
            })?;
            let role = function.role.clone();
            let name = if role.is_some() {
                Some(enum_path(function, &span.file, packages, config)?)
            } else {
                None
            };
            plan.add(span, "result_return", move |plan, _| {
                let value = match &value {
                    Some(value) => plan.get(value)?,
                    None => "()".into(),
                };
                Ok(match (&role, &name) {
                    (Some(role), Some(name)) => format!(
                        "return Ok(match {value} {{ true => {name}::{}, false => {name}::{} }})",
                        role["yes"].as_str().unwrap(),
                        role["no"].as_str().unwrap()
                    ),
                    _ => format!("return Ok({value})"),
                })
            })?;
        }
    }
    for record in syntax.iter().filter(|r| r["kind"] == "try") {
        let span = from(&record["span"], &plan.root)?;
        let owner = functions
            .values()
            .filter(|f| f.body.file == span.file && f.body.lo <= span.lo && span.hi <= f.body.hi)
            .min_by_key(|f| f.body.hi - f.body.lo);
        let Some(function) = owner else {
            continue;
        };
        let output = function.record["output"].as_str().unwrap_or("");
        if !output.starts_with("Option<") && !output.contains("::Option<") {
            continue;
        }
        if syntax.iter().filter(|r| r["kind"] == "closure").any(|r| {
            from(&r["span"], &plan.root).is_ok_and(|closure| {
                closure.file == span.file
                    && function.body.lo <= closure.lo
                    && closure.lo <= span.lo
                    && span.hi <= closure.hi
            })
        }) {
            continue;
        }
        let value = from(&record["value"], &plan.root)?;
        plan.add(span, "option_return", move |plan, _| {
            Ok(format!(
                "(match {} {{ Some(explicit_value) => explicit_value, None => return Ok(None) }})",
                plan.get(&value)?
            ))
        })?;
    }
    for (package, enums) in declarations {
        let file = packages[&package].join("src/explicit_answers.rs");
        let mut text = "//! Named outcomes at the crate's interfaces.\n\n".to_owned();
        for (name, role) in enums {
            text += &format!(
                "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {name} {{ {}, {} }}\n\n",
                role["yes"].as_str().unwrap(),
                role["no"].as_str().unwrap()
            );
        }
        plan.new_file(file, text)?;
        let file = packages[&package].join("src/lib.rs");
        let span = Span {
            lo: 0,
            hi: plan.sources[&file].text.len(),
            file,
        };
        plan.add(span, "answers_module", |_, text| {
            Ok(format!("{text}\npub mod explicit_answers;\n"))
        })?;
    }
    Ok(())
}

fn complement(pattern: &Pat, ty: &str) -> Option<String> {
    match pattern {
        Pat::TupleStruct(tuple)
            if tuple.elems.len() == 1
                && tuple.elems.iter().all(|p| {
                    matches!(
                        p,
                        Pat::Wild(_) | Pat::Ident(syn::PatIdent { subpat: None, .. })
                    )
                }) =>
        {
            match tuple.path.segments.last()?.ident.to_string().as_str() {
                "Some" if ty.contains("Option<") => Some("None".into()),
                "Ok" if ty.contains("Result<") => Some("Err(_)".into()),
                "Err" if ty.contains("Result<") => Some("Ok(_)".into()),
                _ => None,
            }
        }
        Pat::Path(path) if path.path.is_ident("None") && ty.contains("Option<") => {
            Some("Some(_)".into())
        }
        _ if matches!(
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
                | "bool"
                | "char"
        ) =>
        {
            Some("_".into())
        }
        _ => None,
    }
}
struct HasLet(bool);
impl<'ast> Visit<'ast> for HasLet {
    fn visit_expr_let(&mut self, _: &'ast syn::ExprLet) {
        self.0 = true;
    }
}
struct BindingNames(Vec<syn::PatIdent>);
impl<'ast> Visit<'ast> for BindingNames {
    fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
        self.0.push(p.clone());
        visit::visit_pat_ident(self, p);
    }
}
struct RemoveMut;
impl VisitMut for RemoveMut {
    fn visit_pat_ident_mut(&mut self, p: &mut syn::PatIdent) {
        p.mutability = None;
        visit_mut::visit_pat_ident_mut(self, p);
    }
}
fn branch_pass(
    plan: &mut Plan,
    facts: &[Value],
    syntax: &[Value],
    packages: &BTreeMap<String, PathBuf>,
    generated: bool,
) -> Outcome<()> {
    let types = facts
        .iter()
        .filter(|r| r["kind"] == "expr")
        .map(|r| {
            Ok((
                from(&r["span"], &plan.root)?,
                r["type"].as_str().unwrap_or("").to_owned(),
            ))
        })
        .collect::<Outcome<BTreeMap<_, _>>>()?;
    for record in syntax
        .iter()
        .filter(|r| r["kind"] == "if" || r["kind"] == "let_else")
    {
        let span = from(&record["span"], &plan.root)?;
        if belongs(&span.file, packages, generated).is_none() {
            continue;
        }
        let binding = record["kind"] == "let_else";
        let attributes = branch_attributes(plan.text(&span)?, binding)
            .map_err(|error| format!("branch attributes at {span:?}: {error}"))?;
        let condition = from(
            if binding {
                &record["init"]
            } else {
                record.get("scrutinee").unwrap_or(&record["cond"])
            },
            &plan.root,
        )?;
        let pattern = if record["pat"].is_null() {
            None
        } else {
            Some(from(&record["pat"], &plan.root)?)
        };
        let other = match &pattern {
            Some(pattern) => complement(
                &Pat::parse_single.parse_str(plan.text(pattern)?)?,
                types.get(&condition).map_or("", String::as_str),
            ),
            None => {
                let mut has = HasLet(false);
                has.visit_expr(&syn::parse_str::<Expr>(plan.text(&condition)?)?);
                if has.0 {
                    None
                } else {
                    Some("false".into())
                }
            }
        };
        let Some(other) = other else {
            bump(plan, "branch_needs_domain_review");
            continue;
        };
        let otherwise = if record["else"].is_null() {
            None
        } else {
            Some(from(&record["else"], &plan.root)?)
        };
        if binding {
            let mut pat = Pat::parse_single.parse_str(plan.text(pattern.as_ref().unwrap())?)?;
            let mut names = BindingNames(vec![]);
            names.visit_pat(&pat);
            if names
                .0
                .iter()
                .any(|p| p.by_ref.is_some() || p.subpat.is_some())
            {
                bump(plan, "binding_needs_borrow_review");
                continue;
            }
            let outer = names
                .0
                .iter()
                .map(|p| {
                    format!(
                        "{}{}",
                        if p.mutability.is_some() { "mut " } else { "" },
                        p.ident
                    )
                })
                .collect::<Vec<_>>();
            let inner = names
                .0
                .iter()
                .map(|p| p.ident.to_string())
                .collect::<Vec<_>>();
            let bind = if outer.len() == 1 {
                outer.join("")
            } else {
                format!("({})", outer.join(", "))
            };
            let values = if inner.len() == 1 {
                inner.join("")
            } else {
                format!("({})", inner.join(", "))
            };
            RemoveMut.visit_pat_mut(&mut pat);
            let pat = pat.to_token_stream().to_string();
            let otherwise = otherwise.ok_or("let-else has no alternative")?;
            plan.add(span, "named_binding", move |plan, _| {
                Ok(format!(
                    "{attributes} let {bind} = match {} {{ {pat} => {values}, {other} => {} }};",
                    plan.get(&condition)?,
                    plan.get(&otherwise)?
                ))
            })?;
        } else {
            let yes = from(&record["then"], &plan.root)?;
            plan.add(span, "named_branch", move |plan, _| {
                Ok(format!(
                    "{attributes} match {} {{ {} => {}, {other} => {} }}",
                    plan.get(&condition)?,
                    match &pattern {
                        Some(p) => plan.get(p)?,
                        None => "true".into(),
                    },
                    plan.get(&yes)?,
                    match &otherwise {
                        Some(p) => plan.get(p)?,
                        None => "{}".into(),
                    }
                ))
            })?;
        }
    }
    Ok(())
}
fn comments(plan: &mut Plan, packages: &BTreeMap<String, PathBuf>, generated: bool) -> Outcome<()> {
    let files = plan
        .sources
        .keys()
        .filter(|path| belongs(path, packages, generated).is_some())
        .cloned()
        .collect::<Vec<_>>();
    for file in files {
        let text = plan.sources[&file].text.clone();
        let mut at = 0;
        let mut safety = false;
        let mut end = 0;
        for token in tokenize(&text, FrontmatterAllowed::Yes) {
            let start = at;
            at += usize::try_from(token.len)?;
            match token.kind {
                TokenKind::LineComment { doc_style } => {
                    let gap = &text[end..start];
                    let continuation =
                        safety && gap.trim().is_empty() && gap.matches('\n').count() <= 1;
                    let reason = text[start..at].starts_with("// SAFETY:") || continuation;
                    if doc_style != Some(DocStyle::Inner) && !reason {
                        plan.add(
                            Span {
                                file: file.clone(),
                                lo: start,
                                hi: at,
                            },
                            "remove_comment",
                            |_, _| Ok(String::new()),
                        )?;
                    }
                    safety = reason;
                    end = at;
                }
                TokenKind::BlockComment { doc_style, .. } => {
                    if doc_style != Some(DocStyle::Inner) {
                        plan.add(
                            Span {
                                file: file.clone(),
                                lo: start,
                                hi: at,
                            },
                            "remove_comment",
                            |_, _| Ok(String::new()),
                        )?;
                    }
                    safety = false;
                    end = at;
                }
                TokenKind::Whitespace => {}
                _ => {
                    safety = false;
                    end = at;
                }
            }
        }
    }
    Ok(())
}

fn package_path(root: &Path, package: &str) -> Outcome<PathBuf> {
    let path = ["crates", "forks"]
        .into_iter()
        .map(|directory| root.join(directory).join(package))
        .find(|path| path.is_dir())
        .ok_or_else(|| format!("unknown package {package}"))?
        .canonicalize()?;
    path.strip_prefix(root)?;
    let manifest = fs::read_to_string(path.join("Cargo.toml"))?.parse::<toml::Table>()?;
    if manifest
        .get("package")
        .and_then(|table| table.get("name"))
        .and_then(toml::Value::as_str)
        != Some(package)
    {
        return Err(format!("package name does not match {}", path.display()).into());
    }
    Ok(path)
}

fn paths_after(
    option: &str,
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    paths: &mut Vec<PathBuf>,
) -> Outcome<()> {
    let initial = paths.len();
    while args.peek().is_some_and(|arg| !arg.starts_with('-')) {
        paths.push(args.next().ok_or("missing JSON path")?.into());
    }
    if paths.len() == initial {
        return Err(format!("{option} needs at least one JSON file").into());
    }
    Ok(())
}

fn run() -> Outcome<()> {
    let mut root = env::current_dir()?;
    let mut packages = vec![];
    let mut typed = vec![];
    let mut syntax = vec![];
    let mut roles = None;
    let mut apply = false;
    let mut generated = false;
    let mut passes = "results,branches,comments".to_owned();
    let mut arithmetic = false;
    let mut diff = None;
    let mut fingerprints = Fingerprints::Required;
    let mut args = env::args().skip(1).peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root needs path")?),
            "--package" => packages.push(args.next().ok_or("--package needs name")?),
            "--facts" => paths_after("--facts", &mut args, &mut typed)?,
            "--syntax" => paths_after("--syntax", &mut args, &mut syntax)?,
            "--diff" => diff = Some(PathBuf::from(args.next().ok_or("--diff needs a path")?)),
            "--allow-legacy-facts" => fingerprints = Fingerprints::WhenPresent,
            "--roles" => roles = Some(PathBuf::from(args.next().ok_or("--roles needs path")?)),
            "--passes" => passes = args.next().ok_or("--passes needs list")?,
            "--arithmetic" => {
                if args.next().as_deref() != Some("wrapping") {
                    return Err("--arithmetic requires an explicitly reviewed wrapping policy".into());
                }
                arithmetic = true;
            }
            "--include-generated" => generated = true,
            "--apply" => apply = true,
            "--help" | "-h" => {
                println!(
                    "apis --package NAME --facts HIR.json... --syntax syntax.json... [--roles roles.json] [--passes results,branches,comments] [--include-generated] [--diff FILE] [--apply]\nRepeat --package for a dependency family. Dry runs report counts and --diff writes the prepared patch. Apply backs up originals under TMPDIR/builds/explicit-apis. Use --allow-legacy-facts only for fresh captures missing fingerprints."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {argument}").into()),
        }
    }
    if packages.is_empty() || typed.is_empty() {
        return Err("--package and --facts required".into());
    }
    let root = root.canonicalize()?;
    let packages = packages
        .into_iter()
        .map(|name| Ok((name.clone(), package_path(&root, &name)?)))
        .collect::<Outcome<BTreeMap<_, _>>>()?;
    let facts = records(&typed)?;
    let syntax = records(&syntax)?;
    let paths = facts
        .iter()
        .chain(syntax.iter())
        .filter_map(|r| r["span"]["file"].as_str())
        .filter(|path| path.ends_with(".rs"))
        .map(PathBuf::from)
        .collect::<BTreeSet<_>>();
    let mut plan = Plan::new(&root, paths)?;
    plan.validate_fingerprints(&facts, fingerprints)?;
    let config = match roles {
        Some(path) => serde_json::from_str(&fs::read_to_string(path)?)?,
        None => json!({}),
    };
    for pass in passes.split(',') {
        match pass {
            "results" => result_pass(&mut plan, &facts, &syntax, &packages, &config, generated)?,
            "branches" if !syntax.is_empty() => {
                branch_pass(&mut plan, &facts, &syntax, &packages, generated)?
            }
            "comments" => comments(&mut plan, &packages, generated)?,
            "casts" | "arithmetic" => {}
            _ => return Err(format!("unknown pass or missing syntax capture: {pass}").into()),
        }
    }
    let selected = passes.split(',').collect::<BTreeSet<_>>();
    if selected.contains("arithmetic") && !arithmetic {
        return Err("arithmetic pass requires --arithmetic wrapping after reviewing the algorithm's overflow policy".into());
    }
    if selected.contains("casts") || selected.contains("arithmetic") {
        numerics::numeric(
            &mut plan,
            &facts,
            &numerics::Options {
                packages: packages.values().cloned().collect(),
                generated,
                casts: selected.contains("casts"),
                arithmetic: selected.contains("arithmetic"),
                number_crate: "console_core_number_conversion".into(),
            },
        )?;
    }
    let outputs = plan.outputs()?;
    if let Some(path) = diff {
        fs::write(path, plan.unified_diff(&outputs)?)?;
    }
    let mut report = json!({"changes":plan.stats,"review":plan.review,"files":outputs.len()});
    if apply && !outputs.is_empty() {
        let temporary =
            env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        report["backup"] = json!(
            plan
                .write(&outputs, &temporary.join("builds/explicit-apis"))?
                .display()
                .to_string()
        );
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("apis: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opposite_predicates_share_a_named_domain() {
        let on = json!({"name":"CurveLocation","yes":"OnCurve","no":"OffCurve"});
        let off = json!({"name":"CurveLocation","yes":"OffCurve","no":"OnCurve"});
        assert!(compatible_roles(&on, &off));
        assert!(!compatible_roles(
            &on,
            &json!({"name":"CurveLocation","yes":"OnCurve","no":"Unknown"})
        ));
        assert!(!role_valid(
            &json!({"name":"CurveLocation","yes":"OnCurve","no":"OnCurve"})
        ));
    }
    #[test]
    fn converted_macro_return_runs_once_and_inactive_cfg_stays_inactive() -> Outcome<()> {
        let temporary =
            env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let root = temporary
            .join("builds")
            .join(format!("api-semantics-{nonce}"));
        let package = root.join("crates/example");
        fs::create_dir_all(package.join("src"))?;
        let file = package.join("src/main.rs");
        let body = "{ return vec![{ SIDE.fetch_add(1, Ordering::SeqCst); 7u8 }]; }";
        let function = format!("fn value() -> Vec<u8> {body}");
        let conditional = "#[cfg(any())] if missing_cfg_condition() { missing_cfg_code(); }";
        let source = format!("use std::sync::atomic::{{AtomicU8, Ordering}};\nmod console_core_never {{ pub enum Never {{}} }}\nstatic SIDE: AtomicU8 = AtomicU8::new(0);\n{function}\nfn main() {{ {conditional} let values = value(); assert_eq!(values, vec![7]); assert_eq!(SIDE.load(Ordering::SeqCst), 1); }}\n");
        fs::write(&file, &source)?;
        let span = |text: &str| {
            let lo = source.find(text).unwrap();
            json!({"file":file,"lo":lo,"hi":lo+text.len()})
        };
        let returned = "return vec![{ SIDE.fetch_add(1, Ordering::SeqCst); 7u8 }]";
        let call = source.rfind("value()").unwrap();
        let facts = vec![
            json!({"kind":"fn","id":"value","span":span(&function),"body":span(body),"ret":span("Vec<u8>"),"default_ret":false,"output":"Vec<u8>","trait_impl":false,"test":false,"result":false,"params":[]}),
            json!({"kind":"expr","node":"return","owner":"value","span":span(returned),"value":null}),
            json!({"kind":"expr","node":"call","def":"value","span":{"file":file,"lo":call,"hi":call+7}}),
        ];
        let syntax = vec![
            json!({"kind":"if","span":span(conditional),"cond":span("missing_cfg_condition()"),"then":span("{ missing_cfg_code(); }"),"else":null}),
        ];
        let packages = BTreeMap::from([("example".into(), package)]);
        let mut plan = Plan::new(&root, [&file])?;
        result_pass(&mut plan, &facts, &syntax, &packages, &json!({}), false)?;
        branch_pass(&mut plan, &facts, &syntax, &packages, false)?;
        let outputs = plan.outputs()?;
        fs::write(&file, &outputs[&file])?;
        let binary = root.join("converted");
        let built = std::process::Command::new("rustc")
            .args(["--edition=2024"])
            .arg(&file)
            .arg("-o")
            .arg(&binary)
            .output()?;
        if !built.status.success() {
            return Err(String::from_utf8_lossy(&built.stderr).into_owned().into());
        }
        let ran = std::process::Command::new(binary).output()?;
        if !ran.status.success() {
            return Err(String::from_utf8_lossy(&ran.stderr).into_owned().into());
        }
        fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn macro_return_operands_are_preserved() {
        let text = "return vec![effect(), café()]";
        let span = Span {
            file: PathBuf::from("source.rs"),
            lo: 100,
            hi: 100 + text.len(),
        };
        let value = return_operand(text, &span).unwrap().unwrap();
        assert_eq!(
            &text[value.lo - 100..value.hi - 100],
            "vec![effect(), café()]"
        );
        assert!(return_operand("return", &Span { hi: 106, ..span })
            .unwrap()
            .is_none());
    }
    #[test]
    fn cfg_expression_attributes_are_preserved() {
        let attributes = branch_attributes(
            "#[cfg(target_arch = \"wasm32\")] if enabled { wasm_only(); }",
            false,
        )
        .unwrap();
        assert!(attributes.contains("target_arch"));
        assert!(attributes.contains("wasm32"));
        let attributes = branch_attributes(
            "#[cfg(any())] let Some(mut value) = input else { return; };",
            true,
        )
        .unwrap();
        assert!(attributes.contains("cfg"));
    }
    #[test]
    fn refutable_inner_variants_are_deferred() {
        let p = Pat::parse_single
            .parse_str("Some(Shape::Circle(x))")
            .unwrap();
        assert!(complement(&p, "Option<Shape>").is_none());
    }
    #[test]
    fn option_remainder_is_explicit() {
        let p = Pat::parse_single.parse_str("Some(mut value)").unwrap();
        assert_eq!(complement(&p, "Option<u8>"), Some("None".into()));
    }
    #[test]
    fn mutable_bindings_survive_outside_match() {
        let mut p = Pat::parse_single.parse_str("Some(mut value)").unwrap();
        let mut names = BindingNames(vec![]);
        names.visit_pat(&p);
        assert!(names.0[0].mutability.is_some());
        RemoveMut.visit_pat_mut(&mut p);
        assert_eq!(p.to_token_stream().to_string(), "Some (value)");
    }
}
