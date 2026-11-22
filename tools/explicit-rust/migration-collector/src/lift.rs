//! Lift a value out of an expression into a `let` of its own, before the statement that used it.
//!
//! Three rules ask for the same edit. EXPLICIT017 wants `frame(settle(x)?, y)` written
//! `let settled = settle(x)?;` first; EXPLICIT002 leaves every caller of a function that now answers
//! `Result<T, Never>` holding a value to take apart, `let Ok(width) = self.width();`; and the
//! number conversions that replace `as` answer the same way. This is the one place that edit is
//! made, so the three read the same.
//!
//! A value may only move when moving it changes nothing a program could see. The walk follows the
//! language's evaluation order through each statement and lifts a value only when everything
//! evaluated before it in the statement is a path, a literal, a field, a reference or a structure of
//! those, and only when it is evaluated every time the statement is: never out of the right side of
//! `&&` or `||`, a match guard, an `else if` condition, a loop condition, a macro whose arguments are
//! not always evaluated, or an expression carrying attributes. A value in the body of a closure or a
//! match arm written without braces is lifted inside that body, which is given braces for it. A
//! value that cannot move is reported and left where it is.
//!
//! A lifted value is named after what it holds (see `names`), never numbered. A statement that
//! decides something keeps the blank line EXPLICIT013 asks for above it, below the new `let`, and a
//! block's tail keeps one too. A `cfg` on the statement goes onto each `let` lifted out of it.

use crate::edits::{Outcome, Plan, Span};
use crate::names::Namer;
use crate::source::{self, Text};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, Member, Pat, Stmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binding {
    Question,
    Irrefutable,
}

#[derive(Clone, Debug)]
pub struct Target {
    pub span: (usize, usize),
    pub init: (usize, usize),
    pub binding: Binding,
    pub unit: bool,
    pub declared: Option<String>,
}

impl Target {
    pub fn at(span: (usize, usize), binding: Binding) -> Self {
        Self { span, init: span, binding, unit: false, declared: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Lifted,
    Wrapped,
    Left(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RootKind {
    Statement,
    Tail,
    Body,
}

#[derive(Clone, Debug)]
struct Root {
    kind: RootKind,
    span: (usize, usize),
    cfgs: Vec<String>,
    decides: bool,
    comma: Option<(usize, usize)>,
}

#[derive(Clone, Debug)]
enum Form {
    Lift { name: String, shorthand: Option<(usize, usize)> },
    Pattern { pattern: (usize, usize), annotated: Option<(usize, usize)> },
    Statement,
}

#[derive(Clone, Debug)]
struct Placement {
    target: usize,
    root: usize,
    form: Form,
}

pub struct Analysis {
    pub decisions: Vec<Option<Decision>>,
    roots: Vec<Root>,
    placements: Vec<Placement>,
}

struct Order {
    root: usize,
    movable: bool,
    pure: bool,
    read: BTreeSet<String>,
}

impl Order {
    fn at(root: usize) -> Self {
        Self { root, movable: true, pure: true, read: BTreeSet::new() }
    }
}

fn named(expression: &Expr) -> BTreeSet<String> {
    let mut idents = Idents(BTreeSet::new());
    idents.visit_expr(expression);
    idents.0
}

struct Walker<'a> {
    text: &'a Text<'a>,
    targets: &'a [Target],
    exact: BTreeMap<(usize, usize), usize>,
    decisions: Vec<Option<Decision>>,
    roots: Vec<Root>,
    placements: Vec<Placement>,
    taken: BTreeSet<String>,
    chosen: BTreeSet<String>,
    own_type: Option<String>,
}

const ALWAYS: [&str; 12] = [
    "format", "print", "println", "eprint", "eprintln", "format_args", "write", "writeln", "vec", "panic",
    "unreachable", "todo",
];

fn decides(statement: &Stmt) -> bool {
    match statement {
        Stmt::Local(local) => local.init.as_ref().is_some_and(|init| init.diverge.is_some()),
        Stmt::Expr(expression, _) => matches!(
            expression,
            Expr::If(_)
                | Expr::Match(_)
                | Expr::While(_)
                | Expr::ForLoop(_)
                | Expr::Loop(_)
                | Expr::Block(_)
                | Expr::Unsafe(_)
        ),
        Stmt::Item(_) | Stmt::Macro(_) => false,
    }
}

fn attributes(expression: &Expr) -> &[syn::Attribute] {
    match expression {
        Expr::Array(e) => &e.attrs,
        Expr::Assign(e) => &e.attrs,
        Expr::Async(e) => &e.attrs,
        Expr::Await(e) => &e.attrs,
        Expr::Binary(e) => &e.attrs,
        Expr::Block(e) => &e.attrs,
        Expr::Break(e) => &e.attrs,
        Expr::Call(e) => &e.attrs,
        Expr::Cast(e) => &e.attrs,
        Expr::Closure(e) => &e.attrs,
        Expr::Const(e) => &e.attrs,
        Expr::Continue(e) => &e.attrs,
        Expr::Field(e) => &e.attrs,
        Expr::ForLoop(e) => &e.attrs,
        Expr::Group(e) => &e.attrs,
        Expr::If(e) => &e.attrs,
        Expr::Index(e) => &e.attrs,
        Expr::Infer(e) => &e.attrs,
        Expr::Let(e) => &e.attrs,
        Expr::Lit(e) => &e.attrs,
        Expr::Loop(e) => &e.attrs,
        Expr::Macro(e) => &e.attrs,
        Expr::Match(e) => &e.attrs,
        Expr::MethodCall(e) => &e.attrs,
        Expr::Paren(e) => &e.attrs,
        Expr::Path(e) => &e.attrs,
        Expr::Range(e) => &e.attrs,
        Expr::RawAddr(e) => &e.attrs,
        Expr::Reference(e) => &e.attrs,
        Expr::Repeat(e) => &e.attrs,
        Expr::Return(e) => &e.attrs,
        Expr::Struct(e) => &e.attrs,
        Expr::Try(e) => &e.attrs,
        Expr::TryBlock(e) => &e.attrs,
        Expr::Tuple(e) => &e.attrs,
        Expr::Unary(e) => &e.attrs,
        Expr::Unsafe(e) => &e.attrs,
        Expr::While(e) => &e.attrs,
        Expr::Yield(e) => &e.attrs,
        _ => &[],
    }
}

fn cfgs(text: &Text<'_>, attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
        .map(|attribute| {
            let (lo, hi) = text.span(attribute.span());
            text.text[lo..hi].to_owned()
        })
        .collect()
}

fn irrefutable(choice: &syn::ExprMatch) -> Option<&Expr> {
    let [arm] = choice.arms.as_slice() else { return None };
    let Pat::TupleStruct(ok) = &arm.pat else { return None };
    let Some(Pat::Ident(name)) = ok.elems.first() else { return None };
    let Expr::Path(body) = &*arm.body else { return None };
    let plain = choice.attrs.is_empty()
        && arm.attrs.is_empty()
        && arm.guard.is_none()
        && ok.path.is_ident("Ok")
        && ok.elems.len() == 1
        && name.by_ref.is_none()
        && name.mutability.is_none()
        && name.subpat.is_none()
        && body.attrs.is_empty()
        && body.path.is_ident(&name.ident);
    plain.then_some(&*choice.expr)
}

fn matched(expression: &Expr) -> bool {
    match expression {
        Expr::Match(_) => true,
        Expr::Paren(paren) => matches!(&*paren.expr, Expr::Match(_)),
        _ => false,
    }
}

pub fn unwrapped(expression: &Expr) -> Option<&Expr> {
    let block = match expression {
        Expr::Paren(paren) if paren.attrs.is_empty() => match &*paren.expr {
            Expr::Block(block) => block,
            Expr::Match(choice) => return irrefutable(choice),
            _ => return None,
        },
        Expr::Block(block) => block,
        Expr::Match(choice) => return irrefutable(choice),
        _ => return None,
    };
    if block.label.is_some() || !block.attrs.is_empty() || block.block.stmts.len() != 2 {
        return None;
    }
    let (Stmt::Local(local), Stmt::Expr(Expr::Path(tail), None)) = (&block.block.stmts[0], &block.block.stmts[1]) else {
        return None;
    };
    let Pat::TupleStruct(ok) = &local.pat else { return None };
    let Some(Pat::Ident(name)) = ok.elems.first() else { return None };
    let init = local.init.as_ref()?;
    let plain = local.attrs.is_empty()
        && tail.attrs.is_empty()
        && ok.path.is_ident("Ok")
        && ok.elems.len() == 1
        && name.by_ref.is_none()
        && name.mutability.is_none()
        && name.subpat.is_none()
        && init.diverge.is_none()
        && tail.path.is_ident(&name.ident);
    plain.then_some(&*init.expr)
}

fn inferred(text: &str) -> bool {
    ["into(", "collect(", "parse(", "sum(", "product(", "default(", "from(", "wrapped(", "fitted(", "try_into("]
        .iter()
        .any(|call| text.contains(call))
}

struct Residue<'a> {
    text: &'a Text<'a>,
    found: Vec<Target>,
}

impl<'ast> Visit<'ast> for Residue<'_> {
    fn visit_expr(&mut self, expression: &'ast Expr) {
        let found = unwrapped(expression).map(|init| (self.text.span(expression.span()), self.text.span(init.span()), init));
        match found {
            Some((span, init_span, init))
                if matched(expression)
                    || (!self.text.text[span.0..init_span.0].contains('\n') && !self.text.text[init_span.1..span.1].contains('\n')) =>
            {
                self.found.push(Target { span, init: init_span, binding: Binding::Irrefutable, unit: false, declared: None });
                self.visit_expr(init);
            }
            _ => visit::visit_expr(self, expression),
        }
    }
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
        if let Ok(arguments) = syn::parse::Parser::parse2(parser, mac.tokens.clone()) {
            for argument in &arguments {
                self.visit_expr(argument);
            }
        }
    }
}

pub fn residue(text: &str) -> Outcome<Vec<Target>> {
    let file = syn::parse_file(text)?;
    let lines = Text::new(text);
    let mut finder = Residue { text: &lines, found: vec![] };
    finder.visit_file(&file);
    Ok(finder.found)
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").replace(" <", "<").replace("< ", "<").replace(" >", ">")
}

pub struct Idents(pub BTreeSet<String>);

impl<'ast> Visit<'ast> for Idents {
    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if let Some(ident) = path.path.get_ident() {
            self.0.insert(ident.to_string());
        }
        visit::visit_expr_path(self, path);
    }
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.0.insert(pattern.ident.to_string());
        visit::visit_pat_ident(self, pattern);
    }
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
        if let Ok(arguments) = syn::parse::Parser::parse2(parser, mac.tokens.clone()) {
            for argument in &arguments {
                self.visit_expr(argument);
            }
            return;
        }
        fn walk(tokens: proc_macro2::TokenStream, found: &mut BTreeSet<String>) {
            for token in tokens {
                match token {
                    proc_macro2::TokenTree::Ident(ident) => {
                        found.insert(ident.to_string());
                    }
                    proc_macro2::TokenTree::Group(group) => walk(group.stream(), found),
                    proc_macro2::TokenTree::Punct(_) | proc_macro2::TokenTree::Literal(_) => {}
                }
            }
        }
        walk(mac.tokens.clone(), &mut self.0);
        for segment in &mac.path.segments {
            self.0.insert(segment.ident.to_string());
        }
    }
}

impl<'a> Walker<'a> {
    fn span_of(&self, node: &impl Spanned) -> (usize, usize) {
        self.text.span(node.span())
    }

    fn target(&self, span: (usize, usize)) -> Option<usize> {
        self.exact.get(&span).copied().filter(|index| self.decisions[*index].is_none())
    }

    fn leave(&mut self, span: (usize, usize), reason: &'static str) {
        for (index, target) in self.targets.iter().enumerate() {
            if self.decisions[index].is_none() && span.0 <= target.span.0 && target.span.1 <= span.1 {
                self.decisions[index] = Some(Decision::Left(reason));
            }
        }
    }

    fn root(&mut self, kind: RootKind, span: (usize, usize), cfgs: Vec<String>, decides: bool) -> usize {
        self.roots.push(Root { kind, span, cfgs, decides, comma: None });
        self.roots.len() - 1
    }

    fn function(&mut self, inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]>, block: &syn::Block) {
        let saved = (std::mem::take(&mut self.taken), std::mem::take(&mut self.chosen));
        let mut idents = Idents(BTreeSet::new());
        for input in inputs {
            idents.visit_fn_arg(input);
        }
        idents.visit_block(block);
        self.taken = idents.0;
        self.block(block);
        (self.taken, self.chosen) = saved;
    }

    fn block(&mut self, block: &syn::Block) {
        let outer = self.chosen.clone();
        let last = block.stmts.len().saturating_sub(1);
        for (index, statement) in block.stmts.iter().enumerate() {
            let tail = index == last && matches!(statement, Stmt::Expr(_, None));
            self.statement(statement, tail);
        }
        self.chosen = outer;
    }

    fn statement(&mut self, statement: &Stmt, tail: bool) {
        let span = self.span_of(statement);
        let kind = match tail {
            true => RootKind::Tail,
            false => RootKind::Statement,
        };
        match statement {
            Stmt::Local(local) => {
                let found = cfgs(self.text, &local.attrs);
                let root = self.root(kind, span, found, decides(statement));
                let mut order = Order::at(root);
                let Some(init) = &local.init else { return };
                let at = self.span_of(&*init.expr);
                let whole = self.target(at).filter(|index| {
                    self.targets[*index].binding == Binding::Irrefutable && init.diverge.is_none()
                });
                match whole {
                    Some(index) => {
                        let value = unwrapped(&init.expr).unwrap_or(&init.expr);
                        let (pattern, annotated) = match &local.pat {
                            Pat::Type(typed) => {
                                let written = self.span_of(&*typed.ty);
                                let droppable = match &self.targets[index].declared {
                                    Some(declared) => normalized(declared) == normalized(&self.text.text[written.0..written.1]),
                                    None => unwrapped(&init.expr).is_some() && !inferred(&self.text.text[at.0..at.1]),
                                };
                                match droppable {
                                    true => (self.span_of(&*typed.pat), Some(self.span_of(typed))),
                                    false => (self.span_of(&local.pat), None),
                                }
                            }
                            other => (self.span_of(other), None),
                        };
                        let typed = matches!(&local.pat, Pat::Type(_)) && annotated.is_none();
                        match typed {
                            true => self.order(&init.expr, &mut order),
                            false => {
                                self.children(value, &mut order);
                                self.decisions[index] = Some(Decision::Wrapped);
                                self.placements.push(Placement {
                                    target: index,
                                    root,
                                    form: Form::Pattern { pattern, annotated },
                                });
                            }
                        }
                    }
                    None => self.order(&init.expr, &mut order),
                }
                if let Some((_, otherwise)) = &init.diverge {
                    self.nested(otherwise);
                }
            }
            Stmt::Expr(expression, semi) => {
                let found = cfgs(self.text, attributes(expression));
                let root = self.root(kind, span, found, decides(statement));
                let mut order = Order::at(root);
                let at = self.span_of(expression);
                let whole = self
                    .target(at)
                    .filter(|index| semi.is_some() && self.targets[*index].binding == Binding::Irrefutable);
                match (whole, unwrapped(expression)) {
                    (Some(index), None) => {
                        self.children(expression, &mut order);
                        self.decisions[index] = Some(Decision::Wrapped);
                        self.placements.push(Placement { target: index, root, form: Form::Statement });
                    }
                    (Some(index), Some(value)) => {
                        self.children(value, &mut order);
                        self.decisions[index] = Some(Decision::Wrapped);
                        self.placements.push(Placement { target: index, root, form: Form::Statement });
                    }
                    (None, _) => self.order_statement(expression, &mut order),
                }
            }
            Stmt::Macro(statement) => {
                let found = cfgs(self.text, &statement.attrs);
                let root = self.root(kind, span, found, false);
                let mut order = Order::at(root);
                self.mac(&statement.mac, &mut order);
            }
            Stmt::Item(item) => self.visit_item(item),
        }
    }

    fn order_statement(&mut self, expression: &Expr, order: &mut Order) {
        match attributes(expression).is_empty() {
            true => self.order(expression, order),
            false => {
                let span = self.span_of(expression);
                let target = self.target(span);
                self.children(expression, order);
                if let Some(index) = target {
                    self.decisions[index] = Some(Decision::Left("an attributed expression"));
                }
            }
        }
    }

    fn nested(&mut self, expression: &Expr) -> Option<usize> {
        let span = self.span_of(expression);
        match expression {
            Expr::Block(block) if block.label.is_none() && block.attrs.is_empty() && self.target(span).is_none() => {
                self.block(&block.block);
                None
            }
            other => Some(self.body(other)),
        }
    }

    fn body(&mut self, expression: &Expr) -> usize {
        let span = self.span_of(expression);
        let root = self.root(RootKind::Body, span, vec![], false);
        let outer = self.chosen.clone();
        let mut order = Order::at(root);
        self.order(expression, &mut order);
        self.chosen = outer;
        root
    }

    fn order(&mut self, expression: &Expr, order: &mut Order) {
        let span = self.span_of(expression);
        if !attributes(expression).is_empty() {
            self.leave(span, "an attributed expression");
            order.pure = false;
            return;
        }
        match self.target(span) {
            Some(index) => self.consider(index, expression, order, None),
            None => {
                self.children(expression, order);
                if effects(expression) {
                    order.pure = false;
                }
            }
        }
    }

    fn consider(&mut self, index: usize, expression: &Expr, order: &mut Order, field: Option<(&syn::Ident, (usize, usize))>) {
        let before = order.pure;
        let read = order.read.clone();
        self.children(unwrapped(expression).unwrap_or(expression), order);
        let reason = match (order.movable, before, read.is_disjoint(&named(expression))) {
            (false, _, _) => Some("evaluated only on some paths"),
            (true, false, _) => Some("something with effects is evaluated before it"),
            (true, true, false) => Some("it names something read before it"),
            (true, true, true) => None,
        };
        let placed = match reason {
            None => self.place(index, order.root, field),
            Some(reason) => {
                self.decisions[index] = Some(Decision::Left(reason));
                false
            }
        };
        match placed {
            true => {
                order.pure = before;
                order.read = read;
            }
            false => order.pure = false,
        }
    }

    fn place(&mut self, index: usize, root: usize, field: Option<(&syn::Ident, (usize, usize))>) -> bool {
        let target = &self.targets[index];
        let said = &self.text.text[target.init.0..target.init.1];
        let parsed = match syn::parse_str::<Expr>(said) {
            Ok(parsed) => parsed,
            Err(_) => {
                self.decisions[index] = Some(Decision::Left("the value does not parse on its own"));
                return false;
            }
        };
        let namer = Namer { taken: &self.taken, own_type: self.own_type.as_deref() };
        let preferred = field
            .map(|(member, _)| member.to_string())
            .filter(|member| !self.taken.contains(member) && !self.chosen.contains(member));
        let (name, shorthand) = match preferred {
            Some(member) => (Some(member), field.map(|(_, span)| span)),
            None => (namer.name(&parsed, &self.chosen), None),
        };
        match name {
            Some(name) => {
                self.chosen.insert(name.clone());
                self.decisions[index] = Some(Decision::Lifted);
                self.placements.push(Placement { target: index, root, form: Form::Lift { name, shorthand } });
                true
            }
            None => {
                self.decisions[index] = Some(Decision::Left("no name can be derived for it"));
                false
            }
        }
    }

    fn conditional(&mut self, expression: &Expr, order: &mut Order) {
        let movable = order.movable;
        order.movable = false;
        self.order(expression, order);
        order.movable = movable;
        order.pure = false;
    }

    fn children(&mut self, expression: &Expr, order: &mut Order) {
        match expression {
            Expr::Array(array) => {
                for element in &array.elems {
                    self.order(element, order);
                }
            }
            Expr::Assign(assign) => {
                self.order(&assign.right, order);
                self.conditional(&assign.left, order);
            }
            Expr::Binary(binary) => {
                use syn::BinOp::*;
                match binary.op {
                    And(_) | Or(_) => {
                        self.order(&binary.left, order);
                        self.conditional(&binary.right, order);
                    }
                    AddAssign(_) | SubAssign(_) | MulAssign(_) | DivAssign(_) | RemAssign(_) | BitXorAssign(_)
                    | BitAndAssign(_) | BitOrAssign(_) | ShlAssign(_) | ShrAssign(_) => {
                        self.conditional(&binary.right, order);
                        self.conditional(&binary.left, order);
                    }
                    _ => {
                        self.order(&binary.left, order);
                        self.order(&binary.right, order);
                    }
                }
            }
            Expr::Block(block) => {
                self.block(&block.block);
                order.pure = false;
            }
            Expr::Unsafe(block) => {
                self.block(&block.block);
                order.pure = false;
            }
            Expr::Break(value) => {
                if let Some(value) = &value.expr {
                    self.order(value, order);
                }
            }
            Expr::Return(value) => {
                if let Some(value) = &value.expr {
                    self.order(value, order);
                }
            }
            Expr::Call(call) => {
                self.order(&call.func, order);
                self.arguments(call.args.iter(), order);
            }
            Expr::MethodCall(call) => {
                self.order(&call.receiver, order);
                self.arguments(call.args.iter(), order);
            }
            Expr::Cast(inner) => self.order(&inner.expr, order),
            Expr::Field(inner) => self.order(&inner.base, order),
            Expr::Group(inner) => self.order(&inner.expr, order),
            Expr::Paren(inner) => self.order(&inner.expr, order),
            Expr::Reference(inner) => self.order(&inner.expr, order),
            Expr::RawAddr(inner) => self.order(&inner.expr, order),
            Expr::Unary(inner) => self.order(&inner.expr, order),
            Expr::Try(inner) => self.order(&inner.expr, order),
            Expr::Let(inner) => self.order(&inner.expr, order),
            Expr::Repeat(inner) => self.order(&inner.expr, order),
            Expr::Index(index) => {
                self.order(&index.expr, order);
                self.order(&index.index, order);
            }
            Expr::Range(range) => {
                if let Some(start) = &range.start {
                    self.order(start, order);
                }
                if let Some(end) = &range.end {
                    self.order(end, order);
                }
            }
            Expr::Tuple(tuple) => {
                for element in &tuple.elems {
                    self.order(element, order);
                }
            }
            Expr::Struct(structure) => {
                for field in &structure.fields {
                    let at = self.span_of(&field.expr);
                    let named = match (&field.member, field.colon_token.is_some()) {
                        (Member::Named(name), true) => Some(name),
                        _ => None,
                    };
                    let whole = self.target(at);
                    match (whole, named, attributes(&field.expr).is_empty()) {
                        (Some(index), Some(name), true) => {
                            let span = self.span_of(field);
                            self.consider(index, &field.expr, order, Some((name, span)));
                        }
                        _ => self.order(&field.expr, order),
                    }
                }
                if let Some(rest) = &structure.rest {
                    self.order(rest, order);
                }
            }
            Expr::Closure(closure) => {
                self.nested(&closure.body);
            }
            Expr::Async(block) => {
                let span = self.span_of(block);
                self.leave(span, "an async block");
            }
            Expr::ForLoop(each) => {
                self.order(&each.expr, order);
                self.block(&each.body);
                order.pure = false;
            }
            Expr::Loop(each) => {
                self.block(&each.body);
                order.pure = false;
            }
            Expr::While(each) => {
                let span = self.span_of(&*each.cond);
                self.leave(span, "a loop condition");
                self.block(&each.body);
                order.pure = false;
            }
            Expr::If(branch) => {
                self.order(&branch.cond, order);
                self.block(&branch.then_branch);
                self.otherwise(branch.else_branch.as_ref().map(|(_, otherwise)| &**otherwise));
                order.pure = false;
            }
            Expr::Match(choice) => {
                self.order(&choice.expr, order);
                for arm in &choice.arms {
                    if let Some((_, guard)) = &arm.guard {
                        let span = self.span_of(&**guard);
                        self.leave(span, "a match guard");
                    }
                    let body_span = self.span_of(&*arm.body);
                    if let Some(root) = self.nested(&arm.body) {
                        let rest = &self.text.text[body_span.1..];
                        let skipped = rest.len() - rest.trim_start().len();
                        if rest.trim_start().starts_with(',') {
                            let at = body_span.1 + skipped;
                            self.roots[root].comma = Some((body_span.1, at + 1));
                        }
                    }
                }
                order.pure = false;
            }
            Expr::Macro(mac) => self.mac(&mac.mac, order),
            Expr::Const(_) | Expr::TryBlock(_) | Expr::Yield(_) | Expr::Verbatim(_) | Expr::Await(_) => {
                let span = self.span_of(expression);
                self.leave(span, "an expression the walk does not order");
                order.pure = false;
            }
            Expr::Path(path) => {
                if let Some(first) = path.path.segments.first() {
                    order.read.insert(first.ident.to_string());
                }
            }
            Expr::Lit(_) | Expr::Continue(_) | Expr::Infer(_) => {}
            _ => {
                let span = self.span_of(expression);
                self.leave(span, "an expression the walk does not order");
                order.pure = false;
            }
        }
    }

    fn arguments<'e>(&mut self, arguments: impl Iterator<Item = &'e Expr>, order: &mut Order) {
        for argument in arguments {
            self.order(argument, order);
        }
    }

    fn otherwise(&mut self, branch: Option<&Expr>) {
        match branch {
            Some(Expr::Block(block)) => self.block(&block.block),
            Some(Expr::If(chained)) => {
                let span = self.span_of(&*chained.cond);
                self.leave(span, "an else-if condition");
                self.block(&chained.then_branch);
                self.otherwise(chained.else_branch.as_ref().map(|(_, otherwise)| &**otherwise));
            }
            Some(other) => {
                let span = self.span_of(other);
                self.leave(span, "an else branch the walk does not order");
            }
            None => {}
        }
    }

    fn mac(&mut self, mac: &syn::Macro, order: &mut Order) {
        let name = mac.path.segments.last().map(|segment| segment.ident.to_string()).unwrap_or_default();
        let span = self.span_of(mac);
        let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
        let always = match name.as_str() {
            "assert" => Some(1),
            "assert_eq" | "assert_ne" => Some(2),
            "matches" => Some(1),
            word if ALWAYS.contains(&word) => Some(usize::MAX),
            _ => None,
        };
        let Some(always) = always else {
            self.leave(span, "a macro whose arguments may not be evaluated");
            order.pure = false;
            return;
        };
        let parsed = match name.as_str() {
            "matches" => syn::parse::Parser::parse2(
                |input: syn::parse::ParseStream<'_>| {
                    let first: Expr = input.parse()?;
                    let _: proc_macro2::TokenStream = input.parse()?;
                    Ok(vec![first])
                },
                mac.tokens.clone(),
            ),
            _ => syn::parse::Parser::parse2(parser, mac.tokens.clone()).map(|list| list.into_iter().collect()),
        };
        let Ok(arguments) = parsed else {
            self.leave(span, "a macro whose arguments do not parse as expressions");
            order.pure = false;
            return;
        };
        for (index, argument) in arguments.iter().enumerate() {
            let argument = match argument {
                Expr::Assign(named) if index > 0 => &*named.right,
                other => other,
            };
            match index < always {
                true => self.order(argument, order),
                false => {
                    let span = self.span_of(argument);
                    self.leave(span, "a macro argument evaluated only on failure");
                }
            }
        }
        order.pure = false;
    }
}

fn effects(expression: &Expr) -> bool {
    match expression {
        Expr::Call(_)
        | Expr::MethodCall(_)
        | Expr::Macro(_)
        | Expr::Assign(_)
        | Expr::Try(_)
        | Expr::Await(_)
        | Expr::Break(_)
        | Expr::Continue(_)
        | Expr::Return(_)
        | Expr::Block(_)
        | Expr::Unsafe(_)
        | Expr::If(_)
        | Expr::Match(_)
        | Expr::Loop(_)
        | Expr::While(_)
        | Expr::ForLoop(_)
        | Expr::Yield(_)
        | Expr::TryBlock(_)
        | Expr::Verbatim(_) => true,
        Expr::Binary(binary) => matches!(
            binary.op,
            syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                | syn::BinOp::RemAssign(_)
                | syn::BinOp::BitXorAssign(_)
                | syn::BinOp::BitAndAssign(_)
                | syn::BinOp::BitOrAssign(_)
                | syn::BinOp::ShlAssign(_)
                | syn::BinOp::ShrAssign(_)
        ),
        _ => false,
    }
}

impl<'ast> Visit<'ast> for Walker<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.function(&item.sig.inputs, &item.block);
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.function(&item.sig.inputs, &item.block);
    }
    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if let Some(block) = &item.default {
            self.function(&item.sig.inputs, block);
        }
    }
    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let saved = self.own_type.take();
        if let syn::Type::Path(path) = &*item.self_ty {
            self.own_type = path.path.segments.last().map(|segment| segment.ident.to_string());
        }
        visit::visit_item_impl(self, item);
        self.own_type = saved;
    }
}

pub fn analyse(text: &str, targets: &[Target]) -> Outcome<Analysis> {
    let file = syn::parse_file(text)?;
    let lines = Text::new(text);
    let exact = targets.iter().enumerate().map(|(index, target)| (target.span, index)).collect();
    let mut walker = Walker {
        text: &lines,
        targets,
        exact,
        decisions: vec![None; targets.len()],
        roots: vec![],
        placements: vec![],
        taken: BTreeSet::new(),
        chosen: BTreeSet::new(),
        own_type: None,
    };
    walker.visit_file(&file);
    let mut decisions = walker.decisions;
    for decision in &mut decisions {
        if decision.is_none() {
            *decision = Some(Decision::Left("not inside a function body"));
        }
    }
    Ok(Analysis { decisions, roots: walker.roots, placements: walker.placements })
}

fn line_above(lines: &Text<'_>, at: usize) -> usize {
    let mut line = lines.line_of(at);
    while line > 0 {
        let above = lines.line(line - 1).trim_start();
        match above.starts_with("//") || above.starts_with("/*") {
            true => line -= 1,
            false => break,
        }
    }
    lines.line_start(line)
}

pub fn register(plan: &mut Plan, file: &Path, text: &str, targets: &[Target], analysis: &Analysis, tag: &str) -> Outcome<()> {
    let lines = Text::new(text);
    let mut by_root = BTreeMap::<usize, Vec<&Placement>>::new();
    for placement in &analysis.placements {
        by_root.entry(placement.root).or_default().push(placement);
    }
    let span = |lo: usize, hi: usize| Span { file: file.to_path_buf(), lo, hi };
    for (root_index, placements) in by_root {
        let root = analysis.roots[root_index].clone();
        let mut lets = vec![];
        let mut whole = None;
        for placement in &placements {
            let target = &targets[placement.target];
            match &placement.form {
                Form::Lift { name, shorthand } => {
                    let pattern = match target.binding {
                        Binding::Question => name.clone(),
                        Binding::Irrefutable => format!("Ok({name})"),
                    };
                    let from = lines.indent_at(target.init.0).to_owned();
                    lets.push((pattern, target.span, target.init, from));
                    let replaced = name.clone();
                    match (shorthand, root.kind == RootKind::Body && target.span == root.span) {
                        (_, true) => whole = Some(name.clone()),
                        (Some((lo, hi)), false) => {
                            plan.add(span(*lo, *hi), format!("{tag}:shorthand"), move |_, _| Ok(replaced.clone()))?
                        }
                        (None, false) => plan.add(
                            span(target.span.0, target.span.1),
                            format!("{tag}:named"),
                            move |_, _| Ok(replaced.clone()),
                        )?,
                    }
                }
                Form::Pattern { pattern, annotated } => {
                    if target.span != target.init {
                        let init = span(target.init.0, target.init.1);
                        plan.add(span(target.span.0, target.span.1), format!("{tag}:unwrapped"), move |plan, _| {
                            plan.get(&init)
                        })?;
                    }
                    let (lo, hi) = annotated.unwrap_or(*pattern);
                    let inner = span(pattern.0, pattern.1);
                    let annotated = annotated.is_some();
                    plan.add(span(lo, hi), format!("{tag}:pattern"), move |plan, said| {
                        Ok(match annotated {
                            true => format!("Ok({})", plan.get(&inner)?),
                            false => format!("Ok({said})"),
                        })
                    })?;
                }
                Form::Statement => {
                    let bound = match target.unit || target.span != target.init {
                        true => "let Ok(()) = ",
                        false => "let Ok(_) = ",
                    };
                    if target.span != target.init {
                        let init = span(target.init.0, target.init.1);
                        plan.add(span(target.span.0, target.span.1), format!("{tag}:unwrapped"), move |plan, _| {
                            plan.get(&init)
                        })?;
                    }
                    plan.add(span(target.span.0, target.span.0), format!("{tag}:statement"), move |_, said| {
                        Ok(format!("{said}{bound}"))
                    })?;
                }
            }
        }
        if lets.is_empty() {
            continue;
        }
        let file = file.to_path_buf();
        let render_lets = move |plan: &Plan, indent: &str, cfgs: &[String]| -> Outcome<Vec<String>> {
            let mut said = vec![];
            for (pattern, at, init, from) in &lets {
                let value = match at == init {
                    true => plan.inner(&Span { file: file.clone(), lo: at.0, hi: at.1 })?,
                    false => plan.get(&Span { file: file.clone(), lo: init.0, hi: init.1 })?,
                };
                let value = match indent.is_empty() {
                    true => value,
                    false => source::moved(&value, from, indent),
                };
                for cfg in cfgs {
                    said.push(format!("{indent}{cfg}"));
                }
                said.push(format!("{indent}let {pattern} = {value};"));
            }
            Ok(said)
        };
        match root.kind {
            RootKind::Statement | RootKind::Tail => {
                let starts = lines.starts_line(root.span.0);
                let indent = lines.indent_at(root.span.0).to_owned();
                let gap = root.decides || root.kind == RootKind::Tail;
                let cfgs = root.cfgs.clone();
                match starts {
                    true => {
                        let at = line_above(&lines, root.span.0);
                        plan.add(span(at, at), format!("{tag}:lets"), move |plan, said| {
                            let mut text = said.to_owned();
                            for line in render_lets(plan, &indent, &cfgs)? {
                                text.push_str(&line);
                                text.push('\n');
                            }
                            if gap {
                                text.push('\n');
                            }
                            Ok(text)
                        })?;
                    }
                    false => {
                        let at = root.span.0;
                        plan.add(span(at, at), format!("{tag}:lets"), move |plan, said| {
                            let mut text = said.to_owned();
                            for line in render_lets(plan, "", &cfgs)? {
                                text.push_str(&line);
                                text.push(' ');
                            }
                            Ok(text)
                        })?;
                    }
                }
            }
            RootKind::Body => {
                let indent = lines.indent_at(root.span.0).to_owned();
                let inner = format!("{indent}    ");
                plan.add(span(root.span.0, root.span.1), format!("{tag}:body"), move |plan, said| {
                    let mut text = String::from("{\n");
                    for line in render_lets(plan, &inner, &[])? {
                        text.push_str(&line);
                        text.push('\n');
                    }
                    text.push('\n');
                    text.push_str(&inner);
                    match &whole {
                        Some(name) => text.push_str(name),
                        None => text.push_str(&source::reindent(said, "    ", &source::literals(said), 0)),
                    }
                    text.push('\n');
                    text.push_str(&indent);
                    text.push('}');
                    Ok(text)
                })?;
                if let Some((lo, hi)) = root.comma {
                    plan.add(span(lo, hi), format!("{tag}:comma"), |_, _| Ok(String::new()))?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifted(text: &str, wanted: &[(&str, Binding)]) -> (String, Vec<Option<Decision>>) {
        let mut targets: Vec<Target> = vec![];
        for (said, binding) in wanted {
            let lo = text
                .match_indices(said)
                .map(|(at, _)| at)
                .find(|at| !targets.iter().any(|target| target.span == (*at, at + said.len())))
                .expect("the target is in the text");
            targets.push(Target::at((lo, lo + said.len()), *binding));
        }
        let root = std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let directory = root.join(format!("lift-test-{}-{}", std::process::id(), text.len()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("source.rs");
        std::fs::write(&file, text).unwrap();
        let file = file.canonicalize().unwrap();
        let mut plan = Plan::new(&directory, [&file]).unwrap();
        let analysis = analyse(text, &targets).unwrap();
        register(&mut plan, &file, text, &targets, &analysis, "lift").unwrap();
        let outputs = plan.outputs().unwrap();
        let output = outputs.get(&file).map_or(text.to_owned(), |bytes| String::from_utf8(bytes.clone()).unwrap());
        std::fs::remove_dir_all(&directory).unwrap();
        syn::parse_file(&output).unwrap_or_else(|error| panic!("{error}\n{output}"));
        (output, analysis.decisions)
    }

    fn tidied(text: &str) -> (String, Vec<Option<Decision>>) {
        let targets = residue(text).unwrap();
        let root = std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let directory = root.join(format!("tidy-test-{}-{}", std::process::id(), text.len()));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("source.rs");
        std::fs::write(&file, text).unwrap();
        let file = file.canonicalize().unwrap();
        let mut plan = Plan::new(&directory, [&file]).unwrap();
        let analysis = analyse(text, &targets).unwrap();
        register(&mut plan, &file, text, &targets, &analysis, "tidy").unwrap();
        let outputs = plan.outputs().unwrap();
        let output = outputs.get(&file).map_or(text.to_owned(), |bytes| String::from_utf8(bytes.clone()).unwrap());
        std::fs::remove_dir_all(&directory).unwrap();
        syn::parse_file(&output).unwrap_or_else(|error| panic!("{error}\n{output}"));
        (output, analysis.decisions)
    }

    #[test]
    fn a_wrapped_block_inside_an_expression_becomes_a_statement() {
        let (output, _) = tidied(
            "impl Document {\n    fn descendants(&self) -> Descendants {\n        { let Ok(explicit_value) = ({ let Ok(explicit_value) = self.root(); explicit_value }).descendants(); explicit_value }\n    }\n}\n",
        );
        assert_eq!(
            output,
            "impl Document {\n    fn descendants(&self) -> Descendants {\n        let Ok(root) = self.root();\n        let Ok(descendants) = root.descendants();\n\n        descendants\n    }\n}\n"
        );
    }

    #[test]
    fn a_wrapped_initializer_becomes_the_pattern() {
        let (output, _) = tidied(
            "fn f(text: &str) {\n    let s = &mut ({ let Ok(explicit_value) = Stream::new(text); explicit_value });\n    let width: u32 = ({ let Ok(explicit_value) = size.width(); explicit_value });\n    let n: u64 = ({ let Ok(converted) = fitted(at); converted });\n    g(s, width, n);\n}\n",
        );
        assert!(output.contains("    let Ok(stream) = Stream::new(text);\n    let s = &mut stream;\n"), "{output}");
        assert!(output.contains("    let Ok(width) = size.width();\n"), "{output}");
        assert!(output.contains("    let n: u64 = ({ let Ok(converted) = fitted(at); converted });"), "{output}");
    }

    #[test]
    fn an_arm_made_of_a_wrapped_block_is_spelled_out() {
        let (output, _) = tidied(
            "fn f(e: E) -> TextPos {\n    match e {\n        E::A => { let Ok(explicit_value) = TextPos::new(Row(1), Column(1)); explicit_value },\n        E::B => at,\n    }\n}\n",
        );
        assert!(
            output.contains("        E::A => {\n            let Ok(text_pos) = TextPos::new(Row(1), Column(1));\n\n            text_pos\n        }\n        E::B => at,"),
            "{output}"
        );
    }

    #[test]
    fn a_wrapped_block_in_a_closure_or_after_an_effect_is_handled() {
        let (output, decisions) = tidied(
            "fn f() {\n    let v = items.iter().map(|item| ({ let Ok(explicit_value) = item.width(); explicit_value }) + 1).collect();\n    let w = (touch(), ({ let Ok(explicit_value) = item.height(); explicit_value }));\n}\n",
        );
        assert!(output.contains(".map(|item| {\n        let Ok(width) = item.width();\n\n        width + 1\n    })"), "{output}");
        assert!(matches!(decisions[1], Some(Decision::Left(_))));
    }

    #[test]
    fn sibling_arms_may_each_use_a_name_and_a_dropped_unit_says_so() {
        let (output, _) = tidied(
            "fn f(e: E) -> P {\n    ({ let Ok(explicit_value) = buffer.push_raw(c); explicit_value });\n    match e {\n        E::A => { let Ok(explicit_value) = TextPos::new(1); explicit_value },\n        E::B => { let Ok(explicit_value) = TextPos::new(2); explicit_value },\n    }\n}\n",
        );
        assert!(output.contains("    let Ok(()) = buffer.push_raw(c);\n"), "{output}");
        assert_eq!(output.matches("let Ok(text_pos) = TextPos::new(").count(), 2, "{output}");
    }

    #[test]
    fn an_irrefutable_match_is_the_same_residue() {
        let (output, _) = tidied(
            "fn f(reader: &mut R) -> Option<u8> {\n    let count = (match reader.read_count() {\n        Ok(value) => value,\n    })\n    .ok_or(E::Invalid)?;\n    g((match reader.width() {\n        Ok(value) => value,\n    }));\n    None\n}\n",
        );
        assert!(output.contains("    let Ok(reader_count) = reader.read_count();\n    let count = reader_count\n    .ok_or(E::Invalid)?;\n"), "{output}");
        assert!(output.contains("    let Ok(width) = reader.width();\n    g(width);\n"), "{output}");
    }

    #[test]
    fn a_body_already_spelled_out_is_not_taken_for_residue() {
        let text = "fn f() {\n    let v = items.iter().map(|item| {\n        let Ok(width) = item.width();\n\n        width\n    });\n}\n";
        assert!(residue(text).unwrap().is_empty());
    }

    #[test]
    fn a_wrapped_block_in_an_unknown_macro_is_left() {
        let (output, decisions) = tidied(
            "fn f() {\n    test!(a, ({ let Ok(explicit_value) = Length::new(1.0); explicit_value }));\n    assert!(({ let Ok(explicit_value) = x.is_abs(); explicit_value }));\n}\n",
        );
        assert!(matches!(decisions[0], Some(Decision::Left(_))));
        assert_eq!(decisions[1], Some(Decision::Lifted));
        assert!(output.contains("    let Ok(is_abs) = x.is_abs();\n    assert!(is_abs);"), "{output}");
    }

    #[test]
    fn a_multi_line_value_keeps_its_shape_at_the_new_depth() {
        let (output, _) = tidied(
            "fn f() {\n    g(\n        ({ let Ok(explicit_value) = Rect::from_points(\n            a,\n            b,\n        ); explicit_value }),\n    );\n}\n",
        );
        assert!(output.contains("    let Ok(rect) = Rect::from_points(\n        a,\n        b,\n    );\n    g(\n        rect,\n    );"), "{output}");
    }

    #[test]
    fn a_buried_question_mark_is_lifted_with_its_name() {
        let (output, _) = lifted(
            "fn f(r: &mut R) -> Option<u32> {\n    let total = add(r.width()?, 2);\n    Some(total)\n}\n",
            &[("r.width()?", Binding::Question)],
        );
        assert_eq!(
            output,
            "fn f(r: &mut R) -> Option<u32> {\n    let width = r.width()?;\n    let total = add(width, 2);\n    Some(total)\n}\n"
        );
    }

    #[test]
    fn calls_are_lifted_in_evaluation_order_with_owner_names() {
        let (output, _) = lifted(
            "impl R {\n    fn f(&self, other: &R) -> i32 {\n        let right = self.right().min(other.right());\n        right\n    }\n}\n",
            &[("self.right()", Binding::Irrefutable), ("other.right()", Binding::Irrefutable)],
        );
        assert!(output.contains("        let Ok(own_right) = self.right();\n        let Ok(other_right) = other.right();\n        let right = own_right.min(other_right);\n"), "{output}");
    }

    #[test]
    fn a_whole_initializer_wraps_its_pattern() {
        let (output, decisions) = lifted(
            "fn f() {\n    let mut width = size.width();\n    let height: u32 = size.height();\n    use_it(width, height);\n}\n",
            &[("size.width()", Binding::Irrefutable)],
        );
        assert_eq!(decisions, vec![Some(Decision::Wrapped)]);
        assert!(output.contains("let Ok(mut width) = size.width();"), "{output}");
    }

    #[test]
    fn a_dropped_value_and_a_unit_call_say_so() {
        let text = "fn f() {\n    record(1);\n}\n";
        let lo = text.find("record(1)").unwrap();
        let mut target = Target::at((lo, lo + 9), Binding::Irrefutable);
        target.unit = true;
        let analysis = analyse(text, &[target.clone()]).unwrap();
        assert_eq!(analysis.decisions, vec![Some(Decision::Wrapped)]);
    }

    #[test]
    fn short_circuits_guards_and_prior_effects_are_left() {
        let (output, decisions) = lifted(
            "fn f() -> Option<u8> {\n    let a = ready && read()?;\n    let b = (touch(), read()?);\n    match x { y if check()? => {}, _ => {} }\n    None\n}\n",
            &[("read()?", Binding::Question), ("read()?", Binding::Question), ("check()?", Binding::Question)],
        );
        assert!(decisions.iter().all(|decision| matches!(decision, Some(Decision::Left(_)))), "{decisions:?}");
        assert!(output.contains("ready && read()?"));
    }

    #[test]
    fn a_value_is_not_lifted_past_a_read_of_something_it_names() {
        let (output, decisions) = lifted(
            "impl S {\n    fn f(&mut self) {\n        (self.joiner)(self.radius, self.builders(), other.width());\n        g(limit, self.count(), b.height());\n    }\n}\n",
            &[
                ("self.builders()", Binding::Irrefutable),
                ("other.width()", Binding::Irrefutable),
                ("self.count()", Binding::Irrefutable),
                ("b.height()", Binding::Irrefutable),
            ],
        );
        assert_eq!(decisions[0], Some(Decision::Left("it names something read before it")));
        assert!(matches!(decisions[1], Some(Decision::Left(_))));
        assert_eq!(decisions[2], Some(Decision::Lifted));
        assert!(output.contains("        let Ok(count) = self.count();\n        let Ok(height) = b.height();\n        g(limit, count, height);"), "{output}");
    }

    #[test]
    fn a_decision_keeps_its_blank_line_and_a_tail_gets_one() {
        let (output, _) = lifted(
            "fn f() -> Option<u8> {\n    a();\n\n    match r.kind()? {\n        _ => {}\n    }\n\n    Some(r.byte()?)\n}\n",
            &[("r.kind()?", Binding::Question), ("r.byte()?", Binding::Question)],
        );
        assert_eq!(
            output,
            "fn f() -> Option<u8> {\n    a();\n\n    let kind = r.kind()?;\n\n    match kind {\n        _ => {}\n    }\n\n    let byte = r.byte()?;\n\n    Some(byte)\n}\n"
        );
    }

    #[test]
    fn a_closure_or_arm_body_is_given_braces() {
        let (output, _) = lifted(
            "fn f() {\n    let names = icons.iter().map(|icon| icon.name()).collect();\n    match x {\n        A => g(r.kind()),\n        B => {}\n    }\n}\n",
            &[("icon.name()", Binding::Irrefutable), ("r.kind()", Binding::Irrefutable)],
        );
        assert!(output.contains(".map(|icon| {\n        let Ok(name) = icon.name();\n\n        name\n    })"), "{output}");
        assert!(output.contains("A => {\n            let Ok(kind) = r.kind();\n\n            g(kind)\n        }\n        B"), "{output}");
    }

    #[test]
    fn a_struct_field_takes_the_shorthand() {
        let (output, _) = lifted(
            "fn f() -> Option<S> {\n    Some(S { width: r.read()?, height: 2 })\n}\n",
            &[("r.read()?", Binding::Question)],
        );
        assert!(output.contains("let width = r.read()?;\n\n    Some(S { width, height: 2 })"), "{output}");
    }

    #[test]
    fn cfg_on_the_statement_goes_onto_the_let() {
        let (output, _) = lifted(
            "fn f() -> Option<u8> {\n    #[cfg(feature = \"std\")]\n    g(r.kind()?);\n    None\n}\n",
            &[("r.kind()?", Binding::Question)],
        );
        assert!(output.contains("    #[cfg(feature = \"std\")]\n    let kind = r.kind()?;\n    #[cfg(feature = \"std\")]\n    g(kind);"), "{output}");
    }

    #[test]
    fn nested_values_are_lifted_inner_first() {
        let (output, _) = lifted(
            "fn f() -> Option<u8> {\n    g(r.table()?.entry()?);\n    None\n}\n",
            &[("r.table()?.entry()?", Binding::Question), ("r.table()?", Binding::Question)],
        );
        assert!(output.contains("    let table = r.table()?;\n    let entry = table.entry()?;\n    g(entry);"), "{output}");
    }

    #[test]
    fn a_name_used_later_in_the_function_is_not_reused() {
        let (output, decisions) = lifted(
            "fn f(x: u8) -> Option<u8> {\n    let y = g(x.x()?);\n    Some(y)\n}\n",
            &[("x.x()?", Binding::Question)],
        );
        assert_eq!(decisions, vec![Some(Decision::Left("no name can be derived for it"))]);
        assert!(output.contains("g(x.x()?)"));
    }

    #[test]
    fn macros_that_always_evaluate_are_walked_and_others_are_left() {
        let (output, decisions) = lifted(
            "fn f() -> Option<u8> {\n    assert_eq!(r.kind()?, 2, \"{}\", r.name()?);\n    debug_assert!(r.ready()?);\n    None\n}\n",
            &[("r.kind()?", Binding::Question), ("r.name()?", Binding::Question), ("r.ready()?", Binding::Question)],
        );
        assert_eq!(decisions[0], Some(Decision::Lifted));
        assert!(matches!(decisions[1], Some(Decision::Left(_))));
        assert!(matches!(decisions[2], Some(Decision::Left(_))));
        assert!(output.contains("let kind = r.kind()?;\n    assert_eq!(kind, 2"), "{output}");
    }

    #[test]
    fn evaluation_order_follows_the_language() {
        let (_, decisions) = lifted(
            "fn f() -> Option<u8> {\n    place[at()] = r.kind()?;\n    let v = [r.first()?, r.second()?];\n    None\n}\n",
            &[("r.kind()?", Binding::Question), ("r.first()?", Binding::Question), ("r.second()?", Binding::Question)],
        );
        assert_eq!(decisions, vec![Some(Decision::Lifted), Some(Decision::Lifted), Some(Decision::Lifted)]);
    }

    #[test]
    fn an_inline_block_gets_an_inline_let() {
        let (output, _) = lifted("fn f() -> Option<u8> { g(r.kind()?) }\n", &[("r.kind()?", Binding::Question)]);
        assert_eq!(output, "fn f() -> Option<u8> { let kind = r.kind()?; g(kind) }\n");
    }
}
