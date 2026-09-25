//! What runs on the device is what the map says runs.
//!
//! `docs/architecture/map.dot` is drawn from the tree: the units the manifest
//! enables, what each one starts, and what the compiler saw each program do.
//! This asks the device the same questions from the other end -- which units
//! are running, what is holding a connection to the pool, and whose child each
//! of those is -- and says what is in one and not the other. The fault it is
//! for is the one the pool was written to end: twenty-five subscriptions were
//! once found alive on this device, the oldest four hours old, and every one
//! of them was a process nobody's map said should be there.
//!
//! Four differences, each a sentence the run prints:
//!
//! - a unit running that the tree has no file for,
//! - a unit the manifest enables that is not running, unless its unit asks an
//!   `ExecCondition` first -- that one is allowed to decline, and says so,
//! - a program connected to the pool that the map says never subscribes,
//! - a subscriber whose parent is the user manager and which is not what any
//!   running unit starts -- a program whose starter went away and left it
//!   listening.
//!
//! It reads rather than presses: nothing is opened and nothing is put back.

use std::collections::{BTreeMap, BTreeSet};

use console_architecture::Architecture;
use console_architecture::facts::{self, Fact, Kind, LIBRARY, Target, Ownership};
use console_architecture::units::{SERVICE, SERVICE_SUFFIX, Started};
use console_core_external_programs::Program;
use console_core_ini_files::{Key, field};
use console_core_never::Never;
use console_test_stages::checking::{Body, Check, CheckResult, cannot, failed};
use console_test_stages::device::Device;

pub const MAPPED: Check = Check {
    name: "470-what-runs-is-what-the-map-says",
    about: "Every unit running is on the map, every unit it enables is running, and whatever holds a \
            connection to the pool is a program the map says subscribes.",
    feature: "architecture",
    since: "2026-09-24",
    bodies: &[Body::Device(mapped)],
};

const PREFIX: &str = "console";

const USER_MANAGER: &str = "systemd";

const DECLINES: Key<'static> = Key("ExecCondition");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Process {
    pub parent: u32,
    pub binary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Difference {
    Unmapped(String),
    NotRunning(String),
    Undeclared(String, u32),
    Orphaned(String, u32),
}

impl std::fmt::Display for Difference {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difference::Unmapped(unit) => write!(to, "{unit} is running and the tree has no unit file for it"),
            Difference::NotRunning(unit) => write!(to, "{unit} is enabled in desktop.conf and not running"),
            Difference::Undeclared(binary, pid) => {
                write!(to, "{binary} ({pid}) holds a connection to the pool and the map says it never subscribes")
            }
            Difference::Orphaned(binary, pid) => write!(
                to,
                "{binary} ({pid}) is subscribed, was left to the user manager, and is not what any running unit starts"
            ),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seen {
    pub units: BTreeSet<String>,
    pub processes: BTreeMap<u32, Process>,
    pub pool: BTreeSet<u32>,
}

fn base(path: &str) -> Result<String, Never> {
    Ok(match path.rsplit_once('/') {
        Some((_, name)) => String::from(name),
        None => String::from(path),
    })
}

pub const RUNNING: &str = "--user list-units --type=service --state=running --plain --no-legend --full";

pub fn units(said: &str) -> Result<BTreeSet<String>, Never> {
    Ok(said.lines().filter_map(|line| line.split_whitespace().next()).map(String::from).collect())
}

pub fn processes(said: &str) -> Result<BTreeMap<u32, Process>, Never> {
    let mut held = BTreeMap::new();

    for line in said.lines() {
        let mut words = line.split_whitespace();
        let pid = words.next().map(str::parse::<u32>);
        let parent = words.next().map(str::parse::<u32>);

        match (pid, parent, words.next()) {
            (Some(Ok(pid)), Some(Ok(parent)), Some(program)) => {
                let Ok(binary) = base(program);
                let _ = held.insert(pid, Process { parent, binary });
            }
            (Some(_) | None, Some(_) | None, Some(_) | None) => {}
        }
    }

    Ok(held)
}

fn pids(users: &str) -> Result<BTreeSet<u32>, Never> {
    let mut found = BTreeSet::new();

    for after in users.split("pid=").skip(1) {
        let number = after.split(|c: char| !c.is_ascii_digit()).next();

        match number.map(str::parse::<u32>) {
            Some(Ok(pid)) => {
                let _ = found.insert(pid);
            }
            None => {}
            Some(Err(_not_a_number)) => {}
        }
    }

    Ok(found)
}

pub fn pool(said: &str) -> Result<BTreeSet<u32>, Never> {
    let socket = console_events::place::SOCKET;
    let rows: Vec<Vec<&str>> = said.lines().map(|line| line.split_whitespace().collect()).collect();
    let mut peers = BTreeSet::new();
    let mut connected = BTreeSet::new();

    for row in &rows {
        match row.as_slice() {
            [_, _, _, _, local, _, _, peer, ..] => match local.ends_with(socket) && *peer != "0" {
                true => {
                    let _ = peers.insert(*peer);
                }
                false => {}
            },
            _ => {}
        }
    }

    for row in &rows {
        match row.as_slice() {
            [_, _, _, _, _, inode, _, _, users, ..] => match peers.contains(inode) {
                true => {
                    let Ok(found) = pids(users);

                    connected.extend(found);
                }
                false => {}
            },
            _ => {}
        }
    }

    Ok(connected)
}

fn subscribes(facts: &[Fact], target: &Target) -> Result<Subscribes, Never> {
    let Ok(reach) = facts::reaching(facts, target);
    let own = |fact: &&Fact| fact.whose(target).is_ok_and(|whose| whose == Ownership::Its);
    let linked = |fact: &&Fact| fact.target == LIBRARY && reach.contains(&fact.package);

    Ok(match facts.iter().filter(|fact| fact.kind == Kind::Subscribes).any(|fact| own(&fact) || linked(&fact)) {
        true => Subscribes::Yes,
        false => Subscribes::No,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subscribes {
    Yes,
    No,
}

pub fn subscribers(facts: &[Fact]) -> Result<BTreeSet<String>, Never> {
    let mut found = BTreeSet::new();

    for fact in facts.iter().filter(|fact| fact.kind == Kind::Builds && fact.target != LIBRARY) {
        let Ok(target) = fact.of();
        let Ok(said) = subscribes(facts, &target);

        match said {
            Subscribes::Yes => {
                let _ = found.insert(target.name);
            }
            Subscribes::No => {}
        }
    }

    Ok(found)
}

fn started_by_running(architecture: &Architecture, seen: &Seen) -> Result<BTreeSet<String>, Never> {
    let mut started = BTreeSet::new();

    for unit in architecture.units.iter().filter(|unit| seen.units.contains(&unit.name)) {
        let Ok(program) = unit.started();

        match program {
            Some(Started::InternalProgram(binary) | Started::ExternalProgram(binary)) => {
                let _ = started.insert(binary);
            }
            None => {}
        }
    }

    Ok(started)
}

fn units_compared(architecture: &Architecture, seen: &Seen) -> Result<Vec<Difference>, Never> {
    let mapped: BTreeSet<&String> = architecture.units.iter().map(|unit| &unit.name).collect();
    let enabled: BTreeSet<&String> = architecture.enabled.iter().collect();
    let mut found = Vec::new();

    for unit in seen.units.iter().filter(|unit| unit.starts_with(PREFIX) && !mapped.contains(unit)) {
        found.push(Difference::Unmapped(unit.clone()));
    }

    for unit in architecture.units.iter().filter(|unit| {
        unit.name.ends_with(SERVICE_SUFFIX) && enabled.contains(&unit.name) && !seen.units.contains(&unit.name)
    }) {
        let Ok(declines) = field(&unit.text, SERVICE, DECLINES);

        match declines {
            Some(_) => {}
            None => found.push(Difference::NotRunning(unit.name.clone())),
        }
    }

    Ok(found)
}

pub fn compared(architecture: &Architecture, seen: &Seen) -> Result<Vec<Difference>, Never> {
    let Ok(mut found) = units_compared(architecture, seen);
    let Ok(subscribers) = subscribers(&architecture.facts);
    let Ok(started) = started_by_running(architecture, seen);

    for pid in &seen.pool {
        let process = match seen.processes.get(pid) {
            Some(process) => process,
            None => continue,
        };
        let parent = seen.processes.get(&process.parent).map(|parent| parent.binary.as_str());

        match (subscribers.contains(&process.binary), parent == Some(USER_MANAGER), started.contains(&process.binary)) {
            (false, true | false, true | false) => found.push(Difference::Undeclared(process.binary.clone(), *pid)),
            (true, true, false) => found.push(Difference::Orphaned(process.binary.clone(), *pid)),
            (true, true, true) | (true, false, true | false) => {}
        }
    }

    found.sort();

    Ok(found)
}

fn seen(stage: &mut Device) -> Result<Seen, Never> {
    let Ok(systemctl) = Program::Systemctl.name();
    let Ok(ss) = Program::Ss.name();
    let Ok(ps) = Program::Ps.name();
    let Ok(running) = stage.user(&format!("{systemctl} {RUNNING}"));
    let Ok(sockets) = stage.user(&format!("{ss} -xp"));
    let Ok(tree) = stage.user(&format!("{ps} -eo pid=,ppid=,args="));
    let Ok(units) = units(&running);
    let Ok(processes) = processes(&tree);
    let Ok(pool) = pool(&sockets);

    Ok(Seen { units, processes, pool })
}

fn mapped(stage: &mut Device) -> CheckResult {
    let architecture = match console_repository::root() {
        Ok(root) => match Architecture::read(&root) {
            Ok(architecture) => architecture,
            Err(fault) => return cannot(&format!("the map cannot be read: {fault}")),
        },
        Err(fault) => return cannot(&format!("{fault}")),
    };
    let Ok(seen) = seen(stage);

    match seen.units.is_empty() {
        true => return cannot("the device named no running unit, so there is nothing to hold the map against"),
        false => {}
    }

    let Ok(differences) = compared(&architecture, &seen);

    match differences.is_empty() {
        true => Ok(()),
        false => failed(format!(
            "the device and the map disagree:\n{}",
            differences.iter().map(|one| format!("  {one}")).collect::<Vec<_>>().join("\n")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SS: &str = "\
Netid State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process
u_str LISTEN 0      4096   /run/user/1000/console/events.sock 100 * 0 users:((\"console-events\",pid=10,fd=3))
u_str ESTAB  0      0      /run/user/1000/console/events.sock 101 * 201 users:((\"console-events\",pid=10,fd=7))
u_str ESTAB  0      0      * 201 * 101 users:((\"console-bar\",pid=20,fd=5))
u_str ESTAB  0      0      * 301 * 302 users:((\"pipewire\",pid=30,fd=5))
";

    #[test]
    fn a_running_unit_is_asked_for_by_its_whole_name() {
        assert!(
            RUNNING.split_whitespace().any(|word| word == "--full"),
            "systemctl cuts a long unit name to fit the terminal, and a cut name is a unit the tree has no file for"
        );
    }

    #[test]
    fn the_pool_is_whoever_is_at_the_other_end_of_its_socket() {
        let Ok(pool) = pool(SS);

        assert_eq!(pool, BTreeSet::from([20]));
    }

    #[test]
    fn a_process_is_named_for_the_program_it_runs_rather_than_its_cut_comm() {
        let Ok(held) = processes("  20     1 /usr/local/bin/console-control-center --flag\n   1     0 /usr/lib/systemd/systemd --user\n");

        assert_eq!(held.get(&20).map(|one| one.binary.as_str()), Some("console-control-center"));
        assert_eq!(held.get(&1).map(|one| one.binary.as_str()), Some("systemd"));
    }

    fn fact(package: &str, target: &str, kind: Kind, what: &str) -> Fact {
        Fact { package: String::from(package), target: String::from(target), kind, what: String::from(what) }
    }

    fn architecture() -> Architecture {
        Architecture {
            facts: vec![
                fact("console-status-bar", "console-bar", Kind::Builds, "console_bar"),
                fact("console-status-bar", "console-bar", Kind::Links, "console_status_bar"),
                fact("console-status-bar", "lib", Kind::Builds, "console_status_bar"),
                fact("console-status-bar", "lib", Kind::Subscribes, "Sound"),
                fact("console-screen", "console-scale", Kind::Builds, "console_scale"),
            ],
            enabled: vec![String::from("console-bar.service")],
            units: vec![console_architecture::units::Unit {
                name: String::from("console-bar.service"),
                text: String::from("[Service]\nExecStart=/usr/local/bin/console-bar\n"),
            }],
        }
    }

    fn process(parent: u32, binary: &str) -> Process {
        Process { parent, binary: String::from(binary) }
    }

    #[test]
    fn a_device_that_matches_the_map_says_nothing() {
        let seen = Seen {
            units: BTreeSet::from([String::from("console-bar.service")]),
            processes: BTreeMap::from([(1, process(0, "systemd")), (20, process(1, "console-bar"))]),
            pool: BTreeSet::from([20]),
        };
        let Ok(differences) = compared(&architecture(), &seen);

        assert_eq!(differences, Vec::new());
    }

    #[test]
    fn what_is_in_one_and_not_the_other_is_said() {
        let seen = Seen {
            units: BTreeSet::from([String::from("console-stray.service")]),
            processes: BTreeMap::from([
                (1, process(0, "systemd")),
                (20, process(1, "console-bar")),
                (21, process(1, "console-scale")),
            ]),
            pool: BTreeSet::from([20, 21]),
        };
        let Ok(differences) = compared(&architecture(), &seen);

        assert_eq!(
            differences,
            vec![
                Difference::Unmapped(String::from("console-stray.service")),
                Difference::NotRunning(String::from("console-bar.service")),
                Difference::Undeclared(String::from("console-scale"), 21),
                Difference::Orphaned(String::from("console-bar"), 20),
            ]
        );
    }
}
