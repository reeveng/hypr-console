//! Where the words come from, one subscription each.
//!
//! The one that is left out is left out for a reason rather than for want of an
//! afternoon. A source is held on the first `listen` for its event group and
//! never before, so an event group no one is listening to costs nothing at all:
//! what decides whether one belongs here is whether the thing that says a
//! change has happened already exists as a program, not whether anyone is
//! asking yet. What an event group with no source does is say so on the journal
//! and hand out nothing, which is an event group that is quiet rather than a
//! pool that is broken.
//!
//! **`Units` has no source because nothing on this machine emits one.** systemd
//! sends `UnitNew`, `JobRemoved` and the rest only while some connection is
//! holding a `Subscribe` open, and a monitor sees what is sent rather than
//! asking for it -- so a `busctl monitor` here would sit watching a bus that
//! stays silent and report a machine where no unit ever changes, which is worse
//! than an event group that says it is quiet. What it wants is a connection
//! that subscribes and stays, and that is a program rather than a line in this
//! file.
//!
//! **`Path` is a folder, and the kernel is its source.** Every other event
//! group here is one watcher for the whole machine; a path is a watcher per
//! path, and that fits the same shape because an `EventGroup::Path` names its
//! folder: two folders are two event groups, held once each like any other.
//! `watching` is the watcher. A path that is not absolute is not a folder
//! anyone can mean, and is refused rather than read against wherever the pool
//! was started.
//!
//! Nothing a program says is passed on. There was one event group a program
//! could publish on, a download finishing, and it was a program speaking for
//! the machine about one kind of change; the folder the download landed in says
//! the same thing now, for every kind of change and every program, and a pool
//! that took words from programs was a pool that had to decide which of them to
//! believe.
//!
//! **What the bus watches is narrowed where it is asked rather than where it is
//! read.** `busctl monitor` given a name is not filtered to that name -- traffic
//! to the music player turns up in a monitor of the notification service -- so
//! without a match the pool would relay every message on the session bus to
//! whoever asked about notifications. The match is the pool spelling what it *asks*,
//! which is its own business the way `pactl subscribe` and `nmcli monitor`
//! already are; what an answer means is still the subscriber's, and the names
//! are exported so that the one filter that reads them does not spell them a
//! second time.
//!
//! **Bluetooth is a connection of the pool's own, like the compositor.** It
//! was `gdbus monitor --dest org.bluez`, because `busctl --system monitor`
//! asks the bus to make it a monitor and the system bus answers that for root
//! alone, and gdbus asked for nothing so large: an ordinary match for the
//! signals one name sends, which any connection may add. That is still what
//! is asked -- `console-bus` adds the same match itself now -- and `bluetooth`
//! reads each signal as the message it is rather than as gdbus's printing of
//! it, which is also where the last of the narrowing happens: a match rule
//! cannot say which keys a changed dictionary holds.
//!
//! **The Wi-Fi is NetworkManager on the system bus, heard through
//! `gdbus monitor` for the reason Bluetooth was.**
//! `nmcli monitor` is `Network`, and it says a device connected and never how
//! strong anything is. What the bus says that it does not is every access
//! point's strength and the moment a scan finished, which is when a list of
//! what is in range is new rather than remembered. It is an event group of its
//! own because the bar wakes on every line of `Network`, and a street full of
//! access points being seen again is not a line it should wake for.
//!
//! **The battery is the kernel's `power_supply` uevents, which is a netlink
//! socket.** `udevadm monitor` on the subsystem rather than on a name, because
//! what changes when this device is plugged in is a USB-C supply one day and
//! the adapter another. It is the one source here that is not a socket in the
//! runtime directory or a bus, and so the one that asked the unit for
//! `AF_NETLINK`, which is a socket to the kernel and still no way off this
//! machine. UPower would have said the same thing on the bus, and it is not in
//! `[packages]`: a daemon installed so that one line could be read from it is a
//! daemon polling the battery so that nothing here has to, which is what the
//! bar's tick already is.
//!
//! **A source that is someone else's program is started `alongside`.** The
//! pool is the only thing holding it, so it dies when the pool does -- by a
//! death signal and by a drop, because a pool that was killed outright must not
//! leave a `pactl subscribe` behind it. Leaving one behind is the exact fault
//! this crate exists to end, and it would be a poor joke to commit it here.
//!
//! **What a source says is not read.** `pactl subscribe` writes a line per
//! event and the line is passed on whole; whoever asked for `Sound` is the one
//! that knows a sink from a source, and the pool that decided would be
//! deciding for every listener at once.
//!
//! What the sound was watched doing, under the unit's own confinement rather
//! than in a shell. `PrivateDevices=yes` and `RestrictAddressFamilies=AF_UNIX`
//! do not stop it: `pactl` reaches the server over a socket in the runtime
//! directory and never opens `/dev/snd`, so the sandbox the pool already had
//! is the sandbox this runs in. Killed outright it is started again and
//! whoever was listening goes on hearing without reconnecting, because the
//! reaching-again is on this side of the pool rather than the far side. And a
//! pool killed with `-9` leaves nothing behind, which was the thing to check
//! rather than assume.

use std::fmt;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::sync::mpsc::SyncSender;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_reconnect::{Round, keep};
use console_core_text_encodings::decode;
use console_operating_system_linux::program_lifetime::alongside;
use console_program_contract::{Change, EventGroup};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subscribed {
  Yes,
  No,
}

pub const NOTIFICATIONS: &str = "org.freedesktop.Notifications";

pub const APPLICATION: &str = "console.Notifications";

pub const PLAYERS: &str = "/org/mpris/MediaPlayer2";

pub const NETWORK_MANAGER: &str = "org.freedesktop.NetworkManager";

pub const SCANNED: &str = "'LastScan'";

pub fn handed_to(
  waiting: Option<&SyncSender<SyncSender<Change>>>,
  wanted: &EventGroup,
  event_group: &EventGroup,
  say: SyncSender<Change>,
) -> Result<Subscribed, Never> {
  Ok(match (event_group == wanted, waiting) {
    (true, Some(waiting_2)) => match waiting_2.send(say) {
      Ok(()) => Subscribed::Yes,
      Err(_nobody_waiting_any_more) => Subscribed::No,
    },
    (true, None) | (false, _) => Subscribed::No,
  })
}

pub fn hold(event_group: &EventGroup, say: SyncSender<Change>) -> Result<Subscribed, Never> {
  Ok(match event_group {
    EventGroup::Compositor => {
      let Ok(()) = compositor(say);

      Subscribed::Yes
    }
    EventGroup::Sound => {
      let Ok(arguments) = to_strings(&["subscribe"]);
      let Ok(()) = once(EventGroup::Sound, Program::Pactl, arguments, say);

      Subscribed::Yes
    }
    EventGroup::Network => {
      let Ok(arguments) = to_strings(&["monitor"]);
      let Ok(()) = once(EventGroup::Network, Program::Nmcli, arguments, say);

      Subscribed::Yes
    }
    EventGroup::Wifi => {
      let Ok(gdbus) = Program::Gdbus.name();
      let Ok(arguments) = to_strings(&[
        "-oL",
        gdbus,
        "monitor",
        "--system",
        "--dest",
        NETWORK_MANAGER,
      ]);
      let Ok(()) = once(EventGroup::Wifi, Program::Stdbuf, arguments, say);

      Subscribed::Yes
    }
    EventGroup::Bluetooth => {
      let Ok(()) = crate::bluetooth::hold(say);

      Subscribed::Yes
    }
    EventGroup::Battery => {
      let Ok(arguments) = to_strings(&["monitor", "--udev", "--subsystem-match=power_supply"]);
      let Ok(()) = once(EventGroup::Battery, Program::Udevadm, arguments, say);

      Subscribed::Yes
    }
    EventGroup::Notifications => {
      let Ok(arguments) = monitoring(&[
        format!("--match=interface={NOTIFICATIONS}"),
        format!("--match=interface={APPLICATION}"),
      ]);
      let Ok(()) = once(EventGroup::Notifications, Program::Stdbuf, arguments, say);

      Subscribed::Yes
    }
    EventGroup::Player => {
      let Ok(arguments) = monitoring(&[format!("--match=path={PLAYERS}")]);
      let Ok(()) = once(EventGroup::Player, Program::Stdbuf, arguments, say);

      Subscribed::Yes
    }
    EventGroup::Units => Subscribed::No,
    EventGroup::Path(folder) => match ::core::primitive::bool::then_some(folder.is_absolute(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = crate::watching::watch(folder.clone(), say);

        Subscribed::Yes
      }
      ::core::option::Option::None => Subscribed::No,
    },
  })
}

fn to_strings(arguments: &[&str]) -> Result<Vec<String>, Never> {
  Ok(arguments.iter().map(|word| (*word).to_string()).collect())
}

fn monitoring(matches: &[String]) -> Result<Vec<String>, Never> {
  let Ok(busctl) = Program::Busctl.name();
  let Ok(mut arguments) = to_strings(&["-oL", busctl, "--user", "monitor"]);

  arguments.extend(matches.iter().cloned());

  Ok(arguments)
}

enum Outage {
  Unreported,
  Reported,
}

impl Outage {
  fn retry(&mut self, source: &EventGroup, cause: fmt::Arguments<'_>) -> Result<Round, Never> {
    match self {
      Self::Unreported => {
        console_core_logging::warn!("{source:?} subscription interrupted: {cause}; retrying");
        *self = Self::Reported;
      }
      Self::Reported => {}
    }

    Ok(Round::Another)
  }
}

fn compositor(say: SyncSender<Change>) -> Result<(), Never> {
  let mut outage = Outage::Unreported;

  let Ok(()) = keep(move || {
    let socket = match console_onscreen::events() {
      Ok(socket) => socket,
      Err(fault) => {
        let Ok(round) = outage.retry(&EventGroup::Compositor, format_args!("{fault}"));

        return round;
      }
    };

    let stream = match UnixStream::connect(&socket) {
      Ok(stream) => stream,
      Err(fault) => {
        let Ok(round) = outage.retry(
          &EventGroup::Compositor,
          format_args!("opening {}: {fault}", socket.display()),
        );

        return round;
      }
    };

    let Ok(round) = passed_on(
      BufReader::new(stream),
      &EventGroup::Compositor,
      &say,
      &mut outage,
    );

    round
  });

  Ok(())
}

fn once(
  about: EventGroup,
  program: Program,
  arguments: Vec<String>,
  say: SyncSender<Change>,
) -> Result<(), Never> {
  let mut outage = Outage::Unreported;

  let Ok(()) = keep(move || {
    let Ok(mut asking) = program.command();
    let Ok(name) = program.name();

    asking
      .args(&arguments)
      .stdout(Stdio::piped())
      .stderr(Stdio::null());

    let mut running = match alongside(&mut asking) {
      Ok(running) => running,
      Err(fault) => {
        let Ok(round) = outage.retry(&about, format_args!("starting {name}: {fault}"));

        return round;
      }
    };

    let Ok(stdout) = running.take_stdout();
    let reading = match stdout {
      Some(stream) => stream,
      None => {
        let Ok(round) = outage.retry(&about, format_args!("{name} has no event stream"));

        return round;
      }
    };

    let Ok(round) = passed_on(BufReader::new(reading), &about, &say, &mut outage);

    round
  });

  Ok(())
}

fn passed_on(
  reading: impl BufRead,
  event_group: &EventGroup,
  say: &SyncSender<Change>,
  outage: &mut Outage,
) -> Result<Round, Never> {
  for read in reading.split(b'\n') {
    let line = match read {
      Ok(line) => line,
      Err(fault) => return outage.retry(event_group, format_args!("reading events: {fault}")),
    };
    let ended = match line.strip_suffix(b"\r") {
      Some(before) => before,
      None => &line,
    };
    let Ok(text) = decode(ended);
    let sent = say.send(Change {
      event_group: event_group.clone(),
      text: text.text.into_owned(),
    });

    match sent {
      Ok(()) => *outage = Outage::Unreported,
      Err(_no_one_is_listening) => return Ok(Round::Finished),
    }
  }

  outage.retry(event_group, format_args!("the event stream closed"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::mpsc::sync_channel;

  #[test]
  fn a_line_that_is_not_utf8_is_passed_on_and_so_is_every_line_after_it() {
    let said: &[u8] =
      b"windowtitlev2>>5634f2a0,tradu\xe7\xe3ocoletiva.docx\nactivewindowv2>>5634f2a0\n";
    let (say, heard) = sync_channel(2);
    let ended = passed_on(said, &EventGroup::Compositor, &say, &mut Outage::Unreported);
    let lines: Vec<String> = heard.try_iter().map(|change| change.text).collect();

    assert_eq!(ended, Ok(Round::Another));
    assert_eq!(
      lines,
      [
        "windowtitlev2>>5634f2a0,traduçãocoletiva.docx",
        "activewindowv2>>5634f2a0"
      ]
    );
  }

  #[test]
  fn a_line_is_passed_on_without_its_ending() {
    let said: &[u8] = b"workspacev2>>2,2\r\nfocusedmonv2>>eDP-1,2";
    let (say, heard) = sync_channel(2);
    let ended = passed_on(said, &EventGroup::Compositor, &say, &mut Outage::Unreported);
    let lines: Vec<String> = heard.try_iter().map(|change| change.text).collect();

    assert_eq!(ended, Ok(Round::Another));
    assert_eq!(lines, ["workspacev2>>2,2", "focusedmonv2>>eDP-1,2"]);
  }
}
