//! A local browser fixture and serial-console check for the NixOS guest.

#![forbid(unsafe_code)]

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use console_repository::RelativePath;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;
use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

const PASSED: &str = "CONSOLE_BROWSER_CHECK=passed";
const PAUSE: Duration = Duration::from_millis(20);
const WAIT_WAKEUPS: u32 = 65_536;

const PAGE: &[u8] = br#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>Waiting for the page script</title>
    <style>
      h1 { color: rgb(1, 2, 3); }
      :root { background: rgb(245, 245, 245); }
      @media (prefers-color-scheme: dark) {
        :root { background: rgb(12, 34, 56); color-scheme: only dark; }
      }
      body { min-height: 2500px; }
      input { position: fixed; left: 10px; top: 20px; width: 300px; height: 40px; }
    </style>
  </head>
  <body>
    <h1>Console Browser</h1>
    <p>This page reloads after the content process is replaced.</p>
    <input aria-label="Browser test input">
    <script>
      if (getComputedStyle(document.querySelector('h1')).color === 'rgb(1, 2, 3)')
        document.title = 'Console browser test';
      const field = document.querySelector('input');
      field.addEventListener('click', () => {
        if (document.activeElement === field)
          document.title = 'Console browser input focused';
      });
      field.addEventListener('input', () => {
        if (field.value === 'z')
          document.title = 'Console browser input typed';
      });
      window.addEventListener('scroll', () => {
        if (field.value === 'z' && window.scrollY >= 100)
          document.title = 'Console browser scrolled down';
      });
    </script>
  </body>
</html>
"#;

struct Cache(PathBuf);

impl Drop for Cache {
  fn drop(&mut self) {
    match fs::remove_dir_all(&self.0) {
      Ok(()) => {}
      Err(fault) => eprintln!(
        "removing browser fixture data {}: {fault}",
        self.0.display()
      ),
    }
  }
}

fn timed_out(action: &str) -> Result<io::Error, Never> {
  Ok(io::Error::new(
    io::ErrorKind::TimedOut,
    format!("{action} timed out"),
  ))
}

fn deadline(duration: Duration) -> io::Result<Instant> {
  Instant::now()
    .checked_add(duration)
    .ok_or_else(|| io::Error::other("browser fixture interval is too large"))
}

fn timeout(until: Instant, action: &str) -> io::Result<Timespec> {
  let Ok(expired) = timed_out(action);
  let time = until
    .checked_duration_since(Instant::now())
    .ok_or(expired)?;

  Timespec::try_from(time).map_err(io::Error::other)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visit {
  Start,
  Page,
}

fn response(mut connection: TcpStream) -> io::Result<Option<Visit>> {
  const REQUEST_LIMIT: u64 = 16 * 1024;

  connection.set_read_timeout(Some(Duration::from_secs(2)))?;
  connection.set_write_timeout(Some(Duration::from_secs(2)))?;
  let mut lines = BufReader::new((&mut connection).take(REQUEST_LIMIT)).lines();
  let next_line = lines.next().transpose()?;
  let line = match next_line {
    Some(line) => line,
    None => return Ok(None),
  };
  let mut parts = line.split_ascii_whitespace();
  let path = match (parts.next(), parts.next(), parts.next()) {
    (Some("GET"), Some(path), Some(_version)) => path,
    _ => return Ok(None),
  };
  let mut ended = None;

  for header in lines {
    let header_2 = header?;

    match header_2.as_str() {
      "" => {
        ended = Some(());
        break;
      }
      _ => {}
    }
  }

  match ended {
    Some(()) => {}
    None => return Ok(None),
  }

  match path {
    "/start" => {
      connection.write_all(b"HTTP/1.1 302 Found\r\nLocation: /page\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;

      Ok(Some(Visit::Start))
    }
    "/page" => {
      write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        PAGE.len()
      )?;
      connection.write_all(PAGE)?;

      Ok(Some(Visit::Page))
    }
    _ => {
      connection
        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;

      Ok(None)
    }
  }
}

fn process_descriptor(child: &BoundToParent) -> io::Result<OwnedFd> {
  let Ok(id) = child.id();
  let native = i32::try_from(id).map_err(io::Error::other)?;
  let pid =
    Pid::from_raw(native).ok_or_else(|| io::Error::other("the fixture process has no PID"))?;

  pidfd_open(pid, PidfdFlags::empty()).map_err(io::Error::from)
}

fn record(listener: &TcpListener, visits: &mut Vec<Visit>) -> io::Result<()> {
  let connection = match listener.accept() {
    Ok((connection, _)) => connection,
    Err(fault) => match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
      Some(()) => return Ok(()),
      None => return Err(fault),
    },
  };

  match response(connection) {
    Ok(Some(visit)) => visits.push(visit),
    Ok(None) => {}
    Err(fault) => match matches!(
      fault.kind(),
      io::ErrorKind::WouldBlock
        | io::ErrorKind::TimedOut
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::BrokenPipe
    )
    .then_some(())
    {
      Some(()) => {}
      None => return Err(fault),
    },
  }

  Ok(())
}

struct Check<'a> {
  browser: &'a Path,
  arguments: &'a [&'a str],
  pages: u32,
  seconds: u64,
}

fn check(check: Check<'_>) -> io::Result<()> {
  let directory = console_core_temporary_directories::private("browser-check-cache")
    .map_err(io::Error::other)?;
  let cache = Cache(directory);
  let listener = TcpListener::bind("127.0.0.1:0")?;
  listener.set_nonblocking(true)?;
  let local = listener.local_addr()?;
  let address = format!("http://{local}/start");
  let mut command = Command::new(check.browser);
  command
    .args(check.arguments)
    .arg(address)
    .env("NO_PROXY", "127.0.0.1")
    .env("no_proxy", "127.0.0.1")
    .env("XDG_CACHE_HOME", &cache.0)
    .env("CONSOLE_BROWSER_CHECK", "1")
    .env(
      "CONSOLE_BROWSER_CHECK_TIMEOUT",
      check.seconds.saturating_sub(5).to_string(),
    );
  let mut child = alongside(&mut command)?;
  let process = process_descriptor(&child)?;
  let until = deadline(Duration::from_secs(check.seconds))?;
  let mut visits = Vec::new();
  let status = wait_for_browser(&mut child, &process, &listener, &mut visits, until)?;

  match status.success().then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(format!(
        "Console browser exited with {status} after {visits:?}"
      )));
    }
  }

  let reached = match visits.as_slice() {
    [Visit::Start, pages @ ..] => {
      let Ok(count) = fitted::<_, u32>(pages.len());

      count == check.pages && pages.iter().all(|visit| *visit == Visit::Page)
    }
    _ => false,
  };

  match reached.then_some(()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(format!(
        "expected /start followed by {} /page visits, got {visits:?}",
        check.pages
      )));
    }
  }

  println!("The browser reopened the redirected URL.");
  Ok(())
}

fn wait_for_browser(
  child: &mut BoundToParent,
  process: &OwnedFd,
  listener: &TcpListener,
  visits: &mut Vec<Visit>,
  until: Instant,
) -> io::Result<ExitStatus> {
  for _ in 0..WAIT_WAKEUPS {
    let status = child.try_wait()?;

    match status {
      Some(status_2) => return Ok(status_2),
      None => {}
    }

    let wait = timeout(until, "Console browser")?;
    let mut watched = [
      PollFd::new(process, PollFlags::IN),
      PollFd::new(listener, PollFlags::IN),
    ];

    match poll(&mut watched, Some(&wait)) {
      Ok(_) => {}
      Err(Errno::INTR) => continue,
      Err(fault) => return Err(io::Error::from(fault)),
    }

    let [process_event, listener_event] = watched;

    match process_event
      .revents()
      .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR)
      .then_some(())
    {
      Some(()) => continue,
      None => {}
    }

    match listener_event
      .revents()
      .contains(PollFlags::IN)
      .then_some(())
    {
      Some(()) => record(listener, visits)?,
      None => {}
    }

    const MAX_VISITS: u32 = 32;
    let Ok(count) = fitted::<_, u32>(visits.len());

    match (count > MAX_VISITS).then_some(()) {
      Some(()) => return Err(io::Error::other("browser fixture exceeded its visit limit")),
      None => {}
    }
  }

  Err(io::Error::other(
    "the browser fixture exceeded its readiness wakeup limit",
  ))
}

enum GuestRenderer {
  Chromium,
  WebKit,
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "the guest Wayland launcher communicates the particular display socket in WAYLAND_DISPLAY"
  )
)]
fn guest_display() -> io::Result<PathBuf> {
  let name = std::env::var_os("WAYLAND_DISPLAY")
    .ok_or_else(|| io::Error::other("WAYLAND_DISPLAY is required"))?;
  let socket = PathBuf::from(name);

  match socket.is_absolute().then_some(()) {
    Some(()) => return Ok(socket),
    None => {}
  }

  let Ok(runtime) = console_core_places::runtime();
  let root = runtime.ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR is required"))?;
  let child = RelativePath::new(&socket).map_err(io::Error::other)?;

  Ok(root.join(child))
}

#[cfg_attr(
  dylint_lib = "explicit021_no_sleeping",
  expect(
    explicit021_no_sleeping,
    reason = "the guest compositor has not created a descriptor to poll yet. This readiness probe retries nonblocking connections for at most 20 seconds"
  )
)]
fn wait_for_display(path: &Path) -> io::Result<()> {
  let address = SocketAddrUnix::new(path)?;
  let until = deadline(Duration::from_secs(20))?;
  let Ok(expired) = timed_out("Weston");

  for _ in 0..1000 {
    let remaining = match until.checked_duration_since(Instant::now()) {
      Some(remaining) => remaining,
      None => return Err(expired),
    };
    let probe = socket_with(
      AddressFamily::UNIX,
      SocketType::STREAM,
      SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
      None,
    )?;

    match connect(&probe, &address) {
      Ok(()) => return Ok(()),
      Err(Errno::NOENT | Errno::CONNREFUSED | Errno::AGAIN | Errno::INPROGRESS) => {
        std::thread::sleep(PAUSE.min(remaining));
      }
      Err(fault) => return Err(io::Error::from(fault)),
    }
  }

  Err(expired)
}

fn guest(browser: &Path, renderer: GuestRenderer) -> io::Result<()> {
  let display = guest_display()?;
  wait_for_display(&display)?;

  match renderer {
    GuestRenderer::WebKit => check(Check {
      browser,
      arguments: &["--webkit-test"],
      pages: 3,
      seconds: 175,
    })?,
    GuestRenderer::Chromium => check(Check {
      browser,
      arguments: &["--smoke-test"],
      pages: 2,
      seconds: 125,
    })?,
  }

  println!("{PASSED}");
  Ok(())
}

struct Vm<'a> {
  launcher: &'a Path,
  log: &'a Path,
}

fn vm(arguments: Vm<'_>) -> io::Result<()> {
  let folder = match arguments
    .log
    .parent()
    .filter(|parent| !parent.as_os_str().is_empty())
  {
    Some(parent) => parent,
    None => Path::new("."),
  };
  let directory = console_core_temporary_directories::private_under(folder, "browser-vm-serial")
    .map_err(io::Error::other)?;
  let scratch = Cache(directory);
  let staged = scratch.0.join("serial.log");
  let mut serial_file = staged_serial(&staged)?;
  let stdout = serial_file.try_clone()?;
  let stderr = serial_file.try_clone()?;
  let mut command = Command::new(arguments.launcher);
  command.stdin(Stdio::null()).stdout(stdout).stderr(stderr);
  let mut machine = alongside(&mut command)?;
  let process = process_descriptor(&machine)?;
  let until = deadline(Duration::from_secs(420))?;
  let result = wait_for_vm(&mut machine, &process, until);
  drop(machine);
  fs::rename(&staged, arguments.log)?;
  let serial = serial_tail(&mut serial_file)?;
  let passed = serial
    .split(|byte| *byte == b'\n')
    .any(|line| line.trim_ascii() == PASSED.as_bytes());

  match (result, passed) {
    (Ok(status), true) => match status.success().then_some(()) {
      Some(()) => {
        println!(
          "NixOS browser VM passed. Serial log is in {}",
          arguments.log.display()
        );
        Ok(())
      }
      None => Err(io::Error::other(format!(
        "NixOS browser VM exited with {status}"
      ))),
    },
    (result_2, false) | (result_2 @ Err(_), true) => {
      io::stderr().write_all(&serial)?;
      result_2?;

      Err(io::Error::other("NixOS browser VM did not report success"))
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "the log is created inside this run's private staging directory. The final path is published only by rename after the VM exits"
  )
)]
fn staged_serial(path: &Path) -> io::Result<File> {
  File::options()
    .create_new(true)
    .read(true)
    .write(true)
    .open(path)
}

fn wait_for_vm(
  child: &mut BoundToParent,
  process: &OwnedFd,
  until: Instant,
) -> io::Result<ExitStatus> {
  for _ in 0..WAIT_WAKEUPS {
    let status = child.try_wait()?;

    match status {
      Some(status_2) => return Ok(status_2),
      None => {}
    }

    let wait = timeout(until, "NixOS browser VM")?;
    let mut watched = [PollFd::new(process, PollFlags::IN)];

    match poll(&mut watched, Some(&wait)) {
      Ok(_) => {}
      Err(Errno::INTR) => continue,
      Err(fault) => return Err(io::Error::from(fault)),
    }
  }

  Err(io::Error::other(
    "the NixOS fixture exceeded its readiness wakeup limit",
  ))
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "serial is a regular file created by this fixture. The VM has exited and the read is limited to its last 16 MiB"
  )
)]
fn serial_tail(serial: &mut File) -> io::Result<Vec<u8>> {
  const SERIAL_LIMIT: u64 = 16 * 1024 * 1024;
  let about = serial.metadata()?;
  let offset = about.len().saturating_sub(SERIAL_LIMIT);
  serial.seek(SeekFrom::Start(offset))?;
  let mut tail = Vec::new();
  serial.take(SERIAL_LIMIT).read_to_end(&mut tail)?;

  Ok(tail)
}

fn run() -> io::Result<()> {
  let args: Vec<String> = std::env::args().skip(1).collect();
  let words: Vec<&str> = args.iter().map(String::as_str).collect();

  match words.as_slice() {
    ["--vm", launcher, log] => vm(Vm {
      launcher: Path::new(launcher),
      log: Path::new(log),
    }),
    ["--guest-webkit", browser] => guest(Path::new(browser), GuestRenderer::WebKit),
    ["--guest-chromium", browser] => guest(Path::new(browser), GuestRenderer::Chromium),
    [browser, "--webkit"] => check(Check {
      browser: Path::new(browser),
      arguments: &["--webkit-test"],
      pages: 3,
      seconds: 120,
    }),
    [browser, servo] => check(Check {
      browser: Path::new(browser),
      arguments: &["--servo-test", "--servo-worker", servo],
      pages: 3,
      seconds: 50,
    }),
    [browser] => check(Check {
      browser: Path::new(browser),
      arguments: &["--smoke-test"],
      pages: 2,
      seconds: 50,
    }),
    _ => Err(io::Error::other(
      "usage: console-browser-check BROWSER [--webkit|SERVO_WORKER]",
    )),
  }
}

fn main() -> ExitCode {
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("console-browser-check: {error}");
      ExitCode::FAILURE
    }
  }
}
