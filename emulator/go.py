"""A Legion Go you can press.

    go = LegionGo()
    go.load_profile("desktop")
    go.press("right-paddle-top")     # the window closes
    go.hold("l2"); go.press("l1")    # the window moves a workspace left

What a press turns into is decided the same way the real machine decides it,
by the profile that is loaded, so this is a test of the profile as much as of
whatever is reading the other end. Loading a different profile changes what the
same press means, exactly as `controller-profile` does on the device.

Two things here are a model of InputPlumber rather than a recording of it:

  * A button with no mapping in the loaded profile is passed through to the
    pad untouched. That is what an empty profile means, and keyboard.yaml,
    which has no mappings at all and is documented as passing everything
    through, is the case that says so.
  * An event can only reach a device the profile lists in `target_devices`.
    InputPlumber builds the targets a profile names and destroys the rest, so
    a mapping that sends a pad button from a profile with no pad in it sends
    it nowhere.

The touchpad is not in this loop at all. InputPlumber cannot translate it and
the compositor makes it absolute, so on the device it is left alone and read
directly. It is left alone here too.
"""

import time
from pathlib import Path

from evdev import ecodes as e

from . import vocabulary
from .profile import load_all
from .targets import Devices

REPO = Path(__file__).resolve().parent.parent

# Which target device a profile has to list before an event can reach it.
NEEDS = {"key": "keyboard", "mouse-button": "mouse", "mouse-motion": "mouse",
         "gamepad-button": "xbox-elite", "gamepad-axis": "xbox-elite",
         "gamepad-trigger": "xbox-elite"}

# The role each of those is published as.
ROLE = {"keyboard": "keyboard", "mouse": "mouse", "xbox-elite": "pad"}

PRESS_SECONDS = 0.02


class LegionGo:
    """The front of the machine, and the devices behind it."""

    def __init__(self, devices=None, root=REPO, profile="desktop"):
        self.profiles = load_all(root)
        self.devices = devices if devices is not None else Devices()
        self.profile = None
        self.held = set()
        self.load_profile(profile)

    # ---------------------------------------------------------------- profiles

    def load_profile(self, name):
        """What `controller-profile <name>` does, without the bus."""
        if name not in self.profiles:
            raise KeyError("no profile called %r; there is %s"
                           % (name, ", ".join(sorted(self.profiles))))
        self.profile = self.profiles[name]

    @property
    def profile_name(self):
        return self.profile.path.stem

    def _publishes(self, target_device):
        return target_device in self.profile.target_devices

    # ----------------------------------------------------------------- buttons

    def down(self, spoken):
        """Press and keep pressing."""
        self.held.add(spoken)
        self._button(spoken, 1)

    def up(self, spoken):
        """Let go."""
        self.held.discard(spoken)
        self._button(spoken, 0)

    def press(self, spoken, seconds=PRESS_SECONDS):
        self.down(spoken)
        time.sleep(seconds)
        self.up(spoken)

    def hold(self, spoken):
        """Held until `release` or `release_all`. Reads better in a scenario."""
        self.down(spoken)

    def release(self, spoken):
        self.up(spoken)

    def release_all(self):
        for spoken in sorted(self.held):
            self.up(spoken)

    def _button(self, spoken, value):
        if spoken in vocabulary.TRIGGERS:
            # A trigger is an axis, and holding one is pulling it all the way.
            # Saying "hold l2" is what a person means, so it is what it does.
            self.trigger(spoken, 1.0 if value else 0.0)
            return
        name = vocabulary.button_name(spoken)
        mappings = self.profile.for_button(spoken)
        if not mappings:
            self._passthrough(name, value)
            return
        for mapping in mappings:
            for target in mapping.targets:
                self._send(target, value)

    def _passthrough(self, name, value):
        """No mapping: the press reaches the pad as itself, if there is a pad."""
        code = vocabulary.GAMEPAD_CODES.get(name)
        if code is None or not self._publishes("xbox-elite"):
            return
        self.devices.emit("pad", e.EV_KEY, code, value)

    def _send(self, target, value):
        needs = NEEDS.get(target.kind)
        if needs is None or not self._publishes(needs):
            return
        role = ROLE[needs]
        if role not in self.devices.devices:
            return
        code = target.code
        if code is not None:
            self.devices.emit(role, e.EV_KEY, code, value)

    # ------------------------------------------------------------------ sticks

    def stick(self, which, x=0.0, y=0.0):
        """Push a stick, each axis from -1 to 1. Up the screen is negative y."""
        name = vocabulary.AXES[which]
        if not self._publishes("xbox-elite"):
            return
        codes = vocabulary.AXIS_CODES[name]
        for code, amount in zip(codes, (x, y)):
            self.devices.emit("pad", e.EV_ABS, code, self._absolute(code, amount),
                              syn=False)
        self.devices.syn("pad")

    def centre(self, which):
        self.stick(which, 0.0, 0.0)

    def trigger(self, which, amount):
        """Pull a trigger, from 0 to 1."""
        name = vocabulary.TRIGGERS[which]
        if not self._publishes("xbox-elite"):
            return
        code = vocabulary.TRIGGER_CODES[name]
        info = self._absinfo("pad", code)
        value = int(round(info["min"] + amount * (info["max"] - info["min"])))
        self.devices.emit("pad", e.EV_ABS, code, value)

    def _absinfo(self, role, code):
        for axis in self.devices.descriptors[role]["capabilities"]["EV_ABS"]:
            if axis["code"] == code:
                return axis
        raise KeyError("%s has no axis %d" % (role, code))

    def _absolute(self, code, amount):
        info = self._absinfo("pad", code)
        span = max(abs(info["max"]), abs(info["min"])) or 1
        return int(round(max(-1.0, min(1.0, amount)) * span))

    # ---------------------------------------------------------------- touchpad

    def touch_down(self, x, y):
        self.devices.emit("touchpad", e.EV_KEY, e.BTN_TOUCH, 1, syn=False)
        self._touch_at(x, y)

    def touch_move(self, x, y):
        self._touch_at(x, y)

    def touch_up(self):
        self.devices.emit("touchpad", e.EV_KEY, e.BTN_TOUCH, 0)

    def touch_click(self, value):
        """The pad pressed in, which is a button of its own, not a tap."""
        self.devices.emit("touchpad", e.EV_KEY, e.BTN_0, value)

    def tap(self, x=512, y=512):
        self.touch_down(x, y)
        self.touch_up()

    def drag(self, start, end, steps=8, seconds=0.0):
        """A finger from one place to another, in as many reports."""
        (x0, y0), (x1, y1) = start, end
        self.touch_down(x0, y0)
        for step in range(1, steps + 1):
            self.touch_move(x0 + (x1 - x0) * step // steps,
                            y0 + (y1 - y0) * step // steps)
            if seconds:
                time.sleep(seconds / steps)
        self.touch_up()

    def _touch_at(self, x, y):
        self.devices.emit("touchpad", e.EV_ABS, e.ABS_X, int(x), syn=False)
        self.devices.emit("touchpad", e.EV_ABS, e.ABS_Y, int(y), syn=False)
        self.devices.syn("touchpad")

    # --------------------------------------------------------------------- raw

    def raw(self, role, etype, code, value):
        """Straight onto a device, with no profile in the way."""
        self.devices.emit(role, etype, code, value)

    def close(self):
        self.devices.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
