//! Convert bounded indexing of fixed-size matrices to iteration over their rows.

use crate::source::Text;
use proc_macro2::{TokenStream, TokenTree};
use std::collections::{BTreeMap, BTreeSet};
use syn::{
    Expr, Pat, Type,
    spanned::Spanned,
    visit::{self, Visit},
};

type Bounds = (usize, usize);

pub struct Rewrite {
    pub findings: usize,
    pub edits: Vec<(usize, usize, String)>,
}

#[derive(Clone, Copy)]
struct Array {
    length: usize,
    columns: Option<usize>,
    mutable: bool,
}

fn integer(expression: &Expr) -> Option<usize> {
    let Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(value),
        attrs,
    }) = expression
    else {
        return None;
    };
    if !attrs.is_empty() || !matches!(value.suffix(), "" | "usize") {
        return None;
    }
    value.base10_parse().ok()
}

fn primitive(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.get_ident().is_some_and(|name|
        matches!(name.to_string().as_str(), "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64")))
}

fn declared(ty: &Type, mutable: bool) -> Option<Array> {
    let (ty, mutable) = match ty {
        Type::Reference(reference) => (&*reference.elem, reference.mutability.is_some()),
        ty => (ty, mutable),
    };
    let Type::Array(array) = ty else { return None };
    let columns = match &*array.elem {
        Type::Array(row) if primitive(&row.elem) => Some(integer(&row.len)?),
        ty if primitive(ty) => None,
        _ => return None,
    };
    Some(Array {
        length: integer(&array.len)?,
        columns,
        mutable,
    })
}

fn repeated(expression: &Expr, mutable: bool) -> Option<Array> {
    let Expr::Repeat(array) = expression else {
        return None;
    };
    if !array.attrs.is_empty() {
        return None;
    }
    let (item, columns) = match &*array.expr {
        Expr::Repeat(row) if row.attrs.is_empty() => (&*row.expr, Some(integer(&row.len)?)),
        item => (item, None),
    };
    let Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(value),
        attrs,
    }) = item
    else {
        return None;
    };
    if !attrs.is_empty()
        || !matches!(
            value.suffix(),
            "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64"
        )
    {
        return None;
    }
    Some(Array {
        length: integer(&array.len)?,
        columns,
        mutable,
    })
}

fn binding(pattern: &Pat) -> Option<&syn::PatIdent> {
    match pattern {
        Pat::Ident(binding)
            if binding.attrs.is_empty() && binding.by_ref.is_none() && binding.subpat.is_none() =>
        {
            Some(binding)
        }
        _ => None,
    }
}

fn name(expression: &Expr) -> Option<&syn::Ident> {
    match expression {
        Expr::Path(path) if path.attrs.is_empty() && path.qself.is_none() => path.path.get_ident(),
        _ => None,
    }
}

fn multiplication(expression: &Expr, counter: &syn::Ident) -> Option<usize> {
    let Expr::Call(call) = expression else {
        return None;
    };
    let Expr::Path(function) = &*call.func else {
        return None;
    };
    let parts = function
        .path
        .segments
        .iter()
        .map(|part| part.ident.to_string())
        .collect::<Vec<_>>();
    let builtin = parts.iter().map(String::as_str).collect::<Vec<_>>();
    if function.qself.is_some()
        || !matches!(
            builtin.as_slice(),
            ["usize", "wrapping_mul"] | ["core", "primitive", "usize", "wrapping_mul"]
        )
        || !call.attrs.is_empty()
        || call.args.len() != 2
    {
        return None;
    }
    let mut operands = call.args.iter();
    let first = operands.next()?;
    let second = operands.next()?;
    if name(first) == Some(counter) {
        return integer(second);
    }
    (name(second) == Some(counter))
        .then(|| integer(first))
        .flatten()
}

fn flat_offset(expression: &Expr, counter: &syn::Ident) -> Option<(usize, usize)> {
    if let Expr::MethodCall(add) = expression
        && add.attrs.is_empty()
        && add.method == "wrapping_add"
        && add.turbofish.is_none()
        && add.args.len() == 1
    {
        return Some((
            multiplication(&add.receiver, counter)?,
            integer(add.args.first()?)?,
        ));
    }
    Some((multiplication(expression, counter)?, 0))
}

#[derive(Default)]
struct Bindings {
    names: BTreeMap<String, usize>,
    statement_macros: Vec<proc_macro2::Span>,
    explicit_mutable_borrow: bool,
}

impl<'ast> Visit<'ast> for Bindings {
    fn visit_pat_ident(&mut self, binding: &'ast syn::PatIdent) {
        *self.names.entry(binding.ident.to_string()).or_default() += 1;
        self.explicit_mutable_borrow |= binding.by_ref.is_some() && binding.mutability.is_some();
        visit::visit_pat_ident(self, binding);
    }

    fn visit_stmt_macro(&mut self, statement: &'ast syn::StmtMacro) {
        self.statement_macros.push(statement.span());
    }

    fn visit_item(&mut self, _: &'ast syn::Item) {}
}

#[derive(Clone, Copy)]
struct Access {
    span: Bounds,
    column: usize,
    findings: usize,
}

struct Uses<'a> {
    text: &'a Text<'a>,
    marks: &'a BTreeSet<Bounds>,
    arrays: &'a BTreeMap<String, Array>,
    counter: &'a syn::Ident,
    height: usize,
    accesses: BTreeMap<(String, usize), Vec<Access>>,
    writes: BTreeSet<Bounds>,
    uncertain_places: BTreeSet<Bounds>,
}

impl Uses<'_> {
    fn project(&self, index: &syn::ExprIndex) -> Option<(String, usize, Access)> {
        if !index.attrs.is_empty() {
            return None;
        }
        let (base, width, column) = match &*index.expr {
            Expr::Index(row) if row.attrs.is_empty() && name(&row.index) == Some(self.counter) => {
                let base = name(&row.expr)?.to_string();
                let array = self.arrays.get(&base)?;
                if self.height > array.length {
                    return None;
                }
                (base, array.columns?, integer(&index.index)?)
            }
            expression => {
                let base = name(expression)?.to_string();
                let (width, column) = flat_offset(&index.index, self.counter)?;
                let array = self.arrays.get(&base)?;
                if array.columns.is_some() || self.height.checked_mul(width)? > array.length {
                    return None;
                }
                (base, width, column)
            }
        };
        if column >= width || width > 32 {
            return None;
        }
        let span = self.text.span(index.span());
        let said = &self.text.text[span.0..span.1];
        if said.contains("//") || said.contains("/*") {
            return None;
        }
        let findings = self
            .marks
            .range((span.0, 0)..=(span.1, span.1))
            .filter(|(_, end)| *end <= span.1)
            .count();
        let access = Access {
            span,
            column,
            findings,
        };
        (findings != 0).then_some((base, width, access))
    }

    fn place(&self, mut expression: &Expr) -> Bounds {
        while let Expr::Paren(inner) = expression {
            expression = &inner.expr;
        }
        self.text.span(expression.span())
    }
}

impl<'ast> Visit<'ast> for Uses<'_> {
    fn visit_expr_index(&mut self, index: &'ast syn::ExprIndex) {
        if let Some((base, width, access)) = self.project(index) {
            self.accesses.entry((base, width)).or_default().push(access);
            return;
        }
        visit::visit_expr_index(self, index);
    }

    fn visit_expr_assign(&mut self, assign: &'ast syn::ExprAssign) {
        self.writes.insert(self.place(&assign.left));
        visit::visit_expr_assign(self, assign);
    }

    fn visit_expr_binary(&mut self, expression: &'ast syn::ExprBinary) {
        if matches!(
            expression.op,
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
        ) {
            self.writes.insert(self.place(&expression.left));
        }
        visit::visit_expr_binary(self, expression);
    }

    fn visit_expr_reference(&mut self, borrow: &'ast syn::ExprReference) {
        if borrow.mutability.is_some() {
            self.writes.insert(self.place(&borrow.expr));
        }
        visit::visit_expr_reference(self, borrow);
    }

    fn visit_expr_method_call(&mut self, method: &'ast syn::ExprMethodCall) {
        self.uncertain_places.insert(self.place(&method.receiver));
        visit::visit_expr_method_call(self, method);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let mut binding = Bindings::default();
        binding.visit_pat(&local.pat);
        if binding.explicit_mutable_borrow
            && let Some(init) = &local.init
        {
            self.uncertain_places.insert(self.place(&init.expr));
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_match(&mut self, expression: &'ast syn::ExprMatch) {
        let mut binding = Bindings::default();
        for arm in &expression.arms {
            binding.visit_pat(&arm.pat);
        }
        if binding.explicit_mutable_borrow {
            self.uncertain_places.insert(self.place(&expression.expr));
        }
        visit::visit_expr_match(self, expression);
    }
}

struct Loops<'a> {
    text: &'a Text<'a>,
    marks: &'a BTreeSet<Bounds>,
    tokens: &'a BTreeMap<String, Vec<usize>>,
    bindings: Bindings,
    arrays: BTreeMap<String, Array>,
    rewrites: &'a mut Vec<Rewrite>,
}

impl Loops<'_> {
    fn declare(&mut self, pattern: &syn::PatIdent, array: Option<Array>) {
        let name = pattern.ident.to_string();
        if let Some(array) = array
            && self.bindings.names.get(&name) == Some(&1)
        {
            self.arrays.insert(name, array);
        }
    }

    fn outside(&self, ident: &str, body: Bounds, accesses: &[Access]) -> usize {
        self.tokens
            .get(ident)
            .into_iter()
            .flatten()
            .filter(|offset| {
                body.0 <= **offset
                    && **offset < body.1
                    && !accesses
                        .iter()
                        .any(|access| access.span.0 <= **offset && **offset < access.span.1)
            })
            .count()
    }

    fn row_loop(&self, walk: &syn::ExprForLoop) -> Option<Rewrite> {
        let counter = binding(&walk.pat)?;
        let Expr::Range(range) = &*walk.expr else {
            return None;
        };
        if counter.mutability.is_some()
            || !walk.attrs.is_empty()
            || !range.attrs.is_empty()
            || !matches!(range.limits, syn::RangeLimits::HalfOpen(_))
            || integer(range.start.as_deref()?)? != 0
        {
            return None;
        }
        let height = integer(range.end.as_deref()?)?;
        let mut bindings = Bindings::default();
        bindings.visit_block(&walk.body);
        if height == 0 || bindings.names.contains_key(&counter.ident.to_string()) {
            return None;
        }
        let body = self.text.span(walk.body.span());
        let begin = self.text.span(walk.pat.span()).0;
        let end = self.text.span(walk.expr.span()).1;
        let header = &self.text.text[begin..end];
        if header.contains("//") || header.contains("/*") {
            return None;
        }
        let mut uses = Uses {
            text: self.text,
            marks: self.marks,
            arrays: &self.arrays,
            counter: &counter.ident,
            height,
            accesses: BTreeMap::new(),
            writes: BTreeSet::new(),
            uncertain_places: BTreeSet::new(),
        };
        uses.visit_block(&walk.body);
        let mut iterators = Vec::new();
        let mut pattern = String::new();
        let mut destructures = String::new();
        let mut edits = Vec::new();
        let mut findings = 0;
        let mut all_accesses = Vec::new();
        for ((base, width), accesses) in uses.accesses {
            if accesses.len() < 2
                || self.outside(&base, body, &accesses) != 0
                || self.bindings.statement_macros.iter().any(|span| {
                    let span = self.text.span(*span);
                    span.0 < begin && self.outside(&base, span, &[]) != 0
                })
                || accesses.iter().any(|access| {
                    let within = |&(lo, hi): &Bounds| lo <= access.span.0 && access.span.1 <= hi;
                    uses.uncertain_places.iter().any(within)
                        || uses
                            .writes
                            .iter()
                            .any(|span| *span != access.span && within(span))
                })
            {
                continue;
            }
            let array = self.arrays[&base];
            let mutable = accesses
                .iter()
                .any(|access| uses.writes.contains(&access.span));
            if mutable && !array.mutable {
                continue;
            }
            let columns: BTreeSet<_> = accesses.iter().map(|access| access.column).collect();
            let prefix = base.trim_start_matches("r#");
            let row_name = format!("{prefix}_row");
            let lanes: Vec<_> = (0..width)
                .map(|offset| match columns.contains(&offset) {
                    true => format!("{prefix}_lane_{offset}"),
                    false => "_".to_owned(),
                })
                .collect();
            if self.tokens.contains_key(&row_name)
                || lanes
                    .iter()
                    .any(|lane| lane != "_" && self.tokens.contains_key(lane))
            {
                continue;
            }
            let (storage, rows) = match array.columns {
                Some(_) => (base.clone(), array.length),
                None => {
                    let chunks = if mutable {
                        "as_chunks_mut"
                    } else {
                        "as_chunks"
                    };
                    (
                        format!("{base}.{chunks}::<{width}>().0"),
                        array.length / width,
                    )
                }
            };
            let iteration = if mutable { "iter_mut" } else { "iter" };
            let take = if rows > height {
                format!(".take({height})")
            } else {
                String::new()
            };
            iterators.push(format!("{storage}.{iteration}(){take}"));
            pattern = if pattern.is_empty() {
                row_name.clone()
            } else {
                format!("({pattern}, {row_name})")
            };
            let indent = self.text.indent_at(begin);
            if width <= 4 {
                destructures.push_str(&format!(
                    "\n{indent}    let [{}] = {row_name};",
                    lanes.join(", ")
                ));
            } else {
                destructures.push_str(&format!("\n{indent}    let ["));
                for line in lanes.chunks(4) {
                    destructures.push_str(&format!("\n{indent}        {},", line.join(", ")));
                }
                destructures.push_str(&format!("\n{indent}    ] = {row_name};"));
            }
            for access in &accesses {
                edits.push((
                    access.span.0,
                    access.span.1,
                    format!("*{}", lanes[access.column]),
                ));
                findings += access.findings;
            }
            all_accesses.extend(accesses);
        }
        let mut parts = iterators.into_iter();
        let first = parts.next()?;
        let iterator = parts.fold(first, |previous, next| format!("{previous}.zip({next})"));
        let counter_name = counter.ident.to_string();
        let header = if self.outside(&counter_name, body, &all_accesses) == 0 {
            format!("{pattern} in {iterator}")
        } else {
            format!("({counter_name}, {pattern}) in {iterator}.enumerate()")
        };
        edits.push((begin, end, header));
        let brace = self.text.span(walk.body.brace_token.span.open()).1;
        if !self.text.text[brace..].starts_with('\n') {
            destructures.push('\n');
        }
        edits.push((brace, brace, destructures));
        Some(Rewrite { findings, edits })
    }
}

impl<'ast> Visit<'ast> for Loops<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        let outer = self.arrays.clone();
        visit::visit_block(self, block);
        self.arrays = outer;
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        visit::visit_local(self, local);
        if !local.attrs.is_empty() {
            return;
        }
        match &local.pat {
            Pat::Type(typed) => {
                if let Some(pattern) = binding(&typed.pat) {
                    self.declare(pattern, declared(&typed.ty, pattern.mutability.is_some()));
                }
            }
            pattern => {
                if let Some(pattern) = binding(pattern) {
                    let array = local
                        .init
                        .as_ref()
                        .and_then(|init| repeated(&init.expr, pattern.mutability.is_some()));
                    self.declare(pattern, array);
                }
            }
        }
    }

    fn visit_expr_for_loop(&mut self, walk: &'ast syn::ExprForLoop) {
        if let Some(rewrite) = self.row_loop(walk) {
            self.rewrites.push(rewrite);
        }
        visit::visit_expr_for_loop(self, walk);
    }

    fn visit_item(&mut self, _: &'ast syn::Item) {}
}

fn identifiers(stream: TokenStream, text: &Text<'_>, names: &mut BTreeMap<String, Vec<usize>>) {
    for token in stream {
        match token {
            TokenTree::Group(group) => identifiers(group.stream(), text, names),
            TokenTree::Ident(ident) => names
                .entry(ident.to_string())
                .or_default()
                .push(text.span(ident.span()).0),
            _ => {}
        }
    }
}

pub fn rewrites(text: &Text<'_>, marks: &BTreeSet<Bounds>) -> crate::edits::Outcome<Vec<Rewrite>> {
    let file = syn::parse_file(text.text)?;
    let mut tokens = BTreeMap::new();
    identifiers(text.text.parse::<TokenStream>()?, text, &mut tokens);
    let mut rewrites = Vec::new();
    for item in file.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        let mut bindings = Bindings::default();
        bindings.visit_signature(&function.sig);
        bindings.visit_block(&function.block);
        let mut loops = Loops {
            text,
            marks,
            tokens: &tokens,
            bindings,
            arrays: BTreeMap::new(),
            rewrites: &mut rewrites,
        };
        for input in &function.sig.inputs {
            if let syn::FnArg::Typed(input) = input
                && input.attrs.is_empty()
                && let Some(pattern) = binding(&input.pat)
            {
                loops.declare(pattern, declared(&input.ty, pattern.mutability.is_some()));
            }
        }
        loops.visit_block(&function.block);
    }
    Ok(rewrites)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct IndexMarks<'a> {
        text: &'a Text<'a>,
        marks: BTreeSet<Bounds>,
    }

    impl<'ast> Visit<'ast> for IndexMarks<'_> {
        fn visit_expr_index(&mut self, index: &'ast syn::ExprIndex) {
            self.marks.insert(self.text.span(index.span()));
            visit::visit_expr_index(self, index);
        }
    }

    fn planned(source: &str) -> (usize, String) {
        let text = Text::new(source);
        let mut marks = IndexMarks {
            text: &text,
            marks: BTreeSet::new(),
        };
        marks.visit_file(&syn::parse_file(source).unwrap());
        let plans = rewrites(&text, &marks.marks).unwrap();
        let findings = plans.iter().map(|plan| plan.findings).sum();
        let mut edits: Vec<_> = plans.into_iter().flat_map(|plan| plan.edits).collect();
        edits.sort_unstable_by_key(|(lo, hi, _)| (*lo, *hi));
        let mut result = source.to_owned();
        for (lo, hi, replacement) in edits.into_iter().rev() {
            result.replace_range(lo..hi, &replacement);
        }
        syn::parse_file(&result).unwrap();
        (findings, result)
    }

    #[test]
    fn flat_and_nested_arrays_share_an_enumerated_row_loop() {
        let (findings, result) = planned(
            "
fn sample(flat: &[i32; 9], matrix: &mut [[i32; 2]; 3]) {
    for row in 0..2 {
        matrix[row][0] = flat[usize::wrapping_mul(row, 3).wrapping_add(1)];
        matrix[row][1] = flat[::core::primitive::usize::wrapping_mul(3, row)] + row as i32;
    }
}",
        );
        assert_eq!(findings, 6);
        assert!(result.contains(
            "flat.as_chunks::<3>().0.iter().take(2).zip(matrix.iter_mut().take(2)).enumerate()"
        ));
        assert!(result.contains("let [flat_lane_0, flat_lane_1, _] = flat_row;"));
        assert!(result.contains("*matrix_lane_1 = *flat_lane_0 + row as i32;"));
    }

    #[test]
    fn preserves_loops_without_a_bounds_or_binding_proof() {
        let expected = "
fn unchanged(short: &[i32; 2], cells: [[i32; 3]; 3]) {
    for row in 0..3 {
        use_values(short[usize::wrapping_mul(row, 2)], short[usize::wrapping_mul(row, 2).wrapping_add(1)]);
    }
    for mut row in 0..3 {
        row = 1;
        use_values(cells[row][0], cells[row][1]);
    }
    for row in 0..3 {
        let row = 1;
        use_values(cells[row][0], cells[row][1]);
    }
    for row in 0..3 {
        hidden!(cells);
        use_values(cells[row][0], cells[row][1]);
    }
}";
        assert_eq!(planned(expected), (0, expected.to_owned()));
    }

    #[test]
    fn preserves_unresolved_places_and_macro_bindings() {
        let source = "
fn unchanged(mut matrix: [[i32; 2]; 3], cells: [[i32; 2]; 3]) {
    shadow!(cells);
    for row in 0..3 {
        use_values(cells[row][0], cells[row][1]);
    }
    for row in 0..3 {
        (matrix[row][0], matrix[row][1]) = (0, 1);
    }
    for row in 0..3 {
        let ref mut cell = matrix[row][1];
        *cell = 0;
        use_value(matrix[row][0]);
    }
}";
        assert_eq!(planned(source), (0, source.to_owned()));
    }
}
