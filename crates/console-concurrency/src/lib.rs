//! Work that does not depend on itself, spread across the cores.
//!
//! A folder of photographs wants a picture made of each, a list of icons wants
//! each one decoded, and the checks that run on an emulated handheld each want a
//! handheld of their own. None of the pieces reads another's answer, and each of
//! them was done one after another on a machine with eight cores and seven of
//! them idle. [`map`] is the one call for that: the cores, a list in, a function
//! over one item, the answers out in the order the items were.
//!
//! **It is rayon's shape and not rayon.** `par_iter().map().collect()` is the
//! right idea and a work-stealing pool is more than a list of independent pieces
//! needs: the workers here take the next item from one shared queue until it is
//! empty, so a slow item holds up one core rather than a share of the list
//! decided in advance. The queue is a lock around an iterator, which is a lock
//! taken once per item, and every item worth handing to this costs a program
//! run or a decode -- milliseconds against the nanoseconds of the lock.
//!
//! **How many threads is decided here and nowhere else.** One per core the
//! machine has, never more than there are items, and one when the machine will
//! not say. A caller says what the work is and not how wide to run it, so the
//! width of every parallel thing on the device is one function to read and one
//! to change.
//!
//! **The cores are counted once and handed in.** Counting them reads a file
//! under `/sys` for every thread the machine has, which took 0.23 ms on an
//! 8840U: nothing beside a folder of thumbnails, and a tenth of a round of a
//! photograph that is decoded a band at a time and spread once a round. A `map`
//! that counted for itself counted every round, and that was 6 ms of a lossy
//! WebP's 0.060 s. So [`Cores::counted`] is asked where the work begins and
//! what it says is handed to every [`map`] after it. It is not a `static` the
//! first call fills, which is the memo EXPLICIT044 refuses: a width frozen for
//! the life of the process, and a test with nowhere to stand to ask for
//! another one.
//!
//! **A core is counted once, not once for each thread it runs.** The handheld's
//! eight cores each run two threads at once, and the machine says sixteen. The
//! second thread on a core shares the first one's caches and units, and work
//! that already keeps a core busy gains next to nothing from it: a folder of
//! thirty-two photographs to thumbnails took 1.02 s on sixteen threads and
//! 1.08 s on eight, and 13.4 s of CPU against 8.5 s. So the count is the cores
//! `/sys/devices/system/cpu` lists, and the machine's number where that cannot
//! be read or is smaller, as it is for a process held to fewer.
//!
//! **The thread that asked is one of them.** It would only wait for the others
//! otherwise, so it takes from the queue like they do, and one fewer thread is
//! started. A list of one is done where it was asked for, with no thread
//! started at all, and so is any list on a machine of one core.
//!
//! **A piece that panics is an answer with a piece missing.** The rest are
//! finished and joined, because a scope does not end while anything it started
//! is running, and then the whole call says [`Error::Defect`] rather than handing
//! back a list shorter than the one it was given.
//!
//! This is not where a thread that listens goes. A socket read for as long as a
//! program runs is `console_program_lifetime::threads`, and it is started once
//! and let go of; this is work that ends, and it is waited for.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::iter::{self, Zip};
use std::ops::RangeFrom;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::slice::Iter;
use std::sync::Mutex;
use std::thread;

use console_core_never::Never;
use console_core_number_conversion::fitted;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Defect,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Defect => write!(formatter, "a piece of the work panicked, so the answer is missing a piece"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cores {
    counted: u32,
}

impl Cores {
    pub fn counted() -> Result<Cores, Never> {
        let Ok(threads) = threads();
        let Ok(cores) = counted_cores(Path::new(CPUS));

        Ok(Cores {
            counted: match cores {
                0 => threads,
                cores => cores.min(threads),
            },
        })
    }
}

type Queue<'items, T> = Mutex<Zip<Iter<'items, T>, RangeFrom<u32>>>;

pub fn map<T, R, F>(cores: Cores, items: &[T], each: F) -> Result<Vec<R>, Error>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let Ok(many) = fitted::<_, u32>(items.len());
    let workers = many.min(cores.counted);
    let queue: Queue<'_, T> = Mutex::new(items.iter().zip(0_u32..));
    let (queue, each) = (&queue, &each);

    let finished = thread::scope(|scope| {
        let started: Vec<_> = (1..workers).map(|_| scope.spawn(move || drain_queue(queue, each))).collect();
        let here = catch_unwind(AssertUnwindSafe(|| drain_queue(queue, each)));

        iter::once(here).chain(started.into_iter().map(|worker| worker.join())).collect::<Vec<_>>()
    });

    let mut answered = Vec::new();

    for worker in finished {
        match worker {
            Ok(Ok(done)) => answered.extend(done),
            Ok(Err(never)) => match never {},
            Err(_a_piece_panicked) => return Err(Error::Defect),
        }
    }

    answered.sort_by_key(|(at, _)| *at);

    Ok(answered.into_iter().map(|(_, answer)| answer).collect())
}

const CPUS: &str = "/sys/devices/system/cpu";

fn threads() -> Result<u32, Never> {
    match thread::available_parallelism() {
        Ok(threads) => fitted::<_, u32>(threads.get()),
        Err(_the_machine_will_not_say) => Ok(1),
    }
}

fn counted_cores(cpus: &Path) -> Result<u32, Never> {
    let listed = match fs::read_dir(cpus) {
        Ok(listed) => listed,
        Err(_no_topology_here) => return Ok(0),
    };

    let mut cores = BTreeSet::new();

    for cpu in listed.flatten() {
        match fs::read_to_string(cpu.path().join("topology").join("core_cpus_list")) {
            Ok(threads) => {
                cores.insert(threads);
            },
            Err(_not_a_cpu) => {},
        }
    }

    fitted::<_, u32>(cores.len())
}

fn drain_queue<T, R, F>(queue: &Queue<'_, T>, each: &F) -> Result<Vec<(u32, R)>, Never>
where
    F: Fn(&T) -> R,
{
    Ok(iter::from_fn(|| match queue.lock() {
        Ok(mut waiting) => waiting.next(),
        Err(_another_piece_panicked) => None,
    })
    .map(|(item, at)| (at, each(item)))
    .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_waiting::{Ready, Schedule, until};
    use std::collections::BTreeSet;
    use std::time::Duration;

    type Failure = Box<dyn std::error::Error>;

    #[test]
    fn the_answers_come_back_in_the_order_the_items_went_in() -> Result<(), Failure> {
        let items: Vec<u32> = (0..500).collect();
        let Ok(cores) = Cores::counted();

        let answered = map(cores, &items, |item| item.saturating_mul(2))?;

        assert_eq!(answered, items.iter().map(|item| item.saturating_mul(2)).collect::<Vec<u32>>());

        Ok(())
    }

    #[test]
    fn nothing_to_do_is_nothing_done() {
        let items: Vec<u32> = Vec::new();
        let Ok(cores) = Cores::counted();

        assert_eq!(map(cores, &items, |item| *item), Ok(Vec::new()));
    }

    #[test]
    fn the_pieces_run_on_more_than_one_thread_where_there_is_more_than_one_core() -> Result<(), Failure> {
        let Ok(cores) = Cores::counted();
        let used = threads_used(cores)?;

        assert_eq!(used, cores.counted.min(8), "one thread per core, and every one of them used");

        Ok(())
    }

    #[test]
    fn the_width_is_the_count_it_was_handed_rather_than_one_it_takes_again() -> Result<(), Failure> {
        let two = threads_used(Cores { counted: 2 })?;
        let one = threads_used(Cores { counted: 1 })?;

        assert_eq!(two, 2);
        assert_eq!(one, 1, "which is the thread that asked, and no other");

        Ok(())
    }

    fn threads_used(cores: Cores) -> Result<u32, Failure> {
        let items: Vec<u32> = (0..8).collect();
        let Ok(a_while) = Schedule::of(Duration::from_millis(50));

        let ran_on = map(cores, &items, |_| {
            let Ok(_held_long_enough_for_the_others_to_start) = until(a_while, || Ok(Ready::NotYet));

            format!("{:?}", std::thread::current().id())
        })?;

        let threads: BTreeSet<String> = ran_on.into_iter().collect();
        let Ok(used) = fitted::<_, u32>(threads.len());

        Ok(used)
    }

    #[test]
    #[cfg_attr(
        dylint_lib = "explicit040_no_torn_write",
        allow(explicit040_no_torn_write, reason = "a topology made up in a directory of this test's own, which nothing reads while it is written")
    )]
    fn a_core_that_runs_two_threads_is_counted_once() -> Result<(), Failure> {
        let cpus = console_core_temporary_directories::fresh("concurrency-cores")?;

        for (cpu, siblings) in [("cpu0", "0-1"), ("cpu1", "0-1"), ("cpu2", "2-3"), ("cpu3", "2-3"), ("cpu4", "4")] {
            let topology = cpus.join(cpu).join("topology");

            fs::create_dir_all(&topology)?;
            fs::write(topology.join("core_cpus_list"), siblings)?;
        }

        fs::create_dir_all(cpus.join("cpufreq"))?;

        assert_eq!(counted_cores(&cpus), Ok(3));
        assert_eq!(counted_cores(&cpus.join("not-there")), Ok(0));

        Ok(())
    }

    #[test]
    fn a_piece_that_panics_is_a_defect_rather_than_a_short_answer() {
        let items: Vec<u32> = (0..16).collect();
        let Ok(cores) = Cores::counted();

        assert_eq!(map(cores, &items, seventh_panics), Err(Error::Defect));
        assert_eq!(map(cores, &[7], seventh_panics), Err(Error::Defect), "and so is one done by the thread that asked");
    }

    fn seventh_panics(item: &u32) -> Result<u32, Never> {
        Ok(match *item {
            #[cfg_attr(
                dylint_lib = "explicit004_no_panic",
                allow(explicit004_no_panic, reason = "a piece that panics is what this test is about, and a panic is the only thing that presses it")
            )]
            7 => panic!("the seventh piece"),
            other => other,
        })
    }
}
