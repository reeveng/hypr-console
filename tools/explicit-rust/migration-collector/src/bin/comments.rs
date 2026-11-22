//! Delete what EXPLICIT020 denies, a line at a time, and keep what a file is obliged to say.
//!
//! The runs are found exactly as the rule finds them, with the compiler's lexer: comments of one
//! doc style, separated by no more than a line break, are one run; a `//!` or `/*!` run and a run
//! whose first line says `SAFETY:` stay. A run that begins with something else and reaches a
//! `// SAFETY:` line loses only the lines above the reason.
//!
//! A deleted comment takes its line with it when it was alone on the line, and the blank lines
//! around it are put back the way a person would leave them: one where there was one, none after
//! an opening brace or before a closing one. A comment after code takes only itself and the space
//! before it.
//!
//! A licence or copyright notice is not commentary, it is a condition of the code being here, so
//! it is moved into the file's `//!` head instead of deleted. A doc comment on an `unsafe fn` or
//! `unsafe trait` carries the contract its callers are held to and Clippy asks for it, so it is
//! left for a person to move. A crate root that warns or denies `missing_docs` is contradicting
//! the rule this pass applies; the lint is taken out of the attribute, and the report says so.
#![feature(rustc_private)]
extern crate rustc_driver;
extern crate rustc_lexer;
#[path = "../edits.rs"]
pub mod edits;
#[path = "../source.rs"]
pub mod source;

use edits::{Outcome, Plan, Span};
use rustc_lexer::{tokenize, DocStyle, FrontmatterAllowed, TokenKind};
use serde_json::json;
use source::Text;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
struct Token {
    lo: usize,
    hi: usize,
    style: Option<DocStyle>,
}

#[derive(Debug)]
struct Run {
    tokens: Vec<Token>,
}

fn runs(text: &str) -> Vec<Run> {
    let mut found: Vec<Run> = vec![];
    let mut open = false;
    let mut gap = "";
    let mut at = 0usize;
    for token in tokenize(text, FrontmatterAllowed::Yes) {
        let lo = at;
        at += token.len as usize;
        let said = &text[lo..at];
        let style = match token.kind {
            TokenKind::LineComment { doc_style } => doc_style,
            TokenKind::BlockComment { doc_style, .. } => doc_style,
            TokenKind::Whitespace => {
                gap = said;
                continue;
            }
            _ => {
                open = false;
                gap = "";
                continue;
            }
        };
        let carries_on = match found.last() {
            Some(last) => {
                open && last.tokens[0].style == style && gap.matches('\n').count() <= 1
            }
            None => false,
        };
        let token = Token { lo, hi: at, style };
        match carries_on {
            true => found.last_mut().expect("a run to carry on").tokens.push(token),
            false => found.push(Run { tokens: vec![token] }),
        }
        open = true;
        gap = "";
    }
    found
}

fn allowed(text: &str, run: &Run) -> bool {
    let first = &run.tokens[0];
    match first.style {
        Some(DocStyle::Inner) => true,
        Some(DocStyle::Outer) => false,
        None => {
            let said = &text[first.lo..first.hi];
            said.starts_with("//") && said.contains("SAFETY:")
        }
    }
}

fn licence(said: &str) -> bool {
    let lower = said.to_lowercase();
    [
        "copyright",
        "spdx-license-identifier",
        "licensed under",
        "mozilla public license",
        "mpl-2.0",
        "apache license",
        "mit license",
        "permission is hereby granted",
        "this source code form is subject",
        "license: ",
        "licence",
        "(c) ",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

fn words(said: &str) -> Vec<String> {
    let body = match said.strip_prefix("/*") {
        Some(rest) => rest
            .trim_start_matches(['*', '!'])
            .strip_suffix("*/")
            .unwrap_or(rest)
            .to_owned(),
        None => said.to_owned(),
    };
    let lines = body
        .lines()
        .map(|line| {
            let line = line.trim_start();
            let line = line
                .strip_prefix("///")
                .or_else(|| line.strip_prefix("//"))
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| line.strip_prefix('*'))
                .unwrap_or(line);
            let line = line.strip_prefix(' ').unwrap_or(line);
            line.trim_end().to_owned()
        })
        .collect::<Vec<_>>();
    let start = lines.iter().position(|line| !line.is_empty()).unwrap_or(lines.len());
    let end = lines.iter().rposition(|line| !line.is_empty()).map_or(start, |at| at + 1);
    lines[start..end].to_vec()
}

fn documents_an_unsafe_contract(text: &str, run: &Run) -> bool {
    if run.tokens[0].style != Some(DocStyle::Outer) {
        return false;
    }
    let after = &text[run.tokens.last().expect("a token").hi..];
    for line in after.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("#[") || line.starts_with("//") {
            continue;
        }
        let words = line.split_whitespace().collect::<Vec<_>>();
        return words
            .windows(2)
            .any(|pair| pair[0] == "unsafe" && matches!(pair[1], "fn" | "trait" | "impl"));
    }
    false
}

#[derive(Default)]
struct Found {
    deleted: Vec<Token>,
    notices: Vec<Vec<String>>,
    kept_contracts: u64,
    outer: bool,
}

fn decide(text: &str) -> Found {
    let mut found = Found::default();
    for run in runs(text) {
        if allowed(text, &run) {
            continue;
        }
        let reason = run.tokens.iter().position(|token| {
            token.style.is_none() && text[token.lo..token.hi].contains("SAFETY:")
        });
        let tokens = match reason {
            Some(at) => run.tokens[..at].to_vec(),
            None => run.tokens.clone(),
        };
        if tokens.is_empty() {
            continue;
        }
        let said = tokens
            .iter()
            .map(|token| &text[token.lo..token.hi])
            .collect::<Vec<_>>()
            .join("\n");
        if licence(&said) {
            let notice = tokens
                .iter()
                .flat_map(|token| words(&text[token.lo..token.hi]))
                .collect::<Vec<_>>();
            found.notices.push(notice);
            found.deleted.extend(tokens);
            continue;
        }
        if documents_an_unsafe_contract(text, &run) {
            found.kept_contracts += 1;
            continue;
        }
        found.outer |= tokens[0].style == Some(DocStyle::Outer);
        found.deleted.extend(tokens);
    }
    found
}

fn closes(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with('}') || line.starts_with(')') || line.starts_with(']')
}

fn opens(line: &str) -> bool {
    let line = line.trim_end();
    line.ends_with('{') || line.ends_with('(') || line.ends_with('[')
}

fn deletions(text: &str, deleted: &[Token]) -> Vec<(usize, usize)> {
    let lines = Text::new(text);
    let mut on_line = BTreeMap::<usize, Vec<&Token>>::new();
    for token in deleted {
        for line in lines.line_of(token.lo)..=lines.line_of(token.hi.saturating_sub(1).max(token.lo)) {
            on_line.entry(line).or_default().push(token);
        }
    }
    let mut whole = BTreeSet::new();
    for (line, tokens) in &on_line {
        let start = lines.line_start(*line);
        let end = lines.line_end(*line);
        let mut rest = String::new();
        let mut cursor = start;
        let mut sorted = tokens.clone();
        sorted.sort_by_key(|token| token.lo);
        for token in sorted {
            if token.lo > cursor {
                rest.push_str(&text[cursor..token.lo.min(end)]);
            }
            cursor = cursor.max(token.hi);
        }
        if cursor < end {
            rest.push_str(&text[cursor..end]);
        }
        if rest.trim().is_empty() {
            whole.insert(*line);
        }
    }
    let mut ranges = vec![];
    for token in deleted {
        let first = lines.line_of(token.lo);
        let last = lines.line_of(token.hi.saturating_sub(1).max(token.lo));
        if (first..=last).all(|line| whole.contains(&line)) {
            continue;
        }
        let line_end = lines.line_end(last);
        let before = &text[lines.line_start(first)..token.lo];
        let after = &text[token.hi..line_end.max(token.hi)];
        let mut lo = token.lo;
        let mut hi = token.hi;
        if after.trim().is_empty() {
            lo -= before.len() - before.trim_end().len();
        } else {
            hi += after.len() - after.trim_start().len();
        }
        ranges.push((lo, hi));
    }
    let total = lines.lines();
    let blank = |line: usize| lines.line(line).trim().is_empty() && !whole.contains(&line);
    let mut line = 0;
    while line < total {
        if !whole.contains(&line) {
            line += 1;
            continue;
        }
        let mut first = line;
        while first > 0 && (whole.contains(&(first - 1)) || blank(first - 1)) {
            first -= 1;
        }
        let mut last = line;
        while last + 1 < total && (whole.contains(&(last + 1)) || blank(last + 1)) {
            last += 1;
        }
        let blanks = (first..=last).filter(|at| blank(*at)).collect::<Vec<_>>();
        let opened = first == 0 || opens(lines.line(first - 1));
        let closed = last + 1 >= total || closes(lines.line(last + 1));
        let keep = match (blanks.is_empty(), opened || closed) {
            (true, _) | (false, true) => None,
            (false, false) => blanks.first().copied(),
        };
        for at in first..=last {
            if Some(at) == keep {
                continue;
            }
            let start = lines.line_start(at);
            let end = match at + 1 < total {
                true => lines.line_start(at + 1),
                false => lines.line_end(at),
            };
            ranges.push((start, end));
        }
        line = last + 1;
    }
    ranges.sort();
    let mut merged: Vec<(usize, usize)> = vec![];
    for (lo, hi) in ranges {
        match merged.last_mut() {
            Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    merged
}

fn head_end(text: &str) -> Option<usize> {
    let mut tokens = vec![];
    let mut at = 0usize;
    for token in tokenize(text, FrontmatterAllowed::Yes) {
        let lo = at;
        at += token.len as usize;
        tokens.push((token.kind, lo, at));
    }
    let mut end = None;
    let mut index = 0;
    while index < tokens.len() {
        let (kind, lo, hi) = tokens[index];
        match kind {
            TokenKind::LineComment { doc_style: Some(DocStyle::Inner) }
            | TokenKind::BlockComment { doc_style: Some(DocStyle::Inner), .. } => end = Some(hi),
            TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. } => {}
            TokenKind::Pound if text[lo..].starts_with("#![") || text[lo..].starts_with("#!") && index == 0 => {
                let mut depth = 0usize;
                index += 1;
                while index < tokens.len() {
                    match tokens[index].0 {
                        TokenKind::OpenBracket => depth += 1,
                        TokenKind::CloseBracket => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    index += 1;
                }
            }
            _ => break,
        }
        index += 1;
    }
    end
}

fn notice_lines(notices: &[Vec<String>]) -> Vec<String> {
    let mut lines = vec![];
    for (index, notice) in notices.iter().enumerate() {
        if index > 0 {
            lines.push("//!".to_owned());
        }
        for line in notice {
            lines.push(match line.is_empty() {
                true => "//!".to_owned(),
                false => format!("//! {line}"),
            });
        }
    }
    lines
}

fn without_missing_docs(text: &str) -> Option<(usize, usize, String)> {
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let inner = match trimmed
            .strip_prefix("#![warn(")
            .or_else(|| trimmed.strip_prefix("#![deny("))
            .or_else(|| trimmed.strip_prefix("#![forbid("))
        {
            Some(rest) => rest,
            None => continue,
        };
        let Some(list) = inner.strip_suffix(")]") else {
            continue;
        };
        let lints = list.split(',').map(str::trim).filter(|lint| !lint.is_empty()).collect::<Vec<_>>();
        if !lints.contains(&"missing_docs") {
            continue;
        }
        let start = text.lines().take(index).map(|line| line.len() + 1).sum::<usize>();
        let end = start + line.len();
        let kept = lints.iter().filter(|lint| **lint != "missing_docs").copied().collect::<Vec<_>>();
        return Some(match kept.is_empty() {
            true => (start, (end + 1).min(text.len()), String::new()),
            false => {
                let level = &trimmed[3..trimmed.find('(').expect("a lint level")];
                (start, end, format!("{}#![{level}({})]", &line[..line.len() - line.trim_start().len()], kept.join(", ")))
            }
        });
    }
    None
}

fn files(directory: &Path, found: &mut BTreeSet<PathBuf>) -> Outcome<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            let name = entry.file_name();
            if name == "target" || name == "fixtures" {
                continue;
            }
            files(&path, found)?;
        } else if kind.is_file() && path.extension().is_some_and(|end| end == "rs") {
            found.insert(path);
        }
    }
    Ok(())
}

struct Prepared {
    ranges: Vec<(usize, usize)>,
    head: Option<(usize, String)>,
    kept_contracts: u64,
    outer: bool,
    notices: usize,
}

fn prepare(text: &str) -> Prepared {
    let found = decide(text);
    let mut ranges = deletions(text, &found.deleted);
    let mut head = None;
    if !found.notices.is_empty() {
        let lines = notice_lines(&found.notices).join("\n");
        head = Some(match head_end(text) {
            Some(end) => (end, format!("\n//!\n{lines}")),
            None => {
                let start = match text.starts_with("#!") && !text.starts_with("#![") {
                    true => text.find('\n').map_or(text.len(), |at| at + 1),
                    false => 0,
                };
                (start, format!("{lines}\n\n"))
            }
        });
        if let Some((at, _)) = head {
            ranges.retain(|(lo, hi)| !(*lo < at && at < *hi));
        }
    }
    Prepared {
        ranges,
        head,
        kept_contracts: found.kept_contracts,
        outer: found.outer,
        notices: found.notices.len(),
    }
}

fn render(text: &str, prepared: &Prepared) -> String {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    let mut inserted = false;
    for (lo, hi) in &prepared.ranges {
        if let Some((at, said)) = &prepared.head {
            if !inserted && *at <= *lo {
                output.push_str(&text[cursor..*at]);
                output.push_str(said);
                cursor = *at;
                inserted = true;
            }
        }
        output.push_str(&text[cursor..*lo]);
        cursor = *hi;
    }
    if let Some((at, said)) = &prepared.head {
        if !inserted {
            let at = (*at).max(cursor);
            output.push_str(&text[cursor..at]);
            output.push_str(said);
            cursor = at;
        }
    }
    output.push_str(&text[cursor..]);
    output
}

fn run() -> Outcome<()> {
    let mut root = env::current_dir()?;
    let mut packages = vec![];
    let mut apply = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root needs a path")?),
            "--package" => packages.push(args.next().ok_or("--package needs a name")?),
            "--apply" => apply = true,
            "--help" | "-h" => {
                println!("comments --package NAME [--package NAME] [--root DIR] [--apply]\nDeletes what EXPLICIT020 denies in every Rust file of each package, moving licence notices into the `//!` head. Defaults to a dry run; --apply backs up originals under TMPDIR/builds/explicit-comments.");
                return Ok(());
            }
            unknown => return Err(format!("unknown argument {unknown}").into()),
        }
    }
    if packages.is_empty() {
        return Err("--package is required".into());
    }
    let root = root.canonicalize()?;
    let mut paths = BTreeSet::new();
    let mut roots = BTreeMap::new();
    for package in &packages {
        let directory = root.join("crates").join(package);
        files(&directory, &mut paths)?;
        roots.insert(package.clone(), directory.join("src/lib.rs"));
    }
    let mut plan = Plan::new(&root, &paths)?;
    let mut report = BTreeMap::<String, u64>::new();
    let mut outer = BTreeSet::new();
    for file in plan.sources.keys().cloned().collect::<Vec<_>>() {
        let text = plan.sources[&file].text.clone();
        let prepared = prepare(&text);
        *report.entry("kept_unsafe_contracts".into()).or_default() += prepared.kept_contracts;
        *report.entry("notices_moved_to_head".into()).or_default() += prepared.notices as u64;
        if prepared.outer {
            for (package, lib) in &roots {
                if file.starts_with(root.join("crates").join(package)) {
                    outer.insert(lib.clone());
                }
            }
        }
        if prepared.ranges.is_empty() && prepared.head.is_none() {
            continue;
        }
        *report.entry("ranges_deleted".into()).or_default() += prepared.ranges.len() as u64;
        let output = render(&text, &prepared);
        syn::parse_file(&output).map_err(|error| format!("{}: comment removal broke the syntax: {error}", file.display()))?;
        plan.add(
            Span { file: file.clone(), lo: 0, hi: text.len() },
            "comments",
            move |_, _| Ok(output.clone()),
        )?;
    }
    for lib in outer {
        let Ok(lib) = lib.canonicalize() else { continue };
        let Some(source) = plan.sources.get(&lib) else { continue };
        let text = source.text.clone();
        if let Some((lo, hi, said)) = without_missing_docs(&text) {
            *report.entry("missing_docs_withdrawn".into()).or_default() += 1;
            plan.add(Span { file: lib.clone(), lo, hi }, "missing_docs", move |_, _| Ok(said.clone()))?;
        }
    }
    let outputs = plan.outputs()?;
    let mut summary = json!({"files": outputs.len(), "report": report});
    if apply && !outputs.is_empty() {
        let temporary = env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        summary["backup"] = json!(plan.write(&outputs, &temporary.join("builds/explicit-comments"))?.display().to_string());
    }
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("comments: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleaned(text: &str) -> String {
        render(text, &prepare(text))
    }

    #[test]
    fn a_comment_alone_on_its_line_takes_the_line() {
        assert_eq!(
            cleaned("fn f() {\n    // explain\n    let x = 1;\n}\n"),
            "fn f() {\n    let x = 1;\n}\n"
        );
    }

    #[test]
    fn a_trailing_comment_takes_only_itself() {
        assert_eq!(cleaned("let x = 1; // one\nlet y = 2;\n"), "let x = 1;\nlet y = 2;\n");
        assert_eq!(cleaned("f(/* first */ 1, 2);\n"), "f(1, 2);\n");
    }

    #[test]
    fn the_blank_line_around_a_decision_is_kept_once() {
        assert_eq!(
            cleaned("fn f() {\n    a();\n\n    // why\n    match x {}\n}\n"),
            "fn f() {\n    a();\n\n    match x {}\n}\n"
        );
        assert_eq!(
            cleaned("fn f() {\n    a();\n\n    // section\n\n    b();\n}\n"),
            "fn f() {\n    a();\n\n    b();\n}\n"
        );
    }

    #[test]
    fn no_blank_line_is_left_after_an_opening_or_before_a_closing_brace() {
        assert_eq!(cleaned("fn f() {\n    // a\n\n    b();\n}\n"), "fn f() {\n    b();\n}\n");
        assert_eq!(cleaned("fn f() {\n    b();\n\n    // done\n}\n"), "fn f() {\n    b();\n}\n");
    }

    #[test]
    fn doc_comments_go_and_the_gap_between_items_stays() {
        assert_eq!(
            cleaned("struct A;\n\n/// The second.\n///\n/// More.\nstruct B;\n"),
            "struct A;\n\nstruct B;\n"
        );
    }

    #[test]
    fn module_heads_and_safety_reasons_stay() {
        let text = "//! Head.\n\nfn f() {\n    // SAFETY: the pointer is live\n    // for the whole call.\n    unsafe { g() }\n}\n";
        assert_eq!(cleaned(text), text);
    }

    #[test]
    fn lines_above_a_safety_reason_go_and_the_reason_stays() {
        assert_eq!(
            cleaned("fn f() {\n    // first a note\n    // SAFETY: live\n    unsafe { g() }\n}\n"),
            "fn f() {\n    // SAFETY: live\n    unsafe { g() }\n}\n"
        );
    }

    #[test]
    fn a_licence_moves_into_an_existing_head() {
        assert_eq!(
            cleaned("// Copyright 2018 the Resvg Authors\n// SPDX-License-Identifier: Apache-2.0 OR MIT\n\n//! Rendering.\n\nuse a::b;\n"),
            "//! Rendering.\n//!\n//! Copyright 2018 the Resvg Authors\n//! SPDX-License-Identifier: Apache-2.0 OR MIT\n\nuse a::b;\n"
        );
    }

    #[test]
    fn a_licence_becomes_the_head_when_there_is_none() {
        assert_eq!(
            cleaned("// Copyright 2018 the Resvg Authors\n// SPDX-License-Identifier: Apache-2.0 OR MIT\n\nuse a::b;\n"),
            "//! Copyright 2018 the Resvg Authors\n//! SPDX-License-Identifier: Apache-2.0 OR MIT\n\nuse a::b;\n"
        );
        assert_eq!(
            cleaned("/* This Source Code Form is subject to the terms of the Mozilla Public\n * License, v. 2.0. */\nuse a::b;\n"),
            "//! This Source Code Form is subject to the terms of the Mozilla Public\n//! License, v. 2.0.\n\nuse a::b;\n"
        );
    }

    #[test]
    fn comments_inside_strings_and_macros_are_handled_by_the_lexer() {
        assert_eq!(
            cleaned("let a = \"// not a comment\";\nm! {\n    // inside\n    x\n}\n"),
            "let a = \"// not a comment\";\nm! {\n    x\n}\n"
        );
    }

    #[test]
    fn an_unsafe_contract_is_left_for_a_person() {
        let text = "/// # Safety\n/// The caller keeps it alive.\npub unsafe fn f() {}\n";
        assert_eq!(cleaned(text), text);
    }

    #[test]
    fn missing_docs_is_withdrawn_from_a_list_or_alone() {
        let (lo, hi, said) = without_missing_docs("#![no_std]\n#![warn(missing_docs)]\nmod a;\n").unwrap();
        assert_eq!((lo, hi, said.as_str()), (11, 34, ""));
        let (_, _, said) = without_missing_docs("#![deny(missing_docs, unsafe_code)]\n").unwrap();
        assert_eq!(said, "#![deny(unsafe_code)]");
    }

    #[test]
    fn cfg_attributes_and_code_are_untouched() {
        let text = "#[cfg(test)] // tests\nmod tests {}\n";
        assert_eq!(cleaned(text), "#[cfg(test)]\nmod tests {}\n");
    }
}
