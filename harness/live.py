"""The daemon as a program, against devices the kernel really made.

The fast tier runs a daemon in this process against a world that is not this
machine's. It answers what the daemon decides. It cannot answer whether the
devices the emulator builds are the ones the daemon goes looking for, because
in that tier the daemon is handed them.

This is the other half, and it is the whole path: uinput devices built from the
capture of the real four, the daemon started as its own program with nothing
told to it, and what comes out read back off a device the kernel published.

Nothing reaches the desktop you are sitting at. The daemon's output device is
grabbed the moment it appears, and a grabbed device delivers to the one that
grabbed it and to nothing else.
"""

import os
import select
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import evdev

from emulator.go import LegionGo
from emulator.targets import Devices

REPO = Path(__file__).resolve().parent.parent
BIN = REPO / "files/usr/local/bin"
FAKEBIN = Path(__file__).resolve().parent / "fakebin"


def uinput_is_open():
    """Whether this user can make a device at all."""
    try:
        node = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
    except OSError:
        return False
    os.close(node)
    return True


def wait_for(name, seconds=5.0, since=None):
    """The device a daemon just made, by the name it gave it."""
    known = since or set()
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        for path in evdev.list_devices():
            if path in known:
                continue
            try:
                device = evdev.InputDevice(path)
            except OSError:
                continue
            if device.name == name:
                return device
            device.close()
        time.sleep(0.02)
    return None


class Running:
    """A daemon, the devices it reads, and the device it writes."""

    def __init__(self, daemon="stick-scroll", profile="desktop"):
        self.devices = Devices()
        self.go = LegionGo(devices=self.devices, profile=profile)
        self.here = tempfile.TemporaryDirectory(prefix="legion-live-")
        self.ran_at = Path(self.here.name) / "ran"
        self.ran_at.touch()

        was = set(evdev.list_devices())
        environment = dict(os.environ)
        environment.update(
            PATH="%s:%s" % (FAKEBIN, environment.get("PATH", "")),
            LEGION_RAN=str(self.ran_at),
            LEGION_PAD=self.devices.path("pad"),
            LEGION_KEYS=self.devices.path("keyboard"),
            LEGION_TOUCHPAD=self.devices.path("touchpad"),
            PYTHONUNBUFFERED="1",
        )
        self.process = subprocess.Popen(
            [sys.executable, str(BIN / daemon)],
            env=environment, stderr=subprocess.PIPE, text=True)
        self.out = wait_for(daemon, since=was)
        if self.out is not None:
            self.out.grab()

    def settle(self, seconds=0.25):
        """Let the daemon get round to what it was sent."""
        time.sleep(seconds)
        return self

    def events(self, seconds=0.3):
        """Everything the daemon wrote, read off its own device."""
        out = []
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            ready, _, _ = select.select([self.out.fd], [], [],
                                        max(0.0, until - time.monotonic()))
            if not ready:
                break
            for event in self.out.read():
                if event.type != evdev.ecodes.EV_SYN:
                    out.append((event.type, event.code, event.value))
        return out

    def total(self, etype, code, seconds=0.3):
        return sum(v for t, c, v in self.events(seconds) if t == etype and c == code)

    @property
    def commands(self):
        """Every program the daemon started, as the name and its arguments."""
        return [line.split("\t")
                for line in self.ran_at.read_text().splitlines() if line]

    @property
    def names(self):
        return [command[0] for command in self.commands]

    @property
    def said(self):
        """What the daemon has printed about itself so far."""
        return self._said

    def close(self):
        if self.out is not None:
            try:
                self.out.ungrab()
            except OSError:
                pass
            self.out.close()
        self.process.terminate()
        try:
            self._said = self.process.communicate(timeout=5)[1]
        except subprocess.TimeoutExpired:
            self.process.kill()
            self._said = self.process.communicate()[1]
        self.devices.close()
        self.here.cleanup()

    _said = ""

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
