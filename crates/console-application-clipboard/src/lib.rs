use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;

use console_compositor::{ActiveWindow, DispatchResult, Layers};
use console_core_arguments::{Command, Flag, Operands, Takes};
use console_core_controls::switch::Switch;
use console_core_internal_programs::{CLIPBOARD_COPY_PNG, CLIPBOARD_COPY_TEXT};
use console_core_never::Never;
use console_panel::card::{Card, Door};
use console_panel::page::{ActionOutcome, Aside, Handler, Page, Row, Rows, Subject};
use console_waiting::{Outcome, Ready, Schedule, until};

mod copying;
mod history;
mod ipc;
mod wayland;

use history::{Capture, Preview};
use ipc::{Reply, Request};
pub use wayland::{read_system_text, watch};

pub const WHO: &str = "console-clipboard";
pub const WATCH: Flag = Flag {
  spelling: "--watch",
  takes: Takes::None,
  about: "run the clipboard history service for this session",
};
pub const READ_TEXT: Flag = Flag {
  spelling: "--read-text",
  takes: Takes::None,
  about: "print the active plain-text clipboard selection",
};
pub const COMMAND: Command = Command {
  name: env!("CARGO_PKG_NAME"),
  about: "choose an item from the system clipboard history",
  flags: &[WATCH, READ_TEXT, CLIPBOARD_COPY_TEXT, CLIPBOARD_COPY_PNG],
  operands: Operands::None,
};

pub fn copy_text(input: impl std::io::Read) -> std::io::Result<()> {
  let text = console_core_bounded_reads::text(input, history::MAX_TEXT_BYTES)?;
  let ask = ipc::ask(Request::CopyText(text))?;

  copied(ask)
}

pub fn copy_png(input: impl std::io::Read) -> std::io::Result<()> {
  copied({
    let checked_value = ipc::copy_png(input)?;
    checked_value
  })
}

fn copied(reply: Reply) -> std::io::Result<()> {
  match reply {
    Reply::Finished => Ok(()),
    Reply::List { .. } | Reply::Text(_) | Reply::Unavailable => Err(std::io::Error::other(
      "clipboard service refused the selection",
    )),
  }
}

fn panel_reply(request: Request) -> Result<Option<Reply>, Never> {
  Ok(match ipc::ask(request) {
    Ok(reply) => Some(reply),
    Err(fault) => {
      console_core_logging::debug!("clipboard panel request failed: {:?}", fault.kind());
      None
    }
  })
}

fn privacy(capture: Capture) -> Result<Row, Never> {
  let (value, next) = match capture {
    Capture::Paused => (Switch::On, Capture::Recording),
    Capture::Recording => (Switch::Off, Capture::Paused),
  };
  let Ok(change) = Handler::and_stay(move |showing| {
    let Ok(reply) = panel_reply(Request::Pause(next));

    match reply {
      Some(Reply::Finished) => showing.replace(0),
      Some(Reply::List { .. } | Reply::Text(_) | Reply::Unavailable) | None => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Clipboard"),
          "Couldn’t change clipboard privacy",
          None,
        );
      }
    }
  });
  let Ok(row) = Row::new("Pause history", Aside(""), change);

  row.switched(value)
}

enum ClearHistory {
  Available,
  Unavailable,
}

fn rows(paste: &SyncSender<String>) -> Result<Vec<Row>, Never> {
  let Ok(reply) = panel_reply(Request::List);
  let (capture, entries) = match reply {
    Some(Reply::List { paused, entries }) => (paused, entries),
    Some(Reply::Finished | Reply::Text(_) | Reply::Unavailable) | None => {
      let Ok(unavailable) = Row::placeholder("Clipboard history is unavailable");
      return Ok(vec![unavailable]);
    }
  };

  let Ok(privacy_row) = privacy(capture);
  let mut rows = vec![privacy_row];
  let clear_history = match (entries.as_slice(), capture) {
    ([], Capture::Paused) => {
      let Ok(hidden) = Row::placeholder("History is hidden while paused");
      rows.push(hidden);
      ClearHistory::Available
    }
    ([], Capture::Recording) => {
      let Ok(empty) = Row::placeholder("Nothing copied yet");
      rows.push(empty);
      ClearHistory::Unavailable
    }
    ([_, ..], Capture::Paused | Capture::Recording) => ClearHistory::Available,
  };

  for entry in entries {
    let Ok(row) = restore_row(entry, paste.clone());
    rows.push(row);
  }

  match clear_history {
    ClearHistory::Available => {
      let Ok(clear) = clear_row();
      rows.push(clear);
    }
    ClearHistory::Unavailable => {}
  }

  Ok(rows)
}

fn restore_row(entry: Preview, paste: SyncSender<String>) -> Result<Row, Never> {
  let Ok(restore) = Handler::call(move |showing| {
    let target = match console_compositor::ask(ActiveWindow) {
      Ok(Some(window)) => Some(window.address),
      Ok(None) => None,
      Err(fault) => {
        console_core_logging::debug!("clipboard paste target unavailable: {fault}");

        None
      }
    };
    let Ok(reply) = panel_reply(Request::Restore(entry.id));

    match reply {
      Some(Reply::Finished) => {
        match target {
          Some(address) => {
            let _queued = paste.try_send(address);
          }
          None => {}
        }

        ActionOutcome::Close
      }
      Some(Reply::List { .. } | Reply::Text(_) | Reply::Unavailable) | None => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Clipboard"),
          "That clipboard item is no longer available",
          None,
        );
        showing.replace(0);
        ActionOutcome::Stay
      }
    }
  });
  let Ok(label) = entry.label.text();

  Row::new(label, Aside(""), restore)
}

fn clear_row() -> Result<Row, Never> {
  let Ok(clear) = Handler::and_stay(|showing| {
    showing.sure(
      "Clear clipboard history?",
      Subject(""),
      &["Clear"],
      Box::new(|confirming, _answer| {
        let Ok(reply) = panel_reply(Request::Clear);

        match reply {
          Some(Reply::Finished) => confirming.replace(0),
          Some(Reply::List { .. } | Reply::Text(_) | Reply::Unavailable) | None => {
            let Ok(()) = console_notifications::notify(
              console_notifications::Application("Clipboard"),
              "Couldn’t clear clipboard history",
              None,
            );
          }
        }
      }),
    );
  });

  Row::new("Clear history…", Aside(""), clear)
}

pub fn door(_asked: &[String]) -> Result<Door, Never> {
  Door::closing(WHO)
}

pub fn card(_asked: &[String]) -> Result<Card, Never> {
  let (paste, after_close) = mpsc::sync_channel(1);
  let Ok(card) = Card::new(move || {
    let Ok(computed) = Rows::computed(move || {
      let Ok(rows) = rows(&paste);
      rows
    });
    let Ok(page) = Page::new("Clipboard History", computed);
    vec![page]
  });

  card.shutting(Box::new(move || paste_after_close(&after_close)))
}

fn paste_after_close(queued: &Receiver<String>) -> Result<(), Never> {
  let address = match queued.try_recv() {
    Ok(address) => address,
    Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => return Ok(()),
  };
  let Ok(patience) =
    Schedule::asking_every(Duration::from_millis(300), Duration::from_millis(10));
  let Ok(closed) = until(patience, || {
    let layers = match console_compositor::ask(Layers) {
      Ok(layers) => layers,
      Err(_the_compositor_is_unavailable) => return Ok(Ready::NotYet),
    };

    let visible = layers.iter().any(|layer| layer.namespace == WHO);

    Ok(match visible.then_some(()) {
      Some(()) => Ready::NotYet,
      None => Ready::Yes,
    })
  });

  match closed {
    Outcome::Happened => {}
    Outcome::RanOut => return Ok(()),
  }

  let Ok(pasted) = console_compositor::paste_into(&address);

  match pasted {
    DispatchResult::Success => {}
    DispatchResult::Failure(fault) => {
      console_core_logging::debug!("clipboard paste failed: {fault}")
    }
  }

  Ok(())
}
