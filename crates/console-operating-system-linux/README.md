# Linux operating-system adapters

Platform packages use the `console-operating-system-<platform>` prefix so they
appear together under `crates/`.

Programs use [events and effects](../console-program-contract/src/lib.rs) to
describe what happened and what they want done. The
[shared execution loop](../console-program-runtime/src/execution.rs) owns the
order of turns, replies, timers and stopping. Linux and
[simulation](../console-program-runtime/src/simulation.rs) both use that loop.

## Effect loops

When an effect produces a reply, the runtime queues it as another event. The
machine can decide from that reply whether to continue, retry, or stop. Use
`Subscription::Timer` for periodic ticks and `Subscription::Once` to schedule a
single tick. For example, wallpaper schedules its next weather attempt after
the current one replies, with a longer delay after a failed fetch.

`execute_with` lets another executor compute each transition for a machine.
The runtime waits for its next state and effect list before sending another
request to that machine. The platform performs the effects in order. To check
a new executor, compare the final state and the ordered event and effect traces
with the inline executor on the same simulation script.

## Supplying a platform

Effect's `Layer.provide` supplies one service's dependencies while assembling
the layer, and `Effect.provide` supplies the resulting services to a program.
Here, a Rust trait describes a required service, and the entry point constructs
the implementations and passes them to the runtime. For example, Linux
[`program_runtime::run`](src/program_runtime/mod.rs) connects the event feed,
sets up the clock and listener, and supplies the resulting platform. Tests
supply a platform with a scripted clock and arrivals.
[Effect layers](https://effect.website/docs/v4/requirements-management/layers)

```rust
let Ok(program) = provide::<MyMachine, _>(&input, &mut platform, Limit::Unbounded);
let completed = program.run()?;
```

`provide` checks that the machine's events and effects match the platform.
`run` begins executing them.

As more services get contracts, the entry point for a full desktop should
require each mandatory service in its type. An incomplete set then fails to
build. Hardware and user services can still be absent at runtime. Report that
through the feature's interface, with enough detail for the program to decide
whether to disable the feature or retry.

## Porting checklist

| Capability | Current contract or owner | Linux status |
| --- | --- | --- |
| Program turns, deadlines and stopping | `console-program-runtime::Platform` | `program_runtime` supplies monotonic time, arrivals and effect execution. |
| Commands and process lifetime | `console-program-runtime::commands::Platform`, `console-program-lifetime` | `program_runtime::commands` runs and launches commands. `program_lifetime` uses parent-death signals, process groups and pidfds. `systemd` supplies session scopes when available. |
| Event subscriptions and change delivery | `console-program-contract`, `console-events::pool` | The `events` module holds the Unix socket client and wire format. The server and Linux event sources still live in `console-events`. |
| Filesystem and storage | File effects in `console-program-contract`, storage code in `console-core-atomic-writes`, `console-files` and `console-kernel-files` | The runtime replaces files atomically and replies with `Event::Written`, including the destination and the stage of any failure. A common contract for permissions and change notifications is still needed. |
| Display, input and session | `console-compositor`, `console-panel`, `console-input-*`, `console-session` | Wayland, Hyprland, Linux input devices and session integration still need capability boundaries. |
| Bluetooth and other connectivity | `console-bluetooth`, `console-bluetooth-agent`, `console-bus`, `console-events` | Bluetooth uses BlueZ over D-Bus. Discovery, pairing and connection still need a contract that can accept another Bluetooth stack. |
| Audio, power, boot and system management | `console-music-player`, `console-battery`, `console-awake`, `console-boot-entries`, `console-manifest-engine` | Host integrations still live with their callers. |

Start a port with the runtime and command traits, then implement the services
required by the programs you want to run. A new kernel supplies the relevant
process, memory and device primitives. A new filesystem can implement the
storage contract independently once that contract exists. Verify the guarantees
each program relies on, such as replacing a file atomically or stopping a
child tree when its owner goes away.

### Checking portable packages

Add `portable = true` under `[package.metadata.console]` in the crate's
`Cargo.toml` to opt in. WebP decoding and pixel buffers use this declaration.
`just map` checks opted-in packages for direct host API use,
links to libraries that use those APIs, and links to operating-system adapters.
Pass the required behavior as a narrow trait or callback where the workflow
needs it.

The compiler records recognized filesystem, process, clock, environment,
network, thread, and low-level system APIs in `docs/architecture/facts.jsonl`.
To see which packages have filesystem access, for example, run
`jq -r 'select(.kind == "host-access" and (.what | startswith("filesystem "))) | .package' docs/architecture/facts.jsonl | sort -u`.
The inventory covers the workspace libraries and binaries built by `just map`.

Bluetooth illustrates a second kind of substitution. BlueZ is a backend.
Classic and LE are transports, and a remote service may require a particular
profile. A future connection policy can try compatible candidates after
discovery. Concurrent attempts need support from the backend and controller,
with one owner coordinating pairing and cancelling attempts once a connection
succeeds.
[BlueZ Device API](https://bluez.readthedocs.io/en/latest/device-api/) ·
[Bluetooth technology overview](https://www.bluetooth.com/learn-about-Bluetooth/tech-overview/)
