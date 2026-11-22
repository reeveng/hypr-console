use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};

const WORKERS: usize = 4;

fn one_each(arrived: Vec<u32>) -> Vec<JoinHandle<u32>> {
    let mut started = Vec::new();

    for one in arrived {
        //~v WARN EXPLICIT103_NO_THREAD_PER_ITEM
        started.push(thread::spawn(move || one));
    }

    started
}

fn one_each_by_a_word(arrived: Vec<u32>) -> Vec<JoinHandle<u32>> {
    //~v WARN EXPLICIT103_NO_THREAD_PER_ITEM
    arrived.into_iter().map(|one| thread::spawn(move || one)).collect()
}

fn one_each_in_a_scope(arrived: &[u32]) {
    thread::scope(|scope| {
        for one in arrived {
            //~v WARN EXPLICIT103_NO_THREAD_PER_ITEM
            let _ = scope.spawn(move || *one);
        }
    });
}

fn one_worker(arriving: Receiver<u32>) -> JoinHandle<u32> {
    thread::spawn(move || arriving.iter().sum())
}

fn decided_once(arrived: Vec<u32>) -> Option<JoinHandle<u32>> {
    let (handing, arriving) = mpsc::sync_channel(WORKERS);
    let worker = one_worker(arriving);

    for one in arrived {
        let _ = handing.send(one);
    }

    Some(worker)
}

fn main() {
    let _ = one_each(vec![1, 2]);
    let _ = one_each_by_a_word(vec![1, 2]);
    one_each_in_a_scope(&[1, 2]);
    let _ = decided_once(vec![1, 2]);
}
