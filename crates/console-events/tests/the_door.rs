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

mod pool;

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use console_events::wire::{self, Message};
use console_program_contract::{Change, Topic};
use console_waiting::{Ready, Schedule, until};
use pool::{BEFORE_LONG, Failure, serve_at, socket};
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

const REFUSING: Duration = Duration::from_secs(1);

const SPINNING: Duration = Duration::from_millis(300);

const FEW: u64 = 256;

const A_TICK: Duration = Duration::from_millis(10);

fn on_the_processor(stat: &mut File) -> Result<Duration, Failure> {
    let mut said = String::new();

    stat.seek(SeekFrom::Start(0))?;
    stat.read_to_string(&mut said)?;

    let (_, after_the_name) = said.rsplit_once(')').ok_or("a name in brackets")?;
    let mut fields = after_the_name.split_whitespace().skip(11);
    let user = fields.next().ok_or("the time spent as the program")?;
    let system = fields.next().ok_or("the time spent as the kernel on its behalf")?;
    let user: u32 = user.parse()?;
    let system: u32 = system.parse()?;

    Ok(A_TICK.saturating_mul(user.saturating_add(system)))
}

#[test]
fn a_door_that_cannot_open_is_not_asked_again_as_fast_as_the_machine_turns() -> Result<(), Failure> {
    let at = socket("door")?;
    let handed = serve_at(&at)?;
    let mut stat = File::open("/proc/self/stat")?;
    let was = getrlimit(Resource::Nofile);

    setrlimit(Resource::Nofile, Rlimit { current: Some(FEW), maximum: was.maximum })?;

    let mut spent: Vec<File> = std::iter::repeat_with(|| File::open("/dev/null"))
        .map_while(|opened| match opened {
            Ok(null) => Some(null),
            Err(_out_of_descriptors) => None,
        })
        .collect();

    drop(spent.pop());

    let queued = UnixStream::connect(&at)?;
    let before = on_the_processor(&mut stat)?;
    let Ok(patience) = Schedule::of(REFUSING);
    let Ok(_refused_all_along) = until(patience, || Ok(Ready::NotYet));
    let after = on_the_processor(&mut stat)?;
    let spun = after.saturating_sub(before);

    drop(spent);
    setrlimit(Resource::Nofile, was)?;

    assert!(
        spun < SPINNING,
        "a door that could not open was asked again as fast as the machine turns: {spun:?} on \
         the processor in {REFUSING:?} of nothing happening"
    );

    let asked = wire::encoded(&Message::Subscribe(Topic::Sound))?;
    let mut asking = queued.try_clone()?;

    writeln!(asking, "{asked}")?;

    let saying = handed.recv_timeout(BEFORE_LONG).map_err(|_| "the program queued at the door was never let in")?;

    saying
        .send(Change { topic: Topic::Sound, text: "let in at last".to_string() })
        .map_err(|_| "the pool stopped listening to its own source")?;

    let _ = queued.set_read_timeout(Some(BEFORE_LONG));
    let mut told = String::new();

    BufReader::new(queued).read_line(&mut told)?;

    assert!(told.contains("let in at last"), "the program let in heard {told:?}");

    let _ = std::fs::remove_file(&at);

    Ok(())
}
