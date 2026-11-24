use std::io::{self, Write};
use std::process::Stdio;
use std::sync::mpsc::{self, SyncSender};

use console_core_external_programs::Program;
use console_core_geometry::Rectangle;
use console_core_internal_programs::{CLIPBOARD_COPY_TEXT, InternalProgram};
use console_core_never::Never;
use console_program_lifetime::{let_go, threads};

const OCR_LANGUAGES: &str = "eng+nld+fra";

pub(super) enum Request {
  Text(Rectangle<i64>),
  Screenshot(Option<Rectangle<i64>>),
}

pub(super) fn worker() -> io::Result<SyncSender<Request>> {
  let (send, receive) = mpsc::sync_channel(0);
  let thread = std::thread::Builder::new()
    .name("screen-capture".to_string())
    .spawn(move || {
      #[cfg_attr(
        dylint_lib = "explicit055_no_unbounded_loop",
        expect(
          explicit055_no_unbounded_loop,
          reason = "the receiver keeps capture available throughout the session and stops when Control Center drops its sole sender"
        )
      )]
      for request in receive {
        let Ok(()) = finish(request);
      }
    })?;
  let Ok(()) = threads::let_go(thread);
  Ok(send)
}

fn finish(request: Request) -> Result<(), Never> {
  match request {
    Request::Text(region) => match capture(region) {
      Ok(Captured::Copied) => notify("Text copied to clipboard"),
      Ok(Captured::Empty) => notify("No text found in the selection"),
      Err(fault) => {
        console_core_logging::warn!("text capture: {fault}");
        notify("Couldn’t capture text")
      }
    },
    Request::Screenshot(region) => match console_application_camera::screenshot(region) {
      Ok(shot) => match shot.announce() {
        Ok(()) => Ok(()),
        Err(fault) => {
          console_core_logging::warn!("couldn’t announce screenshot: {fault}");
          notify("Screenshot saved to Pictures")
        }
      },
      Err(fault) => {
        console_core_logging::warn!("screenshot: {fault}");
        notify("Couldn’t save screenshot")
      }
    },
  }
}

enum Captured {
  Copied,
  Empty,
}

fn capture(region: Rectangle<i64>) -> io::Result<Captured> {
  let geometry = format!(
    "{},{} {}x{}",
    region.origin.x, region.origin.y, region.size.width, region.size.height
  );
  let text = recognize(&geometry)?;
  let text_2 = text.trim_end();

  match text_2.as_bytes() {
    [] => return Ok(Captured::Empty),
    [_, ..] => {}
  }

  copy(text_2)?;
  Ok(Captured::Copied)
}

fn recognize(geometry: &str) -> io::Result<String> {
  let Ok(mut grabbing) = Program::Grim.command();
  grabbing
    .args(["-g", geometry, "-"])
    .stdin(Stdio::null())
    .stdout(Stdio::piped());
  let mut grim =
    let_go(&mut grabbing).map_err(|fault| io::Error::other(format!("grim: {fault}")))?;
  let Ok(picture) = grim.take_stdout();
  let Ok(mut reading) = Program::Tesseract.command();

  match picture {
    Some(picture_2) => {
      reading.stdin(picture_2);
    }
    ::core::option::Option::None => {
      match grim.wait() {
        Ok(_reaped) => {}
        Err(fault) => {
          console_core_logging::warn!("waiting for grim after missing image stream: {fault}");
        }
      }

      return Err(io::Error::other("grim opened no image stream"));
    }
  }

  let text = reading
    .args(["stdin", "stdout", "--psm", "6", "-l", OCR_LANGUAGES])
    .output();
  drop(reading);
  let grabbed = grim.wait();
  let text_2 = text.map_err(|fault| io::Error::other(format!("tesseract: {fault}")))?;
  let grabbed_2 = grabbed?;

  match CommandOutcome::from(grabbed_2.success()) {
    CommandOutcome::Failed => {
      return Err(io::Error::other(format!(
        "grim exited with {grabbed}",
        grabbed = grabbed_2
      )));
    }
    CommandOutcome::Succeeded => {}
  }

  match CommandOutcome::from(text_2.status.success()) {
    CommandOutcome::Failed => {
      return Err(io::Error::other(format!(
        "tesseract exited with {}",
        text_2.status
      )));
    }
    CommandOutcome::Succeeded => {}
  }

  String::from_utf8(text_2.stdout).map_err(io::Error::other)
}

fn copy(text: &str) -> io::Result<()> {
  let Ok(mut copying) = InternalProgram::Clipboard.command();
  copying
    .arg(CLIPBOARD_COPY_TEXT.spelling)
    .stdin(Stdio::piped());
  let mut clipboard = let_go(&mut copying)?;
  let Ok(input) = clipboard.take_stdin();
  let written = match input {
    Some(mut input_2) => input_2.write_all(text.as_bytes()),
    None => Err(io::Error::other("clipboard opened no text stream")),
  };
  let status = clipboard.wait()?;
  written?;

  match CommandOutcome::from(status.success()) {
    CommandOutcome::Failed => {
      return Err(io::Error::other(format!(
        "clipboard copy exited with {status}"
      )));
    }
    CommandOutcome::Succeeded => {}
  }

  Ok(())
}

enum CommandOutcome {
  Succeeded,
  Failed,
}

impl From<bool> for CommandOutcome {
  fn from(succeeded: bool) -> Self {
    match bool::then_some(succeeded, ()) {
      Some(()) => Self::Succeeded,
      None => Self::Failed,
    }
  }
}

pub(super) fn notify(message: &str) -> Result<(), Never> {
  let options = console_operating_system_linux::notification::Options {
    app_name: Some("Console"),
    expire_ms: Some(2500),
    ..Default::default()
  };
  let Ok(mut saying) = options.command(message, None);
  let result = saying.stdin(Stdio::null()).status();

  match result {
    Ok(status) => match CommandOutcome::from(status.success()) {
      CommandOutcome::Succeeded => {}
      CommandOutcome::Failed => {
        console_core_logging::warn!("screen capture notification exited with {status}")
      }
    },
    Err(fault) => console_core_logging::warn!("screen capture notification: {fault}"),
  }

  Ok(())
}
