"""X shows the keyboard, and X puts it away."""

FEATURE = "keyboard"
SINCE = "2026-08-25"


def device(pad, seen):
    if seen.keyboard():
        pad.press("x")
        seen.settle(1.5)
    pad.press("x")
    seen.settle(1.5)
    assert seen.keyboard(), "the keyboard did not come up"
    pad.press("x")
    seen.settle(1.5)
    assert not seen.keyboard(), "the keyboard would not go away"
