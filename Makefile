# The loop: change something, run `make test`, and only then `make deploy`.
#
# Nothing has to be remembered before a deploy. `make ready` is the whole list
# and `tools/console-deploy` runs it itself, so what reaches the device has
# passed it whether or not anybody thought to.

# The device, which only somebody with one can name. The tools read it too, and
# say so if it is not set.
HOST ?= $(CONSOLE_HOST)

.PHONY: test ready theme garden sky live emulate checks device-checks desktop shot capture check deploy migrate pull clean

theme:                             ## write the palette into every file that spends it
	cargo run --quiet --release --bin console-theme

garden:                            ## draw the wallpaper again, out of the palette
	cargo run --quiet --release --bin console-garden

sky:                               ## press the wallpapers the table names
	cargo run --quiet --release --bin sky-press

test:                              ## every test that can run on this machine
	cargo test --quiet --workspace

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
#                   read about. A third of a second for sixteen of them,
#                   because no machine takes part: the real profiles, the real
#                   pad, and the daemon running in this process.
#
# `make emulate` is deliberately not here. That one runs the features against a
# nested desktop of its own, which is minutes and a compositor, and a gate
# somebody starts dreading is a gate somebody starts going around. The device
# tier is not here either, for a better reason: it is somebody's machine, and
# it belongs at the end of a deploy rather than before one.
ready:                             ## everything that must hold before a deploy
	cargo test --quiet --locked --workspace
	cargo clippy --quiet --locked --workspace --all-targets -- -D warnings
	cargo run --quiet --bin console-check

live:                              ## the tier that makes real input devices, if it can
	cargo test --quiet -p console-controller --test really_running -- --nocapture

emulate:                           ## a Legion Go on this machine, to press
	cargo run --quiet --bin console-emulate

checks:                            ## every feature, tried again, here
	cargo run --quiet --bin console-check

device-checks:                     ## what those would do to the device
	cargo run --quiet --bin console-check -- --stage device --dry

desktop:                           ## the device's desktop here, in a window
	cargo run --quiet --bin console-desktop -- run

shot:                              ## a picture of it, at the device's size
	cargo run --quiet --bin console-desktop -- shot desktop.png
	@echo "desktop.png"

# The device compiles it, out of the tree `make deploy` pushed there, so this
# describes whatever was last deployed. Deploy first if that matters.
capture:                           ## write down the real devices again
	ssh $(HOST) cargo run --release --locked --quiet \
	  --manifest-path /etc/console/Cargo.toml --bin capture-devices \
	  > crates/console-pad/fixtures/devices.json
	git diff --stat crates/console-pad/fixtures/devices.json

check:                             ## what deploying would change, changing nothing
	tools/console-deploy --check

deploy:                            ## put this on the device and apply it
	tools/console-deploy

migrate:                           ## move a device still called legion over, once
	tools/console-migrate

pull:                              ## take what was changed on the device back
	tools/console-pull

clean:
	rm -rf .stage target
