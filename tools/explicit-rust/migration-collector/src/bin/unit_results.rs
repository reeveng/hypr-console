//! Normalize unit-result bodies without evaluating their side effects as an argument.
#[path = "../edits.rs"]
pub mod edits;
use edits::{Outcome, Plan};
use proc_macro2::Span;
use serde_json::json;
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Expr, GenericArgument, PathArguments, ReturnType, Stmt, Type,
};

struct UnitResults<'a> {
    text: &'a str,
    lines: Vec<usize>,
    changes: Vec<((usize, usize), (usize, usize))>,
}
impl UnitResults<'_> {
    fn span(&self, span: Span) -> (usize, usize) {
        let offset = |position: proc_macro2::LineColumn| {
            let line = &self.text[self.lines[position.line - 1]..];
            self.lines[position.line - 1]
                + line
                    .char_indices()
                    .map(|(offset, _)| offset)
                    .nth(position.column)
                    .unwrap_or(line.len())
        };
        (offset(span.start()), offset(span.end()))
    }
    fn function(&mut self, output: &ReturnType, body: &syn::Block) {
        let ReturnType::Type(_, ty) = output else {
            return;
        };
        let Type::Path(path) = &**ty else { return };
        let Some(segment) = path.path.segments.last() else {
            return;
        };
        if segment.ident != "Result" {
            return;
        }
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return;
        };
        if !matches!(arguments.args.first(),Some(GenericArgument::Type(Type::Tuple(tuple))) if tuple.elems.is_empty())
        {
            return;
        }
        if !matches!(arguments.args.iter().nth(1),Some(GenericArgument::Type(Type::Path(error))) if error.path.segments.last().is_some_and(|segment|segment.ident=="Never"))
        {
            return;
        }
        let [Stmt::Expr(Expr::Call(call), None)] = body.stmts.as_slice() else {
            return;
        };
        if !matches!(&*call.func,Expr::Path(path) if path.path.is_ident("Ok")) {
            return;
        }
        let Some(Expr::Block(inner)) = call.args.first() else {
            return;
        };
        if call.args.len() != 1 || inner.label.is_some() {
            return;
        }
        self.changes
            .push((self.span(body.span()), self.span(inner.span())));
    }
}
impl<'ast> Visit<'ast> for UnitResults<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.function(&item.sig.output, &item.block);
        visit::visit_item_fn(self, item);
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.function(&item.sig.output, &item.block);
        visit::visit_impl_item_fn(self, item);
    }
}
fn files(path: &Path, output: &mut BTreeSet<PathBuf>) -> Outcome<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if !entry.file_type()?.is_symlink() {
                files(&entry.path(), output)?;
            }
        }
    } else if path.extension().is_some_and(|extension| extension == "rs") {
        output.insert(path.to_owned());
    }
    Ok(())
}
fn prepare(plan: &mut Plan) -> Outcome<()> {
    for file in plan.sources.keys().cloned().collect::<Vec<_>>() {
        let text = &plan.sources[&file].text;
        let syntax = syn::parse_file(text)?;
        let mut lines = vec![0];
        lines.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset + 1)),
        );
        let mut visitor = UnitResults {
            text,
            lines,
            changes: vec![],
        };
        visitor.visit_file(&syntax);
        for ((lo, hi), (inner_lo, inner_hi)) in visitor.changes {
            let inner = edits::Span {
                file: file.clone(),
                lo: inner_lo,
                hi: inner_hi,
            };
            plan.add(
                edits::Span {
                    file: file.clone(),
                    lo,
                    hi,
                },
                "unit_result",
                move |plan, _| Ok(format!("{{ {}; Ok(()) }}", plan.get(&inner)?)),
            )?;
        }
    }
    Ok(())
}
fn run() -> Outcome<()> {
    let mut paths = BTreeSet::new();
    let mut apply = false;
    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--apply" => apply = true,
            "--help" | "-h" => {
                println!("unit_results PATH... [--apply]\nDefaults to a dry run. Only exact Result<(),Never> body wrappers are changed; originals are backed up under TMPDIR/builds/unit-result-repair.");
                return Ok(());
            }
            _ => files(Path::new(&argument), &mut paths)?,
        }
    }
    if paths.is_empty() {
        return Err("provide Rust files or source directories".into());
    }
    let mut plan = Plan::new(env::current_dir()?, paths)?;
    prepare(&mut plan)?;
    let outputs = plan.outputs()?;
    for bytes in outputs.values() {
        let text = std::str::from_utf8(bytes)?.trim_start_matches('\u{feff}');
        syn::parse_file(text)?;
    }
    let mut report = json!({"changes":plan.stats,"files":outputs.len()});
    if apply && !outputs.is_empty() {
        let temporary =
            env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        report["backup"] = json!(plan
            .write(&outputs, &temporary.join("builds/unit-result-repair"))?
            .display()
            .to_string());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("unit_results: {error}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_and_value_results_are_distinguished() {
        let text="fn unit()->Result<(),Never>{Ok({side_effect();})} fn value()->Result<u8,Never>{Ok({7})}";
        let syntax = syn::parse_file(text).unwrap();
        let mut walker = UnitResults {
            text,
            lines: vec![0],
            changes: vec![],
        };
        walker.visit_file(&syntax);
        assert_eq!(walker.changes.len(), 1);
        let (body, inner) = walker.changes[0];
        assert!(text[body.0..body.1].contains("side_effect"));
        assert_eq!(&text[inner.0..inner.1], "{side_effect();}");
    }
}
