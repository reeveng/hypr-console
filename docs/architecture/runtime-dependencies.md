# Runtime dependency owners

| Dependency | Implementation to replace |
| --- | --- |
| `xdg-open`, `xdg-mime`, `xdg-settings` | `console-default-applications`. Installed Syncthing and indexed bookmarks enter through `console-open`. Opening the browser and launching a specific desktop entry are separate operations. |
| `gio` | `console-gio`. It launches desktop entries for `console-applications` and supplies recoverable trash operations. |
| Hyprland IPC and launch | `console-compositor`. The installed Wayland session and NixOS bootstrap call `console-compositor start`. The idle unit and Hypridle call its display power commands. |
| Wayland client protocols | `console-wayland`. The optional Hyprland thumbnail and color transform bindings live in `console-compositor`. |
| Night Light | `console-night-light` builds `console-warm` and sends per-output color transforms to Hyprland. |
| Steam Game Mode switching | `console-session-cachyos` implements `console-session::SwitchBackend`. It builds the installed session buttons and the Steam compatibility switcher. The handheld profile in `machines.conf` supplies the Gamescope configuration and systemd units. |
| Next login session | `console-session-plasma` implements `console-session::LoginBackend`. The login window links it to read the PlasmaLogin autologin selection written during the Steam handoff. |
| `pacman` | `console-package-manager-pacman` through `console-package-manager::system`. The package list and handheld policy live in the deployment manifests. |
| `nix` | `console-package-manager-nix` through `console-package-manager::user`. The Software Store supplies the path to its per-user profile. |
| `mise` | `console-package-manager-mise` through `console-package-manager::tools`. `just dev-tools` uses the checkout's `mise.toml` and `mise.lock`. |
| Selecting the next boot entry | `console-boot-entries` owns `bootctl set-oneshot`. The boot-complete and recovery units use `bootctl set-timeout-oneshot` separately to manage boot-menu visibility. |
| `notify-send` | `console-operating-system-linux::notification`. The notifications integration check exercises the external client separately. |
| D-Bus | `console-bus` owns the native transport and the remaining `busctl` commands. The notifications integration check uses `gdbus` as an external peer. |
| InputPlumber | `console-input-gamepad::front` owns profile queries and changes, device queries, and button chord commands. The Legion Go device override and game profile are installed by `machines.conf`. |

The ownership tests in `crates/console-architecture/tests/dependencies` read the current source and configuration. They check named Rust program variants, common literal executable arguments, and direct Cargo Wayland dependencies. They also check commands in systemd units and drop-ins, desktop entries, Hypridle and Lua hooks, and Nix bootstrap configuration. Run `cargo test -p console-architecture --test the_rules dependencies` to check them without rebuilding the architecture map.

## Checking availability

Run `console doctor --help` for profile descriptions, options, and examples.
From a checkout, `just dependencies` and `console-dependencies` use the same
checks. `just dependencies` builds the standalone checker with Cargo.
On a new build host, check the bootstrap tools with `command -v just`,
`command -v cargo`, and `command -v rustc`.

| Command | Checks |
| --- | --- |
| `console doctor` | Arch/CachyOS desktop executables, deployment hook paths, and runtime data |
| `console doctor handheld` | CachyOS Steam and Gamescope handoff and boot helpers |
| `console doctor development` | Build tools, native library metadata, and QEMU firmware |
| `console doctor optional` | Nix, OBS, fingerprint service registration, and WPE WebKit development libraries |
| `console doctor all` | Every profile |
| `console doctor executable ffmpeg /usr/bin/findmnt` | Executable names on the current PATH and at exact paths |
| `console doctor path /dev/uinput` | A file, directory, device, or symlink target at a given path |
| `console doctor pkg-config 'wpe-webkit-2.0 >= 2.54'` | A native module and any requested version constraint |

The checker lists every selected item and summarizes availability at the end.
`just dependencies` reports missing items and exits successfully.
`console doctor` and `console-dependencies` exit with status 1 when a dependency
is missing or could not be checked. Use `console doctor --report-only all` to
report every profile without failing on unavailable dependencies.
The checker reads the Rust executable registry directly, so new variants
appear in the report automatically. Run it as the account that will use the
dependency. The desktop and handheld profiles include distribution-specific
paths. Use individual checks for paths supplied by a Nix environment.

The checker reads the current environment on every run. Each running Linux
effect interpreter keeps successful executable lookups for up to one minute
when its search path uses absolute directories. A launch error indicating a
missing or inaccessible executable drops its cached path immediately. The next
request retries missing dependencies, so an installation can take effect as soon
as its executable is visible to the running process.

On Arch and CachyOS, `console check` checks the actual packages named by the
desktop and matching machine manifests. On NixOS, evaluating the runtime
package set checks the manifest's Nix package decisions and resolves the
selected Nixpkgs attributes. Check locked Rust source availability with
`cargo fetch --locked --offline` in the workspace being built. For the browser
scripts, `npm ls --all` from `tools/browser-scripts` checks the installed
JavaScript dependency tree. Service presence and service activity can be
inspected with `systemctl show -p LoadState -p ActiveState UNIT` or
`busctl --system list` for system-bus names.

The Linux process adapter verifies execute access when it resolves a registered
external executable. Run, stream, and fork effects deliver a failed reply when
provision fails. Spawn uses the existing launch diagnostic. Local adapters that construct a
`std::process::Command` directly can use `Program::provide()` for the same
file lookup. Keep launch errors handled after provision because the file or
its interpreter can change. Remote command text is resolved on the remote
machine when it runs.

## POSIX shell use

- `console-core-external-programs::shell` builds quoted command text for SSH and other interfaces that take a command line. `console-session::reaching`, `console-device`, and device stage checks use it. Hyprland binding generation uses it when the compositor takes a command string.
- The boot-started, boot-complete, and recovery units use `sh -c` with `findmnt` to distinguish the installed root from a snapshot boot. `nix/nixos-module.nix` uses a shell script to initialize and clean up the login session. `nix/runtime.nix` supplies Bash for the NixOS runtime environment.
- `console-awake` retains a shell heartbeat for older remote Console installations. Wallpaper tooling can read the shipped `palette.sh` with `sh`. Test stages and build recipes also use shell commands to prepare and inspect their environments.
