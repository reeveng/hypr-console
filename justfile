# The loop: change something, run `just test`, and only then `just deploy`.
#
# Nothing has to be remembered before a deploy. `just ready` is the whole list
# and `console-deploy` runs it itself, so what reaches the device has
# passed it whether or not anyone thought to.

# The device, which only someone with one can name. The tools read it too, and
# say so if it is not set.
HOST := env_var_or_default("CONSOLE_HOST", "")

# what this can do, which is what `just` on its own says
default:
    @just --list --unsorted

# Everything a run starts, in a control group of its own.
#
# A nested desktop is a compositor, and that compositor starts a session, a bar,
# a keyboard and everything those reach for. Killing the compositor reaches none
# of them: they are reparented to the user manager and stay in the control group
# of whoever is logged in, where nothing can tell them from the desktop someone
# is using -- which is why they are found weeks later by `ps` and never by a
# program. Thirty-six gigabytes of stages and an hour-old compositor on a hidden
# workspace is what that looks like in the end.
#
# A transient scope is the one handle a whole run has. Whatever it starts, at
# whatever depth, is inside it, and stopping the unit takes all of it without
# naming a single process -- which matters here more than anywhere, because the
# names are the session's own and `pgrep -x Hyprland` on this laptop matches the
# compositor the person is looking at.
#
# It costs the runs nothing, which took some finding out. A scope looked for a
# while like it made one of the panel tier's seven sessions fail to come up about
# a third of the time, and the reason it looked that way is worth keeping: the
# sweep this landed beside was asking a machine in the hot path, and deleting the
# runtime directory of a compositor that had registered and was not answering
# hyprctl yet. Every scoped run was measured with that in the tree and every
# unscoped one without it. With the sweep asking only what is in /proc, the tier
# passes inside a scope in the same seventeen seconds it takes outside one.
#
# The lesson is the measurement rather than the scope: two changes were in the
# tree at once and the A and the B differed by both of them.
#
# This is not where a nested desktop is held. `console-desktop` puts its own
# compositor in a scope of its own, because the panel tier is a `cargo test` and
# is run as often without `just` as with it -- and a scope made inside a scope is
# its sibling rather than its child, so stopping this one would not have reached
# it either way. What this one is for is everything else a run starts.
#
# A machine with no user manager to ask runs the command as it always did, and
# says so once rather than failing at something that is not the test.

most := "32G"

threads := "8192"

[private]
alone +command:
    #!/usr/bin/env bash
    set -uo pipefail
    unit="console-run-$$"
    if ! command -v systemd-run >/dev/null 2>&1 || ! systemctl --user show-environment >/dev/null 2>&1; then
        echo "no user manager here, so this run is not in a group of its own" >&2
        {{command}}
        exit $?
    fi
    systemd-run --user --scope --quiet --unit="$unit" \
        -p MemoryHigh={{most}} -p TasksMax={{threads}} -- {{command}}
    status=$?
    systemctl --user stop --no-block "$unit.scope" >/dev/null 2>&1
    exit $status

# How the running desktop is connected, drawn again.
#
# Two halves, and only one of them is text. Which units the manifest enables,
# what each starts and what it is ordered against are in `desktop.conf` and
# `files/`, and `console-architecture` reads them. What a program does once it
# is running -- what it runs, which pool topic it subscribes to, whether it
# asks the compositor or opens a socket or keeps a clock -- is in its code, and
# that is asked of the compiler: `tools/explicit-rust/architecture_facts` is a
# dylint pass that warns about nothing and writes a file per crate instead.
#
# A crate cargo has cached is never handed to a lint, so a second run over an
# unchanged tree would write nothing and the map would say half the desktop
# does nothing. The run has a target directory of its own so the gate's cache
# is not the one thrown away, and the workspace's own crates are cleaned out of
# it first -- the dependencies stay, which is most of the time. Then
# `console-architecture` asks `cargo metadata` for every library and program
# and refuses to draw if one of them left no file.
#
# What it writes is in `docs/architecture/`: the facts, the Graphviz source,
# and a picture of it when `dot` is on the machine. The docs rather than the
# crate, because the picture is the one thing in the tree meant to be looked
# at rather than read, and because the facts are what the rules in
# `console-architecture/tests` are asked over -- the crate is the reading of
# them, not where they live. `the_map` holds the source against what the tree
# would draw now; `ready` runs this and fails if a line changed, which is the
# half no test on stable can collect.

# draw how the running desktop is connected
map:
    #!/usr/bin/env sh
    set -eu
    PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
    export PATH
    command -v rustup >/dev/null 2>&1 || {
        echo "map: rustup is not on PATH; the lint suite cannot name its toolchain" >&2
        exit 1
    }
    facts="$PWD/target/architecture/facts"
    rm -rf "$facts"
    for checked in target/architecture/dylint/target/*/; do
        [ -d "$checked" ] && cargo clean --quiet --workspace --target-dir "$checked"
    done
    CARGO_TARGET_DIR=target/architecture CONSOLE_ARCHITECTURE_FACTS="$facts" \
        cargo dylint --quiet --path tools/explicit-rust --pattern architecture_facts -- \
        --quiet --locked --workspace --lib --bins --all-features
    cargo run --quiet --locked --bin console-architecture -- "$facts"
    if command -v dot >/dev/null 2>&1; then
        dot -Tsvg docs/architecture/map.dot -o docs/architecture/map.svg
    else
        echo "map: no graphviz here, so docs/architecture/map.svg is as it was" >&2
    fi

# write the palette into every file that spends it
theme:
    cargo run --quiet --release --bin console-palette

# press the wallpapers the table names
sky:
    cargo run --quiet --release --bin wallpaper-render

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
    @just alone cargo build --quiet --workspace --all-features
    @just alone cargo test --quiet --workspace --all-features

# The tree's own accent, printed rather than enforced.
#
# The enforcing is in `just test` already, because the gate is a test like any
# other: a word written far out of English's proportion is either under a
# heading in `words.conf` or the run is red. This is the same measurement with
# the table left on the screen, which is the half worth reading when nothing is
# failing -- the words at the top are what this desktop sounds like, in order,
# and a word no one meant to lean on is visible there long before it is a habit.

# the words this tree writes furthest out of English's proportion
words:
    cargo test --quiet -p console-vocabulary --test the_words -- --nocapture

# The number literals that would take rustc's fallback, by crate.
#
# Counted rather than enforced, for the same reason the warned EXPLICIT tier is:
# `ready` runs clippy with `-D warnings`, so this rule cannot be a `warn` in
# Cargo.toml without being a `deny` at the gate before a single call site has
# moved. The lint is allowed there and asked for here, and a crate leaves this
# list when every literal in it says its own width.

# the number literals whose type no one wrote
literals:
    cargo clippy --quiet --workspace --all-targets --all-features \
        --message-format=short -- -W clippy::default_numeric_fallback 2>&1 \
        | grep 'default numeric fallback' \
        | awk -F/ '{print $2}' | sort | uniq -c | sort -rn

# What a deploy runs before it sends anything, and the only place the list
# lives.
#
#   the programs    because `cargo test` does not leave them built. Only a
#                   crate with a `tests/` directory of its own gets its program
#                   relinked, and the panel tier opens those programs; without
#                   this the tier stops and says to run the build by hand,
#                   which would make this list no longer the whole list.
#   the tests       what the desktop promises about itself
#   clippy          denied rather than printed, because a warning no one is
#                   made to read is a warning no one reads
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
#   the map         collected again and held against the one committed, because
#                   what a program does is in its code and only the lint suite's
#                   nightly can ask the compiler for it. The picture is left out
#                   of the comparison: Graphviz lays it out differently from one
#                   version to the next, and the source is the thing drawn.
#
# `just emulate` is deliberately not here. That one runs the features against a
# nested desktop of its own, which is minutes and a compositor, and a gate
# someone starts dreading is a gate someone starts going around. The device
# tier is not here either, for a better reason: it is someone's machine, and
# it belongs at the end of a deploy rather than before one.
#
# Because the checks above have already run, the device tier asks the machine
# only what nothing here can answer and says of the rest where it was answered.
# `--all` is the whole tier when the question is about the hardware.
#
# A pass is written down against the tree it passed on, and the same tree is
# not asked again: a second `just deploy` with nothing changed goes straight to
# the device instead of spending the minutes a second time. The tree is git's
# own hash of every tracked file, so one changed byte is a tree nothing has
# passed on, and a checkout with anything uncommitted or untracked in it is
# never written down and never skipped, because that is not a tree at all.
# The mark lives beside the deploy lock in the git directory, so writing it
# does not dirty what it describes.

# everything that must hold before a deploy
ready:
    #!/usr/bin/env bash
    set -euo pipefail
    passed="$(git rev-parse --git-common-dir)/console-ready"
    tree="$(git rev-parse 'HEAD^{tree}')"
    loose="$(git status --porcelain)"
    case "$loose:$(cat "$passed" 2>/dev/null || true)" in
        ":$tree") echo "ready already passed on this tree ($tree), nothing to ask again"; exit 0 ;;
        *) ;;
    esac
    just alone cargo build --quiet --locked --workspace --all-features
    just alone cargo test --quiet --locked --workspace --all-features
    just alone cargo clippy --quiet --locked --workspace --all-targets --all-features -- -D warnings
    just alone cargo run --quiet --bin console-check
    just explicit-gate
    just map
    git diff --exit-code --stat -- docs/architecture/facts.jsonl docs/architecture/map.dot
    just rename check
    case "$loose:$(git status --porcelain):$(git rev-parse 'HEAD^{tree}')" in
        "::$tree") echo "$tree" > "$passed" ;;
        *) echo "ready passed, but the tree was not committed and still, so the pass is not written down" ;;
    esac

# The words `vocabulary.conf` retired, renamed one definition at a time.
#
# `plan` writes `renames.plan` -- every place a retired word is still defined,
# with the table's options beside it -- and keeps every choice already written
# there. `apply` hands each settled line to rust-analyzer, which renames that
# definition and exactly the uses that refer to it. `check` fails while a
# retired word is defined anywhere the plan did not keep it.
rename *arguments:
    @cargo run --quiet --release --manifest-path tools/rename-words/Cargo.toml -- {{arguments}}

# The EXPLICIT_* rules, counted rather than enforced.
#
# Deliberately not in `ready`, because what it counts is the warned tier:
# production code is held to the denied rules by `just explicit-gate`, and this
# is where a rule the code has not caught up with says how far there is left to
# go. Nothing stands there today -- 047 and 048 were the last two and came out
# together -- so what this prints is a clean run until someone writes the next
# rule ahead of the code. It is the whole tree rather than production alone, which is the
# other half of why it is worth running even when the tier is empty.
# tools/explicit-rust/README.md says what each rule is for.
#
# Capped to warnings so the run reaches every crate. Left uncapped it stops at
# the first one that fails, which is the first one alphabetically and tells
# no one anything.
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
# recipe is a number that goes stale the first time someone writes a line of
# code. `just explicit` says where the workspace actually stands, today, and it
# is the only thing that should be believed about the distance.
#
# Tests are exempt inside the lints themselves, so this is production code and
# nothing else. A test that panics is a test that fails, which is what a test
# is for.

# The ALLOW list this recipe once carried is gone for good: a rule's tier now
# lives in its own crate, as the level in `declare_late_lint!`. Every rule in
# the suite is Deny and fails this gate; nothing stands warned. A rule moves
# from Warn to Deny in its own source when the last call site that broke it is
# fixed, and it never moves back.
#
# 047 and 048 were the last two out, and they came out together. 047's sites
# were one design question repeated: an iterator word standing in for a loop,
# and a closure handed nothing and then given what it needed by capture -- a
# wait, a stretch of an apply, the sentence a failed check prints, the line a
# run draws quietly. Each of those has a `_handed` spelling now that takes the
# thing it is asked with and hands it in at the call. 048's sites split: some were the flat enum it is
# named for and are enums, and the rest were fields that really are independent
# and carry the allow with a sentence saying so.
#
# The five written before them were read off a rule taxonomy someone keeps for C++,
# which is worth saying because of what it cost: most of that list was already
# answered here or is a fault Rust will not compile, and five questions came
# back that this tree could be asked. Two had call sites -- 035, a thread let
# go without the word being said, and 036, a program named by a string that no
# list of what this desktop runs can see. Two arrived green and are ratchets
# rather than sweeps, 034 and 037. The fifth is 038, the argument the suite had
# been having with itself since 001, and it broke the tree in more crates than
# anything since 019: a fault said as a `String` is one no caller can decide
# anything about, and the answer is an enum per crate naming the ways its own
# calls fail. It came out bottom-up, a crate at a time, the way 002 went.
#
# Before them 033 arrived denied
# with the tree breaking it in more places than anything before it: the
# `unwrap_or` family over an `Option`, which is 001's rule about a `Result` said
# where there is no error to swallow, and what it found in three hundred-odd
# sites was mostly a `match` with both arms and a named value, twice a real fault
# wearing a default, and three times one answer written out in several crates --
# `console-core-walking` is where the first of those went. Before it 024, which
# arrived with the tree breaking it in more places than anything since 019 and
# came out a family of quantities at a time rather than a function at a time.
# Before it, 026
# through 032, seven at once, which arrived together and
# came out together. Before them 023, which went the way 019 went -- every `let … else` in the tree turned
# into a `match` in the initializer. Before it, 022, and what let that one out was not a check
# but what a wait may carry: `Device::until` now carries the fault its question
# carries, so a question that reads the screen -- which is what the home
# screen's checks were waiting on -- can be handed to one. Before it, 002 had
# waited longest because it had nowhere to point until `console-core-never` was
# written; before that, 020 came out the way a rule should not have to: the
# comments it forbids had been swept out of the tree once already, by hand, and
# were back in most of the crates by the time anyone looked. A rule kept by
# memory is a rule with a half-life.
#
# 013 is the one rule here that can be applied rather than only reported:
# `cargo dylint --fix --lib explicit013_breathing_room` puts the missing blank
# lines in. Every other rule in the suite is asking for a decision, which is
# not a thing to hand to a machine.
#
# The kernel is linted a second time for the firmware, because the half of it
# that talks to the firmware only exists when it is built for it; that run
# wants `rustup target add --toolchain` of the suite's nightly for
# `x86_64-unknown-uefi`.
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
    cargo dylint --all -- --locked -p console-kernel --target x86_64-unknown-uefi

# The kernel is a member of the workspace like any other crate, built here for
# the firmware rather than for this machine. It runs in QEMU on OVMF with the
# exit port wired, and QEMU reports the kernel's answer shifted left and ored
# with one: one is every step held, anything else is not.

# the kernel, booted in a virtual machine
kernel:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --quiet --release -p console-kernel --target x86_64-unknown-uefi
    mkdir -p target/kernel/esp/EFI/BOOT
    cp target/x86_64-unknown-uefi/release/console-kernel.efi target/kernel/esp/EFI/BOOT/BOOTX64.EFI
    set +e
    timeout 60 qemu-system-x86_64 -machine q35 -m 256M -bios /usr/share/edk2/x64/OVMF.4m.fd \
        -drive format=raw,file=fat:rw:target/kernel/esp -display none -serial stdio \
        -device isa-debug-exit,iobase=0xf4,iosize=0x04 -no-reboot \
        | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | tr -d '\r' | grep -a --line-buffered '^console-kernel'
    answered=${PIPESTATUS[0]}
    set -e
    [ "$answered" -eq 1 ] || { echo "kernel: QEMU answered $answered" >&2; exit 1; }

# the tier that makes real input devices, if it can
live:
    cargo test --quiet -p console-input-controller --test really_running -- --nocapture

# `--features` because the emulator reads the profiles, and reading one is
# behind `console-input-gamepad/read` -- off by default so the handheld does not
# compile a YAML parser for a program it never runs. `console-emulate` says it
# needs the feature rather than quietly not existing without it.

# a Legion Go on this machine, to press
emulate:
    @just alone cargo run --quiet --features console-input-gamepad/read --bin console-emulate

# every feature, tried again, here
checks:
    @just alone cargo run --quiet --bin console-check

# Minutes rather than a fraction of a second, because each of these is a whole
# compositor of its own. The build is first, and it is the whole workspace on
# purpose: the nested session stages whatever is in target/debug and rebuilds
# only itself, so a program no one built is a picture in which nothing was
# pressed, taken by a check that then says the feature is broken.

# the checks written for a screen, pressed against a nested desktop
desktop-checks:
    @just alone cargo build --quiet --workspace
    @just alone cargo run --quiet --bin console-check -- --stage desktop

# one panel, opened alone, held to what the buttons promise a hand
#
# A name after it runs one panel, which is the whole point of the tier: a change
# to the files panel is tried against the files panel in about fifteen seconds.
#
#   just panel-checks the_files
panel-checks name="":
    @just alone cargo build --quiet --workspace
    @just alone cargo test --quiet -p console-panel --test every_panel_answers_a_finger -- {{name}}
    @just alone cargo test --quiet -p console-media-viewer --test a_finger -- {{name}}

# what those would do to the device
device-checks:
    cargo run --quiet --bin console-check -- --stage device --dry

# and what the whole tier would do, emulator second opinions and all
device-checks-all:
    cargo run --quiet --bin console-check -- --stage device --dry --all

# the device's desktop here, in a window
desktop:
    @just alone cargo run --quiet --bin console-desktop -- run

# a picture of it, at the device's size
shot:
    @just alone cargo run --quiet --bin console-desktop -- shot desktop.png
    @echo "desktop.png"

# The device compiles it, out of the tree `just deploy` pushed there, so this
# describes whatever was last deployed. Deploy first if that matters.

# write down the real devices again
capture:
    ssh {{ HOST }} cargo run --release --locked --quiet \
      --manifest-path /etc/console/Cargo.toml --bin capture-devices \
      > crates/console-input-gamepad/fixtures/devices.json
    git diff --stat crates/console-input-gamepad/fixtures/devices.json

# What the device measured for itself in its last check run, carried back here
# so a machine that has never been checked still gets a bar shaped like the run
# rather than one that counts. The device has to have run the checks at least
# once; `console-check --stage device --yes` is what writes the file.

# take the check lengths off the device again
lengths:
    ssh {{ HOST }} "cat /home/*/.local/state/console/checked" \
      | sort > crates/console-test-stages/fixtures/lengths
    git diff --stat crates/console-test-stages/fixtures/lengths

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
