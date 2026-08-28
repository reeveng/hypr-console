"""The right stick turns the wheel, and how far it is pushed is how fast."""

FEATURE = "scroll"
SINCE = "2026-08-24"


def here(pad, seen):
    from evdev import ecodes as e
    pad.stick("right-stick", 0.0, -1.0)
    seen.settle(turns=12)
    up = seen.wrote(e.EV_REL, e.REL_WHEEL)
    assert up > 0, "the wheel did not turn"

    pad.stick("right-stick", 0.0, 1.0)
    seen.settle(turns=12)
    assert seen.wrote(e.EV_REL, e.REL_WHEEL) < up, \
        "pushing the other way did not turn it back"


def device(pad, seen):
    """Not asserted on the machine. What the wheel did is a thing the window
    under the pointer knows and nothing else can be asked, so this is a check
    the emulator answers and the device cannot."""
    raise NotImplementedError("nothing on the device can see a page scroll")
