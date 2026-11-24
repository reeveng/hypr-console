//! Boots Console with an xHCI keyboard and types through the guest's USB path.

use console_core_external_programs::Program;
use console_core_iteration::{Step, iterate};
use console_core_number_conversion::index;
use console_operating_system_linux::program_lifetime::{
  BoundToParent as Guest, TimedPipe, TimedReadPipe, alongside,
};
use console_waiting::clock::LiveClock;
use console_waiting::{Schedule, until_some_handed_on_result};
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Error, ErrorKind, Write};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

type Failure = Box<dyn std::error::Error>;

const POLL: Duration = Duration::from_millis(20);
const MONITOR_TIMEOUT: Duration = Duration::from_secs(25);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const SCREENSHOT: &str = "screen.ppm";
const MAX_QMP_LINE_BYTES: u32 = 65_536;

#[derive(Clone, Copy)]
enum Dma {
  Direct,
  AmdVi,
}

#[derive(Clone, Copy)]
enum Check {
  Keyboard(Dma),
  Browser,
  Calculator,
  Launcher,
}

struct Scratch(PathBuf);

impl Drop for Scratch {
  fn drop(&mut self) {
    let _cleanup = fs::remove_dir_all(&self.0);
  }
}

enum QmpMessage {
  Greeting,
  Success,
  Error(Box<RawValue>),
  EventOrUnknown,
}

struct Monitor {
  commands: TimedPipe,
  output: TimedReadPipe,
  received: Vec<u8>,
}

impl Monitor {
  fn connect(guest: &mut Guest) -> Result<Self, Failure> {
    let Ok(stdin) = guest.take_stdin();
    let Ok(stdout) = guest.take_stdout();
    let command = stdin.ok_or("QEMU has no command pipe")?;
    let responses = stdout.ok_or("QEMU has no monitor output")?;
    let commands = TimedPipe::new(OwnedFd::from(command))?;
    let output = TimedReadPipe::new(OwnedFd::from(responses))?;
    let mut monitor = Self {
      commands,
      output,
      received: Vec::new(),
    };
    let deadline = Instant::now()
      .checked_add(MONITOR_TIMEOUT)
      .ok_or("QMP deadline overflowed")?;

    let greeting = monitor.read_message(deadline)?;

    match greeting {
      QmpMessage::Greeting => {}
      QmpMessage::Success | QmpMessage::Error(_) | QmpMessage::EventOrUnknown => {
        return Err(Failure::from("QEMU sent no QMP greeting"));
      }
    }

    monitor.execute("qmp_capabilities", json!({}))?;
    Ok(monitor)
  }

  fn next_line(&mut self, deadline: Instant) -> Result<Vec<u8>, Failure> {
    let mut buffer = [0; 4096];
    let mut scanned = 0_u32;

    for _ in 0..=MAX_QMP_LINE_BYTES {
      let wait = deadline
        .checked_duration_since(Instant::now())
        .ok_or("QEMU did not answer the monitor")?;

      let Ok(start) = index(scanned);
      let new_bytes = self
        .received
        .get(start..)
        .ok_or("invalid QMP scan position")?;

      match new_bytes.iter().position(|&byte| byte == b'\n') {
        Some(newline) => {
          let newline_offset = u32::try_from(newline)?;
          let ending = scanned.saturating_add(newline_offset).saturating_add(1);
          let Ok(end) = index(ending);
          let line = self.received.drain(..end).collect();
          return Ok(line);
        }
        None => {}
      }

      let total = u32::try_from(self.received.len())?;
      scanned = total;
      let remaining = MAX_QMP_LINE_BYTES.saturating_sub(total).min(4096);

      match remaining {
        0 => break,
        _ => {}
      }

      let Ok(capacity) = index(remaining);
      let window = buffer
        .get_mut(..capacity)
        .ok_or("invalid QMP read capacity")?;
      let received = self.output.read_some(window, wait)?;

      match received {
        0 => return Err(Failure::from("QEMU closed the monitor")),
        _ => {}
      }

      let Ok(length) = index(received);
      let data = window.get(..length).ok_or("invalid QMP read length")?;
      self.received.extend_from_slice(data);
    }

    Err(Failure::from("QMP response line exceeds 64 KiB"))
  }

  fn read_message(&mut self, deadline: Instant) -> Result<QmpMessage, Failure> {
    let line = self.next_line(deadline)?;
    let mut members: BTreeMap<String, Box<RawValue>> = serde_json::from_slice(&line)?;

    Ok(match members.remove("error") {
      Some(error) => QmpMessage::Error(error),
      None => match (members.contains_key("return"), members.contains_key("QMP")) {
        (true, _) => QmpMessage::Success,
        (false, true) => QmpMessage::Greeting,
        (false, false) => QmpMessage::EventOrUnknown,
      },
    })
  }

  fn execute(&mut self, name: &str, arguments: Value) -> Result<(), Failure> {
    let deadline = Instant::now()
      .checked_add(MONITOR_TIMEOUT)
      .ok_or("QMP deadline overflowed")?;
    let mut request = serde_json::to_vec(&json!({"execute": name, "arguments": arguments}))?;
    request.push(b'\n');
    self.write_request(&request, deadline)?;

    iterate(self, |monitor| {
      Ok(match monitor.read_message(deadline) {
        Ok(QmpMessage::Success) => Step::Halt(Ok(())),
        Ok(QmpMessage::Error(fault)) => {
          Step::Halt(Err(Failure::from(format!("QMP {name} failed: {fault}"))))
        }
        Ok(QmpMessage::Greeting | QmpMessage::EventOrUnknown) => Step::Again(monitor),
        Err(fault) => Step::Halt(Err(fault)),
      })
    })?
  }

  fn write_request(&self, request: &[u8], deadline: Instant) -> Result<(), Failure> {
    let mut remaining = request;

    for _ in request {
      match remaining {
        [] => return Ok(()),
        [_, ..] => {}
      }

      let wait = deadline
        .checked_duration_since(Instant::now())
        .ok_or("QEMU did not read the monitor command")?;
      let written = self.commands.write_some(remaining, wait)?;

      match written {
        0 => return Err(Failure::from("QEMU did not read the monitor command")),
        _ => {}
      }

      let Ok(start) = index(written);
      let unwritten = remaining.get(start..).ok_or("invalid QMP write length")?;
      remaining = unwritten;
    }

    Ok(())
  }

  fn send_keys(&mut self, keys: &[&str]) -> Result<(), Failure> {
    let codes: Vec<_> = keys
      .iter()
      .map(|key| json!({"type": "qcode", "data": key}))
      .collect();
    self.execute("send-key", json!({"keys": codes, "hold-time": 30}))
  }

  fn capture(&mut self, screenshot: &Path) -> Result<Vec<u8>, Failure> {
    self.execute("stop", json!({}))?;
    let captured = self
      .execute("screendump", json!({"filename": SCREENSHOT}))
      .and_then(|()| fs::read(screenshot).map_err(Into::into));
    let resumed = self.execute("cont", json!({}));
    let image = captured?;
    resumed?;
    Ok(image)
  }
}

fn serial_occurrences(serial: &Path, expected: &[u8]) -> Result<u32, Failure> {
  let bytes = match fs::read(serial) {
    Ok(bytes) => bytes,
    Err(fault) => match (fault.kind() == ErrorKind::NotFound).then_some(()) {
      Some(()) => return Ok(0),
      None => return Err(Failure::from(fault)),
    },
  };
  let seen = bytes
    .split(|&byte| byte == b'\n')
    .filter(|line| {
      let without_cr = match line.strip_suffix(b"\r") {
        Some(stripped) => stripped,
        None => line,
      };
      without_cr == expected
    })
    .fold(0_u32, |count, _| count.saturating_add(1));

  Ok(seen)
}

fn expect_line(
  guest: &mut Guest,
  serial: &Path,
  line: &str,
  count: u32,
  timeout: Duration,
) -> Result<(), Failure> {
  let expected = format!("console-kernel: {line}");
  let Ok(patience) = Schedule::asking_every(timeout, POLL);
  let Ok(mut clock) = LiveClock::started();
  let found = until_some_handed_on_result(&mut clock, patience, guest, |active| {
    let seen = serial_occurrences(serial, expected.as_bytes())?;

    match (seen >= count).then_some(()) {
      Some(()) => return Ok(Some(())),
      None => {}
    }

    let running = active.try_wait()?;

    match running {
      Some(status) => Err(Failure::from(format!(
        "QEMU exited ({status}) before {count} occurrence(s) of {expected:?}"
      ))),
      None => Ok(None),
    }
  })?;
  found.ok_or_else(|| Failure::from(format!("expected {count} occurrence(s) of {expected:?}")))
}

fn wait_for_exit(guest: &mut Guest) -> Result<ExitStatus, Failure> {
  let Ok(patience) = Schedule::asking_every(SHUTDOWN_TIMEOUT, POLL);
  let Ok(mut clock) = LiveClock::started();
  let status =
    until_some_handed_on_result(&mut clock, patience, guest, |active| active.try_wait())?;

  status.ok_or_else(|| Failure::from("QEMU did not exit after Escape"))
}

fn verify_guest(guest: &mut Guest, work: &Path, serial: &Path, dma: Dma) -> Result<(), Failure> {
  let mut monitor = Monitor::connect(guest)?;
  expect_line(
    guest,
    serial,
    "USB keyboard ready. Press keys to see events. Escape exits.",
    1,
    Duration::from_secs(45),
  )?;
  expect_line(
    guest,
    serial,
    "the panel is 1600 by 2560, and boot progress is drawn on it",
    1,
    SHUTDOWN_TIMEOUT,
  )?;

  match dma {
    Dma::AmdVi => {
      expect_line(
        guest,
        serial,
        "AMD-Vi DMA window active",
        1,
        SHUTDOWN_TIMEOUT,
      )?;
    }
    Dma::Direct => {}
  }

  let screen = work.join(SCREENSHOT);
  let before = monitor.capture(&screen)?;
  monitor.send_keys(&["shift", "a"])?;
  expect_line(
    guest,
    serial,
    "key pressed: A (USB 0x04)",
    1,
    SHUTDOWN_TIMEOUT,
  )?;
  expect_line(guest, serial, "key released: USB 0x04", 1, SHUTDOWN_TIMEOUT)?;

  let after = monitor.capture(&screen)?;

  match (before == after).then_some(()) {
    ::core::option::Option::Some(()) => {
      return Err(Failure::from(
        "the key events did not change the framebuffer",
      ));
    }
    ::core::option::Option::None => {}
  }

  for count in 1..=35 {
    monitor.send_keys(&["b"])?;
    expect_line(
      guest,
      serial,
      "key pressed: b (USB 0x05)",
      count,
      SHUTDOWN_TIMEOUT,
    )?;
    expect_line(
      guest,
      serial,
      "key released: USB 0x05",
      count,
      SHUTDOWN_TIMEOUT,
    )?;
  }

  monitor.send_keys(&["esc"])?;
  expect_line(
    guest,
    serial,
    "USB keyboard test ended",
    1,
    SHUTDOWN_TIMEOUT,
  )?;

  match dma {
    Dma::AmdVi => {
      expect_line(
        guest,
        serial,
        "AMD-Vi DMA window revoked",
        1,
        SHUTDOWN_TIMEOUT,
      )?;
    }
    Dma::Direct => {}
  }

  let status = wait_for_exit(guest)?;

  match { ::core::primitive::bool::then_some(status.code() != Some(1), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(Failure::from(format!(
        "the kernel did not exit successfully ({status})"
      )));
    }
    ::core::option::Option::None => {}
  }

  println!(
    "USB keyboard press and release reached Console, changed the portrait framebuffer, and survived ring wrap."
  );
  Ok(())
}

struct BrowserFrames {
  opened: Vec<u8>,
  scrolled: Vec<u8>,
}

fn verify_browser(guest: &mut Guest, work: &Path, serial: &Path) -> Result<(), Failure> {
  let mut monitor = Monitor::connect(guest)?;
  expect_line(
    guest,
    serial,
    "F2 opens the native browser.",
    1,
    MONITOR_TIMEOUT,
  )?;

  let screen = work.join(SCREENSHOT);
  let frames = open_and_scroll(&mut monitor, guest, serial, &screen)?;
  verify_renderer_restart(&mut monitor, guest, serial, &screen, &frames)?;
  monitor.send_keys(&["esc"])?;
  expect_line(
    guest,
    serial,
    "Native browser closed. F2 opens it again.",
    1,
    SHUTDOWN_TIMEOUT,
  )?;
  monitor.send_keys(&["a"])?;
  expect_line(
    guest,
    serial,
    "key pressed: a (USB 0x04)",
    1,
    SHUTDOWN_TIMEOUT,
  )?;
  monitor.send_keys(&["esc"])?;
  let status = wait_for_exit(guest)?;

  match status.code() {
    Some(1) => {}
    Some(_) | None => {
      return Err(Failure::from(format!(
        "the kernel did not exit successfully ({status})"
      )));
    }
  }

  println!("The native HTML page scrolled and kept its viewport across renderer faults.");
  Ok(())
}

fn open_and_scroll(
  monitor: &mut Monitor,
  guest: &mut Guest,
  serial: &Path,
  screen: &Path,
) -> Result<BrowserFrames, Failure> {
  let preview = monitor.capture(screen)?;
  monitor.send_keys(&["f2"])?;
  expect_line(
    guest,
    serial,
    "Native browser opened the local HTML page",
    1,
    SHUTDOWN_TIMEOUT,
  )?;
  let opened = monitor.capture(screen)?;

  match (opened == preview).then_some(()) {
    Some(()) => return Err(Failure::from("the local HTML page did not appear")),
    None => {}
  }

  monitor.send_keys(&["down"])?;
  expect_line(
    guest,
    serial,
    "Native browser scrolled",
    1,
    SHUTDOWN_TIMEOUT,
  )?;
  let scrolled = monitor.capture(screen)?;

  match (scrolled == opened).then_some(()) {
    Some(()) => return Err(Failure::from("scrolling did not change the framebuffer")),
    None => {}
  }

  Ok(BrowserFrames { opened, scrolled })
}

fn verify_renderer_restart(
  monitor: &mut Monitor,
  guest: &mut Guest,
  serial: &Path,
  screen: &Path,
  frames: &BrowserFrames,
) -> Result<(), Failure> {
  for count in 1..=2 {
    monitor.send_keys(&["r"])?;
    expect_line(
      guest,
      serial,
      "Native browser replaced the faulted renderer",
      count,
      SHUTDOWN_TIMEOUT,
    )?;
    let restored = monitor.capture(screen)?;

    match (restored == frames.scrolled).then_some(()) {
      Some(()) => {}
      None => {
        return Err(Failure::from(
          "the replacement renderer did not restore the scrolled frame",
        ));
      }
    }
  }

  monitor.send_keys(&["up"])?;
  expect_line(
    guest,
    serial,
    "Native browser scrolled",
    2,
    SHUTDOWN_TIMEOUT,
  )?;
  let restored = monitor.capture(screen)?;

  match (restored == frames.opened).then_some(()) {
    Some(()) => Ok(()),
    None => Err(Failure::from(
      "the replacement renderer could not scroll back",
    )),
  }
}

#[cfg_attr(
  dylint_lib = "explicit021_no_sleeping",
  expect(
    explicit021_no_sleeping,
    reason = "QMP releases a send-key event after its hold interval; the second Escape must follow the first release"
  )
)]
fn wait_for_escape_release() -> Result<(), console_core_never::Never> {
  thread::sleep(Duration::from_millis(50));
  Ok(())
}

fn verify_generation(guest: &mut Guest, serial: &Path, check: Check) -> Result<(), Failure> {
  let mut monitor = Monitor::connect(guest)?;
  expect_line(guest, serial, "Console keyboard ready", 1, MONITOR_TIMEOUT)?;
  expect_line(guest, serial, "Console launcher", 1, SHUTDOWN_TIMEOUT)?;
  monitor.send_keys(&["c"])?;

  match check {
    Check::Calculator => {
      expect_line(
        guest,
        serial,
        "C: Calculator | Esc: Exit",
        1,
        SHUTDOWN_TIMEOUT,
      )?;
      expect_line(
        guest,
        serial,
        "Calculator opened. Type 12+34 and press Enter. Esc returns.",
        1,
        SHUTDOWN_TIMEOUT,
      )?;

      let sequence: &[(&[&str], &str)] = &[
        (&["1"], "1"),
        (&["2"], "12"),
        (&["shift", "equal"], "12+"),
        (&["3"], "12+3"),
        (&["4"], "12+34"),
        (&["ret"], "12+34 = 46"),
        (&["9"], "9"),
        (&["slash"], "9/"),
        (&["0"], "9/0"),
        (&["ret"], "9/0 = Error"),
        (&["esc"], "Console launcher"),
      ];
      for &(keys, shown) in sequence {
        monitor.send_keys(keys)?;
        let count = match shown {
          "Console launcher" => 2,
          _ => 1,
        };
        expect_line(guest, serial, shown, count, SHUTDOWN_TIMEOUT)?;
      }

      let Ok(()) = wait_for_escape_release();
    }
    Check::Keyboard(_) | Check::Browser | Check::Launcher => {}
  }

  monitor.send_keys(&["esc"])?;
  let status = wait_for_exit(guest)?;

  match { ::core::primitive::bool::then_some(status.code() != Some(65), ()) } {
    ::core::option::Option::Some(()) => {
      return Err(Failure::from(format!(
        "the generation did not exit successfully ({status})"
      )));
    }
    ::core::option::Option::None => {}
  }

  match check {
    Check::Launcher => {
      let printed = fs::read_to_string(serial)?;

      match printed.contains("Calculator").then_some(()) {
        ::core::option::Option::Some(()) => {
          return Err(Failure::from(
            "a disabled Calculator appeared in the session",
          ));
        }
        ::core::option::Option::None => {}
      }
    }
    Check::Keyboard(_) | Check::Browser | Check::Calculator => {}
  }

  println!("The native generation booted and honored programs.calculator.enable.");
  Ok(())
}

fn run(esp: &Path, work: &Path, check: Check) -> Result<(), Failure> {
  let esp_2 = esp.to_str().ok_or("the ESP path must be UTF-8")?;
  let drive = format!(
    "if=none,id=preview,format=raw,file=fat:{},readonly=on",
    esp_2.replace(',', ",,")
  );
  let Ok(mut command) = Program::QemuX86_64.command();

  command.current_dir(work).args([
    "-machine",
    "q35",
    "-cpu",
    "qemu64",
    "-m",
    "256M",
    "-bios",
    "/usr/share/edk2/x64/OVMF.4m.fd",
    "-drive",
    &drive,
  ]);
  match check {
    Check::Keyboard(Dma::AmdVi) => command.args([
      "-device",
      "amd-iommu,dma-remap=on,intremap=off",
      "-device",
      "pcie-root-port,id=usb-root,chassis=1,slot=1",
      "-device",
      "qemu-xhci,id=xhci,bus=usb-root",
    ]),
    Check::Keyboard(Dma::Direct) | Check::Browser | Check::Calculator | Check::Launcher => {
      command.args(["-device", "qemu-xhci,id=xhci"])
    }
  };

  command
    .args([
      "-device",
      "usb-storage,bus=xhci.0,drive=preview,removable=on,bootindex=1",
      "-device",
      "usb-kbd,bus=xhci.0",
      "-serial",
      "file:serial.log",
      "-display",
      "none",
      "-qmp",
      "stdio",
      "-nic",
      "none",
      "-vga",
      "none",
      "-device",
      "VGA,vgamem_mb=32,xres=1600,yres=2560,xmax=1600,ymax=2560",
      "-device",
      "isa-debug-exit,iobase=0xf4,iosize=0x04",
      "-no-reboot",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped());
  let mut guest =
    alongside(&mut command).map_err(|fault| format!("could not start QEMU: {fault}"))?;

  let serial = work.join("serial.log");
  let result = match check {
    Check::Keyboard(dma) => verify_guest(&mut guest, work, &serial, dma),
    Check::Browser => verify_browser(&mut guest, work, &serial),
    Check::Calculator | Check::Launcher => verify_generation(&mut guest, &serial, check),
  };

  match result {
    Err(fault) => {
      match fs::read(&serial) {
        Ok(bytes) => match std::io::stderr().write_all(&bytes) {
          Ok(()) => {}
          Err(output) => eprintln!("could not display QEMU serial log: {output}"),
        },
        Err(read) => eprintln!("could not read QEMU serial log: {read}"),
      }

      Err(fault)
    }
    Ok(()) => Ok(()),
  }
}

fn main() -> Result<(), Failure> {
  let mut args = std::env::args_os().skip(1);
  let usage = || {
    Error::new(
      ErrorKind::InvalidInput,
      "usage: kernel-usb ESP_DIRECTORY [amd-vi | browser | calculator | launcher]",
    )
  };
  let esp_argument = args.next().ok_or_else(usage)?;
  let esp = fs::canonicalize(esp_argument)?;
  let mode = args.next();
  let check = match mode.as_deref().and_then(std::ffi::OsStr::to_str) {
    Some("amd-vi") => Check::Keyboard(Dma::AmdVi),
    Some("browser") => Check::Browser,
    Some("calculator") => Check::Calculator,
    Some("launcher") => Check::Launcher,
    None => match mode {
      Some(_) => return Err(Failure::from(usage())),
      None => Check::Keyboard(Dma::Direct),
    },
    Some(_) => return Err(Failure::from(usage())),
  };

  match args.next() {
    Some(_) => return Err(Failure::from(usage())),
    None => {}
  }

  let Ok(configured) = console_core_temporary_directories::configured_root();
  let scratch_root = match configured {
    Some(path) => path,
    None => PathBuf::from("target/kernel"),
  };
  let canonical_root = fs::canonicalize(scratch_root)?;
  let directory = console_core_temporary_directories::private_under(&canonical_root, "usb")?;
  let scratch = Scratch(directory);
  run(&esp, &scratch.0, check)
}
