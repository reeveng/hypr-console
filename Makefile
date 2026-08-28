# The loop: change something, run `make test`, and only then `make deploy`.

PYTHON ?= .venv/bin/python
HOST ?= root@handheld

# The daemons are loaded from the tree they are installed from, and a stray
# __pycache__ beside them is a file the manifest does not carry.
export PYTHONDONTWRITEBYTECODE = 1

.PHONY: setup test rust theme garden fast live emulate capture check deploy pull clean

setup: .venv/bin/pytest            ## everything the tests need, in a venv here

.venv/bin/pytest:
	python3 -m venv .venv
	.venv/bin/pip install --quiet --upgrade pip
	.venv/bin/pip install --quiet evdev pycairo pytest pyyaml

theme:                             ## write the palette into every file that spends it
	cargo run --quiet --release --bin legion-theme

garden:                            ## draw the wallpaper again, out of the palette
	python3 tools/legion-garden

test: setup rust                   ## every test that can run on this machine
	$(PYTHON) -m pytest -q

rust:                              ## everything written in rust, and its tests
	cargo test --quiet --workspace

fast: setup                        ## the tier that needs no devices at all
	$(PYTHON) -m pytest -q --ignore=tests/test_live.py

live: setup                        ## the tier that makes real input devices
	$(PYTHON) -m pytest -q tests/test_live.py

emulate: setup                     ## a Legion Go on this machine, to press
	$(PYTHON) tools/legion-emulate

checks:  setup                     ## every feature, tried again, here
	$(PYTHON) tools/legion-check

device-checks: setup               ## what those would do to the device
	$(PYTHON) tools/legion-check --stage device --dry

desktop:                           ## the device's desktop here, in a window
	tools/legion-desktop run

shot:                              ## a picture of it, at the device's size
	tools/legion-desktop shot desktop.png
	@echo "desktop.png"

capture: setup                     ## write down the real devices again
	scp tools/capture-devices $(HOST):/tmp/capture-devices
	ssh $(HOST) python3 /tmp/capture-devices > emulator/fixtures/devices.json
	git diff --stat emulator/fixtures/devices.json

check: setup                       ## what deploying would change, changing nothing
	tools/legion-deploy --check

deploy: setup                      ## put this on the device and apply it
	tools/legion-deploy

pull:                              ## take what was changed on the device back
	tools/legion-pull

clean:
	rm -rf .venv .pytest_cache .stage
	find . -name __pycache__ -type d -prune -exec rm -rf {} +
