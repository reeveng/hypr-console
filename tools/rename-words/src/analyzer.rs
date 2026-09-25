//! rust-analyzer, spoken to over its language server protocol, for one thing:
//! rename this definition and every use that really refers to it.
//!
//! A text sweep cannot tell `Missing` the surface from `Missing` the package,
//! a glob import from a local of the same spelling, or a field from the
//! shorthand that binds it. The compiler's own model of the tree can, and
//! rust-analyzer is that model with a rename built on it. Every file it edits
//! is written back to disk and told to it again, so what it answers next is
//! about the tree as it now is.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

use crate::definitions::Definition;

pub struct Analyzer {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
    opened: BTreeMap<PathBuf, u64>,
}

fn binary() -> String {
    Command::new("rustup")
        .args(["which", "rust-analyzer"])
        .output()
        .ok()
        .and_then(|found| String::from_utf8(found.stdout).ok())
        .map(|found| found.trim().to_string())
        .filter(|found| !found.is_empty())
        .unwrap_or_else(|| "rust-analyzer".to_string())
}

fn uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

impl Analyzer {
    pub fn start(root: &Path) -> std::io::Result<Analyzer> {
        let mut child = Command::new(binary()).current_dir(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let input = child.stdin.take().ok_or_else(|| std::io::Error::other("rust-analyzer has no stdin"))?;
        let output = BufReader::new(child.stdout.take().ok_or_else(|| std::io::Error::other("rust-analyzer has no stdout"))?);
        let mut analyzer = Analyzer { child, input, output, next: 0, opened: BTreeMap::new() };

        analyzer.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": uri(root),
                "capabilities": {
                    "workspace": { "workspaceEdit": { "documentChanges": true } },
                    "experimental": { "serverStatusNotification": true },
                },
                "initializationOptions": { "checkOnSave": false },
            }),
        )?;
        analyzer.notify("initialized", json!({}))?;
        eprintln!("rust-analyzer is reading the workspace");
        analyzer.until_quiet()?;

        Ok(analyzer)
    }

    fn send(&mut self, message: &Value) -> std::io::Result<()> {
        let body = message.to_string();

        write!(self.input, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
        self.input.flush()
    }

    fn receive(&mut self) -> std::io::Result<Value> {
        let mut length = 0;

        loop {
            let mut line = String::new();

            if self.output.read_line(&mut line)? == 0 {
                return Err(std::io::Error::other("rust-analyzer stopped"));
            }
            let line = line.trim_end();

            if line.is_empty() {
                break;
            }
            if let Some(n) = line.strip_prefix("Content-Length: ") {
                length = n.parse().map_err(std::io::Error::other)?;
            }
        }
        let mut body = vec![0; length];

        self.output.read_exact(&mut body)?;
        let message: Value = serde_json::from_slice(&body)?;

        if message.get("id").is_some() && message.get("method").is_some() {
            self.send(&json!({ "jsonrpc": "2.0", "id": message["id"], "result": null }))?;
        }

        Ok(message)
    }

    fn until_quiet(&mut self) -> std::io::Result<()> {
        loop {
            let message = self.receive()?;

            if message["method"] == "experimental/serverStatus" && message["params"]["quiescent"] == true {
                return Ok(());
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> std::io::Result<Value> {
        self.next += 1;
        let id = self.next;

        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        loop {
            let message = self.receive()?;

            if message["id"] == id && message.get("method").is_none() {
                return Ok(message);
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) -> std::io::Result<()> {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    fn changed(&mut self, path: &Path, text: &str) -> std::io::Result<()> {
        let version = self.opened.get(path).copied().unwrap_or(0) + 1;
        let document = uri(path);

        match self.opened.contains_key(path) {
            true => self.notify(
                "textDocument/didChange",
                json!({ "textDocument": { "uri": document, "version": version }, "contentChanges": [{ "text": text }] }),
            )?,
            false => self.notify(
                "textDocument/didOpen",
                json!({ "textDocument": { "uri": document, "languageId": "rust", "version": version, "text": text } }),
            )?,
        }
        self.opened.insert(path.to_path_buf(), version);

        Ok(())
    }

    pub fn rename(&mut self, path: &Path, definition: &Definition, new: &str) -> std::io::Result<Result<Vec<PathBuf>, String>> {
        let text = std::fs::read_to_string(path)?;

        self.changed(path, &text)?;
        let reply = self.request(
            "textDocument/rename",
            json!({
                "textDocument": { "uri": uri(path) },
                "position": { "line": definition.line, "character": definition.column },
                "newName": new,
            }),
        )?;

        if let Some(fault) = reply.get("error") {
            return Ok(Err(fault["message"].as_str().unwrap_or("refused").to_string()));
        }
        let result = &reply["result"];
        let mut changes: Vec<(String, Vec<Value>)> = Vec::new();

        for change in result["documentChanges"].as_array().into_iter().flatten() {
            if let (Some(document), Some(edits)) = (change["textDocument"]["uri"].as_str(), change["edits"].as_array()) {
                changes.push((document.to_string(), edits.clone()));
            }
        }
        for (document, edits) in result["changes"].as_object().into_iter().flatten() {
            changes.push((document.clone(), edits.as_array().cloned().unwrap_or_default()));
        }
        let mut clashing = Vec::new();

        for (document, _) in &changes {
            let target = PathBuf::from(document.trim_start_matches("file://"));
            let code = crate::lexing::blank(&std::fs::read_to_string(&target)?);

            if declares(&code, new) {
                clashing.push(target.display().to_string());
            }
        }
        if !clashing.is_empty() {
            return Ok(Err(format!("{new} is already a name in {}", clashing.join(", "))));
        }
        let mut touched = Vec::new();

        for (document, edits) in changes {
            let target = PathBuf::from(document.trim_start_matches("file://"));
            let before = std::fs::read_to_string(&target)?;
            let edits = match definition.kind {
                "field" => shorthand(&before, &definition.word, edits),
                _ => edits,
            };
            let text = edited(&before, &edits);

            std::fs::write(&target, &text)?;
            self.changed(&target, &text)?;
            touched.push(target);
        }

        Ok(Ok(touched))
    }
}

impl Drop for Analyzer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn shorthand(text: &str, old: &str, edits: Vec<Value>) -> Vec<Value> {
    edits
        .into_iter()
        .map(|mut edit| {
            let from = offset(text, &edit["range"]["start"]);
            let to = offset(text, &edit["range"]["end"]);
            let before = text[..from].trim_end().chars().last();
            let after = text[to..].trim_start().chars().next();
            let new = edit["newText"].as_str().unwrap_or("").to_string();

            if &text[from..to] == old && !new.contains(':') && matches!(before, Some('{' | ',')) && matches!(after, Some(',' | '}')) {
                edit["newText"] = Value::String(format!("{new}: {old}"));
            }
            let same = format!(": {new}");

            if &text[from..to] == old && matches!(before, Some('{' | ',')) && text[to..].starts_with(&same) {
                let rest = text[to + same.len()..].trim_start().chars().next();

                if matches!(rest, Some(',' | '}')) {
                    let line = edit["range"]["end"]["line"].clone();
                    let character = edit["range"]["end"]["character"].as_u64().unwrap_or(0) + u64::try_from(same.len()).unwrap_or(0);

                    edit["range"]["end"] = serde_json::json!({ "line": line, "character": character });
                }
            }

            edit
        })
        .collect()
}

fn declares(code: &str, name: &str) -> bool {
    let words: Vec<(usize, &str)> = crate::lexing::identifiers(code).collect();
    let item = words.windows(2).any(|pair| match pair {
        [(_, kind), (_, word)] => *word == name && matches!(*kind, "struct" | "enum" | "type" | "trait" | "union" | "mod" | "fn"),
        _ => false,
    });
    let imported = code.split(';').filter_map(|statement| statement.trim_start().strip_prefix("use ").or_else(|| statement.split("pub use ").nth(1))).any(|path| {
        crate::lexing::identifiers(path).any(|(_, word)| word == name)
    });

    item || imported
}

fn offset(text: &str, position: &Value) -> usize {
    let line = usize::try_from(position["line"].as_u64().unwrap_or(0)).unwrap_or(0);
    let character = usize::try_from(position["character"].as_u64().unwrap_or(0)).unwrap_or(0);
    let start = text.split_inclusive('\n').take(line).map(str::len).sum::<usize>();
    let mut units = 0;
    let mut bytes = 0;

    for c in text[start..].chars() {
        if units >= character || c == '\n' {
            break;
        }
        units += c.len_utf16();
        bytes += c.len_utf8();
    }

    start + bytes
}

fn edited(text: &str, edits: &[Value]) -> String {
    let mut spans: Vec<(usize, usize, &str)> = edits
        .iter()
        .map(|edit| (offset(text, &edit["range"]["start"]), offset(text, &edit["range"]["end"]), edit["newText"].as_str().unwrap_or("")))
        .collect();
    let mut out = text.to_string();

    spans.sort_by(|a, b| b.0.cmp(&a.0));
    for (from, to, new) in spans {
        out.replace_range(from..to, new);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{declares, edited, shorthand};
    use serde_json::json;

    fn at(line: u64, from: u64, to: u64, new: &str) -> serde_json::Value {
        json!({ "range": { "start": { "line": line, "character": from }, "end": { "line": line, "character": to } }, "newText": new })
    }

    #[test]
    fn a_field_written_in_shorthand_keeps_the_local_it_binds() {
        let text = "Point { across: 1, down }";
        let edits = shorthand(text, "down", vec![at(0, 19, 23, "y")]);

        assert_eq!(edited(text, &edits), "Point { across: 1, y: down }");
    }

    #[test]
    fn a_field_bound_to_a_local_of_its_new_name_becomes_shorthand() {
        let text = "Point { across: x, y: top }";
        let edits = shorthand(text, "across", vec![at(0, 8, 14, "x")]);

        assert_eq!(edited(text, &edits), "Point { x, y: top }");
    }

    #[test]
    fn a_name_is_taken_where_it_is_defined_or_imported_and_not_where_it_is_only_used() {
        assert!(declares("use a::b::{Binding, Input};", "Binding"));
        assert!(declares("pub struct Touch { at: u8 }", "Touch"));
        assert!(declares("fn press(&mut self) {}", "press"));
        assert!(!declares("let x = Option::None; match y { None => 1 }", "None"));
    }

    #[test]
    fn an_edit_is_placed_by_utf16_units_not_bytes() {
        let text = "a → Held\nHeld";
        let edits = [
            json!({ "range": { "start": { "line": 0, "character": 4 }, "end": { "line": 0, "character": 8 } }, "newText": "Stored" }),
            json!({ "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 4 } }, "newText": "Stored" }),
        ];

        assert_eq!(edited(text, &edits), "a → Stored\nStored");
    }
}
