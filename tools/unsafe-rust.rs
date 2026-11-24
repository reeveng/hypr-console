//! Inventory the unsafe Rust syntax in every owned source file, including
//! files that are inactive for the host's feature and target configuration.

#![feature(rustc_private)]
#![forbid(unsafe_code)]

extern crate rustc_driver;
extern crate rustc_lexer;

use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, ExitCode};
use std::{env, fs};

type Inventory = BTreeMap<(String, String), usize>;

const BASELINE: &str = "tools/unsafe-rust-baseline.txt";

fn git(arguments: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
  let output = Command::new("git").args(arguments).output()?;
  if !output.status.success() {
    return Err(io::Error::other(String::from_utf8_lossy(&output.stderr).into_owned()).into());
  }
  Ok(output.stdout)
}

fn tracked_sources() -> Result<BTreeSet<String>, Box<dyn Error>> {
  let files = git(&[
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
  ])?;
  let files = String::from_utf8(files)?;
  Ok(
    files
      .split_terminator('\0')
      .filter(|path| path.ends_with(".rs"))
      .filter(|path| {
        !matches!(
          path.split('/').nth(1),
          Some("console-kernel" | "console-kernel-handles" | "console-kernel-heap")
        ) || !path.starts_with("crates/")
      })
      .map(str::to_owned)
      .collect(),
  )
}

fn sites(source: &str) -> Vec<String> {
  let mut offset = 0;
  let mut lines: Vec<Vec<&str>> = vec![Vec::new()];
  let mut matches = Vec::new();
  let mut pending_attribute = false;
  let mut attribute_depth = 0usize;
  let mut previous = ("", 0);

  for token in tokenize(source, FrontmatterAllowed::Yes) {
    let start = offset;
    offset += token.len as usize;
    let spelling = &source[start..offset];
    let line = lines.len() - 1;

    if !matches!(
      token.kind,
      TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
    ) {
      match spelling {
        "#" if attribute_depth == 0 => pending_attribute = true,
        "!" if pending_attribute => {}
        "[" if pending_attribute || attribute_depth > 0 => {
          pending_attribute = false;
          attribute_depth += 1;
        }
        "]" if attribute_depth > 0 => attribute_depth -= 1,
        _ => pending_attribute = false,
      }

      if spelling == "unsafe"
        || (attribute_depth > 0
          && matches!(
            spelling,
            "no_mangle" | "export_name" | "link_section" | "naked"
          )
          && previous.0 != "(")
      {
        matches.push((line, attribute_depth > 0));
      }
      if spelling == "!" && matches!(previous.0, "global_asm" | "naked_asm") {
        matches.push((previous.1, false));
      }
      previous = (spelling, line);
      lines[line].push(spelling);
    }

    lines.extend(
      spelling
        .bytes()
        .filter(|byte| *byte == b'\n')
        .map(|_| Vec::new()),
    );
  }

  matches
    .into_iter()
    .map(|(line, attribute)| {
      let mut anchor = lines[line].join(" ");
      if attribute || anchor.ends_with('{') || anchor.ends_with("unsafe") {
        if let Some(next) = lines[line + 1..]
          .iter()
          .find(|tokens| !tokens.is_empty() && (!attribute || tokens.first() != Some(&"#")))
        {
          anchor.push_str(" | ");
          anchor.push_str(&next.join(" "));
        }
      }
      anchor.replace('\t', "\\t").replace('\n', "\\n")
    })
    .collect()
}

fn inventory() -> Result<Inventory, Box<dyn Error>> {
  let mut found = Inventory::new();
  for file in tracked_sources()? {
    if !Path::new(&file).is_file() {
      continue;
    }
    for anchor in sites(&fs::read_to_string(&file)?) {
      *found.entry((file.clone(), anchor)).or_default() += 1;
    }
  }
  Ok(found)
}

fn baseline(text: &str) -> Result<Inventory, Box<dyn Error>> {
  let mut entries = Inventory::new();
  for line in text.lines().filter(|line| !line.starts_with('#')) {
    let [file, count, anchor] = line.splitn(3, '\t').collect::<Vec<_>>()[..] else {
      return Err(io::Error::other(format!("invalid baseline entry: {line}")).into());
    };
    let count = count.parse()?;
    if count == 0
      || entries
        .insert((file.to_owned(), anchor.to_owned()), count)
        .is_some()
    {
      return Err(io::Error::other(format!("invalid baseline count: {line}")).into());
    }
  }
  Ok(entries)
}

fn compare(actual: &Inventory, permitted: &Inventory, message: &str) -> bool {
  let mut equal_or_smaller = true;
  for ((file, anchor), count) in actual {
    let previous = permitted
      .get(&(file.clone(), anchor.clone()))
      .copied()
      .unwrap_or(0);
    if *count > previous {
      eprintln!("{file}: {message} ({previous} → {count}): {anchor}");
      equal_or_smaller = false;
    }
  }
  equal_or_smaller
}

fn run() -> Result<bool, Box<dyn Error>> {
  let args: Vec<_> = env::args().skip(1).collect();
  match args.as_slice() {
    [mode] if mode == "--inventory" => {
      let mut out = io::stdout().lock();
      writeln!(
        out,
        "# Unsafe Rust sites. Generated by just unsafe-rust --inventory."
      )?;
      writeln!(
        out,
        "# Path, occurrence count, and source anchor, separated by tabs."
      )?;
      for ((file, anchor), count) in inventory()? {
        writeln!(out, "{file}\t{count}\t{anchor}")?;
      }
      Ok(true)
    }
    [] | [_] => {
      let expected = baseline(&fs::read_to_string(BASELINE)?)?;
      let actual = inventory()?;
      let within_budget = compare(&actual, &expected, "new unsafe Rust");
      let current = compare(&expected, &actual, "remove stale baseline entry");
      let shrink_only = match args.first() {
        Some(base) => {
          let files = git(&["ls-tree", "--name-only", base, "--", BASELINE])?;
          if files.is_empty() {
            eprintln!("Unsafe Rust baseline introduced on this branch.");
            true
          } else {
            let previous = git(&["show", &format!("{base}:{BASELINE}")])?;
            let previous = baseline(std::str::from_utf8(&previous)?)?;
            compare(&expected, &previous, "unsafe Rust baseline grew")
          }
        }
        None => true,
      };
      if within_budget && current && shrink_only {
        let count: usize = actual.values().sum();
        println!(
          "{count} unsafe Rust sites across {} source anchors",
          actual.len()
        );
      }
      Ok(within_budget && current && shrink_only)
    }
    _ => Err(io::Error::other("usage: unsafe-rust [--inventory | BASE-REVISION]").into()),
  }
}

fn main() -> ExitCode {
  match run() {
    Ok(true) => ExitCode::SUCCESS,
    Ok(false) => ExitCode::FAILURE,
    Err(error) => {
      eprintln!("unsafe-rust: {error}");
      ExitCode::FAILURE
    }
  }
}

#[cfg(test)]
mod tests {
  use super::sites;

  #[test]
  fn finds_inactive_items_and_raw_attributes_outside_comments_and_strings() {
    let source = r##"
            // unsafe {}
            const EXAMPLE: &str = r#"unsafe { }"#;
            #[cfg(any())]
            unsafe impl Send for Example {}
            #[unsafe(no_mangle)]
            #[cfg_attr(any(), export_name = "somewhere")]
            #[cfg_attr(any(), unsafe(link_section = ".text"))]
            extern "C" fn symbol() {}
            core::arch::global_asm!("");
        "##;

    assert_eq!(sites(source).len(), 5);
  }
}
