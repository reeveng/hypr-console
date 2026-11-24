//! Source snapshots, inventories and guarded application of compiler suggestions.

use crate::edits::{Outcome, fingerprint};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub type Snapshot = BTreeMap<String, String>;

pub fn path(root: &Path, name: &str) -> Outcome<PathBuf> {
    let file = root.join(name).canonicalize()?;
    file.strip_prefix(root)?;
    Ok(file)
}

pub fn walk(directory: &Path, files: &mut BTreeSet<PathBuf>) -> Outcome<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() && entry.file_name() != "target" && entry.file_name() != ".git" {
            walk(&entry.path(), files)?;
        } else if kind.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        {
            files.insert(entry.path().canonicalize()?);
        }
    }
    Ok(())
}

pub fn snapshot(root: &Path, files: &BTreeSet<PathBuf>) -> Outcome<Snapshot> {
    files
        .iter()
        .map(|file| {
            Ok((
                file.strip_prefix(root)?.to_string_lossy().into_owned(),
                fingerprint(&fs::read(file)?),
            ))
        })
        .collect()
}

pub fn verify(root: &Path, snapshot: &Snapshot) -> Outcome<()> {
    for (name, expected) in snapshot {
        if fingerprint(&fs::read(path(root, name)?)?) != *expected {
            return Err(format!("stale capture: {name}; recapture before fixing").into());
        }
    }
    Ok(())
}

pub fn messages(files: &[PathBuf]) -> Outcome<Vec<Value>> {
    let mut result = vec![];
    for file in files {
        for line in BufReader::new(fs::File::open(file)?).lines() {
            let line = line?;
            if !line.trim_start().starts_with('{') {
                continue;
            }
            let mut record: Value = serde_json::from_str(&line)?;
            if record["message"].is_object() {
                record = record["message"].take();
            }
            if matches!(
                record["reason"].as_str(),
                Some("build-finished" | "compiler-artifact")
            ) {
                result.push(record);
            } else if record["code"]["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("explicit"))
            {
                // The batch uses structured spans and replacements. Rendered
                // source snippets can account for most of a workspace capture.
                if let Some(diagnostic) = record.as_object_mut() {
                    diagnostic.remove("rendered");
                }
                result.push(record);
            }
        }
    }
    Ok(result)
}

pub fn inventory<'a>(
    root: &Path,
    records: impl IntoIterator<Item = &'a Value>,
    snapshot: &Snapshot,
) -> Outcome<Value> {
    let mut sites = BTreeMap::new();
    for message in records {
        let Some(rule) = message["code"]["code"]
            .as_str()
            .filter(|rule| rule.starts_with("explicit"))
        else {
            continue;
        };
        let Some(description) = message["message"].as_str() else {
            continue;
        };
        let automatic = machine_suggestions(message).any(|spans| {
            spans.iter().all(|span| {
                let Some(file) = span["file_name"]
                    .as_str()
                    .and_then(|name| path(root, name).ok())
                else {
                    return false;
                };
                file.strip_prefix(root)
                    .is_ok_and(|name| snapshot.contains_key(name.to_string_lossy().as_ref()))
            })
        });
        for span in message["spans"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|span| span["is_primary"] == true)
        {
            let Some(name) = span["file_name"].as_str() else {
                continue;
            };
            let Ok(file) = path(root, name) else { continue };
            let name = file.strip_prefix(root)?.to_string_lossy().into_owned();
            if !snapshot.contains_key(&name) {
                continue;
            }
            let key = (
                rule,
                name.clone(),
                span["byte_start"].as_u64(),
                span["byte_end"].as_u64(),
                description,
            );
            let site = sites.entry(key).or_insert_with(|| json!({
                "rule": rule, "file": name, "line": span["line_start"],
                "column": span["column_start"], "message": message["message"], "automatic": false,
                "anchor": span_anchor(span),
            }));
            if automatic {
                site["automatic"] = json!(true);
            }
            record_guidance(site, message);
        }
    }
    let mut by_rule = BTreeMap::<String, usize>::new();
    let mut automatic_by_rule = BTreeMap::<String, usize>::new();
    for ((rule, _, _, _, _), site) in &sites {
        *by_rule.entry((*rule).to_owned()).or_default() += 1;
        if site["automatic"] == true {
            *automatic_by_rule.entry((*rule).to_owned()).or_default() += 1;
        }
    }
    Ok(json!({
        "total": sites.len(), "by_rule": by_rule, "automatic_by_rule": automatic_by_rule,
        "review_patterns": review_patterns(sites.values()),
        "findings": sites.into_values().collect::<Vec<_>>(),
    }))
}

fn record_guidance(site: &mut Value, message: &Value) {
    for child in message["children"].as_array().into_iter().flatten() {
        let Some(text) = child["message"].as_str() else {
            continue;
        };
        let mut spans = child["spans"].as_array().into_iter().flatten();
        match child["level"].as_str() {
            Some("help") => {
                let field = if spans.any(|span| span["suggested_replacement"].is_string()) {
                    "suggestion"
                } else {
                    "help"
                };
                if site.get(field).is_none() {
                    site[field] = json!(text);
                }
            }
            Some("note") if site.get("related").is_none() => {
                if let Some(span) = spans.find(|span| span["is_primary"] == true) {
                    site["related"] = json!({
                        "file": span["file_name"], "line": span["line_start"],
                        "column": span["column_start"], "message": text,
                    });
                }
            }
            _ => {}
        }
    }
}

pub fn write_findings<'a>(
    output: &mut impl Write,
    findings: impl IntoIterator<Item = &'a Value>,
) -> Outcome<()> {
    for site in findings {
        let file = site["file"].as_str().unwrap_or_default();
        let rule = site["rule"].as_str().unwrap_or_default();
        let message = site["message"].as_str().unwrap_or_default();
        let automatic = site["automatic"] == true;
        let status = if automatic {
            "automatic suggestion"
        } else {
            "manual review"
        };
        writeln!(
            output,
            "{file}:{}:{} [{rule}, {status}] {message}",
            site["line"], site["column"],
        )?;
        if let Some(related) = site.get("related") {
            let file = related["file"].as_str().unwrap_or_default();
            let note = related["message"].as_str().unwrap_or_default();
            writeln!(
                output,
                "  note: {file}:{}:{} {note}",
                related["line"], related["column"],
            )?;
        }
        let help = site["help"].as_str();
        let suggestion = site["suggestion"].as_str();
        if let Some(help) = help {
            writeln!(output, "  help: {help}")?;
        }
        if let Some(suggestion) = suggestion.filter(|text| Some(*text) != help) {
            writeln!(output, "  suggestion: {suggestion}")?;
        }
        if help.is_none() && suggestion.is_none() {
            if automatic {
                writeln!(
                    output,
                    "  help: preview with `just explicit-batch fix --package PACKAGE --rules {rule} --selected-rules-only`",
                )?;
            } else {
                writeln!(
                    output,
                    "  help: see tools/explicit-rust/{rule}/src/lib.rs for the rule's guidance",
                )?;
            }
        }
        writeln!(output)?;
    }
    Ok(())
}

fn span_anchor(span: &Value) -> String {
    let lines: Vec<_> = span["text"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|line| line["text"].as_str().map(str::trim))
        .collect();
    fingerprint(lines.join("\n").as_bytes())
}

pub fn regressions<'a>(baseline: &Value, current: &'a Value) -> Outcome<Vec<&'a Value>> {
    fn findings(report: &Value) -> Outcome<&[Value]> {
        let findings = report["findings"].as_array().ok_or("missing findings")?;
        if report["total"].as_u64() != Some(findings.len() as u64) {
            return Err("finding count does not match the report".into());
        }
        Ok(findings)
    }

    fn key(site: &Value) -> Outcome<[&str; 4]> {
        Ok([
            site["rule"].as_str().ok_or("finding has no rule")?,
            site["file"].as_str().ok_or("finding has no file")?,
            site["message"].as_str().ok_or("finding has no message")?,
            site["anchor"]
                .as_str()
                .ok_or("baseline predates source anchors; regenerate it")?,
        ])
    }

    let mut allowances = BTreeMap::new();
    for site in findings(baseline)? {
        *allowances.entry(key(site)?).or_insert(0usize) += 1;
    }
    let mut new = Vec::new();
    for site in findings(current)? {
        match allowances.get_mut(&key(site)?) {
            Some(left) if *left > 0 => *left -= 1,
            _ => new.push(site),
        }
    }
    Ok(new)
}

#[derive(Default)]
struct ReviewPattern<'a> {
    total: usize,
    directories: BTreeMap<String, usize>,
    example: Option<&'a Value>,
}

fn review_patterns<'a>(sites: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    let mut patterns = BTreeMap::<(&str, &str), ReviewPattern<'_>>::new();
    for site in sites.into_iter().filter(|site| site["automatic"] == false) {
        let rule = site["rule"].as_str().unwrap_or_default();
        let guidance = site["help"]
            .as_str()
            .or_else(|| site["suggestion"].as_str())
            .or_else(|| site["message"].as_str())
            .unwrap_or_default();
        let directory = site["file"]
            .as_str()
            .unwrap_or_default()
            .split('/')
            .take(2)
            .collect::<Vec<_>>()
            .join("/");
        let pattern = patterns.entry((rule, guidance)).or_default();
        pattern.total += 1;
        *pattern.directories.entry(directory).or_default() += 1;
        pattern.example.get_or_insert(site);
    }
    let mut patterns: Vec<_> = patterns.into_iter().collect();
    patterns.sort_by_key(|(key, pattern)| (std::cmp::Reverse(pattern.total), *key));
    patterns
        .into_iter()
        .map(|((rule, guidance), pattern)| {
            json!({
                "rule": rule, "guidance": guidance, "total": pattern.total,
                "directories": pattern.directories, "example": pattern.example,
            })
        })
        .collect()
}

fn machine_suggestions(message: &Value) -> impl Iterator<Item = &[Value]> {
    message["children"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|child| {
            let spans = child["spans"].as_array()?;
            (!spans.is_empty()
                && spans.iter().all(|span| {
                    span["suggestion_applicability"] == "MachineApplicable"
                        && span["expansion"].is_null()
                        && span["suggested_replacement"].is_string()
                }))
            .then_some(spans.as_slice())
        })
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Replacement {
    file: PathBuf,
    start: usize,
    end: usize,
    text: String,
}

type Group = BTreeSet<Replacement>;

impl Replacement {
    fn from_span(root: &Path, span: &Value) -> Option<Self> {
        Some(Self {
            file: path(root, span["file_name"].as_str()?).ok()?,
            start: usize::try_from(span["byte_start"].as_u64()?).ok()?,
            end: usize::try_from(span["byte_end"].as_u64()?).ok()?,
            text: span["suggested_replacement"].as_str()?.to_owned(),
        })
    }

    fn overlaps(&self, other: &Self) -> bool {
        self != other
            && self.file == other.file
            && if self.start == self.end || other.start == other.end {
                self.start <= other.end && other.start <= self.end
            } else {
                self.start < other.end && other.start < self.end
            }
    }
}

fn selected_rule(message: &Value, rules: &[String]) -> bool {
    message["code"]["code"].as_str().is_some_and(|rule| {
        rule.starts_with("explicit") && (rules.is_empty() || rules.iter().any(|name| name == rule))
    })
}

fn owned_suggestions<'a>(
    root: &'a Path,
    message: &'a Value,
    packages: &'a BTreeMap<String, PathBuf>,
) -> impl Iterator<Item = Group> + 'a {
    machine_suggestions(message).filter_map(move |spans| {
        let group = spans
            .iter()
            .map(|span| Replacement::from_span(root, span))
            .collect::<Option<Group>>()?;
        group
            .iter()
            .all(|edit| {
                edit.file
                    .extension()
                    .is_some_and(|extension| extension == "rs")
                    && packages
                        .values()
                        .any(|directory| edit.file.starts_with(directory))
            })
            .then_some(group)
    })
}

pub struct Fixes(Vec<Group>);

pub fn plan_suggestions<'a>(
    root: &Path,
    records: impl IntoIterator<Item = &'a Value>,
    packages: &BTreeMap<String, PathBuf>,
    rules: &[String],
) -> Fixes {
    let mut by_site = BTreeMap::<_, BTreeSet<Group>>::new();
    for message in records
        .into_iter()
        .filter(|record| selected_rule(record, rules))
    {
        let Some(primary) = message["spans"]
            .as_array()
            .and_then(|spans| spans.iter().find(|span| span["is_primary"] == true))
        else {
            continue;
        };
        let Some(file) = primary["file_name"]
            .as_str()
            .and_then(|name| path(root, name).ok())
        else {
            continue;
        };
        let key = (
            file,
            primary["byte_start"].as_u64(),
            primary["byte_end"].as_u64(),
            message["code"]["code"].as_str().unwrap_or_default(),
        );
        for group in owned_suggestions(root, message, packages) {
            by_site.entry(key.clone()).or_default().insert(group);
        }
    }

    let mut accepted = BTreeMap::<PathBuf, BTreeSet<Replacement>>::new();
    let mut used = BTreeSet::new();
    let mut fixes = Vec::new();
    for mut suggestions in by_site.into_values().filter(|groups| groups.len() == 1) {
        let Some(group) = suggestions.pop_first() else {
            continue;
        };
        if group.iter().any(|edit| {
            group.iter().any(|other| edit.overlaps(other))
                || accepted
                    .get(&edit.file)
                    .is_some_and(|earlier| earlier.iter().any(|other| edit.overlaps(other)))
        }) || !used.insert(group.clone())
        {
            continue;
        }
        for edit in &group {
            accepted
                .entry(edit.file.clone())
                .or_default()
                .insert(edit.clone());
        }
        fixes.push(group);
    }
    Fixes(fixes)
}

impl Fixes {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn snapshot(
        &self,
        root: &Path,
        original: &Snapshot,
        window: std::ops::Range<usize>,
    ) -> Outcome<Snapshot> {
        let groups = self.0.get(window).ok_or("invalid suggestion range")?;
        groups
            .iter()
            .flatten()
            .map(|edit| {
                let name = edit.file.strip_prefix(root)?.to_string_lossy();
                let hash = original
                    .get(name.as_ref())
                    .ok_or("suggestion outside the source snapshot")?;
                Ok((name.into_owned(), hash.clone()))
            })
            .collect()
    }

    pub fn apply(
        &self,
        root: &Path,
        snapshot: &Snapshot,
        window: std::ops::Range<usize>,
        minimum: usize,
        directory: &Path,
    ) -> Outcome<(usize, Snapshot)> {
        let groups = self.0.get(window).ok_or("invalid suggestion range")?;
        if groups.len() < minimum {
            return Ok((0, Snapshot::new()));
        }
        let mut by_file = BTreeMap::<PathBuf, BTreeSet<&Replacement>>::new();
        for edit in groups.iter().flatten() {
            by_file.entry(edit.file.clone()).or_default().insert(edit);
        }
        let changed = apply_edits(root, snapshot, by_file, directory)?;
        Ok((groups.len(), changed))
    }
}

fn apply_edits(
    root: &Path,
    snapshot: &Snapshot,
    by_file: BTreeMap<PathBuf, BTreeSet<&Replacement>>,
    directory: &Path,
) -> Outcome<Snapshot> {
    let mut outputs = Vec::new();
    let mut changed = Snapshot::new();
    for (file, edits) in by_file {
        let original = fs::read(&file)?;
        let name = file.strip_prefix(root)?.to_string_lossy();
        let hash = fingerprint(&original);
        if snapshot.get(name.as_ref()) != Some(&hash) {
            return Err(format!("stale source before applying suggestions: {name}").into());
        }
        // rustc's JSON reports offsets in the original file, including a BOM
        // and carriage returns. Keep all bytes outside the edited spans.
        let mut text = std::str::from_utf8(&original)?.to_owned();
        let crlf =
            text.contains("\r\n") && text.matches("\r\n").count() == text.matches('\n').count();
        for edit in edits.into_iter().rev() {
            if text.get(edit.start..edit.end).is_none() {
                return Err(
                    format!("invalid suggestion offset in {name} at {}", edit.start).into(),
                );
            }
            let replacement = if crlf {
                edit.text.replace("\r\n", "\n").replace('\n', "\r\n")
            } else {
                edit.text.clone()
            };
            text.replace_range(edit.start..edit.end, &replacement);
        }
        if original != text.as_bytes() {
            let name = name.into_owned();
            changed.insert(name.clone(), fingerprint(text.as_bytes()));
            outputs.push((name, file, hash, text));
        }
    }
    if changed.is_empty() {
        return Ok(changed);
    }
    backup(root, snapshot, directory)?;
    write_manifest(root, &changed, directory)?;
    for (index, (_, file, hash, text)) in outputs.iter().enumerate() {
        if let Err(error) = write_source(file, hash, text.as_bytes()) {
            let applied = outputs[..index]
                .iter()
                .map(|(name, _, _, _)| (name.clone(), changed[name].clone()))
                .collect();
            write_manifest(root, &applied, directory)
                .and_then(|()| restore(directory))
                .map_err(|restore| {
                    format!(
                        "{error}; could not restore {}: {restore}",
                        directory.display()
                    )
                })?;
            return Err(error);
        }
    }
    Ok(changed)
}

fn write_source(file: &Path, expected: &str, bytes: &[u8]) -> Outcome<()> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let pending = file.with_extension(format!("explicit-fix-{}-{nonce}", std::process::id()));
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)?;
    let result = (|| {
        output.write_all(bytes)?;
        output.set_permissions(fs::metadata(file)?.permissions())?;
        drop(output);
        if fingerprint(&fs::read(file)?) != expected {
            return Err(format!(
                "source changed while applying suggestions: {}",
                file.display()
            )
            .into());
        }
        fs::rename(&pending, file)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}

pub fn automatic_packages(
    root: &Path,
    records: &[Value],
    packages: &BTreeMap<String, PathBuf>,
    rules: &[String],
) -> BTreeMap<String, usize> {
    let mut groups = BTreeMap::<String, BTreeSet<Group>>::new();
    for message in records.iter().filter(|record| selected_rule(record, rules)) {
        for group in owned_suggestions(root, message, packages) {
            let owners: BTreeSet<_> = group
                .iter()
                .filter_map(|edit| {
                    packages
                        .iter()
                        .find(|(_, directory)| edit.file.starts_with(directory))
                })
                .map(|(package, _)| package)
                .collect();
            for owner in owners {
                groups
                    .entry(owner.clone())
                    .or_default()
                    .insert(group.clone());
            }
        }
    }
    groups
        .into_iter()
        .map(|(name, groups)| (name, groups.len()))
        .collect()
}

pub fn choose_batch(costs: &BTreeMap<String, usize>, target: usize) -> Vec<String> {
    let mut ranked: Vec<_> = costs.iter().filter(|(_, cost)| **cost > 0).collect();
    ranked.sort_by_key(|(name, cost)| (std::cmp::Reverse(**cost), *name));

    let mut chosen = vec![];
    let mut count = 0usize;
    for (name, cost) in ranked {
        if !chosen.is_empty() && count.saturating_add(*cost) > target {
            continue;
        }
        chosen.push(name.clone());
        count = count.saturating_add(*cost);
    }
    chosen
}

pub fn backup(root: &Path, snapshot: &Snapshot, directory: &Path) -> Outcome<()> {
    verify(root, snapshot)?;
    fs::create_dir_all(directory)?;
    for name in snapshot.keys().filter(|name| name.ends_with(".rs")) {
        let saved = directory.join(name);
        fs::create_dir_all(saved.parent().ok_or("backup has no parent")?)?;
        fs::copy(path(root, name)?, saved)?;
    }
    fs::write(
        directory.join("before.json"),
        serde_json::to_vec_pretty(snapshot)?,
    )?;
    verify(root, snapshot)
}

#[cfg(test)]
fn seal_backup(root: &Path, before: &Snapshot, directory: &Path) -> Outcome<Snapshot> {
    let mut changed = Snapshot::new();
    for (name, expected) in before.iter().filter(|(name, _)| name.ends_with(".rs")) {
        let current = fingerprint(&fs::read(path(root, name)?)?);
        if current != *expected {
            changed.insert(name.clone(), current);
        }
    }
    write_manifest(root, &changed, directory)?;
    Ok(changed)
}

fn write_manifest(root: &Path, changed: &Snapshot, directory: &Path) -> Outcome<()> {
    fs::write(
        directory.join("batch-manifest.json"),
        serde_json::to_vec_pretty(&json!({"root": root, "after": changed}))?,
    )?;
    Ok(())
}

pub fn restore(directory: &Path) -> Outcome<()> {
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("batch-manifest.json"))?)?;
    let root = Path::new(manifest["root"].as_str().ok_or("backup has no root")?).canonicalize()?;
    let after: Snapshot = serde_json::from_value(manifest["after"].clone())?;
    let before: Snapshot = serde_json::from_slice(&fs::read(directory.join("before.json"))?)?;
    let mut restores = Vec::new();
    for (name, expected) in &after {
        let source = path(&root, name)?;
        let current = fingerprint(&fs::read(&source)?);
        let original = fs::read(directory.join(name))?;
        let original_hash = fingerprint(&original);
        if before.get(name) != Some(&original_hash) {
            return Err(format!("backup changed: {name}").into());
        }
        if current == original_hash {
            continue;
        }
        if current != *expected {
            return Err(format!("refusing to overwrite subsequent changes: {name}").into());
        }
        restores.push((source, expected, original));
    }
    for (source, expected, original) in restores {
        write_source(&source, expected, &original)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
        file: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temporary =
                env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = temporary
                .join("builds/explicit-batch-tests")
                .join(format!("{}-{nonce}", std::process::id()));
            let file = root.join("crates/example/src/lib.rs");
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(
                &file,
                b"\xef\xbb\xbf// existing user change\r\nfn caf\xc3\xa9() {}\r\n",
            )
            .unwrap();
            Self {
                root: root.canonicalize().unwrap(),
                file: file.canonicalize().unwrap(),
            }
        }

        fn snapshot(&self) -> Snapshot {
            snapshot(&self.root, &BTreeSet::from([self.file.clone()])).unwrap()
        }

        fn message(&self, at: u64) -> Value {
            let span = json!({"file_name": self.file, "byte_start": at, "byte_end": at,
                "is_primary": true, "line_start": 2, "column_start": 1,
                "suggestion_applicability": "MachineApplicable", "suggested_replacement": "\n", "expansion": null});
            json!({"message": "a decision wants space", "code": {"code": "explicit013_breathing_room"},
                "spans": [span.clone()], "children": [{"spans": [span]}]})
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn inventories_count_each_primary_span_once_across_targets() {
        let fixture = Fixture::new();
        let mut without_fix = fixture.message(0);
        without_fix["children"] = json!([]);
        let mut review = fixture.message(4);
        review["children"] = json!([]);
        let records = vec![without_fix.clone(), fixture.message(0), without_fix, review];
        let result = inventory(&fixture.root, &records, &fixture.snapshot()).unwrap();
        assert_eq!(result["total"], 2);
        assert_eq!(result["by_rule"]["explicit013_breathing_room"], 2);
        assert_eq!(result["automatic_by_rule"]["explicit013_breathing_room"], 1);
        assert_eq!(result["findings"][0]["automatic"], true);
        assert_eq!(result["findings"][1]["automatic"], false);
    }

    #[test]
    fn findings_show_lint_help_suggestion_and_related_location() {
        let fixture = Fixture::new();
        let mut automatic = fixture.message(0);
        automatic["children"][0]["level"] = json!("help");
        automatic["children"][0]["message"] = json!("add a blank line");
        let mut manual = fixture.message(4);
        manual["code"]["code"] = json!("explicit105_no_blocking_in_handler");
        manual["message"] = json!("blocking call in an event handler");
        manual["children"] = json!([
            {"level": "note", "message": "this helper blocks", "spans": [{
                "file_name": "crates/example/src/lib.rs", "line_start": 7,
                "column_start": 5, "is_primary": true,
            }]},
            {"level": "help", "message": "offer an effect and handle its reply", "spans": []},
        ]);

        let report = inventory(&fixture.root, [&automatic, &manual], &fixture.snapshot()).unwrap();
        let mut output = Vec::new();
        write_findings(&mut output, report["findings"].as_array().unwrap()).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            concat!(
                "crates/example/src/lib.rs:2:1 [explicit013_breathing_room, automatic suggestion] a decision wants space\n",
                "  suggestion: add a blank line\n\n",
                "crates/example/src/lib.rs:2:1 [explicit105_no_blocking_in_handler, manual review] blocking call in an event handler\n",
                "  note: crates/example/src/lib.rs:7:5 this helper blocks\n",
                "  help: offer an effect and handle its reply\n\n",
            ),
        );
    }

    #[test]
    fn baseline_accepts_shifted_findings_but_rejects_new_sites_and_duplicates() {
        let fixture = Fixture::new();
        let mut original = fixture.message(0);
        original["spans"][0]["text"] = json!([{"text": "  first[index]"}]);
        let mut shifted = fixture.message(4);
        shifted["spans"][0]["line_start"] = json!(7);
        shifted["spans"][0]["text"] = json!([{"text": "    first[index]"}]);
        let mut duplicate = shifted.clone();
        duplicate["spans"][0]["byte_start"] = json!(5);
        let mut changed = fixture.message(6);
        changed["spans"][0]["text"] = json!([{"text": "second[index]"}]);

        let snapshot = fixture.snapshot();
        let baseline = inventory(&fixture.root, [&original], &snapshot).unwrap();
        let current =
            inventory(&fixture.root, [&shifted, &duplicate, &changed], &snapshot).unwrap();
        assert_eq!(
            regressions(&baseline, &current).unwrap(),
            vec![&current["findings"][1], &current["findings"][2]],
        );
    }

    #[test]
    fn review_patterns_group_guidance_across_packages() {
        let findings = [
            json!({"rule": "explicit015", "file": "crates/one/src/lib.rs", "help": "name the overflow policy", "automatic": false}),
            json!({"rule": "explicit015", "file": "forks/two/src/lib.rs", "help": "name the overflow policy", "automatic": false}),
            json!({"rule": "explicit015", "file": "crates/three/src/lib.rs", "help": "name the overflow policy", "automatic": true}),
        ];
        let patterns = review_patterns(&findings);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0]["total"], 2);
        assert_eq!(
            patterns[0]["directories"],
            json!({"crates/one": 1, "forks/two": 1})
        );
    }

    #[test]
    fn distinct_discriminant_pairs_at_one_enum_span_are_counted() {
        let fixture = Fixture::new();
        let mut first = fixture.message(0);
        first["code"]["code"] = json!("explicit070_spaced_discriminants");
        first["message"] = json!("State's discriminants Started and Stopped are one bit apart");
        first["children"] = json!([]);
        let mut second = first.clone();
        second["message"] = json!("State's discriminants Started and Paused are one bit apart");
        let records = [first.clone(), second, first];
        let result = inventory(&fixture.root, &records, &fixture.snapshot()).unwrap();
        assert_eq!(result["total"], 2);
        assert_eq!(result["by_rule"]["explicit070_spaced_discriminants"], 2);
    }

    #[test]
    fn cargo_envelopes_and_raw_diagnostics_are_both_read() {
        let fixture = Fixture::new();
        let file = fixture.root.join("rules.jsonl");
        let raw = fixture.message(0);
        let wrapped = json!({"reason": "compiler-message", "message": raw});
        fs::write(&file, format!("progress\n{raw}\n{wrapped}\n")).unwrap();
        let records = messages(&[file]).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0], records[1]);
    }

    #[test]
    fn stale_sources_are_refused_before_backup() {
        let fixture = Fixture::new();
        let before = fixture.snapshot();
        fs::write(&fixture.file, "fn changed() {}\n").unwrap();
        assert!(
            verify(&fixture.root, &before)
                .unwrap_err()
                .to_string()
                .contains("stale capture")
        );
        assert!(backup(&fixture.root, &before, &fixture.root.join("backup")).is_err());
    }

    #[test]
    fn restore_preserves_dirty_work_and_exact_original_encoding() {
        let fixture = Fixture::new();
        let before = fixture.snapshot();
        let original = fs::read(&fixture.file).unwrap();
        let directory = fixture.root.join("backup");
        backup(&fixture.root, &before, &directory).unwrap();
        fs::write(&fixture.file, "fn changed() {}\n").unwrap();
        seal_backup(&fixture.root, &before, &directory).unwrap();
        restore(&directory).unwrap();
        assert_eq!(fs::read(&fixture.file).unwrap(), original);
    }

    #[test]
    fn restore_recovers_an_interrupted_batch() {
        let fixture = Fixture::new();
        let other = fixture.file.with_file_name("other.rs");
        let original = b"fn other() {}\n";
        fs::write(&other, original).unwrap();
        let before = snapshot(
            &fixture.root,
            &BTreeSet::from([fixture.file.clone(), other.clone()]),
        )
        .unwrap();
        let directory = fixture.root.join("backup");
        backup(&fixture.root, &before, &directory).unwrap();

        let updated = b"fn changed() {}\n";
        let after: Snapshot = before
            .keys()
            .map(|name| (name.clone(), fingerprint(updated)))
            .collect();
        write_manifest(&fixture.root, &after, &directory).unwrap();
        fs::write(&fixture.file, updated).unwrap();

        restore(&directory).unwrap();
        verify(&fixture.root, &before).unwrap();
    }

    #[test]
    fn restore_never_overwrites_changes_made_after_the_batch() {
        let fixture = Fixture::new();
        let before = fixture.snapshot();
        let directory = fixture.root.join("backup");
        backup(&fixture.root, &before, &directory).unwrap();
        fs::write(&fixture.file, "fn fixed() {}\n").unwrap();
        seal_backup(&fixture.root, &before, &directory).unwrap();
        fs::write(&fixture.file, "fn subsequent() {}\n").unwrap();
        assert!(
            restore(&directory)
                .unwrap_err()
                .to_string()
                .contains("subsequent changes")
        );
        assert_eq!(
            fs::read_to_string(&fixture.file).unwrap(),
            "fn subsequent() {}\n"
        );
    }

    #[test]
    fn five_thousand_suggestions_are_one_batch_and_duplicates_are_free() {
        let fixture = Fixture::new();
        let packages = BTreeMap::from([("example".into(), fixture.root.join("crates/example"))]);
        let mut records: Vec<_> = (0..5000).map(|at| fixture.message(at)).collect();
        records.extend(records.clone());
        let costs = automatic_packages(&fixture.root, &records, &packages, &[]);
        assert_eq!(costs["example"], 5000);
        assert_eq!(choose_batch(&costs, 5000), ["example"]);
    }

    #[test]
    fn batch_budget_never_splits_a_crate() {
        let costs = BTreeMap::from([("a-small".into(), 100), ("z-large".into(), 6000)]);
        assert_eq!(choose_batch(&costs, 5000), ["z-large"]);
    }

    #[test]
    fn applies_compatible_groups_once_at_original_byte_offsets() {
        let fixture = Fixture::new();
        let packages = BTreeMap::from([("example".into(), fixture.root.join("crates/example"))]);
        let before = fixture.snapshot();
        let original = fs::read_to_string(&fixture.file).unwrap();
        let at = u64::try_from(original.find("fn").unwrap()).unwrap();
        let newline = fixture.message(at);
        let mut other_target = fixture.message(at);
        other_target["code"]["code"] = json!("explicit051_no_machine_width");
        let records = [newline.clone(), newline, other_target];
        let fixes = plan_suggestions(&fixture.root, &records, &packages, &[]);
        let directory = fixture.root.join("backup");
        assert_eq!(
            fixes
                .apply(&fixture.root, &before, 0..fixes.len(), 2, &directory)
                .unwrap()
                .0,
            0,
        );
        assert_eq!(
            fixes
                .apply(&fixture.root, &before, 0..fixes.len(), 1, &directory)
                .unwrap()
                .0,
            1,
        );
        assert_eq!(
            fs::read(&fixture.file).unwrap(),
            b"\xef\xbb\xbf// existing user change\r\n\r\nfn caf\xc3\xa9() {}\r\n",
        );
    }

    #[test]
    fn a_conflicting_edit_defers_its_entire_multipart_suggestion() {
        let fixture = Fixture::new();
        let packages = BTreeMap::from([("example".into(), fixture.root.join("crates/example"))]);
        let before = fixture.snapshot();
        let original = fs::read_to_string(&fixture.file).unwrap();
        let at = u64::try_from(original.find("fn").unwrap()).unwrap();
        let first = fixture.message(at);
        let mut conflicts = fixture.message(at);
        conflicts["code"]["code"] = json!("explicit051_no_machine_width");
        conflicts["children"][0]["spans"][0]["suggested_replacement"] = json!("/* conflicting */");
        let unrelated_part = fixture.message(at + 3);
        conflicts["children"][0]["spans"]
            .as_array_mut()
            .unwrap()
            .push(unrelated_part["children"][0]["spans"][0].clone());
        let fixes = plan_suggestions(&fixture.root, &[first, conflicts], &packages, &[]);
        let directory = fixture.root.join("backup");
        assert_eq!(
            fixes
                .apply(&fixture.root, &before, 0..fixes.len(), 1, &directory)
                .unwrap()
                .0,
            1
        );
        assert_eq!(
            fs::read(&fixture.file).unwrap(),
            b"\xef\xbb\xbf// existing user change\r\n\r\nfn caf\xc3\xa9() {}\r\n",
        );
    }

    #[test]
    fn applying_part_of_a_plan_keeps_multipart_edits_together() {
        let fixture = Fixture::new();
        let packages = BTreeMap::from([("example".into(), fixture.root.join("crates/example"))]);
        let before = fixture.snapshot();
        let original = fs::read_to_string(&fixture.file).unwrap();
        let at = u64::try_from(original.find("fn").unwrap()).unwrap();
        let first = fixture.message(at);
        let mut multipart = fixture.message(at + 3);
        multipart["children"][0]["spans"]
            .as_array_mut()
            .unwrap()
            .push(fixture.message(at + 10)["children"][0]["spans"][0].clone());
        let fixes = plan_suggestions(&fixture.root, &[first, multipart], &packages, &[]);
        assert_eq!(fixes.len(), 2);
        let directory = fixture.root.join("backup");
        assert_eq!(
            fixes
                .apply(&fixture.root, &before, 1..2, 1, &directory)
                .unwrap()
                .0,
            1
        );
        let actual = fs::read_to_string(&fixture.file).unwrap();
        let mut expected = original;
        expected.insert_str(at as usize + 10, "\r\n");
        expected.insert_str(at as usize + 3, "\r\n");
        assert_eq!(actual, expected);
    }

    #[test]
    fn uncertain_suggestions_and_unowned_multipart_groups_are_excluded() {
        let fixture = Fixture::new();
        let packages = BTreeMap::from([("example".into(), fixture.root.join("crates/example"))]);
        let mut uncertain = fixture.message(1);
        uncertain["children"][0]["spans"][0]["suggestion_applicability"] = json!("MaybeIncorrect");
        let other = fixture.root.join("other.rs");
        fs::write(&other, "fn other() {}\n").unwrap();
        let mut multipart = fixture.message(0);
        let mut unowned = multipart["children"][0]["spans"][0].clone();
        unowned["file_name"] = json!(other);
        multipart["children"][0]["spans"]
            .as_array_mut()
            .unwrap()
            .push(unowned);
        let costs = automatic_packages(&fixture.root, &[uncertain, multipart], &packages, &[]);
        assert!(costs.is_empty());
    }
}
