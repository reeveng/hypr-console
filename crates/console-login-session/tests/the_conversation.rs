use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use console_login_session::{Request, LoginError, Rules, Stage, authenticated, trusted};

const SERVICE: &str = "console-login-test";

fn rules(name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let check = folder.join("check");

    match fs::create_dir_all(&folder) {
        Ok(()) => {}
        Err(why) => panic!("no folder for the rules: {why}"),
    }

    let script = "#!/bin/sh\nIFS= read -r secret\n[ \"$secret\" = tat ]\n";
    let stack = format!(
        "auth required pam_exec.so type=auth expose_authtok quiet {}\n\
         auth optional pam_permit.so\n\
         account required pam_permit.so\n\
         session required pam_permit.so\n",
        check.display()
    );

    let wrote = fs::write(&check, script)
        .and_then(|()| fs::set_permissions(&check, fs::Permissions::from_mode(0o755)))
        .and_then(|()| fs::write(folder.join(SERVICE), stack));

    match wrote {
        Ok(()) => folder,
        Err(why) => panic!("the rules would not write: {why}"),
    }
}

fn person() -> String {
    match std::env::var("USER") {
        Ok(person) => person,
        Err(why) => panic!("no USER to ask about: {why}"),
    }
}

#[test]
fn the_pattern_is_what_a_module_is_handed_when_it_asks_for_a_password() {
    let folder = rules("right");
    let person = person();
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };

    match authenticated(request, "tat") {
        Ok(_) => {}
        Err(why) => panic!("the right pattern was refused: {why}"),
    }
}

#[test]
fn a_wrong_pattern_is_refused_before_anything_past_the_secret_is_asked() {
    let folder = rules("wrong");
    let person = person();
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };

    match authenticated(request, "tab") {
        Ok(_) => panic!("a wrong pattern was let in"),
        Err(why) => assert!(
            matches!(why, LoginError::WrongSecret | LoginError::PamFailure { stage: Stage::Authenticating, .. }),
            "{why:?}"
        ),
    }
}

#[test]
fn a_session_opens_with_what_it_was_handed_and_closes_when_dropped() {
    let folder = rules("session");
    let person = person();
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };
    let transaction = match authenticated(request, "tat") {
        Ok(transaction) => transaction,
        Err(why) => panic!("the right pattern was refused: {why}"),
    };
    let session = match transaction.opened("tty7", &[("XDG_SESSION_CLASS", "user"), ("XDG_VTNR", "1")]) {
        Ok(session) => session,
        Err(why) => panic!("the session would not open: {why}"),
    };
    let Ok(environment) = session.environment();

    assert!(environment.contains(&"XDG_SESSION_CLASS=user".to_string()), "{environment:?}");
    assert!(environment.contains(&"XDG_VTNR=1".to_string()), "{environment:?}");
}

#[test]
fn a_trusted_login_is_never_asked_for_a_secret() {
    let folder = rules("trusted");
    let person = person();
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };

    match trusted(request) {
        Ok(_) => {}
        Err(why) => panic!("a login nobody types was refused: {why}"),
    }
}
