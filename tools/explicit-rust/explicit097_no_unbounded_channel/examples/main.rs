use std::sync::mpsc::{self, Receiver, Sender, SyncSender};

const THE_MOST_WAITING: usize = 64;

fn patient() -> (Sender<u8>, Receiver<u8>) {
    //~v WARN EXPLICIT097_NO_UNBOUNDED_CHANNEL
    mpsc::channel()
}

fn spelled_out() -> (Sender<String>, Receiver<String>) {
    //~v WARN EXPLICIT097_NO_UNBOUNDED_CHANNEL
    std::sync::mpsc::channel::<String>()
}

fn decided() -> (SyncSender<u8>, Receiver<u8>) {
    mpsc::sync_channel(THE_MOST_WAITING)
}

fn channel() -> u8 {
    1
}

fn main() {
    let _ = patient();
    let _ = spelled_out();
    let _ = decided();
    let _ = channel();
}
