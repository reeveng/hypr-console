//! Bring this machine to match /etc/console/desktop.conf.
//!
//!     console list      what the desktop is made of
//!     console check     where the machine has drifted from it
//!     console apply     bring the machine back to it
//!     console save      take a file edited in place back into the source
//!
//! The manifest is the source of truth and this is only the engine that reads
//! it. Anything installed or enabled outside it is invisible here, which is the
//! point: a desktop assembled by hand is one nobody can put back together.

mod build;
mod buttons;
mod install;
mod laying;
mod machine;
mod manifest;
mod packages;
mod units;

use std::path::{Path, PathBuf};
use std::process::ExitCode;


use laying::Deploy;
use manifest::{Manifest, Section};

const ROOT: &str = "/etc/console";

/// Named by number, so they are whatever this terminal's palette says. The dim
/// attribute is not among them: it halves whatever colour it is applied to, and
/// half of a colour chosen to clear 7:1 is a colour that does not.
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const OFF: &str = "\x1b[0m";

/// The column every state is written in, so that the entries beside them line
/// up whatever the state says.
const COLUMN: usize = 18;

fn main() -> ExitCode {
    let asked: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = asked
        .split_first()
        .map_or(("check", &[][..]), |(one, rest)| (one.as_str(), rest));

    // A tree to read instead of /etc/console, for looking at a manifest that is
    // not the one this machine is wearing. Reading only: `apply` and `save`
    // write, and a flag that could point writing somewhere else is a flag that
    // will one day point writing somewhere else.
    let (root, rest) = match (command, rest.split_first()) {
        ("list" | "check", Some((flag, [at]))) if flag == "--root" => (PathBuf::from(at), &[][..]),
        _ => (PathBuf::from(ROOT), rest),
    };

    let manifest = match read(&root) {
        Ok(manifest) => manifest,
        Err(fault) => {
            eprintln!("{fault}");
            return ExitCode::FAILURE;
        }
    };

    match command {
        "list" => list(&manifest),
        "check" => return check(&root, &manifest),
        "apply" => return report(apply(&root, &manifest)),
        "buttons" => return report(rebuttoned(&root, &manifest)),
        "save" => return report(save(&root, &manifest, rest)),
        _ => {
            println!("{}", HELP);
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

const HELP: &str = "\
console list      what the desktop is made of
console check     where the machine has drifted from it
console apply     bring the machine back to it
console buttons   write the profiles again, with this device's buttons in them
console save      take a file edited in place back into the source";

fn read(root: &Path) -> Result<Manifest, String> {
    let at = root.join("desktop.conf");
    let held = std::fs::read_to_string(&at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
    Manifest::read(&held)
}

fn report(done: Result<(), String>) -> ExitCode {
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{RED}{fault}{OFF}");
            ExitCode::FAILURE
        }
    }
}

/// One line of a report: a state in its own colour, then what it is about.
fn line(colour: &str, state: &str, about: &str) {
    let pad = " ".repeat(COLUMN.saturating_sub(state.chars().count()));
    println!("  {colour}{state}{OFF}{pad}  {about}");
}

fn settled(ok: bool) -> &'static str {
    match ok {
        true => GREEN,
        false => RED,
    }
}

// ------------------------------------------------------------------- list

fn list(manifest: &Manifest) {
    for (section, entries) in manifest.sections() {
        println!("{YELLOW}[{}]{OFF}", section.name());
        entries.iter().for_each(|entry| println!("  {entry}"));
        println!();
    }
}

// ------------------------------------------------------------------ check

/// Report every difference between the manifest and the machine.
///
/// Every section is counted where it is printed. A lazily counted one prints
/// its heading and none of its rows until something asks for the number, and
/// then the whole report arrives in the wrong order.
fn check(root: &Path, manifest: &Manifest) -> ExitCode {
    let source = root.join("files");
    let have = machine::installed_packages();
    let asked_for = machine::wanted_packages();

    let drift = [
        under("packages", manifest.of(Section::Packages), |package| {
            let held = packages::held(&have, &asked_for, package);
            (held.settled(), held.name().into(), package.clone())
        }),
        under("built", manifest.of(Section::Build), |name| {
            let state = build::state(root, name);
            (state.settled(), state.name().into(), build::live(name))
        }),
        under("files", manifest.of(Section::Files), |path| {
            let state = install::state(&source, path, machine::whoever());
            (state.settled(), state.name().into(), path.clone())
        }),
        under("services", manifest.of(Section::Services), |unit| {
            let (enabled, active) = machine::unit_state(unit);
            let ok = enabled == "enabled" && active == "active";
            (ok, format!("{enabled}, {active}"), unit.clone())
        }),
        under("masked", manifest.of(Section::Masked), |unit| {
            let enabled = machine::unit_state(unit).0;
            let ok = enabled == "masked";
            let said = match enabled.is_empty() {
                true => "not masked".to_string(),
                false => enabled,
            };
            (ok, said, unit.clone())
        }),
    ]
    .iter()
    .sum::<usize>();

    front(&buttons::standing(root, &home()));

    match drift {
        0 => {
            println!("{GREEN}The machine matches the manifest.{OFF}");
            ExitCode::SUCCESS
        }
        drift => {
            println!("{RED}{drift} differences.{OFF} `console apply` settles them.");
            ExitCode::FAILURE
        }
    }
}

/// The front of the machine, which is not drift and is never counted as it.
///
/// Every other section here is a thing an apply settles. A device that has no
/// right paddle is not going to grow one, so this section is printed and left
/// out of the number: a report that ended "3 differences, `console apply`
/// settles them" while one of the three was a button that does not exist would
/// be the engine promising something it cannot do.
fn front(standing: &buttons::Standing) {
    println!("{YELLOW}buttons{OFF}");
    match (standing.asked, standing.settled()) {
        // Not knowing is its own answer, and it is the usual one for a minute
        // after a boot: InputPlumber waits for udev to finish before it takes
        // the controller, and a check run in that minute has asked nothing.
        (false, _) => line(YELLOW, "not asked", "InputPlumber did not say what this device sends"),
        (true, true) => line(GREEN, "all here", "every button this desktop binds"),
        (true, false) => {
            standing.missing.iter().for_each(|lost| line(RED, "not here", lost));
        }
    }
    // Said even when nothing is missing, because when nothing is missing
    // because somebody moved it, that is the reason and it should be readable.
    if standing.moved > 0 {
        let many = standing.moved;
        line(GREEN, "moved", &format!("{many} of them are elsewhere on this device"));
    }
    // The other half of the contract. Every button on the front of this
    // machine has an answer for a hand holding nothing, and on a device with
    // no screen to touch those answers are all unreachable at once.
    if standing.touchscreen == Some(false) {
        line(YELLOW, "no touchscreen", "nothing here can be driven by a finger");
    }
    println!();
}

/// One section of the report, and how many of its entries have drifted.
fn under<T>(name: &str, entries: &[T], state: impl Fn(&T) -> (bool, String, String)) -> usize {
    println!("{YELLOW}{name}{OFF}");
    let drift = entries
        .iter()
        .map(state)
        .filter(|(ok, said, about)| {
            line(settled(*ok), said, about);
            !ok
        })
        .count();
    println!();
    drift
}

// ------------------------------------------------------------------ apply

/// The notice on the screen for as long as the apply lasts.
///
/// An apply rewrites files, restarts services and compiles every program the
/// manifest names, and for the minute that takes the screen said nothing at
/// all. So the answer to "is the thing I am about to press the new one?" was
/// to remember how long ago the deploy went, and a fault reported against a
/// copy that had already been replaced costs an evening at both ends.
///
/// A guard rather than two calls, because `apply` leaves by half a dozen
/// question marks and every one of them has to take the notice down. Missing
/// one leaves a machine sitting under "Updating the console" until somebody
/// reboots it, which is a worse lie than saying nothing was.
struct Updating {
    finished: bool,
}

impl Updating {
    fn started() -> Self {
        machine::in_the_session("console-updating start");
        Updating { finished: false }
    }

    fn done(mut self) {
        self.finished = true;
        machine::in_the_session("console-updating done");
    }
}

impl Drop for Updating {
    fn drop(&mut self) {
        if !self.finished {
            machine::in_the_session("console-updating failed");
        }
    }
}


fn apply(root: &Path, manifest: &Manifest) -> Result<(), String> {
    if !nix_is_root() {
        return Err("console apply has to run as root.".into());
    }
    let source = root.join("files");
    // After the root check and before anything is touched, so that what is up
    // on the screen and what is true of the machine begin together.
    let saying = Updating::started();

    let named = manifest.of(Section::Packages);
    let have = machine::installed_packages();
    let asked_for = machine::wanted_packages();

    let missing = packages::missing(named, &have);
    if !missing.is_empty() {
        println!("{YELLOW}installing{OFF} {}", missing.join(" "));
        let argv: Vec<&str> = ["pacman", "-S", "--needed", "--noconfirm"]
            .into_iter()
            .chain(missing)
            .collect();
        if !machine::run_seen(&argv) {
            return Err("pacman could not install what the manifest asks for.".into());
        }
    }

    // Named here and on the machine on somebody else's word. Installing it
    // again would do nothing, because it is already there; what is missing is
    // pacman knowing that this desktop wants it, without which it is swept with
    // the orphans the day the package that brought it in leaves.
    let borrowed = packages::borrowed(named, &have, &asked_for);
    if !borrowed.is_empty() {
        println!("{YELLOW}keeping{OFF} {}", borrowed.join(" "));
        let argv: Vec<&str> = ["pacman", "-D", "--asexplicit", "--quiet"]
            .into_iter()
            .chain(borrowed)
            .collect();
        if !machine::run_seen(&argv) {
            return Err("pacman would not be told the desktop asks for these.".into());
        }
    }

    // Staged, all of it, and still nothing changed on the machine. A build
    // that fails, a file the machine will not take -- either of those and the
    // desktop that was running a moment ago is the desktop still running.
    // Anything left beside a live file by a run that did not finish -- a
    // machine turned off mid-apply, a power cut. A kept copy holds the inode of
    // a program nothing runs any more, and a staged one is a release nobody
    // decided to have.
    swept(manifest);

    let mut here = machine::Here;
    let mut deploy = Deploy::default();
    let staged = compile(root, manifest, &mut deploy, &mut here)
        .and_then(|built| Ok((built, write(&source, manifest, &mut deploy, &mut here)?)));
    let written = match staged {
        Ok((built, files)) => built.into_iter().chain(files).collect::<Vec<String>>(),
        Err(fault) => {
            deploy.abandon(&mut here);
            return Err(fault);
        }
    };

    // The moment. Everything fallible is behind us and what is left is renames.
    deploy.swap(&mut here)?;

    packed_the_add_on();
    told_the_browsers();

    // Both profiles that are made rather than kept: the one this desktop is
    // driven by, and the one the setup screen asks its question with. Out of
    // what this device says it can send, so that what InputPlumber is given
    // names the buttons this hardware actually has and no others.
    for live in [buttons::wrote_router(), buttons::wrote_asking()].into_iter().flatten() {
        println!("{YELLOW}writing{OFF} {live}");
    }

    if written.iter().any(|path| path.contains("/systemd/")) {
        machine::user_systemctl(&["daemon-reload"]);
    }

    // The profile was just written again, and InputPlumber is still holding
    // the text it had a moment ago: it reads the file when it is asked to load
    // one, and its name has not changed, so nothing watching which profile is
    // on has anything to notice. Without this, a device whose buttons have
    // changed goes on wearing the old routing until something else swaps the
    // profile.
    buttons::wear_again();

    let mut asked_to_run: Vec<&String> = Vec::new();
    for unit in manifest.of(Section::Services) {
        let (enabled, active) = machine::unit_state(unit);
        if enabled != "enabled" {
            println!("{YELLOW}enabling{OFF} {unit}");
            machine::user_systemctl(&["enable", unit]);
        }
        match active.as_str() {
            "active" if restarted_by(&source, unit, &written) => {
                println!("{YELLOW}restarting{OFF} {unit}");
                machine::user_systemctl(&["restart", unit]);
                asked_to_run.push(unit);
            }
            "active" => {}
            _ => {
                println!("{YELLOW}starting{OFF} {unit}");
                machine::user_systemctl(&["start", unit]);
                asked_to_run.push(unit);
            }
        }
    }

    // The release has to stand up before it is kept. Until here the old files
    // are still on the machine under a second name, and this is the last point
    // at which putting them back is a rename rather than a deploy.
    let fell = fallen(&asked_to_run);
    if !fell.is_empty() {
        println!("\n{RED}did not come up{OFF} {}", fell.join(" "));
        for one in deploy.undo(&mut here) {
            match one.fault {
                None => println!("{YELLOW}put back{OFF} {}", one.at),
                Some(fault) => println!("{RED}{fault}{OFF}"),
            }
        }
        if written.iter().any(|path| path.contains("/systemd/")) {
            machine::user_systemctl(&["daemon-reload"]);
        }
        for unit in &asked_to_run {
            println!("{YELLOW}restarting{OFF} {unit}");
            machine::user_systemctl(&["restart", unit]);
        }
        return Err(format!(
            "put back: {} would not run what this was about to install.",
            fell.join(", ")
        ));
    }
    deploy.settle(&mut here);

    for unit in manifest.of(Section::Masked) {
        if machine::unit_state(unit).0 != "masked" {
            println!("{YELLOW}masking{OFF} {unit}");
            machine::user_systemctl(&["mask", unit]);
        }
    }

    for wake in units::woken_by(&written) {
        println!("{YELLOW}reloading{OFF} {}", wake.name);
        // Seen rather than swallowed. `machine::run` keeps what a command said
        // and throws away how it exited, which is right where failing is an
        // answer and wrong here: a wake that did not run leaves the machine
        // looking applied and behaving as it did before, which is the fault
        // the whole table exists to prevent.
        if !machine::run_seen(&["su", machine::whoever(), "-c", wake.run]) {
            println!("{RED}did not{OFF} {}: {}", wake.name, wake.run);
        }
    }

    // Said rather than committed. An apply writes nothing into this tree --
    // everything it writes is on the machine -- so a commit here could only
    // ever be somebody else's open work, taken under the name "apply". What is
    // worth knowing is the other half of it: a tree with changes in it is a
    // machine that now matches no commit, and walking it back means finding
    // out what those changes were.
    for open in machine::uncommitted(root) {
        println!("{YELLOW}not committed{OFF} {open}");
    }
    saying.done();
    told_the_front(root);
    println!("\n{GREEN}Done.{OFF}");
    Ok(())
}

/// Pack this desktop's own add-on for the browser, out of the crate that holds
/// it, and leave it where the policy written a moment later says it will be.
///
/// Before that policy rather than after it. The policy names a file, and a
/// browser told to install one that is not there has been told to install
/// nothing at all -- and would not be told again until the next apply.
///
/// It does nothing on a machine where neither the add-on nor the palette has
/// changed, which is nearly every apply: a browser takes an add-on again when
/// its version goes up, and a version raised for nothing is a browser
/// reinstalling something nobody has touched every time this is run.
///
/// As her, and not as root with her home named. This is the one thing here that
/// writes inside the browser's profile, and the directory it writes into is the
/// same one the browser puts its own add-ons in. Run as root it makes that
/// directory root's, and then the browser -- which is her -- cannot write to it:
/// uBlock, Bitwarden and Dark Reader are all fetched into it by policy and all
/// three fail, silently, while the one already sitting there goes on working. It
/// looked exactly like the policy having stopped working, and it was a `chown`.
fn packed_the_add_on() {
    println!("{YELLOW}packing{OFF} the browser's own add-on");
    let whom = machine::whoever();
    let home = format!("HOME=/home/{whom}");
    machine::run_seen(&["runuser", "-u", whom, "--", "env", &home, "console-web"]);
}

/// Say to the browsers what this desktop has decided: which engine a question
/// is asked of, and which add-ons it puts in front of her.
///
/// Written by console-engine, which is where it belongs, and run here because
/// nothing else ever ran it on a machine where nobody had chosen an engine yet.
/// A browser's policy lives under /etc and this is the part of the day that is
/// root; her own choice is read out of her home rather than root's, which is
/// what HOME says.
fn told_the_browsers() {
    println!("{YELLOW}telling{OFF} the browsers");
    let home = format!("HOME=/home/{}", machine::whoever());
    machine::run_seen(&["env", &home, "console-engine"]);
}

/// Say which of the buttons this desktop binds are not on this device.
///
/// After the apply rather than before it, and after the notice saying the
/// apply is happening has been taken down, because two notices arguing over
/// the same corner of the screen is one notice nobody reads. Never a failure:
/// an install on a device missing a paddle is an install that worked, and what
/// is left is a thing to move rather than a thing to fix.
///
/// The setup screen is opened here only on a device nobody has answered for
/// yet. Somebody who walked through it and left every button where it was has
/// answered, and a machine that put the same screen up after every apply would
/// be a machine that had not listened.
fn told_the_front(root: &Path) {
    let standing = buttons::standing(root, &home());
    if !standing.asked || standing.settled() {
        return;
    }
    println!("{YELLOW}saying{OFF} {}", standing.summary());
    machine::in_the_session(&said(&["console-say", "buttons", &standing.summary(), &standing.body()]));
    if !standing.told {
        println!("{YELLOW}asking{OFF} which buttons this device has");
        machine::in_the_session("layout-panel --first");
    }
}

/// A command line for a shell, with every word held together however it is
/// written.
///
/// What goes into a notice is a sentence with the names of buttons in it, and
/// `in_the_session` hands what it is given to `sh`. A word with a space in it
/// is two words there, and a sentence with an apostrophe in it ends the
/// quoting halfway through and takes the rest of the line with it.
fn said(argv: &[&str]) -> String {
    argv.iter()
        .map(|word| format!("'{}'", word.replace('\'', "'\\''")))
        .collect::<Vec<String>>()
        .join(" ")
}

/// The home of whoever this desktop belongs to.
fn home() -> String {
    format!("/home/{}", machine::whoever())
}

/// Whether writing these files means this unit is now running the wrong thing.
fn restarted_by(source: &Path, unit: &str, written: &[String]) -> bool {
    let its_own = format!("/etc/systemd/user/{unit}");
    if written.contains(&its_own) {
        return true;
    }
    let held = std::fs::read_to_string(install::source_of(source, &its_own)).unwrap_or_default();
    units::named_by(&held)
        .iter()
        .any(|named| written.contains(named))
}

/// Which of the units asked to run are not running.
///
/// `failed` and nothing else. A unit that has finished its work and gone is
/// `inactive`, and most of what this desktop starts at login is exactly that,
/// so reading anything short of active as a fault would put back every release
/// on a machine where one-shot succeeded. Failed is systemd's own word for a
/// unit that tried and could not, which is the only thing worth undoing a
/// release for.
///
/// What this does not catch is a unit that comes up and falls over later.
/// `systemctl restart` waits for the job, so a service that dies during start
/// is failed by the time this asks; a service that dies a minute in is
/// somebody watching the machine, and no amount of looking here would have
/// seen it.
fn fallen(units: &[&String]) -> Vec<String> {
    units
        .iter()
        .filter(|unit| machine::unit_state(unit).1 == "failed")
        .map(|unit| unit.to_string())
        .collect()
}

/// Sweep what a run that did not finish left behind.
///
/// Beside every file the manifest claims, because that is where this engine
/// puts things and asking by name needs no directory walked. A leftover beside
/// a file the manifest has stopped claiming stays, which is the same hole as
/// the file itself staying, and is written down in `todos.md` rather than
/// half-answered here.
fn swept(manifest: &Manifest) {
    let claimed = manifest
        .of(Section::Files)
        .iter()
        .cloned()
        .chain(manifest.of(Section::Build).iter().map(|name| build::live(name)));
    for live in claimed {
        machine::drop_staged(&live);
        machine::drop_kept(&live);
    }
}

/// Compile what the device makes for itself, and stage it.
///
/// Staged rather than installed. Nothing this returns is on the machine yet;
/// see `Deploy`.
fn compile(
    root: &Path,
    manifest: &Manifest,
    deploy: &mut Deploy,
    here: &mut machine::Here,
) -> Result<Vec<String>, String> {
    let names = manifest.of(Section::Build);
    if names.is_empty() {
        return Ok(Vec::new());
    }
    println!("{YELLOW}building{OFF} {}", names.join(" "));
    let how = build::how(names);
    let argv: Vec<&str> = ["cargo"]
        .into_iter()
        .chain(how.iter().map(String::as_str))
        .collect();
    let built = std::process::Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(root)
        .status()
        .map_err(|fault| format!("cargo could not be run: {fault}"))?;
    if !built.success() {
        return Err("cargo could not build what the manifest asks for.".into());
    }

    names
        .iter()
        .filter(|name| !build::state(root, name).settled())
        .map(|name| {
            let live = build::live(name);
            println!("{YELLOW}staging{OFF} {live}");
            deploy.stage(here, &build::made(root, name), &live).map(|()| live)
        })
        .collect()
}

/// Stage every file that is not already what the source says.
///
/// A file with no source is said and passed over, as it always was: the
/// manifest naming something the tree does not hold is a fault to fix in the
/// tree, and it is not made better by refusing to lay down the other ninety.
///
/// A file that will not stage is different and stops everything. It is the
/// machine refusing, not the tree being wrong, and half a release is the state
/// this whole arrangement exists to make impossible.
fn write(
    source: &Path,
    manifest: &Manifest,
    deploy: &mut Deploy,
    here: &mut machine::Here,
) -> Result<Vec<String>, String> {
    let mut staged = Vec::new();
    for path in manifest.of(Section::Files) {
        match install::state(source, path, machine::whoever()) {
            install::State::Ok => {}
            install::State::Unsourced => println!("{RED}no source for{OFF} {path}"),
            _ => {
                println!("{YELLOW}staging{OFF} {path}");
                deploy.stage(here, &install::source_of(source, path), path)?;
                staged.push(path.clone());
            }
        }
    }
    Ok(staged)
}

// ---------------------------------------------------------------- buttons

/// Write the profiles again, with this device's own buttons in them.
///
/// What the setup screen calls when somebody has moved one. The rendering is
/// the same rendering an apply does -- one function, in `console_pad::layout`
/// -- and this is only the four files it applies to, without the packages, the
/// compiling and the minute those take. A person who has just pressed a button
/// should see it take effect, and an apply is not a thing to run at somebody
/// waiting.
///
/// It is the second thing on this device somebody may run as root. The screen
/// that writes the table runs as them, `/etc` is not theirs, and this takes no
/// argument at all: what it writes is decided by the table in their own home
/// and by the tree, and there is nothing to hand it that would make it write
/// anything else.
fn rebuttoned(_root: &Path, _manifest: &Manifest) -> Result<(), String> {
    if !nix_is_root() {
        return Err("console buttons has to run as root.".into());
    }
    // Both profiles that are made rather than kept, written again out of what
    // this device says it can send.
    //
    // There used to be four of them in the tree, and this command rewrote them
    // through a table of moved buttons -- staged and swapped together, because
    // four files holding one answer written four times is a set that must not
    // be half laid down. There is one profile now, it says nothing about what a
    // button means, and moving a job does not touch `/etc` at all: what a press
    // comes to is read by the daemon out of a file in the owner's own home.
    // What is left here is the one thing that does change under `/etc`, and
    // only when the device itself has changed -- a different handheld, or a
    // controller that has grown a button.
    let mut written = 0;
    for live in [buttons::wrote_router(), buttons::wrote_asking()].into_iter().flatten() {
        println!("{YELLOW}writing{OFF} {live}");
        written += 1;
    }
    match written {
        0 => println!("This machine would not say what buttons it has."),
        // The profile on the pad is one of the files just written, and
        // InputPlumber is holding what that file said a moment ago. Asked for
        // again by its path: the name has not changed, so the daemon watching
        // which profile is on has nothing to notice.
        _ => buttons::wear_again(),
    }
    Ok(())
}

// ------------------------------------------------------------------- save

/// Take a file that was edited in place back into the source tree.
///
/// Editing the live file is the natural thing to do while chasing a fault. The
/// next apply would put it back, so this is how that edit is kept.
fn save(root: &Path, manifest: &Manifest, asked: &[String]) -> Result<(), String> {
    let source = root.join("files");
    let wanted: Vec<String> = match asked {
        [] => manifest
            .of(Section::Files)
            .iter()
            .filter(|path| {
                install::state(&source, path, machine::whoever()) == install::State::Differs
            })
            .cloned()
            .collect(),
        asked => asked.to_vec(),
    };
    if wanted.is_empty() {
        println!("Nothing differs from the source.");
        return Ok(());
    }
    let mut taken: Vec<PathBuf> = Vec::new();
    for path in &wanted {
        // Said either way round: the manifest's paths carry the mark, and a
        // person chasing a fault names the file they were just editing, which
        // is the one with their own name in it.
        let declared = install::as_declared(path, machine::whoever());
        let on = install::on_machine(&declared, machine::whoever());
        if !Path::new(&on).exists() {
            println!("{RED}not on the machine{OFF} {path}");
            continue;
        }
        let into = install::source_of(&source, &declared);
        if let Some(holding) = into.parent() {
            std::fs::create_dir_all(holding)
                .map_err(|fault| format!("{}: {fault}", holding.display()))?;
        }
        let held = std::fs::read(&on).map_err(|fault| format!("{path}: {fault}"))?;
        std::fs::write(
            &into,
            install::content_as_declared(&held, machine::whoever()),
        )
        .map_err(|fault| format!("{path}: {fault}"))?;
        println!("{YELLOW}saved{OFF} {path}");
        taken.push(into);
    }
    machine::commit(root, "save", &taken);
    Ok(())
}

fn nix_is_root() -> bool {
    machine::run(&["id", "-u"]).out == "0"
}
