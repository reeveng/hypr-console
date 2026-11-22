use std::process::{Child, Command, ExitStatus, Output};

const WHERE_PROGRAMS_ARE: &str = "/usr/bin";

fn inherited(program: &str) -> std::io::Result<ExitStatus> {
    //~v WARN EXPLICIT093_NO_INHERITED_ENVIRONMENT
    Command::new(program).arg("--version").status()
}

fn inherited_and_kept(program: &str) -> std::io::Result<Child> {
    let mut starting = Command::new(program);

    //~v WARN EXPLICIT093_NO_INHERITED_ENVIRONMENT
    starting.spawn()
}

fn added_to(program: &str) -> std::io::Result<Output> {
    //~v WARN EXPLICIT093_NO_INHERITED_ENVIRONMENT
    Command::new(program).env("LANG", "C").output()
}

fn told(program: &str) -> std::io::Result<ExitStatus> {
    Command::new(program).env_clear().env("PATH", WHERE_PROGRAMS_ARE).status()
}

fn told_first(program: &str) -> std::io::Result<Child> {
    let mut starting = Command::new(program);
    let _ = starting.env_clear();
    let _ = starting.env("PATH", WHERE_PROGRAMS_ARE);

    starting.spawn()
}

fn main() {
    let _ = inherited("true");
    let _ = inherited_and_kept("true");
    let _ = added_to("true");
    let _ = told("true");
    let _ = told_first("true");
}
