//! Renames the words `vocabulary.conf` retired, one definition at a time.
//!
//! ```text
//! rename-words plan  [PATH...]   write renames.plan: every place a retired
//!                                word is defined, and what it may become
//! rename-words apply [PATH...]   carry out what renames.plan settled
//! rename-words check [PATH...]   fail while a retired word is still defined
//!                                somewhere the plan did not keep it
//! ```
//!
//! Every session that renamed by hand renamed a word, not a thing. `Missing`
//! is a surface nobody drew in the compositor and a package nobody installed
//! in the engine, and a sweep that followed the table's one line for it made
//! one of them wrong; the next session read the wrong one as the house style
//! and put the old word back. So the plan is written per definition -- the
//! enum, the variant, the struct, the field -- with the table's options beside
//! it, and each settled line is handed to rust-analyzer, which renames the
//! definition and exactly the uses that refer to it.
//!
//! A lowercase word is planned the same way, as the fields and functions that
//! define it, and never as text: `down` is the `y` of a point in one file and a
//! button that is pressed in the next, and a sweep over locals renamed both. A
//! local keeps its word until somebody renames it by hand. `[*]` is still read,
//! for the rare word that really does mean one thing everywhere, and renames it
//! as text across the tree after the definitions.
//!
//! `check` is what keeps a finished rename finished: a retired word defined
//! anywhere the plan did not mark as kept is a failure, so a session that writes
//! one back hears about it from a tool rather than from the person.

mod analyzer;
mod definitions;
mod lexing;
mod plan;
mod vocabulary;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use definitions::{Definition, definitions};
use lexing::{blank, identifiers};
use plan::{EVERYWHERE, KEEP, PICK, Plan};
use vocabulary::Vocabulary;

fn root() -> std::io::Result<PathBuf> {
    let here = std::env::current_dir()?;

    here.ancestors()
        .find(|at| at.join("desktop.conf").is_file() && at.join("vocabulary.conf").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| std::io::Error::other("not inside the tree: no desktop.conf above here"))
}

fn tracked(root: &Path, paths: &[String]) -> std::io::Result<Vec<String>> {
    let wanted: Vec<&str> = match paths.is_empty() {
        true => vec!["crates"],
        false => paths.iter().map(String::as_str).collect(),
    };
    let listed = Command::new("git").arg("-C").arg(root).arg("ls-files").arg("--").args(wanted).output()?;

    Ok(String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|file| file.ends_with(".rs") && root.join(file).is_file())
        .map(str::to_string)
        .collect())
}

fn named(vocabulary: &Vocabulary) -> impl Fn(&str) -> bool + '_ {
    |word: &str| vocabulary.words.contains_key(word)
}

fn plan(root: &Path, paths: &[String]) -> std::io::Result<ExitCode> {
    let vocabulary = vocabulary::read(root)?;
    let previous = plan::read(root)?;
    let files = tracked(root, paths)?;
    let mut out = vec![
        "# Written by rename-words plan and read back by it: a choice written here".to_string(),
        "# survives the next plan. `[*]` is renamed as text across the whole tree;".to_string(),
        "# every other heading is one file and the definitions in it.".to_string(),
        "#   Word kind of Owner = New          settled".to_string(),
        "#   Word kind of Owner = ? A | B      open: write a name over it".to_string(),
        "#   Word kind of Owner = -            kept on purpose".to_string(),
        String::new(),
        format!("[{EVERYWHERE}]"),
    ];
    let mut sources = Vec::new();

    for file in &files {
        sources.push((file, std::fs::read_to_string(root.join(file))?));
    }
    for (word, right) in previous.get(EVERYWHERE).into_iter().flatten() {
        out.push(format!("{word} = {right}"));
    }
    out.push(String::new());
    let mut still_open = 0;
    let wanted = named(&vocabulary);

    for (file, src) in &sources {
        let found = definitions(src, &wanted);
        let keys: Vec<String> = found.iter().map(Definition::key).collect();
        let applied: Vec<(&String, &String)> = previous
            .get(file.as_str())
            .into_iter()
            .flatten()
            .filter(|(key, right)| plan::settled(right) && !keys.contains(key))
            .collect();

        if found.is_empty() && applied.is_empty() {
            continue;
        }
        out.push(format!("[{file}]"));
        for (key, right) in applied {
            out.push(format!("{key} = {right}    # applied"));
        }
        for definition in found {
            let key = definition.key();
            let options: Vec<String> = vocabulary.words.get(&definition.word).cloned().unwrap_or_default();
            let right = previous
                .get(file.as_str())
                .and_then(|keys| keys.get(&key))
                .cloned()
                .or_else(|| vocabulary.site_choice(&definition.word, file).map(str::to_string))
                .unwrap_or_else(|| plan::open(&options));

            if right.starts_with(PICK) {
                still_open += 1;
            }
            let context: String = definition.context.chars().take(90).collect();

            out.push(format!("{key} = {right}    # {}: {context}", definition.line + 1));
        }
        out.push(String::new());
    }
    std::fs::write(root.join(plan::FILE), out.join("\n"))?;
    println!("{}: {still_open} definitions still to choose a name for", plan::FILE);

    Ok(ExitCode::SUCCESS)
}

fn everywhere(root: &Path, files: &[String], settled: &BTreeMap<String, String>) -> std::io::Result<()> {
    for file in files {
        let path = root.join(file);
        let src = std::fs::read_to_string(&path)?;
        let code = blank(&src);
        let mut new = src.clone();
        let mut hits: Vec<(usize, &str)> = identifiers(&code).filter(|(_, word)| settled.contains_key(*word)).collect();

        for (old, _) in settled {
            for (at, _) in src.match_indices(&format!("{{{old}")) {
                let name = at + 1;
                let after = src.as_bytes().get(name + old.len());

                if code.as_bytes()[at] == b' ' && matches!(after, Some(b'}' | b':')) {
                    hits.push((name, old.as_str()));
                }
            }
            for (at, _) in src.match_indices(&format!("{old}$")) {
                let before = at.checked_sub(1).and_then(|n| src.as_bytes().get(n));

                if code.as_bytes()[at] == b' ' && !before.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_') {
                    hits.push((at, old.as_str()));
                }
            }
        }
        hits.sort();

        if hits.is_empty() {
            continue;
        }
        let before: std::collections::BTreeSet<&str> = identifiers(&code).map(|(_, word)| word).collect();
        let clash: Vec<&str> = settled.iter().filter(|(old, new)| before.contains(old.as_str()) && before.contains(new.as_str())).map(|(_, new)| new.as_str()).collect();

        for (at, word) in hits.into_iter().rev() {
            new.replace_range(at..at + word.len(), &settled[word]);
        }
        std::fs::write(&path, new)?;
        match clash.is_empty() {
            true => println!("{file}"),
            false => println!("{file}  CLASH {}", clash.join(",")),
        }
    }

    Ok(())
}

fn apply(root: &Path, paths: &[String]) -> std::io::Result<ExitCode> {
    let mut chosen: Plan = plan::read(root)?;
    let textual: BTreeMap<String, String> = chosen.remove(EVERYWHERE).unwrap_or_default().into_iter().filter(|(_, right)| plan::settled(right)).collect();
    let files = tracked(root, paths)?;
    let mut work: Vec<(String, String, String)> = chosen
        .into_iter()
        .filter(|(file, _)| files.contains(file))
        .flat_map(|(file, keys)| keys.into_iter().filter(|(_, right)| plan::settled(right)).map(move |(key, right)| (file.clone(), key, right)))
        .collect();
    let mut refused = 0;

    work.sort_by_key(|(_, key, _)| !key.contains(" of "));

    if !work.is_empty() {
        let mut analyzer = analyzer::Analyzer::start(root)?;

        loop {
            let mut missing = Vec::new();
            let mut renamed = 0;

            for (file, key, new) in work {
                let path = root.join(&file);
                let word = key.split(' ').next().unwrap_or_default();
                let src = std::fs::read_to_string(&path)?;
                let found = definitions(&src, &|candidate| candidate == word);
                let Some(definition) = found.iter().find(|definition| definition.key() == key) else {
                    missing.push((file, key, new));
                    continue;
                };

                match analyzer.rename(&path, definition, &new)? {
                    Ok(touched) => {
                        if definition.owner.is_none() {
                            plan::rekey(root, &file, &definition.word, &new)?;
                        }
                        renamed += 1;
                        println!("{file}: {key} -> {new}  ({} files)", touched.len());
                    },
                    Err(fault) => {
                        refused += 1;
                        println!("{file}: {key} -> {new}  REFUSED {fault}");
                    },
                }
            }
            if missing.is_empty() || renamed == 0 {
                for (file, key, _) in &missing {
                    println!("{file}: {key} is no longer there");
                }
                break;
            }
            work = missing;
        }
    }
    if !textual.is_empty() {
        everywhere(root, &files, &textual)?;
    }

    Ok(match refused {
        0 => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    })
}

fn check(root: &Path, paths: &[String]) -> std::io::Result<ExitCode> {
    let vocabulary = vocabulary::read(root)?;
    let chosen = plan::read(root)?;
    let wanted = named(&vocabulary);
    let mut left = 0;

    for file in tracked(root, paths)? {
        let src = std::fs::read_to_string(root.join(&file))?;

        for definition in definitions(&src, &wanted) {
            if chosen.get(&file).and_then(|keys| keys.get(&definition.key())).is_some_and(|right| right == KEEP) {
                continue;
            }
            println!("{file}:{}: {}", definition.line + 1, definition.key());
            left += 1;
        }
    }

    Ok(match left {
        0 => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    })
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (command, paths) = match arguments.split_first() {
        Some((command, paths)) => (command.as_str(), paths),
        None => ("", &[][..]),
    };
    let ran = root().and_then(|root| match command {
        "plan" => plan(&root, paths),
        "apply" => apply(&root, paths),
        "check" => check(&root, paths),
        _ => {
            eprintln!("usage: rename-words plan|apply|check [PATH...]");
            Ok(ExitCode::from(2))
        },
    });

    match ran {
        Ok(code) => code,
        Err(fault) => {
            eprintln!("rename-words: {fault}");
            ExitCode::FAILURE
        },
    }
}
