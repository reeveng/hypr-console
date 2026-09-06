//! The pool over a socket: who is connected, what they asked for, and what to
//! do when a source says something.
//!
//! Split from the program so that it can be started on a socket of a test's
//! own, with sources of a test's own. What that buys is the one thing the unit
//! tests in `pool` cannot ask: that a program which asks for a topic over a
//! real socket is told the last word on it *before* anything changes, which is
//! the whole reason the pool remembers anything.
//!
//! Where the words come from is handed in, the same way
//! `console_controller::turning` is handed a machine. There is no other way to
//! ask what the pool does about a compositor that says something, because the
//! only compositor is the one this is running under.
//!
//! ## A door that cannot open is not a door that is quiet
//!
//! The thread that lets programs in used to read `incoming().flatten()`, which
//! is an iterator that throws every refused `accept` away and asks again. What
//! that costs depends on why the call failed, and there is one reason it never
//! stops failing: `accept` takes the descriptor it is going to need before it
//! does anything else, so a process with none left is refused with `EMFILE`
//! before the queue is even looked at. Nothing about asking again changes that
//! answer, and the loop turns as fast as the machine will turn it, silently, in
//! a thread nobody is watching.
//!
//! So the refusal is waited on rather than dropped. [`BREATH`] is long enough
//! that a door which cannot open costs nothing and short enough that a program
//! waiting to be let in does not notice, and the wait is the whole of it:
//! descriptors come back, the connection that was queued is taken, and the pool
//! carries on. Saying so once rather than every turn is the other half, because
//! a fault that repeats faster than it can be read is a fault nobody reads.
//!
//! There is no test here, and the reason is worth writing down. The refusal
//! needs a full descriptor table at the moment this thread asks again, and a
//! thread already waiting inside `accept` is holding the descriptor it reserved
//! on the way in -- so it takes the next connection however little is left, and
//! the drought a test creates around it is one it cannot feel. Pressing it
//! wants the client in another process, because a test that opens the socket
//! itself is spending the same table it is trying to empty.
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use console_never::Never;
use console_program_contract::{Changed, Topic};

use crate::pool::{Pool, Who};
use crate::sources::Held;
use crate::wire::{self, Says};

const BREATH: Duration = Duration::from_millis(200);

pub type Holding = fn(&Topic, Sender<Changed>) -> Result<Held, Never>;

enum Happened {
    Arrived(UnixStream),
    Asked(Who, Says),
    Left(Who),
    Said(Changed),
}

pub fn serve(socket: &Path, holding: Holding) -> Result<(), String> {

    let at = socket.parent().ok_or("the socket has no directory")?;
    let made = std::fs::create_dir_all(at);

    made.map_err(|fault| format!("{}: {fault}", at.display()))?;

    let gone = std::fs::remove_file(socket);

    match gone {
        Ok(()) => {},
        Err(_) => {},
    }

    let listening = UnixListener::bind(socket)
        .map_err(|fault| format!("{}: {fault}", socket.display()))?;
    let (say, happened) = channel();
    let arriving = say.clone();

    let _ = std::thread::spawn(move || {
        let mut quiet = false;

        for coming in listening.incoming() {
            let stream = match coming {
                Ok(stream) => {
                    quiet = false;

                    stream
                }
                Err(fault) => {
                    match quiet {
                        true => {},
                        false => {
                            eprintln!("console-events: nobody can be let in: {fault}");

                            quiet = true;
                        }
                    }

                    std::thread::sleep(BREATH);

                    continue;
                }
            };

            let told = arriving.send(Happened::Arrived(stream));

            match told {
                Ok(()) => {},
                Err(_) => return,
            }
        }
    });

    let mut pool = Pool::default();
    let mut writing: BTreeMap<Who, UnixStream> = BTreeMap::new();
    let mut held: Vec<Topic> = Vec::new();

    for word in happened {
        match word {
            Happened::Arrived(stream) => {
                let Ok(()) = arrived(&mut pool, &mut writing, stream, &say);
            }
            Happened::Asked(who, says) => {
                let Ok(()) = asked(&mut pool, &mut writing, &mut held, who, says, &say, holding);
            }
            Happened::Left(who) => {
                let Ok(()) = pool.gone(who);

                let _ = writing.remove(&who);
            }
            Happened::Said(changed) => {
                let Ok(()) = said(&mut pool, &mut writing, &changed);
            }
        }
    }

    Ok(())
}

fn arrived(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, UnixStream>,
    stream: UnixStream,
    say: &Sender<Happened>,
) -> Result<(), Never> {
    let reading = match stream.try_clone() {
        Ok(reading) => reading,
        Err(fault) => {
            eprintln!("console-events: a program connected and could not be read: {fault}");

            return Ok(());
        }
    };

    let Ok(who) = pool.joined();
    let _ = writing.insert(who, stream);
    let telling = say.clone();

    let _ = std::thread::spawn(move || {
        for line in BufReader::new(reading).lines().map_while(Result::ok) {
            let Ok(said) = wire::read(&line);

            match said {
                Some(says) => {
                    let told = telling.send(Happened::Asked(who, says));

                    match told {
                        Ok(()) => {},
                        Err(_) => return,
                    }
                }
                None => eprintln!("console-events: {who} said {line:?}, which is nothing"),
            }
        }

        let _ = telling.send(Happened::Left(who));
    });

    Ok(())
}

fn asked(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, UnixStream>,
    held: &mut Vec<Topic>,
    who: Who,
    says: Says,
    say: &Sender<Happened>,
    holding: Holding,
) -> Result<(), Never> {
    match says {
        Says::Listen(topic) => {
            let Ok(replay) = pool.listens(who, topic.clone());
            let Ok(()) = hold(held, &topic, say, holding);

            match replay {
                Some(changed) => {
                    let Ok(()) = tell(pool, writing, who, &changed);
                }
                None => {},
            }
        }
        Says::Deafen(topic) => {
            let Ok(()) = pool.deafen(who, &topic);
        }
        Says::Said(_) => eprintln!("console-events: {who} tried to tell the pool something"),
    }

    Ok(())
}

fn hold(
    held: &mut Vec<Topic>,
    topic: &Topic,
    say: &Sender<Happened>,
    holding: Holding,
) -> Result<(), Never> {
    match held.contains(topic) {
        true => return Ok(()),
        false => {},
    }

    let (said, arriving) = channel();
    let telling = say.clone();

    let Ok(held_now) = holding(topic, said);

    match held_now {
        Held::Yes => {
            held.push(topic.clone());

            let _ = std::thread::spawn(move || {
                for changed in arriving {
                    let told = telling.send(Happened::Said(changed));

                    match told {
                        Ok(()) => {},
                        Err(_) => return,
                    }
                }
            });
        }
        Held::Nothing => {
            let Ok(token) = wire::token(topic);

            eprintln!(
                "console-events: nothing here watches {token}, so anybody listening to it will \
                 hear nothing"
            );
        }
    }

    Ok(())
}

fn said(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, UnixStream>,
    changed: &Changed,
) -> Result<(), Never> {
    let Ok(everybody) = pool.said(changed);

    for who in everybody {
        let Ok(()) = tell(pool, writing, who, changed);
    }

    Ok(())
}

fn tell(
    pool: &mut Pool,
    writing: &mut BTreeMap<Who, UnixStream>,
    who: Who,
    changed: &Changed,
) -> Result<(), Never> {
    let Ok(spelt) = wire::spelt(&Says::Said(changed.clone()));

    let told = match writing.get_mut(&who) {
        Some(stream) => writeln!(stream, "{spelt}"),
        None => return Ok(()),
    };

    match told {
        Ok(()) => {},
        Err(_) => {
            let Ok(()) = pool.gone(who);

            let _ = writing.remove(&who);
        }
    }

    Ok(())
}
