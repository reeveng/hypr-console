use std::path::Path;
use std::process::{Command, Output};

use console_core_external_programs::Program;

const PUBLISHER: &str = env!("CARGO_BIN_EXE_console-manifest-publish");
const EMAIL: &str = "release-fixture@example.invalid";

#[test]
fn an_offline_export_accepts_clean_files_and_refuses_the_local_git_email()
-> Result<(), Box<dyn std::error::Error>> {
  let empty = console_core_temporary_directories::fresh("publish-local-privacy");
  let fixture = empty?;
  let source = fixture.join("source");
  let export = fixture.join("export");
  let note = source.join("docs/note.txt");
  let docs = std::fs::create_dir_all(source.join("docs"));

  docs?;

  let manifest = console_core_atomic_writes::whole(&source.join("desktop.conf"), b"[desktop]\n");
  manifest?;

  let clean = console_core_atomic_writes::whole(&note, b"A release note.\n");
  clean?;

  let initialized = git(&source, &["init", "--quiet"]);
  initialized?;

  let configured = git(&source, &["config", "user.email", EMAIL]);
  configured?;

  let tracked = git(&source, &["add", "desktop.conf", "docs/note.txt"]);
  tracked?;

  let offline = publish(&source, &export);
  let passed = offline?;
  assert!(passed.status.success(), "{passed:?}");

  let text = format!("Personal contact {EMAIL}.\n");
  let identifiable = console_core_atomic_writes::whole(&note, text.as_bytes());
  identifiable?;

  let repeated = publish(&source, &export);
  let refused = repeated?;
  let returned = String::from_utf8(refused.stderr);
  let diagnostics = returned?;

  assert!(!refused.status.success());
  assert!(diagnostics.contains("docs/note.txt contains the configured Git email"));
  assert!(!diagnostics.contains(EMAIL));

  let removed = std::fs::remove_dir_all(fixture);
  removed?;
  Ok(())
}

fn git(source: &Path, arguments: &[&str]) -> std::io::Result<()> {
  let Ok(mut command) = Program::Git.command();
  let run = command.current_dir(source).args(arguments).status();
  let result = run?;

  assert!(result.success(), "git {arguments:?} returned {result}");
  Ok(())
}

fn publish(source: &Path, export: &Path) -> std::io::Result<Output> {
  let output = Command::new(PUBLISHER)
    .current_dir(source)
    .env_remove("CONSOLE_HOST")
    .arg("--export-only")
    .arg(export)
    .output();

  output
}
