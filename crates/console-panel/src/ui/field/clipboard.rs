//! The clipboard effects issued by an editor and the worker that carries them out.

use std::io::{self, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::process::Stdio;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use console_concurrency::Latest;
use console_core_internal_programs::{CLIPBOARD_COPY_TEXT, CLIPBOARD_READ_TEXT, InternalProgram};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_state_machine::{
  EffectDependencies, Service, SingleEffectMachine, SingleTransition,
};
use console_draw_surface::{KeyPress, KeyPressKind, Keysym};

const CAPACITY: u32 = 4;
const COMMAND_MODIFIERS: u32 = 1 | KeyPress::CONTROL | (1 << 3) | (1 << 6);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
  Copy,
  Cut,
  Paste,
  Repeat,
  Other,
}

pub fn shortcut(press: KeyPress) -> Result<Shortcut, Never> {
  Ok(
    match (press.modifiers & COMMAND_MODIFIERS, press.key, press.kind) {
      (KeyPress::CONTROL, Keysym::c | Keysym::C, KeyPressKind::Initial) => Shortcut::Copy,
      (KeyPress::CONTROL, Keysym::x | Keysym::X, KeyPressKind::Initial) => Shortcut::Cut,
      (KeyPress::CONTROL, Keysym::v | Keysym::V, KeyPressKind::Initial) => Shortcut::Paste,
      (
        KeyPress::CONTROL,
        Keysym::c | Keysym::C | Keysym::x | Keysym::X | Keysym::v | Keysym::V,
        KeyPressKind::Repeated,
      ) => Shortcut::Repeat,
      _ => Shortcut::Other,
    },
  )
}

#[derive(Default)]
struct Pending {
  last: u64,
  active: Option<u64>,
}

enum Request {
  Copy(String),
  Cut(String),
  Paste,
  Cancel,
  Reply(Reply),
}

struct Reply {
  id: u64,
  change: Option<Change>,
}

enum Work {
  Write { text: String, cut: Option<u64> },
  Read(u64),
}

pub enum Change {
  Paste(String),
  Cut,
}

enum Effect {
  Clipboard(Work),
  Edit(Change),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Clipboard(_) => &[Service::Clipboard],
      Self::Edit(_) => &[],
    })
  }
}

impl SingleEffectMachine for Pending {
  type Input = ();
  type State = Self;
  type Request = Request;
  type Effect = Effect;

  fn initialize(_: &(), _: Option<Self>) -> Result<SingleTransition<Self, Effect>, Never> {
    Ok(SingleTransition {
      state: Self::default(),
      effect: None,
    })
  }

  fn handle(mut state: Self, request: Request) -> Result<SingleTransition<Self, Effect>, Never> {
    let effect = match request {
      Request::Copy(text) => Some(Effect::Clipboard(Work::Write { text, cut: None })),
      Request::Cut(text) => {
        let Ok(id) = state.begin();
        Some(Effect::Clipboard(Work::Write {
          text,
          cut: Some(id),
        }))
      }
      Request::Paste => {
        let Ok(id) = state.begin();
        Some(Effect::Clipboard(Work::Read(id)))
      }
      Request::Cancel => {
        state.active = None;
        None
      }
      Request::Reply(reply) => match state.active.filter(|active| *active == reply.id) {
        Some(_) => {
          state.active = None;
          reply.change.map(Effect::Edit)
        }
        None => None,
      },
    };

    Ok(SingleTransition { state, effect })
  }
}

impl Pending {
  fn begin(&mut self) -> Result<u64, Never> {
    self.last = self.last.wrapping_add(1);
    self.active = Some(self.last);
    Ok(self.last)
  }
}

#[derive(Default)]
pub struct Clipboard {
  pending: Pending,
  worker: Option<Worker>,
}

impl Clipboard {
  pub fn descriptor(&self) -> Result<Option<BorrowedFd<'_>>, Never> {
    Ok(self.worker.as_ref().map(|worker| worker.ready.as_fd()))
  }

  pub fn copy(&mut self, text: &str) -> Result<(), Never> {
    self.submit(Request::Copy(text.to_owned()))
  }

  pub fn cut(&mut self, text: &str) -> Result<(), Never> {
    self.submit(Request::Cut(text.to_owned()))
  }

  pub fn paste(&mut self) -> Result<(), Never> {
    self.submit(Request::Paste)
  }

  pub fn cancel(&mut self) -> Result<(), Never> {
    let Ok(_effect) = self.request(Request::Cancel);
    Ok(())
  }

  pub fn receive(&mut self) -> Result<Option<Change>, Never> {
    let worker = match self.worker.as_ref() {
      Some(worker) => worker,
      None => return Ok(None),
    };
    let mut notices = [0; 4];

    match rustix::io::read(&worker.ready, &mut notices) {
      Ok(_) => {}
      Err(rustix::io::Errno::AGAIN) => return Ok(None),
      Err(fault) => {
        console_core_logging::warn!("reading clipboard results: {fault}");
        return Ok(None);
      }
    }

    let Ok(capacity) = index(CAPACITY);
    let replies: Vec<_> = worker.replies.try_iter().take(capacity).collect();

    for reply in replies {
      let Ok(effect) = self.request(Request::Reply(reply));

      match effect {
        Some(Effect::Edit(change)) => return Ok(Some(change)),
        Some(Effect::Clipboard(_)) | None => {}
      }
    }

    Ok(None)
  }

  fn request(&mut self, request: Request) -> Result<Option<Effect>, Never> {
    let Ok(turn) = Pending::handle(std::mem::take(&mut self.pending), request);
    self.pending = turn.state;
    Ok(turn.effect)
  }

  fn submit(&mut self, request: Request) -> Result<(), Never> {
    let Ok(effect) = self.request(request);
    let work = match effect {
      Some(Effect::Clipboard(work)) => work,
      Some(Effect::Edit(_)) | None => return Ok(()),
    };
    let wait = matches!(work, Work::Read(_) | Work::Write { cut: Some(_), .. });
    let sent = self.send(work);

    match sent {
      Ok(()) => Ok(()),
      Err(fault) => {
        console_core_logging::warn!("queueing clipboard work: {fault}");

        match wait.then_some(()) {
          Some(()) => {
            let Ok(()) = self.cancel();
          }
          None => {}
        }

        Ok(())
      }
    }
  }

  fn send(&mut self, work: Work) -> io::Result<()> {
    let worker = match self.worker.as_ref() {
      Some(worker) => worker,
      None => {
        let started = Worker::start()?;
        self.worker.insert(started)
      }
    };
    worker
      .work
      .try_send(work)
      .map_err(|fault| io::Error::other(format!("{fault}")))?;
    worker
      .wake
      .schedule()
      .map_err(|_| io::Error::other("clipboard worker stopped"))
  }
}

struct Worker {
  work: SyncSender<Work>,
  replies: Receiver<Reply>,
  ready: OwnedFd,
  wake: Latest,
}

impl Worker {
  fn start() -> io::Result<Self> {
    let (ready, notify) = rustix::pipe::pipe_with(
      rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
    )?;
    let Ok(capacity) = index(CAPACITY);
    let (work, jobs) = sync_channel(capacity);
    let (publish, replies) = sync_channel(capacity);
    let wake = Latest::start("editor-clipboard".to_owned(), move || {
      for job in jobs.try_iter().take(capacity) {
        let Ok(()) = perform(job, &publish, &notify);
      }

      Ok(())
    })?;

    Ok(Self {
      work,
      replies,
      ready,
      wake,
    })
  }
}

fn perform(work: Work, replies: &SyncSender<Reply>, notify: &OwnedFd) -> Result<(), Never> {
  let reply = match work {
    Work::Write { text, cut } => {
      let change = match put_on_clipboard(&text) {
        Ok(()) => Some(Change::Cut),
        Err(fault) => {
          console_core_logging::warn!("copying editor selection: {fault}");
          None
        }
      };
      cut.map(|id| Reply { id, change })
    }
    Work::Read(id) => {
      let text = match clipboard_text() {
        Ok(text) => text,
        Err(fault) => {
          console_core_logging::warn!("pasting into an editor: {fault}");
          None
        }
      };
      Some(Reply {
        id,
        change: text.map(Change::Paste),
      })
    }
  };

  match reply {
    Some(answer) => match replies.send(answer) {
      Ok(()) => match rustix::io::write(notify, &[1]) {
        Ok(_) => {}
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::PIPE) => return Ok(()),
        Err(fault) => console_core_logging::warn!("waking clipboard editor: {fault}"),
      },
      Err(_editor_closed) => return Ok(()),
    },
    None => {}
  }

  Ok(())
}

fn clipboard_text() -> io::Result<Option<String>> {
  let Ok(mut command) = InternalProgram::Clipboard.command();
  let copied = command
    .arg(CLIPBOARD_READ_TEXT.spelling)
    .stdin(Stdio::null())
    .output()?;

  match copied.status.success().then_some(()) {
    Some(()) => String::from_utf8(copied.stdout)
      .map(Some)
      .map_err(io::Error::other),
    None => Ok(None),
  }
}

fn put_on_clipboard(text: &str) -> io::Result<()> {
  let Ok(mut command) = InternalProgram::Clipboard.command();
  command
    .arg(CLIPBOARD_COPY_TEXT.spelling)
    .stdin(Stdio::piped());
  let mut copying = console_program_lifetime::let_go(&mut command)?;
  let Ok(pipe) = copying.take_stdin();
  let written = match pipe {
    Some(mut input) => input.write_all(text.as_bytes()),
    None => Err(io::Error::other("clipboard opened no text stream")),
  };
  let status = copying.wait()?;
  written?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other(format!(
      "clipboard copy exited with {status}"
    ))),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn only_the_current_clipboard_reply_can_edit() {
    let Ok(first) = Pending::handle(Pending::default(), Request::Paste);
    let Ok(cancelled) = Pending::handle(first.state, Request::Cancel);
    let Ok(second) = Pending::handle(cancelled.state, Request::Cut("selection".to_owned()));
    let Ok(stale) = Pending::handle(
      second.state,
      Request::Reply(Reply {
        id: 1,
        change: Some(Change::Paste("wrong editor".to_owned())),
      }),
    );
    let Ok(current) = Pending::handle(
      stale.state,
      Request::Reply(Reply {
        id: 2,
        change: Some(Change::Cut),
      }),
    );

    assert!(stale.effect.is_none());
    assert!(matches!(current.effect, Some(Effect::Edit(Change::Cut))));
  }
}
