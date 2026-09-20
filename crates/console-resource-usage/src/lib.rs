//! What the machine spent while nobody was watching, and what spent it.
//!
//! The device's own idle draw took a person sitting over an ssh session with
//! two sixty-second windows to find, and what it found -- an audio stack awake
//! on a machine with nothing playing -- was true for months before anybody
//! measured it. A measurement somebody has to be present for is a measurement
//! of the moment they were present. So this is the same two readings taken
//! while nobody is there: a line every few minutes, kept, and read afterwards.
//!
//! ## What one looks like
//!
//! ```text
//! {"at":1758240300,"up":67932.4,"slept":10423.0,"energy":41.230,"draw":9.12,
//!  "gpu":3.10,"percent":88,
//!  "busy":{"wireplumber":1234.5,"pipewire":701.2,"kew":688.0}}
//! ```
//!
//! Every number on a line is a **counter, not a rate**: what is left in the
//! battery, seconds of CPU since a program started, seconds since the machine
//! came up.
//! A rate is a question about two moments and nothing here is entitled to
//! answer it -- a snapshot that wrote watts would be writing an average over a
//! window it chose, and the window somebody wants is the one they ask about
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
//! ## What spent it, by name rather than by number
//!
//! A pid is a number that means a different program next week, and a report
//! that names one is a report nobody can read. So CPU time is summed by the
//! name in `/proc/pid/stat` -- every `wireplumber` on the machine is one line
//! -- and a restart shows up as a counter going backwards, which [`between`]
//! drops rather than counts as time spent. `NAMED` of them, which is the top
//! of the list by what each has spent since it started: a program that has
//! never used a second of CPU is not the one draining anything.
//!
//! `TICK` is USER_HZ, which the kernel fixes at 100 for every architecture this
//! desktop is built for. `sysconf` would answer it at the cost of the one
//! unsafe call in this crate, and a wrong answer here scales every reading by
//! the same constant rather than changing which name is at the top.
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
use console_core_never::Never;
use console_core_number_conversion::Float;
use console_default_applications::battery::{BATTERY, Charge, SUPPLIES, charge};

const UPTIME: &str = "/proc/uptime";

const SLEPT: &str = "/sys/power/suspend_stats/total_hw_sleep";

const SENSORS: &str = "/sys/class/hwmon";

const AMDGPU: &str = "amdgpu";

const DRAWN: &str = "power1_average";

const RUNNING: &str = "/proc";

const STORE: &str = "used.jsonl";

const MILLION: f64 = 1_000_000.0;

const TICK: f64 = 100.0;

const HOUR: f64 = 3_600.0;

const WHOLE: f64 = 100.0;

const NONE: f64 = 0.0;

const NAMED: usize = 20;

const FIELDS_BEFORE_UTIME: usize = 11;


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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Used {
    pub awake: f64,
    pub asleep: f64,
    pub watthours: f64,
    pub flat: f64,
    pub gpu: Option<f64>,
    pub busy: Vec<(String, f64)>,
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

fn counted(said: &str) -> Result<Option<f64>, Never> {
    Ok(match said.trim().parse::<f64>() {
        Ok(number) => Some(number),
        Err(_not_a_number) => None,
    })
}

fn number(at: &Path) -> Result<Option<f64>, Never> {
    let Ok(held) = read(at);
    let Ok(said) = held.said();

    match said {
        Some(said) => counted(&said),
        None => Ok(None),
    }
}

fn up() -> Result<Option<f64>, Never> {
    let Ok(held) = read(Path::new(UPTIME));
    let Ok(said) = held.said();

    let first = said.as_deref().map(str::split_whitespace).and_then(|mut words| words.next());

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
        let Ok(named) = held.said();

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

fn spending(said: &str) -> Result<Option<(String, f64)>, Never> {
    let ends = match said.rfind(')') {
        Some(ends) => ends,
        None => return Ok(None),
    };
    let starts = match said.find('(') {
        Some(starts) => starts,
        None => return Ok(None),
    };

    let named = match said.get(starts.saturating_add(1)..ends) {
        Some(named) => named.to_string(),
        None => return Ok(None),
    };
    let rest = match said.get(ends.saturating_add(1)..) {
        Some(rest) => rest,
        None => return Ok(None),
    };

    let mut fields = rest.split_whitespace().skip(FIELDS_BEFORE_UTIME);
    let mine = fields.next().map(str::parse::<u64>);
    let system = fields.next().map(str::parse::<u64>);

    Ok(match (mine, system) {
        (Some(Ok(mine)), Some(Ok(system))) => {
            let Ok(ticks) = mine.saturating_add(system).float();

            Some((named, ticks / TICK))
        }
        (Some(Err(_)), _) | (_, Some(Err(_))) | (None, _) | (_, None) => None,
    })
}

fn busy() -> Result<Vec<(String, f64)>, Never> {
    let running = match std::fs::read_dir(RUNNING) {
        Ok(running) => running,
        Err(_nothing_to_ask) => return Ok(Vec::new()),
    };
    let mut spent: BTreeMap<String, f64> = BTreeMap::new();

    for process in running.flatten().map(|process| process.path()) {
        let numbered = process
            .file_name()
            .and_then(|named| named.to_str())
            .map(|named| named.parse::<u32>());

        match numbered {
            Some(Ok(_a_process)) => {}
            Some(Err(_)) | None => continue,
        }

        let Ok(held) = read(&process.join("stat"));
        let Ok(said) = held.said();

        let said = match said {
            Some(said) => said,
            None => continue,
        };
        let Ok(spending) = spending(&said);

        let (named, seconds) = match spending {
            Some(spending) => spending,
            None => continue,
        };
        let held = spent.entry(named).or_insert(NONE);

        *held += seconds;
    }

    let mut gathered: Vec<(String, f64)> = spent.into_iter().collect();

    gathered.sort_by(|one, another| another.1.total_cmp(&one.1));
    gathered.truncate(NAMED);

    Ok(gathered)
}

fn battery() -> Result<Option<PathBuf>, Never> {
    let supplies = match std::fs::read_dir(SUPPLIES) {
        Ok(supplies) => supplies,
        Err(_nothing_to_ask) => return Ok(None),
    };

    for supply in supplies.flatten().map(|supply| supply.path()) {
        let Ok(held) = read(&supply.join("type"));
        let Ok(kind) = held.said();

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
    let Ok(said) = charge();
    let Ok(charged) = Charge::of(&said);
    let Ok(busy) = busy();

    Ok(Moment { at, up, slept, energy, draw, gpu, percent: charged.percent, busy })
}

fn quoted(said: &str) -> Result<String, Never> {
    Ok(serde_json::Value::String(said.to_string()).to_string())
}

pub fn written(moment: &Moment) -> Result<String, Never> {
    let mut said = format!("{{\"at\":{}", moment.at);

    match moment.up {
        Some(up) => said.push_str(&format!(",\"up\":{up:.1}")),
        None => {}
    }

    said.push_str(&format!(",\"slept\":{:.1}", moment.slept));

    match moment.energy {
        Some(energy) => said.push_str(&format!(",\"energy\":{energy:.3}")),
        None => {}
    }

    match moment.draw {
        Some(draw) => said.push_str(&format!(",\"draw\":{draw:.2}")),
        None => {}
    }

    match moment.gpu {
        Some(gpu) => said.push_str(&format!(",\"gpu\":{gpu:.2}")),
        None => {}
    }

    match moment.percent {
        Some(percent) => said.push_str(&format!(",\"percent\":{percent}")),
        None => {}
    }

    said.push_str(",\"busy\":{");

    let mut first = true;

    for (named, seconds) in &moment.busy {
        match first {
            true => {}
            false => said.push(','),
        }

        first = false;

        let Ok(named) = quoted(named);

        said.push_str(&format!("{named}:{seconds:.1}"));
    }

    said.push_str("}}");

    Ok(said)
}

pub fn of(said: &str) -> Result<Option<Moment>, Never> {
    let held: serde_json::Value = match serde_json::from_str(said) {
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
    let busy = match object.get("busy").and_then(serde_json::Value::as_object) {
        Some(busy) => busy
            .iter()
            .filter_map(|(named, seconds)| {
                seconds.as_f64().map(|seconds| (named.to_string(), seconds))
            })
            .collect(),
        None => Vec::new(),
    };
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

    let Ok(said) = written(moment);

    file.write_all(format!("{said}\n").as_bytes())
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
    };
    let mut drawn = (NONE, NONE);
    let mut busy: BTreeMap<String, f64> = BTreeMap::new();

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

        match (before.energy, after.energy) {
            (Some(was), Some(is)) => match was > is {
                true => {
                    used.watthours += was - is;
                    used.flat += wall / HOUR;
                }
                false => {}
            },
            (None, _) | (_, None) => {}
        }

        match after.gpu {
            Some(watts) => drawn = (drawn.0 + watts, drawn.1 + 1.0),
            None => {}
        }

        let earlier: BTreeMap<&str, f64> =
            before.busy.iter().map(|(named, seconds)| (named.as_str(), *seconds)).collect();

        'over_engines: for (named, seconds) in &after.busy {
            let was = match earlier.get(named.as_str()) {
                Some(was) => *was,
                None => continue 'over_engines,
            };
            let since = seconds - was;

            match since > NONE {
                true => {
                    let held = busy.entry(named.clone()).or_insert(NONE);

                    *held += since;
                }
                false => {}
            }
        }
    }

    match used.awake > NONE {
        true => {}
        false => return Ok(None),
    }

    used.gpu = match drawn.1 > NONE {
        true => Some(drawn.0 / drawn.1),
        false => None,
    };

    let mut gathered: Vec<(String, f64)> = busy.into_iter().collect();

    gathered.sort_by(|one, another| another.1.total_cmp(&one.1));
    gathered.truncate(NAMED);

    used.busy = gathered;

    Ok(Some(used))
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

pub fn told(used: &Used) -> Result<String, Never> {
    let Ok(awake) = stretch(used.awake);
    let Ok(asleep) = stretch(used.asleep);
    let mut said = format!("{awake} awake, {asleep} asleep\n");
    let Ok(drawn) = watts(used);

    match drawn {
        Some(drawn) => {
            let Ok(flat) = stretch(used.flat * HOUR);

            said.push_str(&format!(
                "{drawn:.1} W off the battery, {:.1} Wh over {flat}\n",
                used.watthours
            ));
        }
        None => said.push_str("on the cable the whole time, so nothing says what it draws\n"),
    }

    match used.gpu {
        Some(gpu) => said.push_str(&format!("the graphics drew {gpu:.1} W\n")),
        None => {}
    }

    said.push_str("\nwhat spent the time awake\n");

    for (named, seconds) in &used.busy {
        let share = seconds / used.awake * WHOLE;

        said.push_str(&format!("  {named:<24}{share:>6.1} % of a core\n"));
    }

    Ok(said)
}
