#[path = "../edits.rs"]
mod edits;

use edits::{Outcome, Plan, Span};
use serde_json::{Value, json};
use std::{collections::{BTreeMap, BTreeSet}, env, fs, path::PathBuf};
use syn::{spanned::Spanned, visit::{self, Visit}, Expr, Pat, Type};

fn array_shape(ty: &str) -> Option<(usize, usize)> {
    let mut ty: Type = syn::parse_str(ty).ok()?;
    let mut references = 0;
    while let Type::Reference(reference) = ty {
        references += 1;
        ty = *reference.elem;
    }
    let Type::Array(array) = ty else { return None };
    let Expr::Lit(length) = array.len else { return None };
    let syn::Lit::Int(length) = length.lit else { return None };
    Some((length.base10_parse().ok()?, references))
}

fn literal(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Lit(value) => match &value.lit { syn::Lit::Int(value) => value.base10_parse().ok(), _ => None },
        Expr::Paren(expr) => literal(&expr.expr),
        _ => None,
    }
}

fn pattern(length: usize, index: usize, prefix_only: bool) -> Option<String> {
    if index >= length { return None }
    let after = length.checked_sub(index)?.checked_sub(1)?;
    let use_prefix = prefix_only || index <= after;
    let padding = if use_prefix { index } else { after };
    if padding > 64 { return None }
    let mut items = vec![];
    if use_prefix {
        items.extend(std::iter::repeat_n("_", index));
        items.push("projected_element");
        if after > 0 || prefix_only { items.push(".."); }
    } else {
        items.push("..");
        items.push("projected_element");
        items.extend(std::iter::repeat_n("_", after));
    }
    Some(format!("[{}]", items.join(", ")))
}

fn bindings(pattern: &Pat, names: &mut BTreeSet<String>) {
    match pattern {
        Pat::Ident(pattern) => { names.insert(pattern.ident.to_string()); }
        Pat::Tuple(pattern) => for element in &pattern.elems { bindings(element, names); },
        Pat::TupleStruct(pattern) => for element in &pattern.elems { bindings(element, names); },
        Pat::Struct(pattern) => for field in &pattern.fields { bindings(&field.pat, names); },
        Pat::Reference(pattern) => bindings(&pattern.pat, names),
        Pat::Type(pattern) => bindings(&pattern.pat, names),
        Pat::Slice(pattern) => for element in &pattern.elems { bindings(element, names); },
        _ => {}
    }
}

struct Walker<'a> {
    plan: &'a mut Plan,
    file: PathBuf,
    lines: Vec<usize>,
    columns: Vec<Vec<usize>>,
    indices: &'a BTreeMap<Span, Value>,
    methods: &'a BTreeMap<Span, bool>,
    scopes: Vec<BTreeSet<String>>,
    mutable: bool,
    unknown_receiver: bool,
    prefix_only: bool,
    failure: Option<String>,
}

impl Walker<'_> {
    fn span(&self, span: proc_macro2::Span) -> Span {
        let offset = |point: proc_macro2::LineColumn| self.lines[point.line - 1] + self.columns[point.line - 1][point.column];
        Span { file: self.file.clone(), lo: offset(span.start()), hi: offset(span.end()) }
    }

    fn stable(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Path(path) if path.path.segments.len() == 1 => self.scopes.iter().rev().any(|scope| scope.contains(&path.path.segments[0].ident.to_string())),
            Expr::Field(field) => self.stable(&field.base),
            Expr::Paren(paren) => self.stable(&paren.expr),
            Expr::Group(group) => self.stable(&group.expr),
            Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => self.stable(&unary.expr),
            Expr::Index(index) => self.stable(&index.expr) && literal(&index.index).is_some(),
            _ => false,
        }
    }

    fn context(&mut self, expr: &Expr, mutable: bool, unknown: bool) {
        let previous = (self.mutable, self.unknown_receiver);
        self.mutable = mutable;
        self.unknown_receiver = unknown;
        self.visit_expr(expr);
        (self.mutable, self.unknown_receiver) = previous;
    }

    fn function(&mut self, signature: &syn::Signature, body: &syn::Block) {
        let mut names = BTreeSet::new();
        for input in &signature.inputs {
            match input {
                syn::FnArg::Receiver(_) => { names.insert("self".into()); }
                syn::FnArg::Typed(input) => bindings(&input.pat, &mut names),
            }
        }
        self.scopes.push(names);
        self.visit_block(body);
        self.scopes.pop();
    }

    fn project(&mut self, index: &syn::ExprIndex) -> Outcome<()> {
        let span = self.span(index.span());
        let Some(record) = self.indices.get(&span) else { return Ok(()) };
        let Some((length, references)) = record["value_type"].as_str().and_then(array_shape) else {
            *self.plan.review.entry("dynamic_container".into()).or_default() += 1;
            return Ok(())
        };
        let Some(position) = literal(&index.index) else {
            *self.plan.review.entry("dynamic_index".into()).or_default() += 1;
            return Ok(())
        };
        if !self.stable(&index.expr) || self.unknown_receiver {
            *self.plan.review.entry("temporary_or_unresolved_receiver".into()).or_default() += 1;
            return Ok(())
        }
        let Some(pattern) = pattern(length, position, self.prefix_only) else {
            *self.plan.review.entry("bounds_or_pattern_budget".into()).or_default() += 1;
            return Ok(())
        };
        let base = self.span(index.expr.span());
        let mutable = self.mutable;
        self.plan.add(span, if mutable { "mutable_array_projection" } else { "array_projection" }, move |plan, _| {
            let base = plan.get(&base)?;
            let dereferences = "*".repeat(references);
            let borrow = if mutable { "&mut " } else { "&" };
            Ok(format!("(*{{ let {pattern} = {borrow}{dereferences}{base}; projected_element }})"))
        })
    }
}

impl<'ast> Visit<'ast> for Walker<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.scopes.push(BTreeSet::new());
        visit::visit_block(self, block);
        self.scopes.pop();
    }
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) { self.function(&function.sig, &function.block); }
    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) { self.function(&function.sig, &function.block); }
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(initializer) = &local.init {
            self.context(&initializer.expr, false, false);
            if let Some((_, divergence)) = &initializer.diverge { self.context(divergence, false, false); }
        }
        if let Some(scope) = self.scopes.last_mut() { bindings(&local.pat, scope); }
    }
    fn visit_expr_index(&mut self, index: &'ast syn::ExprIndex) {
        if let Err(error) = self.project(index) { self.failure = Some(error.to_string()); }
        self.context(&index.expr, self.mutable, self.unknown_receiver);
        self.context(&index.index, false, false);
    }
    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.context(&assignment.left, true, false);
        self.context(&assignment.right, false, false);
    }
    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        let mutable = matches!(binary.op, syn::BinOp::AddAssign(_) | syn::BinOp::SubAssign(_) | syn::BinOp::MulAssign(_) | syn::BinOp::DivAssign(_) | syn::BinOp::RemAssign(_) | syn::BinOp::BitXorAssign(_) | syn::BinOp::BitAndAssign(_) | syn::BinOp::BitOrAssign(_) | syn::BinOp::ShlAssign(_) | syn::BinOp::ShrAssign(_));
        self.context(&binary.left, mutable, false);
        self.context(&binary.right, false, false);
    }
    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) { self.context(&reference.expr, reference.mutability.is_some(), false); }
    fn visit_expr_method_call(&mut self, method: &'ast syn::ExprMethodCall) {
        let mutable = self.methods.get(&self.span(method.span())).copied();
        self.context(&method.receiver, mutable.unwrap_or(false), mutable.is_none());
        for argument in &method.args { self.context(argument, false, false); }
    }
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        self.context(&call.func, false, false);
        for argument in &call.args { self.context(argument, false, false); }
    }
}

fn main() -> Outcome<()> {
    let mut args = env::args().skip(1).peekable();
    let root = env::current_dir()?;
    let mut packages = vec![];
    let mut facts = vec![];
    let mut apply = false;
    let mut generated = false;
    let mut prefix_only = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--package" => packages.push(args.next().ok_or("--package needs a name")?),
            "--facts" => while args.peek().is_some_and(|arg| !arg.starts_with("--")) { facts.push(PathBuf::from(args.next().unwrap())); },
            "--apply" => apply = true,
            "--include-generated" => generated = true,
            "--prefix-only" => prefix_only = true,
            _ => return Err(format!("unknown argument {argument}").into()),
        }
    }
    if packages.is_empty() || facts.is_empty() { return Err("provide --package and --facts".into()) }
    let mut records = vec![];
    for path in facts { records.extend(serde_json::from_slice::<Vec<Value>>(&fs::read(path)?)?); }
    let selected = |file: &str| packages.iter().any(|package| file.contains(&format!("/{package}/")));
    let mut indices = BTreeMap::new();
    let mut methods = BTreeMap::new();
    let mut files = BTreeSet::new();
    for record in &records {
        let Some(file) = record["span"]["file"].as_str() else { continue };
        if !selected(file) || record["kind"] != "expr" { continue }
        if !generated && record["generated_owner"].as_bool() != Some(false) { continue }
        let span = Span::from_json(&record["span"], &root)?;
        match record["node"].as_str() {
            Some("index") => { files.insert(span.file.clone()); indices.insert(span, record.clone()); }
            Some("method") => if let Some(mutable) = record["receiver_mut"].as_bool() { methods.insert(span, mutable); },
            _ => {}
        }
    }
    let mut plan = Plan::new(&root, &files)?;
    plan.validate_sources(&records)?;
    for file in files {
        let source = plan.sources[&file].text.clone();
        let tree = syn::parse_file(&source)?;
        let mut lines = vec![0];
        lines.extend(source.bytes().enumerate().filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)));
        let columns = source.split('\n').map(|line| line.char_indices().map(|(index, _)| index).chain(std::iter::once(line.len())).collect()).collect();
        let mut walker = Walker { plan: &mut plan, file, lines, columns, indices: &indices, methods: &methods, scopes: vec![], mutable: false, unknown_receiver: false, prefix_only, failure: None };
        walker.visit_file(&tree);
        if let Some(error) = walker.failure { return Err(error.into()) }
    }
    let output = plan.outputs()?;
    for source in output.values() { syn::parse_file(std::str::from_utf8(source)?)?; }
    let backup = if apply { Some(plan.write(&output, &PathBuf::from(env::var_os("TMPDIR").unwrap_or_else(|| "/var/tmp".into())).join("builds/array-projections"))?) } else { None };
    println!("{}", serde_json::to_string_pretty(&json!({ "changes": plan.stats, "review": plan.review, "files": output.len(), "backup": backup }))?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn exact_array_shape() { assert_eq!(array_shape("&mut [u32; 4]"), Some((4, 1))); assert_eq!(array_shape("[u8]"), None); assert_eq!(array_shape("[u8; N]"), None); }
    #[test] fn compact_projection_patterns() { assert_eq!(pattern(10000, 9999, false).as_deref(), Some("[.., projected_element]")); assert_eq!(pattern(4, 2, false).as_deref(), Some("[.., projected_element, _]")); assert_eq!(pattern(0, 0, false), None); assert_eq!(pattern(4, 4, false), None); }
    #[test] fn prefix_projections_stay_anchored_at_the_original_index() {
        assert_eq!(pattern(4, 2, true).as_deref(), Some("[_, _, projected_element, ..]"));
        assert_eq!(pattern(4, 3, true).as_deref(), Some("[_, _, _, projected_element, ..]"));
        assert_eq!(pattern(10000, 9999, true), None);
    }
    #[test] fn mutable_places_and_borrows() { let mut values = [String::from("first"), String::from("last")]; *{ let [.., projected_element] = &mut values; projected_element } = String::from("changed"); let borrowed = &*{ let [projected_element, ..] = &values; projected_element }; assert_eq!(borrowed, "first"); assert_eq!(values[1], "changed"); }
    #[test] fn reference_binding_does_not_need_mut() { let mut values = [3, 4]; let reference = &mut values; *{ let [.., projected_element] = &mut *reference; projected_element } = 7; assert_eq!(values, [3, 7]); }
}
