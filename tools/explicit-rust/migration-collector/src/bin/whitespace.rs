//! Trim migration residue while retaining every byte inside Rust literals.
#[path = "../edits.rs"]
pub mod edits;
use edits::{Outcome, Plan, Span};
use proc_macro2::{TokenStream, TokenTree};
use serde_json::json;
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

fn offset(text: &str, position: proc_macro2::LineColumn) -> Outcome<usize> {
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
        .ok_or("invalid literal column")?;
    Ok(before + column)
}
fn literals(tokens: TokenStream, source: &str, spans: &mut Vec<(usize, usize)>) -> Outcome<()> {
    for token in tokens {
        match token {
            TokenTree::Literal(literal) => spans.push((
                offset(source, literal.span().start())?,
                offset(source, literal.span().end())?,
            )),
            TokenTree::Group(group) => literals(group.stream(), source, spans)?,
            _ => {}
        }
    }
    Ok(())
}
fn residue(source: &str) -> Outcome<Vec<(usize, usize)>> {
    let mut protected = vec![];
    literals(TokenStream::from_str(source)?, source, &mut protected)?;
    let mut start = 0;
    let mut edits = vec![];
    for line in source.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let trimmed = body.trim_end_matches([' ', '\t']);
        let lo = start + trimmed.len();
        let hi = start + body.len();
        if lo < hi && !protected.iter().any(|(a, b)| *a < hi && lo < *b) {
            edits.push((lo, hi));
        }
        start += line.len();
    }
    Ok(edits)
}
fn files(path: &Path, out: &mut BTreeSet<PathBuf>) -> Outcome<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            files(&entry.path(), out)?;
        } else if kind.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        {
            out.insert(entry.path());
        }
    }
    Ok(())
}
fn run() -> Outcome<()> {
    let mut root = env::current_dir()?;
    let mut packages = vec![];
    let mut apply = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("--root needs path")?),
            "--package" => packages.push(args.next().ok_or("--package needs name")?),
            "--apply" => apply = true,
            "--help" | "-h" => {
                println!("whitespace --package NAME [--package NAME] [--root DIR] [--apply]\nTrims trailing spaces outside literals in src/, generated/, tests/, and benches/. Defaults to a dry-run report.");
                return Ok(());
            }
            _ => return Err(format!("unknown argument {argument}").into()),
        }
    }
    if packages.is_empty() {
        return Err("--package is required".into());
    }
    let root = root.canonicalize()?;
    let mut paths = BTreeSet::new();
    for package in packages {
        for directory in ["src", "generated", "tests", "benches"] {
            files(
                &root.join("crates").join(&package).join(directory),
                &mut paths,
            )?;
        }
    }
    let mut plan = Plan::new(&root, paths)?;
    for file in plan.sources.keys().cloned().collect::<Vec<_>>() {
        for (lo, hi) in residue(&plan.sources[&file].text)? {
            plan.add(
                Span {
                    file: file.clone(),
                    lo,
                    hi,
                },
                "trailing_space",
                |_, _| Ok(String::new()),
            )?;
        }
    }
    let outputs = plan.outputs()?;
    let mut report = json!({"changes":plan.stats,"files":outputs.len()});
    if apply && !outputs.is_empty() {
        let temporary =
            env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        report["backup"] = json!(plan
            .write(&outputs, &temporary.join("builds/explicit-whitespace"))?
            .display()
            .to_string());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("whitespace: {error}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_whitespace_is_preserved() {
        let source = "const TEXT: &str = r#\"literal   \nnext\"#;   \n   \n";
        let edits = residue(source).unwrap();
        assert_eq!(edits.len(), 2);
        assert!(!edits
            .iter()
            .any(|(lo, _)| *lo < source.find("next").unwrap()));
    }
    #[test]
    fn unicode_columns_preserve_literal_boundaries() {
        let source = "const café: &str = \"é   \";  \n";
        let edits = residue(source).unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(&source[edits[0].0..edits[0].1], "  ");
    }
}
