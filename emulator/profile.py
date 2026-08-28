"""An InputPlumber profile, read as what each button turns into.

A profile is the whole of what a button means: the compositor is not in the
loop and the daemons only see what came out of here. So a change to what the
device does is a change to one of these files, and anything that wants to know
what the device does, the guide and the tests included, reads them rather than
being told twice.
"""

import re
from pathlib import Path

import yaml

from . import vocabulary


class Target:
    """One thing a press turns into: a key, a mouse button, a pad button."""

    def __init__(self, kind, name, detail=None):
        self.kind = kind          # key, mouse-button, mouse-motion,
        self.name = name          # gamepad-button, gamepad-axis, gamepad-trigger
        self.detail = detail or {}

    @property
    def code(self):
        """The kernel code this arrives as, where there is one."""
        if self.kind == "key":
            return vocabulary.key_code(self.name)
        if self.kind == "mouse-button":
            return vocabulary.MOUSE_CODES[self.name]
        if self.kind == "gamepad-button":
            return vocabulary.GAMEPAD_CODES[self.name]
        return None

    def __repr__(self):
        return "Target(%s %s)" % (self.kind, self.name)

    def __eq__(self, other):
        return (isinstance(other, Target) and self.kind == other.kind
                and self.name == other.name)

    def __hash__(self):
        return hash((self.kind, self.name))


class Mapping:
    """One entry: what was pressed, what it becomes, and what it is called."""

    def __init__(self, label, source_kind, source_name, targets,
                 direction=None, deadzone=None):
        self.label = label
        self.source_kind = source_kind    # button, axis, trigger
        self.source_name = source_name    # the profile's name for it
        self.targets = targets
        self.direction = direction
        self.deadzone = deadzone

    @property
    def button(self):
        """What is written on the button, where the source is one."""
        if self.source_kind != "button":
            return None
        return vocabulary.spoken_for(self.source_name)

    @property
    def does(self):
        """The half of the label after the dash: what it does, in words.

        Every mapping is named "Button - what it does", and the guide on the
        device prints exactly this. A mapping that says nothing about what it
        does reads as nothing rather than being quietly dropped.
        """
        _, _, does = self.label.partition(" - ")
        return does.strip()

    def __repr__(self):
        return "Mapping(%r)" % self.label


def _targets(entries):
    out = []
    for entry in entries or []:
        if not isinstance(entry, dict):
            continue
        for kind, body in entry.items():
            if kind == "keyboard":
                out.append(Target("key", body))
            elif kind == "mouse":
                if "button" in body:
                    out.append(Target("mouse-button", body["button"]))
                elif "motion" in body:
                    out.append(Target("mouse-motion", "Motion", body["motion"]))
            elif kind == "gamepad":
                if "button" in body:
                    out.append(Target("gamepad-button", body["button"]))
                elif "axis" in body:
                    out.append(Target("gamepad-axis", body["axis"]["name"],
                                      body["axis"]))
                elif "trigger" in body:
                    out.append(Target("gamepad-trigger", body["trigger"]["name"],
                                      body["trigger"]))
    return out


class Profile:
    """What the pad is, while this profile is loaded."""

    def __init__(self, path):
        self.path = Path(path)
        raw = yaml.safe_load(self.path.read_text())
        self.name = raw.get("name", self.path.stem)
        self.description = (raw.get("description") or "").strip()
        self.target_devices = raw.get("target_devices") or []
        self.mappings = []
        for entry in raw.get("mapping") or []:
            source = (entry.get("source_event") or {}).get("gamepad") or {}
            label = entry.get("name", "")
            targets = _targets(entry.get("target_events"))
            if "button" in source:
                self.mappings.append(
                    Mapping(label, "button", source["button"], targets))
            elif "axis" in source:
                axis = source["axis"]
                self.mappings.append(
                    Mapping(label, "axis", axis["name"], targets,
                            direction=axis.get("direction"),
                            deadzone=axis.get("deadzone")))
            elif "trigger" in source:
                trigger = source["trigger"]
                self.mappings.append(
                    Mapping(label, "trigger", trigger["name"], targets,
                            deadzone=trigger.get("deadzone")))

    def for_button(self, spoken):
        """Every mapping a named button has here, usually none or one."""
        name = vocabulary.button_name(spoken)
        return [m for m in self.mappings
                if m.source_kind == "button" and m.source_name == name]

    def targets_of(self, spoken):
        """What pressing that button turns into here."""
        return [t for m in self.for_button(spoken) for t in m.targets]

    def for_axis(self, spoken, direction=None):
        name = vocabulary.AXES.get(spoken, spoken)
        return [m for m in self.mappings
                if m.source_kind == "axis" and m.source_name == name
                and (direction is None or m.direction in (None, direction))]

    def for_trigger(self, spoken):
        name = vocabulary.TRIGGERS.get(spoken, spoken)
        return [m for m in self.mappings
                if m.source_kind == "trigger" and m.source_name == name]

    def __repr__(self):
        return "Profile(%s, %d mappings)" % (self.name, len(self.mappings))


PROFILE_DIR = "files/etc/inputplumber/profiles"


def load_all(root):
    """Every profile in a checkout, by the word controller-profile takes."""
    root = Path(root)
    return {p.stem: Profile(p)
            for p in sorted((root / PROFILE_DIR).glob("*.yaml"))}
