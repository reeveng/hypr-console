//! Run the background layer or send one request to the session's renderer.

mod display;
mod picture;
mod play;

use std::ffi::OsString;
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::os::unix::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_state_machine::Machine;
use console_paper::{self as paper, Motion, Request, Socket};

use display::Wall;
use picture::Drawing;
use play::{Effect, Event, Paper, Playback};

enum Command {
  Serve([u8; 3]),
  Ask(Request),
}

fn main() -> ExitCode {
  let words: Vec<OsString> = std::env::args_os().skip(1).collect();

  match command(&words).and_then(run) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::error!("console-paper: {fault}");
      ExitCode::FAILURE
    }
  }
}

fn command(words: &[OsString]) -> io::Result<Command> {
  let verb = words.first().and_then(|word| word.to_str());

  match (verb, words.get(1..)) {
    (Some("query"), Some([])) => Ok(Command::Ask(Request::Query)),
    (Some("quit"), Some([])) => Ok(Command::Ask(Request::Quit)),
    (Some("serve"), Some([color])) => parse_color(color).map(Command::Serve),
    (Some("clear"), Some([color])) => {
      parse_color(color).map(|rgb| Command::Ask(Request::Clear(rgb)))
    }
    (Some("img"), Some([path])) => {
      let full = std::path::absolute(PathBuf::from(path))?;
      Ok(Command::Ask(Request::Show(full, Motion::Play)))
    }
    _ => Err(io::Error::new(
      io::ErrorKind::InvalidInput,
      "usage: console-paper serve RRGGBB | img FILE.webp | clear RRGGBB | query | quit",
    )),
  }
}

fn parse_color(color: &std::ffi::OsStr) -> io::Result<[u8; 3]> {
  let checked_value = color
    .to_str()
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "expected an RGB color"))?;
  paper::color(checked_value)
}

fn run(command: Command) -> io::Result<()> {
  match command {
    Command::Ask(request) => {
      let reply = paper::ask(&request)?;
      io::stdout().lock().write_all(reply.as_bytes())
    }
    Command::Serve(color) => {
      let mut desktop = Desktop::open(color)?;
      desktop.serve()
    }
  }
}

struct Desktop {
  wall: Wall,
  socket: Socket,
  drawing: Drawing,
  playback: Playback,
}

enum Turn {
  Next,
  Stop,
  Report(Event),
}

fn now() -> Result<Instant, Never> {
  Ok(Instant::now())
}

enum Running {
  Wait,
  Stop,
}

impl Desktop {
  fn open(color: [u8; 3]) -> io::Result<Self> {
    let wall = Wall::open()?;
    let socket = Socket::open()?;
    let Ok(drawing) = Drawing::new(color);
    let Ok(initial) = Paper::initial_transition(&(), None);
    console_readiness::ready()?;
    Ok(Self {
      wall,
      socket,
      drawing,
      playback: initial.state,
    })
  }

  fn serve(&mut self) -> io::Result<()> {
    match iterate(self, |desktop| {
      Ok(match desktop.turn() {
        Ok(Running::Wait) => Step::Again(desktop),
        Ok(Running::Stop) => Step::Halt(Ok(())),
        Err(fault) => Step::Halt(Err(fault)),
      })
    }) {
      Ok(result) => result,
      Err(fault) => Err(io::Error::other(fault)),
    }
  }

  fn turn(&mut self) -> io::Result<Running> {
    let received = self.socket.receive()?;

    match received {
      Some((peer, request)) => {
        let handling = self.respond(Event::Asked(request), Some(&peer))?;

        match handling {
          Turn::Stop => return Ok(Running::Stop),
          Turn::Next | Turn::Report(_) => {}
        }
      }
      None => {}
    }

    let Ok(display) = self.wall.observe(&self.drawing);
    let Ok(woke) = now();
    let handling = self.respond(Event::Woke(woke, display), None)?;

    match handling {
      Turn::Stop => return Ok(Running::Stop),
      Turn::Next | Turn::Report(_) => {}
    }

    let Ok(availability) = self.wall.observe(&self.drawing);
    let Ok(sample) = now();
    let Ok(wait) = self.playback.wait(availability, sample);
    self.wall.wait(self.socket.channel.as_fd(), wait)?;
    Ok(Running::Wait)
  }

  fn respond(&mut self, event: Event, peer: Option<&SocketAddr>) -> io::Result<Turn> {
    let mut next = event;

    for _ in 0..3 {
      let Ok(step) = Paper::transition(self.playback, next);
      self.playback = step.state;
      let mut report = None;

      for action in step.effects {
        let result = self.effect(action, peer)?;

        match result {
          Turn::Next => {}
          Turn::Stop => return Ok(Turn::Stop),
          Turn::Report(event_2) => report = Some(event_2),
        }
      }

      match report {
        Some(event_3) => next = event_3,
        None => return Ok(Turn::Next),
      }
    }

    Err(io::Error::other("wallpaper effect did not settle"))
  }

  fn effect(&mut self, effect: Effect, peer: Option<&SocketAddr>) -> io::Result<Turn> {
    match effect {
      Effect::Load(path, motion) => {
        let Ok(result) = self.drawing.load(path, motion);
        let Ok(sample) = now();
        return Ok(Turn::Report(Event::Loaded(result, sample)));
      }
      Effect::Clear(color) => {
        let Ok(()) = self.drawing.clear(color);
      }
      Effect::Present => self.wall.present(&self.drawing)?,
      Effect::Advance => {
        let Ok(timing) = self.drawing.advance();
        let Ok(sample) = now();
        return Ok(Turn::Report(Event::Advanced(timing, sample)));
      }
      Effect::ReadStatus => {
        let Ok(status) = self.wall.status();
        self.reply(peer, Ok(&status))?;
      }
      Effect::Reply(result) => {
        self.reply(peer, result.as_ref().map(|_| "").map_err(String::as_str))?
      }
      Effect::Stop => return Ok(Turn::Stop),
    }

    Ok(Turn::Next)
  }

  fn reply(&self, peer: Option<&SocketAddr>, response: Result<&str, &str>) -> io::Result<()> {
    match peer {
      Some(client) => {
        let Ok(()) = self.socket.reply(client, response);
        Ok(())
      }
      None => Err(io::Error::other("a wallpaper reply needs a client")),
    }
  }
}
