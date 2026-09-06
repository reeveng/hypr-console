# The loop: change something, run `just test`, and only then `just deploy`.
#
# Nothing has to be remembered before a deploy. `just ready` is the whole list
# and `console-deploy` runs it itself, so what reaches the device has
# passed it whether or not anybody thought to.

# The device, which only somebody with one can name. The tools read it too, and
# say so if it is not set.
HOST := env_var_or_default("CONSOLE_HOST", "")

# what this can do, which is what `just` on its own says
default:
    @just --list --unsorted

# write the palette into every file that spends it
theme:
    cargo run --quiet --release --bin console-theme

# press the wallpapers the table names
sky:
    cargo run --quiet --release --bin sky-press

# `--all-features` because the keyboard's Rust port is behind one. It is off by
# default so the device does not compile cairo and pango for a program it does
# not run, and on here so that being unfinished is not the same as being
# untested.
#
# The build is not a convenience in front of the tests, it is part of what they
# need. `cargo test` builds every crate's *test* targets; the program at
# `target/debug/launcher` it only relinks for a crate that has a `tests/`
# directory of its own, and the panel crates mostly do not. The panel tier
# opens those programs in a nested desktop, so without this it holds today's
# rules against whichever build happened to be lying there -- which is why it
# refuses to run at all when it finds one, and why the answer is to build
# rather than to make the check quieter.

# every test that can run on this machine
test:
    cargo build --quiet --workspace --all-features
    cargo test --quiet --workspace --all-features

# What a deploy runs before it sends anything, and the only place the list
# lives.
#
#   the programs    because `cargo test` does not leave them built. Only a
#                   crate with a `tests/` directory of its own gets its program
#                   relinked, and the panel tier opens those programs; without
#                   this the tier stops and says to run the build by hand,
#                   which would make this list no longer the whole list.
#   the tests       what the desktop promises about itself
#   clippy          denied rather than printed, because a warning nobody is
#                   made to read is a warning nobody reads
#   --locked        the device builds with it. A Cargo.lock that is behind
#                   would fail there instead, halfway through an apply, on a
#                   handheld: the same answer, found in the worse place.
#
#   the checks      the features, pressed against the emulator rather than
#                   read about. A fraction of a second each, because no machine
#                   takes part: the real profiles, the real pad, and the daemon
#                   running in this process.
#
#   the rules       the EXPLICIT_* rules the workspace already keeps. Only the
#                   ones nothing breaks, so this holds today and goes on
#                   holding; `just explicit` is where the rest of the distance
#                   is. It wants the lint suite's nightly, which is the one
#                   thing in this list that is not already on the machine, and
#                   it says so rather than passing quietly if it is missing.
#
# `just emulate` is deliberately not here. That one runs the features against a
# nested desktop of its own, which is minutes and a compositor, and a gate
# somebody starts dreading is a gate somebody starts going around. The device
# tier is not here either, for a better reason: it is somebody's machine, and
# it belongs at the end of a deploy rather than before one.
#
# Because the checks above have already run, the device tier asks the machine
# only what nothing here can answer and says of the rest where it was answered.
# `--all` is the whole tier when the question is about the hardware.

# everything that must hold before a deploy
ready:
    cargo build --quiet --locked --workspace --all-features
    cargo test --quiet --locked --workspace --all-features
    cargo clippy --quiet --locked --workspace --all-targets --all-features -- -D warnings
    cargo run --quiet --bin console-check
    just explicit-gate

# The EXPLICIT_* rules, counted rather than enforced.
#
# Deliberately not in `ready`, because what it counts is the warned tier:
# production code is held to the denied rules by `just explicit-gate`, and this
# is where a rule the code has not caught up with says how far there is left to
# go. Nothing stands there now -- 002 was the last out -- so this counts what
# the gate already enforces, and waits for whatever is written ahead of the
# code next. tools/explicit-rust/README.md says what each rule is for.
#
# Capped to warnings so the run reaches every crate. Left uncapped it stops at
# the first one that fails, which is the first one alphabetically and tells
# nobody anything.
#
# `cargo dylint` needs `rustup` on PATH: it asks the toolchain what it is
# before it builds anything. cargo finds the `cargo-dylint` subcommand in
# CARGO_HOME/bin whether or not that is on PATH, so without this the run gets
# far enough to look like it worked and then dies -- and a summary that greps
# for warnings reports a clean workspace when nothing was ever linted. That is
# the one failure this recipe must never repeat, so the run is checked before
# it is counted.

# the EXPLICIT_* rules, by kind, by count
explicit:
    #!/usr/bin/env sh
    set -eu
    PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
    export PATH
    command -v rustup >/dev/null 2>&1 || {
        echo "explicit: rustup is not on PATH; the lint suite cannot name its toolchain" >&2
        exit 1
    }
    out=$(mktemp)
    trap 'rm -f "$out"' EXIT
    RUSTFLAGS="--cap-lints warn" cargo dylint --all -- \
      --locked --all-targets --all-features >"$out" 2>&1 || {
        echo "explicit: the lint run itself failed -- the counts below would be a lie" >&2
        grep -E '^error' "$out" | head -20 >&2
        exit 1
    }
    grep '^warning: ' "$out" | grep -v 'generated' \
    | sed -E 's/`[^`]*`/X/g' | sort | uniq -c | sort -rn

# The rules the workspace already keeps, enforced rather than counted.
#
# Every rule in the suite runs over every crate. This recipe is about which
# ones are allowed to fail the build, and the answer is: the ones nothing in
# the tree breaks. A rule moves out of ALLOW and into the gate when the last
# call site that broke it is fixed, and it never moves back.
#
# That is the whole ratchet. There is no count here on purpose -- a number in a
# recipe is a number that goes stale the first time somebody writes a line of
# code. `just explicit` says where the workspace actually stands, today, and it
# is the only thing that should be believed about the distance.
#
# Tests are exempt inside the lints themselves, so this is production code and
# nothing else. A test that panics is a test that fails, which is what a test
# is for.

# The ALLOW list this recipe once carried is gone for good: a rule's tier now
# lives in its own crate, as the level in `declare_late_lint!`. Every rule the
# workspace keeps is Deny and fails this gate, and today that is every rule
# there is -- nothing stands in the warned tier, which is where a rule written
# ahead of the code prints its remaining distance on every run without blocking
# it. `just explicit` is where that distance is read, for whatever waits there
# next. A rule moves from Warn to Deny in its own source when the last call site
# that broke it is fixed, and it never moves back. The last out was 002, which
# had waited longest because it had nowhere to point until `console-never` was
# written; before it, 020 came out the way a rule should not have to: the
# comments it forbids had been swept out of the tree once already, by hand, and
# were back in most of the crates by the time anybody looked. A rule kept by
# memory is a rule with a half-life.
#
# 013 is the one rule here that can be applied rather than only reported:
# `cargo dylint --fix --lib explicit013_breathing_room` puts the missing blank
# lines in. Every other rule in the suite is asking for a decision, which is
# not a thing to hand to a machine.
explicit-gate:
    #!/usr/bin/env sh
    set -eu
    PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
    export PATH
    command -v rustup >/dev/null 2>&1 || {
        echo "explicit-gate: rustup is not on PATH; the lint suite cannot name its toolchain" >&2
        exit 1
    }
    cargo dylint --all -- --locked --all-targets --all-features

# the tier that makes real input devices, if it can
live:
    cargo test --quiet -p console-controller --test really_running -- --nocapture

# `--features` because the emulator reads the profiles, and reading one is
# behind `console-gamepad/read` -- off by default so the handheld does not
# compile a YAML parser for a program it never runs. `console-emulate` says it
# needs the feature rather than quietly not existing without it.

# a Legion Go on this machine, to press
emulate:
    cargo run --quiet --features console-gamepad/read --bin console-emulate

# every feature, tried again, here
checks:
    cargo run --quiet --bin console-check

# Minutes rather than a fraction of a second, because each of these is a whole
# compositor of its own. The build is first, and it is the whole workspace on
# purpose: the nested session stages whatever is in target/debug and rebuilds
# only itself, so a program nobody built is a picture in which nothing was
# pressed, taken by a check that then says the feature is broken.

# the checks written for a screen, pressed against a nested desktop
desktop-checks:
    cargo build --quiet --workspace
    cargo run --quiet --bin console-check -- --stage desktop

# one panel, opened alone, held to what the buttons promise a hand
#
# A name after it runs one panel, which is the whole point of the tier: a change
# to the files panel is tried against the files panel in about fifteen seconds.
#
#   just panel-checks the_files
panel-checks name="":
    cargo build --quiet --workspace
    cargo test --quiet -p console-panel --test every_panel_answers_a_finger -- {{name}}
    cargo test --quiet -p console-viewer --test a_finger -- {{name}}

# what those would do to the device
device-checks:
    cargo run --quiet --bin console-check -- --stage device --dry

# and what the whole tier would do, emulator second opinions and all
device-checks-all:
    cargo run --quiet --bin console-check -- --stage device --dry --all

# the device's desktop here, in a window
desktop:
    cargo run --quiet --bin console-desktop -- run

# a picture of it, at the device's size
shot:
    cargo run --quiet --bin console-desktop -- shot desktop.png
    @echo "desktop.png"

# The device compiles it, out of the tree `just deploy` pushed there, so this
# describes whatever was last deployed. Deploy first if that matters.

# write down the real devices again
capture:
    ssh {{ HOST }} cargo run --release --locked --quiet \
      --manifest-path /etc/console/Cargo.toml --bin capture-devices \
      > crates/console-gamepad/fixtures/devices.json
    git diff --stat crates/console-gamepad/fixtures/devices.json

# what deploying would change, changing nothing
check:
    cargo run --quiet --bin console-deploy -- --check

# put this on the device and apply it
deploy:
    cargo run --quiet --bin console-deploy

# move a device still called legion over, once
migrate:
    cargo run --quiet --bin console-migrate

# take what was changed on the device back
pull:
    cargo run --quiet --bin console-pull

clean:
    rm -rf .stage target
