//! Promote selected private, infallible helpers to the error their callers already return.
//!
//! bounds_errors ROOT FACTS SOURCE ERROR_TYPE HELPER... [--write BACKUP_DIRECTORY]
//! Run the existing EXPLICIT014 bounds policy on the promoted helpers afterwards.

#[path = "../edits.rs"]
#[expect(dead_code, reason = "this binary uses the edit planner without the application helpers used by the other migration commands")]
mod edits;
#[path = "../source.rs"]
#[expect(dead_code, reason = "this binary maps source positions without the reindentation helpers used by the other migration commands")]
mod source;

use edits::{Outcome, Plan, Span};
use proc_macro2::{TokenStream, TokenTree};
use serde_json::Value;
use source::Text;
use std::{
  collections::{BTreeMap, BTreeSet},
  env, fs,
  path::{Path, PathBuf},
};
use syn::{
  Expr, GenericArgument, Item, Pat, PathArguments, Type,
  spanned::Spanned,
  visit::{self, Visit},
};

const NEVER: &str = "console_core_never::Never";
type Range = (usize, usize);

fn location(raw: &Value, file: &Path) -> Option<Range> {
  let observed = Path::new(raw["file"].as_str()?);
  if !file.ends_with(observed) {
    return None;
  }
  Some((
    raw["lo"].as_u64()?.try_into().ok()?,
    raw["hi"].as_u64()?.try_into().ok()?,
  ))
}

fn spelled_error(ty: &Type) -> Option<&Type> {
  let Type::Path(path) = ty else { return None };
  let segment = path.path.segments.last()?;
  if segment.ident != "Result" {
    return None;
  }
  let PathArguments::AngleBracketed(args) = &segment.arguments else {
    return None;
  };
  match args.args.iter().nth(1)? {
    GenericArgument::Type(error) if args.args.len() == 2 => Some(error),
    _ => None,
  }
}

#[derive(Default)]
struct Uses<'a> {
  locals: Vec<&'a syn::Local>,
  tries: Vec<&'a syn::ExprTry>,
}
impl<'a> Visit<'a> for Uses<'a> {
  fn visit_local(&mut self, local: &'a syn::Local) {
    self.locals.push(local);
    visit::visit_local(self, local);
  }
  fn visit_expr_try(&mut self, expr: &'a syn::ExprTry) {
    self.tries.push(expr);
    visit::visit_expr_try(self, expr);
  }
  // Each of these establishes an independent error target.
  fn visit_item(&mut self, _: &'a Item) {}
  fn visit_expr_closure(&mut self, _: &'a syn::ExprClosure) {}
  fn visit_expr_async(&mut self, _: &'a syn::ExprAsync) {}
  fn visit_expr_const(&mut self, _: &'a syn::ExprConst) {}
  fn visit_expr_try_block(&mut self, _: &'a syn::ExprTryBlock) {}
}

fn register(
  plan: &mut Plan,
  file: &Path,
  (lo, hi): Range,
  reason: &str,
  replacement: String,
) -> Outcome<()> {
  plan.add(
    Span {
      file: file.to_owned(),
      lo,
      hi,
    },
    reason,
    move |_, _| Ok(replacement.clone()),
  )
}

fn references(
  tokens: TokenStream,
  text: &Text<'_>,
  names: &BTreeSet<String>,
  found: &mut BTreeSet<Range>,
) {
  for token in tokens {
    match token {
      TokenTree::Group(group) => references(group.stream(), text, names, found),
      TokenTree::Ident(name) if names.contains(&name.to_string()) => {
        found.insert(text.span(name.span()));
      }
      _ => {}
    }
  }
}

// Private names may also be visible to a descendant module that was disabled
// during fact collection. Decline the group if any other package source names it.
fn reject_other_sources(
  directory: &Path,
  selected: &Path,
  names: &BTreeSet<String>,
) -> Outcome<()> {
  for entry in fs::read_dir(directory)? {
    let entry = entry?;
    let path = entry.path();
    let kind = entry.file_type()?;
    if kind.is_dir() && !matches!(entry.file_name().to_str(), Some("target" | ".git")) {
      reject_other_sources(&path, selected, names)?;
    } else if kind.is_file()
      && path != selected
      && path.extension().is_some_and(|ext| ext == "rs")
    {
      let source = fs::read_to_string(&path)?;
      if names.iter().any(|n| source.contains(n)) {
        let mut found = BTreeSet::new();
        references(source.parse()?, &Text::new(&source), names, &mut found);
        if !found.is_empty() {
          return Err(
            format!("another source names a selected helper: {}", path.display()).into(),
          );
        }
      }
    }
  }
  Ok(())
}

fn plan_calls(
  plan: &mut Plan,
  file: &Path,
  rows: &[Value],
  error_type: &str,
  names: &BTreeSet<String>,
) -> Outcome<()> {
  let src = plan.sources[file].text.clone();
  let text = Text::new(&src);
  let ast = syn::parse_file(&src)?;
  let functions: BTreeMap<_, _> = ast
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Fn(f) => Some((f.sig.ident.to_string(), f)),
      _ => None,
    })
    .collect();
  let typed: BTreeMap<_, _> = rows
    .iter()
    .filter(|r| r["kind"] == "fn")
    .filter_map(|r| Some((location(&r["ret"], file)?, r)))
    .collect();
  let mut helpers = BTreeSet::new();
  let mut callers = BTreeMap::new();
  let mut accounted = BTreeSet::new();
  let mut crate_name = None;
  for (name, fun) in &functions {
    let syn::ReturnType::Type(_, ret) = &fun.sig.output else {
      continue;
    };
    let Some(record) = typed.get(&text.span(ret.span())) else {
      continue;
    };
    let id = record["id"].as_str().ok_or("missing function identity")?;
    callers.insert(id, (fun, record["result_error"].as_str()));
    if !names.contains(name) {
      continue;
    }
    if !matches!(fun.vis, syn::Visibility::Inherited)
      || fun.sig.unsafety.is_some()
      || [
        "async",
        "exported",
        "entry",
        "foreign_abi",
        "in_trait",
        "trait_impl",
        "const",
      ]
      .iter()
      .any(|key| record[*key] != false)
      || record["result_error"] != NEVER
      || fun.attrs.iter().any(|a| !a.path().is_ident("inline"))
    {
      return Err(format!("unsupported declaration for {name}").into());
    }
    let old = spelled_error(ret).ok_or("an explicit Result error is required")?;
    crate_name = Some(id.split_once("::").ok_or("missing crate identity")?.0);
    helpers.insert(id);
    accounted.insert(text.span(fun.sig.ident.span()));
    register(
      plan,
      file,
      text.span(old.span()),
      "promote private Result",
      error_type.to_owned(),
    )?;
  }
  if helpers.len() != names.len() {
    return Err("not every selected function was captured".into());
  }
  let domain = format!(
    "{}::{}",
    crate_name.ok_or("no functions selected")?,
    error_type
      .strip_prefix("crate::")
      .ok_or("supply a crate-relative domain error")?
  );
  let mut calls = BTreeMap::new();
  let mut paths = BTreeSet::new();
  let mut infallible = BTreeSet::new();
  let mut ok_patterns = BTreeSet::new();
  for row in rows {
    if row["kind"] == "local"
      && row["result_ok"] == true
      && let Some(pattern) = location(&row["pattern"], file)
    {
      ok_patterns.insert(pattern);
    }
    if row["result_error"] == NEVER
      && row["kind"] == "expr"
      && let Some(range) = location(&row["span"], file)
    {
      infallible.insert(range);
    }
    let Some(def) = row["def"].as_str().filter(|d| helpers.contains(*d)) else {
      continue;
    };
    let range = location(&row["span"], file)
      .ok_or("selected helper has an expanded or out-of-module use")?;
    match row["node"].as_str() {
      Some("path") => {
        paths.insert(range);
      }
      Some("call") => {
        let callee = location(&row["callee"], file).ok_or("generated call")?;
        let owner = row["owner"].as_str().ok_or("missing caller")?;
        if row["generated_owner"] != false {
          return Err("generated caller".into());
        }
        calls.insert(range, (owner, def, callee));
      }
      _ => return Err("indirect use of selected helper".into()),
    }
  }
  if paths != calls.values().map(|(_, _, span)| *span).collect() {
    return Err("non-call reference to a selected helper".into());
  }
  let mut edges = BTreeMap::<&str, BTreeSet<&str>>::new();
  let mut terminals = BTreeSet::new();
  let mut handled = BTreeSet::new();
  for (id, (fun, error)) in callers {
    let mut uses = Uses::default();
    uses.visit_block(&fun.block);
    if helpers.contains(id) {
      for expr in uses.tries {
        let inner = text.span(expr.expr.span());
        if !infallible.contains(&inner) {
          return Err("cannot prove the old ? operand is infallible".into());
        }
        // Edit the punctuation, retaining comments and any planned changes
        // inside the operand (including a nested call to another helper).
        register(
          plan,
          file,
          (inner.0, inner.0),
          "preserve infallible operand",
          "(match ".into(),
        )?;
        register(
          plan,
          file,
          text.span(expr.question_token.span()),
          "exhaust infallible operand",
          " { ::core::result::Result::Ok(value) => value })".into(),
        )?;
      }
    }
    for local in uses.locals {
      let Some(init) = &local.init else { continue };
      let range = text.span(init.expr.span());
      let Some(&(owner, def, _)) = calls.get(&range) else {
        continue;
      };
      let Expr::Call(call) = init.expr.as_ref() else {
        continue;
      };
      let Expr::Path(path) = call.func.as_ref() else {
        continue;
      };
      let Pat::TupleStruct(pattern) = &local.pat else {
        continue;
      };
      if !ok_patterns.contains(&text.span(pattern.span()))
        || owner != id
        || init.diverge.is_some()
        || !pattern.path.is_ident("Ok")
        || pattern.elems.len() != 1
      {
        continue;
      }
      // Rust has already checked this ordinary let pattern was exhaustive.
      // Keep its inner binding or destructuring pattern verbatim, including
      // comments and binding modes. Remove just the known Result constructor.
      let old = text.span(pattern.elems[0].span());
      let wrapper = text.span(pattern.span());
      if [&src[wrapper.0..old.0], &src[old.1..wrapper.1]]
        .iter()
        .any(|part| part.contains("//") || part.contains("/*"))
      {
        continue;
      }
      let suffix = if helpers.contains(owner) {
        edges.entry(def).or_default().insert(owner);
        "?"
      } else if error == Some(domain.as_str()) {
        terminals.insert(def);
        "?"
      } else if error == Some("()") {
        terminals.insert(def);
        ".map_err(|_| ())?"
      } else {
        return Err(format!("caller {} has an incompatible error", fun.sig.ident).into());
      };
      let callee = path.path.segments.last().ok_or("empty callee path")?;
      accounted.insert(text.span(callee.ident.span()));
      handled.insert(range);
      register(
        plan,
        file,
        text.span(pattern.span()),
        "propagate binding",
        src[old.0..old.1].to_owned(),
      )?;
      register(
        plan,
        file,
        (range.1, range.1),
        "propagate call",
        suffix.into(),
      )?;
    }
  }
  if handled.len() != calls.len() {
    return Err("a call is outside a supported irrefutable binding or error context".into());
  }
  loop {
    let old = terminals.len();
    for (callee, callers) in &edges {
      if callers.iter().any(|caller| terminals.contains(caller)) {
        terminals.insert(callee);
      }
    }
    if old == terminals.len() {
      break;
    }
  }
  if terminals != helpers {
    return Err("private call chain has no fallible decoder endpoint".into());
  }
  // Also reject authored references disabled by this build's configuration,
  // including names in macro arguments and cfg-stripped declarations.
  let mut authored = BTreeSet::new();
  references(src.parse()?, &text, names, &mut authored);
  if authored != accounted {
    return Err("an authored reference to a selected name lacks a direct typed call".into());
  }
  Ok(())
}

fn main() -> Outcome<()> {
  let mut args = env::args().skip(1);
  let root = PathBuf::from(args.next().ok_or("ROOT is required")?);
  let facts = args.next().ok_or("FACTS is required")?;
  let file = root
    .join(args.next().ok_or("SOURCE is required")?)
    .canonicalize()?;
  let error = args.next().ok_or("ERROR_TYPE is required")?;
  let mut names = BTreeSet::new();
  let mut backup = None;
  while let Some(arg) = args.next() {
    if arg == "--write" {
      backup = Some(PathBuf::from(
        args.next().ok_or("missing backup directory")?,
      ));
    } else {
      names.insert(arg);
    }
  }
  let package = file
    .ancestors()
    .find(|p| p.join("Cargo.toml").is_file())
    .ok_or("missing package manifest")?;
  reject_other_sources(package, &file, &names)?;
  let rows: Vec<Value> = serde_json::from_slice(&fs::read(facts)?)?;
  let mut plan = Plan::new(&root, [&file])?;
  plan.validate_sources(&rows)?;
  plan_calls(&mut plan, &file, &rows, &error, &names)?;
  let output = plan.outputs()?;
  println!("{:?}", plan.stats);
  if let Some(backup) = backup {
    println!("backup {}", plan.write(&output, &backup)?.display());
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;
  use std::{
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
  };
  const CODE: &str = r#"
extern crate self as console_core_never;
pub enum Never {}
#[derive(Debug, PartialEq)] enum Fault { Bounds }
fn coefficient(bytes: &[u8]) -> Result<u8, console_core_never::Never> { Ok(bytes[0]) }
fn decode(bytes: &[u8]) -> Result<u8, console_core_never::Never> {
    let Ok(v) = coefficient(bytes); Ok(v)
}
fn decoder(bytes: &[u8]) -> Result<u8, Fault> {
    let Ok(v) = decode(bytes); Ok(v)
}
fn legacy(bytes: &[u8]) -> Result<u8, ()> {
    let Ok(v) = decode(bytes); Ok(v)
}
fn main() {
    assert_eq!(decoder(&[23]), Ok(23)); assert_eq!(legacy(&[23]), Ok(23));
    assert_eq!(decoder(&[]), Err(Fault::Bounds)); assert_eq!(legacy(&[]), Err(()));
}
"#;

  // Supply the same resolved identities and source spans as the collector.
  // Only immediate irrefutable bindings are valid direct uses in this fixture.
  fn rewrite(source: &str) -> Outcome<(PathBuf, String)> {
    let root = env::temp_dir().join(format!(
      "bounds-errors-{}-{}",
      std::process::id(),
      SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&root)?;
    let file = root.join("lib.rs");
    fs::write(&file, source)?;
    let text = Text::new(source);
    let location = |span: proc_macro2::Span| {
      let (lo, hi) = text.span(span);
      json!({"lo":lo,"hi":hi,"file":file})
    };
    let mut rows = vec![];
    for item in syn::parse_file(source)?.items {
      let Item::Fn(fun) = item else { continue };
      let name = fun.sig.ident.to_string();
      let id = format!("example::{name}");
      let error = match name.as_str() {
        "coefficient" | "decode" => NEVER,
        "decoder" => "example::Fault",
        "legacy" => "()",
        _ => continue,
      };
      let syn::ReturnType::Type(_, ret) = &fun.sig.output else {
        continue;
      };
      rows.push(json!({"kind":"fn","ret":location(ret.span()),"id":id,"result_error":error,"async":false,"exported":false,"entry":false,"foreign_abi":false,"in_trait":false,"trait_impl":false,"const":fun.sig.constness.is_some()}));
      let mut uses = Uses::default();
      uses.visit_block(&fun.block);
      for local in uses.locals {
        let Some(init) = &local.init else { continue };
        let Expr::Call(call) = init.expr.as_ref() else {
          continue;
        };
        let Expr::Path(callee) = call.func.as_ref() else {
          continue;
        };
        rows.push(json!({"kind":"local","pattern":location(local.pat.span()),"result_ok":true}));
        let function = &callee.path.segments.last().unwrap().ident;
        let def = format!("example::{function}");
        rows.push(json!({"kind":"expr","node":"path","span":location(callee.span()),"def":def}));
        rows.push(json!({"kind":"expr","node":"call","span":location(call.span()),"callee":location(callee.span()),"def":def,"owner":id,"generated_owner":false}));
      }
    }
    let mut plan = Plan::new(&root, [&file])?;
    plan_calls(
      &mut plan,
      &file,
      &rows,
      "crate::Fault",
      &BTreeSet::from(["coefficient".into(), "decode".into()]),
    )?;
    let result = String::from_utf8(plan.outputs()?.remove(&file).unwrap())?;
    Ok((root, result))
  }

  #[test]
  fn propagates_bounds_to_enum_and_unit_endpoints() -> Outcome<()> {
    let (root, source) = rewrite(CODE)?;
    // The subsequent EXPLICIT014 recovery pass adds the bounds branch.
    let source = source.replace(
      "Ok(bytes[0])",
      "bytes.first().copied().ok_or(Fault::Bounds)",
    );
    let p = root.join("result.rs");
    fs::write(&p, source)?;
    let exe = root.join("result");
    assert!(
      Command::new("rustc")
        .args(["--edition=2024"])
        .arg(p)
        .arg("-o")
        .arg(&exe)
        .status()?
        .success()
    );
    assert!(Command::new(exe).status()?.success());
    Ok(())
  }

  #[test]
  fn refuses_inactive_references_and_function_pointers() {
    for extra in [
      "#[cfg(any())] fn hidden(){decode(&[]);}",
      "fn ptr(){let _ = coefficient;}",
      "fn indirect(){let _ = || decode(&[]);}",
    ] {
      assert!(rewrite(&format!("{CODE}\n{extra}")).is_err());
    }
  }

  #[test]
  fn preserves_unit_and_named_destructuring_patterns() -> Outcome<()> {
    let (_, unit) = rewrite(
      &CODE
        .replace(
          "fn decode(bytes: &[u8]) -> Result<u8",
          "fn decode(bytes: &[u8]) -> Result<()",
        )
        .replace(
          "let Ok(v) = coefficient(bytes); Ok(v)",
          "let Ok(_) = coefficient(bytes); Ok(())",
        )
        .replace(
          "let Ok(v) = decode(bytes); Ok(v)",
          "let Ok(()) = decode(bytes); Ok(0)",
        ),
    )?;
    assert!(unit.contains("let () = decode(bytes)?"));
    let (_, named) = rewrite(
      &CODE
        .replace(
          "fn decode(bytes: &[u8]) -> Result<u8",
          "struct Fields { v: u8 } fn decode(bytes: &[u8]) -> Result<Fields",
        )
        .replace(
          "let Ok(v) = coefficient(bytes); Ok(v)",
          "let Ok(v) = coefficient(bytes); Ok(Fields { v })",
        )
        .replace(
          "let Ok(v) = decode(bytes); Ok(v)",
          "let Ok(Fields { v }) = decode(bytes); Ok(v)",
        ),
    )?;
    assert!(named.contains("let Fields { v } = decode(bytes)?"));
    Ok(())
  }

  #[test]
  fn rejects_public_helpers_and_const_functions() {
    for prefix in ["pub fn coefficient", "const fn coefficient"] {
      let source = CODE.replace("fn coefficient", prefix);
      assert!(rewrite(&source).is_err());
    }
  }
}
