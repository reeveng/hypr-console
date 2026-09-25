//! Work that does not depend on itself, spread across the cores.
//!
//! A folder of photographs wants a picture made of each, a list of icons wants
//! each one decoded, and the checks that run on an emulated handheld each want a
//! handheld of their own. None of the pieces reads another's answer, and each of
//! them was done one after another on a machine with eight cores and seven of
//! them idle. [`map`] is the one call for that: a list in, a function over one
//! item, the answers out in the order the items were.
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
//! machine says it has, never more than there are items, and one when the
//! machine will not say. A caller says what the work is and not how wide to run
//! it, so the width of every parallel thing on the device is one function to
//! read and one to change.
//!
//! **A piece that panics is an answer with a piece missing.** The rest are
//! finished and joined, because a scope does not end while anything it started
//! is running, and then the whole call says [`Error::Defect`] rather than handing
//! back a list shorter than the one it was given.
//!
//! This is not where a thread that listens goes. A socket read for as long as a
//! program runs is `console_program_lifetime::threads`, and it is started once
//! and let go of; this is work that ends, and it is waited for.

use std::fmt;
use std::iter::Zip;
use std::ops::RangeFrom;
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

type Queue<'items, T> = Mutex<Zip<Iter<'items, T>, RangeFrom<u32>>>;

pub fn map<T, R, F>(items: &[T], each: F) -> Result<Vec<R>, Error>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let Ok(many) = fitted::<_, u32>(items.len());
    let Ok(cores) = cores();
    let workers = many.min(cores);
    let queue: Queue<'_, T> = Mutex::new(items.iter().zip(0_u32..));
    let (queue, each) = (&queue, &each);

    let finished = thread::scope(|scope| {
        let started: Vec<_> = (0..workers).map(|_| scope.spawn(move || worked(queue, each))).collect();

        started.into_iter().map(|worker| worker.join()).collect::<Vec<_>>()
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

fn cores() -> Result<u32, Never> {
    match thread::available_parallelism() {
        Ok(cores) => fitted::<_, u32>(cores.get()),
        Err(_the_machine_will_not_say) => Ok(1),
    }
}

fn worked<T, R, F>(queue: &Queue<'_, T>, each: &F) -> Result<Vec<(u32, R)>, Never>
where
    F: Fn(&T) -> R,
{
    let mut done = Vec::new();

    loop {
        let next = match queue.lock() {
            Ok(mut waiting) => waiting.next(),
            Err(_another_piece_panicked) => None,
        };

        match next {
            Some((item, at)) => done.push((at, each(item))),
            None => return Ok(done),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::time::Duration;

    #[test]
    fn the_answers_come_back_in_the_order_the_items_went_in() {
        let items: Vec<u32> = (0..500).collect();

        let answered = map(&items, |item| item * 2).expect("every piece answers");

        assert_eq!(answered, items.iter().map(|item| item * 2).collect::<Vec<u32>>());
    }

    #[test]
    fn nothing_to_do_is_nothing_done() {
        let items: Vec<u32> = Vec::new();

        assert_eq!(map(&items, |item| *item), Ok(Vec::new()));
    }

    #[test]
    fn the_pieces_run_on_more_than_one_thread_where_there_is_more_than_one_core() {
        let Ok(cores) = cores();
        let items: Vec<u32> = (0..8).collect();

        let ran_on = map(&items, |_| {
            std::thread::sleep(Duration::from_millis(50));

            format!("{:?}", std::thread::current().id())
        })
        .expect("every piece answers");

        let threads: BTreeSet<String> = ran_on.into_iter().collect();

        let Ok(used) = fitted::<_, u32>(threads.len());

        assert_eq!(used, cores.min(8), "one thread per core, and every one of them used");
    }

    #[test]
    fn a_piece_that_panics_is_a_defect_rather_than_a_short_answer() {
        let items: Vec<u32> = (0..16).collect();

        let answered = map(&items, |item| match *item {
            7 => panic!("the seventh piece"),
            other => other,
        });

        assert_eq!(answered, Err(Error::Defect));
    }
}
