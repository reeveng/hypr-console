//! A local browser fixture and serial-console check for the NixOS guest.

#![forbid(unsafe_code)]

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use console_browser_downloads::Id;
use console_core_external_programs::Installed;
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
const FIRST_DOWNLOAD: &[u8] = b"the first report\n";
const SECOND_DOWNLOAD: &[u8] = b"the second report\n";

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
      a { position: fixed; left: 10px; top: 80px; padding: 10px; }
    </style>
  </head>
  <body>
    <h1>Console Browser</h1>
    <p>This page reloads after the content process is replaced.</p>
    <input aria-label="Browser test input">
    <a href="/download/first">Download report</a>
    <script>
      function ready() {
        if (new Date().getTimezoneOffset() !== 0)
          return 'Browser exposed the host time zone';
        if (navigator.language.toLowerCase() !== 'en-us' ||
            navigator.languages.some(language => !['en-us', 'en'].includes(language.toLowerCase())))
          return 'Browser exposed the host language preferences';
        if (document.createElement('canvas').getContext('webgl'))
          return 'Browser exposed WebGL';
        if (document.cookie.includes('console_fixture=') || localStorage.console_fixture)
          return 'Browser reused storage from another tab or worker';
        document.cookie = 'console_fixture=1; Path=/';
        localStorage.console_fixture = '1';
        if (!document.cookie.includes('console_fixture=1') || localStorage.console_fixture !== '1')
          return 'Browser could not use first-party storage';
        if (!innerWidth || !innerHeight || innerWidth % 200 || innerHeight % 100)
          return 'Browser exposed the exact viewport size';
        if (navigator.userAgent.includes('HeadlessChrome/')) {
          const canvas = document.createElement('canvas');
          const context = canvas.getContext('2d');
          context.fillRect(0, 0, 1, 1);
          try {
            context.getImageData(0, 0, 1, 1);
            return 'Chromium exposed readable canvas pixels';
          } catch (error) {
            if (error.name !== 'SecurityError') throw error;
          }
        }
        return 'Console browser test';
      }
      if (getComputedStyle(document.querySelector('h1')).color === 'rgb(1, 2, 3)') {
        try { document.title = ready(); }
        catch (error) { document.title = String(error); }
      }
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
  let mut language = None;

  for header in lines {
    let header_2 = header?;

    match header_2.as_str() {
      "" => {
        ended = Some(());
        break;
      }
      _ => {
        let parsed = header_2
          .split_once(':')
          .filter(|(name, _)| name.eq_ignore_ascii_case("Accept-Language"));

        match parsed {
          Some((_, value)) => language = Some(value.trim().to_owned()),
          None => {}
        }
      }
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
      let language_2 = match language.as_deref() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => "",
      };
      let primary = match language_2.split(',').next() {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => "",
      };

      match { ::core::primitive::bool::then_some(!primary.eq_ignore_ascii_case("en-US"), ()) } {
        ::core::option::Option::Some(()) => {
          return Err(io::Error::other(format!(
            "unexpected Accept-Language {language:?}",
            language = language_2
          )));
        }
        ::core::option::Option::None => {}
      }

      write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        PAGE.len()
      )?;
      connection.write_all(PAGE)?;

      Ok(Some(Visit::Page))
    }
    "/download/first" | "/download/second" => {
      let body = match path {
        "/download/first" => FIRST_DOWNLOAD,
        _ => SECOND_DOWNLOAD,
      };
      write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=report.txt\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
      )?;
      connection.write_all(body)?;
      Ok(None)
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
  let home = cache.0.join("home");
  let data = home.join(".local/share");
  fs::create_dir_all(&data)?;
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
    .env("HOME", &home)
    .env("XDG_CACHE_HOME", &cache.0)
    .env("XDG_DATA_HOME", &data)
    .env("LANGUAGE", "fr_BE:de_DE")
    .env("TZ", "Pacific/Honolulu")
    .env_remove("CONSOLE_BROWSER_CHECK_TRASH_ROOT")
    .env("CONSOLE_BROWSER_CHECK", "1")
    .env(
      "CONSOLE_BROWSER_CHECK_TIMEOUT",
      check.seconds.saturating_sub(5).to_string(),
    );
  let Ok(available) = console_gio::availability();
  let trash = match available {
    Installed::Yes => TrashCheck::Move,
    Installed::No => TrashCheck::Keep,
  };

  match trash {
    TrashCheck::Move => {
      command.env(
        "CONSOLE_BROWSER_CHECK_TRASH_ROOT",
        home.join("Downloads/Console Browser"),
      );
    }
    TrashCheck::Keep => {}
  }

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

  println!("The redirected URL reopened with fresh first-party storage in a new worker and tab.");
  verify_downloads(&home, &data, trash)?;

  match trash {
    TrashCheck::Move => println!(
      "The selected download and its metadata moved to Trash. The other ID stayed in Downloads."
    ),
    TrashCheck::Keep => {
      println!("Two downloads named report.txt kept separate IDs and contents.")
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
enum TrashCheck {
  Move,
  Keep,
}

fn download_id(name: &std::ffi::OsStr) -> io::Result<Id> {
  let text = name
    .to_str()
    .ok_or_else(|| io::Error::other("a download has a non-UTF-8 ID"))?;
  Id::parse(text).map_err(io::Error::other)
}

fn verify_downloads(home: &Path, data: &Path, trash: TrashCheck) -> io::Result<()> {
  let root = home.join("Downloads/Console Browser");
  let mut contents = Vec::new();
  let dir = fs::read_dir(&root)?;

  for entry in dir {
    let found = entry?;
    let name = found.file_name();

    match name.to_str() {
      Some(".partial") => {
        let mut staging = fs::read_dir(found.path())?;

        match staging.next() {
          Some(_) => {
            return Err(io::Error::other(
              "a closed tab left download staging behind",
            ));
          }
          None => {}
        }
      }
      Some(_) | None => {
        let _id = download_id(&name)?;
        let bytes = fs::read(found.path().join("report.txt"))?;
        contents.push(bytes);
      }
    }
  }

  match trash {
    TrashCheck::Move => {
      match contents.as_slice() {
        [_] => {}
        [] | [_, _, ..] => {
          return Err(io::Error::other(
            "one download should remain after Move to Trash",
          ));
        }
      }

      let recovered = trashed_report(data, &root)?;
      contents.push(recovered);
    }
    TrashCheck::Keep => {}
  }

  contents.sort_unstable();

  match contents.as_slice() {
    [first, second] => match (first == FIRST_DOWNLOAD && second == SECOND_DOWNLOAD).then_some(())
    {
      Some(()) => Ok(()),
      None => Err(io::Error::other(
        "expected two separate downloads named report.txt",
      )),
    },
    [] | [_] | [_, _, _, ..] => Err(io::Error::other(
      "expected two separate downloads named report.txt",
    )),
  }
}

fn trashed_report(data: &Path, root: &Path) -> io::Result<Vec<u8>> {
  let trash = data.join("Trash");
  let dir = fs::read_dir(trash.join("files"))?;
  let entries = dir.collect::<io::Result<Vec<_>>>()?;
  let entry = match entries.as_slice() {
    [single] => single,
    [] | [_, _, ..] => {
      return Err(io::Error::other(
        "expected one recoverable download in Trash",
      ));
    }
  };

  let id = download_id(&entry.file_name())?;
  let Ok(id_text) = id.as_str();
  let record = entry.path();
  let details = fs::metadata(record.join(".download.json"))?;
  let original = root.join(&id);
  let info_name = format!("{id_text}.trashinfo");
  let information = RelativePath::new(Path::new(&info_name)).map_err(io::Error::other)?;
  let info = fs::read_to_string(trash.join("info").join(information))?;
  let Ok(path) = console_core_file_urls::escaped(&original);
  let location = format!("Path={path}");
  let original_removed = match fs::metadata(&original) {
    Ok(_) => false,
    Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
      Some(()) => true,
      None => return Err(fault),
    },
  };

  match (details.is_file() && original_removed && info.lines().any(|line| line == location))
    .then_some(())
  {
    Some(()) => fs::read(record.join("report.txt")),
    None => Err(io::Error::other(
      "Trash did not preserve the complete download and its restore location",
    )),
  }
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

fn guest_display() -> io::Result<PathBuf> {
  let Ok(display) = console_core_places::wayland_display();
  let name = display.ok_or_else(|| io::Error::other("WAYLAND_DISPLAY is required"))?;
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
      pages: 4,
      seconds: 175,
    })?,
    GuestRenderer::Chromium => check(Check {
      browser,
      arguments: &["--smoke-test"],
      pages: 3,
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
      pages: 4,
      seconds: 120,
    }),
    [browser, servo] => check(Check {
      browser: Path::new(browser),
      arguments: &["--servo-test", "--servo-worker", servo],
      pages: 4,
      seconds: 50,
    }),
    [browser] => check(Check {
      browser: Path::new(browser),
      arguments: &["--smoke-test"],
      pages: 3,
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
