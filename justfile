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

# Check external dependencies on this machine.
[positional-arguments]
dependencies *arguments:
    @cargo run --locked --quiet --bin console-dependencies -- --report-only "$@"

# format the Rust workspace
format:
    cargo fmt

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


[private]
alone +command:
    @cargo x alone {{command}}

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
# A crate cargo has cached is never handed to a lint, so a crate's file is the
# one written the last time it was built, and it stands until cargo builds it
# again -- which it does when the crate's sources change, when a crate it uses
# does, or when the lint does, since the driver names every lint it loads as
# an input of every crate it checks. That is the same question a stale file
# would be wrong about, answered by the one thing that already keeps it. The
# run has a target directory of its own, so no other check's cache answers for
# it and the facts have one writer. It used to clean the workspace's crates out
# of that directory every time, which made every run check the whole
# workspace again under the nightly to learn what had not changed; now that is
# done only when the facts are missing altogether. Then
# `console-architecture` asks `cargo metadata` for every library and program
# and refuses to draw if one of them left no file.
#
# What it writes is in `docs/architecture/`: the facts, the Graphviz source,
# a picture of it when `dot` is on the machine, and `map.html`, the same map
# as a page a browser opens, where a node tapped lights what it touches. The
# docs rather than the crate, because the pictures are the one thing in the
# tree meant to be looked at rather than read, and because the facts are what
# the rules in `console-architecture/tests` are asked over -- the crate is the
# reading of them, not where they live. `just map-check` checks the saved map
# and its facts on demand.

# draw how the running desktop is connected
map:
    @cargo x map

# validate the saved architecture map and its facts
map-check:
    cargo test --locked -p console-architecture --test the_map --test the_rules -- --include-ignored

# write the palette into every file that spends it
theme:
    cargo run --quiet --release --bin console-palette

# press the wallpapers the table names
sky:
    cargo run --quiet --release --bin wallpaper-render

# `--all-features` because the keyboard's Rust port is behind one. It is off by
# default so the device does not compile the rasterizer and text engine for a program it does
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
#
# The run is nextest's rather than `cargo test`'s. `cargo test` walks the test
# binaries one after another, and with a crate per job that is most of the
# wait; nextest runs every test in a process of its own, all binaries at once.
# The one thing it does not run is a doctest, and this tree has none, since a
# doc comment is a comment.

# every test that can run on this machine
test:
    @cargo x test

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
#
# The rename check and the word count go first: they are the ones a
# change most often breaks, and each answers in seconds where the tests take
# minutes, so a red one is heard before the long wait rather than after it.
#
# The tests, clippy and the rules are asked only of the crates the change since
# the last passed tree can reach -- `cargo x reached` says which and why. The
# build and the checks stay whole: the build is cargo's own to keep small, and
# the checks are seconds.

# everything that must hold before a deploy
ready:
    @cargo x ready

# The words `vocabulary.conf` retired, renamed one definition at a time.
#
# `plan` writes `renames.plan` -- every place a retired word is still defined,
# with the table's options beside it -- and keeps every choice already written
# there. `apply` hands each settled line to rust-analyzer, which renames that
# definition and exactly the uses that refer to it. `pick` is the plan's open
# lines one at a time, the code around each on screen and a key per option,
# written back into the plan as each is answered. `check` fails while a
# retired word is defined anywhere the plan did not keep it, except a word
# under a `(walking)` heading, which it counts instead.
#
# `at FILE:LINE OLD NEW` is the same rename for any one name, whatever it is
# and whether or not the table knows the word: a function, a field, a local, a
# parameter. Several triples in one call share one rust-analyzer start.
rename *arguments:
    @cargo run --quiet --release --manifest-path tools/rename-words/Cargo.toml -- {{arguments}}

# The EXPLICIT_* rules, counted rather than enforced.
#
# This prints the legacy workspace summary. The merge audit with the owned
# forks and the default and all-features build configurations is explicit-gate.
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
    RUSTFLAGS="--cap-lints warn" cargo x rules >"$out" 2>&1 || {
        echo "explicit: the lint run itself failed -- the counts below would be a lie" >&2
        grep -E '^error' "$out" | head -20 >&2
        exit 1
    }
    grep '^warning: ' "$out" | grep -v 'generated' \
    | sed -E 's/`[^`]*`/X/g' | sort | uniq -c | sort -rn

# Verify the Explicit Rust merge gate. Supply a pinned report with --baseline
# while the backlog is open. A scoped audit without a baseline requires zero.
[positional-arguments]
explicit-gate *scope:
    @just explicit-batch audit "$@"

# Verify that unsafe Rust is confined to the recorded sites outside the kernel.
[positional-arguments]
unsafe-rust *arguments:
    @mkdir -p target/unsafe-rust
    @sysroot=$(rustc +nightly-2026-05-28 --print sysroot) && \
      rustc +nightly-2026-05-28 --edition=2024 -O -L "$sysroot/lib" -C "link-args=-Wl,-rpath,$sysroot/lib" tools/unsafe-rust.rs -o target/unsafe-rust/check
    @target/unsafe-rust/check "$@"

# Apply compiler-checked EXPLICIT suggestions to one package.
[positional-arguments]
explicit-fix package *flags:
    @just _explicit-fix --all-targets -p "$@"

# Rewrite supported if expressions and boolean matches as enum matches.
[positional-arguments]
explicit-fix-branches *flags:
    DYLINT_TOML="${DYLINT_TOML-explicit019_structural_only = true}" DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+$DYLINT_RUSTFLAGS }--cap-lints=warn" cargo dylint --fix --no-deps --lib explicit019_no_if -- "$@"

# Apply and recheck all automatic Explicit Rust fixes.
[positional-arguments]
explicit-rust action *flags:
    @test "$1" = fix-all || { printf 'Run just explicit-rust fix-all [OPTIONS].\n' >&2; exit 1; }
    @shift && \
        if [ "${DYLINT_TOML+x}" != x ] && [ ! -f dylint.toml ]; then \
            DYLINT_TOML="$(printf '%s\n' 'explicit019_structural_only = true' && cat \
                tools/explicit-rust/explicit007_no_bool_return/font-type1-outcomes.toml \
                tools/explicit-rust/explicit008_no_bool_param/reviewed/av1-rectangular-scale.toml \
                tools/explicit-rust/explicit008_no_bool_param/reviewed/av1-superblock-size.toml \
                tools/explicit-rust/explicit008_no_bool_param/reviewed/av1-adst-row-order.toml \
                tools/explicit-rust/explicit008_no_bool_param/reviewed/av1-dc-shortcut.toml)" || exit; \
            export DYLINT_TOML; \
        fi && exec just explicit-batch fix-all "$@"

# Apply wrapping arithmetic after choosing that policy for a package.
[positional-arguments]
explicit-wrap package *flags:
    DYLINT_TOML='explicit015_wrapping = true' DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+$DYLINT_RUSTFLAGS }--cap-lints=warn" cargo dylint --fix --no-deps --lib explicit015_no_bare_arithmetic -- --all-targets -p "$@"

# Apply the same fixes across every host crate and the UEFI kernel.
[positional-arguments]
explicit-fix-all *flags:
    @just _explicit-fix --workspace --all-targets --all-features "$@"
    @just _explicit-fix -p console-kernel --target x86_64-unknown-uefi --lib --bins --all-features "$@"

# Apply the reviewed public-API, test, decoder, and local pipe fixes together.
[positional-arguments]
explicit-fix-reviewed *flags:
    #!/usr/bin/env bash
    set -eu
    rules=tools/explicit-rust
    export DYLINT_TOML="$(printf '%s\n' 'explicit051_public_compatibility = true' 'explicit024_typed_coordinates = true' 'explicit004_test_results = true' 'explicit004_fixed_array_bounds = true' 'explicit019_structural_only = true'; cat \
        "$rules/explicit017_question_mark_alone/av1-first-slice.toml" \
        "$rules/explicit073_no_shadowing/av1-macros.toml" \
        "$rules/explicit091_closes_on_exec/local-pipes.toml" \
        "$rules/explicit014_no_index_slice/decoder-bounds.toml" \
        "$rules/explicit007_no_bool_return/font-type1-outcomes.toml" \
        "$rules/explicit008_no_bool_param/reviewed/av1-rectangular-scale.toml" \
        "$rules/explicit008_no_bool_param/reviewed/av1-superblock-size.toml" \
        "$rules/explicit008_no_bool_param/reviewed/av1-adst-row-order.toml" \
        "$rules/explicit008_no_bool_param/reviewed/av1-dc-shortcut.toml")"
    just explicit-fix-all "$@"

# Inventory all EXPLICIT rules and apply selected Dylint fixes in verified batches.
[positional-arguments]
explicit-batch *flags:
    cargo +nightly-2026-05-28 run --offline --locked --manifest-path tools/explicit-rust/migration-collector/Cargo.toml --target-dir "${TMPDIR:-/var/tmp}/builds/explicit-migration-collector" --bin batch -- "$@"

# Preview or apply the source repairs that use the batch inventory's diagnostics.
[positional-arguments]
explicit-repair *flags:
    cargo +nightly-2026-05-28 run --offline --locked --manifest-path tools/explicit-rust/migration-collector/Cargo.toml --target-dir "${TMPDIR:-/var/tmp}/builds/explicit-migration-collector" --bin repair -- "$@"

# Prepare compiler-guided migration edits from captured compiler facts.
[positional-arguments]
explicit-apis *flags:
    cargo +nightly-2026-05-28 run --offline --locked --manifest-path tools/explicit-rust/migration-collector/Cargo.toml --target-dir "${TMPDIR:-/var/tmp}/builds/explicit-migration-collector" --bin apis -- "$@"

# Apply fixed-array patterns and checked codec lookups.
[positional-arguments]
explicit-index-bulk *flags:
    DYLINT_TOML="$(cat tools/explicit-rust/explicit014_no_index_slice/decoder-bounds.toml)" DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+$DYLINT_RUSTFLAGS }--cap-lints=warn" cargo dylint --fix --no-deps --lib explicit014_no_index_slice -- --all-targets --all-features \
        -p console-core-video-decoding-av1 -p console-core-video-decoding-advanced-video-coding \
        -p console-core-video-decoding-high-efficiency-video-coding -p console-core-video-decoding-vp9 \
        -p console-core-paths -p console-core-path-rasterizer -p console-core-portable-document-syntax -p console-core-font-reading \
        -p console-core-scalable-vector-graphics-values "$@"

# Apply the checked indexing fixes enabled by AV1's typed transform errors.
[positional-arguments]
explicit-av1-transform-bounds *flags:
    DYLINT_TOML="$(cat tools/explicit-rust/explicit014_no_index_slice/av1-transform-bounds.toml)" DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+$DYLINT_RUSTFLAGS }--cap-lints=warn" cargo dylint --fix --no-deps --lib explicit014_no_index_slice -- -p console-core-video-decoding-av1 --lib --all-features "$@"

# Rename AV1 bindings through reviewed macro expansions.
[positional-arguments]
explicit-shadow-av1 *flags:
    DYLINT_TOML="$(cat tools/explicit-rust/explicit073_no_shadowing/av1-macros.toml)" DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+$DYLINT_RUSTFLAGS }--cap-lints=warn" cargo dylint --fix --no-deps --lib explicit073_no_shadowing -- -p console-core-video-decoding-av1 --all-targets --all-features "$@"

[positional-arguments]
_explicit-fix +args:
    DYLINT_RUSTFLAGS="${DYLINT_RUSTFLAGS:+$DYLINT_RUSTFLAGS }--cap-lints=warn" cargo dylint --fix --no-deps --all -- "$@"

# The kernel is a member of the workspace like any other crate, built here for
# the firmware rather than for this machine. It runs in QEMU on OVMF with the
# exit port wired, and QEMU reports the kernel's answer shifted left and ored
# with one: one is every step held, anything else is not.

# the x86-64 UEFI kernel staged in a removable-media layout
kernel-image:
    cargo build --quiet --release -p console-kernel --target x86_64-unknown-uefi --target-dir "${CARGO_TARGET_DIR:-target}"
    install -Dm644 "${CARGO_TARGET_DIR:-target}/x86_64-unknown-uefi/release/console-kernel.efi" target/kernel/esp/EFI/BOOT/BOOTX64.EFI

# boot the removable-media tree with UEFI and check the serial and display milestones
[positional-arguments]
kernel cpu="qemu64" memory="256M": kernel-image
    #!/usr/bin/env bash
    set -euo pipefail
    set +e
    timeout 60 qemu-system-x86_64 -machine q35 -cpu "$1" -m "$2" -bios /usr/share/edk2/x64/OVMF.4m.fd \
        -drive if=none,id=preview,format=raw,file=fat:target/kernel/esp,readonly=on \
        -device qemu-xhci -device usb-storage,drive=preview,removable=on,bootindex=1 \
        -display none -serial stdio \
        -device isa-debug-exit,iobase=0xf4,iosize=0x04 -no-reboot \
        | sed 's/\x1b\[[0-9;]*[A-Za-z]//g' | tr -d '\r' \
        | awk '/^console-kernel/ { print; fflush() } \
            /^console-kernel: every step held$/ { finished = 1 } \
            /^console-kernel: the browser page is on the panel$/ { displayed = 1 } \
            END { exit !(finished && displayed) }'
    statuses=("${PIPESTATUS[@]}")
    set -e
    [ "${statuses[*]}" = "1 0 0 0" ] || { echo "kernel: QEMU exited ${statuses[0]}, and a complete transcript is required" >&2; exit 1; }

# inject keys through a virtual xHCI keyboard and check the portrait screen
kernel-input: kernel-image
    TMPDIR="${TMPDIR:-/var/tmp}" cargo run --quiet -p console-tasks --example kernel-usb -- target/kernel/esp

# check native USB input with AMD-Vi translation and revocation
kernel-input-iommu: kernel-image
    TMPDIR="${TMPDIR:-/var/tmp}" cargo run --quiet -p console-tasks --example kernel-usb -- target/kernel/esp amd-vi

# Scroll local HTML and recover the native renderer after a user page fault.
kernel-browser: kernel-image
    TMPDIR="${TMPDIR:-/var/tmp/$(id -un)}" cargo run --locked --quiet -p console-tasks --example kernel-usb -- target/kernel/esp browser

# Build the working tree's configuration into the Nix store and keep a link to it.
[positional-arguments]
native-build generation="target/kernel/current":
    mkdir -p "$(dirname -- "$1")"
    nix build --impure --file nix/native-generation.nix --out-link "$1"

# Boot a native generation, with an optional previous generation in the firmware menu.
[positional-arguments]
native-boot generation previous="":
    #!/usr/bin/env bash
    set -euo pipefail
    drives=()
    add_drive() {
        local esp id="$2"
        esp=$(realpath -e "$1/esp")
        test -f "$esp/EFI/BOOT/BOOTX64.EFI"
        drives+=(-drive "if=none,id=$id,format=raw,file=fat:${esp//,/,,},readonly=on"
            -device "usb-storage,bus=xhci.0,drive=$id,serial=console-$id,removable=on,bootindex=$3")
    }
    add_drive "$1" current 1
    if [ -n "$2" ]; then add_drive "$2" previous 2; fi
    status=0
    qemu-system-x86_64 -machine q35 -cpu qemu64 -m 256M -bios /usr/share/edk2/x64/OVMF.4m.fd \
        -device qemu-xhci,id=xhci -device usb-kbd,bus=xhci.0 "${drives[@]}" \
        -boot menu=on -nic none -serial stdio -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
        -no-reboot || status=$?
    if [ "$status" -ne 65 ]; then exit "$status"; fi

# Verify the Calculator or launcher in a built native generation over USB input.
[positional-arguments]
native-check generation expected:
    TMPDIR="${TMPDIR:-/var/tmp}" cargo run --locked --quiet -p console-tasks --example kernel-usb -- "$1/esp" "$2"

# Build the optional Linux compute worker.
compute-worker:
    #!/usr/bin/env bash
    set -euo pipefail
    build="${CARGO_TARGET_DIR:-target}/kernel-compute"
    mkdir -p "$build"
    cc -std=c17 -O2 -Wall -Wextra -Werror crates/console-operating-system-linux/compute/vulkan.c -o "$build/vulkan" -lvulkan

# compare batched checksums and their tree reduction on CPU and Vulkan
kernel-gpu-checksums: compute-worker
    #!/usr/bin/env bash
    set -euo pipefail
    build="${CARGO_TARGET_DIR:-target}/kernel-compute"
    for shader in checksums wrapping-sum; do
        cargo run --locked --quiet --release -p console-kernel --features hosted-compute --example gpu-checksums -- --emit-shader "$shader" > "$build/$shader.comp"
        glslangValidator --target-env vulkan1.0 -V "$build/$shader.comp" -o "$build/$shader.spv"
        spirv-val --target-env vulkan1.0 "$build/$shader.spv"
    done
    cargo run --locked --release -p console-kernel --features hosted-compute --example gpu-checksums -- "$build/vulkan" "$build/checksums.spv"

# compare the CPU image-mask reply with separable and fused Vulkan replies
system-gpu-compute: compute-worker
    #!/usr/bin/env bash
    set -euo pipefail
    build="${CARGO_TARGET_DIR:-target}/kernel-compute"
    for shader in mask blur threshold; do
        cargo run --locked --quiet --release -p console-operating-system-linux --example gpu-image-mask -- --emit-shader "$shader" > "$build/$shader.comp"
        glslangValidator --target-env vulkan1.0 -V "$build/$shader.comp" -o "$build/$shader.spv"
        spirv-val --target-env vulkan1.0 "$build/$shader.spv"
    done
    cargo run --locked --release -p console-operating-system-linux --example gpu-image-mask -- --require "$build/vulkan" "$build/blur.spv" "$build/threshold.spv"
    cargo run --locked --release -p console-operating-system-linux --example gpu-image-mask -- --require "$build/vulkan" "$build/mask.spv"

# Measure CPU and Vulkan wall times and show the route selected from recent history.
[positional-arguments]
compute-benchmark profile="standard": compute-worker
    #!/usr/bin/env bash
    set -euo pipefail
    build="${CARGO_TARGET_DIR:-target}/kernel-compute"
    for shader in checksum-bench wrapping-sum mandelbrot; do
        cargo run --locked --quiet --release -p console-kernel --features hosted-compute --example compute-benchmark -- --emit-shader "$shader" > "$build/$shader.comp"
        glslangValidator --target-env vulkan1.0 -V "$build/$shader.comp" -o "$build/$shader.spv"
        spirv-val --target-env vulkan1.0 "$build/$shader.spv"
    done
    cargo run --locked --release -p console-kernel --features hosted-compute --example compute-benchmark -- "$build/vulkan" "$build/checksum-bench.spv" "$build/mandelbrot.spv" "$1"

# Compare decoded-frame readiness and compositor presentation on a live Wayland display.
[positional-arguments]
video-presentation-benchmark clip: compute-worker
    #!/usr/bin/env bash
    set -euo pipefail
    build="${CARGO_TARGET_DIR:-target}/kernel-compute"
    cargo run --locked --quiet --release -p console-application-media-viewer --example video-presentation -- --emit-shader > "$build/video-colour.comp"
    glslangValidator --target-env vulkan1.0 -V "$build/video-colour.comp" -o "$build/video-colour.spv"
    spirv-val --target-env vulkan1.0 "$build/video-colour.spv"
    cargo run --locked --release -p console-application-media-viewer --example video-presentation -- "$1" "$build/vulkan" "$build/video-colour.spv"

# Regenerate the browser extension's packaged WebAssembly with rustup's wasm32-unknown-unknown target.
browser-pad-wasm:
    mkdir -p target/browser-pad
    rustc +1.98.0 --edition=2024 --crate-type=cdylib --target=wasm32-unknown-unknown -C opt-level=z -C lto -C panic=abort -C strip=symbols crates/console-browser-extension/wasm/pad.rs -o target/browser-pad/pad.wasm
    install -m644 target/browser-pad/pad.wasm crates/console-browser-extension/web/pad.wasm

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
    @just alone cargo test --quiet -p console-application-media-viewer --test a_finger -- {{name}}

# Measure the browser toolbar, drawing, launcher search, Files and Calculator.
[positional-arguments]
bench-ui *arguments:
    cargo bench --locked -p console-application-browser -p console-draw-painting -p console-applications -p console-application-files -p console-application-calculator \
        --bench browser_ui --bench picture_painting --bench launcher_search --bench folder_listing --bench calculator_keypad -- "$@"

# what those would do to the device
device-checks:
    cargo run --quiet --bin console-check -- --stage device --dry-run

# and what the whole tier would do, emulator second opinions and all
device-checks-all:
    cargo run --quiet --bin console-check -- --stage device --dry-run --all

# open the experimental browser window on this desktop
[positional-arguments]
browser *arguments:
    cargo build --quiet --locked -p console-application-browser -p console-browser-renderer-chromium
    CONSOLE_BROWSER_SERVO_WORKER="${CONSOLE_SERVO_TARGET_DIR:-crates/console-browser-renderer-servo/target}/debug/console-browser-renderer-servo" "${CARGO_TARGET_DIR:-target}/debug/console-application-browser" "$@"

# load a local website and reopen it in a fresh Chromium context
browser-check:
    cargo build --quiet --locked -p console-application-browser -p console-browser-renderer-chromium --features console-application-browser/checks
    "${CARGO_TARGET_DIR:-target}/debug/console-browser-check" "${CARGO_TARGET_DIR:-target}/debug/console-application-browser"

[private]
browser-servo-build:
    cargo build --locked --manifest-path crates/console-browser-renderer-servo/Cargo.toml --target-dir "${CONSOLE_SERVO_TARGET_DIR:-crates/console-browser-renderer-servo/target}"

[positional-arguments]
browser-servo *arguments: browser-servo-build
    cargo run --quiet --locked -p console-application-browser -- --servo-worker "${CONSOLE_SERVO_TARGET_DIR:-crates/console-browser-renderer-servo/target}/debug/console-browser-renderer-servo" "$@"

browser-servo-check: browser-servo-build
    cargo build --quiet --locked -p console-application-browser -p console-browser-renderer-chromium --features console-application-browser/checks
    "${CARGO_TARGET_DIR:-target}/debug/console-browser-check" "${CARGO_TARGET_DIR:-target}/debug/console-application-browser" "${CONSOLE_SERVO_TARGET_DIR:-crates/console-browser-renderer-servo/target}/debug/console-browser-renderer-servo"

# Run the packaged Chromium and WPE renderers in the Console window.
browser-webkit *arguments:
    nix run .#browser -- "$@"

# Run the browser unit tests and renderer switch in a pinned NixOS VM.
browser-webkit-check:
    nix build --no-link --print-build-logs .#checks.x86_64-linux.browser

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
    ssh {{ HOST }} "cd /etc/console && cargo run --release --locked --quiet \
      --bin capture-devices" > crates/console-input-gamepad/fixtures/devices.json
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

# put this on the device and apply it, asking nothing of the tests
release:
    cargo run --quiet --bin console-deploy -- --untested

# move a device still called legion over, once
migrate:
    cargo run --quiet --bin console-migrate

# take what was changed on the device back
pull:
    cargo run --quiet --bin console-pull

clean:
    rm -rf .stage target
