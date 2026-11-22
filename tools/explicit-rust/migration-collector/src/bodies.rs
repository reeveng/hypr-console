//! A function body written `Ok({ … })` written out as the statements it holds and an `Ok` at the end.
//!
//! When a function's answer became `Result<T, Never>`, the first rewrite wrapped the whole old body
//! in `Ok({ … })`, and the one that took a `bool` out of a signature wrapped it again in
//! `match { … } { true => …, false => … }`. Both compile and neither reads as written: the
//! statements sit one level deeper than the function, the closing brace lands at the wrong depth,
//! and clippy names the shapes (`unit_arg`, `blocks_in_conditions`). This puts the statements back
//! at the function's depth and leaves the answer as the tail, `Ok(tail)`, `Ok(())` when the answer
//! is unit, or `Ok(match tail { … })` when the old body was a decision.
//!
//! Moving the statements out of the inner block into the function's own block changes no scope a
//! program can see: the inner block was the function's only statement, so whatever it declared
//! lived until the function returned either way.

use crate::lift::Idents;
use crate::names::Namer;
use crate::source::{self, Text};
use std::collections::BTreeSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Stmt};

pub struct Rewrite {
    pub span: (usize, usize),
    pub text: String,
}

struct Finder<'a> {
    text: &'a Text<'a>,
    found: Vec<Rewrite>,
}

fn unit(output: &syn::ReturnType) -> bool {
    let syn::ReturnType::Type(_, ty) = output else { return false };
    let syn::Type::Path(path) = &**ty else { return false };
    let Some(last) = path.path.segments.last() else { return false };
    let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else { return false };
    last.ident == "Result"
        && matches!(arguments.args.first(), Some(syn::GenericArgument::Type(syn::Type::Tuple(tuple))) if tuple.elems.is_empty())
}

fn expression_attributes(expression: &Expr) -> bool {
    match expression {
        Expr::Block(block) => !block.attrs.is_empty(),
        Expr::Call(call) => !call.attrs.is_empty(),
        Expr::MethodCall(call) => !call.attrs.is_empty(),
        Expr::Match(choice) => !choice.attrs.is_empty(),
        Expr::If(branch) => !branch.attrs.is_empty(),
        Expr::Macro(mac) => !mac.attrs.is_empty(),
        Expr::Path(path) => !path.attrs.is_empty(),
        Expr::Unsafe(block) => !block.attrs.is_empty(),
        Expr::Assign(assign) => !assign.attrs.is_empty(),
        Expr::ForLoop(each) => !each.attrs.is_empty(),
        Expr::While(each) => !each.attrs.is_empty(),
        Expr::Loop(each) => !each.attrs.is_empty(),
        Expr::Return(value) => !value.attrs.is_empty(),
        Expr::Struct(value) => !value.attrs.is_empty(),
        Expr::Tuple(value) => !value.attrs.is_empty(),
        Expr::Lit(value) => !value.attrs.is_empty(),
        Expr::Binary(value) => !value.attrs.is_empty(),
        Expr::Unary(value) => !value.attrs.is_empty(),
        Expr::Paren(value) => !value.attrs.is_empty(),
        Expr::Field(value) => !value.attrs.is_empty(),
        Expr::Reference(value) => !value.attrs.is_empty(),
        Expr::Try(value) => !value.attrs.is_empty(),
        Expr::Cast(value) => !value.attrs.is_empty(),
        Expr::Closure(value) => !value.attrs.is_empty(),
        Expr::Index(value) => !value.attrs.is_empty(),
        Expr::Range(value) => !value.attrs.is_empty(),
        _ => true,
    }
}

fn plain(block: &syn::ExprBlock) -> bool {
    block.label.is_none() && block.attrs.is_empty()
}

fn indented(text: &str, from: &str, to: &str) -> String {
    let moved = source::moved(&format!("\n{text}"), from, to);
    moved.strip_prefix('\n').unwrap_or(&moved).to_owned()
}

impl Finder<'_> {
    fn function(&mut self, signature: &syn::Signature, block: &syn::Block) {
        let [Stmt::Expr(Expr::Call(call), None)] = block.stmts.as_slice() else { return };
        let Expr::Path(callee) = &*call.func else { return };
        if !callee.path.is_ident("Ok") || call.args.len() != 1 || !call.attrs.is_empty() {
            return;
        }
        let (inner, arms) = match call.args.first() {
            Some(Expr::Block(inner)) if plain(inner) => (inner, None),
            Some(Expr::Match(choice)) if choice.attrs.is_empty() => match &*choice.expr {
                Expr::Block(inner) if plain(inner) => (inner, Some(choice)),
                _ => return,
            },
            _ => return,
        };
        let statements = &inner.block.stmts;
        let attributed = statements.iter().any(|statement| match statement {
            Stmt::Expr(expression, _) => expression_attributes(expression),
            Stmt::Local(_) | Stmt::Item(_) | Stmt::Macro(_) => false,
        });
        if attributed {
            return;
        }
        let tail = match statements.last() {
            Some(Stmt::Expr(expression, None)) => Some(expression),
            _ => None,
        };
        let hoisted_tail = tail.filter(|expression| {
            arms.is_none()
                && unit(&signature.output)
                && matches!(
                    expression,
                    Expr::Match(_) | Expr::If(_) | Expr::ForLoop(_) | Expr::While(_) | Expr::Loop(_) | Expr::Block(_)
                )
        });
        let answer = match (tail, hoisted_tail) {
            (Some(_), Some(_)) | (None, _) => None,
            (Some(tail), None) => Some(tail),
        };
        let decided = matches!(
            answer,
            Some(Expr::Match(_) | Expr::If(_) | Expr::Block(_) | Expr::Loop(_) | Expr::While(_) | Expr::ForLoop(_) | Expr::Unsafe(_))
        );
        if arms.is_some() && (answer.is_none() || decided) {
            return;
        }
        let questioned = match answer {
            Some(Expr::Try(inner)) if arms.is_none() => {
                let mut idents = Idents(BTreeSet::new());
                for input in &signature.inputs {
                    idents.visit_fn_arg(input);
                }
                idents.visit_block(block);
                let namer = Namer { taken: &idents.0, own_type: None };
                match namer.name(&inner.expr, &BTreeSet::new()) {
                    Some(name) => Some(name),
                    None => return,
                }
            }
            _ => None,
        };
        let own = self.text.span(block.span());
        let function_indent = self.text.indent_at(self.text.span(signature.span()).0).to_owned();
        let body_indent = format!("{function_indent}    ");
        let hoisted = match answer {
            Some(_) => &statements[..statements.len() - 1],
            None => &statements[..],
        };
        let mut said = String::from("{\n");
        if let (Some(first), Some(last)) = (hoisted.first(), hoisted.last()) {
            let first_at = self.text.span(first.span()).0;
            let last_at = self.text.span(last.span()).1;
            let from = self.text.indent_at(first_at).to_owned();
            let region = match self.text.starts_line(first_at) {
                true => indented(&self.text.text[self.text.line_start(self.text.line_of(first_at))..last_at], &from, &body_indent),
                false => format!("{body_indent}{}", source::moved(&self.text.text[first_at..last_at], &from, &body_indent)),
            };
            said.push_str(region.trim_end());
            match (&questioned, last) {
                (Some(_), Stmt::Local(_)) => said.push('\n'),
                _ => said.push_str("\n\n"),
            }
        }
        said.push_str(&body_indent);
        match (answer, arms) {
            (None, _) => said.push_str("Ok(())"),
            (Some(tail), None) => {
                let at = self.text.span(tail.span());
                let from = self.text.indent_at(at.0).to_owned();
                let written = source::moved(&self.text.text[at.0..at.1], &from, &body_indent);
                match &questioned {
                    Some(name) => said.push_str(&format!("let {name} = {written};\n\n{body_indent}Ok({name})")),
                    None => said.push_str(&format!("Ok({written})")),
                }
            }
            (Some(tail), Some(choice)) => {
                let at = self.text.span(tail.span());
                let from = self.text.indent_at(at.0).to_owned();
                let scrutinee = source::moved(&self.text.text[at.0..at.1], &from, &body_indent);
                let group = self.text.span(choice.brace_token.span.join());
                let written = &self.text.text[group.0..group.1];
                let arms = match written.contains('\n') {
                    true => source::moved(written, self.text.indent_at(group.0), &body_indent),
                    false => {
                        let mut listed = String::from("{\n");
                        for arm in &choice.arms {
                            let (lo, hi) = self.text.span(arm.span());
                            let arm = self.text.text[lo..hi].trim_end_matches(',');
                            listed.push_str(&format!("{body_indent}    {arm},\n"));
                        }
                        listed.push_str(&body_indent);
                        listed.push('}');
                        listed
                    }
                };
                said.push_str(&format!("Ok(match {scrutinee} {arms})"));
            }
        }
        said.push('\n');
        said.push_str(&function_indent);
        said.push('}');
        self.found.push(Rewrite { span: own, text: said });
    }
}

impl<'ast> Visit<'ast> for Finder<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.function(&item.sig, &item.block);
        visit::visit_item_fn(self, item);
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.function(&item.sig, &item.block);
        visit::visit_impl_item_fn(self, item);
    }
    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if let Some(block) = &item.default {
            self.function(&item.sig, block);
        }
        visit::visit_trait_item_fn(self, item);
    }
}

pub fn rewrites(text: &str) -> syn::Result<Vec<Rewrite>> {
    let file = syn::parse_file(text)?;
    let lines = Text::new(text);
    let mut finder = Finder { text: &lines, found: vec![] };
    finder.visit_file(&file);
    let mut found = finder.found;
    found.sort_by_key(|rewrite| rewrite.span);
    let mut outermost: Vec<Rewrite> = vec![];
    for rewrite in found {
        match outermost.last() {
            Some(last) if last.span.0 <= rewrite.span.0 && rewrite.span.1 <= last.span.1 => {}
            _ => outermost.push(rewrite),
        }
    }
    Ok(outermost)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewritten(text: &str) -> String {
        let mut output = text.to_owned();
        for rewrite in rewrites(text).unwrap().into_iter().rev() {
            output.replace_range(rewrite.span.0..rewrite.span.1, &rewrite.text);
        }
        syn::parse_file(&output).unwrap_or_else(|error| panic!("{error}\n{output}"));
        output
    }

    #[test]
    fn a_unit_body_ends_in_ok() {
        assert_eq!(
            rewritten("impl A {\n    fn shrink(&mut self) -> core::result::Result<(), Never> {\n    Ok({\n        self.values.shrink_to_fit();\n\n        self.sorted = Vec::new();\n    })\n}\n}\n"),
            "impl A {\n    fn shrink(&mut self) -> core::result::Result<(), Never> {\n        self.values.shrink_to_fit();\n\n        self.sorted = Vec::new();\n\n        Ok(())\n    }\n}\n"
        );
    }

    #[test]
    fn a_unit_body_ending_in_a_decision_keeps_it_as_a_statement() {
        let output = rewritten("fn f(&mut self) -> Result<(), Never> {\nOk({\n    match self.depth == 0 { true => {\n        self.references = 0;\n    }, false => {} }\n})\n}\n");
        assert_eq!(output, "fn f(&mut self) -> Result<(), Never> {\n    match self.depth == 0 { true => {\n        self.references = 0;\n    }, false => {} }\n\n    Ok(())\n}\n");
    }

    #[test]
    fn a_tail_becomes_the_answer() {
        assert_eq!(
            rewritten("    pub fn descendants(&self) -> Result<D, Never> {\n    Ok({\n        let Ok(root) = self.root();\n        let Ok(descendants) = root.descendants();\n\n        descendants\n    })\n}\n"),
            "    pub fn descendants(&self) -> Result<D, Never> {\n        let Ok(root) = self.root();\n        let Ok(descendants) = root.descendants();\n\n        Ok(descendants)\n    }\n"
        );
        assert_eq!(rewritten("fn f() -> Result<u8, Never> { Ok({ 1 }) }\n"), "fn f() -> Result<u8, Never> {\n    Ok(1)\n}\n");
    }

    #[test]
    fn a_decision_body_becomes_a_match_on_its_tail() {
        assert_eq!(
            rewritten("    pub fn is_root(&self) -> Result<NodeMatch, Never> {\n    Ok(match {\n        let Ok(node_type) = self.node_type();\n\n        node_type == NodeType::Root\n    } {true => NodeMatch::Matches, false => NodeMatch::Different})\n}\n"),
            "    pub fn is_root(&self) -> Result<NodeMatch, Never> {\n        let Ok(node_type) = self.node_type();\n\n        Ok(match node_type == NodeType::Root {\n            true => NodeMatch::Matches,\n            false => NodeMatch::Different,\n        })\n    }\n"
        );
    }

    #[test]
    fn a_questioned_tail_keeps_its_question_mark_alone() {
        assert_eq!(
            rewritten("fn f(&self) -> Result<Range, E> {\n    Ok({\n        let end = 1;\n        ShortRange::try_from(self.start..end)?\n    })\n}\n"),
            "fn f(&self) -> Result<Range, E> {\n    let end = 1;\n    let short_range = ShortRange::try_from(self.start..end)?;\n\n    Ok(short_range)\n}\n"
        );
    }

    #[test]
    fn a_body_chosen_by_cfg_is_left() {
        let text = "fn f(a: f32) -> Result<f32, Never> {\n    Ok({\n        #[cfg(feature = \"std\")]\n        {\n            a.floor()\n        }\n        #[cfg(not(feature = \"std\"))]\n        {\n            a\n        }\n    })\n}\n";
        assert!(rewrites(text).unwrap().is_empty());
    }

    #[test]
    fn other_bodies_are_left() {
        let text = "fn f() -> Result<u8, Never> {\n    let a = 1;\n    Ok({ a })\n}\nfn g() -> Result<u8, Never> {\n    Ok(unsafe { h() })\n}\nfn h() -> Result<M, Never> {\n    Ok(match { match k { _ => true } } {true => M::A, false => M::B})\n}\n";
        assert!(rewrites(text).unwrap().is_empty());
    }
}
