//! The thing that answers when anything on this machine raises a notification.
//!
//!     console-notify
//!
//! It takes `org.freedesktop.Notifications` and draws the card, which mako did
//! before it and GTK did after that. What decides anything is
//! `console_notifications::serving`, which has never heard of a socket or a
//! screen; what is here is the three machines that module was written to be
//! kept away from -- a bus connection, a layer surface, and a clock.
//!
//! **It draws itself now, and that is the whole of this file's history.** The
//! card was a GTK box of labels under a stylesheet, and a stylesheet is a second
//! copy of numbers `showing` already held. Everything that decided anything
//! about the card -- how wide, how far in, how far down a reading fills --
//! was in this repository twice, once as arithmetic nothing could draw with and
//! once as CSS nothing could check. Now `showing::cards` answers in shapes,
//! `console-draw-painting` puts them in a buffer and `console-draw-surface`
//! hands the buffer to the compositor, and `sheet()` is gone.
//!
//! **One loop, and the bus read is the only thing on a thread.** GTK draws on
//! one thread and will not be driven from somebody else's, which is why this
//! used to be a glib main loop with the state in a `RefCell` and the reading
//! pushed through `spawn_blocking`. With the surface ours the loop is ours, so
//! what is left on a thread is the one thing that genuinely blocks: `heard`
//! waits for a whole message and a poll on the socket cannot promise one has
//! arrived. It reads, puts what it heard behind a lock, and writes a byte down
//! a pipe the loop is already watching -- which is what glib was doing, with
//! the wakeup visible instead of buried.
//!
//! **A length that runs out is a timer, and it is the one wait on this machine
//! that is honestly a duration.** Everything else here asks for the thing it is
//! waiting for; a notification that says it lasts five seconds is asking for
//! five seconds to pass, and there is nothing else to ask. What is armed is
//! kept with the raise it was armed for, so a card replaced a moment before its
//! seconds run out is not taken down by the old card's clock -- which is what a
//! rocker held down would do to itself twenty times a second.
//!
//! **A frame is drawn when what it would show has changed, and not otherwise.**
//! The loop drew on every pass, and a commit is answered by the compositor
//! releasing the buffer it was handed -- which is an event, which wakes the
//! poll, which draws again. A card standing on the screen with nothing
//! happening to it held a whole core that way, and the one the battery leaves
//! up held it until somebody touched the card. What was last drawn is kept and
//! compared: the stack, and the size and scale the compositor last said, so a
//! configure or a scale arriving late still redraws while a card that has not
//! moved does not.
//!
//! **A card is taken down by touching it.** It is the only thing on this
//! desktop a person can reach without opening anything, and it was mako's
//! `on-touch=dismiss` before it was ours: a card that stands over what somebody
//! is doing and cannot be got rid of is worse than one that never came. The hit
//! test is `Panel::covers` over the same stack that was drawn, so what a thumb
//! lands on is decided by the arithmetic rather than by a second opinion about
//! where the cards ended up.
//!
//! **The pipe refuses to block at both ends, which it did not.** A loop woken
//! by either the compositor's socket or the pipe drained the pipe on every
//! pass, so a wake that was the compositor's read a pipe nobody had written
//! to and waited there for a byte that was not coming -- a daemon that stops
//! answering the first time two things happen in the wrong order. It is
//! `console_waiting::woken` now, where the reason lives beside the `unsafe`
//! and the bar is the second caller.
//!
//! **The file is written every time what is held changes.** The panel and the
//! bell read that rather than asking here, for the reason the strip under the
//! bar reads a file: the bell reads on every notification and a reading that
//! costs a subprocess is a reading it cannot take that often.
//!
//! **There is no GPU behind this and now nothing wants one.** The unit gives it
//! a private `/dev`, so there is no render node; GTK found neither Vulkan nor
//! hardware GL and came out on Mesa's software EGL, which asks `mincore`
//! whether a pointer is mapped, which `@system-service` allows no syscall group
//! for, so every attempt to draw a card was a SIGSYS. `GDK_DISABLE` was the
//! answer and it was an answer to a question this file no longer asks: cairo
//! over a shared buffer is all there ever was behind the card, and it is what
//! is written here.

use std::collections::VecDeque;
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_bus::messages::Message;
use console_bus::talking::{Bus, Got, Hearing, Saying};
use console_core_colour::spent::{beside, read};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_draw_painting::{Font, Frame, Run, measured, onto};
use console_draw_surface::standing::{
    Anchor, Gone as Closed, Keyboard, Margin, Room, Under, Wanted,
};
use console_draw_surface::scale::Scale;
use console_draw_surface::{Missing, Poke, Surface};
use console_notifications::reading::written;
use console_notifications::saying::Expiry;
use console_notifications::serving::{
    self, Armed, Changed, Gone, Held, Holding, Turn, Why, going,
};
use console_notifications::showing::{self, Measured, Saying as Said, Stack, Wearing};

#[derive(Debug)]
enum Cannot {
    Wire(console_bus::talking::Wire),
    Taken,
    Lost(std::io::Error),
    NoPalette { at: std::path::PathBuf, why: std::io::Error },
    Undressed(showing::Undressed),
    Pipe(std::io::Error),
    Compositor(Missing),
}

impl std::fmt::Display for Cannot {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cannot::Wire(why) => write!(to, "{why}"),
            Cannot::Taken => write!(
                to,
                "something else holds {}, so nothing here would ever be asked",
                serving::NOTICES
            ),
            Cannot::Lost(why) => write!(to, "this program cannot find itself: {why}"),
            Cannot::NoPalette { at, why } => {
                write!(to, "no palette at {}: {why}", at.display())
            }
            Cannot::Undressed(why) => write!(to, "{why}"),
            Cannot::Pipe(why) => write!(to, "nothing to be woken down: {why}"),
            Cannot::Compositor(why) => write!(to, "{why}"),
        }
    }
}

impl std::error::Error for Cannot {}

impl From<console_bus::talking::Wire> for Cannot {
    fn from(why: console_bus::talking::Wire) -> Cannot {
        Cannot::Wire(why)
    }
}

impl From<showing::Undressed> for Cannot {
    fn from(why: showing::Undressed) -> Cannot {
        Cannot::Undressed(why)
    }
}

impl From<Missing> for Cannot {
    fn from(why: Missing) -> Cannot {
        Cannot::Compositor(why)
    }
}

fn main() -> ExitCode {
    match answering() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("console-notify: {why}");

            ExitCode::FAILURE
        }
    }
}

fn answering() -> Result<(), Cannot> {
    let mut bus = Bus::session()?;
    let got = bus.taking(serving::NOTICES)?;

    match got {
        Got::Ours | Got::Already => {}
        Got::Queued | Got::Taken => return Err(Cannot::Taken),
    }

    let wearing = dressed()?;
    let mut surface = Surface::connect()?;
    let Ok((hearing, saying)) = bus.apart();
    let queue = listening(hearing)?;

    standing(&mut surface, &queue, &saying, &wearing)
}

struct Queue {
    heard: Arc<Mutex<VecDeque<Message>>>,
    ended: Arc<Mutex<Ended>>,
    woken: OwnedFd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    Yes,
    No,
}

fn listening(hearing: Hearing) -> Result<Queue, Cannot> {
    let woken = console_waiting::woken::pipe().map_err(Cannot::Pipe)?;
    let (reading, writing) = (woken.waiting, woken.saying);
    let heard = Arc::new(Mutex::new(VecDeque::new()));
    let ended = Arc::new(Mutex::new(Ended::No));
    let held = Arc::clone(&heard);
    let over = Arc::clone(&ended);

    let reading_the_bus = std::thread::spawn(move || {
            let mut hearing = hearing;
            let mut writing = writing;

            loop {
                let message = match hearing.heard() {
                    Ok(message) => message,
                    Err(why) => {
                        eprintln!("console-notify: {why}");

                        match over.lock() {
                            Ok(mut over) => *over = Ended::Yes,
                            Err(_) => {},
                        }

                        let _ = writing.write_all(&[1]);

                        return;
                    }
                };

                match held.lock() {
                    Ok(mut held) => held.push_back(message),
                    Err(_) => return,
                }

                match writing.write_all(&[1]) {
                    Ok(()) => {}
                    Err(_) => return,
                }
            }
    });
    let Ok(()) = console_program_lifetime::threads::let_go(reading_the_bus);

    Ok(Queue { heard, ended, woken: reading })
}

fn dressed() -> Result<Wearing, Cannot> {
    let me = std::env::current_exe().map_err(Cannot::Lost)?;
    let Ok(at) = beside(&me);
    let held = std::fs::read_to_string(&at)
        .map_err(|why| Cannot::NoPalette { at: at.clone(), why })?;
    let Ok(spent) = read(&held);
    let wearing = Wearing::out_of(&spent)?;

    Ok(wearing)
}

struct Waiting {
    armed: Armed,
    until: Instant,
}

#[derive(PartialEq)]
struct Drew {
    stack: Stack,
    logical: Option<Size<u32>>,
    scale: Scale,
}

fn asking(surface: &Surface, stack: &Stack) -> Result<Drew, Never> {
    let Ok(logical) = surface.logical();
    let Ok(scale) = surface.scale();

    Ok(Drew { stack: stack.clone(), logical, scale })
}

fn standing(
    surface: &mut Surface,
    queue: &Queue,
    saying: &Saying,
    wearing: &Wearing,
) -> Result<(), Cannot> {
    let mut holding = Holding::default();
    let mut waiting: Vec<Waiting> = Vec::new();
    let woken = queue.woken.as_raw_fd();
    let Ok(font) = showing::font();
    let mut drew: Option<Drew> = None;

    loop {
        let Ok(()) = drained(queue, &mut holding, saying, &mut waiting);
        let Ok(()) = ran_out(&mut holding, saying, &mut waiting);

        let Ok(stack) = shown(&holding, wearing, &font);
        let Ok(wanted) = asking(surface, &stack);

        match drew.as_ref() == Some(&wanted) {
            true => {}
            false => {
                drawn(surface, &stack)?;

                let Ok(after) = asking(surface, &stack);

                drew = Some(after);
            }
        }

        let Ok(closed) = surface.closed();

        match closed {
            Closed::Yes => return Ok(()),
            Closed::No => {}
        }

        let Ok(until) = soonest(&waiting);

        surface.wait(&[woken], until).map_err(Cannot::Compositor)?;

        let Ok(()) = touched(surface, &stack, &mut holding, saying);

        let Ok(ended) = ending(queue);

        match ended {
            Ended::Yes => return Ok(()),
            Ended::No => {}
        }
    }
}

fn ending(queue: &Queue) -> Result<Ended, Never> {
    Ok(match queue.ended.lock() {
        Ok(ended) => *ended,
        Err(_) => Ended::Yes,
    })
}

fn drained(
    queue: &Queue,
    holding: &mut Holding,
    saying: &Saying,
    waiting: &mut Vec<Waiting>,
) -> Result<(), Never> {
    let Ok(()) = console_waiting::woken::drained(&queue.woken);

    loop {
        let message = match queue.heard.lock() {
            Ok(mut held) => held.pop_front(),
            Err(_) => None,
        };
        let message = match message {
            Some(message) => message,
            None => return Ok(()),
        };
        let Ok(turn) = serving::heard(holding, &message);
        let Ok(()) = turned(&turn, saying, waiting);

        match turn.changed {
            Changed::Yes => {
                let Ok(()) = kept(holding);
            }
            Changed::No => {}
        }
    }
}

fn turned(turn: &Turn, saying: &Saying, waiting: &mut Vec<Waiting>) -> Result<(), Never> {
    for gone in &turn.gone {
        let _ = saying.say(gone);
    }

    match &turn.say {
        Some(say) => {
            let _ = saying.say(say);
        }
        None => {}
    }

    match turn.arm {
        Some(armed) => {
            let Ok(()) = arming(armed, waiting);
        }
        None => {}
    }

    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit021_no_sleeping",
    allow(
        explicit021_no_sleeping,
        reason = "a notification says how long it lasts and the elapsing is the whole of what it asked for; there is no event to wait on, because the thing being waited for is that nothing happened for this long"
    )
)]
fn arming(armed: Armed, waiting: &mut Vec<Waiting>) -> Result<(), Never> {
    let many = match armed.expiry {
        Expiry::Stays => return Ok(()),
        Expiry::Milliseconds(many) => many,
    };
    let until = Instant::now().checked_add(Duration::from_millis(u64::from(many)));

    match until {
        Some(until) => waiting.push(Waiting { armed, until }),
        None => {},
    }

    Ok(())
}

fn soonest(waiting: &[Waiting]) -> Result<Option<Duration>, Never> {
    let now = Instant::now();
    let soonest = waiting.iter().map(|one| one.until).min();

    Ok(soonest
        .map(|until| until.saturating_duration_since(now).max(Duration::from_millis(1))))
}

fn ran_out(
    holding: &mut Holding,
    saying: &Saying,
    waiting: &mut Vec<Waiting>,
) -> Result<(), Never> {
    let now = Instant::now();
    let mut over = Vec::new();
    let mut still = Vec::new();

    for one in std::mem::take(waiting) {
        match one.until > now {
            true => still.push(one),
            false => over.push(one.armed),
        }
    }

    *waiting = still;

    for armed in over {
        let Ok(gone) = holding.ran_out(&armed);

        match gone {
            Gone::No => {}
            Gone::Yes => {
                let Ok(said) = going(&[armed.id], Why::RanOut);

                for gone in &said {
                    let _ = saying.say(gone);
                }

                let Ok(()) = kept(holding);
            }
        }
    }

    Ok(())
}

fn shown(holding: &Holding, wearing: &Wearing, font: &Font) -> Result<Stack, Never> {
    let Ok(showing) = holding.showing();
    let mut said = Vec::new();

    for held in showing.iter().take(showing::MOST) {
        let Ok(one) = sized(held, font);

        said.push(one);
    }

    let mut cards = Vec::new();

    for (held, measure) in showing.iter().take(showing::MOST).zip(&said) {
        cards.push((
            Said {
                id: held.notice.id,
                summary: &held.notice.summary,
                body: &held.notice.body,
                urgency: held.notice.urgency,
                value: held.value,
            },
            *measure,
        ));
    }

    let Ok(stack) = showing::cards(&cards, wearing);

    Ok(stack)
}

fn sized(held: &Held, font: &Font) -> Result<Measured, Never> {
    let Ok(inner) = showing::across();
    let Ok(inner) = console_core_number_conversion::fitted::<i32, u32>(inner);
    let Ok(summary) = measured(
        Run { said: &held.notice.summary, weight: console_core_shapes::Weight::Bold, wide: inner },
        font,
    );
    let body = match held.notice.body.is_empty() {
        true => None,
        false => {
            let Ok(body) = measured(
                Run {
                    said: &held.notice.body,
                    weight: console_core_shapes::Weight::Plain,
                    wide: inner,
                },
                font,
            );

            Some(body)
        }
    };

    Ok(Measured { summary, body })
}

fn drawn(surface: &mut Surface, stack: &Stack) -> Result<(), Cannot> {
    match stack.touching.is_empty() {
        true => {
            let Ok(()) = surface.hide();

            return Ok(());
        }
        false => {},
    }

    surface.show(&Wanted {
        namespace: showing::WHO.to_string(),
        anchor: Anchor::TopRight,
        size: stack.room,
        margin: Margin { top: showing::DOWN, right: showing::IN, bottom: 0, left: 0 },
        keyboard: Keyboard::Declines,
        room: Room::Over,
        under: Under::Nothing,
    })?;
    let Ok(()) = surface.resize(stack.room);

    let shapes = stack.shapes.clone();
    let points = stack.room;

    surface.draw(move |pixels, device, _scale| {
        match onto(pixels, Frame { device, points }, &shapes) {
            Ok(()) => {}
            Err(why) => eprintln!("console-notify: {why}"),
        }

        Ok(())
    })?;

    Ok(())
}

fn touched(
    surface: &mut Surface,
    stack: &Stack,
    holding: &mut Holding,
    saying: &Saying,
) -> Result<(), Never> {
    let Ok(pokes) = surface.pokes();

    for poke in pokes {
        let at = match poke {
            Poke::Up | Poke::Moved { .. } => continue,
            Poke::Down { at } => at,
        };
        let Ok(across) = console_core_number_conversion::toward_zero_i32(at.0);
        let Ok(down) = console_core_number_conversion::toward_zero_i32(at.1);
        let Ok(on) = stack.on(Point { across, down });

        match on {
            Some(id) => {
                let Ok(()) = dismissed(id, holding, saying);
            }
            None => {}
        }
    }

    Ok(())
}

fn dismissed(id: u32, holding: &mut Holding, saying: &Saying) -> Result<(), Never> {
    let Ok(gone) = holding.closed(id);

    match gone {
        Gone::No => Ok(()),
        Gone::Yes => {
            let Ok(said) = going(&[id], Why::Dismissed);

            for going in &said {
                let _ = saying.say(going);
            }

            kept(holding)
        }
    }
}

fn kept(holding: &Holding) -> Result<(), Never> {
    let Ok(at) = serving::kept();
    let Ok(whole) = serving::keeping(holding);
    let Ok(said) = written(&whole);

    let where_it_goes = match at.parent() {
        Some(where_it_goes) => where_it_goes,
        None => return Ok(()),
    };

    match std::fs::create_dir_all(where_it_goes) {
        Ok(()) => {}
        Err(fault) => {
            eprintln!("console-notify: {}: {fault}", where_it_goes.display());

            return Ok(());
        }
    }

    match console_core_atomic_writes::whole(&at, said.as_bytes()) {
        Ok(()) => Ok(()),
        Err(why) => {
            eprintln!("console-notify: {why}");

            Ok(())
        }
    }
}
