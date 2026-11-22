//! Immutable, fingerprint-checked source snapshots and nested byte-span edits.

use serde_json::Value;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    error::Error,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub type Outcome<T> = Result<T, Box<dyn Error>>;

pub enum Fingerprints {
    Required,
    WhenPresent,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub file: PathBuf,
    pub lo: usize,
    pub hi: usize,
}

impl Span {
    pub fn from_json(value: &Value, root: &Path) -> Outcome<Self> {
        let name = value["file"].as_str().ok_or("span has no file")?;
        let file = root.join(name).canonicalize()?;
        Ok(Self {
            file,
            lo: usize::try_from(value["lo"].as_u64().ok_or("span has no lo")?)?,
            hi: usize::try_from(value["hi"].as_u64().ok_or("span has no hi")?)?,
        })
    }
}

pub fn fingerprint(data: &[u8]) -> String {
    let mut value = 0xcbf29ce484222325_u64;
    for byte in data {
        value = (value ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("{value:016x}")
}

#[derive(Debug)]
pub struct Source {
    pub raw: Vec<u8>,
    pub text: String,
    bom: bool,
    crlf: bool,
}

impl Source {
    pub fn read(path: &Path) -> Outcome<Self> {
        Self::from_bytes(fs::read(path)?)
    }

    pub fn from_bytes(raw: Vec<u8>) -> Outcome<Self> {
        let bom = raw.starts_with(&[0xef, 0xbb, 0xbf]);
        let body = if bom { &raw[3..] } else { &raw[..] };
        let body = std::str::from_utf8(body)?;
        let linefeeds = body.bytes().filter(|byte| *byte == b'\n').count();
        let crlf = linefeeds > 0 && body.matches("\r\n").count() == linefeeds;
        let text = body.replace("\r\n", "\n");
        Ok(Self {
            raw,
            text,
            bom,
            crlf,
        })
    }

    pub fn encode(&self, text: &str) -> Vec<u8> {
        let body = if self.crlf {
            text.replace('\n', "\r\n")
        } else {
            text.to_owned()
        };
        let mut output = Vec::with_capacity(body.len() + usize::from(self.bom) * 3);
        if self.bom {
            output.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        }
        output.extend_from_slice(body.as_bytes());
        output
    }
}

type Transform = Box<dyn Fn(&Plan, &str) -> Outcome<String>>;

struct Operation {
    name: String,
    transform: Transform,
}

struct Node {
    span: Span,
    children: Vec<usize>,
    root: bool,
}

pub struct Plan {
    pub root: PathBuf,
    pub sources: BTreeMap<PathBuf, Source>,
    pub stats: BTreeMap<String, u64>,
    pub review: BTreeMap<String, u64>,
    operations: BTreeMap<Span, Vec<Operation>>,
    new_files: BTreeMap<PathBuf, Vec<u8>>,
    nodes: Vec<Node>,
    indices: BTreeMap<Span, usize>,
    roots: BTreeMap<PathBuf, usize>,
    rendered: RefCell<BTreeMap<usize, String>>,
    built: bool,
}

impl Plan {
    pub fn new<P, I, Q>(root: P, paths: I) -> Outcome<Self>
    where
        P: AsRef<Path>,
        I: IntoIterator<Item = Q>,
        Q: AsRef<Path>,
    {
        let root = root.as_ref().canonicalize()?;
        let mut sources = BTreeMap::new();
        for path in paths {
            let path = root.join(path).canonicalize()?;
            if !path.starts_with(&root) {
                return Err(format!("source outside repository: {}", path.display()).into());
            }
            if !sources.contains_key(&path) {
                sources.insert(path.clone(), Source::read(&path)?);
            }
        }
        Ok(Self {
            root,
            sources,
            stats: BTreeMap::new(),
            review: BTreeMap::new(),
            operations: BTreeMap::new(),
            new_files: BTreeMap::new(),
            nodes: vec![],
            indices: BTreeMap::new(),
            roots: BTreeMap::new(),
            rendered: RefCell::new(BTreeMap::new()),
            built: false,
        })
    }

    pub fn validate_sources(&self, records: &[Value]) -> Outcome<()> {
        self.validate_fingerprints(records, Fingerprints::Required)
    }

    pub fn validate_fingerprints(&self, records: &[Value], policy: Fingerprints) -> Outcome<()> {
        let mut supplied = BTreeMap::<PathBuf, (String, u64)>::new();
        for record in records.iter().filter(|r| r["kind"] == "source") {
            let span = Span::from_json(&record["span"], &self.root)?;
            let digest = record["fnv1a64"]
                .as_str()
                .ok_or("source has no fingerprint")?
                .to_owned();
            let bytes = record["bytes"]
                .as_u64()
                .ok_or("source has no byte length")?;
            if let Some(old) = supplied.insert(span.file.clone(), (digest.clone(), bytes))
                && old != (digest, bytes)
            {
                return Err(format!("conflicting source captures: {}", span.file.display()).into());
            }
        }
        for (path, source) in &self.sources {
            let Some((expected, bytes)) = supplied.get(path) else {
                match policy {
                    Fingerprints::WhenPresent => continue,
                    Fingerprints::Required => {
                        return Err(
                            format!(
                                "missing source fingerprint: {}; recapture current facts",
                                path.display()
                            )
                            .into(),
                        );
                    }
                }
            };
            if *expected != fingerprint(source.text.as_bytes())
                || *bytes != u64::try_from(source.text.len())?
            {
                return Err(format!("stale typed source capture: {}", path.display()).into());
            }
        }
        Ok(())
    }

    pub fn text(&self, span: &Span) -> Outcome<&str> {
        self.sources
            .get(&span.file)
            .ok_or("span file not loaded")?
            .text
            .get(span.lo..span.hi)
            .ok_or_else(|| format!("invalid UTF-8 span: {span:?}").into())
    }

    pub fn add<F>(&mut self, span: Span, name: impl Into<String>, transform: F) -> Outcome<()>
    where
        F: Fn(&Plan, &str) -> Outcome<String> + 'static,
    {
        self.text(&span)?;
        let name = name.into();
        let operations = self.operations.entry(span).or_default();
        if !operations.iter().any(|operation| operation.name == name) {
            operations.push(Operation {
                name: name.clone(),
                transform: Box::new(transform),
            });
            *self.stats.entry(name).or_default() += 1;
        }
        self.built = false;
        Ok(())
    }

    pub fn new_file(&mut self, path: impl AsRef<Path>, text: impl AsRef<[u8]>) -> Outcome<()> {
        let path = self.root.join(path);
        let parent = path
            .parent()
            .ok_or("new file has no parent")?
            .canonicalize()?;
        if !parent.starts_with(&self.root) || path.exists() {
            return Err(format!(
                "new file must be absent and inside repository: {}",
                path.display()
            )
            .into());
        }
        if self
            .new_files
            .insert(path.clone(), text.as_ref().to_vec())
            .is_some()
        {
            return Err(format!("new file supplied twice: {}", path.display()).into());
        }
        Ok(())
    }

    fn contains(parent: &Node, child: &Span) -> bool {
        if parent.root {
            return parent.span.lo <= child.lo && child.hi <= parent.span.hi;
        }
        if child.lo == child.hi {
            return parent.span.lo < child.lo && child.lo < parent.span.hi;
        }
        parent.span.lo <= child.lo && child.hi <= parent.span.hi
    }

    fn build(&mut self) -> Outcome<()> {
        self.nodes.clear();
        self.indices.clear();
        self.roots.clear();
        self.rendered.borrow_mut().clear();
        for (path, source) in &self.sources {
            let index = self.nodes.len();
            self.nodes.push(Node {
                span: Span {
                    file: path.clone(),
                    lo: 0,
                    hi: source.text.len(),
                },
                children: vec![],
                root: true,
            });
            self.roots.insert(path.clone(), index);
        }
        let mut files = BTreeMap::<PathBuf, Vec<Span>>::new();
        for span in self.operations.keys() {
            files
                .entry(span.file.clone())
                .or_default()
                .push(span.clone());
        }
        for (path, mut spans) in files {
            spans.sort_by(|a, b| {
                a.lo.cmp(&b.lo)
                    .then((a.lo != a.hi).cmp(&(b.lo != b.hi)))
                    .then(b.hi.cmp(&a.hi))
            });
            let mut stack = vec![*self.roots.get(&path).ok_or("missing root")?];
            for span in spans {
                while stack
                    .last()
                    .is_some_and(|index| !Self::contains(&self.nodes[*index], &span))
                {
                    stack.pop();
                }
                let parent = *stack.last().ok_or("edit crosses root span")?;
                if let Some(previous) = self.nodes[parent].children.last()
                    && self.nodes[*previous].span.hi > span.lo
                {
                    return Err(format!(
                        "crossing edits: {:?} and {span:?}",
                        self.nodes[*previous].span
                    )
                    .into());
                }
                let index = self.nodes.len();
                self.nodes.push(Node {
                    span: span.clone(),
                    children: vec![],
                    root: false,
                });
                self.nodes[parent].children.push(index);
                self.indices.insert(span, index);
                stack.push(index);
            }
        }
        self.built = true;
        Ok(())
    }

    fn render_node(&self, index: usize) -> Outcome<String> {
        if let Some(text) = self.rendered.borrow().get(&index).cloned() {
            return Ok(text);
        }
        let node = &self.nodes[index];
        let mut output = self.render_children(index)?;
        if !node.root {
            for operation in self.operations.get(&node.span).into_iter().flatten() {
                output = (operation.transform)(self, &output)?;
            }
        }
        self.rendered.borrow_mut().insert(index, output.clone());
        Ok(output)
    }

    fn render_children(&self, index: usize) -> Outcome<String> {
        let node = &self.nodes[index];
        let source = &self.sources[&node.span.file].text;
        let mut cursor = node.span.lo;
        let mut output = String::new();
        for child in &node.children {
            let child_node = &self.nodes[*child];
            output.push_str(
                source
                    .get(cursor..child_node.span.lo)
                    .ok_or("invalid child start")?,
            );
            output.push_str(&self.render_node(*child)?);
            cursor = child_node.span.hi;
        }
        output.push_str(
            source
                .get(cursor..node.span.hi)
                .ok_or("invalid child end")?,
        );
        Ok(output)
    }

    /// The text of a span with every edit inside it applied and the span's own edits left out.
    pub fn inner(&self, span: &Span) -> Outcome<String> {
        if !self.built {
            return Err("call outputs/build before rendering edited spans".into());
        }
        match self.indices.get(span) {
            Some(index) => self.render_children(*index),
            None => self.get(span),
        }
    }

    fn contained(&self, index: usize, span: &Span, output: &mut Vec<usize>) -> Outcome<()> {
        for child in &self.nodes[index].children {
            let child_span = &self.nodes[*child].span;
            if span.lo <= child_span.lo && child_span.hi <= span.hi {
                output.push(*child);
            } else if child_span.lo <= span.lo && span.hi <= child_span.hi {
                self.contained(*child, span, output)?;
            } else if child_span.lo < span.hi && span.lo < child_span.hi {
                return Err(
                    format!("edit crosses rendered view: {span:?} / {child_span:?}").into(),
                );
            }
        }
        Ok(())
    }

    pub fn get(&self, span: &Span) -> Outcome<String> {
        if !self.built {
            return Err("call outputs/build before rendering edited spans".into());
        }
        if let Some(index) = self.indices.get(span) {
            return self.render_node(*index);
        }
        let mut children = vec![];
        self.contained(
            *self.roots.get(&span.file).ok_or("missing source root")?,
            span,
            &mut children,
        )?;
        let source = &self.sources[&span.file].text;
        let mut cursor = span.lo;
        let mut output = String::new();
        for child in children {
            output.push_str(
                source
                    .get(cursor..self.nodes[child].span.lo)
                    .ok_or("invalid view start")?,
            );
            output.push_str(&self.render_node(child)?);
            cursor = self.nodes[child].span.hi;
        }
        output.push_str(source.get(cursor..span.hi).ok_or("invalid view end")?);
        Ok(output)
    }

    pub fn outputs(&mut self) -> Outcome<BTreeMap<PathBuf, Vec<u8>>> {
        self.build()?;
        let mut output = self.new_files.clone();
        for (path, index) in &self.roots {
            let source = &self.sources[path];
            let bytes = source.encode(&self.render_node(*index)?);
            if bytes != source.raw {
                output.insert(path.clone(), bytes);
            }
        }
        Ok(output)
    }

    pub fn unified_diff(&self, outputs: &BTreeMap<PathBuf, Vec<u8>>) -> Outcome<String> {
        let mut diff = String::new();
        for (path, output) in outputs {
            let original = self.sources.get(path);
            let old = original.map_or(Ok(""), |source| std::str::from_utf8(&source.raw))?;
            let before: Vec<_> = old.split_inclusive('\n').collect();
            let after: Vec<_> = std::str::from_utf8(output)?.split_inclusive('\n').collect();
            let prefix = before
                .iter()
                .zip(&after)
                .take_while(|(a, b)| a == b)
                .count();
            let suffix = before[prefix..]
                .iter()
                .rev()
                .zip(after[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            let start = prefix.saturating_sub(3);
            let old_changed_end = before.len() - suffix;
            let new_changed_end = after.len() - suffix;
            let old_end = (old_changed_end + 3).min(before.len());
            let new_end = (new_changed_end + 3).min(after.len());
            let relative = path.strip_prefix(&self.root)?.display();
            let old_path = if original.is_some() {
                format!("a/{relative}")
            } else {
                String::from("/dev/null")
            };
            let old_count = old_end - start;
            let new_count = new_end - start;
            let old_start = start + usize::from(old_count != 0);
            let new_start = start + usize::from(new_count != 0);
            write!(
                diff,
                "--- {old_path}\n+++ b/{relative}\n@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
            )?;

            for (prefix, lines) in [
                (' ', &before[start..prefix]),
                ('-', &before[prefix..old_changed_end]),
                ('+', &after[prefix..new_changed_end]),
                (' ', &before[old_changed_end..old_end]),
            ] {
                for line in lines {
                    write!(diff, "{prefix}{line}")?;
                    if !line.ends_with('\n') {
                        diff.push_str("\n\\ No newline at end of file\n");
                    }
                }
            }
        }
        Ok(diff)
    }

    pub fn write(
        &self,
        outputs: &BTreeMap<PathBuf, Vec<u8>>,
        backup_root: &Path,
    ) -> Outcome<PathBuf> {
        for (path, source) in &self.sources {
            if fs::read(path)? != source.raw {
                return Err(format!("source changed during planning: {}", path.display()).into());
            }
        }
        for path in self.new_files.keys() {
            if path.exists() {
                return Err(format!("new output already exists: {}", path.display()).into());
            }
        }
        fs::create_dir_all(backup_root)?;
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let backup = backup_root.join(format!("batch-{nonce}-{}", std::process::id()));
        fs::create_dir(&backup)?;
        // Back up everything before starting the first source write.
        for path in outputs.keys().filter(|path| path.exists()) {
            let destination = backup.join(path.strip_prefix(&self.root)?);
            fs::create_dir_all(destination.parent().ok_or("backup has no parent")?)?;
            fs::copy(path, destination)?;
        }
        for (path, bytes) in outputs {
            fs::create_dir_all(path.parent().ok_or("output has no parent")?)?;
            fs::write(path, bytes)?;
        }
        Ok(backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(text: &str) -> Outcome<(PathBuf, PathBuf)> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = env_temp().join(format!("edits-test-{nonce}"));
        fs::create_dir_all(&root)?;
        let path = root.join("source.rs");
        fs::write(&path, text)?;
        Ok((root, path))
    }

    fn env_temp() -> PathBuf {
        std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/tmp"))
    }

    #[test]
    fn nested_edits_use_original_offsets_and_transformed_children() -> Outcome<()> {
        let (root, path) = fixture("α + (2 as u8)")?;
        let mut plan = Plan::new(&root, [&path])?;
        let inner = Span {
            file: path.clone(),
            lo: 6,
            hi: 13,
        };
        plan.add(inner.clone(), "cast", |_, _| Ok("wrapped(2)".into()))?;
        let outer = Span {
            file: path.clone(),
            lo: 0,
            hi: 14,
        };
        plan.add(outer, "add", move |p, _| {
            Ok(format!("add(α, {})", p.get(&inner)?))
        })?;
        let outputs = plan.outputs()?;
        assert_eq!(std::str::from_utf8(&outputs[&path])?, "add(α, wrapped(2))");
        Ok(())
    }

    #[test]
    fn crossing_edits_are_rejected() -> Outcome<()> {
        let (root, path) = fixture("0123456789")?;
        let mut plan = Plan::new(&root, [&path])?;
        plan.add(
            Span {
                file: path.clone(),
                lo: 1,
                hi: 6,
            },
            "one",
            |_, _| Ok("x".into()),
        )?;
        plan.add(
            Span {
                file: path,
                lo: 4,
                hi: 8,
            },
            "two",
            |_, _| Ok("y".into()),
        )?;
        assert!(plan.outputs().is_err());
        Ok(())
    }

    #[test]
    fn bom_and_uniform_crlf_are_preserved() -> Outcome<()> {
        let source = Source::from_bytes(b"\xef\xbb\xbfline\r\nnext\r\n".to_vec())?;
        assert_eq!(source.text, "line\nnext\n");
        assert_eq!(
            source.encode("changed\nnext\n"),
            b"\xef\xbb\xbfchanged\r\nnext\r\n"
        );
        Ok(())
    }

    #[test]
    fn stale_fingerprints_are_rejected() -> Outcome<()> {
        let (root, path) = fixture("first")?;
        let plan = Plan::new(&root, [&path])?;
        let records = vec![
            serde_json::json!({"kind":"source","span":{"file":path,"lo":0,"hi":5},"fnv1a64":fingerprint(b"other"),"bytes":5}),
        ];
        assert!(plan.validate_sources(&records).is_err());
        Ok(())
    }
}
