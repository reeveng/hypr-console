# The rename

Everything here used to be called `legion-*`, after the machine it was first
written for. Nothing in it is about that machine: the panels, the manifest, the
theme and the checks would run on any handheld with a pad and a compositor. The
prefix is `console-*` now, as `console-music` set it, and the repository is
`reeveng/hypr-console`.

The rename is done in this repository. It is not done on a device until
`tools/console-migrate` has been run against it, which is what section 3 is
about: a name in a file here is a string, and the same name on the machine is
an enabled unit, an installed binary, a checkout at `/etc/legion` and
directories in a home with somebody's own answers in them.

## 1. What does not move

The device is a Lenovo Legion Go and several things name it truthfully. They
stay as they are, and a rename that catches them is a rename that has gone too
far.

| Stays | What it is |
| --- | --- |
| `files/usr/share/inputplumber/devices/50-legion_go.yaml` | the shipped hardware definition, edited. InputPlumber loads it by that name |
| `ATTRS{name}=="*Legion Controller*Touchpad*"` in `91-console-touchpad.rules` | the kernel's name for the pad, matched |
| `name = "--legion-controller--touchpad"` in `hyprland.lua` | the same device, under the name Hyprland derives from it |
| `legion-left` and `legion-right` in the button vocabulary | the two buttons with the Legion mark on them, which is what a person calls them |
| `LegionGo` in `console-pad` | the type is a model of that hardware, and of nothing else |
| the sentences about the Legion Go's buttons, in `docs/` and in the profiles | true of that hardware, and the reason the button contract reads as it does |

`@user@` is not a name and is not a prefix. It is the mark the manifest writes
for whoever the desktop belongs to, filled in by `machine::whoever()` at apply.
It stands in the path `files/home/@user@/`, and since the tree stopped naming
anybody it also stands in the bodies of `/etc/sudoers.d/console` and
`91-console-touchpad.rules`.

`/usr/local/bin/hyprsession` and `/usr/local/bin/wvkbd-mobintl` are forks of
other people's GPL programs, carried here as built binaries. They keep the
names their projects gave them. What is ours is the unit that runs each one.

## 2. What moved

In order, one commit each, every one of them with the whole suite green.

1. **Crates.** Nineteen directories under `crates/`, their package names, their
   lib names, the workspace dependency table, and every `use` in the tree.
2. **Developer binaries.** `console-check` `console-desktop` `console-emulate`
   `console-garden` `console-publish` `console-theme`. None is installed, so
   only the `Makefile` and the docs followed them.
3. **Docs, tools, prose, and the environment.** `tools/console-deploy`,
   `tools/console-pull`, and `CONSOLE_HOST` `CONSOLE_KEYS` `CONSOLE_PAD`
   `CONSOLE_RAN` `CONSOLE_STAGE` `CONSOLE_TOUCHPAD` `CONSOLE_USER`.
4. **Installed binaries and their entries.** The engine is `console`, and with
   it `console-buttons`, `console-engine`, `console-sky`, the five scripts under
   `/usr/local/bin/`, the three `.desktop` files, the sudoers line, the two udev
   rules, and the `hyprland.lua` binds.
5. **Units.** `console.target` and `console-{bar,controller,keyboard,paper,
   polkit,session,sky}.service`, the gamescope drop-in, and the three files that
   name the target: `hyprland.lua`, `session-start`, `steamos-session-select`.
6. **The directories under a home.** `~/.config/console`,
   `~/.local/state/console`, `~/.local/share/console`, `~/.cache/console`,
   `~/.librewolf/console`, and the three paths under `XDG_RUNTIME_DIR`.
7. **The theme's own files.** `console.webp`, `console-placeholder.svg`,
   `/usr/local/lib/console/palette.sh`, the pressed pictures under
   `/usr/share/backgrounds/console`, and the `console-theme:begin` markers
   inside every generated file. The picture is unchanged: `console-garden`
   draws the same bytes, and only the stamp's hash of its own sources moved.
8. **The checkout.** `/etc/console`: `ROOT` in `console-manifest`, `TREE` in
   `console-sky`, the target's `Documentation=`, the `Makefile`, and both ssh
   tools. That also fixed the install line, which still copied
   `target/release/legion`, a name nothing had built since the engine became
   `console`.
9. **The sweep.** What is left of the word is section 1, and the `__pycache__`
   directories from before this was Rust are gone.

## 3. The migration

`tools/console-migrate`, once per device. `make migrate` runs it, and
`tools/console-migrate --check` says what the machine is called now and changes
nothing.

Run it over ssh from a laptop, in a shell you are already sitting in. The
desktop is down between the disable and the enable, so a machine doing this
from its own screen has no way of finishing the job.

The order it goes in, and why each step is where it is:

1. **The history, into the tree where it still stands.** Nothing installed
   moves. This is only so the machine has the source the new engine is built
   from, and the `files/` the apply will read later.
2. **The engine, built at the old path.** It holds `/etc/console` as its root,
   so it is installed now and asked for nothing until the tree has moved under
   it. A manifest binary compiled with the new root, on a machine whose tree is
   still at the old one, cannot find a single file it installs — and `console
   apply` is how anything gets fixed.
3. **The old units are disabled.** What systemd is running is the name it was
   enabled under: installing `console-controller.service` beside an enabled
   `legion-controller.service` gives the machine two units for one daemon, both
   wanting the pad. `syncthing.service` keeps its own name but goes with them,
   because what pulled it in was the target being renamed.
4. **The tree and the directories move.** `/etc/legion` to `/etc/console`, and
   in the home: the config directory with `sky.toml` and `defaults` in it, the
   state directory with `menu-counts` and the panels' tabs, the browser profile
   with history and logins. Renaming these in the files without moving them on
   the machine is a menu that has forgotten the order everything was in and a
   browser that starts on an empty profile. The pictures somebody pressed are
   moved too: `sky-press` writes them onto the device and nothing in the
   repository carries them.
5. **`console apply`.** The first thing the new engine is asked to do, and the
   first moment the machine has the new names installed.
6. **What the old names left behind goes to an attic.** The manifest installs a
   name and never sweeps one, so both sets are on the machine at this point.
   Nothing is deleted: everything is moved to `/var/tmp/console-migration-<when>`
   and the last line says where. Look at the desktop, then empty it.
7. **The new units are enabled and the target is started.** The desktop comes
   back here.

Afterwards, every clone needs its `device` remote pointed at the new path. The
clone the script was run from is done for you; the rest is
`git remote set-url device ssh://HOST/etc/console`.

## 4. What was decided

1. **`/etc/legion` moved to `/etc/console`**, rather than being left as the one
   old name on the machine.
2. **The command is `console`.** `console apply`, `console check`,
   `console save`. It is the name typed most days.
3. **`~/.librewolf/legion` moved too.** It was going to be left alone — it holds
   history, bookmarks and logins — but the profile is named in `profiles.ini`
   rather than invented by the browser, so it can be renamed as long as the
   directory moves underneath it in the same step. That is step 4 of the
   migration, and it refuses to run while LibreWolf is open.
4. **The prefix is `console-*`**, as `console-music` set it.
5. **The hardware keeps its name.** Section 1 is the whole of it.

Still open, and small: whether this checkout's own directory moves from
`~/Documents/projects/legion-go`.
