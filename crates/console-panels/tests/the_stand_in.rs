//! What a panel does when it has asked someone else to draw it.
//!
//! The socket is between processes and this is not two processes, which is a
//! fair thing to hold against a test. What it is here to hold shut is the half
//! that has nothing to do with drawing: a request that reaches the host says
//! what was asked for, a host that answers means the panel does not draw, and
//! every way of the host not answering means the panel does. That last one is
//! the whole of `a panel with no host is merely slower`, and it is the line
//! most likely to be got wrong, because it is the line no one sees working.
//!
//! The screen is where the rest of it is proved. `console-check` opens the menu
//! against a nested desktop and reads back what it drew, and that check does not
//! know or care which side of this socket drew it -- which is the point.

use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use console_core_never::Never;

use console_panel::handoff::{self, Request, DrawnBy};

fn somewhere(named: &str) -> Result<PathBuf, console_core_temporary_directories::Unmade> {
    let at = console_core_temporary_directories::fresh(&format!("panels-{named}"))?;

    Ok(at.join("host.sock"))
}

fn arguments(words: &[&str]) -> Result<Vec<String>, Never> {
    Ok(words.iter().map(|word| (*word).to_string()).collect())
}

struct Host {
    at: PathBuf,
    heard: std::sync::mpsc::Receiver<Request>,
    serving: Option<JoinHandle<Result<(), std::io::Error>>>,
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

fn a_host(named: &str, answering: Answering) -> Result<Host, Box<dyn Error>> {
    let at = somewhere(named)?;
    let listening = UnixListener::bind(&at)?;
    let (say, heard) = channel();

    let serving = std::thread::spawn(move || {
        let (asking, _) = listening.accept()?;
        let second_hand = asking.try_clone()?;
        let mut reading = BufReader::new(second_hand);
        let mut line = String::new();
        let _ = reading.read_line(&mut line);

        match handoff::read(line.trim_end()) {
            Ok(Some(asked)) => {
                let _ = say.send(asked);
            }
            Ok(None) | Err(_) => {}
        }

        let Ok(()) = answering(&say, asking);

        Ok(())
    });

    Ok(Host { at, heard, serving: Some(serving) })
}

type Answering = fn(&Sender<Request>, std::os::unix::net::UnixStream) -> Result<(), Never>;

fn draws_and_is_closed(_say: &Sender<Request>, mut telling: std::os::unix::net::UnixStream) -> Result<(), Never> {
    let _ = writeln!(telling, "drawn");
    let _ = telling.flush();
    let _ = writeln!(telling, "gone");
    let _ = telling.flush();

    Ok(())
}

fn hangs_up_saying_nothing(_say: &Sender<Request>, telling: std::os::unix::net::UnixStream) -> Result<(), Never> {
    drop(telling);

    Ok(())
}

#[test]
fn a_panel_with_no_host_draws_itself() -> Result<(), Box<dyn Error>> {
    let at = somewhere("no one-is-listening")?;
    let Ok(words) = arguments(&[]);

    assert_eq!(
        handoff::stood_in_at(&at, "launcher", &words),
        Ok(DrawnBy::Here),
        "a host that is down has to be a panel that is slower, not a button that does nothing"
    );

    Ok(())
}

#[test]
fn a_host_that_draws_it_is_a_panel_that_does_not() -> Result<(), Box<dyn Error>> {
    let host = a_host("draws", draws_and_is_closed)?;
    let Ok(words) = arguments(&["--keep"]);

    assert_eq!(
        handoff::stood_in_at(&host.at, "launcher", &words),
        Ok(DrawnBy::ByTheHost),
        "drawn twice is two menus over each other"
    );

    Ok(())
}

#[test]
fn what_was_typed_is_what_the_host_is_asked_for() -> Result<(), Box<dyn Error>> {
    let host = a_host("arguments", draws_and_is_closed)?;
    let Ok(words) = arguments(&["Game Mode"]);

    let _ = handoff::stood_in_at(&host.at, "settings-panel", &words);
    let asked = host.heard.recv().map_err(|_| "the host was told nothing")?;

    assert_eq!(asked.who, "settings-panel");

    let Ok(words) = arguments(&["Game Mode"]);

    assert_eq!(asked.arguments, words, "the tab a bar icon asks for is one word");

    Ok(())
}

#[test]
fn a_host_that_goes_away_before_it_draws_leaves_the_panel_to_draw() -> Result<(), Box<dyn Error>> {
    let host = a_host("hangs-up", hangs_up_saying_nothing)?;
    let Ok(words) = arguments(&[]);

    assert_eq!(
        handoff::stood_in_at(&host.at, "launcher", &words),
        Ok(DrawnBy::Here),
        "a host that died between accepting and drawing is a press that answered nothing"
    );

    Ok(())
}
