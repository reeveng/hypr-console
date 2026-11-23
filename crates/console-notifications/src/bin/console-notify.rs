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
//! one thread and will not be driven from someone else's, which is why this
//! used to be a glib main loop with the state in a `RefCell` and the reading
//! pushed through `spawn_blocking`. With the surface ours the loop is ours, so
//! what is left on a thread is the one thing that genuinely blocks: `update`
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
//! up held it until someone touched the card. What was last drawn is kept and
//! compared: the stack, and the size and scale the compositor last said, so a
//! configure or a scale arriving late still redraws while a card that has not
//! moved does not.
//!
//! **A card is taken down by touching it.** It is the only thing on this
//! desktop a person can reach without opening anything, and it was mako's
//! `on-touch=dismiss` before it was ours: a card that stands over what someone
//! is doing and cannot be got rid of is worse than one that never came. The hit
//! test is `Panel::covers` over the same stack that was drawn, so what a thumb
//! lands on is decided by the arithmetic rather than by a second opinion about
//! where the cards ended up.
//!
//! **The pipe refuses to block at both ends, which it did not.** A loop woken
//! by either the compositor's socket or the pipe drained the pipe on every
//! pass, so a wake that was the compositor's read a pipe no one had written
//! to and waited there for a byte that was not coming -- a daemon that stops
//! answering the first time two things happen in the wrong order. It is
//! `console_waiting::latch` now, where the reason lives beside the `unsafe`
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

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::process::ExitCode;
use std::sync::mpsc::{self, SyncSender, TryRecvError};
use std::time::{Duration, Instant};

use console_bus::connection::{Bus, NameRequestResult, Receiver, Sender};
use console_bus::messages::Message;
use console_core_color::palette::WearingError;
use console_core_geometry::{Point, Size};
use console_core_iteration::Step;
use console_core_never::Never;
use console_draw_painting::{Frame, Lines, Run, at_most, measure_text, onto};
use console_draw_surface::layer_focus::Selection;
use console_draw_surface::scale::Scale;
use console_draw_surface::standing::{Anchor, Closed, Keyboard, Margin, Room, Under, Wanted};
use console_draw_surface::{Activation, KeyboardEvent, PointerEvent, Surface, SurfaceError};
use console_notifications::reading::serialize;
use console_notifications::saying::Expiry;
use console_notifications::serving::{
  self, Armed, Closed as NotificationClosed, Holding, Modified, Turn, Why, closed_signals,
};
use console_notifications::showing::{self, CardContent, Measured, Stack, Wearing};
use console_onscreen::focus;

#[derive(Debug)]
enum Cannot {
  ConnectionError(console_bus::connection::ConnectionError),
  NameTaken,
  Palette(WearingError),
  Pipe(std::io::Error),
  Compositor(SurfaceError),
}

impl std::fmt::Display for Cannot {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Cannot::ConnectionError(why) => write!(to, "{why}"),
      Cannot::NameTaken => write!(
        to,
        "something else holds {}, so nothing here would ever be asked",
        serving::NOTIFICATIONS
      ),
      Cannot::Palette(why) => write!(to, "{why}"),
      Cannot::Pipe(why) => write!(to, "nothing to be woken down: {why}"),
      Cannot::Compositor(why) => write!(to, "{why}"),
    }
  }
}

impl std::error::Error for Cannot {}

impl From<console_bus::connection::ConnectionError> for Cannot {
  fn from(why: console_bus::connection::ConnectionError) -> Cannot {
    Cannot::ConnectionError(why)
  }
}

impl From<WearingError> for Cannot {
  fn from(why: WearingError) -> Cannot {
    Cannot::Palette(why)
  }
}

impl From<SurfaceError> for Cannot {
  fn from(why: SurfaceError) -> Cannot {
    Cannot::Compositor(why)
  }
}

fn main() -> ExitCode {
  match ::core::primitive::bool::then_some(
    {
      std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == focus::FLAG)
    },
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return match focus::Layer::Popups.take() {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
          eprintln!("console-notify: focus: {why}");

          ExitCode::FAILURE
        }
      };
    }
    ::core::option::Option::None => {}
  }

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
  let got = bus.request_name(serving::NOTIFICATIONS)?;

  match got {
    NameRequestResult::PrimaryOwner | NameRequestResult::AlreadyOwner => {}
    NameRequestResult::InQueue | NameRequestResult::Exists => return Err(Cannot::NameTaken),
  }

  let wearing = Wearing::worn()?;
  let mut surface = Surface::connect()?;
  let focus_requests = match focus::Layer::Popups.listen() {
    Ok(requests) => Some(requests),
    Err(fault) => {
      console_core_logging::warn!("console-notify: focus requests: {fault}");

      None
    }
  };
  let Ok((hearing, saying)) = bus.split();
  let queue = spawn_listener(hearing)?;

  run(
    &mut surface,
    &queue,
    &saying,
    &wearing,
    focus_requests.as_ref(),
  )
}

const QUEUE_CAPACITY: u32 = 256;

struct Queue {
  heard: mpsc::Receiver<Message>,
  woken: OwnedFd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
  Yes,
  No,
}

fn spawn_listener(hearing: Receiver) -> Result<Queue, Cannot> {
  let woken = console_waiting::latch::pipe().map_err(Cannot::Pipe)?;
  let (reading, writing) = (woken.waiting, woken.saying);
  let Ok(capacity) = console_core_number_conversion::index(QUEUE_CAPACITY);
  let (carrying, heard) = mpsc::sync_channel(capacity);

  let reading_the_bus = std::thread::spawn(move || {
    let Ok(()) = relay_bus(hearing, writing, carrying);
  });
  let Ok(()) = console_program_lifetime::threads::let_go(reading_the_bus);

  Ok(Queue {
    heard,
    woken: reading,
  })
}

fn relay_bus(
  hearing: Receiver,
  writing: File,
  carrying: SyncSender<Message>,
) -> Result<(), Never> {
  let stopped = console_core_iteration::iterate((hearing, writing), |(mut bus, mut latch)| {
    let message = match bus.receive() {
      Ok(message) => message,
      Err(fault) => {
        console_core_logging::warn!("console-notify: {fault}");

        return Ok(Step::Halt(latch));
      }
    };

    match carrying.send(message) {
      Ok(()) => {}
      Err(_notification_loop_closed) => return Ok(Step::Halt(latch)),
    }

    Ok(match wake(&mut latch) {
      Ok(()) => Step::Again((bus, latch)),
      Err(fault) => {
        console_core_logging::warn!(
          "console-notify: could not wake the notification loop: {fault}"
        );

        Step::Halt(latch)
      }
    })
  });

  drop(carrying);

  match stopped {
    Ok(mut latch) => {
      let _loop_can_also_see_the_pipe_close = wake(&mut latch);
    }
    Err(endless) => console_core_logging::warn!("console-notify: {endless}"),
  }

  Ok(())
}

fn wake(latch: &mut File) -> io::Result<()> {
  match latch.write_all(&[1]) {
    Ok(()) => Ok(()),
    Err(fault) => {
      match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::WouldBlock, ()) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => Err(fault),
      }
    }
  }
}

struct Waiting {
  armed: Armed,
  until: Instant,
}

#[derive(PartialEq)]
struct Rendered {
  stack: Stack,
  logical: Option<Size<u32>>,
  scale: Scale,
}

fn snapshot(surface: &Surface, stack: &Stack) -> Result<Rendered, Never> {
  let Ok(logical) = surface.logical();
  let Ok(scale) = surface.scale();

  Ok(Rendered {
    stack: stack.clone(),
    logical,
    scale,
  })
}

fn run(
  surface: &mut Surface,
  queue: &Queue,
  saying: &Sender,
  wearing: &Wearing,
  focus_requests: Option<&UnixDatagram>,
) -> Result<(), Cannot> {
  let running = Running {
    surface,
    holding: Holding::default(),
    waiting: Vec::new(),
    drew: None,
    selection: Selection::Idle,
    focus_layer: FocusLayer::Passive,
  };
  let asked = Context {
    queue,
    saying,
    wearing,
    woken: queue.woken.as_fd(),
    focus_requests,
  };

  match console_core_iteration::iterate(running, |running_2| turned(running_2, &asked)) {
    Ok(ended) => ended,
    Err(endless) => {
      eprintln!("console-notify: {endless}");

      Ok(())
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusLayer {
  Popups,
  Passive,
}

struct Running<'a> {
  surface: &'a mut Surface,
  holding: Holding,
  waiting: Vec<Waiting>,
  drew: Option<Rendered>,
  selection: Selection<u32>,
  focus_layer: FocusLayer,
}

struct Context<'a> {
  queue: &'a Queue,
  saying: &'a Sender,
  wearing: &'a Wearing,
  woken: BorrowedFd<'a>,
  focus_requests: Option<&'a UnixDatagram>,
}

fn turned<'a>(
  mut running: Running<'a>,
  asked: &Context<'_>,
) -> Result<Step<Running<'a>, Result<(), Cannot>>, Never> {
  let Ok(ended) = drain_queue(
    asked.queue,
    &mut running.holding,
    asked.saying,
    &mut running.waiting,
  );

  match ended {
    Ended::Yes => return Ok(Step::Halt(Ok(()))),
    Ended::No => {}
  }

  let Ok(()) = ran_out(&mut running.holding, asked.saying, &mut running.waiting);

  let Ok(mut stack) = shown(&running.holding, asked.wearing);
  let actions: Vec<_> = stack.touching.iter().map(|region| region.id).collect();
  let Ok(()) = running.selection.keep(&actions);
  let Ok(target) = running.selection.target();
  let focus_layer = match target {
    Some(_) => FocusLayer::Popups,
    None => FocusLayer::Passive,
  };

  match (running.focus_layer, focus_layer) {
    (FocusLayer::Passive, FocusLayer::Popups) | (FocusLayer::Popups, FocusLayer::Passive) => {
      let Ok(()) = running.surface.hide();
      running.drew = None;
      running.focus_layer = focus_layer;
    }
    (FocusLayer::Popups, FocusLayer::Popups) | (FocusLayer::Passive, FocusLayer::Passive) => {}
  }

  match target.and_then(|id| stack.touching.iter().find(|region| region.id == id)) {
    Some(region) => {
      let Ok(outline) = region.panel.focus_outline(asked.wearing.text);

      stack
        .shapes
        .extend(outline.map(console_core_shapes::Shape::Line));
    }
    None => {}
  }

  let Ok(wanted) = snapshot(running.surface, &stack);

  match ::core::primitive::bool::then_some(running.drew.as_ref() == Some(&wanted), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      match draw(running.surface, &stack, focus_layer) {
        Ok(()) => {}
        Err(fault) => return Ok(Step::Halt(Err(fault))),
      }

      let Ok(after) = snapshot(running.surface, &stack);

      running.drew = Some(after);
    }
  }

  let Ok(closed) = running.surface.closed();

  match closed {
    Closed::Yes => return Ok(Step::Halt(Ok(()))),
    Closed::No => {}
  }

  let Ok(until) = soonest(&running.waiting);
  let mut listening = vec![asked.woken];
  listening.extend(asked.focus_requests.map(AsFd::as_fd));
  let Ok(focus_timeout) = running.selection.timeout(Instant::now());
  let until_2 = until.into_iter().chain(focus_timeout).min();

  match running.surface.wait(&listening, until_2) {
    Ok(()) => {}
    Err(fault) => return Ok(Step::Halt(Err(Cannot::Compositor(fault)))),
  }

  let focus_request = asked
    .focus_requests
    .and_then(|socket| match focus::heard(socket) {
      Ok(request) => request,
      Err(fault) => {
        console_core_logging::debug!("console-notify: focus request could not be read: {fault}");

        None
      }
    });
  let Ok(owns_keyboard) = running.surface.keyboard_focus();
  let Ok(()) = running.selection.observe(owns_keyboard, Instant::now());

  match ::core::primitive::bool::then_some({ focus_request == Some(focus::Request::Release) }, ())
  {
    ::core::option::Option::Some(()) => {
      running.selection = Selection::Idle;
    }
    ::core::option::Option::None => {}
  }

  let Ok(keys) = running.surface.keyboard_events();

  for KeyboardEvent::Down { key } in keys {
    let Ok(activated) = running.selection.pressed(key, &actions);

    match activated {
      Some(id) => {
        let Ok(()) = dismissed(id, &mut running.holding, asked.saying);
      }
      ::core::option::Option::None => {}
    }
  }

  let Ok(()) = handle_pointer(
    running.surface,
    &stack,
    &mut running.selection,
    &mut running.holding,
    asked.saying,
  );

  match ::core::primitive::bool::then_some({ focus_request == Some(focus::Request::Take) }, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = running.selection.enter(&actions, Instant::now());
      let Ok(focused_target) = running.selection.target();

      match focused_target {
        Some(_) => {
          let _peer_may_be_absent = focus::Layer::Popups.release_peer();
        }
        None => {}
      }
    }
    ::core::option::Option::None => {}
  }

  Ok(Step::Again(running))
}

fn drain_queue(
  queue: &Queue,
  holding: &mut Holding,
  saying: &Sender,
  waiting: &mut Vec<Waiting>,
) -> Result<Ended, Never> {
  let Ok(()) = console_waiting::latch::drain(&queue.woken);

  for _slot in 0..QUEUE_CAPACITY {
    let message = match queue.heard.try_recv() {
      Ok(message) => message,
      Err(TryRecvError::Empty) => return Ok(Ended::No),
      Err(TryRecvError::Disconnected) => return Ok(Ended::Yes),
    };

    let Ok(turn) = serving::handle_message(holding, &message);
    let Ok(()) = apply_turn(&turn, saying, waiting);

    match turn.changed {
      Modified::Yes => {
        let Ok(()) = save_inbox(holding);
      }
      Modified::No => {}
    }
  }

  Ok(Ended::No)
}

fn apply_turn(turn: &Turn, saying: &Sender, waiting: &mut Vec<Waiting>) -> Result<(), Never> {
  for closed in &turn.closed {
    let Ok(()) = reply(saying, closed);
  }

  match &turn.say {
    Some(say) => {
      let Ok(()) = reply(saying, say);
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

fn reply(saying: &Sender, message: &Message) -> Result<(), Never> {
  match saying.say(message) {
    Ok(_serial) => {}
    Err(fault) => {
      console_core_logging::warn!("console-notify: could not send a bus reply: {fault}")
    }
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
    Some(until_2) => waiting.push(Waiting {
      armed,
      until: until_2,
    }),
    None => {}
  }

  Ok(())
}

fn soonest(waiting: &[Waiting]) -> Result<Option<Duration>, Never> {
  let now = Instant::now();
  let soonest = waiting.iter().map(|one| one.until).min();

  Ok(soonest.map(|until| {
    until
      .saturating_duration_since(now)
      .max(Duration::from_millis(1))
  }))
}

fn ran_out(
  holding: &mut Holding,
  saying: &Sender,
  waiting: &mut Vec<Waiting>,
) -> Result<(), Never> {
  let now = Instant::now();
  let mut over = Vec::new();
  let mut still = Vec::new();

  for one in std::mem::take(waiting) {
    match ::core::primitive::bool::then_some(one.until > now, ()) {
      ::core::option::Option::Some(()) => still.push(one),
      ::core::option::Option::None => over.push(one.armed),
    }
  }

  *waiting = still;

  for armed in over {
    let Ok(ran_out) = holding.ran_out(&armed);

    match ran_out {
      NotificationClosed::No => {}
      NotificationClosed::Yes => {
        let Ok(said) = closed_signals(&[armed.id], Why::RanOut);

        for closed in &said {
          let Ok(()) = reply(saying, closed);
        }

        let Ok(()) = save_inbox(holding);
      }
    }
  }

  Ok(())
}

const SUMMARY_LINES: Lines = Lines(2);

const BODY_LINES: Lines = Lines(2);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Clipped {
  summary: String,
  body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Text<'a> {
  summary: &'a str,
  body: &'a str,
}

fn clipped(words: Text<'_>) -> Result<Clipped, Never> {
  let Text { summary, body } = words;
  let Ok(summary_font) = showing::SUMMARY.font();
  let Ok(summary_weight) = showing::SUMMARY.weight();
  let Ok(body_font) = showing::BODY.font();
  let Ok(body_weight) = showing::BODY.weight();
  let Ok(inner) = showing::x();
  let Ok(inner_2) = console_core_number_conversion::fitted::<i32, u32>(inner);
  let Ok(summary_2) = at_most(
    Run {
      said: summary,
      weight: summary_weight,
      width: inner_2,
    },
    &summary_font,
    SUMMARY_LINES,
  );
  let Ok(body_2) = at_most(
    Run {
      said: body,
      weight: body_weight,
      width: inner_2,
    },
    &body_font,
    BODY_LINES,
  );

  Ok(Clipped {
    summary: summary_2,
    body: body_2,
  })
}

fn shown(holding: &Holding, wearing: &Wearing) -> Result<Stack, Never> {
  let Ok(showing) = holding.visible();
  let Ok(most) = console_core_number_conversion::index(showing::MOST);
  let mut said = Vec::new();

  for held in showing.iter().take(most) {
    let Ok(words) = clipped(Text {
      summary: &held.notification.summary,
      body: &held.notification.body,
    });
    let Ok(measure) = sized(&words);

    said.push((words, measure));
  }

  let mut cards = Vec::new();

  for (held, (words, measure)) in showing.iter().take(most).zip(&said) {
    cards.push((
      CardContent {
        id: held.notification.id,
        summary: &words.summary,
        body: &words.body,
        urgency: held.notification.urgency,
        value: held.value,
      },
      *measure,
    ));
  }

  let Ok(stack) = showing::cards(&cards, wearing);

  Ok(stack)
}

fn sized(words: &Clipped) -> Result<Measured, Never> {
  let Ok(summary_font) = showing::SUMMARY.font();
  let Ok(summary_weight) = showing::SUMMARY.weight();
  let Ok(body_font) = showing::BODY.font();
  let Ok(body_weight) = showing::BODY.weight();
  let Ok(inner) = showing::x();
  let Ok(inner_2) = console_core_number_conversion::fitted::<i32, u32>(inner);
  let Ok(summary) = measure_text(
    Run {
      said: &words.summary,
      weight: summary_weight,
      width: inner_2,
    },
    &summary_font,
  );

  let body = match ::core::primitive::bool::then_some(words.body.is_empty(), ()) {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => {
      let Ok(body) = measure_text(
        Run {
          said: &words.body,
          weight: body_weight,
          width: inner_2,
        },
        &body_font,
      );

      Some(body)
    }
  };

  Ok(Measured { summary, body })
}

fn draw(surface: &mut Surface, stack: &Stack, layer: FocusLayer) -> Result<(), Cannot> {
  match stack.touching.as_slice() {
    [] => {
      let Ok(()) = surface.hide();

      return Ok(());
    }
    [_first, ..] => {}
  }

  let (namespace, keyboard) = match layer {
    FocusLayer::Popups => {
      let Ok(namespace) = focus::Layer::Popups.namespace();

      (namespace, Keyboard::Takes)
    }
    FocusLayer::Passive => (showing::WHO, Keyboard::Declines),
  };

  surface.show(&Wanted {
    namespace: namespace.to_string(),
    anchor: Anchor::TopRight,
    size: stack.room,
    margin: Margin {
      top: showing::DOWN,
      right: showing::IN,
      bottom: 0,
      left: 0,
    },
    keyboard,
    room: Room::Over,
    under: Under::None,
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

fn handle_pointer(
  surface: &mut Surface,
  stack: &Stack,
  selection: &mut Selection<u32>,
  holding: &mut Holding,
  saying: &Sender,
) -> Result<(), Never> {
  let Ok(pointer_events) = surface.pointer_events();

  for event in pointer_events {
    let at = match event {
      PointerEvent::Up
      | PointerEvent::Left
      | PointerEvent::Moved { .. }
      | PointerEvent::Scrolled { .. }
      | PointerEvent::Pinched { .. } => continue,
      PointerEvent::Down { at } => at,
    };
    let Ok(across) = console_core_number_conversion::toward_zero_i32(at.0);
    let Ok(down) = console_core_number_conversion::toward_zero_i32(at.1);
    let Ok(on) = Activation::At(Point { x: across, y: down }).resolve(
      || None,
      |point| {
        let Ok(hit) = stack.on(point);

        hit
      },
    );

    match on {
      Some(id) => {
        let Ok(()) = dismissed(id, holding, saying);
      }
      None => *selection = Selection::Idle,
    }
  }

  Ok(())
}

fn dismissed(id: u32, holding: &mut Holding, saying: &Sender) -> Result<(), Never> {
  let Ok(closed) = holding.closed(id);

  match closed {
    NotificationClosed::No => Ok(()),
    NotificationClosed::Yes => {
      let Ok(said) = closed_signals(&[id], Why::Dismissed);

      for going in &said {
        let Ok(()) = reply(saying, going);
      }

      save_inbox(holding)
    }
  }
}

fn save_inbox(holding: &Holding) -> Result<(), Never> {
  let Ok(at) = serving::inbox_path();
  let Ok(whole) = serving::to_inbox(holding);
  let Ok(said) = serialize(&whole);

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

#[cfg(test)]
mod tests {
  use super::*;

  const SCREEN_TALL: u32 = 480;

  fn stacked(words: Text<'_>) -> Result<u32, Never> {
    let Ok(clipped) = clipped(words);
    let Ok(measure) = sized(&clipped);
    let Ok(tall) = showing::height(&measure, None);
    let Ok(gaps) = console_core_number_conversion::fitted::<i32, u32>(
      showing::BETWEEN
        .saturating_mul(2)
        .saturating_add(showing::DOWN),
    );

    Ok(tall.saturating_mul(showing::MOST).saturating_add(gaps))
  }

  #[test]
  fn three_cards_of_a_very_long_notification_still_fit_on_the_screen() {
    let words = "Meditations of the Emperor ".repeat(400);
    let unbroken = "x".repeat(8000);

    let Ok(worded) = stacked(Text {
      summary: &words,
      body: &words,
    });
    let Ok(unbroken_height) = stacked(Text {
      summary: &unbroken,
      body: &unbroken,
    });

    assert!(
      worded <= SCREEN_TALL,
      "{worded} is past the foot of the screen"
    );
    assert!(
      unbroken_height <= SCREEN_TALL,
      "a word with no space in it is cut the same"
    );
  }

  #[test]
  fn a_long_notification_says_it_was_cut_and_a_short_one_is_left_alone() {
    let many = "word ".repeat(2000);
    let Ok(long) = clipped(Text {
      summary: &many,
      body: &many,
    });
    let Ok(short) = clipped(Text {
      summary: "Meditations",
      body: "is in Books",
    });

    assert!(long.summary.ends_with(console_draw_painting::CUT));
    assert!(long.body.ends_with(console_draw_painting::CUT));
    assert_eq!(
      short,
      Clipped {
        summary: "Meditations".to_string(),
        body: "is in Books".to_string()
      }
    );
  }
}
