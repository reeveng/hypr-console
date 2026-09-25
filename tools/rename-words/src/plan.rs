//! `renames.plan`: what each definition becomes, written by `plan` and read
//! back by `plan`, `apply` and `check`.
//!
//! It is a file rather than a prompt because the choosing is the slow part
//! and the part a person does, and it has to survive the tree moving under it.
//! A choice written here is kept by the next `plan`, keyed by the file and the
//! definition rather than by a line number, so re-planning after a peer's
//! commit adds what is new and forgets nothing that was decided. A line whose
//! definition is already renamed stays, marked applied, so the same plan run
//! over the tree before the rename carries it out again.

use std::collections::BTreeMap;
use std::path::Path;

pub const PICK: &str = "?";

pub const KEEP: &str = "-";

pub const EVERYWHERE: &str = "*";

pub const FILE: &str = "renames.plan";

pub type Plan = BTreeMap<String, BTreeMap<String, String>>;

pub fn read(root: &Path) -> std::io::Result<Plan> {
    let mut plan = Plan::new();
    let mut here = String::new();
    let text = match std::fs::read_to_string(root.join(FILE)) {
        Ok(text) => text,
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => return Ok(plan),
        Err(fault) => return Err(fault),
    };

    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();

        if line.is_empty() {
            continue;
        }
        if let Some(section) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            here = section.to_string();
            plan.entry(here.clone()).or_default();
            continue;
        }
        if let Some((key, right)) = line.rsplit_once('=') {
            plan.entry(here.clone()).or_default().insert(key.trim().to_string(), right.trim().to_string());
        }
    }

    Ok(plan)
}

pub fn settled(right: &str) -> bool {
    right != KEEP && !right.starts_with(PICK) && !right.is_empty()
}

pub fn open(options: &[String]) -> String {
    match options.is_empty() {
        true => PICK.to_string(),
        false => format!("{PICK} {}", options.join(" | ")),
    }
}

pub fn rekey(root: &Path, file: &str, old: &str, new: &str) -> std::io::Result<()> {
    let path = root.join(FILE);
    let text = std::fs::read_to_string(&path)?;
    let heading = format!("[{file}]");
    let owned = format!(" of {old} =");
    let mut inside = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with('[') {
                inside = line == heading;
            }
            match inside && line.contains(&owned) {
                true => line.replacen(&owned, &format!(" of {new} ="), 1),
                false => line.to_string(),
            }
        })
        .collect();

    std::fs::write(path, lines.join("\n") + "\n")
}
