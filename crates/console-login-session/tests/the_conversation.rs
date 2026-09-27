use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use console_login_session::{Request, LoginError, Rules, Stage, authenticate, trusted};

const SERVICE: &str = "console-login-test";

fn rules(name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let check = folder.join("check");

    fs::create_dir_all(&folder)?;

    let script = "#!/bin/sh\nIFS= read -r secret\n[ \"$secret\" = tat ]\n";
    let stack = format!(
        "auth required pam_exec.so type=auth expose_authtok quiet {}\n\
         auth optional pam_permit.so\n\
         account required pam_permit.so\n\
         session required pam_permit.so\n",
        check.display()
    );

    console_core_atomic_writes::whole(&check, script.as_bytes())?;
    fs::set_permissions(&check, fs::Permissions::from_mode(0o755))?;
    console_core_atomic_writes::whole(&folder.join(SERVICE), stack.as_bytes())?;

    Ok(folder)
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "PAM is asked about the account running this test, and USER is the only place a test harness is told whose that is"
    )
)]
fn person() -> Result<String, Box<dyn Error>> {
    let person = std::env::var("USER")?;

    Ok(person)
}

#[test]
fn the_pattern_is_what_a_module_is_handed_when_it_asks_for_a_password() -> Result<(), Box<dyn Error>> {
    let folder = rules("right")?;
    let person = person()?;
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };

    authenticate(request, "tat").map_err(|why| format!("the right pattern was refused: {why}"))?;

    Ok(())
}

#[test]
fn a_wrong_pattern_is_refused_before_anything_past_the_secret_is_asked() -> Result<(), Box<dyn Error>> {
    let folder = rules("wrong")?;
    let person = person()?;
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };

    match authenticate(request, "tab") {
        Ok(_) => Err(Box::from("a wrong pattern was let in")),
        Err(why) => {
            assert!(
                matches!(why, LoginError::WrongSecret | LoginError::PamFailure { stage: Stage::Authenticating, .. }),
                "{why:?}"
            );

            Ok(())
        }
    }
}

#[test]
fn a_session_opens_with_what_it_was_handed_and_closes_when_dropped() -> Result<(), Box<dyn Error>> {
    let folder = rules("session")?;
    let person = person()?;
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };
    let transaction = authenticate(request, "tat").map_err(|why| format!("the right pattern was refused: {why}"))?;
    let session = transaction
        .open_session("tty7", &[("XDG_SESSION_CLASS", "user"), ("XDG_VTNR", "1")])
        .map_err(|why| format!("the session would not open: {why}"))?;
    let Ok(environment) = session.environment();

    assert!(environment.contains(&"XDG_SESSION_CLASS=user".to_string()), "{environment:?}");
    assert!(environment.contains(&"XDG_VTNR=1".to_string()), "{environment:?}");

    Ok(())
}

#[test]
fn a_trusted_login_is_never_asked_for_a_secret() -> Result<(), Box<dyn Error>> {
    let folder = rules("trusted")?;
    let person = person()?;
    let request = Request { service: SERVICE, person: &person, rules: Rules::Directory(&folder) };

    trusted(request).map_err(|why| format!("a login nobody types was refused: {why}"))?;

    Ok(())
}
