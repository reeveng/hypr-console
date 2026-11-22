use proc_macro2::{Span, TokenStream, TokenTree};
use serde_json::{json, Value};
use std::{env, fs, path::Path};
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    Expr, Pat, Stmt,
};

struct Collector {
    file: String,
    records: Vec<Value>,
    lines: Vec<usize>,
    columns: Vec<Vec<usize>>,
}
impl Collector {
    fn location(&self, s: Span) -> Value {
        let a = s.start();
        let b = s.end();
        json!({"file":self.file,"lo":self.lines[a.line-1]+self.columns[a.line-1].get(a.column).copied().unwrap_or(a.column),"hi":self.lines[b.line-1]+self.columns[b.line-1].get(b.column).copied().unwrap_or(b.column)})
    }
    fn stmt(&self, s: &Stmt) -> Value {
        self.location(s.span())
    }
    fn expr(&self, e: &Expr) -> Value {
        self.location(e.span())
    }
    fn push(&mut self, kind: &str, span: Span, mut data: Value) {
        data["kind"] = json!(kind);
        data["span"] = self.location(span);
        self.records.push(data);
    }
    fn tokens(&mut self, tokens: TokenStream) {
        for token in tokens {
            match token {
                TokenTree::Literal(l) => self.push("literal", l.span(), json!({})),
                TokenTree::Group(g) => self.tokens(g.stream()),
                _ => {}
            }
        }
    }
}
fn bindings(p: &Pat, out: &mut Vec<String>) {
    match p {
        Pat::Ident(i) => out.push(i.ident.to_string()),
        Pat::Tuple(t) => {
            for x in &t.elems {
                bindings(x, out)
            }
        }
        Pat::TupleStruct(t) => {
            for x in &t.elems {
                bindings(x, out)
            }
        }
        Pat::Struct(t) => {
            for x in &t.fields {
                bindings(&x.pat, out)
            }
        }
        Pat::Reference(t) => bindings(&t.pat, out),
        Pat::Type(t) => bindings(&t.pat, out),
        Pat::Slice(t) => {
            for x in &t.elems {
                bindings(x, out)
            }
        }
        _ => {}
    }
}
impl<'ast> Visit<'ast> for Collector {
    fn visit_attribute(&mut self, a: &'ast syn::Attribute) {
        if a.path().is_ident("doc") {
            self.push("doc", a.span(), json!({}));
        }
        visit::visit_attribute(self, a);
    }
    fn visit_expr_if(&mut self, e: &'ast syn::ExprIf) {
        let mut data = json!({"cond":self.expr(&e.cond),"then":self.location(e.then_branch.span()),"else":e.else_branch.as_ref().map(|(_,e)|self.expr(e))});
        if let Expr::Let(test) = &*e.cond {
            data["pat"] = self.location(test.pat.span());
            data["scrutinee"] = self.expr(&test.expr);
        }
        self.push("if", e.span(), data);
        visit::visit_expr_if(self, e);
    }
    fn visit_expr_while(&mut self, e: &'ast syn::ExprWhile) {
        let mut data = json!({"cond":self.expr(&e.cond),"body":self.location(e.body.span()),"label":e.label.as_ref().map(|l|l.name.to_string())});
        if let Expr::Let(test) = &*e.cond {
            data["pat"] = self.location(test.pat.span());
            data["scrutinee"] = self.expr(&test.expr);
        }
        self.push("while", e.span(), data);
        visit::visit_expr_while(self, e);
    }
    fn visit_local(&mut self, l: &'ast syn::Local) {
        if let Some(init) = &l.init {
            if let Some((_, otherwise)) = &init.diverge {
                let mut names = Vec::new();
                bindings(&l.pat, &mut names);
                self.push("let_else",l.span(),json!({"pat":self.location(l.pat.span()),"init":self.expr(&init.expr),"else":self.expr(otherwise),"names":names}));
            }
        }
        visit::visit_local(self, l);
    }
    fn visit_expr_try(&mut self, e: &'ast syn::ExprTry) {
        self.push("try", e.span(), json!({"value":self.expr(&e.expr)}));
        visit::visit_expr_try(self, e);
    }
    fn visit_expr_reference(&mut self, e: &'ast syn::ExprReference) {
        self.push(
            "reference",
            e.span(),
            json!({"value":self.expr(&e.expr),"mutable":e.mutability.is_some()}),
        );
        visit::visit_expr_reference(self, e);
    }
    fn visit_expr_closure(&mut self, e: &'ast syn::ExprClosure) {
        self.push("closure", e.span(), json!({"body":self.expr(&e.body)}));
        visit::visit_expr_closure(self, e);
    }
    fn visit_block(&mut self, b: &'ast syn::Block) {
        self.push(
            "block",
            b.span(),
            json!({"stmts":b.stmts.iter().map(|s|self.stmt(s)).collect::<Vec<_>>()}),
        );
        visit::visit_block(self, b);
    }
    fn visit_type(&mut self, t: &'ast syn::Type) {
        self.push(
            "type",
            t.span(),
            json!({"written":quote::quote!(#t).to_string()}),
        );
        visit::visit_type(self, t);
    }
    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        self.push(
            "fn",
            f.span(),
            json!({"name":f.sig.ident.to_string(),"body":self.location(f.block.span())}),
        );
        visit::visit_item_fn(self, f);
    }
    fn visit_impl_item_fn(&mut self, f: &'ast syn::ImplItemFn) {
        self.push(
            "fn",
            f.span(),
            json!({"name":f.sig.ident.to_string(),"body":self.location(f.block.span())}),
        );
        visit::visit_impl_item_fn(self, f);
    }
}
fn walk(path: &Path, out: &mut Vec<Value>) {
    if path.is_dir() {
        for p in fs::read_dir(path).unwrap() {
            walk(&p.unwrap().path(), out)
        }
    } else if path.extension().is_some_and(|x| x == "rs") {
        let raw = fs::read_to_string(path).unwrap();
        let text = raw.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let parsed = syn::parse_file(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut lines = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                lines.push(i + 1)
            }
        }
        let columns = text
            .split('\n')
            .map(|line| {
                line.char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(line.len()))
                    .collect()
            })
            .collect();
        let mut c = Collector {
            file: path.display().to_string(),
            records: vec![],
            lines,
            columns,
        };
        c.tokens(text.parse().unwrap());
        c.visit_file(&parsed);
        out.extend(c.records);
    }
}
fn main() {
    let mut out = vec![];
    for path in env::args().skip(1) {
        walk(Path::new(&path), &mut out)
    }
    println!("{}", serde_json::to_string(&out).unwrap());
}
