//! Run the standard Criterion measurements on disposable event sockets.

#[path = "../tests/fixture/mod.rs"]
mod fixture;
mod hyprwire;
mod timing;

use std::hint::black_box;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use console_compositor::socket::{self, Socket};
use console_compositor::workspace_snapshot::{self, QueryPlan, Snapshot};
use console_core_bounded_reads::{Line, LineEnding};
use console_core_never::Never;
use console_events::ipc::{Client, Reply, Request, Trace, Workspace};
use criterion::{BatchSize, Bencher, Criterion, SamplingMode};
use fixture::{Failure, Fixture, Process, Runtime, TIMEOUT};
use timing::Samples;

const RELAY: &str = env!("CARGO_BIN_EXE_console-events");
const BURST: u32 = 1_000;
const LISTENERS: u32 = 8;

fn main() -> Result<(), Failure> {
  let completed = std::panic::catch_unwind(benchmark);

  match completed {
    Ok(Ok(())) => Ok(()),
    Err(fault) => match fault.downcast::<String>() {
      Ok(diagnostic) => Err(Failure::from(*diagnostic)),
      Err(unknown) => std::panic::resume_unwind(unknown),
    },
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "this benchmark owns CONSOLE_IPC_BASELINE, CONSOLE_IPC_HYPRWIRE, and CONSOLE_IPC_LIVE and reads each once at startup"
  )
)]
fn benchmark() -> Result<(), Never> {
  let mut criterion = Criterion::default().configure_from_args();
  let baseline = std::env::var_os("CONSOLE_IPC_BASELINE").map(PathBuf::from);
  let reference = std::env::var_os("CONSOLE_IPC_HYPRWIRE");
  let live = std::env::var_os("CONSOLE_IPC_LIVE");
  let edited = Path::new(RELAY);
  let Ok(()) = run(
    &mut criterion,
    edited,
    baseline.as_deref(),
    reference.as_ref(),
  );

  match live {
    Some(_enabled) => {
      let Ok(()) = live_workspace(&mut criterion);
    }
    None => {}
  }

  criterion.final_summary();

  Ok(())
}

fn run(
  criterion: &mut Criterion,
  edited: &Path,
  baseline: Option<&Path>,
  reference: Option<&std::ffi::OsString>,
) -> Result<(), Never> {
  let Ok(()) = decode(criterion);
  let Ok(()) = framing(criterion);
  let Ok(()) = console_point(criterion, edited);

  match reference {
    Some(_enabled) => {
      let Ok(()) = reference_point(criterion);
    }
    None => {}
  }

  let Ok(()) = fanout(criterion, edited, "fanout/current");
  let Ok(()) = recovery(criterion, edited, "reconnect/current", Recovery::Snapshot);

  match baseline {
    Some(previous) => {
      let Ok(()) = fanout(criterion, previous, "fanout/baseline");
      let Ok(()) = recovery(criterion, previous, "reconnect/baseline", Recovery::Legacy);
    }
    None => {}
  }

  Ok(())
}

fn checked<T>(result: Result<T, Failure>) -> Result<T, Never> {
  match result {
    Ok(value) => Ok(value),
    Err(fault) => std::panic::resume_unwind(Box::new(fault.to_string())),
  }
}

#[cfg_attr(
  dylint_lib = "explicit047_no_captured_permission",
  expect(
    explicit047_no_captured_permission,
    reason = "Criterion drives the scoped timed loop, which exclusively owns the mutable benchmark state and sample ring"
  )
)]
fn timed<State>(
  bench: &mut Bencher<'_>,
  mut state: State,
  exercise: impl Fn(&mut State) -> Result<(), Failure>,
) -> Result<(), Never> {
  let mut samples = Samples::default();

  bench.iter_custom(|iterations| {
    let mut total = Duration::ZERO;

    for _iteration in 0..iterations {
      let began = Instant::now();
      let Ok(()) = checked(exercise(&mut state));
      let elapsed = began.elapsed();
      let Ok(()) = samples.add(elapsed);
      total = total.saturating_add(elapsed);
    }

    total
  });

  samples.report()
}

fn console_point(criterion: &mut Criterion, binary: &Path) -> Result<(), Never> {
  criterion.bench_function("point/console/cold_ping", |bench| {
    let Ok(fixture) = checked(Fixture::start(binary));
    let Ok(before) = resource(&fixture.process);
    let Ok(()) = timed(bench, &fixture.socket, |at| {
      let mut peer = client(at)?;

      ping(&mut peer)
    });
    let Ok(()) = report_resources(&fixture.process, before);
  });
  criterion.bench_function("point/console/warm_ping", |bench| {
    let Ok(fixture) = checked(Fixture::start(binary));
    let Ok(mut peer) = checked(client(&fixture.socket));
    let Ok(()) = checked(ping(&mut peer));
    let Ok(before) = resource(&fixture.process);
    let Ok(()) = timed(bench, peer, ping);
    let Ok(()) = report_resources(&fixture.process, before);
  });

  Ok(())
}

fn client(at: &Path) -> Result<Client, Failure> {
  Client::at(at, Trace::Quiet).map_err(Failure::from)
}

fn ping(peer: &mut Client) -> Result<(), Failure> {
  let replied = peer.call(Request::Ping, TIMEOUT)?;

  match replied {
    Reply::Pong => Ok(()),
    Reply::Workspace(_) | Reply::Fault(_) => Err(Failure::from("unexpected ping reply")),
  }
}

#[cfg_attr(
  dylint_lib = "explicit047_no_captured_permission",
  expect(
    explicit047_no_captured_permission,
    reason = "Criterion fills the sample ring on its own synchronous callback before the operator reads it"
  )
)]
fn reference_point(criterion: &mut Criterion) -> Result<(), Never> {
  for (name, mode) in [
    ("point/hyprwire/cold_ping", hyprwire::Mode::Cold),
    ("point/hyprwire/warm_ping", hyprwire::Mode::Warm),
  ] {
    criterion.bench_function(name, |bench| {
      let Ok(runtime) = checked(Runtime::new());
      let Ok(directory) = runtime.directory();
      let Ok(binary) = checked(hyprwire::build(directory));
      let at = directory.join("reference.sock");
      let Ok(server) = checked(Process::start(
        Command::new(&binary).arg("serve").arg(&at),
        &at,
      ));
      let Ok(before) = resource(&server);
      let mut samples = Samples::default();
      bench.iter_custom(|count| {
        let Ok((elapsed, individual)) = checked(hyprwire::sample(&binary, &at, mode, count));

        for sample in individual {
          let Ok(()) = samples.add(sample);
        }

        elapsed
      });
      let Ok(()) = samples.report();
      let Ok(()) = report_resources(&server, before);
    });
  }

  Ok(())
}

fn legacy(at: &Path) -> Result<BufReader<UnixStream>, Failure> {
  let mut peer = UnixStream::connect(at)?;
  peer.set_read_timeout(Some(TIMEOUT))?;
  peer.set_write_timeout(Some(TIMEOUT))?;
  peer.write_all(b"subscribe compositor\n")?;

  Ok(BufReader::new(peer))
}

fn expected(peer: &mut BufReader<UnixStream>, event: &str) -> Result<(), Failure> {
  let mut line = String::new();
  peer.read_line(&mut line)?;

  match (line.strip_prefix("publish compositor ") == Some(event)).then_some(()) {
    Some(()) => Ok(()),
    None => Err(Failure::from(format!("expected {event:?}, got {line:?}"))),
  }
}

struct Burst {
  fixture: Fixture,
  upstream: UnixStream,
  peers: Vec<BufReader<UnixStream>>,
  events: Vec<String>,
  bytes: Vec<u8>,
}

impl Burst {
  fn open(binary: &Path) -> Result<Self, Failure> {
    let fixture = Fixture::start(binary)?;
    let mut peers = (0..LISTENERS)
      .map(|_listener| legacy(&fixture.socket))
      .collect::<Result<Vec<_>, _>>()?;
    let mut upstream = fixture.source()?;
    upstream.write_all(b"ready>>now\n")?;

    for peer in &mut peers {
      expected(peer, "ready>>now\n")?;
    }

    let events: Vec<String> = (0..BURST)
      .map(|word| format!("workspacev2>>{word},main\n"))
      .collect();
    let bytes = events.concat().into_bytes();

    Ok(Self {
      fixture,
      upstream,
      peers,
      events,
      bytes,
    })
  }

  fn deliver(&mut self) -> Result<(), Failure> {
    self.upstream.write_all(&self.bytes)?;

    for peer in &mut self.peers {
      for event in &self.events {
        expected(peer, event)?;
      }
    }

    Ok(())
  }
}

fn fanout(criterion: &mut Criterion, binary: &Path, name: &str) -> Result<(), Never> {
  criterion.bench_function(name, |bench| {
    let Ok(mut burst) = checked(Burst::open(binary));
    let Ok(before) = resource(&burst.fixture.process);
    let Ok(()) = timed(bench, &mut burst, |fixture| fixture.deliver());
    let Ok(()) = report_resources(&burst.fixture.process, before);
  });

  Ok(())
}

#[derive(Clone, Copy)]
enum Recovery {
  Snapshot,
  Legacy,
}

enum Feed {
  Snapshot { client: Client, generation: u64 },
  Legacy(BufReader<UnixStream>),
}

struct Connection {
  fixture: Fixture,
  upstream: UnixStream,
  feed: Feed,
}

impl Connection {
  fn open(binary: &Path, mode: Recovery) -> Result<Self, Failure> {
    let fixture = Fixture::start(binary)?;
    let (upstream, feed) = match mode {
      Recovery::Snapshot => {
        let (client, upstream, generation) = fixture.watch()?;

        (upstream, Feed::Snapshot { client, generation })
      }
      Recovery::Legacy => {
        let mut peer = legacy(&fixture.socket)?;
        let mut upstream = fixture.source()?;
        upstream.write_all(b"ready>>now\n")?;
        expected(&mut peer, "ready>>now\n")?;

        (upstream, Feed::Legacy(peer))
      }
    };

    Ok(Self {
      fixture,
      upstream,
      feed,
    })
  }

  fn reconnect(self) -> Result<(Duration, Fixture), Failure> {
    let Self {
      fixture,
      upstream,
      feed,
    } = self;
    let Ok(()) = fixture.change_front(2);
    let began = Instant::now();
    drop(upstream);

    match feed {
      Feed::Snapshot {
        mut client,
        generation,
      } => {
        let deadline = began.checked_add(TIMEOUT).ok_or("clock overflow")?;
        let stale = client.next_workspace(Some(deadline))?;

        match stale {
          Workspace::Stale => {}
          Workspace::Fresh { .. } => return Err(Failure::from("expected a stale notification")),
        }

        let _connected = fixture.source()?;
        let _generation = fixture::fresh(&mut client, 2, generation)?;
      }
      Feed::Legacy(mut peer) => {
        let mut connected = fixture.source()?;
        connected.write_all(b"workspacev2>>2,2\n")?;
        expected(&mut peer, "workspacev2>>2,2\n")?;
        let (snapshot, _timing) =
          workspace_snapshot::capture_at(&fixture.requests, QueryPlan::Separate)?;
        black_box(snapshot);
      }
    }

    Ok((began.elapsed(), fixture))
  }
}

#[cfg_attr(
  dylint_lib = "explicit047_no_captured_permission",
  expect(
    explicit047_no_captured_permission,
    reason = "Criterion's synchronous sample callback owns the only writable sample ring"
  )
)]
fn recovery(
  criterion: &mut Criterion,
  binary: &Path,
  name: &str,
  mode: Recovery,
) -> Result<(), Never> {
  let mut group = criterion.benchmark_group(name);
  group.sample_size(10);
  group.sampling_mode(SamplingMode::Flat);
  group.warm_up_time(Duration::from_secs(1));
  group.measurement_time(Duration::from_secs(1));
  group.bench_function("disconnect_to_fresh", |bench| {
    let mut samples = Samples::default();
    bench.iter_custom(|iterations| {
      let mut total = Duration::ZERO;

      for _iteration in 0..iterations {
        let Ok(connection) = checked(Connection::open(binary, mode));
        let Ok(before) = resource(&connection.fixture.process);
        let Ok((elapsed, fixture)) = checked(connection.reconnect());
        let Ok(()) = samples.add(elapsed);
        let Ok(()) = report_resources(&fixture.process, before);
        total = total.saturating_add(elapsed);
      }

      total
    });
    let Ok(()) = samples.report();
  });
  group.finish();

  Ok(())
}

fn live_workspace(criterion: &mut Criterion) -> Result<(), Never> {
  for (name, mode) in [
    ("workspace/live_separate_queries", QueryPlan::Separate),
    ("workspace/live_batched_queries", QueryPlan::Batch),
  ] {
    criterion.bench_function(name, |bench| {
      let Ok(at) = checked(socket::socket(Socket::Requests).map_err(Failure::from));
      bench.iter(|| {
        let Ok((snapshot, _timing)) =
          checked(workspace_snapshot::capture_at(&at, mode).map_err(Failure::from));
        black_box(snapshot);
      });
    });

    for (kind, decode) in [("socket", Component::Socket), ("decode", Component::Decode)] {
      criterion.bench_function(&format!("{name}_{kind}"), |bench| {
        let Ok(at) = checked(socket::socket(Socket::Requests).map_err(Failure::from));
        bench.iter_custom(|count| {
          let mut elapsed = Duration::ZERO;

          for _sample in 0..count {
            let Ok((_snapshot, timing)) =
              checked(workspace_snapshot::capture_at(&at, mode).map_err(Failure::from));
            let measured = match decode {
              Component::Socket => timing.socket,
              Component::Decode => timing.decode,
            };
            elapsed = elapsed.saturating_add(measured);
          }

          elapsed
        });
      });
    }
  }

  criterion.bench_function("workspace/live_shared_snapshot", |bench| {
    let Ok(peer) = checked(Client::session(Trace::Quiet).map_err(Failure::from));
    let Ok(()) = timed(bench, peer, |client| {
      let snapshot = cached(client)?;
      black_box(snapshot);

      Ok(())
    });
  });

  Ok(())
}

enum Component {
  Socket,
  Decode,
}

fn cached(peer: &mut Client) -> Result<Snapshot, Failure> {
  let reply = peer.call(
    Request::GetWorkspace {
      max_age_ms: u32::MAX,
    },
    TIMEOUT,
  )?;

  match reply {
    Reply::Workspace(Workspace::Fresh { snapshot, .. }) => Ok(snapshot),
    Reply::Workspace(Workspace::Stale) | Reply::Pong | Reply::Fault(_) => Err(Failure::from(
      "the workspace feed must have a fresh snapshot before benchmarking",
    )),
  }
}

fn decode(criterion: &mut Criterion) -> Result<(), Never> {
  let front = r#"{"id":2,"name":"two","windows":32}"#;
  let workspaces = format!("[{front}]");
  let windows = (0..32)
    .map(|id| {
      format!(
        r#"{{"address":"0x{id:x}","title":"Terminal","initialClass":"foot","initialTitle":"foot",
          "workspace":{{"id":2,"name":"2"}},"monitor":0,"floating":false,
          "fullscreen":0,"at":[16,36],"size":[700,500],"pid":1854}}"#
      )
    })
    .collect::<Vec<_>>();
  let clients = format!("[{}]", windows.join(","));
  let monitors =
    r#"[{"name":"eDP-1","width":1920,"height":1200,"scale":1.5,"refreshRate":144.0}]"#;
  let batch = format!("{front}\n\n\n{workspaces}\n\n\n{clients}\n\n\n{monitors}");
  let Ok(snapshot) =
    checked(workspace_snapshot::decode_batch(batch.as_bytes()).map_err(Failure::from));
  let Ok(typed) = checked(serde_json::to_vec(&snapshot).map_err(Failure::from));
  criterion.bench_function("decode/compositor_json", |bench| {
    bench.iter(|| {
      let Ok(snapshot_2) = checked(
        workspace_snapshot::decode_batch(black_box(batch.as_bytes())).map_err(Failure::from),
      );
      black_box(snapshot_2);
    });
  });
  criterion.bench_function("decode/typed_snapshot_json", |bench| {
    bench.iter(|| {
      let Ok(snapshot_3) =
        checked(serde_json::from_slice::<Snapshot>(black_box(&typed)).map_err(Failure::from));
      black_box(snapshot_3);
    });
  });

  Ok(())
}

fn framing(criterion: &mut Criterion) -> Result<(), Never> {
  let mut bytes = vec![b'x'; 16_384];
  bytes.push(b'\n');
  criterion.bench_function("framing/fragmented_16_kib", |bench| {
    bench.iter_batched_ref(
      || {
        let Ok(frame) = Line::new(32_768, LineEnding::Required);

        (frame, BufReader::with_capacity(256, bytes.as_slice()))
      },
      |(frame, input)| {
        let Ok(read) = checked(frame.read(input).map_err(Failure::from));

        black_box(read);
      },
      BatchSize::SmallInput,
    );
  });

  Ok(())
}

#[derive(Default)]
struct Resource {
  rss: Option<u64>,
  switches: u64,
  wakeups: Option<u64>,
}

fn resource(process: &Process) -> Result<Resource, Never> {
  let Ok(pid) = process.pid();
  let at = PathBuf::from(format!("/proc/{pid}"));
  let Ok(rss) = read_counter(&at.join("status"), "VmHWM");
  let tasks = match std::fs::read_dir(at.join("task")) {
    Ok(tasks) => tasks,
    Err(_proc_unavailable) => return Ok(Resource::default()),
  };
  let mut switches = 0_u64;
  let mut wakeups: Option<u64> = None;

  for task in tasks.flatten() {
    let thread = task.path();
    let Ok(switched) = read_counter(&thread.join("status"), "voluntary_ctxt_switches");
    let Ok(woken) = read_counter(&thread.join("sched"), "se.statistics.nr_wakeups");

    match switched {
      Some(count) => switches = switches.saturating_add(count),
      None => {}
    }

    match (woken, wakeups) {
      (Some(count), Some(total)) => wakeups = Some(total.saturating_add(count)),
      (Some(first), None) => wakeups = Some(first),
      (None, _) => {}
    }
  }

  Ok(Resource {
    rss,
    switches,
    wakeups,
  })
}

fn read_counter(file: &Path, field: &str) -> Result<Option<u64>, Never> {
  let text = match std::fs::read_to_string(file) {
    Ok(text) => text,
    Err(_proc_counter_unavailable) => return Ok(None),
  };
  let count = text.lines().find_map(|line| {
    let (name, value) = line.split_once(':')?;
    let reading = value.split_whitespace().next()?;

    match (name.trim() == field).then_some(()) {
      Some(()) => match reading.parse() {
        Ok(count) => Some(count),
        Err(_malformed_counter) => None,
      },
      None => None,
    }
  });

  Ok(count)
}

fn report_resources(process: &Process, before: Resource) -> Result<(), Never> {
  let Ok(after) = resource(process);
  let switches = after.switches.saturating_sub(before.switches);
  let wakeups = after
    .wakeups
    .zip(before.wakeups)
    .map(|(right, left)| right.saturating_sub(left));
  eprintln!(
    "relay peak_rss_kib={:?} voluntary_switches={switches} wakeups={wakeups:?}",
    after.rss
  );

  Ok(())
}
