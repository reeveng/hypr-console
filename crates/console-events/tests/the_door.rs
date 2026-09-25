//! A door that cannot open is waited on rather than asked again.
//!
//! A process with no descriptors left is refused by `accept` before the queue
//! is looked at, and the door stays readable for as long as someone is queued
//! at it -- so a loop asleep in `poll` that asks the door again every time it
//! is readable asks it as fast as the machine turns, in the one thread every
//! program on the desktop is told from. What would say so is the time the
//! process spends on the processor while nothing is happening, so that is what
//! is read: a second of a refusing door, and how much of it was spent.
//!
//! The refusal is made here rather than waited for. The limit on descriptors
//! is lowered and spent on `/dev/null` until one is left, and a program queued
//! at the door takes that one on the way in, which is the drought the pool
//! meets. The test is alone in its binary because the limit is the process's,
//! and every other test would be refused beside it; the process's own numbers
//! are opened before the drought, because reading them wants a descriptor too.
//!
//! Then the descriptors are handed back and the program queued all along is
//! let in and told something, because a door that stops spinning by never
//! opening again is the other way to get this wrong.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use console_events::serving;
use console_events::sources::Subscribed;
use console_events::wire::{self, Message};
use console_program_contract::{Change, Topic};
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

const BEFORE_LONG: Duration = Duration::from_secs(5);

const REFUSING: Duration = Duration::from_secs(1);

const SPINNING: Duration = Duration::from_millis(300);

const FEW: u64 = 256;

static SAYING: OnceLock<Sender<Sender<Change>>> = OnceLock::new();

fn source(topic: &Topic, say: Sender<Change>) -> Result<Subscribed, console_core_never::Never> {
    console_events::sources::handed_to(SAYING.get(), &Topic::Sound, topic, say)
}

fn socket() -> PathBuf {
    std::env::temp_dir().join(format!("console-events-door-{}.sock", std::process::id()))
}

fn up(at: &Path) {
    let began = Instant::now();

    while began.elapsed() < BEFORE_LONG {
        match at.exists() {
            true => return,
            false => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

fn on_the_processor(stat: &mut File) -> Duration {
    let mut said = String::new();

    stat.seek(SeekFrom::Start(0)).expect("the start of the process's own numbers");
    stat.read_to_string(&mut said).expect("the process's own numbers");

    let stat = said;
    let (_, after_the_name) = stat.rsplit_once(')').expect("a name in brackets");
    let fields: Vec<&str> = after_the_name.split_whitespace().collect();
    let ticks: u64 = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();

    Duration::from_millis(ticks * 10)
}

#[test]
fn a_door_that_cannot_open_is_not_asked_again_as_fast_as_the_machine_turns() {
    let at = socket();
    let (handing, handed) = channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, source));

    up(&at);

    let mut stat = File::open("/proc/self/stat").expect("the process's own numbers");
    let was = getrlimit(Resource::Nofile);

    setrlimit(Resource::Nofile, Rlimit { current: Some(FEW), maximum: was.maximum }).expect("fewer descriptors");

    let mut spent: Vec<File> = Vec::new();

    while let Ok(null) = File::open("/dev/null") {
        spent.push(null);
    }

    drop(spent.pop());

    let queued = UnixStream::connect(&at).expect("a place in the queue at the door");

    let before = on_the_processor(&mut stat);

    std::thread::sleep(REFUSING);

    let spun = on_the_processor(&mut stat) - before;

    drop(spent);
    setrlimit(Resource::Nofile, was).expect("the descriptors back");

    assert!(
        spun < SPINNING,
        "a door that could not open was asked again as fast as the machine turns: {spun:?} on \
         the processor in {REFUSING:?} of nothing happening"
    );

    let asked = wire::encoded(&Message::Subscribe(Topic::Sound)).expect("the wire");
    let mut asking = queued.try_clone().expect("the connection");

    writeln!(asking, "{asked}").expect("asking to listen");

    let saying = handed.recv_timeout(BEFORE_LONG).expect("the program queued at the door was never let in");

    saying.send(Change { topic: Topic::Sound, text: "let in at last".to_string() }).expect("the pool");

    let _ = queued.set_read_timeout(Some(BEFORE_LONG));
    let mut told = String::new();

    BufReader::new(queued).read_line(&mut told).expect("told something once it was let in");

    assert!(told.contains("let in at last"), "the program let in heard {told:?}");

    let _ = std::fs::remove_file(&at);
}
