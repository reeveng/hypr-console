# hypr-console

A Lenovo Legion Go running Hyprland as a desktop, driven entirely by its
controller. No Game Mode and no Plasma: a compositor, a bar, a menu, an
on-screen keyboard, panels and a guide, every one of them reachable with two
thumbs.

Two things hold it up. `desktop.conf` is the whole inventory -- packages,
files, services, masked units -- and `console apply` is the only thing that
installs any of it, so a restart cannot lose a fix nobody wrote down. And the
controller is emulated from a capture of the real one, so what a button does
can be tried on a laptop in a second rather than over ssh.

    console check / apply / save     where the machine has drifted, put it back,
                                     take a file edited in place back into source
    just test / desktop / checks     here, nested at the device's own size, and
                                     every feature it has grown
    just deploy                      push it to the device and apply it

## The tree

    desktop.conf    the whole inventory
    files/          the content of every file it names, at the same path
    crates/         every program on the device, in one workspace
    docs/           why each thing is the way it is
    migrations/     what a machine is told when the manifest stops naming something
    scenarios/      presses a person would make, replayed
    theme/          palette.toml, the one place a colour is chosen
    tools/          the lint suite, and what a deploy runs

## docs

- [`button-contract.md`](docs/button-contract.md) -- what the buttons promise
- [`console-ui.md`](docs/console-ui.md) -- what a surface on this machine is
- [`panels.md`](docs/panels.md) -- how a panel is built
- [`programs.md`](docs/programs.md) -- how a program is built
- [`checks.md`](docs/checks.md) -- one check to a feature
- [`flows.md`](docs/flows.md) -- the long way round, across features
- [`emulator.md`](docs/emulator.md) -- the controller, in software
- [`desktop.md`](docs/desktop.md) -- the desktop on a machine that is not the device
- [`deploy.md`](docs/deploy.md) -- what a deploy is
- [`migrations.md`](docs/migrations.md) -- what the manifest cannot say
- [`theme.md`](docs/theme.md) -- the palette, and the one place it is chosen
- [`screen.md`](docs/screen.md) -- size, brightness, and being left alone
- [`sky.md`](docs/sky.md) -- the wallpapers
- [`files.md`](docs/files.md) -- the file browser
- [`downloads.md`](docs/downloads.md) -- getting something off the net
- [`browser.md`](docs/browser.md) -- the pad, in a page
- [`voice.md`](docs/voice.md) -- dictation
- [`notifications.md`](docs/notifications.md) -- what the desktop has said
- [`rename.md`](docs/rename.md) -- why nothing here is called legion
- [`forks.md`](docs/forks.md) -- the two programs not in this repository
- [`pictures/`](docs/pictures) -- the menu, the panels, the keyboard, the guide

## crates

- [`console-applications`](crates/console-applications) -- the menu
- [`console-browser-extension`](crates/console-browser-extension) -- the add-on this desktop puts in its browser
- [`console-button-guide`](crates/console-button-guide) -- what every button does
- [`console-core-atomic-writes`](crates/console-core-atomic-writes) -- writing a file so a machine that stops has a whole one
- [`console-core-colour`](crates/console-core-colour) -- colours, and how far apart two of them are
- [`console-core-external-programs`](crates/console-core-external-programs) -- every program this desktop runs and did not write
- [`console-core-localization`](crates/console-core-localization) -- everything a person reads, in the language they read
- [`console-core-never`](crates/console-core-never) -- the error of a function that cannot fail
- [`console-core-number-conversion`](crates/console-core-number-conversion) -- the one place a number changes width
- [`console-core-reconnect`](crates/console-core-reconnect) -- reaching again for something that has gone
- [`console-cpu-boost`](crates/console-cpu-boost) -- the processors, asked to hurry while somebody waits
- [`console-default-applications`](crates/console-default-applications) -- what this desktop opens things with
- [`console-device`](crates/console-device) -- the handheld, reached from this checkout
- [`console-downloads`](crates/console-downloads) -- something off the net, into the folders this device plays out of
- [`console-events`](crates/console-events) -- one subscription per source, and everybody else is told
- [`console-files`](crates/console-files) -- the files, as something the front of the machine can walk
- [`console-home-screen`](crates/console-home-screen) -- what is on the wallpaper, and where the thumb is on it
- [`console-input-controller`](crates/console-input-controller) -- input in, a doing out; it opens no device
- [`console-input-dictation`](crates/console-input-dictation) -- speaking instead of typing
- [`console-input-focus`](crates/console-input-focus) -- input, claimed by whatever is in front of you
- [`console-input-gamepad`](crates/console-input-gamepad) -- a Legion Go you can press, on a machine that is not one
- [`console-input-keyboard`](crates/console-input-keyboard) -- the on-screen keyboard, and the one crate with a licence of its own
- [`console-input-mapping`](crates/console-input-mapping) -- where the buttons are on a device that is not this one
- [`console-input-pointer`](crates/console-input-pointer) -- a pointer made of nothing, for the checks to press with
- [`console-input-touchscreen`](crates/console-input-touchscreen) -- a finger put down at a place on the picture
- [`console-launcher`](crates/console-launcher) -- the program the menu is
- [`console-manifest-engine`](crates/console-manifest-engine) -- `console`, the apply engine
- [`console-manifest-migrations`](crates/console-manifest-migrations) -- what a machine has to be told, because the manifest cannot say it
- [`console-manifest-publish`](crates/console-manifest-publish) -- builds this copy
- [`console-media-viewer`](crates/console-media-viewer) -- a photograph and a film, on the machine that holds them
- [`console-music`](crates/console-music) -- the music player: what it draws, what it lists, what it presses
- [`console-notifications`](crates/console-notifications) -- what the desktop has said, kept where somebody can look
- [`console-onscreen`](crates/console-onscreen) -- whether something is on the screen, asked of the compositor
- [`console-palette`](crates/console-palette) -- spends the palette into every file that holds a colour
- [`console-panel`](crates/console-panel) -- a panel: tabs across the top, and under them only what that tab is about
- [`console-panels`](crates/console-panels) -- one program, holding every panel
- [`console-program-contract`](crates/console-program-contract) -- what a program here is: words in, doings out
- [`console-program-lifetime`](crates/console-program-lifetime) -- how long a started program lives, said at the call site
- [`console-program-runtime`](crates/console-program-runtime) -- the loop that carries out what a program decided
- [`console-repository`](crates/console-repository) -- where the repository is, from anywhere inside it
- [`console-response-times`](crates/console-response-times) -- how long the machine kept somebody waiting
- [`console-screen`](crates/console-screen) -- the device's screen, read out of the compositor's own file
- [`console-session`](crates/console-session) -- which session has the screen, and how to get to the other one
- [`console-settings`](crates/console-settings) -- what Legion right opens
- [`console-status-bar`](crates/console-status-bar) -- what the bar says about the machine
- [`console-test-checks`](crates/console-test-checks) -- the checks, one module to a feature
- [`console-test-desktop`](crates/console-test-desktop) -- this desktop, in a window, on another machine
- [`console-test-flows`](crates/console-test-flows) -- the long way round, on purpose
- [`console-test-stages`](crates/console-test-stages) -- where a check can run, and what can be seen from there
- [`console-wallpaper`](crates/console-wallpaper) -- the sky: which wallpaper is up, and why

## tools

- [`console-deploy`](tools/console-deploy) -- put this checkout on the device and bring it to match
- [`console-migrate`](tools/console-migrate) -- move a device still called legion over to the console names
- `just pull` -- take what was changed on the device back into this checkout
- `cargo run --bin allow-uinput` -- let this machine make input devices without being root
- [`voice-compare`](tools/voice-compare) -- which hearing this device should use, measured on it
- [`explicit-rust`](tools/explicit-rust) -- the dylint suite this workspace is written to, a workspace of its own
  - [`explicit001_fallible_result`](tools/explicit-rust/explicit001_fallible_result) -- fallible fns return `Result<T, E>`
  - [`explicit002_infallible_result`](tools/explicit-rust/explicit002_infallible_result) -- infallible fns return `Result<T, Never>`
  - [`explicit003_no_never_error`](tools/explicit-rust/explicit003_no_never_error) -- `Result<T, !>` is forbidden; the name is `Never`
  - [`explicit004_no_panic`](tools/explicit-rust/explicit004_no_panic) -- no `unwrap`, `expect`, `panic`, `todo`, `unreachable`
  - [`explicit005_handle_fallible`](tools/explicit-rust/explicit005_handle_fallible) -- fallible values are handled or propagated
  - [`explicit006_option_not_error`](tools/explicit-rust/explicit006_option_not_error) -- `Option` is for "may not exist", not for errors
  - [`explicit007_no_bool_return`](tools/explicit-rust/explicit007_no_bool_return) -- no `bool` return values
  - [`explicit008_no_bool_param`](tools/explicit-rust/explicit008_no_bool_param) -- no `bool` parameters
  - [`explicit009_explicit_discard`](tools/explicit-rust/explicit009_explicit_discard) -- a discarded `#[must_use]` is `let _ =`
  - [`explicit010_no_numeric_into`](tools/explicit-rust/explicit010_no_numeric_into) -- no implicit numeric coercion
  - [`explicit011_no_as_cast`](tools/explicit-rust/explicit011_no_as_cast) -- no `as` casts
  - [`explicit012_safety_doc`](tools/explicit-rust/explicit012_safety_doc) -- `unsafe` carries a `// SAFETY:` reason
  - [`explicit013_breathing_room`](tools/explicit-rust/explicit013_breathing_room) -- a block that decides something gets a blank line around it
  - [`explicit014_no_index_slice`](tools/explicit-rust/explicit014_no_index_slice) -- no indexing or slicing; ask with `get`
  - [`explicit015_no_bare_arithmetic`](tools/explicit-rust/explicit015_no_bare_arithmetic) -- no bare integer arithmetic; the policy has a name
  - [`explicit016_no_wildcard_arm`](tools/explicit-rust/explicit016_no_wildcard_arm) -- no wildcard arm over an enum
  - [`explicit017_question_mark_alone`](tools/explicit-rust/explicit017_question_mark_alone) -- `?` is the whole of a statement, never buried
  - [`explicit018_allow_with_reason`](tools/explicit-rust/explicit018_allow_with_reason) -- an `allow` carries its reason
  - [`explicit019_no_if`](tools/explicit-rust/explicit019_no_if) -- no `if`; a decision is a `match` that names both outcomes
  - [`explicit020_no_comment`](tools/explicit-rust/explicit020_no_comment) -- no comments; a `//!` head and a `// SAFETY:` stay

## Licence

AGPL-3.0-or-later, for everything in this repository but one crate. Take it,
change it, run it, sell it -- and whoever you hand it to, over a wire as much
as on a disk, gets the source of what you handed them under the same terms.
That is the whole reason for the choice: what is open here stays open
downstream, and a desktop somebody serves rather than ships is not a hole in
that.

The exception is
[`crates/console-input-keyboard`](crates/console-input-keyboard), which is
GPL-3.0-or-later. It is a port of [wvkbd](https://git.sr.ht/~proycon/wvkbd) and
carries the licence it was given; the AGPL is not a later version of the GPL, so
it was never ours to move. Its own module comment says so.

The two forks named in [`docs/forks.md`](docs/forks.md) are not here and keep
their own.
