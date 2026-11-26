mod fixture;
pub mod pool;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

use console_events::ipc::{Reply, Trace};
use console_events::serving::REQUEST_BYTES;
use pool::{BEFORE_LONG, Failure, change, serve_at, socket};

#[test]
fn version_zero_subscribers_keep_their_wire_format_on_the_new_server() -> Result<(), Failure> {
  let at = socket("v0-compatible")?;
  let source = serve_at(&at)?;
  let mut old = UnixStream::connect(&at)?;
  old.set_read_timeout(Some(BEFORE_LONG))?;
  old.write_all(b"subscribe sound\n")?;
  let saying = source.recv_timeout(BEFORE_LONG)?;
  let Ok(said) = change("sink 1 at 40%");
  saying.send(said)?;

  let mut old_2 = BufReader::new(old);
  let mut line = String::new();
  old_2.read_line(&mut line)?;
  assert_eq!(line, "publish sound sink 1 at 40%\n");

  let mut new = console_events::ipc::Client::at(&at, Trace::Quiet)?;
  let reply = new.call(console_events::ipc::Request::Ping, BEFORE_LONG)?;
  assert_eq!(reply, Reply::Pong);

  std::fs::remove_file(&at)?;

  Ok(())
}

#[test]
fn fragmented_and_pipelined_requests_are_answered_in_order() -> Result<(), Failure> {
  let at = socket("ipc-fragmented")?;
  let _source = serve_at(&at)?;
  let mut client = UnixStream::connect(&at)?;
  client.set_read_timeout(Some(BEFORE_LONG))?;
  client.write_all(b"ipc 1 1 1")?;
  client.write_all(b" -\nipc 4 1 2 -\nipc 1 1 3 -\n")?;

  let mut client_2 = BufReader::new(client);
  let mut line = String::new();

  for expected in [
    "ipc 1 101 1 -\n",
    "ipc 1 103 2 \"unsupported_version\"\n",
    "ipc 1 101 3 -\n",
  ] {
    client_2.read_line(&mut line)?;
    assert_eq!(line, expected);
    line.clear();
  }

  std::fs::remove_file(&at)?;

  Ok(())
}

#[test]
#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the offending socket has a read deadline before the test asserts EOF"
  )
)]
fn an_oversized_unterminated_request_only_closes_its_own_connection() -> Result<(), Failure> {
  let at = socket("ipc-oversized")?;
  let _source = serve_at(&at)?;
  let mut too_long = UnixStream::connect(&at)?;
  too_long.set_read_timeout(Some(BEFORE_LONG))?;
  let full = [b'x'; 16 << 10];
  let Ok(bytes) = console_core_number_conversion::fixed_index(full.len());
  assert_eq!(bytes, REQUEST_BYTES);
  too_long.write_all(&full)?;
  too_long.write_all(b"x")?;
  let read = too_long
    .read(&mut [0; 1])
    .map(console_core_number_conversion::fixed_index)?;
  assert_eq!(read, Ok(0));

  let mut new = console_events::ipc::Client::at(&at, Trace::Quiet)?;
  let call = new.call(console_events::ipc::Request::Ping, BEFORE_LONG)?;
  assert_eq!(call, Reply::Pong);

  std::fs::remove_file(&at)?;

  Ok(())
}

#[test]
fn a_workspace_watch_gets_stale_then_a_fresh_generation_after_reconnect() -> Result<(), Failure> {
  let binary = Path::new(env!("CARGO_BIN_EXE_console-events"));
  let relay = fixture::Fixture::start(binary)?;
  let Ok(pid) = relay.process.pid();
  assert_ne!(pid, 0);
  let (opened, _timing) = console_compositor::workspace_snapshot::capture_at(
    &relay.requests,
    console_compositor::workspace_snapshot::QueryPlan::Batch,
  )?;
  assert_eq!(opened.front.map(|front| front.id), Some(1));
  let (mut client, upstream, previous) = relay.watch()?;
  let Ok(()) = relay.change_front(2);
  drop(upstream);
  let deadline = Instant::now()
    .checked_add(BEFORE_LONG)
    .ok_or("clock overflow")?;
  let loss = client.next_workspace(Some(deadline))?;
  assert_eq!(loss, console_events::ipc::Workspace::Stale);

  let _reconnected = relay.source()?;
  let generation = fixture::fresh(&mut client, 2, previous)?;
  assert!(generation > previous);
  let mut reader = console_events::ipc::Client::at(&relay.socket, Trace::Quiet)?;
  let request = console_events::ipc::Request::GetWorkspace { max_age_ms: 5_000 };
  let current = reader.call(request, BEFORE_LONG)?;
  assert!(matches!(
    current,
    Reply::Workspace(console_events::ipc::Workspace::Fresh { generation: source, revision: 1, .. })
      if source == generation
  ));

  Ok(())
}
