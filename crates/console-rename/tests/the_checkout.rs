use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use console_core_atomic_writes::whole;
use console_core_external_programs::Program;

type Failure = Box<dyn Error>;

const RENAME: &str = env!("CARGO_BIN_EXE_console-rename");

fn tree(named: &str) -> Result<PathBuf, Failure> {
    let at = console_core_temporary_directories::fresh(named)?;

    whole(&at.join("desktop.conf"), b"[packages]\n")?;
    whole(&at.join("notes.txt"), b"the console-lamp is on\n")?;

    Ok(at)
}

fn renaming(at: &Path) -> Result<Output, Failure> {
    let above = at.parent().ok_or("a directory above")?;
    let done = Command::new(RENAME)
        .args(["console-lamp", "console-light"])
        .current_dir(at)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env("GIT_CEILING_DIRECTORIES", above)
        .output()?;

    Ok(done)
}

#[test]
fn a_tree_git_will_not_list_is_a_failure_and_not_a_tree_with_nothing_in_it() -> Result<(), Failure> {
    let at = tree("rename-no-checkout")?;

    let done = renaming(&at)?;

    assert!(!done.status.success(), "the rename finished as though it had swept a tree git never listed");
    assert!(
        String::from_utf8_lossy(&done.stderr).contains("nothing was renamed"),
        "the rename did not say why it stopped: {}",
        String::from_utf8_lossy(&done.stderr)
    );

    let notes = std::fs::read_to_string(at.join("notes.txt"))?;

    assert_eq!(notes, "the console-lamp is on\n");

    Ok(())
}

#[test]
fn a_checkout_is_swept() -> Result<(), Failure> {
    let at = tree("rename-checkout")?;

    for asked in [vec!["init", "--quiet"], vec!["add", "desktop.conf", "notes.txt"]] {
        let Ok(mut git) = Program::Git.command();
        let done = git.arg("-C").arg(&at).args(asked).output()?;

        assert!(done.status.success(), "git: {}", String::from_utf8_lossy(&done.stderr));
    }

    let done = renaming(&at)?;

    assert!(done.status.success(), "the rename failed: {}", String::from_utf8_lossy(&done.stderr));

    let notes = std::fs::read_to_string(at.join("notes.txt"))?;

    assert_eq!(notes, "the console-light is on\n");

    Ok(())
}
