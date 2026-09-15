# The rename

Everything here used to be called `legion-*`, after the machine it was first
written for. Nothing in it is about that machine: the panels, the manifest, the
theme and the checks would run on any handheld with a pad and a compositor. The
prefix is `console-*` now, as the music panel set it, and the repository is
`reeveng/hypr-console`.

There have been three of them. Sections 1 to 4 are the first, from `legion-*`
to `console-*`; section 5 is the second, which moved no prefix and gave the
families inside it a word to share; section 6 is the third, which left the
crates alone and went through the programs `[build]` installs. A name in an
earlier section may have moved again in a later one -- `console-sky` and
`sky-press` are section 2's names for what section 6 calls `console-wallpaper`
and `wallpaper-press` -- and the earlier text is left saying what was true when
it was written, because it is the record of a rename rather than a list of what
things are called.

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
| `LegionGo` in `console-input-gamepad` | the type is a model of that hardware, and of nothing else |
| the sentences about the Legion Go's buttons, in `docs/` and in the profiles | true of that hardware, and the reason the button contract reads as it does |

`@user@` is not a name and is not a prefix. It is the mark the manifest writes
for whoever the desktop belongs to, filled in by `machine::whoever()` at apply.
It stands in the path `files/home/@user@/`, and since the tree stopped naming
anybody it also stands in the bodies of `/etc/sudoers.d/console` and
`91-console-touchpad.rules`.

`kew` was a fork of somebody else's GPL program, carried as a built binary at
`/usr/local/bin/kew`, and it kept the name its project gave it: the panel
started it by that name, and what was in front on the path was what answered.
It is a crate now, `console-music-player`, and the binary is `music-player` --
named for what somebody types rather than for whose program it once was.

`hyprsession` went the same way and kept its name for the same reason, until the
source came into the tree. A crate the device builds is named for what it does
like every other crate here, so it is `console-resume`, and the name upstream
gave it is recorded in `papers/forks.md` rather than in a path. Which is the
rule this rename set: a name says what the thing is, and whose it was is a
question the papers answer.

`/usr/local/bin/virtual-keyboard` was a third and is not a fork any more.
It is a Rust crate the device compiles, and the name is the one this rename gave
it: named for what it is rather than for whose it was, which turned out to be
the right name a release before it was the true one. The wvkbd source it was
ported from was kept beside it for a while as the way back, and has since gone.

## 2. What moved

In order, one commit each, every one of them with the whole suite green.

1. **Crates.** Nineteen directories under `crates/`, their package names, their
   lib names, the workspace dependency table, and every `use` in the tree.
2. **Developer binaries.** `console-check` `console-desktop` `console-emulate`
   `console-garden` `console-manifest-publish` `console-palette`. None is installed, so
   only the `Makefile` and the docs followed them.
3. **Docs, tools, prose, and the environment.** `console-deploy`,
   `console-pull`, and `CONSOLE_HOST` `CONSOLE_KEYS` `CONSOLE_PAD`
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
   `/usr/share/backgrounds/console`, and the `console-palette:begin` markers
   inside every generated file. The picture is unchanged: `console-garden`
   draws the same bytes, and only the stamp's hash of its own sources moved.
8. **The checkout.** `/etc/console`: `ROOT` in `console-manifest-engine`, `TREE` in
   `console-sky`, the target's `Documentation=`, the `Makefile`, and both ssh
   tools. That also fixed the install line, which still copied
   `target/release/legion`, a name nothing had built since the engine became
   `console`.
9. **The sweep.** What is left of the word is section 1, and the `__pycache__`
   directories from before this was Rust are gone.

## 3. The migration

`just migrate`, once per device, and `console-migrate --check` says what the
machine is called now and changes nothing.

It has been run, and the attic it left is named in `docs/migrations.md`. It
stays anyway: a machine that was never brought over is a machine this is the
only way back for, and a sweep that exists only in a commit message is a sweep
nobody can run.

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
   enabled under: installing `console-input-controller.service` beside an enabled
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
4. **The prefix is `console-*`**, as the music panel set it.
5. **The hardware keeps its name.** Section 1 is the whole of it.
6. **A name has to be guessable by somebody who has never been here.** That is
   the test every crate was held to, and why `again`, `door`, `haste`, `sky`,
   `garden`, `stage`, `flows`, `menu`, `words` and `pad` are all gone.
7. **A crate is a thing, and never a place to put a function two callers
   share.** A `console-duplicate-keys` was made and unmade in one afternoon,
   and what was actually wrong with the four tests it existed for was that they
   never said which key was written twice.

Both of those last two were got wrong on the way here, which is why they are
written down rather than left as taste.

Two names were left alone on purpose. The controller's crate stays one crate
because the three devices it reads are all the controller's own. The nested
desktop kept the word *nested* while it had it: a nested compositor is a
compositor in another compositor's window, and the tree said that everywhere
before the crate was called it. Section 5 is where both of those names moved
again.

`console-launcher` and `console-applications` are two crates for one feature
and have to stay two. The launcher's binary reaches for the home screen's
`Spot` and the home screen's binary reaches for the application index, so one
crate holding both puts each of `console-home-screen` and `console-launcher` in
the other's dependency table, which Cargo refuses.

Still open, and small: whether this checkout's own directory moves from
`~/Documents/projects/legion-go`.

## 5. The families

The prefix said which tree a crate belongs to and nothing else, so fifty names
sorted into one flat list where the only thing `ls crates/` told you was the
alphabet. The second rename gave the families that already existed a word to
share:

| Family | What it holds |
| --- | --- |
| `console-core-*` | what the rest imports and nothing outside the workspace ever names: the error of a function that cannot fail, the colour arithmetic, the one place a number changes width, the whole-file write, the language a person reads, the programs this desktop did not write, the retry |
| `console-input-*` | everything a press comes through: the controller, the pad it is emulated on, the mapping, what has the input, the pointer and the touchscreen the checks press with, the on-screen keyboard, dictation |
| `console-manifest-*` | what a deploy is: the engine, the sweeps, the public copy |
| `console-music-*` | the music: `console-music` is what it draws, lists and presses, and `console-music-player` is what makes the sound |
| `console-program-*` | what a program on this device is: the contract, the loop that carries it out, how long a started one lives |
| `console-test-*` | where a check runs: the checks themselves, the flows, the stages, the desktop nested here |

Everything else is a thing somebody already has a word for -- `console-files`,
`console-settings`, `console-panel` -- and stays one word past the prefix.

`console-music-*` is the newest of the families and is the clearest case of how
one arrives. There was a crate called `console-music` and it was the panel;
what played the sound was kew, which was nobody's here. When the player was
written the panel's name stopped being available to it -- two crates cannot both
be *the music* -- so the panel became `console-music-panel`, the player is
`console-music-player`, and the word they share is the heading. A family is not
designed and then filled: it is what a second crate makes true.

Section 6 took the `-panel` back off it. What settled the question the second
time is that the crate builds five programs and four of them are not panels:
the bar's reading, the index, what plays next, and the sleeve drawn as letters.
`-panel` named how one of the five is drawn, so it was never the crate's
subject. `console-music` is the music -- what it draws, what it lists, what it
presses -- and `console-music-player` is the thing that makes the sound, which
is a different job and not *the music*. The family word is still `music` and
the crate that carries it bare is the head of it.

The line `console-core-*` is drawn at: no machine, no feature, and no program
somebody types. That is why `console-onscreen` and `console-screen` are not in
it -- both ask the compositor -- and why `console-repository` is not either,
though its library is as internal as any of them: it ships `console-pull`, and a
crate that owns a command is a crate somebody has heard of.

The rule the families are an instance of: **a name is as few words as say what
is inside, and no fewer.** Both halves have teeth. `console-event-broker`,
`console-panel-host`, `console-input-claim`, `console-bar-modules` and
`console-child-processes` each spent a word on the mechanism, and lost it.
`console-guide`, `console-defaults` and `console-viewer` each saved a word by
making the reader supply it, and gained one: `console-button-guide`,
`console-default-applications`, `console-media-viewer`. A shorter name that has
to be looked up is not shorter.

`console-theme` became `console-palette` because what it does is spend
`theme/palette.toml`, and `console-translation` became
`console-core-localization` because nothing in it translates -- it picks the
language a person reads.

Four installed names moved with their crates, which is the whole reason this
rename needed a migration and the crates on their own would not have:
`console-controller.service`, `console-keyboard.service`, and the two desktop
entries named after the viewer and the downloads. A machine that applied the
commit before it has both units enabled and would have run the old pair beside
the new -- two daemons on one pad -- so the sweep disables before it moves
anything. Every other installed name was left where it was, on the
grounds that it was already named for what somebody types -- which section 6 is
about, because for two names out of three it was not true.

## 6. The binaries

`[build]` is the list of programs this device compiles for itself, and until
this rename only about a third of them had a rule. *A binary is named for what
somebody types* is true of `console`, `console-deploy` and `console-check`, and
false of `bar-clock`, `home-square`, `panel-pictures` and `files-thumbs`, which
nobody has ever typed: they are started by a unit, by the bar's configuration,
by a keybinding, or by another program of ours. With no rule covering them each
was named by whoever wrote it, and there were four schemes in one list.

So there are two kinds of name and they take two rules.

A **command** is typed, and opens with `console-` and the word a person would
say: `console-buttons`, `console-screenshot`, `console-dictate`. `console`
itself is the one exception, being named for the whole desktop rather than for
a part of it.

A **part** is reached for by something else, and opens with a word its own
crate's name holds -- the word rather than the whole name, because a crate's
name is its job in as few words as say it and a part is one thing that crate
does. `bar-clock` out of `console-status-bar` and `home-square` out of
`console-home-screen` are each a part of what the crate is for, and demanding
the whole subject would have named them `status-bar-clock` and
`home-screen-square`. The family word comes off first: `console-input-keyboard`
is in the `input` family and its subject is the keyboard.

What that buys is that `[build]` sorted is `ls crates/` sorted. A reader
looking at a name knows where to open it, and a crate that grows a second
program gives it a name nobody has to choose.

`console-manifest-engine/tests/the_binaries.rs` holds it. What it cannot ask is
whether the word a part opens with is the one worth opening with: `bar-` names
what consumes the output rather than what produces it, and reads as a family
only because three crates happened to agree. The test caught the two that did
not agree -- `bar-door` out of `console-panel` and `bar-updating` out of
`console-notifications` -- and the answer there was to move the binaries rather
than the names, so all five now come out of the crate whose subject is the bar.

Seventeen names moved, and the shape of the list is the argument for them:

| Was | Is | Why |
| --- | --- | --- |
| `stick-scroll` | `controller-desktop` | the desktop half of the controller; the name said one thing it does |
| `game-return` | `controller-game` | the Game Mode half of the same crate |
| `game-mode` | `session-game` | it switches the session, which is what the crate is |
| `desktop-mode` | `session-desktop` | the other direction of the same switch |
| `layout-panel` | `mapping-panel` | out of `console-input-mapping` |
| `switch-language` | `language-switch` | the subject first, then what it does to it |
| `notices-panel` | `notifications-panel` | the crate is `console-notifications`; *notices* was a third word for it |
| `download-panel` | `downloads-panel` | one word in two numbers |
| `download-find` | `downloads-find` | |
| `download-get` | `downloads-get` | |
| `one-format` | `downloads-format` | named for what it makes rather than for anything a reader could place |
| `cover-ascii` | `music-cover` | named neither its crate nor, to anybody who had not read it, what it does |
| `sky-press` | `wallpaper-press` | `console-wallpaper` built `console-sky` and `sky-press`: two words for one subject |
| `console-sky` | `console-wallpaper` | |
| `put-away` | `console-put-away` | typed, and said the way a person says it |
| `dictate` | `console-dictate` | |
| `virtual-keyboard` | `console-keyboard` | *virtual* was whose it used to be rather than what it is |

`theme/sky.toml` and `docs/sky.md` keep their names: the sky is what the
wallpaper follows, and a table of pictures indexed by the weather and the sun
is named for the thing it is about rather than for the program that reads it.

The migration is the usual shape and is larger than the last one, because a
binary is installed under its own name and an apply removes nothing. A machine
that applied the commit before this one has both spellings of all sixteen
installed programs, and the old one still runs -- it is the same code compiled a
day earlier, which is what makes it worth sweeping rather than leaving: nothing
tells a person which of the two they have. The unit is the half that bites on
its own. `console-sky.service` is enabled and pulled in by `console.target`,
and an apply that installs `console-wallpaper.service` beside it leaves two
daemons choosing a picture for one screen, so the sweep stops and disables
before it moves anything.
