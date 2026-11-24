//! Optional fingerprint checks run in a child process so the pattern stays usable.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use console_bus::connection::Bus;
use console_bus::messages::{Kind, Message, Value, Whom};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_login_session::{Request, Rules, authenticate_without_secret};
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use crate::authentication::{Availability, PAM_DIRECTORY, PamService, Verification, worker_path};

const SERVICE: PamService = PamService::Fingerprint;
const FPRINT: &str = "net.reactivated.Fprint";

pub const WORKER: &str = "--fingerprint-worker";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
  Enrollment,
  Verification,
}

impl Check {
  fn argument(self) -> Result<&'static str, Never> {
    Ok(match self {
      Self::Enrollment => "enrollment",
      Self::Verification => "verify",
    })
  }

  fn start(self, person: &str) -> Result<Option<Worker>, Never> {
    let Ok(service) = SERVICE.availability();

    match service {
      Availability::Available => {}
      Availability::Unavailable => return Ok(None),
    }

    Ok(match self.spawn(person) {
      Ok(worker) => Some(worker),
      Err(fault) => {
        console_core_logging::warn!("cannot start fingerprint check: {fault}");
        None
      }
    })
  }

  fn spawn(self, person: &str) -> io::Result<Worker> {
    let executable = worker_path()?;
    let Ok(operation) = self.argument();
    let mut command = Command::new(executable);
    command
      .args([WORKER, operation, person])
      .env_remove("DBUS_SYSTEM_BUS_ADDRESS")
      .stdin(Stdio::null())
      .stdout(Stdio::null());
    Worker::spawn(&mut command)
  }
}

pub(crate) struct Worker {
  child: BoundToParent,
  process: OwnedFd,
}

impl AsFd for Worker {
  fn as_fd(&self) -> BorrowedFd<'_> {
    self.process.as_fd()
  }
}

impl Worker {
  pub(crate) fn spawn(command: &mut Command) -> io::Result<Self> {
    let child = alongside(command)?;
    let Ok(id) = child.id();
    let Ok(raw) = fitted::<u32, i32>(id);
    let pid = Pid::from_raw(raw).ok_or(io::ErrorKind::InvalidData)?;
    let process = pidfd_open(pid, PidfdFlags::empty())?;

    Ok(Self { child, process })
  }

  pub(crate) fn result(&mut self) -> Result<Verification, Never> {
    Ok(match self.child.try_wait() {
      Ok(Some(status)) => match status.success().then_some(()) {
        Some(()) => Verification::Accepted,
        None => Verification::Refused,
      },
      Ok(None) => Verification::Pending,
      Err(fault) => {
        console_core_logging::warn!("cannot read authentication result: {fault}");
        Verification::Refused
      }
    })
  }
}

enum VerifierState {
  Unavailable,
  CheckingEnrollment(Worker),
  Ready,
  Scanning(Worker),
}

pub struct Verifier {
  person: String,
  state: VerifierState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
  Available,
  Accepted,
  Refused,
}

impl Verifier {
  #[must_use]
  pub fn availability(&self) -> Result<Availability, Never> {
    Ok(match self.state {
      VerifierState::Ready | VerifierState::Scanning(_) => Availability::Available,
      VerifierState::Unavailable | VerifierState::CheckingEnrollment(_) => {
        Availability::Unavailable
      }
    })
  }

  #[must_use]
  pub fn for_session() -> Result<Self, Never> {
    Self::new("")
  }

  #[must_use]
  pub fn new(person: &str) -> Result<Self, Never> {
    let Ok(enrollment) = Check::Enrollment.start(person);
    let state = match enrollment {
      Some(worker) => VerifierState::CheckingEnrollment(worker),
      None => VerifierState::Unavailable,
    };

    Ok(Self {
      person: person.to_owned(),
      state,
    })
  }

  #[must_use]
  pub fn fd(&self) -> Result<Option<BorrowedFd<'_>>, Never> {
    Ok(match &self.state {
      VerifierState::CheckingEnrollment(worker) | VerifierState::Scanning(worker) => {
        Some(worker.as_fd())
      }
      VerifierState::Unavailable | VerifierState::Ready => None,
    })
  }

  pub fn cancel_scan(&mut self) -> Result<(), Never> {
    match self.state {
      VerifierState::Scanning(_) => self.state = VerifierState::Ready,
      VerifierState::CheckingEnrollment(_)
      | VerifierState::Unavailable
      | VerifierState::Ready => {}
    }

    Ok(())
  }

  pub fn scan(&mut self) -> Result<Option<Event>, Never> {
    Ok(match self.state {
      VerifierState::Ready => {
        let Ok(scan) = Check::Verification.start(&self.person);

        match scan {
          Some(worker) => {
            self.state = VerifierState::Scanning(worker);
            None
          }
          None => Some(Event::Refused),
        }
      }
      VerifierState::Unavailable => Some(Event::Refused),
      VerifierState::CheckingEnrollment(_) | VerifierState::Scanning(_) => None,
    })
  }

  pub fn response(&mut self) -> Result<Option<Event>, Never> {
    let (check, outcome) = match &mut self.state {
      VerifierState::CheckingEnrollment(worker) => {
        let Ok(result) = worker.result();
        (Check::Enrollment, result)
      }
      VerifierState::Scanning(worker) => {
        let Ok(result) = worker.result();
        (Check::Verification, result)
      }
      VerifierState::Unavailable | VerifierState::Ready => return Ok(None),
    };

    Ok(match (check, outcome) {
      (Check::Enrollment, Verification::Accepted) => {
        self.state = VerifierState::Ready;
        Some(Event::Available)
      }
      (Check::Enrollment, Verification::Refused) => {
        self.state = VerifierState::Unavailable;
        None
      }
      (Check::Verification, Verification::Accepted) => {
        self.state = VerifierState::Ready;
        Some(Event::Accepted)
      }
      (Check::Verification, Verification::Refused) => {
        self.state = VerifierState::Ready;
        Some(Event::Refused)
      }
      (Check::Verification | Check::Enrollment, Verification::Pending) => None,
    })
  }
}

#[must_use]
pub fn work(request: WorkerRequest<'_>, now: Instant) -> Result<ExitCode, Never> {
  let Ok(service) = SERVICE.availability();

  match service {
    Availability::Available => {}
    Availability::Unavailable => return Ok(ExitCode::FAILURE),
  }

  let person = match request.person {
    "" => {
      let Ok(session) = session_account(request.login_name);

      match session {
        Some(account) => account,
        None => return Ok(ExitCode::FAILURE),
      }
    }
    requested_person => requested_person,
  };

  Ok(match request.operation {
    "enrollment" => {
      let Ok(known) = enrolled(person, now);

      match known {
        Enrollment::Present => ExitCode::SUCCESS,
        Enrollment::Absent => ExitCode::FAILURE,
      }
    }
    "verify" => {
      let Ok(service_name) = SERVICE.name();
      let result = authenticate_without_secret(Request {
        service: service_name,
        person,
        rules: Rules::Directory(Path::new(PAM_DIRECTORY)),
      });

      match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(fault) => {
          console_core_logging::debug!("fingerprint verification did not succeed: {fault}");
          ExitCode::FAILURE
        }
      }
    }
    _ => ExitCode::FAILURE,
  })
}

pub struct WorkerRequest<'a> {
  pub operation: &'a str,
  pub person: &'a str,
  pub login_name: Option<&'a str>,
}

#[must_use]
pub(crate) fn session_account(login_name: Option<&str>) -> Result<Option<&str>, Never> {
  let name = match login_name {
    Some(name) => name,
    None => return Ok(None),
  };

  let person = match crate::system::person(name) {
    Ok(Some(person)) => person,
    Ok(None) => return Ok(None),
    Err(fault) => {
      console_core_logging::debug!("cannot look up the session account: {fault}");
      return Ok(None);
    }
  };

  Ok((person.user == rustix::process::getuid().as_raw()).then_some(name))
}

enum Enrollment {
  Present,
  Absent,
}

fn enrolled(person: &str, now: Instant) -> Result<Enrollment, Never> {
  let mut bus = match Bus::system() {
    Ok(connection) => connection,
    Err(fault) => {
      console_core_logging::debug!("cannot check fingerprint enrollment: {fault}");
      return Ok(Enrollment::Absent);
    }
  };

  let deadline = match now.checked_add(Duration::from_secs(3)) {
    Some(deadline) => deadline,
    None => return Ok(Enrollment::Absent),
  };
  let Ok(get_devices) = Message::call(&Whom {
    to: FPRINT,
    at: "/net/reactivated/Fprint/Manager",
    on: "net.reactivated.Fprint.Manager",
    calling: "GetDevices",
  });

  let devices = match bus.call_before(&get_devices, deadline) {
    Ok(devices) => devices,
    Err(fault) => {
      console_core_logging::debug!("cannot list fingerprint devices: {fault}");
      return Ok(Enrollment::Absent);
    }
  };

  let listed = match (devices.kind, devices.values.as_slice()) {
    (Kind::Answer, [Value::List(listed)]) => listed,
    _ => return Ok(Enrollment::Absent),
  };

  for device in listed {
    let path = match device {
      Value::Path(path) => path,
      Value::Byte(_)
      | Value::Truth(_)
      | Value::Signed16(_)
      | Value::Unsigned16(_)
      | Value::Signed32(_)
      | Value::Unsigned32(_)
      | Value::Signed64(_)
      | Value::Unsigned64(_)
      | Value::Fraction(_)
      | Value::Word(_)
      | Value::Shape(_)
      | Value::List(_)
      | Value::Group(_)
      | Value::Variant { .. } => continue,
    };
    let Ok(call) = Message::call(&Whom {
      to: FPRINT,
      at: path,
      on: "net.reactivated.Fprint.Device",
      calling: "ListEnrolledFingers",
    });
    let Ok(list) = call.carrying("s", vec![Value::Word(person.to_owned())]);

    match bus.call_before(&list, deadline) {
      Ok(reply) => match (reply.kind, reply.values.as_slice()) {
        (Kind::Answer, [Value::List(fingers)]) => match fingers.as_slice() {
          [_, ..] => return Ok(Enrollment::Present),
          [] => {}
        },
        _ => {}
      },
      Err(fault) => {
        console_core_logging::debug!("cannot list enrolled fingerprints: {fault}");
      }
    }
  }

  Ok(Enrollment::Absent)
}
