//! Repairs read off the rules' own diagnostics, so each edit is at a place a rule named.
//!
//! `spacing` inserts the blank lines EXPLICIT013 offers as machine-applicable suggestions, at the
//! exact positions it suggests. `questions` lifts each `?` EXPLICIT017 reports into a `let` of its
//! own, through `lift`, which keeps evaluation order and conditional execution. `must_use` puts
//! `#[must_use]` on each function EXPLICIT034 reports, directly above its signature.
//!
//! The diagnostics are cargo's JSON messages from a run of the rules; every span is checked against
//! the source text it quotes, so a stale capture is refused rather than applied somewhere else.
#[path = "../bodies.rs"]
pub mod bodies;
#[path = "../edits.rs"]
pub mod edits;
#[path = "../index_rows.rs"]
mod index_rows;
#[path = "../lift.rs"]
pub mod lift;
#[path = "../names.rs"]
pub mod names;
#[path = "../source.rs"]
pub mod source;

use edits::{Outcome, Plan, Span};
use lift::{Binding, Decision, Target};
use serde_json::{json, Value};
use source::Text;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

#[derive(Clone, Debug)]
struct Mark {
    lo: usize,
    hi: usize,
    line: usize,
    quoted: String,
}

fn package_directory(root: &Path, package: &str) -> PathBuf {
    let directory = root.join("crates").join(package);

    match directory.is_dir() {
        true => directory,
        false => root.join("forks").join(package),
    }
}

fn owned(root: &Path, packages: &[String], path: &Path) -> bool {
    packages.iter().any(|package| {
        ["crates", "forks"].iter().any(|family| path.starts_with(root.join(family).join(package)))
    })
}

fn quoted(span: &Value) -> Option<Mark> {
    Some(Mark {
        lo: usize::try_from(span["byte_start"].as_u64()?).ok()?,
        hi: usize::try_from(span["byte_end"].as_u64()?).ok()?,
        line: usize::try_from(span["line_start"].as_u64()?).ok()?,
        quoted: span["text"][0]["text"].as_str()?.to_owned(),
    })
}

fn current(text: &str, mark: &Mark) -> bool {
    let lines = Text::new(text);
    mark.line >= 1
        && lines.line(mark.line - 1) == mark.quoted
        && mark.hi <= text.len()
        && text.is_char_boundary(mark.lo)
        && text.is_char_boundary(mark.hi)
        && lines.line_of(mark.lo) == mark.line - 1
}

fn marks(
    diagnostics: &Path,
    root: &Path,
    packages: &[String],
    code: &str,
    suggestions: bool,
) -> Outcome<BTreeMap<PathBuf, BTreeSet<(usize, usize, usize, String)>>> {
    let mut found = BTreeMap::<PathBuf, BTreeSet<(usize, usize, usize, String)>>::new();
    for line in BufReader::new(fs::File::open(diagnostics)?).lines() {
        let line = line?;
        if !line.contains(code) {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let message = match record.get("message") {
            Some(message) => message,
            None => &record,
        };
        if message["code"]["code"].as_str() != Some(code) {
            continue;
        }
        let mut spans = vec![];
        match suggestions {
            true => {
                for child in message["children"].as_array().into_iter().flatten() {
                    for span in child["spans"].as_array().into_iter().flatten() {
                        if span["suggested_replacement"].as_str() == Some("\n") && span["expansion"].is_null() {
                            spans.push(span.clone());
                        }
                    }
                }
            }
            false => spans.extend(
                message["spans"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|span| span["is_primary"] == true && span["expansion"].is_null())
                    .cloned(),
            ),
        }
        for span in spans {
            let Some(name) = span["file_name"].as_str() else { continue };
            let path = root.join(name).canonicalize()?;
            if !owned(root, packages, &path) {
                continue;
            }
            let Some(mark) = quoted(&span) else { continue };
            found.entry(path).or_default().insert((mark.lo, mark.hi, mark.line, mark.quoted));
        }
    }
    Ok(found)
}

fn spacing(plan: &mut Plan, file: &Path, text: &str, marks: &[Mark], report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    let literals = source::literals(text);
    let mut positions = BTreeSet::new();
    for mark in marks {
        if !current(text, mark) {
            return Err(format!("stale diagnostic at {}:{}; recapture", file.display(), mark.line).into());
        }
        if source::inside_literal(&literals, mark.lo) {
            *report.entry("inside_a_literal".into()).or_default() += 1;
            continue;
        }
        positions.insert(mark.lo);
    }
    for at in positions {
        *report.entry("blank_lines".into()).or_default() += 1;
        plan.add(Span { file: file.to_path_buf(), lo: at, hi: at }, "spacing", |_, said| Ok(format!("{said}\n")))?;
    }
    Ok(())
}

fn explained(file: &Path, text: &str, targets: &[Target], analysis: &lift::Analysis, report: &mut BTreeMap<String, u64>) {
    let explain = env::var_os("REPAIR_EXPLAIN").is_some();
    let lines = Text::new(text);
    for (target, decision) in targets.iter().zip(analysis.decisions.iter().flatten()) {
        let key = match decision {
            Decision::Lifted | Decision::Wrapped => "lifted".to_owned(),
            Decision::Left(reason) => format!("left: {reason}"),
        };
        if explain && key != "lifted" {
            eprintln!("{}:{}: {key}", file.display(), lines.line_of(target.span.0) + 1);
        }
        *report.entry(key).or_default() += 1;
    }
}

fn tidy(plan: &mut Plan, file: &Path, text: &str, report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    let targets = lift::residue(text)?;
    let analysis = lift::analyse(text, &targets)?;
    explained(file, text, &targets, &analysis, report);
    lift::register(plan, file, text, &targets, &analysis, "tidy")
}

fn unwrap_bodies(plan: &mut Plan, file: &Path, text: &str, report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    for rewrite in bodies::rewrites(text)? {
        *report.entry("bodies".into()).or_default() += 1;
        let said = rewrite.text;
        plan.add(Span { file: file.to_path_buf(), lo: rewrite.span.0, hi: rewrite.span.1 }, "bodies", move |_, _| Ok(said.clone()))?;
    }
    Ok(())
}

fn suggested(plan: &mut Plan, file: &Path, text: &str, edits: &[(usize, usize, String)], report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    let literals = source::literals(text);
    let mut placed = BTreeSet::new();
    for (lo, hi, replacement) in edits {
        if *hi > text.len() || source::inside_literal(&literals, *lo) || !placed.insert((*lo, *hi)) {
            continue;
        }
        *report.entry("suggestions".into()).or_default() += 1;
        let replacement = replacement.clone();
        let inserted = lo == hi;
        plan.add(Span { file: file.to_path_buf(), lo: *lo, hi: *hi }, "suggested", move |_, said| {
            Ok(match inserted {
                true => format!("{said}{replacement}"),
                false => replacement.clone(),
            })
        })?;
    }
    Ok(())
}

fn compiler(
    diagnostics: &Path,
    root: &Path,
    packages: &[String],
) -> Outcome<BTreeMap<PathBuf, Vec<(usize, usize, String)>>> {
    let mut found = BTreeMap::<PathBuf, Vec<(usize, usize, String)>>::new();
    for line in fs::read_to_string(diagnostics)?.lines() {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let message = match record.get("message") {
            Some(message) => message,
            None => &record,
        };
        if message["code"]["code"].as_str() == Some("E0308") {
            for span in message["spans"].as_array().into_iter().flatten() {
                let unit = span["is_primary"] == true
                    && span["expansion"].is_null()
                    && span["label"].as_str().is_some_and(|label| label.ends_with("found `()`"));
                let Some(name) = span["file_name"].as_str().filter(|_| unit) else { continue };
                let path = root.join(name).canonicalize()?;
                if !owned(root, packages, &path) {
                    continue;
                }
                let (Some(lo), Some(hi)) = (
                    span["byte_start"].as_u64().and_then(|at| usize::try_from(at).ok()),
                    span["byte_end"].as_u64().and_then(|at| usize::try_from(at).ok()),
                ) else {
                    continue;
                };
                let text = fs::read_to_string(&path)?;
                let bound = text.get(lo..hi) == Some("()") && text.get(lo.saturating_sub(7)..lo) == Some("let Ok(");
                if bound {
                    found.entry(path).or_default().push((lo, hi, "_".to_owned()));
                }
            }
            continue;
        }
        if message["code"]["code"].as_str() != Some("E0596") {
            continue;
        }
        for child in message["children"].as_array().into_iter().flatten() {
            for span in child["spans"].as_array().into_iter().flatten() {
                let wanted = span["suggested_replacement"].as_str() == Some("mut ")
                    && span["byte_start"] == span["byte_end"]
                    && span["expansion"].is_null();
                if !wanted {
                    continue;
                }
                let Some(name) = span["file_name"].as_str() else { continue };
                let path = root.join(name).canonicalize()?;
                if !owned(root, packages, &path) {
                    continue;
                }
                let Some(at) = span["byte_start"].as_u64().and_then(|at| usize::try_from(at).ok()) else { continue };
                found.entry(path).or_default().push((at, at, "mut ".to_owned()));
            }
        }
    }
    Ok(found)
}

fn sources(root: &Path, packages: &[String]) -> Outcome<Vec<PathBuf>> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Outcome<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            match (path.is_dir(), name) {
                (true, "target" | "fixtures") => {}
                (true, _) => walk(&path, found)?,
                (false, _) if name.ends_with(".rs") => found.push(path.canonicalize()?),
                (false, _) => {}
            }
        }
        Ok(())
    }
    let mut found = vec![];
    for package in packages {
        let directory = package_directory(root, package);
        if !directory.is_dir() {
            return Err(format!("no package directory: {}", directory.display()).into());
        }
        walk(&directory, &mut found)?;
    }
    found.sort();
    Ok(found)
}

fn questions(plan: &mut Plan, file: &Path, text: &str, marks: &[Mark], include_residue: bool, report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    let mut targets = vec![];
    for mark in marks {
        if !current(text, mark) {
            return Err(format!("stale diagnostic at {}:{}; recapture", file.display(), mark.line).into());
        }
        targets.push(Target::at((mark.lo, mark.hi), Binding::Question));
    }
    if include_residue {
        let asked = targets.iter().map(|target| target.span).collect::<BTreeSet<_>>();
        targets.extend(lift::residue(text)?.into_iter().filter(|target| !asked.contains(&target.span)));
    }
    let analysis = lift::analyse(text, &targets)?;
    explained(file, text, &targets, &analysis, report);
    lift::register(plan, file, text, &targets, &analysis, "questions")
}

struct Signatures<'a> {
    text: &'a Text<'a>,
    wanted: &'a BTreeSet<(usize, usize)>,
    found: Vec<(usize, usize)>,
}

impl Signatures<'_> {
    fn signature(&mut self, start: proc_macro2::Span, output: &syn::ReturnType, whole: proc_macro2::Span) {
        let (lo, hi) = self.text.span(whole);
        let returned = match output {
            syn::ReturnType::Type(_, ty) => Some(self.text.span(ty.span())),
            syn::ReturnType::Default => None,
        };
        let named = self.wanted.iter().any(|(at, end)| match returned {
            Some(returned) => returned.0 <= *at && *end <= returned.1,
            None => lo <= *at && *end <= hi && *at == *end,
        });
        if named {
            self.found.push((self.text.span(start).0, lo));
        }
    }
}

impl<'ast> Visit<'ast> for Signatures<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let start = match &item.vis {
            syn::Visibility::Inherited => item.sig.span(),
            visibility => visibility.span(),
        };
        self.signature(start, &item.sig.output, item.sig.span());
        visit::visit_item_fn(self, item);
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let start = match &item.vis {
            syn::Visibility::Inherited => item.sig.span(),
            visibility => visibility.span(),
        };
        self.signature(start, &item.sig.output, item.sig.span());
        visit::visit_impl_item_fn(self, item);
    }
    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.signature(item.sig.span(), &item.sig.output, item.sig.span());
        visit::visit_trait_item_fn(self, item);
    }
}

struct Divisions<'a> {
    text: &'a Text<'a>,
    wanted: &'a BTreeSet<(usize, usize)>,
    found: Vec<(usize, usize, String)>,
    left: Vec<(usize, &'static str)>,
}

fn positive_literal(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(value), attrs }) => {
            attrs.is_empty() && value.base10_parse::<u128>().is_ok_and(|value| value > 0)
        }
        syn::Expr::Paren(inner) => positive_literal(&inner.expr),
        _ => false,
    }
}

fn primary(expression: &syn::Expr) -> bool {
    matches!(
        expression,
        syn::Expr::Path(_)
            | syn::Expr::Field(_)
            | syn::Expr::MethodCall(_)
            | syn::Expr::Call(_)
            | syn::Expr::Paren(_)
            | syn::Expr::Index(_)
            | syn::Expr::Macro(_)
            | syn::Expr::Tuple(_)
    )
}

fn simple_place(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Path(path) => path.path.get_ident().is_some(),
        syn::Expr::Field(field) => simple_place(&field.base),
        _ => false,
    }
}

impl<'ast> Visit<'ast> for Divisions<'_> {
    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        let span = self.text.span(binary.span());
        if self.wanted.contains(&span) {
            let method = match binary.op {
                syn::BinOp::Div(_) | syn::BinOp::DivAssign(_) => Some("wrapping_div"),
                syn::BinOp::Rem(_) | syn::BinOp::RemAssign(_) => Some("wrapping_rem"),
                _ => None,
            };
            let assigning = matches!(binary.op, syn::BinOp::DivAssign(_) | syn::BinOp::RemAssign(_));
            let left = self.text.span(binary.left.span());
            let right = self.text.span(binary.right.span());
            let left_text = &self.text.text[left.0..left.1];
            let right_text = &self.text.text[right.0..right.1];
            let decided = match (method, positive_literal(&binary.right), binary.attrs.is_empty()) {
                (None, _, _) => Err("an operator other than division or remainder"),
                (Some(_), false, _) => Err("a divisor that is not a positive literal"),
                (Some(_), true, false) => Err("an attributed expression"),
                (Some(method), true, true) => match (assigning, simple_place(&binary.left), primary(&binary.left)) {
                    (true, true, _) => Ok(format!("{left_text} = {left_text}.{method}({right_text})")),
                    (true, false, _) => Err("a compound assignment to a place that is not a name or a field"),
                    (false, _, true) => Ok(format!("{left_text}.{method}({right_text})")),
                    (false, _, false) => Ok(format!("({left_text}).{method}({right_text})")),
                },
            };
            match decided {
                Ok(said) => self.found.push((span.0, span.1, said)),
                Err(reason) => self.left.push((span.0, reason)),
            }
        }
        visit::visit_expr_binary(self, binary);
    }
}

fn division(plan: &mut Plan, file: &Path, text: &str, marks: &[Mark], report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    for mark in marks {
        if !current(text, mark) {
            return Err(format!("stale diagnostic at {}:{}; recapture", file.display(), mark.line).into());
        }
    }
    let wanted = marks.iter().map(|mark| (mark.lo, mark.hi)).collect::<BTreeSet<_>>();
    let lines = Text::new(text);
    let mut divisions = Divisions { text: &lines, wanted: &wanted, found: vec![], left: vec![] };
    divisions.visit_file(&syn::parse_file(text)?);
    let explain = env::var_os("REPAIR_EXPLAIN").is_some();
    for (at, reason) in &divisions.left {
        if explain {
            eprintln!("{}:{}: left: {reason}", file.display(), lines.line_of(*at) + 1);
        }
        *report.entry(format!("left: {reason}")).or_default() += 1;
    }
    let mut outermost: Vec<(usize, usize, String)> = vec![];
    for (lo, hi, said) in divisions.found {
        match outermost.iter().any(|(outer_lo, outer_hi, _)| *outer_lo <= lo && hi <= *outer_hi) {
            true => *report.entry("left: inside another division, for the next run".into()).or_default() += 1,
            false => outermost.push((lo, hi, said)),
        }
    }
    for (lo, hi, said) in outermost {
        *report.entry("divisions".into()).or_default() += 1;
        plan.add(Span { file: file.to_path_buf(), lo, hi }, "division", move |_, _| Ok(said.clone()))?;
    }
    Ok(())
}

fn index_rows(plan: &mut Plan, file: &Path, text: &str, marks: &[Mark], report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    let lines = Text::new(text);
    let mut wanted = BTreeSet::new();
    for mark in marks {
        if mark.line == 0 || lines.line(mark.line - 1) != mark.quoted
            || mark.lo > mark.hi || mark.hi > text.len()
            || !text.is_char_boundary(mark.lo) || !text.is_char_boundary(mark.hi)
            || lines.line_of(mark.lo) != mark.line - 1
        {
            return Err(format!("stale diagnostic at {}:{}; recapture", file.display(), mark.line).into());
        }
        wanted.insert((mark.lo, mark.hi));
    }
    let rewrites = index_rows::rewrites(&lines, &wanted)?;
    for rewrite in rewrites {
        *report.entry("loops".into()).or_default() += 1;
        *report.entry("indexing_sites".into()).or_default() += rewrite.findings as u64;
        for (lo, hi, replacement) in rewrite.edits {
            plan.add(Span { file: file.to_path_buf(), lo, hi }, "index_rows", move |_, _| Ok(replacement.clone()))?;
        }
    }
    Ok(())
}

fn must_use(plan: &mut Plan, file: &Path, text: &str, marks: &[Mark], report: &mut BTreeMap<String, u64>) -> Outcome<()> {
    for mark in marks {
        if !current(text, mark) {
            return Err(format!("stale diagnostic at {}:{}; recapture", file.display(), mark.line).into());
        }
    }
    let wanted = marks.iter().map(|mark| (mark.lo, mark.hi)).collect::<BTreeSet<_>>();
    let lines = Text::new(text);
    let mut signatures = Signatures { text: &lines, wanted: &wanted, found: vec![] };
    signatures.visit_file(&syn::parse_file(text)?);
    let mut placed = BTreeSet::new();
    for (start, _) in signatures.found {
        if !placed.insert(start) {
            continue;
        }
        let line = lines.line_of(start);
        let indent = lines.indent_of_line(line).to_owned();
        let edit = match lines.starts_line(start) {
            true => (lines.line_start(line), format!("{indent}#[must_use]\n")),
            false => (start, "#[must_use] ".to_owned()),
        };
        *report.entry("must_use".into()).or_default() += 1;
        let said = edit.1;
        plan.add(Span { file: file.to_path_buf(), lo: edit.0, hi: edit.0 }, "must_use", move |_, before| {
            Ok(format!("{before}{said}"))
        })?;
    }
    let missed = wanted.len().saturating_sub(placed.len());
    if missed > 0 {
        *report.entry("signature_not_found".into()).or_default() += missed as u64;
    }
    Ok(())
}

fn run() -> Outcome<()> {
    let mut root = env::current_dir()?;
    let mut diagnostics = None;
    let mut packages = vec![];
    let mut pass = String::new();
    let mut apply = false;
    let mut questions_only = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root needs a path")?),
            "--diagnostics" => diagnostics = Some(PathBuf::from(args.next().ok_or("--diagnostics needs a path")?)),
            "--package" => packages.push(args.next().ok_or("--package needs a package")?),
            "--pass" => pass = args.next().ok_or("--pass needs spacing, questions, must_use, division, index_rows, tidy, bodies or compiler")?,
            "--apply" => apply = true,
            "--questions-only" => questions_only = true,
            "--help" | "-h" => {
                println!("repair [--diagnostics rules.json] --package NAME [--package NAME] --pass spacing|questions|must_use|division|index_rows|tidy|bodies|compiler [--root DIR] [--apply]\nDry runs report prepared edits. Apply saves originals under TMPDIR/builds/explicit-repair.\nWith --pass questions, --questions-only limits the rewrites to EXPLICIT017 diagnostics.\ntidy and bodies read the sources and need no diagnostics; the others read cargo's JSON messages.");
                return Ok(());
            }
            unknown => return Err(format!("unknown option: {unknown}").into()),
        }
    }
    let code = match pass.as_str() {
        "spacing" => "explicit013_breathing_room",
        "questions" => "explicit017_question_mark_alone",
        "must_use" => "explicit034_no_unclaimed_answer",
        "division" => "explicit015_no_bare_arithmetic",
        "index_rows" => "explicit014_no_index_slice",
        "tidy" | "compiler" | "bodies" => "",
        _ => return Err("specify --pass spacing|questions|must_use|division|index_rows|tidy|bodies|compiler".into()),
    };
    if packages.is_empty() {
        return Err("specify --package".into());
    }
    if questions_only && pass != "questions" {
        return Err("--questions-only requires --pass questions".into());
    }
    let root = root.canonicalize()?;
    let mut report = BTreeMap::<String, u64>::new();
    let found = match pass.as_str() {
        "tidy" | "bodies" => {
            let files = sources(&root, &packages)?;
            let mut plan = Plan::new(&root, files.iter())?;
            for file in &files {
                let text = plan.sources[file].text.clone();
                match pass.as_str() {
                    "tidy" => tidy(&mut plan, file, &text, &mut report)?,
                    _ => unwrap_bodies(&mut plan, file, &text, &mut report)?,
                }
            }
            return finish(plan, &pass, report, apply);
        }
        "compiler" => {
            let diagnostics = diagnostics.ok_or("--diagnostics is required")?;
            let edits = compiler(&diagnostics, &root, &packages)?;
            let mut plan = Plan::new(&root, edits.keys())?;
            for (file, edits) in &edits {
                let text = plan.sources[file].text.clone();
                suggested(&mut plan, file, &text, edits, &mut report)?;
            }
            return finish(plan, &pass, report, apply);
        }
        _ => {
            let diagnostics = diagnostics.ok_or("--diagnostics is required")?;
            marks(&diagnostics, &root, &packages, code, pass == "spacing")?
        }
    };
    let mut plan = Plan::new(&root, found.keys())?;
    for (file, spans) in &found {
        let text = plan.sources[file].text.clone();
        let marks = spans
            .iter()
            .map(|(lo, hi, line, quoted)| Mark { lo: *lo, hi: *hi, line: *line, quoted: quoted.clone() })
            .collect::<Vec<_>>();
        match pass.as_str() {
            "spacing" => spacing(&mut plan, file, &text, &marks, &mut report)?,
            "questions" => questions(&mut plan, file, &text, &marks, !questions_only, &mut report)?,
            "division" => division(&mut plan, file, &text, &marks, &mut report)?,
            "index_rows" => index_rows(&mut plan, file, &text, &marks, &mut report)?,
            _ => must_use(&mut plan, file, &text, &marks, &mut report)?,
        }
    }
    finish(plan, &pass, report, apply)
}

fn finish(mut plan: Plan, pass: &str, report: BTreeMap<String, u64>, apply: bool) -> Outcome<()> {
    let outputs = plan.outputs()?;
    for (path, bytes) in &outputs {
        let said = std::str::from_utf8(bytes)?;
        syn::parse_file(said.trim_start_matches('\u{feff}'))
            .map_err(|error| format!("{}: the repair broke the syntax: {error}", path.display()))?;

        if pass == "spacing" {
            let before = plan.sources[path].text.parse::<proc_macro2::TokenStream>()?;
            let after = said.trim_start_matches('\u{feff}').parse::<proc_macro2::TokenStream>()?;

            if before.to_string() != after.to_string() {
                return Err(format!("{}: spacing changed Rust tokens", path.display()).into());
            }
        }
    }
    let mut summary = json!({"pass": pass, "report": report, "files": outputs.len()});
    if apply && !outputs.is_empty() {
        let temporary = env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        summary["backup"] = json!(plan.write(&outputs, &temporary.join("builds/explicit-repair"))?.display().to_string());
    }
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("repair: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_includes_local_forks_and_keeps_package_boundaries() {
        let root = Path::new("/repository");
        let packages = vec!["sample".to_owned()];

        assert!(owned(root, &packages, Path::new("/repository/crates/sample/src/lib.rs")));
        assert!(owned(root, &packages, Path::new("/repository/forks/sample/src/lib.rs")));
        assert!(!owned(root, &packages, Path::new("/repository/forks/sample-other/src/lib.rs")));
        assert!(!owned(root, &packages, Path::new("/elsewhere/crates/sample/src/lib.rs")));
    }

    fn planned(text: &str, pass: &str, marks: &[(usize, usize)]) -> String {
        let root = env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let directory = root.join(format!("repair-test-{}-{pass}-{}", std::process::id(), text.len()));
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join("source.rs");
        fs::write(&file, text).unwrap();
        let file = file.canonicalize().unwrap();
        let lines = Text::new(text);
        let marks = marks
            .iter()
            .map(|(lo, hi)| Mark { lo: *lo, hi: *hi, line: lines.line_of(*lo) + 1, quoted: lines.line(lines.line_of(*lo)).to_owned() })
            .collect::<Vec<_>>();
        let mut plan = Plan::new(&directory, [&file]).unwrap();
        let mut report = BTreeMap::new();
        match pass {
            "spacing" => spacing(&mut plan, &file, text, &marks, &mut report).unwrap(),
            "must_use" => must_use(&mut plan, &file, text, &marks, &mut report).unwrap(),
            "division" => division(&mut plan, &file, text, &marks, &mut report).unwrap(),
            _ => questions(&mut plan, &file, text, &marks, true, &mut report).unwrap(),
        }
        let outputs = plan.outputs().unwrap();
        fs::remove_dir_all(&directory).unwrap();
        outputs.get(&file).map_or(text.to_owned(), |bytes| String::from_utf8(bytes.clone()).unwrap())
    }

    #[test]
    fn spacing_inserts_once_at_each_suggested_line() {
        let text = "fn f() {\n    a();\n    for x in y {}\n    b();\n}\n";
        let at = text.find("    b();").unwrap();
        let above = text.find("    for").unwrap();
        assert_eq!(
            planned(text, "spacing", &[(at, at), (at, at), (above, above)]),
            "fn f() {\n    a();\n\n    for x in y {}\n\n    b();\n}\n"
        );
    }

    #[test]
    fn must_use_goes_directly_above_the_signature() {
        let text = "impl A {\n    /// Width.\n    #[inline]\n    pub fn width(&self) -> u32 {\n        self.w\n    }\n}\n";
        let lo = text.find("u32").unwrap();
        assert_eq!(
            planned(text, "must_use", &[(lo, lo + 3)]),
            "impl A {\n    /// Width.\n    #[inline]\n    #[must_use]\n    pub fn width(&self) -> u32 {\n        self.w\n    }\n}\n"
        );
    }

    #[test]
    fn must_use_reaches_free_and_trait_default_functions() {
        let text = "fn free() -> u8 { 1 }\ntrait T {\n    fn given(&self) -> u8 {\n        2\n    }\n}\n";
        let first = text.find("u8").unwrap();
        let second = text.rfind("u8").unwrap();
        assert_eq!(
            planned(text, "must_use", &[(first, first + 2), (second, second + 2)]),
            "#[must_use]\nfn free() -> u8 { 1 }\ntrait T {\n    #[must_use]\n    fn given(&self) -> u8 {\n        2\n    }\n}\n"
        );
    }

    #[test]
    fn a_question_inside_an_unnamed_residue_block_is_lifted_above_the_statement() {
        let text = "fn f(&mut self) -> Option<u8> {\n    color.green = ({ let Ok(numeric_value) = convert((self.parse_number()? * 2.0).round()); numeric_value });\n    None\n}\n";
        let lo = text.find("self.parse_number()?").unwrap();
        assert_eq!(
            planned(text, "questions", &[(lo, lo + 20)]),
            "fn f(&mut self) -> Option<u8> {\n    let number = self.parse_number()?;\n    color.green = ({ let Ok(numeric_value) = convert((number * 2.0).round()); numeric_value });\n    None\n}\n"
        );
    }

    #[test]
    fn division_by_a_positive_literal_names_its_policy() {
        let text = "fn f(x: u32, a: &mut A) -> u32 {\n    a.n /= 2;\n    let y = (x + 1) % 3 + x / n;\n    x.len() % 2\n}\n";
        let spans = ["a.n /= 2", "(x + 1) % 3", "x / n", "x.len() % 2"]
            .iter()
            .map(|said| {
                let lo = text.find(said).unwrap();
                (lo, lo + said.len())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            planned(text, "division", &spans),
            "fn f(x: u32, a: &mut A) -> u32 {\n    a.n = a.n.wrapping_div(2);\n    let y = (x + 1).wrapping_rem(3) + x / n;\n    x.len().wrapping_rem(2)\n}\n"
        );
    }

    #[test]
    fn questions_lift_through_the_shared_engine() {
        let text = "fn f(r: &mut R) -> Option<u8> {\n    g(r.kind()?);\n    None\n}\n";
        let lo = text.find("r.kind()?").unwrap();
        assert_eq!(
            planned(text, "questions", &[(lo, lo + 9)]),
            "fn f(r: &mut R) -> Option<u8> {\n    let kind = r.kind()?;\n    g(kind);\n    None\n}\n"
        );
    }
}
