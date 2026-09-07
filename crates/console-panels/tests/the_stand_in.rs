//! What a panel does when it has asked somebody else to draw it.
//!
//! The socket is between processes and this is not two processes, which is a
//! fair thing to hold against a test. What it is here to hold shut is the half
//! that has nothing to do with drawing: a request that reaches the host says
//! what was asked for, a host that answers means the panel does not draw, and
//! every way of the host not answering means the panel does. That last one is
//! the whole of `a panel with no host is merely slower`, and it is the line
//! most likely to be got wrong, because it is the line nobody sees working.
//!
//! The screen is where the rest of it is proved. `console-check` opens the menu
//! against a nested desktop and reads back what it drew, and that check does not
//! know or care which side of this socket drew it -- which is the point.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use console_panel::held::{self, Asked, Drawn};

fn somewhere(named: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("console-panels-{}-{named}", std::process::id()));
    let _ = std::fs::remove_file(&at);

    at
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

struct Host {
    at: PathBuf,
    heard: std::sync::mpsc::Receiver<Asked>,
    serving: Option<JoinHandle<()>>,
}

impl Drop for Host {
    fn drop(&mut self) {
        match self.serving.take() {
            Some(serving) => {
                let _ = serving.join();
            }
            None => {}
        }

        let _ = std::fs::remove_file(&self.at);
    }
}

fn a_host(named: &str, answering: Answering) -> Host {
    let at = somewhere(named);
    let listening = UnixListener::bind(&at).expect("nowhere to listen");
    let (say, heard) = channel();

    let serving = std::thread::spawn(move || {
        let (asking, _) = listening.accept().expect("nobody asked");
        let mut reading = BufReader::new(asking.try_clone().expect("no second hand on the socket"));
        let mut line = String::new();
        let _ = reading.read_line(&mut line);

        match held::read(line.trim_end()) {
            Ok(Some(asked)) => {
                let _ = say.send(asked);
            }
            Ok(None) | Err(_) => {}
        }

        answering(&say, asking);
    });

    Host { at, heard, serving: Some(serving) }
}

type Answering = fn(&Sender<Asked>, std::os::unix::net::UnixStream);

fn draws_and_is_closed(_say: &Sender<Asked>, mut telling: std::os::unix::net::UnixStream) {
    let _ = writeln!(telling, "drawn");
    let _ = telling.flush();
    let _ = writeln!(telling, "gone");
    let _ = telling.flush();
}

fn hangs_up_saying_nothing(_say: &Sender<Asked>, telling: std::os::unix::net::UnixStream) {
    drop(telling);
}

#[test]
fn a_panel_with_no_host_draws_itself() {
    let at = somewhere("nobody-is-listening");

    assert_eq!(
        held::stood_in_at(&at, "launcher", &argv(&[])),
        Ok(Drawn::Here),
        "a host that is down has to be a panel that is slower, not a button that does nothing"
    );
}

#[test]
fn a_host_that_draws_it_is_a_panel_that_does_not() {
    let host = a_host("draws", draws_and_is_closed);

    assert_eq!(
        held::stood_in_at(&host.at, "launcher", &argv(&["--keep"])),
        Ok(Drawn::ByTheHost),
        "drawn twice is two menus over each other"
    );
}

#[test]
fn what_was_typed_is_what_the_host_is_asked_for() {
    let host = a_host("argv", draws_and_is_closed);
    let _ = held::stood_in_at(&host.at, "settings-panel", &argv(&["Game Mode"]));

    let asked = host.heard.recv().expect("the host was told nothing");

    assert_eq!(asked.who, "settings-panel");
    assert_eq!(asked.argv, argv(&["Game Mode"]), "the tab a bar icon asks for is one word");
}

#[test]
fn a_host_that_goes_away_before_it_draws_leaves_the_panel_to_draw() {
    let host = a_host("hangs-up", hangs_up_saying_nothing);

    assert_eq!(
        held::stood_in_at(&host.at, "launcher", &argv(&[])),
        Ok(Drawn::Here),
        "a host that died between accepting and drawing is a press that answered nothing"
    );
}
