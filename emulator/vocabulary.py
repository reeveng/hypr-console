"""The names of the things on the front of the machine, and what they are.

Two vocabularies meet here. One is the Legion Go's, the names a person uses for
what their thumbs are on, and the names InputPlumber's profiles are written in.
The other is the kernel's, the codes that come out of a device. Everything that
has to cross between them crosses here, once, so that a button called X in a
profile and a button called X in a test are the same button.

The face buttons are the trap. On this device the one labelled X is BTN_NORTH
and the one labelled Y is BTN_WEST, which is not what either name suggests and
not what most pads do. It is written down here rather than remembered.
"""

from evdev import ecodes as e

# What a person calls it, and what InputPlumber's profiles call it.
#
# The left column is what you would say out loud. The right is the `button:`
# name in a profile, which is the only name InputPlumber answers to.
BUTTONS = {
    "a": "South",
    "b": "East",
    "x": "North",
    "y": "West",
    "dpad-up": "DPadUp",
    "dpad-down": "DPadDown",
    "dpad-left": "DPadLeft",
    "dpad-right": "DPadRight",
    "l1": "LeftBumper",
    "r1": "RightBumper",
    "l3": "LeftStick",
    "r3": "RightStick",
    "menu": "Start",
    "view": "Select",
    "legion-left": "Guide",
    "legion-right": "QuickAccess",
    "keyboard": "Keyboard",
    "left-paddle-top": "LeftPaddle1",
    "left-paddle-bottom": "LeftPaddle2",
    "right-paddle-top": "RightPaddle1",
    "right-paddle-bottom": "RightPaddle2",
}

# The two analogue sticks and the two triggers, under the names a profile uses.
AXES = {"left-stick": "LeftStick", "right-stick": "RightStick"}
TRIGGERS = {"l2": "LeftTrigger", "r2": "RightTrigger"}

# What a profile can send, and the code it arrives as.
#
# Only what the profiles actually target is here. A name a profile does not use
# is a name nothing has confirmed, and a guess in this table would be a guess
# about which button a thumb is on.
GAMEPAD_CODES = {
    "South": e.BTN_SOUTH,
    "East": e.BTN_EAST,
    "North": e.BTN_NORTH,
    "West": e.BTN_WEST,
    "Start": e.BTN_START,
    "Select": e.BTN_SELECT,
    "Guide": e.BTN_MODE,
    "LeftBumper": e.BTN_TL,
    "RightBumper": e.BTN_TR,
    "LeftStick": e.BTN_THUMBL,
    "RightStick": e.BTN_THUMBR,
}

MOUSE_CODES = {
    "Left": e.BTN_LEFT,
    "Right": e.BTN_RIGHT,
    "Middle": e.BTN_MIDDLE,
}

# A stick, as the pair of axes it arrives on.
AXIS_CODES = {
    "LeftStick": (e.ABS_X, e.ABS_Y),
    "RightStick": (e.ABS_RX, e.ABS_RY),
}

# A trigger, as the axis it arrives on. Both also report as a button when they
# are pulled far enough, which is how the daemon learns that L2 is being held.
TRIGGER_CODES = {"LeftTrigger": e.ABS_Z, "RightTrigger": e.ABS_RZ}
TRIGGER_BUTTONS = {"LeftTrigger": e.BTN_TL2, "RightTrigger": e.BTN_TR2}


def key_code(name):
    """`KeyPageUp` as the kernel's KEY_PAGEUP.

    InputPlumber writes a key as Key followed by its name in the shape a
    person would write it. The kernel writes the same name in capitals. That
    is the whole of the difference, for every key any profile here sends.
    """
    if not name.startswith("Key"):
        raise KeyError("not a key name: %r" % name)
    code = getattr(e, "KEY_" + name[3:].upper(), None)
    if code is None:
        raise KeyError("no such key: %r" % name)
    return code


def button_name(spoken):
    """`x` as `North`, which is what a profile calls it."""
    try:
        return BUTTONS[spoken]
    except KeyError:
        raise KeyError("no button called %r; try one of %s"
                       % (spoken, ", ".join(sorted(BUTTONS)))) from None


def spoken_for(profile_name):
    """`North` as `x`, which is what is written on the button."""
    for spoken, name in BUTTONS.items():
        if name == profile_name:
            return spoken
    return profile_name
