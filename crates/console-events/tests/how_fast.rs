//! Not a check. A number, for as long as somebody is looking at one.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use console_events::listening::{Heard, listen_at};
use console_events::serving;
use console_events::sources::Held;
use console_program_contract::{Changed, Topic};

fn how_many(named: &str, usual: usize) -> usize {
    match std::env::var(named) {
        Ok(said) => said.parse().unwrap_or(usual),
        Err(_) => usual,
    }
}

static SAYING: OnceLock<Sender<Sender<Changed>>> = OnceLock::new();

static TAKEN: AtomicUsize = AtomicUsize::new(0);

static AGAIN: AtomicUsize = AtomicUsize::new(0);

fn source(topic: &Topic, say: Sender<Changed>) -> Result<Held, console_core_never::Never> {
    Ok(match topic {
        Topic::Sound => match SAYING.get() {
            Some(handing) => match handing.send(say) {
                Ok(()) => Held::Yes,
                Err(_) => Held::Nothing,
            },
            None => Held::Nothing,
        },
        _other => Held::Nothing,
    })
}

fn up(at: &Path) {
    let began = Instant::now();

    while began.elapsed() < Duration::from_secs(5) {
        match at.exists() {
            true => return,
            false => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

#[test]
#[ignore]
fn how_many_words_a_second() {
    let who = how_many("WHO", 8);
    let words = how_many("WORDS", 100_000);
    let at: PathBuf =
        std::env::temp_dir().join(format!("console-events-how-fast-{}.sock", std::process::id()));
    let (handing, handed) = channel();
    let _ = SAYING.set(handing);

    let serving = at.clone();
    let _ = std::thread::spawn(move || serving::serve(&serving, source));

    up(&at);

    let raw = how_many("RAW", 0);

    for _who in 0..who {
        let mine = at.clone();

        let _ = std::thread::spawn(move || {
            match raw {
                0 => {
                    let listening = listen_at(&mine, &[Topic::Sound]).expect("listening");
                    let heard = listening.heard().expect("words");

                    for word in heard {
                        match word {
                            Heard::Said(_) => {
                                let _ = TAKEN.fetch_add(1, Ordering::Relaxed);
                            }
                            Heard::GotIn => {
                                let _ = AGAIN.fetch_add(1, Ordering::Relaxed);
                            },
                        }
                    }
                }
                _bytes_only => {
                    let mut stream =
                        std::os::unix::net::UnixStream::connect(&mine).expect("connecting");
                    let asked = console_events::wire::spelt(&console_events::wire::Says::Listen(
                        Topic::Sound,
                    ))
                    .expect("the wire");
                    let mut asking = stream.try_clone().expect("the connection");

                    std::io::Write::write_all(&mut asking, format!("{asked}\n").as_bytes())
                        .expect("asking");

                    let _ = AGAIN.fetch_add(1, Ordering::Relaxed);

                    let mut buffer = vec![0; 1 << 16];

                    loop {
                        match std::io::Read::read(&mut stream, &mut buffer) {
                            Ok(0) => return,
                            Ok(read) => {
                                let lines =
                                    buffer.get(..read).unwrap_or_default().iter().filter(|byte| **byte == b'\n').count();

                                let _ = TAKEN.fetch_add(lines, Ordering::Relaxed);
                            }
                            Err(_) => return,
                        }
                    }
                }
            }
        });
    }

    let saying = handed.recv_timeout(Duration::from_secs(5)).expect("the source");

    std::thread::sleep(Duration::from_millis(500));

    let wanted = who * words;
    let began = Instant::now();

    for word in 0..words {
        let said = format!("Event 'change' on sink #{word} at 0x7f3a2b4c5d6e volume 0.42");

        saying.send(Changed { about: Topic::Sound, said }).expect("the pool");
    }

    let sent = began.elapsed();

    while TAKEN.load(Ordering::Relaxed) < wanted && began.elapsed() < Duration::from_secs(120) {
        std::thread::sleep(Duration::from_millis(5));
    }

    let took = began.elapsed();
    let taken = TAKEN.load(Ordering::Relaxed);

    println!(
        "{who} listeners, {words} words: handed over in {sent:?}, {taken} of {wanted} \
         lines out in {took:?}, {} subscriptions made -- {:.0} words/s, {:.0} lines/s",
        AGAIN.load(Ordering::Relaxed),
        words as f64 / took.as_secs_f64(),
        taken as f64 / took.as_secs_f64()
    );

    let _ = std::fs::remove_file(&at);
}
