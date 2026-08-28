"""A world of input devices that exist only inside one test.

The daemons find their devices by asking evdev what is plugged in. That is the
right way round on the machine and the wrong way round in a test: it needs
/dev/uinput, root, and a kernel that will then deliver whatever comes out to
whatever has focus. So the same daemons are run here against a stand-in evdev,
with devices built from the same capture the real emulator uses. The daemon is
not modified and does not know.

What this gives that the real thing cannot is a clock. Time is a number this
holds, so a stick held for exactly one second scrolls exactly as far as the
arithmetic says, every run, on any machine.
"""

import sys
from types import ModuleType

import evdev as real_evdev
from evdev import AbsInfo, InputEvent, ecodes as e

from emulator.targets import descriptors


class Stop(BaseException):
    """Enough. Raised through the daemon's loop to end it.

    A BaseException on purpose: the daemons catch OSError around every read,
    because a device really does go away when a profile is switched, and a
    plain exception would be swallowed by that.
    """


class FakeDevice:
    """One device, and the events waiting on it.

    Opening a device gives back a handle, not the device. Closing a handle
    closes nothing else: on the machine a daemon opens the same device over
    and over while it hunts for the one it wants, and closes each that turns
    out to be the wrong one. Modelling a close as the end of the device made
    the search close the very device it had just found.
    """

    def __init__(self, path, descriptor):
        self.path = path
        self.name = descriptor["name"]
        self.phys = descriptor["phys"]
        self.uniq = descriptor["uniq"]
        self.capability = descriptor["capabilities"]
        self.queue = []
        self.plugged = True

    def unplug(self):
        """What a profile switch does: the device is gone mid-read."""
        self.plugged = False
        self.queue = []

    def plug(self):
        self.plugged = True


class Handle:
    """One open of a device. What a daemon actually holds."""

    def __init__(self, device):
        self.device = device
        self.closed = False
        self.grabbed = False

    @property
    def path(self):
        return self.device.path

    @property
    def name(self):
        return self.device.name

    @property
    def phys(self):
        return self.device.phys

    @property
    def uniq(self):
        return self.device.uniq

    def capabilities(self, absinfo=False, verbose=False):
        out = {}
        for kind, codes in self.device.capability.items():
            if kind == "EV_ABS":
                out[e.EV_ABS] = [(a["code"], self._absinfo(a) if absinfo else None)
                                 for a in codes]
            else:
                out[getattr(e, kind)] = list(codes)
        return out

    @staticmethod
    def _absinfo(axis):
        return AbsInfo(value=0, min=axis["min"], max=axis["max"],
                       fuzz=axis["fuzz"], flat=axis["flat"],
                       resolution=axis["resolution"])

    def absinfo(self, code):
        for axis in self.device.capability.get("EV_ABS", []):
            if axis["code"] == code:
                return self._absinfo(axis)
        raise OSError("no axis %d on %s" % (code, self.name))

    def _live(self):
        if self.closed or not self.device.plugged:
            raise OSError("%s has gone" % self.name)

    def read(self):
        self._live()
        if not self.device.queue:
            raise BlockingIOError
        events, self.device.queue = self.device.queue, []
        return events

    def read_one(self):
        self._live()
        return self.device.queue.pop(0) if self.device.queue else None

    def grab(self):
        self._live()
        self.grabbed = True

    def ungrab(self):
        self.grabbed = False

    def close(self):
        self.closed = True


class Output:
    """A device a daemon made, and everything it wrote to it."""

    def __init__(self, name, events, **rest):
        self.name = name
        self.declared = events
        self.rest = rest
        self.written = []
        self.frames = []
        self._frame = []

    def write(self, etype, code, value):
        self.written.append((etype, code, value))
        self._frame.append((etype, code, value))

    def syn(self):
        if self._frame:
            self.frames.append(self._frame)
            self._frame = []

    def close(self):
        pass

    def of_type(self, etype, code=None):
        return [w for w in self.written
                if w[0] == etype and (code is None or w[1] == code)]

    def total(self, etype, code):
        return sum(value for _, _, value in self.of_type(etype, code))


class World:
    """Every device there is, and it is only these.

    Stands in for both halves at once: the emulator writes into it as if it
    were a set of uinput devices, and the daemon reads out of it as if it were
    the input subsystem.
    """

    def __init__(self, roles=("pad", "keyboard", "mouse", "touchpad")):
        self.descriptors = {r: d for r, d in descriptors().items() if r in roles}
        self.devices = {}
        self.log = []
        self.outputs = []
        for number, role in enumerate(sorted(self.descriptors)):
            self.devices[role] = FakeDevice("/dev/input/event%d" % number,
                                            self.descriptors[role])

    # what the emulator writes into

    def path(self, role):
        return self.devices[role].path

    def paths(self):
        return {role: self.path(role) for role in self.devices}

    def emit(self, role, etype, code, value, syn=True):
        device = self.devices[role]
        device.queue.append(InputEvent(0, 0, etype, code, value))
        self.log.append((role, etype, code, value))
        if syn:
            self.syn(role)

    def syn(self, role):
        self.devices[role].queue.append(
            InputEvent(0, 0, e.EV_SYN, e.SYN_REPORT, 0))

    def close(self):
        for device in self.devices.values():
            device.unplug()

    # what the daemon reads out of

    def list_devices(self):
        return [d.path for d in self.devices.values() if d.plugged]

    def open(self, path):
        for device in self.devices.values():
            if device.path == path:
                if not device.plugged:
                    raise OSError("no such device")
                return Handle(device)
        raise OSError("no such device: %s" % path)

    def uinput(self, events=None, name="py-evdev-uinput", **rest):
        out = Output(name, events, **rest)
        self.outputs.append(out)
        return out

    @property
    def output(self):
        """The one device the daemon made, when there is only one."""
        if len(self.outputs) != 1:
            raise AssertionError("expected one output device, got %d"
                                 % len(self.outputs))
        return self.outputs[0]


def install(world):
    """Put this world where `import evdev` will find it.

    Returns what was there before, so a test can put it back.
    """
    module = ModuleType("evdev")
    module.list_devices = world.list_devices
    module.InputDevice = world.open
    module.UInput = world.uinput
    module.ecodes = e
    module.AbsInfo = AbsInfo
    module.InputEvent = InputEvent
    module.util = real_evdev.util
    was = sys.modules.get("evdev")
    sys.modules["evdev"] = module
    return was
