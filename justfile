# The loop: change something, run `just test`, and only then `just deploy`.
#
# Nothing has to be remembered before a deploy. `just ready` is the whole list
# and `console-deploy` runs it itself, so what reaches the device has
# passed it whether or not anyone thought to.

# Build the recipe runner with its own manifest and lockfile.
runner := "cargo run --locked --quiet --manifest-path tools/just-recipes/Cargo.toml --target-dir target/just-recipes --"

# what this can do, which is what `just` on its own says
default:
    @just --list --unsorted

# Check external dependencies on this machine.
[positional-arguments]
dependencies *arguments:
    @cargo run --locked --quiet --bin console-dependencies -- --report-only "$@"

# Inspect, install, or update the checkout's mise-managed tools.
[positional-arguments]
dev-tools *arguments:
    @cargo run --locked --quiet -p console-package-manager --features tools --bin console-dev-tools -- "$@"

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
# compositor in a scope of its own, including when the test VM runs the panel
# tier -- and a scope made inside a scope is
# its sibling rather than its child, so stopping this one would not have reached
# it either way. What this one is for is everything else a run starts.
#
# A machine with no user manager to ask runs the command as it always did, and
# says so once rather than failing at something that is not the test.

[positional-arguments]
[private]
alone +command:
    @cargo x alone "$@"

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
map-check: (test "architecture")

# write the palette into every file that spends it
theme:
    cargo run --quiet --release --bin console-palette

# press the wallpapers the table names
sky:
    cargo run --quiet --release --bin wallpaper-render

# Run the default VM suites or select a suite.
[positional-arguments]
test suite="quick" *arguments:
    @cargo x test "$@"

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
    @{{ runner }} words

# The number literals that would take rustc's fallback, by crate.
#
# Counted rather than enforced, for the same reason the warned EXPLICIT tier is:
# `ready` runs clippy with `-D warnings`, so this rule cannot be a `warn` in
# Cargo.toml without being a `deny` at the gate before a single call site has
# moved. The lint is allowed there and asked for here, and a crate leaves this
# list when every literal in it says its own width.

# the number literals whose type no one wrote
literals:
    @{{ runner }} literals

# Deployment requires the vocabulary check, test VMs, Clippy and Explicit Rust.
# Clippy and Explicit Rust use the crates reached since the last recorded pass.

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
[positional-arguments]
rename *arguments:
    @cargo run --quiet --release --manifest-path tools/rename-words/Cargo.toml -- "$@"

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
    @{{ runner }} explicit

# Verify the Explicit Rust merge gate. Supply a pinned report with --baseline
# while the backlog is open. A scoped audit without a baseline requires zero.
[positional-arguments]
explicit-gate *scope:
    @{{ runner }} explicit-batch audit "$@"

# Verify that unsafe Rust is confined to the recorded sites outside the kernel.
[positional-arguments]
unsafe-rust *arguments:
    @{{ runner }} unsafe-rust "$@"

# Apply compiler-checked EXPLICIT suggestions to one package.
[positional-arguments]
explicit-fix package *flags:
    @{{ runner }} explicit-fix --all-targets -p "$@"

# Rewrite supported if expressions and boolean matches as enum matches.
[positional-arguments]
explicit-fix-branches *flags:
    @{{ runner }} explicit-fix-branches "$@"

# Apply and recheck all automatic Explicit Rust fixes.
[positional-arguments]
explicit-rust action *flags:
    @{{ runner }} explicit-rust "$@"

# Apply wrapping arithmetic after choosing that policy for a package.
[positional-arguments]
explicit-wrap package *flags:
    @{{ runner }} explicit-wrap "$@"

# Apply the same fixes across every host crate and the UEFI kernel.
[positional-arguments]
explicit-fix-all *flags:
    @{{ runner }} explicit-fix-all "$@"

# Apply the reviewed public-API, test, decoder, and local pipe fixes together.
[positional-arguments]
explicit-fix-reviewed *flags:
    @{{ runner }} explicit-fix-reviewed "$@"

# Inventory all EXPLICIT rules and apply selected Dylint fixes in verified batches.
[positional-arguments]
explicit-batch *flags:
    @{{ runner }} explicit-batch "$@"

# Preview or apply the source repairs that use the batch inventory's diagnostics.
[positional-arguments]
explicit-repair *flags:
    @{{ runner }} explicit-repair "$@"

# Prepare compiler-guided migration edits from captured compiler facts.
[positional-arguments]
explicit-apis *flags:
    @{{ runner }} explicit-apis "$@"

# Apply fixed-array patterns and checked codec lookups.
[positional-arguments]
explicit-index-bulk *flags:
    @{{ runner }} explicit-index-bulk "$@"

# Apply the checked indexing fixes enabled by AV1's typed transform errors.
[positional-arguments]
explicit-av1-transform-bounds *flags:
    @{{ runner }} explicit-av1-transform-bounds "$@"

# Rename AV1 bindings through reviewed macro expansions.
[positional-arguments]
explicit-shadow-av1 *flags:
    @{{ runner }} explicit-shadow-av1 "$@"

[positional-arguments]
_explicit-fix +args:
    @{{ runner }} explicit-fix "$@"

# The kernel is a member of the workspace like any other crate, built here for
# the firmware rather than for this machine. It runs in QEMU on OVMF with the
# exit port wired, and QEMU reports the kernel's answer shifted left and ored
# with one: one is every step held, anything else is not.

# the x86-64 UEFI kernel staged in a removable-media layout
kernel-image:
    @{{ runner }} kernel-image

# boot the removable-media tree with UEFI and check the serial and display milestones
[positional-arguments]
kernel cpu="qemu64" memory="256M": kernel-image
    @{{ runner }} kernel "$@"

# inject keys through a virtual xHCI keyboard and check the portrait screen
kernel-input: kernel-image
    @{{ runner }} kernel-input

# check native USB input with AMD-Vi translation and revocation
kernel-input-iommu: kernel-image
    @{{ runner }} kernel-input amd-vi

# Scroll local HTML and recover the native renderer after a user page fault.
kernel-browser: kernel-image
    @{{ runner }} kernel-browser

# Build the working tree's configuration into the Nix store and keep a link to it.
[positional-arguments]
native-build generation="target/kernel/current":
    @{{ runner }} native-build "$@"

# Boot a native generation, with an optional previous generation in the firmware menu.
[positional-arguments]
native-boot generation previous="":
    @{{ runner }} native-boot "$@"

# Verify the Calculator or launcher in a built native generation over USB input.
[positional-arguments]
native-check generation expected:
    @{{ runner }} native-check "$@"

# Build the optional Linux compute worker.
compute-worker:
    @{{ runner }} compute-worker

# compare batched checksums and their tree reduction on CPU and Vulkan
kernel-gpu-checksums: (test "kernel-gpu-checksums")

# compare the CPU image-mask reply with separable and fused Vulkan replies
system-gpu-compute: (test "system-gpu-compute")

# Measure CPU and Vulkan wall times and show the route selected from recent history.
[positional-arguments]
compute-benchmark profile="standard": compute-worker
    @{{ runner }} compute-benchmark "$@"

# Compare decoded-frame readiness and compositor presentation on a live Wayland display.
[positional-arguments]
video-presentation-benchmark clip: compute-worker
    @{{ runner }} video-presentation-benchmark "$@"

# Regenerate the browser extension's packaged WebAssembly with rustup's wasm32-unknown-unknown target.
browser-pad-wasm:
    @{{ runner }} browser-pad-wasm

# Exercise the controller through virtual input devices inside a VM.
live: (test "e2e-live")

# `--features` because the emulator reads the profiles, and reading one is
# behind `console-input-gamepad/read` -- off by default so the handheld does not
# compile a YAML parser for a program it never runs. `console-emulate` says it
# needs the feature rather than quietly not existing without it.

# a Legion Go on this machine, to press
emulate:
    @cargo x alone cargo run --quiet --features console-input-gamepad/read --bin console-emulate

# Exercise component interactions and the controller emulator.
checks: (test "integration")

# Run the desktop scenarios against a compositor inside a VM.
desktop-checks: (test "e2e-desktop")

# Run the panel scenarios inside a VM, optionally by test-name substring.
panel-checks name="": (test "e2e-panels" name)

# Measure the status bar, browser toolbar, drawing, launcher search, Files and Calculator.
[positional-arguments]
bench-ui *arguments:
    @{{ runner }} bench-ui "$@"

# what those would do to the device
device-checks:
    cargo run --quiet --bin console-check -- --stage device --dry-run

# and what the whole tier would do, emulator second opinions and all
device-checks-all:
    cargo run --quiet --bin console-check -- --stage device --dry-run --all

# open the experimental browser window on this desktop
[positional-arguments]
browser *arguments:
    @{{ runner }} browser "$@"

# Run the Chromium browser fixture inside a VM.
browser-check: (test "browser-chromium")

[private]
browser-servo-build:
    @{{ runner }} browser-servo-build

[positional-arguments]
browser-servo *arguments: browser-servo-build
    @{{ runner }} browser-servo "$@"

# Run the experimental Servo fixture inside a VM.
browser-servo-check: (test "browser-servo")

# Run the packaged Chromium and WPE renderers in the Console window.
[positional-arguments]
browser-webkit *arguments:
    @nix run .#browser -- "$@"

# Run the Chromium and WebKit renderer switch inside a VM.
browser-webkit-check: (test "browser")

# the device's desktop here, in a window
desktop:
    @cargo x alone cargo run --quiet --bin console-desktop -- run

# a picture of it, at the device's size
shot:
    @{{ runner }} shot

# The device compiles it, out of the tree `just deploy` pushed there, so this
# describes whatever was last deployed. Deploy first if that matters.

# write down the real devices again
capture:
    @{{ runner }} capture

# What the device measured for itself in its last check run, carried back here
# so a machine that has never been checked still gets a bar shaped like the run
# rather than one that counts. The device has to have run the checks at least
# once; `console-check --stage device --yes` is what writes the file.

# take the check lengths off the device again
lengths:
    @{{ runner }} lengths

# what deploying would change, changing nothing
check:
    cargo run --quiet --bin console-deploy -- --check

# put this on the device and apply it
deploy:
    cargo run --quiet --bin console-deploy

# put this on the device and apply it, asking nothing of the tests
release:
    cargo run --quiet --bin console-deploy -- --untested

# Push a new hypr-console release from VERSION [SOURCE-REVISION] (default HEAD).
[positional-arguments]
publish version *arguments:
    @{{ runner }} publish "$@"

# Tag a public commit with VERSION [PUBLIC-REVISION] (default origin/main).
[positional-arguments]
release-tag version *arguments:
    @{{ runner }} release-tag "$@"

# move a device still called legion over, once
migrate:
    cargo run --quiet --bin console-migrate

# take what was changed on the device back
pull:
    cargo run --quiet --bin console-pull

clean:
    @{{ runner }} clean
