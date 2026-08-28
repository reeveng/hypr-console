"""The devices a Legion Go publishes, made again somewhere else.

InputPlumber grabs the physical controller and publishes three devices of its
own: a pad, a keyboard and a mouse. Those three, plus the controller's
touchpad, which InputPlumber never touches, are everything the desktop's
daemons read. Nothing reads the physical controller, so nothing here pretends
to be one.

What they are is not invented. `tools/capture-devices` wrote down the real ones
on the machine itself, down to the range of every axis, and this builds devices
from that. The one property that cannot be captured, and matters most, is that
a device made through uinput has no physical location: that empty `phys` is the
only thing telling the pad InputPlumber published apart from the pad a person
is holding, and it is how the daemons tell them apart too.
"""

import json
from pathlib import Path

from evdev import AbsInfo, UInput, ecodes as e

FIXTURES = Path(__file__).with_name("fixtures") / "devices.json"

# What each captured device is for.
ROLES = {
    "Microsoft X-Box One Elite 2 pad": "pad",
    "InputPlumber Keyboard": "keyboard",
    "InputPlumber Mouse": "mouse",
    "  Legion Controller  Touchpad": "touchpad",
}


def descriptors(path=FIXTURES):
    """Every captured device, by the part it plays."""
    captured = json.loads(Path(path).read_text())
    return {ROLES[d["name"]]: d for d in captured if d["name"] in ROLES}


def _events(descriptor):
    """A capture, as the argument uinput wants.

    Force feedback is left out. Nothing here reads it, and a uinput device that
    claims it has to answer for effects it was never asked to play.
    """
    events = {}
    for kind, codes in descriptor["capabilities"].items():
        if kind == "EV_FF":
            continue
        if kind == "EV_ABS":
            events[e.EV_ABS] = [
                (a["code"], AbsInfo(value=0, min=a["min"], max=a["max"],
                                    fuzz=a["fuzz"], flat=a["flat"],
                                    resolution=a["resolution"]))
                for a in codes
            ]
        else:
            events[getattr(e, kind)] = list(codes)
    return events


class Devices:
    """The four of them, open at once, for as long as this is held."""

    def __init__(self, path=FIXTURES, roles=None):
        self.descriptors = descriptors(path)
        if roles is not None:
            self.descriptors = {r: d for r, d in self.descriptors.items()
                                if r in roles}
        self.devices = {}
        for role, descriptor in self.descriptors.items():
            self.devices[role] = UInput(
                _events(descriptor),
                name=descriptor["name"],
                vendor=descriptor["vendor"],
                product=descriptor["product"],
                version=descriptor["version"],
                bustype=descriptor["bustype"],
                phys=descriptor["phys"],
                input_props=descriptor["properties"] or None,
            )

    def path(self, role):
        """Where the kernel put it, which is how a daemon is pointed at it."""
        return self.devices[role].device.path

    def paths(self):
        return {role: self.path(role) for role in self.devices}

    def emit(self, role, etype, code, value, syn=True):
        device = self.devices[role]
        device.write(etype, code, value)
        if syn:
            device.syn()

    def syn(self, role):
        self.devices[role].syn()

    def close(self):
        for device in self.devices.values():
            device.close()
        self.devices = {}

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
