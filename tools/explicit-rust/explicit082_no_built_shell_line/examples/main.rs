use std::process::Command;

const SCRIPT: &str = "command -v \"$1\"";

fn assembled(named: &str) -> Vec<String> {
    let line = format!("command -v {named}");

    //~v WARN EXPLICIT082_NO_BUILT_SHELL_LINE
    vec!["sh".to_string(), "-c".to_string(), line.to_string()]
}

fn lent(owner: &str, line: &str) -> usize {
    //~v WARN EXPLICIT082_NO_BUILT_SHELL_LINE
    let arguments = ["su", owner, "-s", "/bin/sh", "-c", &line];

    arguments.len()
}

fn chained(named: &str) -> Command {
    let mut asking = Command::new(named);

    //~v WARN EXPLICIT082_NO_BUILT_SHELL_LINE
    asking.arg("-c").arg(format!("command -v {named}"));

    asking
}

fn written(named: &str) -> usize {
    let arguments = ["sh", "-c", "command -v \"$1\"", "sh", named];

    arguments.len()
}

fn named_once(named: &str) -> usize {
    let arguments = ["sh", "-c", SCRIPT, "sh", named];

    arguments.len()
}

fn another_flag(named: &str) -> usize {
    let arguments = ["tar", "-C", named];

    arguments.len()
}

fn main() {
    let _ = assembled("ls");
    let _ = lent("somebody", "ls");
    let _ = chained("sh");
    let _ = written("ls");
    let _ = named_once("ls");
    let _ = another_flag("/");
}
