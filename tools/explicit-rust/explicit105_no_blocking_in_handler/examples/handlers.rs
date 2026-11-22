use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::thread::sleep as wait;
use std::time::Duration;

use console_actor::{Actor, Completions};
use console_core_state_machine::{Machine, Never, Queue};

const WAIT: Duration = Duration::from_millis(1);

pub struct Service;

pub struct Request {
    events: Receiver<()>,
    actor: Actor<Service>,
    completions: Completions<u32, ()>,
}

impl Machine for Service {
    type Input = ();
    type State = ();
    type Request = Box<Request>;
    type Effect = ();

    fn initialize(_: &(), _: Option<()>, _: &mut Queue<()>) -> Result<(), Never> {
        startup();
        Ok(())
    }

    fn handle(_: (), mut request: Self::Request, _: &mut Queue<()>) -> Result<(), Never> {
        wait(WAIT);
        let _ = request.events.recv_timeout(WAIT);
        let _ = Receiver::recv_timeout(&request.events, WAIT);
        let _ = request.events.try_recv();
        let mutex = Mutex::new(());
        let _guard = mutex.lock();
        let _ = request.actor.get();
        let _ = request.completions.recv_timeout(WAIT);
        let _ = request.completions.try_recv();
        let _deferred = || wait(WAIT);
        query();
        let _deferred_helper = || startup();

        Ok(())
    }
}

fn startup() {
    read(0);
}

fn read(retry: u8) {
    if retry > 0 {
        read(retry - 1);
    }
    let _ = std::fs::read_to_string("settings");
}

fn query() {
    let _ = std::process::Command::new("example").output();
}

impl Service {
    pub fn handle(events: &Receiver<()>) {
        let _ = events.recv_timeout(WAIT);
    }
}

fn main() {}
