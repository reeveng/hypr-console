//! Boots Console with an xHCI keyboard and types through the guest's USB path.

use console_core_external_programs::Program;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

type Failure = Box<dyn std::error::Error>;

const POLL: Duration = Duration::from_millis(20);
const MONITOR_TIMEOUT: Duration = Duration::from_secs(25);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const SCREENSHOT: &str = "screen.ppm";

#[derive(Clone, Copy)]
enum Dma {
  Direct,
  AmdVi,
}

#[derive(Clone, Copy)]
enum Check {
  Keyboard(Dma),
  Calculator,
  Launcher,
}

struct Scratch(PathBuf);

impl Drop for Scratch {
  fn drop(&mut self) {
    let _cleanup = fs::remove_dir_all(&self.0);
  }
}

struct Guest(Child);

impl Drop for Guest {
  fn drop(&mut self) {
    let _signal = self.0.kill();
    let _reaped = self.0.wait();
  }
}

struct Monitor {
  commands: ChildStdin,
  messages: Receiver<std::io::Result<String>>,
}

impl Monitor {
  fn connect(guest: &mut Guest) -> Result<Self, Failure> {
    let commands = guest.0.stdin.take().ok_or("QEMU has no command pipe")?;
    let output = guest.0.stdout.take().ok_or("QEMU has no monitor output")?;
    let (sender, messages) = mpsc::sync_channel(8);
    // The receiver applies deadlines while the reader waits for complete
    // messages. Killing and reaping the guest closes the reader's pipe.
    thread::Builder::new().name("qmp".into()).spawn(move || {
      for message in BufReader::new(output).lines() {
        let failed = message.is_err();
        if sender.send(message).is_err() || failed {
          break;
        }
      }
    })?;
    let monitor = Self { commands, messages };
    let greeting = monitor.read_message(Instant::now() + MONITOR_TIMEOUT)?;
    if greeting.get("QMP").is_none() {
      return Err("QEMU sent no QMP greeting".into());
    }
    Ok(monitor)
  }

  fn read_message(&self, deadline: Instant) -> Result<Value, Failure> {
    let timeout = deadline.saturating_duration_since(Instant::now());
    if timeout.is_zero() {
      return Err("QEMU did not answer the monitor".into());
    }
    let line = match self.messages.recv_timeout(timeout) {
      Ok(message) => message?,
      Err(RecvTimeoutError::Timeout) => return Err("QEMU did not answer the monitor".into()),
      Err(RecvTimeoutError::Disconnected) => return Err("QEMU closed the monitor".into()),
    };
    Ok(serde_json::from_str(&line)?)
  }

  fn execute(&mut self, name: &str, arguments: Value) -> Result<(), Failure> {
    let deadline = Instant::now() + MONITOR_TIMEOUT;
    serde_json::to_writer(
      &mut self.commands,
      &json!({"execute": name, "arguments": arguments}),
    )?;
    self.commands.write_all(b"\n")?;
    loop {
      let response = self.read_message(deadline)?;
      if let Some(error) = response.get("error") {
        return Err(format!("QMP {name} failed: {error}").into());
      }
      if response.get("return").is_some() {
        return Ok(());
      }
    }
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

fn expect_line(
  guest: &mut Guest,
  serial: &Path,
  line: &str,
  count: usize,
  timeout: Duration,
) -> Result<(), Failure> {
  let expected = format!("console-kernel: {line}");
  let deadline = Instant::now() + timeout;
  loop {
    match fs::read(serial) {
      Ok(bytes) => {
        let seen = bytes
          .split(|byte| *byte == b'\n')
          .filter(|found| found.strip_suffix(b"\r").unwrap_or(found) == expected.as_bytes())
          .take(count)
          .count();
        if seen == count {
          return Ok(());
        }
      }
      Err(error) if error.kind() == ErrorKind::NotFound => {}
      Err(error) => return Err(error.into()),
    }
    if Instant::now() >= deadline || guest.0.try_wait()?.is_some() {
      return Err(format!("expected {count} occurrence(s) of {expected:?}").into());
    }
    thread::sleep(POLL);
  }
}

fn wait_for_exit(guest: &mut Guest) -> Result<ExitStatus, Failure> {
  let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
  while Instant::now() < deadline {
    if let Some(status) = guest.0.try_wait()? {
      return Ok(status);
    }
    thread::sleep(POLL);
  }
  Err("QEMU did not exit after Escape".into())
}

fn verify_guest(guest: &mut Guest, work: &Path, serial: &Path, dma: Dma) -> Result<(), Failure> {
  let mut monitor = Monitor::connect(guest)?;
  monitor.execute("qmp_capabilities", json!({}))?;
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
  if let Dma::AmdVi = dma {
    expect_line(
      guest,
      serial,
      "AMD-Vi DMA window active",
      1,
      SHUTDOWN_TIMEOUT,
    )?;
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
  if before == monitor.capture(&screen)? {
    return Err("the key events did not change the framebuffer".into());
  }

  // Two reports per keystroke, enough to wrap both xHCI rings.
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
  if let Dma::AmdVi = dma {
    expect_line(
      guest,
      serial,
      "AMD-Vi DMA window revoked",
      1,
      SHUTDOWN_TIMEOUT,
    )?;
  }
  let status = wait_for_exit(guest)?;
  if status.code() != Some(1) {
    return Err(format!("the kernel did not exit successfully ({status})").into());
  }

  println!(
    "USB keyboard press and release reached Console, changed the portrait framebuffer, and survived ring wrap."
  );
  Ok(())
}

fn verify_generation(guest: &mut Guest, serial: &Path, check: Check) -> Result<(), Failure> {
  let mut monitor = Monitor::connect(guest)?;
  monitor.execute("qmp_capabilities", json!({}))?;
  expect_line(guest, serial, "Console keyboard ready", 1, MONITOR_TIMEOUT)?;
  expect_line(guest, serial, "Console launcher", 1, SHUTDOWN_TIMEOUT)?;
  monitor.send_keys(&["c"])?;

  if let Check::Calculator = check {
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
    for (keys, shown) in [
      (&["1"][..], "1"),
      (&["2"][..], "12"),
      (&["shift", "equal"][..], "12+"),
      (&["3"][..], "12+3"),
      (&["4"][..], "12+34"),
      (&["ret"][..], "12+34 = 46"),
      (&["9"][..], "9"),
      (&["slash"][..], "9/"),
      (&["0"][..], "9/0"),
      (&["ret"][..], "9/0 = Error"),
      (&["esc"][..], "Console launcher"),
    ] {
      monitor.send_keys(keys)?;
      let count = if shown == "Console launcher" { 2 } else { 1 };
      expect_line(guest, serial, shown, count, SHUTDOWN_TIMEOUT)?;
    }
    thread::sleep(Duration::from_millis(50));
  }

  monitor.send_keys(&["esc"])?;
  let status = wait_for_exit(guest)?;
  if status.code() != Some(65) {
    return Err(format!("the generation did not exit successfully ({status})").into());
  }
  if let Check::Launcher = check {
    if fs::read_to_string(serial)?.contains("Calculator") {
      return Err("a disabled Calculator appeared in the session".into());
    }
  }
  println!("The native generation booted and honored programs.calculator.enable.");
  Ok(())
}

fn run(esp: &Path, work: &Path, check: Check) -> Result<(), Failure> {
  let esp = esp.to_str().ok_or("the ESP path must be UTF-8")?;
  let drive = format!(
    "if=none,id=preview,format=raw,file=fat:{},readonly=on",
    esp.replace(',', ",,")
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
    _ => command.args(["-device", "qemu-xhci,id=xhci"]),
  };
  let mut guest = Guest(
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
      .stdout(Stdio::piped())
      .spawn()
      .map_err(|fault| format!("could not start QEMU: {fault}"))?,
  );

  let serial = work.join("serial.log");
  let result = match check {
    Check::Keyboard(dma) => verify_guest(&mut guest, work, &serial, dma),
    Check::Calculator | Check::Launcher => verify_generation(&mut guest, &serial, check),
  };
  if result.is_err() {
    if let Ok(bytes) = fs::read(&serial) {
      eprint!("{}", String::from_utf8_lossy(&bytes));
    }
  }
  result
}

fn main() -> Result<(), Failure> {
  let mut args = std::env::args_os().skip(1);
  let usage = || {
    Error::new(
      ErrorKind::InvalidInput,
      "usage: kernel-usb ESP_DIRECTORY [amd-vi | calculator | launcher]",
    )
  };
  let esp = fs::canonicalize(args.next().ok_or_else(usage)?)?;
  let check = match args.next() {
    None => Check::Keyboard(Dma::Direct),
    Some(mode) if mode == "amd-vi" => Check::Keyboard(Dma::AmdVi),
    Some(mode) if mode == "calculator" => Check::Calculator,
    Some(mode) if mode == "launcher" => Check::Launcher,
    Some(_) => return Err(usage().into()),
  };
  if args.next().is_some() {
    return Err(usage().into());
  }
  let scratch_root =
    std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("target/kernel"), PathBuf::from);
  let scratch = Scratch(console_core_temporary_directories::private_under(
    &fs::canonicalize(scratch_root)?,
    "usb",
  )?);
  run(&esp, &scratch.0, check)
}
