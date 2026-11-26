# Console IPC tests and benchmarks

## Protocol

The event socket accepts the original `subscribe`, `unsubscribe`, and
`publish` spelling. Version 1 uses one newline-terminated UTF-8 frame per
request, reply, or snapshot. Each frame begins with `ipc VERSION ID SERIAL`,
followed by a space and the payload.

| ID | Direction | Message | Payload |
| --- | --- | --- | --- |
| 1 | Request | Ping | `-` |
| 2 | Request | Get workspace | Maximum snapshot age in milliseconds |
| 3 | Request | Watch workspace | `-` |
| 101 | Reply | Pong | `-` |
| 102 | Reply or update | Workspace | JSON-encoded `Workspace` |
| 103 | Reply | Fault | JSON-encoded `Fault` |

Replies carry the request serial. Serial zero marks a workspace update. A
watch gets an initial reply before updates. Stale state has no snapshot. Fresh
state carries the compositor connection generation and the revision within
that generation. Clients should treat a relay reconnection as a fresh session.
The relay sends the latest complete state when it can write an update.

Requests are limited to 16 KiB and each client has a 4 MiB output budget. The
relay answers oversized versioned replies or congestion with an overload
fault when it has room. The client reports an explicit deadline error when an
answer arrives too late. A client that loses its connection can reconnect and
subscribe again.

Run `console-events --probe --trace` in the session to show a request and its
reply. Run a relay with `--trace` to see message names, versions, serials, and
frame sizes in its diagnostics.

## Measure on the Legion Go

Use the same power profile for the current and baseline runs. Build the
pre-change relay in a separate checkout and pass the absolute path to its
release binary in `CONSOLE_IPC_BASELINE`. The current relay and measurement
client are built by Cargo. The Criterion harness starts disposable sockets
and leaves the production relay alone.

```sh
cargo bench --locked -p console-events --bench ipc
CONSOLE_IPC_BASELINE=/path/to/previous/target/release/console-events \
  cargo bench --locked -p console-events --bench ipc -- fanout
```

Set `CONSOLE_IPC_HYPRWIRE=1` to compare with the installed Hyprwire release.
The Rust harness generates a reference helper in the temporary build
directory, using the installed scanner, C++ compiler and libraries. Rust
and Hyprwire both time equivalent Ping/Pong work. Cold Hyprwire timing
includes connecting, negotiating, and binding.

```sh
CONSOLE_IPC_HYPRWIRE=1 cargo bench --locked -p console-events --bench ipc -- point
```

Criterion saves timing data under `target/criterion`. The harness reports
p50 and p99 from a bounded window of individual latency samples. It also
reports peak relay RSS and scheduler counters from `/proc`. Voluntary context
switches show blocking behavior. Wakeup counts appear on kernels that expose
them through schedstats. The reconnect measurement ends when a fresh
snapshot reaches the client. Overview's existing response-time recording
has a later `drawn` mark for the UI.

Run the workspace comparison in a live Hyprland session with the updated
relay running. It times four direct queries, one compositor batch, and a warm
typed reply. The direct and batch runs also report socket and decode
measurements individually.

```sh
CONSOLE_IPC_LIVE=1 cargo bench --locked -p console-events --bench ipc -- workspace
cargo bench --locked -p console-events --bench ipc -- decode
cargo bench --locked -p console-events --bench ipc -- framing
```

The decode benchmark uses a fixed 32-window fixture and requires no sockets.
Use the live timings to decide whether another wire encoding is worth testing.
