//! What the machine spent while no one was watching, and what spent it.
//!
//! The device's own idle draw took a person sitting over an ssh session with
//! two sixty-second windows to find, and what it found -- an audio stack awake
//! on a machine with nothing playing -- was true for months before anyone
//! measured it. A measurement someone has to be present for is a measurement
//! of the moment they were present. So this is the same two readings taken
//! while no one is there: a line every few minutes, kept, and read afterwards.
//!
//! ## What one looks like
//!
//! ```text
//! {"at":1758240300,"up":67932.4,"slept":10423.0,"energy":41.230,"draw":9.12,
//!  "gpu":3.10,"percent":88,
//!  "busy":{"wireplumber":1234.5,"pipewire":701.2,"kew":688.0},
//!  "held":{"console-panels":212.4,"Hyprland":180.9,"kew":41.0}}
//! ```
//!
//! Every number on a line is a **counter, not a rate**: what is left in the
//! battery, seconds of CPU since a program started, seconds since the machine
//! came up.
//! A rate is a question about two moments and nothing here is entitled to
//! answer it -- a snapshot that wrote watts would be writing an average over a
//! window it chose, and the window someone wants is the one they ask about
//! afterwards. [`between`] is where two lines become watts. `draw` is the one
//! reading that is already a rate, because the battery answers that question
//! itself and nothing is gained by throwing the answer away.
//!
//! **The battery rather than RAPL.** `energy_uj` under `powercap` is the
//! obvious counter and a user service cannot read it: it is root-only on every
//! machine that carries the PLATYPUS mitigation, which is all of them. The
//! battery's own files are world-readable and answer a better question anyway
//! -- RAPL is the package, and what empties a handheld is the package plus the
//! screen plus the radios. The cost is that a machine on the cable says
//! nothing, which is honest: there is no drain to measure while it is filling.
//!
//! `up` counts suspended time and `slept` is how much of it was spent asleep,
//! so `up - slept` is what the machine was awake for and the only denominator
//! worth dividing by: a device that suspends for the night and is read in the
//! morning otherwise reports an idle draw of about a watt, which is a fact
//! about the night rather than about the desktop. A kernel with no
//! `total_hw_sleep` says nothing, and nothing is taken to mean it never slept.
//!
//! Which is why the watts come out of the windows that held no sleep at all,
//! and a reading with a suspend inside it is dropped from the rate the way a
//! reading on the cable is. One fall in the battery across a window that was
//! partly awake and partly asleep cannot be divided between the two: the
//! counters say how long each lasted and never which of them spent what.
//! Apportioning it at some assumed rate would be this crate writing down the
//! answer it was built to go and measure. A log that is nothing but straddling
//! windows says nothing, and says so.
//!
//! What a night asleep cost is a different question and the same windows
//! answer it the other way round. A window that was mostly asleep is one
//! sleep, near enough, because the timer does not fire while the machine is
//! down and fires within a few minutes of it waking; what the battery fell by
//! across it, over the hours asleep, is a rate that is *at most* what the
//! sleep drew, since the awake minutes either side are in it too. [`between`]
//! keeps the worst of them, because one sleep that kept the machine half awake
//! is the fault worth finding and an average over a week of good nights hides
//! it. A window that rose was on the cable and is not a sleep's cost.
//!
//! ## What spent it, by name rather than by number
//!
//! A pid is a number that means a different program next week, and a report
//! that names one is a report no one can read. So CPU time is summed by the
//! name in `/proc/pid/stat` -- every `wireplumber` on the machine is one line
//! -- and a restart shows up as a counter going backwards, which [`between`]
//! drops rather than counts as time spent. `NAMED` of them, which is the top
//! of the list by what each has spent since it started: a program that has
//! never used a second of CPU is not the one draining anything.
//!
//! What each name holds in memory is on the same line of `/proc/pid/stat`,
//! summed the same way and kept as `held`, in megabytes, `NAMED` of them by
//! size. A leak is the one thing a single reading cannot show and every pair
//! of them can, so [`between`] answers it as growth from the first reading to
//! the last, and only for the names that were there at both: a program that
//! started in between did not grow, it arrived.
//!
//! ## What died and came back
//!
//! A unit systemd restarts reads as up to every check that asks after it, and
//! a program that dumped core and was started again is running when anybody
//! looks. So a reading also keeps two counters: `restarts`, each unit's own
//! `NRestarts` from the system manager and the user's, and `crashes`, the
//! core dumps `systemd-coredump` is holding, by the name in the dump's file
//! name, which is the second of its dotted fields: a dot in the program's own
//! name is written `\x2e`, so that one never splits it. Only names above nothing are written, which is what keeps the line
//! short on a machine that is well. [`between`] adds up how far each rose
//! window by window, the way it adds up CPU time: a count that fell is a unit
//! stopped by hand or a dump vacuumed away, and neither is a crash.
//!
//! `TICK` is USER_HZ, which the kernel fixes at 100 for every architecture this
//! desktop is built for. `sysconf` would answer it at the cost of the one
//! unsafe call in this crate, and a wrong answer here scales every reading by
//! the same constant rather than changing which name is at the top. `PAGE` is
//! the same argument about the size of a page, which is four kilobytes on
//! every machine this desktop is built for.
//!
//! ## What it costs to ask
//!
//! A few hundred small reads out of `/proc` and a handful out of `/sys`, once every
//! few minutes, from a process that exits. Nothing stays resident, nothing
//! holds a device open, and the timer that runs it accepts a minute of slack so
//! the wake-up is shared with whatever else the machine was going to do.

use std::collections::BTreeMap;
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::read;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::{Float, index};
use console_battery::{BATTERY, Charge, SUPPLIES, charge};

const UPTIME: &str = "/proc/uptime";

const SLEPT: &str = "/sys/power/suspend_stats/total_hw_sleep";

const SENSORS: &str = "/sys/class/hwmon";

const AMDGPU: &str = "amdgpu";

const DRAWN: &str = "power1_average";

const RUNNING: &str = "/proc";

const COREDUMPS: &str = "/var/lib/systemd/coredump";

const DUMPED: &str = "core";

const ESCAPED_DOT: &str = "\\x2e";

const RESTARTS: &str = "NRestarts=";

const UNIT: &str = "Id=";

const FRACTION: u32 = 1;

const WHOLE_NUMBER: u32 = 0;

const STORE: &str = "used.jsonl";

const MILLION: f64 = 1_000_000.0;

const TICK: f64 = 100.0;

const HOUR: f64 = 3_600.0;

const WHOLE: f64 = 100.0;

const NONE: f64 = 0.0;

const NAMED: u32 = 20;

const FIELDS_BEFORE_UTIME: u32 = 11;

const FIELDS_BETWEEN_STIME_AND_RSS: u32 = 8;

const PAGE: f64 = 4_096.0;

const MEGABYTE: f64 = 1_048_576.0;

struct Spent {
    named: String,
    seconds: f64,
    megabytes: Option<f64>,
}


#[derive(Debug, Clone, PartialEq)]
pub struct Moment {
    pub at: u64,
    pub up: Option<f64>,
    pub slept: f64,
    pub energy: Option<f64>,
    pub draw: Option<f64>,
    pub gpu: Option<f64>,
    pub percent: Option<i32>,
    pub busy: Vec<(String, f64)>,
    pub held: Vec<(String, f64)>,
    pub restarts: Vec<(String, f64)>,
    pub crashes: Vec<(String, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Used {
    pub awake: f64,
    pub asleep: f64,
    pub watthours: f64,
    pub flat: f64,
    pub gpu: Option<f64>,
    pub busy: Vec<(String, f64)>,
    pub grew: Vec<(String, f64)>,
    pub restarted: Vec<(String, f64)>,
    pub crashed: Vec<(String, f64)>,
    pub worst_sleep: Option<Sleep>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sleep {
    pub woke: u64,
    pub asleep: f64,
    pub watthours: f64,
}

impl Sleep {
    pub fn at_most(self) -> Result<f64, Never> {
        Ok(self.watthours / (self.asleep / HOUR))
    }
}

#[derive(Debug)]
pub enum Unsaid {
    Nowhere,
    Making(PathBuf, std::io::Error),
    Opening(PathBuf, std::io::Error),
    Writing(PathBuf, std::io::Error),
}

impl fmt::Display for Unsaid {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unsaid::Nowhere => write!(to, "nothing says where a reading would be written"),
            Unsaid::Making(at, fault) => {
                write!(to, "{}: making the directory for it: {fault}", at.display())
            }
            Unsaid::Opening(at, fault) => write!(to, "{}: opening it: {fault}", at.display()),
            Unsaid::Writing(at, fault) => write!(to, "{}: appending to it: {fault}", at.display()),
        }
    }
}

impl std::error::Error for Unsaid {}

pub fn where_() -> Result<Option<PathBuf>, Never> {
    let Ok(ours) = console_core_places::Base::State.ours();

    Ok(ours.map(|ours| ours.join(STORE)))
}

fn counted(text: &str) -> Result<Option<f64>, Never> {
    Ok(match text.trim().parse::<f64>() {
        Ok(number) => Some(number),
        Err(_not_a_number) => None,
    })
}

fn number(at: &Path) -> Result<Option<f64>, Never> {
    let Ok(held) = read(at);
    let Ok(text) = held.text();

    match text {
        Some(text) => counted(&text),
        None => Ok(None),
    }
}

fn up() -> Result<Option<f64>, Never> {
    let Ok(held) = read(Path::new(UPTIME));
    let Ok(text) = held.text();

    let first = text.as_deref().map(str::split_whitespace).and_then(|mut words| words.next());

    match first {
        Some(first) => counted(first),
        None => Ok(None),
    }
}

fn slept() -> Result<f64, Never> {
    let Ok(microseconds) = number(Path::new(SLEPT));

    Ok(match microseconds {
        Some(microseconds) => microseconds / MILLION,
        None => NONE,
    })
}

fn gpu() -> Result<Option<f64>, Never> {
    let sensors = match std::fs::read_dir(SENSORS) {
        Ok(sensors) => sensors,
        Err(_nothing_to_ask) => return Ok(None),
    };

    for sensor in sensors.flatten().map(|sensor| sensor.path()) {
        let Ok(held) = read(&sensor.join("name"));
        let Ok(named) = held.text();

        match named.as_deref().map(str::trim) {
            Some(AMDGPU) => {}
            Some(_) | None => continue,
        }

        let Ok(microwatts) = number(&sensor.join(DRAWN));

        match microwatts {
            Some(microwatts) => return Ok(Some(microwatts / MILLION)),
            None => continue,
        }
    }

    Ok(None)
}

fn spending(text: &str) -> Result<Option<Spent>, Never> {
    let (through_name, rest) = match text.rsplit_once(')') {
        Some(split) => split,
        None => return Ok(None),
    };

    let named = match through_name.split_once('(') {
        Some((_, named)) => named.to_string(),
        None => return Ok(None),
    };

    let Ok(before) = index(FIELDS_BEFORE_UTIME);
    let mut fields = rest.split_whitespace().skip(before);
    let mine = fields.next().map(str::parse::<u64>);
    let system = fields.next().map(str::parse::<u64>);
    let Ok(between) = index(FIELDS_BETWEEN_STIME_AND_RSS);
    let resident = fields.nth(between).map(str::parse::<u64>);

    let megabytes = match resident {
        Some(Ok(pages)) => {
            let Ok(pages) = pages.float();

            Some(pages * PAGE / MEGABYTE)
        }
        Some(Err(_not_a_number)) => None,
        None => None,
    };

    Ok(match (mine, system) {
        (Some(Ok(mine)), Some(Ok(system))) => {
            let Ok(ticks) = mine.saturating_add(system).float();

            Some(Spent { named, seconds: ticks / TICK, megabytes })
        }
        (None, _) | (_, None) => None,
        (Some(Err(_not_a_number)), _) | (_, Some(Err(_not_a_number))) => None,
    })
}

fn largest(summed: BTreeMap<String, f64>) -> Result<Vec<(String, f64)>, Never> {
    let mut gathered: Vec<(String, f64)> = summed.into_iter().collect();

    let Ok(named) = index(NAMED);

    gathered.sort_by(|one, another| another.1.total_cmp(&one.1));
    gathered.truncate(named);

    Ok(gathered)
}

#[derive(Debug, Clone, PartialEq)]
struct Running {
    busy: Vec<(String, f64)>,
    held: Vec<(String, f64)>,
}

fn running() -> Result<Running, Never> {
    let running = match std::fs::read_dir(RUNNING) {
        Ok(running) => running,
        Err(_nothing_to_ask) => return Ok(Running { busy: Vec::new(), held: Vec::new() }),
    };
    let mut spent: BTreeMap<String, f64> = BTreeMap::new();
    let mut resident: BTreeMap<String, f64> = BTreeMap::new();

    for process in running.flatten().map(|process| process.path()) {
        let numbered = process
            .file_name()
            .and_then(|named| named.to_str())
            .map(|named| named.parse::<u32>());

        match numbered {
            Some(Ok(_a_process)) => {}
            None => continue,
            Some(Err(_not_a_number)) => continue,
        }

        let Ok(held) = read(&process.join("stat"));
        let Ok(text) = held.text();

        let text = match text {
            Some(text) => text,
            None => continue,
        };
        let Ok(spending) = spending(&text);

        let Spent { named, seconds, megabytes } = match spending {
            Some(spending) => spending,
            None => continue,
        };

        match megabytes {
            Some(megabytes) => *resident.entry(named.clone()).or_insert(NONE) += megabytes,
            None => {}
        }

        *spent.entry(named).or_insert(NONE) += seconds;
    }

    let Ok(busy) = largest(spent);
    let Ok(held) = largest(resident);

    Ok(Running { busy, held })
}

pub fn restarts_in(said: &str) -> Result<Vec<(String, f64)>, Never> {
    let mut counted = Vec::new();

    for block in said.split("\n\n") {
        let unit = block.lines().find_map(|line| line.strip_prefix(UNIT));
        let restarts = block.lines().find_map(|line| line.strip_prefix(RESTARTS)).map(str::parse::<u32>);

        match (unit, restarts) {
            (Some(unit), Some(Ok(restarts))) => match restarts > 0 {
                true => counted.push((unit.to_string(), f64::from(restarts))),
                false => {}
            },
            (Some(_), Some(Err(_not_a_number))) => {}
            (None, _) | (_, None) => {}
        }
    }

    Ok(counted)
}

fn restarts() -> Result<Vec<(String, f64)>, Never> {
    let mut counted = Vec::new();

    for manager in [None, Some("--user")] {
        let Ok(mut asking) = Program::Systemctl.command();

        asking.args(manager).args(["show", "--property=Id,NRestarts", "*"]);

        match asking.output() {
            Ok(said) => match said.status.success() {
                true => {
                    let Ok(these) = restarts_in(&String::from_utf8_lossy(&said.stdout));

                    counted.extend(these);
                }
                false => eprintln!(
                    "console-resource-usage: systemctl would not say what restarted: {}",
                    String::from_utf8_lossy(&said.stderr).trim()
                ),
            },
            Err(fault) => eprintln!("console-resource-usage: systemctl would not say what restarted: {fault}"),
        }
    }

    Ok(counted)
}

pub fn dumped(file: &str) -> Result<Option<String>, Never> {
    let mut fields = file.split('.');

    match fields.next() {
        Some(DUMPED) => {}
        Some(_) | None => return Ok(None),
    }

    let named = fields.next();
    let (_whose, _boot, _process, time) = (fields.next(), fields.next(), fields.next(), fields.next());

    Ok(match (named, time) {
        (Some(named), Some(time)) => match (named.is_empty(), time.bytes().all(|digit| digit.is_ascii_digit())) {
            (false, true) => Some(named.replace(ESCAPED_DOT, ".")),
            (true, _) | (_, false) => None,
        },
        (None, _) | (_, None) => None,
    })
}

fn crashes() -> Result<Vec<(String, f64)>, Never> {
    let dumps = match std::fs::read_dir(COREDUMPS) {
        Ok(dumps) => dumps,
        Err(_nothing_to_ask) => return Ok(Vec::new()),
    };
    let mut counted: BTreeMap<String, f64> = BTreeMap::new();

    for dump in dumps.flatten() {
        let file = dump.file_name();
        let Ok(named) = dumped(&file.to_string_lossy());

        match named {
            Some(named) => *counted.entry(named).or_insert(NONE) += 1.0,
            None => {}
        }
    }

    Ok(counted.into_iter().collect())
}

fn battery() -> Result<Option<PathBuf>, Never> {
    let supplies = match std::fs::read_dir(SUPPLIES) {
        Ok(supplies) => supplies,
        Err(_nothing_to_ask) => return Ok(None),
    };

    for supply in supplies.flatten().map(|supply| supply.path()) {
        let Ok(held) = read(&supply.join("type"));
        let Ok(kind) = held.text();

        match kind.as_deref().map(str::trim) {
            Some(BATTERY) => return Ok(Some(supply)),
            Some(_) | None => {}
        }
    }

    Ok(None)
}

pub fn taken(at: u64) -> Result<Moment, Never> {
    let Ok(up) = up();
    let Ok(slept) = slept();
    let Ok(gpu) = gpu();
    let Ok(supply) = battery();

    let (energy, draw) = match supply {
        Some(supply) => {
            let Ok(microwatthours) = number(&supply.join("energy_now"));
            let Ok(microwatts) = number(&supply.join("power_now"));

            (
                microwatthours.map(|microwatthours| microwatthours / MILLION),
                microwatts.map(|microwatts| microwatts / MILLION),
            )
        }
        None => (None, None),
    };
    let Ok(text) = charge();
    let Ok(charged) = Charge::of(&text);
    let Ok(Running { busy, held }) = running();
    let Ok(restarts) = restarts();
    let Ok(crashes) = crashes();

    Ok(Moment { at, up, slept, energy, draw, gpu, percent: charged.percent, busy, held, restarts, crashes })
}

fn quoted(text: &str) -> Result<String, Never> {
    Ok(serde_json::Value::String(text.to_string()).to_string())
}

pub fn written(moment: &Moment) -> Result<String, Never> {
    let mut text = format!("{{\"at\":{}", moment.at);

    match moment.up {
        Some(up) => text.push_str(&format!(",\"up\":{up:.1}")),
        None => {}
    }

    text.push_str(&format!(",\"slept\":{:.1}", moment.slept));

    match moment.energy {
        Some(energy) => text.push_str(&format!(",\"energy\":{energy:.3}")),
        None => {}
    }

    match moment.draw {
        Some(draw) => text.push_str(&format!(",\"draw\":{draw:.2}")),
        None => {}
    }

    match moment.gpu {
        Some(gpu) => text.push_str(&format!(",\"gpu\":{gpu:.2}")),
        None => {}
    }

    match moment.percent {
        Some(percent) => text.push_str(&format!(",\"percent\":{percent}")),
        None => {}
    }

    let Ok(busy) = object(&moment.busy, FRACTION);
    let Ok(held) = object(&moment.held, FRACTION);
    let Ok(restarts) = object(&moment.restarts, WHOLE_NUMBER);
    let Ok(crashes) = object(&moment.crashes, WHOLE_NUMBER);

    text.push_str(&format!(",\"busy\":{busy},\"held\":{held},\"restarts\":{restarts},\"crashes\":{crashes}}}"));

    Ok(text)
}

fn object(named: &[(String, f64)], places: u32) -> Result<String, Never> {
    let Ok(places) = index(places);
    let mut text = String::from("{");
    let mut first = true;

    for (named, amount) in named {
        match first {
            true => {}
            false => text.push(','),
        }

        first = false;

        let Ok(named) = quoted(named);

        text.push_str(&format!("{named}:{amount:.places$}"));
    }

    text.push('}');

    Ok(text)
}

fn named(object: &serde_json::Map<String, serde_json::Value>, key: &str) -> Result<Vec<(String, f64)>, Never> {
    Ok(match object.get(key).and_then(serde_json::Value::as_object) {
        Some(named) => named
            .iter()
            .filter_map(|(named, amount)| amount.as_f64().map(|amount| (named.to_string(), amount)))
            .collect(),
        None => Vec::new(),
    })
}

pub fn of(text: &str) -> Result<Option<Moment>, Never> {
    let held: serde_json::Value = match serde_json::from_str(text) {
        Ok(held) => held,
        Err(_not_a_line) => return Ok(None),
    };

    let object = match held.as_object() {
        Some(object) => object,
        None => return Ok(None),
    };

    let at = match object.get("at").and_then(serde_json::Value::as_u64) {
        Some(at) => at,
        None => return Ok(None),
    };
    let Ok(busy) = named(object, "busy");
    let Ok(held) = named(object, "held");
    let Ok(restarts) = named(object, "restarts");
    let Ok(crashes) = named(object, "crashes");
    let slept = match object.get("slept").and_then(serde_json::Value::as_f64) {
        Some(slept) => slept,
        None => NONE,
    };

    Ok(Some(Moment {
        at,
        up: object.get("up").and_then(serde_json::Value::as_f64),
        slept,
        energy: object.get("energy").and_then(serde_json::Value::as_f64),
        draw: object.get("draw").and_then(serde_json::Value::as_f64),
        gpu: object.get("gpu").and_then(serde_json::Value::as_f64),
        percent: match object.get("percent").and_then(serde_json::Value::as_i64) {
            Some(percent) => match i32::try_from(percent) {
                Ok(percent) => Some(percent),
                Err(_no_battery_says_that) => None,
            },
            None => None,
        },
        busy,
        held,
        restarts,
        crashes,
    }))
}

pub fn kept(at: &Path, moment: &Moment) -> Result<(), Unsaid> {
    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unsaid::Making(holding.to_path_buf(), fault))?,
        None => {}
    }

    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(
            explicit040_no_torn_write,
            reason = "a log appended to a line at a time: a rename over it would take away every reading already in it, and one short line written to an O_APPEND descriptor lands whole"
        )
    )]
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(at)
        .map_err(|fault| Unsaid::Opening(at.to_path_buf(), fault))?;

    let Ok(text) = written(moment);

    file.write_all(format!("{text}\n").as_bytes())
        .map_err(|fault| Unsaid::Writing(at.to_path_buf(), fault))
}

pub fn between(moments: &[Moment]) -> Result<Option<Used>, Never> {
    let mut used = Used {
        awake: NONE,
        asleep: NONE,
        watthours: NONE,
        flat: NONE,
        gpu: None,
        busy: Vec::new(),
        grew: Vec::new(),
        restarted: Vec::new(),
        crashed: Vec::new(),
        worst_sleep: None,
    };
    let mut drawn = (NONE, NONE);
    let mut busy: BTreeMap<String, f64> = BTreeMap::new();
    let mut restarted: BTreeMap<String, f64> = BTreeMap::new();
    let mut crashed: BTreeMap<String, f64> = BTreeMap::new();

    for pair in moments.windows(2) {
        let (before, after) = match (pair.first(), pair.last()) {
            (Some(before), Some(after)) => (before, after),
            (None, _) | (_, None) => continue,
        };

        let went = match (before.up, after.up) {
            (Some(was), Some(is)) => is - was,
            (None, _) | (_, None) => continue,
        };

        match went > NONE {
            true => {}
            false => continue,
        }

        let Ok(wall) = after.at.saturating_sub(before.at).float();
        let asleep = (after.slept - before.slept).max(NONE);
        let awake = (went - asleep).max(NONE);

        used.awake += awake;
        used.asleep += asleep;

        let dropped = match (before.energy, after.energy) {
            (Some(was), Some(is)) => (was - is).max(NONE),
            (None, _) | (_, None) => NONE,
        };

        match (dropped > NONE, asleep > NONE) {
            (true, false) => {
                used.watthours += dropped;
                used.flat += wall / HOUR;
            }
            (true, true) | (false, false) | (false, true) => {}
        }

        match (dropped > NONE, asleep > awake) {
            (true, true) => {
                let slept = Sleep { woke: after.at, asleep, watthours: dropped };
                let Ok(worse) = worse(used.worst_sleep, slept);

                used.worst_sleep = Some(worse);
            }
            (true, false) | (false, true) | (false, false) => {}
        }

        match after.gpu {
            Some(watts) => drawn = (drawn.0 + watts, drawn.1 + 1.0),
            None => {}
        }

        let Ok(()) = risen(Counted { before: &before.busy, after: &after.busy, unseen: Unseen::Unknown }, &mut busy);
        let Ok(()) =
            risen(Counted { before: &before.restarts, after: &after.restarts, unseen: Unseen::None }, &mut restarted);
        let Ok(()) =
            risen(Counted { before: &before.crashes, after: &after.crashes, unseen: Unseen::None }, &mut crashed);
    }

    match used.awake > NONE {
        true => {}
        false => return Ok(None),
    }

    used.gpu = match drawn.1 > NONE {
        true => Some(drawn.0 / drawn.1),
        false => None,
    };

    let Ok(busy) = largest(busy);
    let Ok(grew) = grew(moments);
    let Ok(restarted) = largest(restarted);
    let Ok(crashed) = largest(crashed);

    used.busy = busy;
    used.grew = grew;
    used.restarted = restarted;
    used.crashed = crashed;

    Ok(Some(used))
}

fn worse(held: Option<Sleep>, slept: Sleep) -> Result<Sleep, Never> {
    let held = match held {
        Some(held) => held,
        None => return Ok(slept),
    };
    let Ok(was) = held.at_most();
    let Ok(is) = slept.at_most();

    Ok(match is > was {
        true => slept,
        false => held,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unseen {
    Unknown,
    None,
}

struct Counted<'a> {
    before: &'a [(String, f64)],
    after: &'a [(String, f64)],
    unseen: Unseen,
}

fn risen(counted: Counted<'_>, into: &mut BTreeMap<String, f64>) -> Result<(), Never> {
    let earlier: BTreeMap<&str, f64> =
        counted.before.iter().map(|(named, amount)| (named.as_str(), *amount)).collect();

    'over_names: for (named, amount) in counted.after {
        let was = match (earlier.get(named.as_str()), counted.unseen) {
            (Some(was), Unseen::Unknown | Unseen::None) => *was,
            (None, Unseen::None) => NONE,
            (None, Unseen::Unknown) => continue 'over_names,
        };
        let since = amount - was;

        match since > NONE {
            true => *into.entry(named.clone()).or_insert(NONE) += since,
            false => {}
        }
    }

    Ok(())
}

fn grew(moments: &[Moment]) -> Result<Vec<(String, f64)>, Never> {
    let mut read = moments.iter().filter(|moment| !moment.held.is_empty());

    let (first, last) = match (read.next(), read.next_back()) {
        (Some(first), Some(last)) => (first, last),
        (None, _) | (_, None) => return Ok(Vec::new()),
    };
    let earlier: BTreeMap<&str, f64> =
        first.held.iter().map(|(named, megabytes)| (named.as_str(), *megabytes)).collect();
    let mut grown: BTreeMap<String, f64> = BTreeMap::new();

    for (named, megabytes) in &last.held {
        let since = match earlier.get(named.as_str()) {
            Some(was) => megabytes - was,
            None => continue,
        };

        match since > NONE {
            true => {
                grown.insert(named.clone(), since);
            }
            false => {}
        }
    }

    largest(grown)
}

pub fn watts(used: &Used) -> Result<Option<f64>, Never> {
    Ok(match (used.flat > NONE, used.watthours > NONE) {
        (true, true) => Some(used.watthours / used.flat),
        (true, false) | (false, true) | (false, false) => None,
    })
}

fn stretch(seconds: f64) -> Result<String, Never> {
    let Ok(hours) = console_core_number_conversion::toward_zero_u64(seconds / HOUR);
    let Ok(minutes) = console_core_number_conversion::toward_zero_u64((seconds / 60.0) % 60.0);

    Ok(format!("{hours}h {minutes:02}m"))
}

fn counts(text: &mut String, heading: &str, counted: &[(String, f64)]) -> Result<(), Never> {
    match counted.is_empty() {
        true => {}
        false => {
            text.push_str(&format!("\n{heading}\n"));

            for (named, times) in counted {
                text.push_str(&format!("  {named:<24}{times:>6.0} times\n"));
            }
        }
    }

    Ok(())
}

pub fn told(used: &Used) -> Result<String, Never> {
    let Ok(awake) = stretch(used.awake);
    let Ok(asleep) = stretch(used.asleep);
    let mut text = format!("{awake} awake, {asleep} asleep\n");
    let Ok(drawn) = watts(used);

    match drawn {
        Some(drawn) => {
            let Ok(flat) = stretch(used.flat * HOUR);

            text.push_str(&format!(
                "{drawn:.1} W awake, {:.1} Wh over {flat} of it\n",
                used.watthours
            ));
        }
        None => text.push_str(
            "never awake off the cable for a whole reading, so nothing says what it draws\n",
        ),
    }

    match used.gpu {
        Some(gpu) => text.push_str(&format!("the graphics drew {gpu:.1} W\n")),
        None => {}
    }

    match used.worst_sleep {
        Some(slept) => {
            let Ok(asleep) = stretch(slept.asleep);
            let Ok(at_most) = slept.at_most();

            text.push_str(&format!(
                "the worst sleep drew at most {at_most:.2} W: {:.1} Wh over {asleep} asleep, woken at {}\n",
                slept.watthours, slept.woke
            ));
        }
        None => {}
    }

    text.push_str("\nwhat spent the time awake\n");

    for (named, seconds) in &used.busy {
        let share = seconds / used.awake * WHOLE;

        text.push_str(&format!("  {named:<24}{share:>6.1} % of a core\n"));
    }

    match used.grew.is_empty() {
        true => {}
        false => {
            text.push_str("\nwhat grew in memory from the first reading to the last\n");

            for (named, megabytes) in &used.grew {
                text.push_str(&format!("  {named:<24}{megabytes:>6.1} MB\n"));
            }
        }
    }

    let Ok(()) = counts(&mut text, "what systemd started again", &used.restarted);
    let Ok(()) = counts(&mut text, "what dumped core", &used.crashed);

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_of_stat_says_the_time_spent_and_the_pages_held() {
        let said = "1212026 (a (b) c) R 1212024 1212024 1212024 0 -1 4194304 134 0 0 0 250 50 0 0 20 0 1 0 \
                    2147753 6438912 512 18446744073709551615 94194824380416";
        let Ok(spent) = spending(said);
        let spent = spent.map(|Spent { named, seconds, megabytes }| (named, seconds, megabytes));

        assert_eq!(spent, Some(("a (b) c".to_string(), 3.0, Some(2.0))));
    }
}
