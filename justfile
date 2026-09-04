# The loop: change something, run `just test`, and only then `just deploy`.
#
# Nothing has to be remembered before a deploy. `just ready` is the whole list
# and `tools/console-deploy` runs it itself, so what reaches the device has
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

# draw the wallpaper again, out of the palette
garden:
    cargo run --quiet --release --bin console-garden

# press the wallpapers the table names
sky:
    cargo run --quiet --release --bin sky-press

# `--all-features` because the keyboard's Rust port is behind one. It is off by
# default so the device does not compile cairo and pango for a program it does
# not run, and on here so that being unfinished is not the same as being
# untested.

# every test that can run on this machine
test:
    cargo test --quiet --workspace --all-features

# What a deploy runs before it sends anything, and the only place the list
# lives.
#
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
    cargo test --quiet --locked --workspace --all-features
    cargo clippy --quiet --locked --workspace --all-targets --all-features -- -D warnings
    cargo run --quiet --bin console-check
    just explicit-gate

# The EXPLICIT_* rules, counted rather than enforced.
#
# Deliberately not in `ready` yet. Production code is held to them by
# `just explicit-gate`; the whole-workspace count below still includes the
# rules that no call site passes. tools/explicit-rust/README.md says what each
# rule is for.
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
# lives in its own crate, as the level in `declare_late_lint!`. The rules the
# workspace keeps are Deny and fail this gate; the rules it is growing into --
# 013 through 019, the warned tier -- print their remaining distance on every
# run without blocking it. A rule moves from Warn to Deny in its own source
# when the last call site that broke it is fixed, and it never moves back.
# The two that were last out -- 001 and 006 -- came out when the last call site
# that swallowed a failure was answered, and by the rule above they do not go
# back.
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

# a Legion Go on this machine, to press
emulate:
    cargo run --quiet --bin console-emulate

# every feature, tried again, here
checks:
    cargo run --quiet --bin console-check

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
      > crates/console-pad/fixtures/devices.json
    git diff --stat crates/console-pad/fixtures/devices.json

# what deploying would change, changing nothing
check:
    tools/console-deploy --check

# put this on the device and apply it
deploy:
    tools/console-deploy

# move a device still called legion over, once
migrate:
    tools/console-migrate

# take what was changed on the device back
pull:
    tools/console-pull

clean:
    rm -rf .stage target
